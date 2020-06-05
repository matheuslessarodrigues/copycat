use std::{
    io::{self, stdin, stdout, Read, Write},
    mem::size_of,
    ops::{Deref, DerefMut},
    ptr::{null_mut, NonNull},
    slice,
};

use atty::Stream;
use byteorder::{LittleEndian, WriteBytesExt};
use clipboard_win::{raw, Clipboard};
use winapi::{
    shared::windef::HDC,
    um::{
        minwinbase::LPTR,
        winbase::{LocalAlloc, LocalFree},
        wingdi::{
            GetDIBits, GetObjectW, BITMAP, BITMAPFILEHEADER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            DIB_RGB_COLORS, RGBQUAD,
        },
        winuser::{GetDC, ReleaseDC, CF_BITMAP},
    },
};

struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub bits: u8,
    pub bytes: Vec<u8>,
}

struct Dc(pub HDC);
impl Dc {
    pub fn new() -> Self {
        Self(unsafe { GetDC(null_mut()) })
    }
}
impl Drop for Dc {
    fn drop(&mut self) {
        unsafe { ReleaseDC(null_mut(), self.0) };
    }
}

struct LocalMemory<T>(NonNull<T>);
impl<T> LocalMemory<T> {
    pub fn new(size: usize) -> Option<Self> {
        let ptr = unsafe { LocalAlloc(LPTR, size) } as *mut _;
        Some(Self(NonNull::new(ptr)?))
    }

    pub fn as_ptr(&mut self) -> *mut T {
        self.0.as_ptr()
    }
}
impl<T> Drop for LocalMemory<T> {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0.as_ptr() as _) };
    }
}
impl<T> Deref for LocalMemory<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
}
impl<T> DerefMut for LocalMemory<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }
    }
}

fn get_clipboard_bitmap() -> Option<Bitmap> {
    let handle = raw::get_clipboard_data(CF_BITMAP).ok()?;
    let mut bitmap = BITMAP {
        bmType: 0,
        bmWidth: 0,
        bmHeight: 0,
        bmWidthBytes: 0,
        bmPlanes: 0,
        bmBitsPixel: 0,
        bmBits: null_mut(),
    };

    if unsafe {
        GetObjectW(
            handle.as_ptr(),
            size_of::<BITMAP>() as _,
            &mut bitmap as *mut BITMAP as _,
        )
    } == 0
    {
        return None;
    }

    let clr_bits = bitmap.bmPlanes * bitmap.bmBitsPixel;
    let clr_bits = if clr_bits == 1 {
        1
    } else if clr_bits <= 4 {
        4
    } else if clr_bits <= 8 {
        8
    } else if clr_bits <= 16 {
        16
    } else if clr_bits <= 24 {
        24
    } else {
        32
    };

    let mut info: LocalMemory<BITMAPINFO> = if clr_bits < 24 {
        LocalMemory::new(size_of::<BITMAPINFOHEADER>() + size_of::<RGBQUAD>() * (1 << clr_bits))?
    } else {
        LocalMemory::new(size_of::<BITMAPINFOHEADER>())?
    };

    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as _;
    info.bmiHeader.biWidth = bitmap.bmWidth;
    info.bmiHeader.biHeight = bitmap.bmHeight;
    info.bmiHeader.biPlanes = bitmap.bmPlanes;
    info.bmiHeader.biBitCount = bitmap.bmBitsPixel;
    info.bmiHeader.biCompression = BI_RGB;
    if clr_bits < 24 {
        info.bmiHeader.biClrUsed = 1 << clr_bits;
    }

    info.bmiHeader.biSizeImage =
        ((((info.bmiHeader.biWidth * clr_bits + 31) & !31) / 8) * info.bmiHeader.biHeight) as _;
    info.bmiHeader.biClrImportant = 0;

    let dc = Dc::new();
    let mut buf = Vec::with_capacity(info.bmiHeader.biSizeImage as _);
    buf.resize(buf.capacity(), 0);

    if unsafe {
        GetDIBits(
            dc.0,
            handle.as_ptr() as _,
            0,
            info.bmiHeader.biHeight as _,
            buf.as_mut_ptr() as _,
            info.as_ptr(),
            DIB_RGB_COLORS,
        )
    } == 0
    {
        return None;
    }

    let mut out = Vec::new();
    out.write_u16::<LittleEndian>(0x4d42).ok()?;
    out.write_u32::<LittleEndian>(
        size_of::<BITMAPFILEHEADER>() as u32
            + info.bmiHeader.biSize
            + info.bmiHeader.biClrUsed * size_of::<RGBQUAD>() as u32
            + info.bmiHeader.biSizeImage,
    )
    .ok()?;
    out.write_u16::<LittleEndian>(0).ok()?;
    out.write_u16::<LittleEndian>(0).ok()?;
    out.write_u32::<LittleEndian>(
        size_of::<BITMAPFILEHEADER>() as u32
            + info.bmiHeader.biSize
            + info.bmiHeader.biClrUsed * size_of::<RGBQUAD>() as u32,
    )
    .ok()?;

    let h = &info.bmiHeader;
    out.write_u32::<LittleEndian>(h.biSize).ok()?;
    out.write_i32::<LittleEndian>(h.biWidth).ok()?;
    out.write_i32::<LittleEndian>(h.biHeight).ok()?;
    out.write_u16::<LittleEndian>(h.biPlanes).ok()?;
    out.write_u16::<LittleEndian>(h.biBitCount).ok()?;
    out.write_u32::<LittleEndian>(h.biCompression).ok()?;
    out.write_u32::<LittleEndian>(h.biSizeImage).ok()?;
    out.write_i32::<LittleEndian>(h.biXPelsPerMeter).ok()?;
    out.write_i32::<LittleEndian>(h.biYPelsPerMeter).ok()?;
    out.write_u32::<LittleEndian>(h.biClrUsed).ok()?;
    out.write_u32::<LittleEndian>(h.biClrImportant).ok()?;

    let colors =
        unsafe { slice::from_raw_parts(info.bmiColors.as_ptr(), info.bmiHeader.biClrUsed as _) };
    for color in colors {
        out.push(color.rgbBlue);
        out.push(color.rgbGreen);
        out.push(color.rgbRed);
        out.push(color.rgbReserved);
    }

    out.write(&buf[..]).ok()?;

    Some(Bitmap {
        width: bitmap.bmWidth as _,
        height: bitmap.bmHeight as _,
        bits: clr_bits as _,
        bytes: out,
    })
}

fn handle_write<F, R>(block: F) -> Result<(), &'static str>
where
    F: FnOnce() -> io::Result<R>,
{
    match block() {
        Ok(_) => Ok(()),
        Err(_) => Err("could not write to stdout"),
    }
}

fn try_main() -> Result<(), &'static str> {
    let clipboard = Clipboard::new().map_err(|_| "could not open clipboard")?;

    if atty::isnt(Stream::Stdin) {
        let mut text = String::new();
        stdin()
            .lock()
            .read_to_string(&mut text)
            .map_err(|_| "could not read from stdin")?;
        let text = if text.ends_with("\r\n") {
            &text[..text.len() - 2]
        } else if text.ends_with('\n') {
            &text[..text.len() - 1]
        } else {
            &text[..]
        };

        clipboard
            .set_string(text)
            .map_err(|_| "could not set clipboard text")?;
    } else {
        let stdout = stdout();
        let mut stdout = stdout.lock();
        let mut text = String::new();
        if let Ok(()) = clipboard.get_string(&mut text) {
            handle_write(|| stdout.write(text.as_bytes()))?;
            if atty::is(Stream::Stdout) && !text.ends_with('\n') {
                handle_write(|| stdout.write_u8('\n' as _))?;
            }
        } else if let Some(bitmap) = get_clipboard_bitmap() {
            if atty::is(Stream::Stdout) {
                handle_write(|| {
                    write!(
                        stdout,
                        "bitmap: {}, {} ({} bits)\n",
                        bitmap.width, bitmap.height, bitmap.bits
                    )
                })?;
            } else {
                handle_write(|| stdout.write(&bitmap.bytes[..]))?;
            }
        } else {
            return Err("clipboard did not contain neither text nor bitmap");
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = try_main() {
        eprintln!("error: {}", error);
    }
}

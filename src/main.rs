use std::{
    io::{stdin, stdout, Read, Write},
    mem::size_of,
    ptr, slice,
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
            CreateCompatibleDC, GetDIBits, GetObjectW, BITMAP, BITMAPFILEHEADER, BITMAPINFO,
            BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, RGBQUAD,
        },
        winuser::{GetDC, ReleaseDC, CF_BITMAP},
    },
};

struct Dc(HDC);
impl Dc {
    fn new() -> Self {
        Self(unsafe { CreateCompatibleDC(GetDC(ptr::null_mut())) })
    }
}
impl Drop for Dc {
    fn drop(&mut self) {
        //dbg!("asdsad");
        //unsafe { ReleaseDC(ptr::null_mut(), self.0) };
    }
}

fn get_clipboard_bitmap() -> Option<Vec<u8>> {
    let handle = raw::get_clipboard_data(CF_BITMAP).ok()?;
    let mut bitmap = BITMAP {
        bmType: 0,
        bmWidth: 0,
        bmHeight: 0,
        bmWidthBytes: 0,
        bmPlanes: 0,
        bmBitsPixel: 0,
        bmBits: ptr::null_mut(),
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

    let info = if clr_bits < 24 {
        let size = size_of::<BITMAPINFOHEADER>() + size_of::<RGBQUAD>() * (1 << clr_bits);
        unsafe { LocalAlloc(LPTR, size) }
    } else {
        unsafe { LocalAlloc(LPTR, size_of::<BITMAPINFOHEADER>()) }
    };
    let info = info as *mut BITMAPINFO;
    let info = &mut unsafe { *info };

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
    let mut buf = Vec::<u8>::with_capacity(info.bmiHeader.biSizeImage as _);
    buf.resize(buf.capacity(), 0);
    if unsafe {
        GetDIBits(
            dc.0,
            handle.as_ptr() as _,
            0,
            info.bmiHeader.biHeight as _,
            buf.as_mut_ptr() as _,
            info as *mut _,
            DIB_RGB_COLORS,
        )
    } == 0
    {
        unsafe {
            LocalFree(info as *mut _ as _);
            ReleaseDC(ptr::null_mut(), dc.0);
            //drop(dc);
        }
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

    unsafe {
        LocalFree(info as *mut _ as _);
        ReleaseDC(ptr::null_mut(), dc.0);
        //drop(dc);
    }

    Some(out)
}

fn write_stdout<W>(write: &mut W, bytes: &[u8]) -> Result<(), &'static str>
where
    W: Write,
{
    match write.write(bytes) {
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
            write_stdout(&mut stdout, text.as_bytes())?;
            if atty::is(Stream::Stdout) && !text.ends_with('\n') {
                write_stdout(&mut stdout, &['\n' as u8])?;
            }
        } else if let Some(bytes) = get_clipboard_bitmap() {
            if atty::is(Stream::Stdout) {
                write_stdout(&mut stdout, "epa era imagem!!\n".as_bytes())?;
            } else {
                write_stdout(&mut stdout, &bytes[..])?;
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

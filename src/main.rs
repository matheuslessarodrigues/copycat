use std::{
    env,
    io::{stdin, stdout, Read, Write},
};

use atty::Stream;
use clipboard_win::{image, Clipboard};

fn try_main() -> Result<(), &'static str> {
    let mut force_in = false;
    let mut force_out = false;

    if let Some(arg) = env::args().nth(1) {
        match arg.as_str() {
            "-i" => force_in = true,
            "-o" => force_out = true,
            "-h" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                println!("\nPipe into it to copy");
                println!("Pipe from it to paste\n");
                println!("\t-h\tprints this help message");
                println!("\t-v\tprints version");
                println!("\t-i\tforce set clipboard from stdin");
                println!("\t-o\tforce output clipboard to stdout");
                return Ok(());
            }
            "-v" => {
                println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => (),
        }
    }

    let clipboard = Clipboard::new().map_err(|_| "could not open clipboard")?;

    if !force_out && (force_in || atty::isnt(Stream::Stdin)) {
        let mut bytes = Vec::new();
        stdin()
            .lock()
            .read_to_end(&mut bytes)
            .map_err(|_| "could not read from stdin")?;
        match std::str::from_utf8(&bytes[..]) {
            Ok(text) => {
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
            }
            Err(_) => {
                let bitmap = image::Image { bytes };
                clipboard
                    .set_bitmap(&bitmap)
                    .map_err(|_| "could not set clipboard bitmap")?;
            }
        }
    } else {
        let stdout = stdout();
        let mut stdout = stdout.lock();
        let mut text = String::new();
        if let Ok(()) = clipboard.get_string(&mut text) {
            stdout
                .write(text.as_bytes())
                .map_err(|_| "could not write text to stdout")?;
            if atty::is(Stream::Stdout) && !text.ends_with('\n') {
                stdout
                    .write(&['\n' as _])
                    .map_err(|_| "could not write text to stdout")?;
            }
        } else if let Ok(bitmap) = clipboard.get_bitmap() {
            if atty::is(Stream::Stdout) {
                write!(stdout, "bitmap: {} bytes\n", bitmap.bytes.len())
                    .map_err(|_| "could not write bitmap to stdout")?;
            } else {
                stdout
                    .write(&bitmap.bytes[..])
                    .map_err(|_| "could not write bitmap to stdout")?;
            }
        } else {
            return Err("clipboard does not contain neither text nor bitmap");
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = try_main() {
        eprintln!("error: {}", error);
    }
}

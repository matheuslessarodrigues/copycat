use std::io::{stdin, Read};
use clipboard2::{Clipboard, SystemClipboard};
use atty::Stream;

fn main() {
    let clipboard = SystemClipboard::new().unwrap();

    if atty::isnt(Stream::Stdin) {
        let mut text = String::new();
        stdin().lock().read_to_string(&mut text).unwrap();
        if text.ends_with('\n') {
            text.pop();
        }
        clipboard.set_string_contents(text).unwrap();
    } else {
        let text = clipboard.get_string_contents().unwrap();
        if atty::is(Stream::Stdout) {
            if text.ends_with('\n') {
                print!("{}", text);
            } else {
                println!("{}", text);
            }
        } else {
            print!("{}", text);
        }
    }
}

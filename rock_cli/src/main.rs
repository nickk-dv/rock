#![forbid(unsafe_code)]

mod ansi;
mod command;
mod error_format;

pub fn main() {
    command::run();
}

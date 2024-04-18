#![forbid(unsafe_code)]

#[allow(dead_code)]
mod ansi;
mod command;
mod error_format;

pub fn main() {
    command::run();
}

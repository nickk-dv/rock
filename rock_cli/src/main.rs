#![forbid(unsafe_code)]

mod ansi;
mod command;
mod error_print;
mod execute;

pub fn main() {
    let command = match command::parse() {
        Ok(command) => command,
        Err(err) => {
            error_print::print_errors(None, err);
            return;
        }
    };

    if let Err(error) = execute::command(command) {
        error_print::print_errors(None, error.into());
    }
}

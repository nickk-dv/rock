use super::ansi::{self, Color};
use super::error::Error;
use super::span_fmt;
use crate::ast::token::Token;

static mut ERR_COUNT: u32 = 0;

pub fn did_error() -> bool {
    unsafe { ERR_COUNT > 0 }
}

pub fn report(error: Error) {
    unsafe { ERR_COUNT += 1 }
    match error {
        Error::Parse(err) => {
            print_error("parse error");
            println!("in {}", err.context.as_str());
            print!("expected: ");
            for (index, token) in err.expected.iter().enumerate() {
                if index < err.expected.len() - 1 {
                    print!("`{}`, ", Token::as_str(*token));
                } else {
                    println!("`{}`", Token::as_str(*token));
                }
            }
            span_fmt::print(
                &err.source.file,
                err.got_token.span,
                Some("unexpected token"),
                false,
            );
        }
        Error::Check(err) => {
            print_error("error");
            println!("{}", err.message);
            print_help(err.help);
            if !err.no_source {
                span_fmt::print(&err.source.file, err.span, None, false);
                for info in err.info.iter() {
                    span_fmt::print(&info.source.file, info.span, Some(info.marker), true);
                }
            }
        }
        Error::FileIO(err) => {
            print_error("file io error");
            println!("{}", err.message);
            print_help(err.help);
        }
        Error::Internal(err) => {
            print_error("error [internal]");
            println!("{}", err.message);
            print_help(err.help);
        }
    }
}

fn print_error(error_name: &'static str) {
    ansi::set_color(Color::BoldRed);
    print!("\n{}: ", error_name);
    ansi::reset();
}

fn print_help(help: Option<&'static str>) {
    if let Some(str) = help {
        ansi::set_color(Color::Cyan);
        print!("help: ");
        ansi::reset();
        println!("{}", str);
    }
}

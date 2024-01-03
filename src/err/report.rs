use super::ansi::{self, Color};
use super::check_err::*;
use super::parse_err::*;
use super::span_fmt;
use crate::ast::ast::{Ast, SourceID};
use crate::ast::span::*;
use crate::ast::token::Token;

static mut ERR_COUNT: u32 = 0;

fn increment_err_count() {
    unsafe {
        ERR_COUNT += 1;
    }
}

pub fn did_error() -> bool {
    unsafe { ERR_COUNT > 0 }
}

pub fn err_no_context(error: CheckError) {
    increment_err_count();

    let error_data = error.get_data();
    ansi::set_color(Color::BoldRed);
    print!("\nerror: ");
    ansi::reset();
    println!("{}", error_data.message);

    if let Some(help) = error_data.help {
        ansi::set_color(Color::Cyan);
        print!("help: ");
        ansi::reset();
        println!("{}", help);
    }
}

pub fn err(ast: &Ast, error: &Error) {
    increment_err_count();

    let error_data = error.error.get_data();
    ansi::set_color(Color::BoldRed);
    print!("\nerror: ");
    ansi::reset();
    println!("{}", error_data.message);

    let source = ast.files.get(error.source as usize).unwrap(); //@err internal?
    span_fmt::print(source, error.span, None, false);

    for info in error.info.iter() {
        let info_source = ast.files.get(info.source as usize).unwrap();
        span_fmt::print(info_source, info.span, Some(info.marker), true)
    }

    if let Some(help) = error_data.help {
        ansi::set_color(Color::Cyan);
        print!("help: ");
        ansi::reset();
        println!("{}", help);
    }
}

pub fn parse_err(ast: &Ast, id: SourceID, err: ParseError) {
    increment_err_count();
    let source = ast.files.get(id as usize).unwrap(); //@err internal?

    ansi::set_color(Color::BoldRed);
    print!("\nparse error: ");
    ansi::reset();
    println!("in {}", err.context.as_str());

    print!("expected: ");
    for (index, token) in err.expected.iter().enumerate() {
        if index < err.expected.len() - 1 {
            print!("`{}`, ", Token::as_str(*token));
        } else {
            println!("`{}`", Token::as_str(*token));
        }
    }
    span_fmt::print(source, err.got_token.span, Some("unexpected token"), false);
}

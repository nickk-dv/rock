use super::ansi::{self, Color};
use super::parse_err::*;
use super::span_fmt;
use crate::ast::ast::{Ast, SourceID};
use crate::ast::span::*;
use crate::ast::token::Token;

pub fn err(ast: &Ast, id: SourceID, span: Span) {
    let source = ast.files.get(id as usize).unwrap(); //@err internal?

    ansi::set_color(Color::Red);
    println!("error: ");
    ansi::reset();
    span_fmt::print(source, span, Some("unexpected token"));
}

pub fn parse_err(ast: &Ast, id: SourceID, err: ParseError) {
    let source = ast.files.get(id as usize).unwrap(); //@err internal?

    ansi::set_color(Color::Red);
    print!("parse error: ");
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
    span_fmt::print(source, err.got_token.span, Some("unexpected token"));
}

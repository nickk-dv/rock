use super::ansi::{self, Color};
use super::span_fmt;
use crate::ast::span::*;
use crate::ast::{Package, SourceID};

pub fn err(package: &Package, id: SourceID, span: Span) {
    let source = package.files.get(id as usize).unwrap(); //@err internal?

    ansi::set_color(Color::Red);
    println!("parse error:");
    ansi::reset();
    span_fmt::print(source, span, Some("unexpected token"));
}

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod arena;
mod ast;
pub mod ast_parse;
pub mod error;
mod hir;
pub mod hir_lower;
mod intern;
mod lexer;
pub mod text;
mod token;
pub mod vfs;

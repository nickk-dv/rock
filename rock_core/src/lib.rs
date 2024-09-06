#![deny(unsafe_code)]

pub mod ast;
pub mod ast_parse;
pub mod config;
pub mod error;
pub mod format;
pub mod fs_env;
pub mod hir;
pub mod hir_lower;
pub mod intern;
mod lexer;
mod macros;
pub mod package;
pub mod session;
mod support;
pub mod syntax;
pub mod text;
mod token;

use package::semver::Semver;

/// toolchain version used to build both `rock_cli` and `rock_ls`  
/// increment minor before 1.0.0 release
pub const VERSION: Semver = Semver::new(0, 1, 0);

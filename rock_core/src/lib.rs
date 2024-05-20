#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod arena;
mod ast;
pub mod ast_parse;
#[cfg(feature = "codegen_llvm")]
pub mod codegen;
pub mod error;
pub mod fs_env;
mod hir;
pub mod hir_lower;
mod intern;
mod lexer;
mod macros;
pub mod package;
pub mod session;
pub mod text;
mod token;

use package::Semver;
/// toolchain version used to build both `rock_cli` and `rock_ls`  
/// increment minor before 1.0.0 release
pub const VERSION: Semver = Semver::new(0, 1, 0);

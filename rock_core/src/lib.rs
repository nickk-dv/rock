#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod arena;
mod ast;
pub mod ast_parse;
#[cfg(not(target_os = "linux"))]
#[cfg(feature = "codegen_llvm")]
pub mod codegen;
pub mod error;
pub mod format;
pub mod fs_env;
mod hir;
pub mod hir_lower;
mod intern;
mod lexer;
mod macros;
pub mod package;
pub mod session;
pub mod syntax;
mod temp_buffer;
pub mod text;
mod timer;
mod token;

use package::semver::Semver;

/// toolchain version used to build both `rock_cli` and `rock_ls`  
/// increment minor before 1.0.0 release
pub const VERSION: Semver = Semver::new(0, 1, 0);

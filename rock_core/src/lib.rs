#![deny(unsafe_code)]

pub mod ast;
pub mod error;
pub mod errors;
pub mod hir;
pub mod hir_lower;
pub mod intern;
pub mod ir;
pub mod session;
pub mod support;
pub mod syntax;
pub mod text;

use session::manifest::Semver;
pub const VERSION: Semver = Semver::new(0, 1, 0);

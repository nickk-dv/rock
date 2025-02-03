mod deps;
pub mod fold;
pub mod layout;

pub use deps::{error_cannot_use_in_constants, resolve_const_dependencies, resolve_const_expr};
pub use layout::type_layout;

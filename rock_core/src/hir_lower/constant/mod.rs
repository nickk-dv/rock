mod deps;
pub mod fold;
pub mod layout;

pub use deps::{resolve_const_dependencies, resolve_const_expr};
pub use fold::fold_const_expr;
pub use layout::type_layout;

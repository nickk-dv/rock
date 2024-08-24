mod deps;
mod fold;
mod layout;

pub use deps::{resolve_const_dependencies, resolve_const_expr};
pub use fold::int_range_check;
pub use layout::{basic_layout, type_layout};

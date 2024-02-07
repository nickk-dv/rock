mod arena;
pub mod arena_id;
mod array;
mod list;
mod ptr;

pub use arena::*;
pub use array::*;
pub use list::*;
pub use ptr::*;

pub type Drop<T> = std::mem::ManuallyDrop<T>;

pub trait ManualDrop: Sized {
    fn manual_drop(self) {}
}

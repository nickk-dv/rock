use super::*;

#[derive(Copy, Clone)]
pub struct Array<T: Copy> {
    pub data: P<T>,
    pub size: u32,
}

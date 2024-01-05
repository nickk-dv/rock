use super::*;
use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct Array<T> {
    ptr: P<T>,
    size: u32,
}

impl<T> Array<T> {
    pub fn new(ptr: P<T>, size: u32) -> Self {
        Array { ptr, size }
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn len(&self) -> usize {
        self.size as usize
    }

    pub fn iter(&self) -> ArrayIterator<T> {
        ArrayIterator {
            ptr: self.ptr.as_mut(),
            size: self.size,
            index: 0,
            phantom: PhantomData,
        }
    }
}

pub struct ArrayIterator<'a, T> {
    ptr: *const T,
    size: u32,
    index: u32,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> Iterator for ArrayIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            let item = unsafe { &*self.ptr.add(self.index as usize) };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

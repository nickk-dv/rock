use std::ops::{Deref, DerefMut};
use std::{alloc, ptr};

#[derive(Copy, Clone)]
pub struct P<T> {
    ptr: *mut T,
}

impl<T> P<T> {
    pub fn new(ptr: *mut T) -> Self {
        P { ptr }
    }

    pub fn null() -> Self {
        P {
            ptr: ptr::null_mut() as *mut T,
        }
    }
}

impl<T> Deref for P<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T> DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

#[derive(Copy, Clone)]
struct ArenaBlock {
    data: *mut u8,
    prev: P<ArenaBlock>,
}

pub struct Arena {
    data: *mut u8,
    pub offset: usize,
    pub block_size: usize,
    curr: P<ArenaBlock>,
    layout: alloc::Layout,
}

impl Arena {
    const PAGE_SIZE: usize = 4096;

    pub fn new(mut block_size: usize) -> Self {
        let align = block_size % Self::PAGE_SIZE;
        block_size = match align {
            0 => block_size,
            _ => block_size + (Self::PAGE_SIZE - align),
        };
        let mut arena = Arena {
            data: ptr::null_mut(),
            offset: 0,
            block_size,
            curr: P::<ArenaBlock>::null(),
            layout: unsafe {
                alloc::Layout::from_size_align_unchecked(block_size, Self::PAGE_SIZE)
            },
        };
        arena.alloc_block();
        return arena;
    }

    pub fn alloc<T>(&mut self) -> P<T> {
        let size = std::mem::size_of::<T>();
        if self.offset + size > self.block_size {
            self.alloc_block();
        }
        let ptr = unsafe { self.data.add(self.offset) };
        self.offset += size;
        return P::new(ptr as *mut T);
    }

    fn alloc_block(&mut self) {
        self.data = unsafe { alloc::alloc_zeroed(self.layout) };
        self.offset = 0;

        let mut block = self.alloc::<ArenaBlock>();
        block.data = self.data;
        block.prev = self.curr;
        self.curr = block;
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        let mut block = self.curr;
        while !block.ptr.is_null() {
            let prev = block.prev;
            unsafe {
                alloc::dealloc(block.data, self.layout);
            }
            block = prev;
        }
    }
}

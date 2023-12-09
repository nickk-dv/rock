use super::*;
use std::{alloc, ptr};

pub struct Arena {
    data: *mut u8,
    offset: usize,
    layout: alloc::Layout,
    blocks: List<*mut u8>,
}

impl Arena {
    const PAGE_SIZE: usize = 4096;

    pub fn new(block_size: usize) -> Self {
        let mut arena = Arena {
            data: ptr::null_mut(),
            offset: 0,
            layout: alloc::Layout::from_size_align(block_size, Self::PAGE_SIZE).unwrap(),
            blocks: List::new(),
        };
        arena.alloc_block();
        return arena;
    }

    pub fn alloc<T: Copy>(&mut self) -> P<T> {
        let size = (std::mem::size_of::<T>() + 7) & !7;
        if self.offset + size > self.layout.size() {
            self.alloc_block();
        }
        let ptr = unsafe { self.data.add(self.offset) };
        self.offset += size;
        return P::new(ptr as *mut T);
    }

    fn alloc_block(&mut self) {
        self.data = unsafe { alloc::alloc_zeroed(self.layout) };
        self.offset = 0;
        let mut blocks = self.blocks;
        blocks.add(self, self.data);
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for block in self.blocks.iter() {
            unsafe {
                alloc::dealloc(block, self.layout);
            }
        }
    }
}

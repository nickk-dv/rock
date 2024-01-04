use super::*;
use std::{alloc, ptr};

type Rawptr = usize;

#[derive(Copy, Clone)]
pub struct Arena {
    data: Rawptr,
    offset: usize,
    layout: alloc::Layout,
    blocks: List<Rawptr>,
}

impl Arena {
    const PAGE_SIZE: usize = 4096;

    pub fn new(block_size: usize) -> Self {
        let mut arena = Arena {
            data: 0,
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
        let ptr = unsafe { (self.data as *mut u8).add(self.offset) };
        self.offset += size;
        return P::new(ptr as Rawptr);
    }

    fn alloc_block(&mut self) {
        self.data = unsafe { alloc::alloc_zeroed(self.layout) as Rawptr };
        self.offset = 0;
        let mut blocks = self.blocks;
        blocks.add(self, self.data);
        self.blocks = blocks;
    }

    fn manual_drop(&mut self) {
        for block in self.blocks.iter() {
            unsafe {
                alloc::dealloc(block as *mut u8, self.layout);
            }
        }
    }
}

use super::*;
use std::alloc;

pub struct Arena {
    data: P<u8>,
    offset: usize,
    layout: alloc::Layout,
    blocks: List<P<u8>>,
}

impl Arena {
    const PAGE_SIZE: usize = 4096;

    pub fn new(block_size: usize) -> Self {
        let mut arena = Arena {
            data: P::null(),
            offset: 0,
            layout: alloc::Layout::from_size_align(block_size, Self::PAGE_SIZE).unwrap(),
            blocks: List::new(),
        };
        arena.alloc_block();
        return arena;
    }

    pub fn alloc<T>(&mut self) -> P<T> {
        self.alloc_buffer(1)
    }

    pub fn alloc_array<T>(&mut self, len: usize) -> Array<T> {
        Array::new(self.alloc_buffer(len), len as u32)
    }

    fn alloc_buffer<T>(&mut self, len: usize) -> P<T> {
        let size = (len * std::mem::size_of::<T>() + 7) & !7;
        if self.offset + size > self.layout.size() {
            self.alloc_block();
        }
        let ptr = self.data.add(self.offset);
        self.offset += size;
        return P::new(ptr.as_raw());
    }

    fn alloc_block(&mut self) {
        self.data = unsafe { P::new(alloc::alloc_zeroed(self.layout) as Rawptr) };
        self.offset = 0;
        let mut blocks = self.blocks;
        blocks.add(self, self.data);
        self.blocks = blocks;
    }

    pub fn manual_drop(&mut self) {
        for block in self.blocks.iter() {
            unsafe {
                alloc::dealloc(block.as_mut(), self.layout);
            }
        }
    }

    pub fn report_memory_usage(&self) {
        let mut bytes_used = 0;
        for _ in self.blocks {
            bytes_used += self.layout.size();
        }
        bytes_used -= self.layout.size();
        bytes_used += self.offset;
        println!(
            "arena bytes used: {}, block size: {}",
            bytes_used,
            self.layout.size()
        );
    }
}

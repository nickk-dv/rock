use super::{list::ListBuilder, ptr::P};
use std::alloc;

const PAGE_SIZE: usize = 4096;
const MAX_PAGE_SIZE: usize = 512 * 4096;

pub struct Arena {
    offset: usize,
    block: Block,
    used_blocks: ListBuilder<Block>,
}

#[derive(Copy, Clone)]
struct Block {
    data: P<u8>,
    layout: alloc::Layout,
}

impl Arena {
    pub fn new() -> Self {
        Self {
            offset: 0,
            block: Block::alloc(PAGE_SIZE),
            used_blocks: ListBuilder::new(),
        }
    }

    pub fn alloc<T: Copy>(&mut self) -> P<T> {
        let size = std::mem::size_of::<T>();
        if self.offset + size > self.block.size() {
            self.grow();
        }
        let offset = unsafe { self.block.data.as_mut().add(self.offset) };
        self.offset += size;
        P::new(offset as *mut T)
    }

    fn grow(&mut self) {
        let used_block = self.block;
        let new_size = usize::min(self.block.size() * 2, MAX_PAGE_SIZE);
        self.offset = 0;
        self.block = Block::alloc(new_size);

        let mut used_blocks = self.used_blocks;
        used_blocks.add(self, used_block);
        self.used_blocks = used_blocks;
    }

    pub fn mem_usage(&self) -> usize {
        let mut usage: usize = 0;
        for block in self.used_blocks.take() {
            usage += block.size();
        }
        usage += self.offset;
        usage
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for block in self.used_blocks.take() {
            block.dealloc();
        }
        self.block.dealloc();
    }
}

impl Block {
    fn alloc(size: usize) -> Self {
        let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
        let bytes = unsafe { alloc::alloc_zeroed(layout) };
        Self {
            data: P::new(bytes),
            layout,
        }
    }

    fn dealloc(self) {
        unsafe { alloc::dealloc(self.data.as_mut(), self.layout) };
    }

    fn size(&self) -> usize {
        self.layout.size()
    }
}

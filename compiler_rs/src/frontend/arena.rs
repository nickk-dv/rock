use std::alloc::{alloc, dealloc, Layout};

pub struct Arena<'a> {
    offset: usize,
    block_size: usize,
    curr_block: ArenaBlock,
    blocks: Vec<ArenaBlock>,
    str: &'a str,
}

#[derive(Copy, Clone)]
struct ArenaBlock {
    data: *mut u8,
}

impl<'a> Arena<'a> {
    pub fn new(block_size: usize) -> Self {
        Self {
            offset: 0,
            block_size,
            curr_block: ArenaBlock::new(block_size),
            blocks: Vec::new(),
            str: "",
        }
    }

    pub fn alloc<T>(&mut self) -> &'a mut T {
        let size = std::mem::size_of::<T>();
        if self.offset + size > self.block_size {
            self.alloc_block();
        }
        let ptr = unsafe {
            let raw_ptr = self.curr_block.data.offset(self.offset as isize) as *mut T;
            &mut *raw_ptr
        };
        self.offset += size;
        return ptr;
    }

    fn alloc_block(&mut self) {
        self.offset = 0;
        self.blocks.push(self.curr_block);
        self.curr_block = ArenaBlock::new(self.block_size);
    }
}

impl Drop for Arena<'_> {
    fn drop(&mut self) {
        for block in &self.blocks {
            block.drop(self.block_size);
        }
    }
}

impl ArenaBlock {
    fn new(block_size: usize) -> Self {
        let layout = Layout::from_size_align(block_size, 1).unwrap();
        Self {
            data: unsafe { alloc(layout) },
        }
    }

    fn drop(&self, block_size: usize) {
        let layout = Layout::from_size_align(block_size, 1).unwrap();
        unsafe {
            if !self.data.is_null() {
                dealloc(self.data, layout)
            }
        }
    }
}

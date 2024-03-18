use std::{alloc, marker::PhantomData};

const PAGE_SIZE: usize = 4096;
const MAX_PAGE_SIZE: usize = 512 * PAGE_SIZE;

pub struct Arena<'arena> {
    offset: usize,
    block: Block,
    full_blocks: Vec<Block>,
    phantom: PhantomData<&'arena ()>,
}

#[derive(Copy, Clone)]
struct Block {
    data: *mut u8,
    layout: alloc::Layout,
}

impl<'arena> Arena<'arena> {
    pub fn new() -> Arena<'arena> {
        Arena {
            offset: 0,
            block: Block::alloc(PAGE_SIZE),
            full_blocks: Vec::new(),
            phantom: PhantomData::default(),
        }
    }

    pub fn alloc<T: Copy>(&mut self, val: T) -> &'arena T {
        let offset = self.offset_raw::<T>(std::mem::size_of::<T>());
        unsafe {
            *offset = val;
            &*offset
        }
    }

    pub fn alloc_slice<T: Copy>(&mut self, val: &[T]) -> &'arena [T] {
        let offset = self.offset_raw::<T>(std::mem::size_of::<T>() * val.len());
        unsafe {
            std::ptr::copy_nonoverlapping(val.as_ptr(), offset as *mut T, val.len());
            std::slice::from_raw_parts(offset as *const T, val.len())
        }
    }

    pub fn alloc_str(&mut self, val: &str) -> &'arena str {
        let bytes = self.alloc_slice(val.as_bytes());
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    fn offset_raw<T: Copy>(&mut self, size: usize) -> *mut T {
        if self.offset + size > self.block.layout.size() {
            self.grow();
        }
        unsafe {
            let offset = self.block.data.add(self.offset) as *mut T;
            self.offset += size;
            offset
        }
    }

    fn grow(&mut self) {
        self.full_blocks.push(self.block);
        self.offset = 0;
        self.block = Block::alloc((self.block.layout.size() * 2).min(MAX_PAGE_SIZE));
    }

    pub fn mem_usage(&self) -> usize {
        let mut usage: usize = self.offset;
        for block in &self.full_blocks {
            usage += block.layout.size();
        }
        usage
    }
}

impl<'arena> Drop for Arena<'arena> {
    fn drop(&mut self) {
        for block in self.full_blocks.iter() {
            block.dealloc();
        }
        self.block.dealloc();
    }
}

impl Block {
    fn alloc(size: usize) -> Block {
        let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
        Block {
            data: unsafe { alloc::alloc_zeroed(layout) },
            layout,
        }
    }

    fn dealloc(self) {
        unsafe { alloc::dealloc(self.data, self.layout) };
    }
}

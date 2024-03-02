use super::{list::ListBuilder, ptr::P};
use std::{alloc, marker::PhantomData};

const PAGE_SIZE: usize = 4096;
const MAX_PAGE_SIZE: usize = 512 * 4096;

pub struct Arena<'ast> {
    offset: usize,
    block: Block,
    used_blocks: ListBuilder<Block>,
    phantom: PhantomData<&'ast ()>,
}

#[derive(Copy, Clone)]
struct Block {
    data: P<u8>,
    layout: alloc::Layout,
}

impl<'ast> Arena<'ast> {
    pub fn new() -> Self {
        Self {
            offset: 0,
            block: Block::alloc(PAGE_SIZE),
            used_blocks: ListBuilder::new(),
            phantom: PhantomData::default(),
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

    pub fn alloc_ref_new<T: Copy>(&mut self, val: T) -> &'ast T {
        let size = std::mem::size_of::<T>();
        if self.offset + size > self.block.size() {
            self.grow();
        }
        let offset = unsafe { self.block.data.as_mut().add(self.offset) };
        self.offset += size;
        unsafe { *(offset as *mut T) = val };
        unsafe { &*(offset as *mut T) }
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

impl<'ast> Drop for Arena<'ast> {
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

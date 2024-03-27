use std::{alloc, marker::PhantomData};

const PAGE_SIZE: usize = 4096;
const MAX_PAGE_SIZE: usize = 512 * PAGE_SIZE;

pub struct Arena<'arena> {
    offset: usize,
    block: (*mut u8, alloc::Layout),
    full_blocks: Vec<(*mut u8, alloc::Layout)>,
    phantom: PhantomData<&'arena ()>,
}

impl<'arena> Arena<'arena> {
    pub fn new() -> Arena<'arena> {
        Arena {
            offset: 0,
            block: alloc_zeroed(PAGE_SIZE),
            full_blocks: Vec::new(),
            phantom: PhantomData,
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
        let offset = self.offset_raw::<T>(std::mem::size_of_val(val));
        unsafe {
            std::ptr::copy_nonoverlapping(val.as_ptr(), offset, val.len());
            std::slice::from_raw_parts(offset as *const T, val.len())
        }
    }

    pub fn alloc_str(&mut self, val: &str) -> &'arena str {
        let bytes = self.alloc_slice(val.as_bytes());
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    fn offset_raw<T: Copy>(&mut self, size: usize) -> *mut T {
        if self.offset + size > self.block.1.size() {
            self.grow();
        }
        unsafe {
            let offset = self.block.0.add(self.offset) as *mut T;
            self.offset += size;
            offset
        }
    }

    fn grow(&mut self) {
        self.full_blocks.push(self.block);
        self.offset = 0;
        self.block = alloc_zeroed((self.block.1.size() * 2).min(MAX_PAGE_SIZE));
    }

    pub fn mem_usage(&self) -> usize {
        let full_bytes: usize = self.full_blocks.iter().map(|block| block.1.size()).sum();
        full_bytes + self.offset
    }
}

impl<'arena> Default for Arena<'arena> {
    fn default() -> Arena<'arena> {
        Arena::new()
    }
}

impl<'arena> Drop for Arena<'arena> {
    fn drop(&mut self) {
        for block in &self.full_blocks {
            dealloc(*block);
        }
        dealloc(self.block);
    }
}

fn alloc_zeroed(size: usize) -> (*mut u8, alloc::Layout) {
    let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
    let data = unsafe { alloc::alloc_zeroed(layout) };
    (data, layout)
}

fn dealloc(block: (*mut u8, alloc::Layout)) {
    unsafe { alloc::dealloc(block.0, block.1) }
}

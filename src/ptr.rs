use std::ops::{Deref, DerefMut};
use std::{alloc, ptr};

#[derive(Copy, Clone)]
pub struct P<T: Copy> {
    ptr: *mut T,
}

impl<T: Copy> P<T> {
    pub fn new(ptr: *mut T) -> Self {
        P { ptr }
    }

    pub fn null() -> Self {
        P {
            ptr: ptr::null_mut() as *mut T,
        }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl<T: Copy> Deref for P<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: Copy> DerefMut for P<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}

#[derive(Copy, Clone)]
pub struct List<T: Copy> {
    first: P<Node<T>>,
    last: P<Node<T>>,
}

#[derive(Copy, Clone)]
struct Node<T: Copy> {
    val: T,
    next: P<Node<T>>,
}

impl<T: Copy> List<T> {
    pub fn add(&mut self, arena: &mut Arena, val: T) {
        let mut node = arena.alloc::<Node<T>>();
        node.val = val;
        node.next = P::null();

        if self.first.is_null() {
            self.first = node;
            self.last = self.first;
        } else {
            self.last.next = node;
            self.last = node;
        }
    }
}

pub struct ListIterator<T: Copy> {
    curr: P<Node<T>>,
}

impl<T: Copy> List<T> {
    pub fn iter(self) -> ListIterator<T> {
        ListIterator { curr: self.first }
    }
}

impl<T: Copy> Iterator for ListIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            return None;
        }
        let val = self.curr.val;
        self.curr = self.curr.next;
        return Some(val);
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

    pub fn alloc<T: Copy>(&mut self) -> P<T> {
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

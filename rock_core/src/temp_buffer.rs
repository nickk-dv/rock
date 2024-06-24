use crate::arena::Arena;
use std::marker::PhantomData;

pub struct TempBuffer<T: Copy> {
    buffer: Vec<T>,
}

pub struct BufferOffset<T: Copy> {
    idx: usize,
    phantom: PhantomData<T>,
}

impl<T: Copy> TempBuffer<T> {
    pub fn new(cap: usize) -> TempBuffer<T> {
        TempBuffer {
            buffer: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn start(&self) -> BufferOffset<T> {
        BufferOffset {
            idx: self.buffer.len(),
            phantom: PhantomData::default(),
        }
    }

    #[inline]
    pub fn add(&mut self, value: T) {
        self.buffer.push(value);
    }

    pub fn take<'arena>(
        &mut self,
        offset: BufferOffset<T>,
        arena: &mut Arena<'arena>,
    ) -> &'arena [T] {
        let slice = arena.alloc_slice(&self.buffer[offset.idx..]);
        self.buffer.truncate(offset.idx);
        slice
    }
}

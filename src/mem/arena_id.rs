use std::marker::PhantomData;

pub struct Arena {
    offset: usize,
    buffer: Vec<u8>,
}

#[derive(Copy, Clone)]
pub struct Id<T: Copy> {
    inner: u32,
    phatom: PhantomData<T>,
}

impl Arena {
    pub fn new(cap: usize) -> Self {
        Self {
            offset: 0,
            buffer: Vec::with_capacity(cap),
        }
    }
    pub fn get<T: Copy>(&self, id: Id<T>) -> &T {
        unsafe { &*(self.buffer.as_ptr().add(id.inner as usize).cast()) }
    }
    pub fn get_mut<T: Copy>(&mut self, id: Id<T>) -> &mut T {
        unsafe { &mut *(self.buffer.as_mut_ptr().add(id.inner as usize).cast()) }
    }
    pub fn buffer_len(&self) -> usize {
        self.offset
    }
    pub fn buffer_cap(&self) -> usize {
        self.buffer.capacity()
    }
    pub fn buffer_drain(mut self) -> Vec<u8> {
        unsafe { self.buffer.set_len(self.offset) };
        self.buffer
    }
    pub fn buffer_clone(&mut self) -> Vec<u8> {
        unsafe { self.buffer.set_len(self.offset) };
        self.buffer.clone()
    }
    pub fn alloc<T: Copy>(&mut self, value: T) -> Id<T> {
        let id = Id::new(self.offset as u32);
        let size_aligned = (std::mem::size_of::<T>() + 7) & !7;
        if self.buffer.capacity() < self.offset + size_aligned {
            self.buffer.reserve(self.offset + size_aligned);
        }
        unsafe {
            let slice = self
                .buffer
                .get_unchecked_mut(self.offset..self.offset + size_aligned);
            std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                slice.as_mut_ptr(),
                std::mem::size_of::<T>(),
            );
        }
        self.offset += size_aligned;
        return id;
    }
}

impl<T: Copy> Id<T> {
    fn new(inner: u32) -> Self {
        Self {
            inner,
            phatom: PhantomData,
        }
    }
    pub(super) fn invalid() -> Self {
        Self::new(u32::MAX)
    }
    pub(super) fn is_invalid(&self) -> bool {
        self.inner == u32::MAX
    }
}

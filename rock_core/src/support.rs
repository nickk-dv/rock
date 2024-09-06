pub use arena::Arena;
pub use bitset::BitSet;
pub use temp_buffer::TempBuffer;
pub use timer::Timer;
pub use typed_id::{IndexID, ID};

#[allow(unsafe_code)]
mod arena {
    use std::alloc;
    use std::marker::PhantomData;

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

        //@UB when size > MAX_PAGE_SIZE
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

    impl<'arena> Drop for Arena<'arena> {
        fn drop(&mut self) {
            for block in &self.full_blocks {
                dealloc(*block);
            }
            dealloc(self.block);
        }
    }

    //@do regular alloc without zeroing? test perf diff
    fn alloc_zeroed(size: usize) -> (*mut u8, alloc::Layout) {
        let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
        let data = unsafe { alloc::alloc_zeroed(layout) };
        (data, layout)
    }

    fn dealloc(block: (*mut u8, alloc::Layout)) {
        unsafe { alloc::dealloc(block.0, block.1) }
    }
}

mod temp_buffer {
    use super::Arena;
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
                phantom: PhantomData,
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
}

mod typed_id {
    use std::cmp::PartialEq;
    use std::hash::{Hash, Hasher};
    use std::marker::PhantomData;

    pub struct ID<T> {
        raw: u32,
        phantom: PhantomData<T>,
    }

    pub trait IndexID<T> {
        fn len(&self) -> usize;
        fn id_get(&self, id: ID<T>) -> &T;
        fn id_get_mut(&mut self, id: ID<T>) -> &mut T;
    }

    impl<T> ID<T> {
        pub fn new(values: &impl IndexID<T>) -> ID<T> {
            ID {
                raw: values.len() as u32,
                phantom: PhantomData,
            }
        }
        pub fn new_raw(index: usize) -> ID<T> {
            ID {
                raw: index as u32,
                phantom: PhantomData,
            }
        }

        #[inline]
        pub fn raw(self) -> u32 {
            self.raw
        }
        #[inline]
        pub fn raw_index(self) -> usize {
            self.raw as usize
        }

        #[must_use]
        pub fn inc(self) -> ID<T> {
            let raw = self.raw + 1;
            ID {
                raw,
                phantom: PhantomData,
            }
        }
        #[must_use]
        pub fn dec(self) -> ID<T> {
            assert!(self.raw > 0);
            let raw = self.raw - 1;
            ID {
                raw,
                phantom: PhantomData,
            }
        }
    }

    impl<T> Copy for ID<T> {}

    impl<T> Clone for ID<T> {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl<T> Eq for ID<T> {}

    impl<T> PartialEq for ID<T> {
        fn eq(&self, other: &Self) -> bool {
            self.raw == other.raw
        }
    }

    impl<T> Hash for ID<T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.raw.hash(state);
        }
    }

    impl<T: Clone> IndexID<T> for [T] {
        fn len(&self) -> usize {
            self.len()
        }
        fn id_get(&self, id: ID<T>) -> &T {
            &self[id.raw as usize]
        }
        fn id_get_mut(&mut self, id: ID<T>) -> &mut T {
            &mut self[id.raw as usize]
        }
    }

    impl<T> IndexID<T> for Vec<T> {
        fn len(&self) -> usize {
            self.len()
        }
        fn id_get(&self, id: ID<T>) -> &T {
            &self[id.raw as usize]
        }
        fn id_get_mut(&mut self, id: ID<T>) -> &mut T {
            &mut self[id.raw as usize]
        }
    }
}

mod bitset {
    use std::marker::PhantomData;

    #[derive(Copy, Clone)]
    pub struct BitSet<T>
    where
        T: Copy + Clone + Into<u32>,
    {
        mask: u32,
        phantom: PhantomData<T>,
    }

    impl<T> BitSet<T>
    where
        T: Copy + Clone + Into<u32>,
    {
        pub fn empty() -> BitSet<T> {
            BitSet {
                mask: 0,
                phantom: PhantomData::default(),
            }
        }

        #[inline]
        pub fn set(&mut self, flag: T) {
            self.mask |= 1 << flag.into();
        }
        #[inline]
        pub fn contains(&self, flag: T) -> bool {
            self.mask & (1 << flag.into()) != 0
        }
    }
}

mod timer {
    use std::time::Instant;

    pub struct Timer {
        start: Instant,
        time_ms: f64,
    }

    impl Timer {
        pub fn new() -> Timer {
            Timer {
                start: Instant::now(),
                time_ms: 9999.0,
            }
        }

        pub fn measure(&mut self) {
            let end = Instant::now();
            self.time_ms = end.duration_since(self.start).as_secs_f64() * 1000.0;
        }

        pub fn display(self, msg: &str) {
            eprintln!("{}: {:.4} ms", msg, self.time_ms);
        }

        pub fn stop(self, msg: &str) {
            let end = Instant::now();
            let ms = end.duration_since(self.start).as_secs_f64() * 1000.0;
            eprintln!("{}: {:.4} ms", msg, ms);
        }
    }
}

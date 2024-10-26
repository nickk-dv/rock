pub use arena::Arena;
pub use bitset::BitSet;
pub use temp_buffer::{BufferOffset, TempBuffer};
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
            let align = align_of::<T>();
            self.offset = (self.offset + align - 1) & !(align - 1);

            if self.offset + size > self.block.layout.size() {
                self.grow(size);
            }
            unsafe {
                let offset = self.block.data.add(self.offset) as *mut T;
                self.offset += size;
                offset
            }
        }

        fn grow(&mut self, size: usize) {
            self.offset = 0;
            self.full_blocks.push(self.block);
            let block_size = (self.block.layout.size() * 2).min(MAX_PAGE_SIZE).max(size);
            self.block = Block::alloc(block_size);
        }

        pub fn mem_usage(&self) -> usize {
            let full_bytes: usize = self
                .full_blocks
                .iter()
                .map(|block| block.layout.size())
                .sum();
            full_bytes + self.offset
        }
    }

    impl Block {
        fn alloc(size: usize) -> Block {
            let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
            let data = unsafe { alloc::alloc(layout) };
            Block { data, layout }
        }

        fn dealloc(&self) {
            unsafe { alloc::dealloc(self.data, self.layout) }
        }
    }

    impl<'arena> Drop for Arena<'arena> {
        fn drop(&mut self) {
            for block in &self.full_blocks {
                block.dealloc();
            }
            self.block.dealloc();
        }
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
        pub fn len(&self) -> usize {
            self.buffer.len()
        }
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.buffer.is_empty()
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
        pub const fn new_raw(index: usize) -> ID<T> {
            ID {
                raw: index as u32,
                phantom: PhantomData,
            }
        }
        pub const fn dummy() -> ID<T> {
            ID {
                raw: u32::MAX,
                phantom: PhantomData,
            }
        }

        #[inline]
        pub const fn raw(self) -> u32 {
            self.raw
        }
        #[inline]
        pub const fn raw_index(self) -> usize {
            self.raw as usize
        }

        #[must_use]
        pub const fn inc(self) -> ID<T> {
            let raw = self.raw + 1;
            ID {
                raw,
                phantom: PhantomData,
            }
        }
        #[must_use]
        pub const fn dec(self) -> ID<T> {
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
        #[inline]
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
    }

    impl Timer {
        pub fn start() -> Timer {
            Timer {
                start: Instant::now(),
            }
        }
        pub fn measure_ms(self) -> f64 {
            let end = Instant::now();
            let duration = end.duration_since(self.start);
            duration.as_secs_f64() * 1000.0
        }
    }
}

/// prevent accidental size changes
#[macro_export]
macro_rules! size_lock {
    ($size:expr, $ty:ty) => {
        #[cfg(target_pointer_width = "64")]
        const _: [(); $size] = [(); std::mem::size_of::<$ty>()];
    };
}

/// multi-purpose trait designed for enums:  
/// `const ALL` slice of all variants.  
/// `fn as_str()` convert enum to string.  
/// `fn from_str()` try to convert string to an enum.
pub trait AsStr
where
    Self: Sized + 'static,
{
    const ALL: &[Self];
    fn as_str(self) -> &'static str;
    fn from_str(string: &str) -> Option<Self>;
}

/// generate enum with `AsStr` trait implementation.
#[macro_export]
macro_rules! enum_as_str {
    (
        $(#[$enum_attr:meta])*
        $vis:vis enum $Enum:ident {
            $(
                $(#[$variant_attr:meta])*
                $variant:ident $string:expr
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_attr])*
        $vis enum $Enum {
            $(
                $(#[$variant_attr])*
                $variant,
            )+
        }

        impl AsStr for $Enum {
            const ALL: &[$Enum] = &[
                $($Enum::$variant,)+
            ];
            fn as_str(self) -> &'static str {
                match self {
                    $($Enum::$variant => $string,)+
                }
            }
            fn from_str(string: &str) -> Option<$Enum> {
                match string {
                    $($string => Some($Enum::$variant),)+
                    _ => None,
                }
            }
        }
    }
}

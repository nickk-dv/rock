pub use arena::Arena;
pub use bitset::BitSet;
pub use temp_buffer::{BufferOffset, TempBuffer};
pub use timer::Timer;

#[allow(unsafe_code)]
mod arena {
    use std::alloc;
    use std::marker::PhantomData;

    const PAGE_SIZE: usize = 4096;
    const MAX_PAGE_SIZE: usize = 512 * PAGE_SIZE;

    pub struct Arena<'arena> {
        offset: usize,
        block: Block<'arena>,
        phantom: PhantomData<&'arena ()>,
    }

    #[derive(Copy, Clone)]
    struct Block<'arena> {
        data: *mut u8,
        layout: alloc::Layout,
        prev: Option<&'arena Block<'arena>>,
    }

    impl<'arena> Arena<'arena> {
        pub fn new() -> Arena<'arena> {
            Arena { offset: 0, block: Block::alloc(PAGE_SIZE), phantom: PhantomData }
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

        pub fn alloc_slice_with_value<T: Copy>(&mut self, val: T, len: usize) -> &'arena [T] {
            let offset = self.offset_raw::<T>(len * std::mem::size_of::<T>());
            unsafe {
                let write_ptr = offset;
                for idx in 0..len {
                    std::ptr::write(write_ptr.add(idx), val);
                }
                std::slice::from_raw_parts(offset as *const T, len)
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
            let prev = self.block;
            let block_size = (self.block.layout.size() * 2).min(MAX_PAGE_SIZE).max(size);
            self.offset = 0;
            self.block = Block::alloc(block_size);
            self.block.prev = Some(self.alloc(prev));
        }

        pub fn mem_usage(&self) -> (usize, usize) {
            let mut total = 0;
            let mut current = Some(self.block);
            while let Some(block) = current {
                total += block.layout.size();
                current = block.prev.copied();
            }
            (total - self.block.layout.size() + self.offset, total)
        }
    }

    impl<'arena> Block<'arena> {
        fn alloc(size: usize) -> Block<'arena> {
            let layout = alloc::Layout::from_size_align(size, PAGE_SIZE).unwrap();
            let data = unsafe { alloc::alloc(layout) };
            Block { data, layout, prev: None }
        }
        fn dealloc(&self) {
            unsafe { alloc::dealloc(self.data, self.layout) }
        }
    }

    impl<'arena> Drop for Arena<'arena> {
        fn drop(&mut self) {
            let mut current = Some(self.block);
            while let Some(block) = current {
                current = block.prev.copied();
                block.dealloc();
            }
        }
    }
}

mod temp_buffer {
    use super::Arena;
    use std::marker::PhantomData;

    pub struct TempBuffer<T> {
        buffer: Vec<T>,
    }

    #[derive(Clone)]
    pub struct BufferOffset<T> {
        idx: usize,
        phantom: PhantomData<T>,
    }

    impl<T> TempBuffer<T> {
        pub fn new(cap: usize) -> TempBuffer<T> {
            TempBuffer { buffer: Vec::with_capacity(cap) }
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
            BufferOffset { idx: self.buffer.len(), phantom: PhantomData }
        }
        #[inline]
        pub fn push(&mut self, value: T) {
            self.buffer.push(value);
        }

        #[inline]
        pub fn view(&self, offset: BufferOffset<T>) -> &[T] {
            &self.buffer[offset.idx..]
        }
        #[inline]
        pub fn pop_view(&mut self, offset: BufferOffset<T>) {
            self.buffer.truncate(offset.idx);
        }
        pub fn take<'arena>(
            &mut self,
            offset: BufferOffset<T>,
            arena: &mut Arena<'arena>,
        ) -> &'arena [T]
        where
            T: Copy,
        {
            let slice = arena.alloc_slice(&self.buffer[offset.idx..]);
            self.buffer.truncate(offset.idx);
            slice
        }
    }
}

mod bitset {
    use std::marker::PhantomData;

    #[derive(Copy, Clone)]
    pub struct BitSet<T>
    where T: Copy + Clone + Into<u32>
    {
        mask: u32,
        phantom: PhantomData<T>,
    }

    impl<T> BitSet<T>
    where T: Copy + Clone + Into<u32>
    {
        #[inline]
        pub fn empty() -> BitSet<T> {
            BitSet { mask: 0, phantom: PhantomData }
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
            Timer { start: Instant::now() }
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

/// generate named ID type
#[macro_export]
macro_rules! define_id {
    ($vis:vis $name:ident) => {
        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        $vis struct $name(u32);

        #[allow(unused)]
        impl $name {
            #[must_use]
            #[inline(always)]
            $vis fn new(index: usize) -> $name {
                $name(index as u32)
            }
            #[must_use]
            #[inline(always)]
            $vis fn dummy() -> $name {
                $name(u32::MAX)
            }
            #[must_use]
            #[inline(always)]
            $vis fn raw(self) -> u32 {
                self.0
            }
            #[must_use]
            #[inline(always)]
            $vis fn index(self) -> usize {
                self.0 as usize
            }
            #[must_use]
            #[inline(always)]
            $vis fn inc(self) -> $name {
                $name(self.0 + 1)
            }
            #[must_use]
            #[inline(always)]
            $vis fn dec(self) -> $name {
                assert!(self.0 != 0);
                $name(self.0 - 1)
            }
        }
    }
}

/// multi-purpose trait designed for enums:  
/// `const ALL` slice of all variants.  
/// `fn as_str()` convert enum to string.  
/// `fn from_str()` try to convert string to an enum.
pub trait AsStr
where Self: Sized + 'static
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

pub use arena::Arena;
pub use bitset::BitSet;
pub use temp_buffer::{TempBuffer, TempOffset};
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
    pub struct TempOffset<T> {
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
        pub fn start(&self) -> TempOffset<T> {
            TempOffset { idx: self.buffer.len(), phantom: PhantomData }
        }
        #[inline]
        pub fn push(&mut self, value: T) {
            self.buffer.push(value);
        }

        #[inline]
        pub fn view(&self, offset: TempOffset<T>) -> &[T] {
            &self.buffer[offset.idx..]
        }
        #[inline]
        pub fn pop_view(&mut self, offset: TempOffset<T>) {
            self.buffer.truncate(offset.idx);
        }
        pub fn take<'arena>(
            &mut self,
            offset: TempOffset<T>,
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

pub mod os {
    use crate::error::Error;
    use crate::errors as err;
    use std::io::Read;
    use std::path::{Path, PathBuf};

    pub fn current_exe_path() -> Result<PathBuf, Error> {
        let mut current_exe = std::env::current_exe()
            .map_err(|io_error| err::os_current_exe_path(io_error.to_string()))?;
        current_exe.pop();
        Ok(current_exe)
    }

    pub fn dir_get_current_working() -> Result<PathBuf, Error> {
        std::env::current_dir()
            .map_err(|io_error| err::os_dir_get_current_working(io_error.to_string()))
    }

    pub fn dir_set_current_working(path: &PathBuf) -> Result<(), Error> {
        std::env::set_current_dir(path)
            .map_err(|io_error| err::os_dir_set_current_working(io_error.to_string(), path))
    }

    pub fn dir_create(path: &PathBuf, force: bool) -> Result<(), Error> {
        if !force && path.exists() {
            return Ok(());
        }
        std::fs::create_dir(path).map_err(|io_error| err::os_dir_create(io_error.to_string(), path))
    }

    pub fn dir_read(path: &PathBuf) -> Result<std::fs::ReadDir, Error> {
        std::fs::read_dir(path).map_err(|io_error| err::os_dir_read(io_error.to_string(), path))
    }

    pub fn dir_entry_read(
        origin: &PathBuf,
        entry_result: Result<std::fs::DirEntry, std::io::Error>,
    ) -> Result<std::fs::DirEntry, Error> {
        entry_result.map_err(|io_error| err::os_dir_entry_read(io_error.to_string(), origin))
    }

    pub fn file_create(path: &PathBuf, text: &str) -> Result<(), Error> {
        std::fs::write(path, text)
            .map_err(|io_error| err::os_file_create(io_error.to_string(), path))
    }

    pub fn file_read(path: &PathBuf) -> Result<String, Error> {
        std::fs::read_to_string(path)
            .map_err(|io_error| err::os_file_read(io_error.to_string(), path))
    }

    pub fn file_read_with_sentinel(path: &PathBuf) -> Result<String, Error> {
        fn inner(path: &Path) -> std::io::Result<String> {
            let mut file = std::fs::File::open(path)?;
            let size = file.metadata().map(|m| m.len() as usize).ok();
            let mut string = String::new();
            string.try_reserve_exact(size.unwrap_or(0) + 1)?;
            file.read_to_string(&mut string)?;
            string.push_str("\0");
            Ok(string)
        }
        inner(path.as_ref()).map_err(|io_error| err::os_file_read(io_error.to_string(), path))
    }

    pub fn filename(path: &PathBuf) -> Result<&str, Error> {
        let file_stem = path.file_stem().ok_or(err::os_filename_missing(path))?;
        file_stem.to_str().ok_or(err::os_filename_non_utf8(path))
    }

    pub fn file_extension(path: &PathBuf) -> Option<&str> {
        let extension = path.extension()?;
        extension.to_str()
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

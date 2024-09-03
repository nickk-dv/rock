/// used to prevent accidental size changes  
/// currently only used in `ast` and `hir`
#[macro_export]
macro_rules! size_assert {
    ($size:expr, $ty:ty) => {
        #[cfg(target_pointer_width = "64")]
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

use std::cmp::PartialEq;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

pub struct ID<T> {
    raw: u32,
    phantom: PhantomData<T>,
}

impl<T> Copy for ID<T> {}

impl<T> Clone for ID<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Hash for ID<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> PartialEq for ID<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Eq for ID<T> {}

impl<T> ID<T> {
    pub fn new(values: &impl IndexID<T>) -> ID<T> {
        ID {
            raw: values.len() as u32,
            phantom: PhantomData,
        }
    }
    pub fn raw_index(self) -> usize {
        self.raw as usize
    }
}

pub trait IndexID<T> {
    fn len(&self) -> usize;
    fn id_get(&self, id: ID<T>) -> &T;
    fn id_get_mut(&mut self, id: ID<T>) -> &mut T;
}

impl<T> IndexID<T> for [T] {
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

/// defines named `ID` newtype
#[macro_export]
macro_rules! id_impl {
    ($name:ident) => {
        #[derive(Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $name(u32);

        impl $name {
            #[allow(unused)]
            #[inline(always)]
            pub const fn new(index: usize) -> $name {
                $name(index as u32)
            }
            #[allow(unused)]
            #[inline(always)]
            pub const fn dummy() -> $name {
                $name(u32::MAX)
            }
            #[allow(unused)]
            #[inline(always)]
            pub const fn raw(self) -> u32 {
                self.0
            }
            #[allow(unused)]
            #[inline(always)]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

/// defines enum with `as_str` and optional `from_str`
#[macro_export]
macro_rules! enum_str_convert {
    (
        fn as_str, fn from_str,
        #[derive($($trait_name:ident $(,)?)*)]
        $vis:vis enum $name:ident {
            $($variant:ident => $string:expr,)+
        }
    ) => {
        #[derive($($trait_name,)*)]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            $($variant,)+
        }

        impl $name {
            $vis fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $string,)+
                }
            }
            $vis fn from_str(string: &str) -> Option<$name> {
                match string {
                    $($string => Some($name::$variant),)+
                    _ => None,
                }
            }
        }
    };
    (
        fn as_str,
        #[derive($($trait_name:ident $(,)?)*)]
        $vis:vis enum $name:ident {
            $($variant:ident => $string:expr,)+
        }
    ) => {
        #[derive($($trait_name,)*)]
        #[allow(non_camel_case_types)]
        $vis enum $name {
            $($variant,)+
        }

        impl $name {
            $vis fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $string,)+
                }
            }
        }
    };
}

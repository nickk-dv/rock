/// used to prevent accidental size changes  
/// currently only used in `ast` and `hir`
#[macro_export]
macro_rules! size_assert {
    ($size:expr, $ty:ty) => {
        #[cfg(target_pointer_width = "64")]
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
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

//! Defines a small DSL-like macro that automates token definition and conversions.
//!
//! `token_gen` generates `Token` enum itself and various conversions.  
//! `token_from_char` maps char to token.  
//! `token_glue_extend` defines token glueing rules.
//!
//! `T` macro is also generated and allows to reference tokens  
//! without directly using `Token` enum: `T![,] T![:] T![pub]`

#[rustfmt::skip]
macro_rules! token_gen {
    {
    $(
        [$token:tt] | $string:literal | $name:ident |
        $(KW $mark:tt)?
        $(BIN[$bin_op:expr])?
        $(UN[$un_op:expr])?
        $(ASSIGN[$assign_op:expr])?
        $(BASIC[$basic_ty:expr])?
    )+
    } => {
        macro_rules! T {
            $( [$token] => [Token::$name]; )+
        }
        #[derive(Copy, Clone, PartialEq)]
        pub enum Token {
            $( $name, )+
        }
        impl Token {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Token::$name => $string, )+
                }
            }
            pub fn as_keyword(ident: &str) -> Option<Token> {
                match ident {
                    $( $string => self::token_gen::token_gen_arms!(@KW_RES $name $(KW $mark)?), )+
                    _ => None,
                }
            }
            pub const fn as_un_op(self) -> Option<UnOp> {
                match self {
                    $( Token::$name => self::token_gen::token_gen_arms!(@UN_RES $(UN[$un_op])?), )+
                }
            }
            pub const fn as_bin_op(self) -> Option<BinOp> {
                match self {
                    $( Token::$name => self::token_gen::token_gen_arms!(@BIN_RES $(BIN[$bin_op])?), )+
                }
            }
            pub const fn as_assign_op(self) -> Option<AssignOp> {
                match self {
                    $( Token::$name => self::token_gen::token_gen_arms!(@ASSIGN_RES $(ASSIGN[$assign_op])?), )+
                }
            }
            pub const fn as_basic_type(self) -> Option<BasicType> {
                match self {
                    $( Token::$name => self::token_gen::token_gen_arms!(@BASIC_RES $(BASIC[$basic_ty])?), )+
                }
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! token_gen_arms {
    (@KW_RES $name:ident)                 => { None };
    (@UN_RES)                             => { None };
    (@BIN_RES)                            => { None };
    (@ASSIGN_RES)                         => { None };
    (@BASIC_RES)                          => { None };
    (@KW_RES $name:ident KW $mark:tt)     => { Some(Token::$name) };
    (@UN_RES UN[$un_op:expr])             => { Some($un_op) };
    (@BIN_RES BIN[$bin_op:expr])          => { Some($bin_op) };
    (@ASSIGN_RES ASSIGN[$assign_op:expr]) => { Some($assign_op) };
    (@BASIC_RES BASIC[$basic_ty:expr])    => { Some($basic_ty) };
}

#[rustfmt::skip]
macro_rules! token_from_char {
    {
    $(
        $ch:literal => $to:expr
    )+
    } => {
        impl Token {
            pub const fn from_char(c: char) -> Option<Token> {
                match c {
                    $(
                        $ch => Some($to),
                    )+
                    _ => None,
                }
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! token_glue_extend {
    {
    $name:ident,
    $(
        $( ($from:pat => $to:expr) )+ if $ch:literal
    )+
    } => {
        impl Token {
            pub const fn $name(c: char, token: Token) -> Option<Token> {
                match c {
                    $(
                        $ch => match token {
                            $( $from => Some($to), )+
                            _ => None,
                        },
                    )+
                    _ => None,
                }
            }
        }
    };
}

pub(super) use token_from_char;
pub(super) use token_gen;
pub(super) use token_gen_arms;
pub(super) use token_glue_extend;

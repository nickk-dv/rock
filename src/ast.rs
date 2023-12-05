use crate::ptr::*;
use crate::token::Span;
use decl::Decl;
use expr::Expr;
use stmt::Block;
use tt::Type;

type SourceID = u32;

#[derive(Copy, Clone)]
pub struct Ident {
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct Package {
    pub root: P<Module>,
}

#[derive(Copy, Clone)]
pub struct Module {
    pub source: SourceID,
    pub parent: Option<P<Module>>,
    pub submodules: List<P<Module>>,
    pub decls: List<Decl>,
}

#[derive(Copy, Clone)]
pub struct ModuleAccess {
    pub names: List<Ident>,
}

#[derive(Copy, Clone)]
pub struct Something {
    pub module_access: P<ModuleAccess>,
    pub access: P<Access>,
}

#[derive(Copy, Clone)]
pub struct Access {
    pub next: Option<P<Access>>,
    pub kind: AccessKind,
}

#[derive(Copy, Clone)]
pub enum AccessKind {
    Ident(Ident),
    Array(P<Expr>),
    Call(P<AccessCall>),
}

#[derive(Copy, Clone)]
pub struct AccessCall {
    pub name: Ident,
    pub input: List<P<Expr>>,
}

pub mod tt {
    use super::*;

    #[derive(Copy, Clone)]
    pub struct Type {
        pub pointer_level: u32,
        pub kind: TypeKind,
    }

    #[derive(Copy, Clone)]
    pub enum TypeKind {
        Basic(BasicType),
        Custom(P<Custom>),
        StaticArray(P<StaticArray>),
    }

    #[derive(Copy, Clone)]
    pub struct Custom {
        pub module_access: Option<P<ModuleAccess>>,
        pub name: Ident,
    }

    #[derive(Copy, Clone)]
    pub struct StaticArray {
        pub element: Type,
        pub constexpr_size: P<Expr>,
    }
}

pub mod decl {
    use super::*;

    #[derive(Copy, Clone)]
    pub enum Decl {
        Mod(P<Mod>),
        Impl(P<Impl>),
        Import(P<Import>),
        Proc(P<Proc>),
        Enum(P<Enum>),
        Struct(P<Struct>),
        Global(P<Global>),
    }

    #[derive(Copy, Clone)]
    pub struct Mod {
        pub is_pub: bool,
        pub name: Ident,
    }

    #[derive(Copy, Clone)]
    pub struct Impl {
        pub tt: Type,
        pub procs: List<P<Proc>>,
    }

    #[derive(Copy, Clone)]
    pub struct Import {
        pub module_names: List<Ident>,
        pub target: ImportTarget,
    }

    #[derive(Copy, Clone)]
    pub enum ImportTarget {
        Wildcard,
        SymbolList(List<Ident>),
        SymbolOrModule(Ident),
    }

    #[derive(Copy, Clone)]
    pub struct Proc {
        pub is_pub: bool,
        pub name: Ident,
        pub params: List<ProcParam>,
        pub is_variadic: bool,
        pub return_type: Option<Type>,
        pub is_external: bool,
        pub block: P<Block>,
    }

    #[derive(Copy, Clone)]
    pub struct ProcParam {
        pub is_mut: bool,
        pub kind: ParamKind,
    }

    #[derive(Copy, Clone)]
    pub enum ParamKind {
        SelfParam(Ident),
        Normal(ParamNormal),
    }

    #[derive(Copy, Clone)]
    pub struct ParamNormal {
        pub name: Ident,
        pub tt: Type,
    }

    #[derive(Copy, Clone)]
    pub struct Enum {
        pub is_pub: bool,
        pub name: Ident,
        pub basic_type: Option<BasicType>,
        pub variants: List<EnumVariant>,
    }

    #[derive(Copy, Clone)]
    pub struct EnumVariant {
        pub name: Ident,
        pub constexpr: P<Expr>,
    }

    #[derive(Copy, Clone)]
    pub struct Struct {
        pub is_pub: bool,
        pub name: Ident,
        pub fields: List<StructField>,
    }

    #[derive(Copy, Clone)]
    pub struct StructField {
        pub is_pub: bool,
        pub name: Ident,
        pub tt: Type,
        pub default_expr: Option<P<Expr>>,
    }

    #[derive(Copy, Clone)]
    pub struct Global {
        pub is_pub: bool,
        pub name: Ident,
        pub constexpr: P<Expr>,
    }
}

pub mod stmt {
    use super::*;

    #[derive(Copy, Clone)]
    pub enum Stmt {
        If(P<If>),
        For(P<For>),
        Block(P<Block>),
        Defer(P<Block>),
        Break,
        Return(P<Return>),
        Continue,
        VarDecl(P<VarDecl>),
        VarAssign(P<VarAssign>),
        ProcCall(P<Something>),
    }

    #[derive(Copy, Clone)]
    pub struct If {
        pub condition: P<Expr>,
        pub block: P<Block>,
        pub else_: Option<Else>,
    }

    #[derive(Copy, Clone)]
    pub enum Else {
        If(P<If>),
        Block(P<Block>),
    }

    #[derive(Copy, Clone)]
    pub struct For {
        pub var_decl: Option<P<VarDecl>>,
        pub condition: P<Expr>,
        pub var_assign: Option<P<VarAssign>>,
        pub block: P<Block>,
    }

    #[derive(Copy, Clone)]
    pub struct Block {
        pub is_short: bool,
        pub stmts: List<Stmt>,
    }

    #[derive(Copy, Clone)]
    pub struct Return {
        pub expr: Option<P<Expr>>,
    }

    #[derive(Copy, Clone)]
    pub struct VarDecl {
        pub is_mut: bool,
        pub name: Ident,
        pub tt: Option<Type>,
        pub expr: Option<P<Expr>>,
    }

    #[derive(Copy, Clone)]
    pub struct VarAssign {
        pub something: P<Something>,
        pub op: AssignOp,
        pub expr: P<Expr>,
    }
}

pub mod expr {
    use super::*;

    #[derive(Copy, Clone)]
    pub enum Expr {
        Unary(P<Unary>),
        Binary(P<Binary>),
        Enum(Enum),
        Cast(P<Cast>),
        Sizeof(P<Sizeof>),
        Literal(Literal),
        ArrayInit(P<ArrayInit>),
        StructInit(P<StructInit>),
        Something(P<Something>),
    }

    #[derive(Copy, Clone)]
    pub struct Unary {
        pub op: UnaryOp,
        pub rhs: P<Expr>,
    }

    #[derive(Copy, Clone)]
    pub struct Binary {
        pub op: BinaryOp,
        pub lhs: P<Expr>,
        pub rhs: P<Expr>,
    }

    #[derive(Copy, Clone)]
    pub struct Enum {
        pub variant_name: Ident,
    }

    #[derive(Copy, Clone)]
    pub struct Cast {
        pub expr: P<Expr>,
        pub into: BasicType,
    }

    #[derive(Copy, Clone)]
    pub struct Sizeof {
        pub tt: Type,
    }

    #[derive(Copy, Clone)]
    pub enum Literal {
        Uint(u64),
        Float(f64),
        Bool(bool),
    }

    #[derive(Copy, Clone)]
    pub struct ArrayInit {
        pub tt: Option<Type>,
        pub input: List<P<Expr>>,
    }

    #[derive(Copy, Clone)]
    pub struct StructInit {
        pub module_access: Option<P<ModuleAccess>>,
        pub struct_name: Option<Ident>,
        pub input: List<P<Expr>>,
    }
}

#[derive(Copy, Clone)]
pub enum UnaryOp {
    Minus,
    BitNot,
    LogicNot,
    AddressOf,
    Dereference,
}

#[derive(Copy, Clone)]
pub enum BinaryOp {
    LogicAnd,
    LogicOr,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    IsEq,
    NotEq,
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

#[derive(Copy, Clone)]
pub enum BasicType {
    S8,
    S16,
    S32,
    S64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    String,
}

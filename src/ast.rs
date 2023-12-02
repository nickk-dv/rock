use crate::ptr::*;
use decl::*;
use expr::*;
use tt::*;

struct Span {
    pub start: u32,
    pub end: u32,
}

pub struct Ident {
    pub span: Span,
}

pub struct Package {
    pub root: P<Module>,
}

pub struct Module {
    pub name: String,
    pub parent: Vec<P<Module>>,
    pub submodules: Vec<P<Module>>,
    pub decls: Vec<P<Decl>>,
}

pub struct ModuleAccess {
    pub names: Vec<Ident>,
}

pub struct Something {
    pub module_access: P<ModuleAccess>,
    pub access: P<Access>,
}

pub struct Access {
    pub next: Option<P<Access>>,
    pub kind: AccessKind,
}

pub enum AccessKind {
    Ident(Ident),
    Array(P<Expr>),
    Call(P<AccessCall>),
}

pub struct AccessCall {
    pub name: Ident,
    pub input: P<ExprList>,
}

pub mod tt {
    use super::*;

    pub enum Type {
        Basic(BasicType),
        Custom(P<Custom>),
        StaticArray(P<StaticArray>),
    }

    pub struct Custom {
        pub module_access: Option<P<ModuleAccess>>,
        pub name: Ident,
    }

    pub struct StaticArray {
        pub element: Type,
        pub constexpr_size: P<Expr>,
    }
}

pub mod decl {
    use super::*;

    pub enum Decl {
        Mod(P<Mod>),
        Impl(P<Impl>),
        Import(P<Import>),
        Proc(P<Proc>),
        Enum(P<Enum>),
        Struct(P<Struct>),
        Global(P<Global>),
    }

    pub struct Mod {
        pub is_pub: bool,
        pub name: Ident,
    }

    pub struct Impl {
        pub tt: Type,
        pub procs: Vec<P<Proc>>,
    }

    pub struct Import {
        pub module_names: Vec<Ident>,
        pub target: ImportTarget,
    }

    pub enum ImportTarget {
        Wildcard,
        SymbolList(Vec<Ident>),
        SymbolOrModule(Ident),
    }

    pub struct Proc {
        pub is_pub: bool,
        pub name: Ident,
        pub params: Vec<ProcParam>,
        pub is_variadic: bool,
        pub return_type: Option<Type>,
        pub is_external: bool,
    }

    pub struct ProcParam {
        pub is_mut: bool,
        pub is_self: bool,
        pub name: Ident,
        pub tt: Type,
    }

    pub struct Enum {
        pub is_pub: bool,
        pub name: Ident,
        pub basic_type: Option<BasicType>,
        pub variants: Vec<EnumVariant>,
    }

    pub struct EnumVariant {
        pub name: Ident,
        pub constexpr: P<Expr>,
    }

    pub struct Struct {
        pub is_pub: bool,
        pub name: Ident,
        pub fields: Vec<StructField>,
    }

    pub struct StructField {
        pub is_pub: bool,
        pub name: Ident,
        pub tt: Type,
        pub default_expr: Option<P<Expr>>,
    }

    pub struct Global {
        pub is_pub: bool,
        pub name: Ident,
        pub constexpr: P<Expr>,
    }
}

mod stmt {
    use super::*;

    pub enum Stmt {
        If(P<If>),
        For(P<For>),
        Block(P<Block>),
        Defer(P<Defer>),
        Break,
        Return(P<Return>),
        Switch(P<Switch>),
        Continue,
        VarDecl(P<VarDecl>),
        VarAssign(P<VarAssign>),
        ProcCall(P<ProcCall>),
    }

    pub struct If {
        pub condition: P<Expr>,
        pub block: P<Block>,
        pub else_: Option<Else>,
    }

    pub enum Else {
        If(P<If>),
        Block(P<Block>),
    }

    pub struct For {
        pub var_decl: Option<P<VarDecl>>,
        pub condition: P<Expr>,
        pub var_assign: Option<P<VarAssign>>,
        pub block: P<Block>,
    }

    pub struct Block {
        pub stmts: Vec<P<Stmt>>,
    }

    pub struct Defer {
        pub block: P<Block>,
    }

    pub struct Return {
        pub expr: Option<P<Expr>>,
    }

    pub struct Switch {
        pub expr: P<Expr>,
        pub cases: Vec<SwitchCase>,
    }

    pub struct SwitchCase {
        pub expr: P<Expr>,
        pub block: Option<P<Block>>,
    }

    pub struct VarDecl {
        pub is_mut: bool,
        pub name: Ident,
        pub tt: Option<Type>,
        pub expr: Option<P<Expr>>,
    }

    pub struct VarAssign {
        pub something: P<Something>,
        pub op: AssignOp,
        pub expr: P<Expr>,
    }

    pub struct ProcCall {
        pub something: P<Something>,
    }
}

pub mod expr {
    use super::*;

    pub enum Expr {
        Term(P<Term>),
        Unary(P<Unary>),
        Binary(P<Binary>),
    }

    pub struct ExprList {
        pub exprs: Vec<P<Expr>>,
    }

    pub enum Term {
        Enum(Enum),
        Cast(P<Cast>),
        Sizeof(P<Sizeof>),
        Literal(Literal),
        ArrayInit(P<ArrayInit>),
        StructInit(P<StructInit>),
        Something(P<Something>),
    }

    pub struct Unary {
        pub op: UnaryOp,
        pub rhs: P<Expr>,
    }

    pub struct Binary {
        pub op: BinaryOp,
        pub lhs: P<Expr>,
        pub rhs: P<Expr>,
    }

    pub struct Enum {
        pub variant_name: Ident,
    }

    pub struct Cast {
        pub expr: P<Expr>,
        pub into: BasicType,
    }

    pub struct Sizeof {
        pub tt: Type,
    }

    pub enum Literal {
        Uint(u64),
        Float(f64),
        Bool(bool),
    }

    pub struct ArrayInit {
        pub tt: Option<Type>,
        pub input: P<ExprList>,
    }

    pub struct StructInit {
        pub module_access: Option<P<ModuleAccess>>,
        pub struct_name: Option<Ident>,
        pub input: P<ExprList>,
    }
}

pub enum UnaryOp {
    Minus,
    BitNot,
    LogicNot,
    AddressOf,
    Dereference,
}

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

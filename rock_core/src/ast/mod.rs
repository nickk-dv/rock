use crate::arena::Arena;
use crate::intern::{InternID, InternPool};
use crate::session::FileID;
use crate::text::TextRange;

pub struct Ast<'ast, 'intern> {
    pub arena: Arena<'ast>,
    pub intern: InternPool<'intern>,
    pub packages: Vec<Package<'ast>>,
}

pub struct Package<'ast> {
    pub name_id: InternID,
    pub modules: Vec<Module<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Module<'ast> {
    pub file_id: FileID,
    pub name_id: InternID,
    pub items: &'ast [Item<'ast>],
}

#[derive(Copy, Clone)]
pub enum Item<'ast> {
    Proc(&'ast ProcItem<'ast>),
    Enum(&'ast EnumItem<'ast>),
    Union(&'ast UnionItem<'ast>),
    Struct(&'ast StructItem<'ast>),
    Const(&'ast ConstItem<'ast>),
    Global(&'ast GlobalItem<'ast>),
    Import(&'ast ImportItem<'ast>),
}

#[derive(Default)]
pub struct ItemCount {
    pub modules: u32,
    pub procs: u32,
    pub enums: u32,
    pub unions: u32,
    pub structs: u32,
    pub consts: u32,
    pub globals: u32,
}

#[derive(Copy, Clone)]
pub struct ProcItem<'ast> {
    pub attr: Option<Attribute>,
    pub vis: Vis,
    pub name: Name,
    pub params: &'ast [ProcParam<'ast>],
    pub is_variadic: bool,
    pub return_ty: Option<Type<'ast>>,
    pub attr_tail: Option<Attribute>,
    pub block: Option<Block<'ast>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam<'ast> {
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct EnumItem<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub basic: Option<BasicType>,
    pub variants: &'ast [EnumVariant<'ast>],
}

//@temporary always specified variant value (to simplify const evaluation) 09.05.24
#[derive(Copy, Clone)]
pub struct EnumVariant<'ast> {
    pub name: Name,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct UnionItem<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub members: &'ast [UnionMember<'ast>],
}

#[derive(Copy, Clone)]
pub struct UnionMember<'ast> {
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct StructItem<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub fields: &'ast [StructField<'ast>],
}

#[derive(Copy, Clone)]
pub struct StructField<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ConstItem<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct GlobalItem<'ast> {
    pub attr: Option<Attribute>,
    pub vis: Vis,
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ImportItem<'ast> {
    pub package: Option<Name>,
    pub module: Name,
    pub alias: Option<Name>,
    pub symbols: &'ast [ImportSymbol],
}

#[derive(Copy, Clone)]
pub struct ImportSymbol {
    pub name: Name,
    pub alias: Option<Name>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Vis {
    Public,
    Private,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Mut {
    Mutable,
    Immutable,
}

#[derive(Copy, Clone)]
pub struct Name {
    pub id: InternID,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Attribute {
    pub kind: AttributeKind,
    pub range: TextRange,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq)]
pub enum AttributeKind {
    Test,
    C_Call,
    Thread_Local,
    Unknown,
}

impl AttributeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            AttributeKind::Test => "test",
            AttributeKind::C_Call => "c_call",
            AttributeKind::Thread_Local => "thread_local",
            AttributeKind::Unknown => "unknown",
        }
    }

    pub fn from_str(string: &str) -> AttributeKind {
        match string {
            "test" => AttributeKind::Test,
            "c_call" => AttributeKind::C_Call,
            "thread_local" => AttributeKind::Thread_Local,
            _ => AttributeKind::Unknown,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Path<'ast> {
    pub names: &'ast [Name],
}

#[derive(Copy, Clone)]
pub enum Type<'ast> {
    Basic(BasicType),
    Custom(&'ast Path<'ast>),
    Reference(&'ast Type<'ast>, Mut),
    Procedure(&'ast ProcType<'ast>),
    ArraySlice(&'ast ArraySlice<'ast>),
    ArrayStatic(&'ast ArrayStatic<'ast>),
}

#[derive(Copy, Clone)]
pub struct ProcType<'ast> {
    pub params: &'ast [Type<'ast>],
    pub return_ty: Option<Type<'ast>>,
    pub is_variadic: bool,
}

#[derive(Copy, Clone)]
pub struct ArraySlice<'ast> {
    pub mutt: Mut,
    pub elem_ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic<'ast> {
    pub len: ConstExpr<'ast>,
    pub elem_ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct Stmt<'ast> {
    pub kind: StmtKind<'ast>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub enum StmtKind<'ast> {
    Break,
    Continue,
    Return(Option<&'ast Expr<'ast>>),
    Defer(&'ast Block<'ast>),
    ForLoop(&'ast For<'ast>),
    Local(&'ast Local<'ast>),
    Assign(&'ast Assign<'ast>),
    ExprSemi(&'ast Expr<'ast>),
    ExprTail(&'ast Expr<'ast>),
}

#[derive(Copy, Clone)]
pub struct For<'ast> {
    pub kind: ForKind<'ast>,
    pub block: Block<'ast>,
}

#[derive(Copy, Clone)]
pub enum ForKind<'ast> {
    Loop,
    While {
        cond: &'ast Expr<'ast>,
    },
    ForLoop {
        local: &'ast Local<'ast>,
        cond: &'ast Expr<'ast>,
        assign: &'ast Assign<'ast>,
    },
}

#[derive(Copy, Clone)]
pub struct Local<'ast> {
    pub mutt: Mut,
    pub name: Name,
    pub ty: Option<Type<'ast>>,
    pub value: Option<&'ast Expr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Assign<'ast> {
    pub op: AssignOp,
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct Block<'ast> {
    pub stmts: &'ast [Stmt<'ast>],
}

#[derive(Copy, Clone)]
pub struct Expr<'ast> {
    pub kind: ExprKind<'ast>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct ConstExpr<'ast>(pub &'ast Expr<'ast>);

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ExprKind<'ast> {
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64 },
    LitFloat    { val: f64 },
    LitChar     { val: char },
    LitString   { id: InternID, c_string: bool },
    If          { if_: &'ast If<'ast> },
    Block       { block: Block<'ast> },
    Match       { match_: &'ast Match<'ast> },
    Field       { target: &'ast Expr<'ast>, name: Name },
    Index       { target: &'ast Expr<'ast>, index: &'ast Expr<'ast> },
    Slice       { target: &'ast Expr<'ast>, mutt: Mut, slice_range: &'ast SliceRange<'ast> },
    Call        { target: &'ast Expr<'ast>, input: &'ast &'ast [&'ast Expr<'ast>] },
    Cast        { target: &'ast Expr<'ast>, into: &'ast Type<'ast> },
    Sizeof      { ty: Type<'ast> },
    Item        { path: &'ast Path<'ast> },
    StructInit  { struct_init: &'ast StructInit<'ast> },
    ArrayInit   { input: &'ast [&'ast Expr<'ast>] },
    ArrayRepeat { expr: &'ast Expr<'ast>, len: ConstExpr<'ast> },
    Address     { mutt: Mut, rhs: &'ast Expr<'ast> },
    Unary       { op: UnOp, rhs: &'ast Expr<'ast> },
    Binary      { op: BinOp, lhs: &'ast Expr<'ast>, rhs: &'ast Expr<'ast> },
}

#[derive(Copy, Clone)]
pub struct If<'ast> {
    pub entry: Branch<'ast>,
    pub branches: &'ast [Branch<'ast>],
    pub else_block: Option<Block<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Branch<'ast> {
    pub cond: &'ast Expr<'ast>,
    pub block: Block<'ast>,
}

#[derive(Copy, Clone)]
pub struct Match<'ast> {
    pub on_expr: &'ast Expr<'ast>,
    pub arms: &'ast [MatchArm<'ast>],
}

#[derive(Copy, Clone)]
pub struct MatchArm<'ast> {
    pub pat: Option<&'ast Expr<'ast>>,
    pub expr: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct StructInit<'ast> {
    pub path: &'ast Path<'ast>,
    pub input: &'ast [FieldInit<'ast>],
}

#[derive(Copy, Clone)]
pub struct FieldInit<'ast> {
    pub name: Name,
    pub expr: &'ast Expr<'ast>,
}

//@this syntax doesnt allow expressions that might be Range or RangeInc 07.05.24
// eg:
// let array = [1, 2, 3];
// let range = 0..<10;
// let slice: []s32 = array[range];

#[derive(Copy, Clone)]
pub struct SliceRange<'ast> {
    pub lower: Option<&'ast Expr<'ast>>,
    pub upper: SliceRangeEnd<'ast>,
}

#[derive(Copy, Clone)]
pub enum SliceRangeEnd<'ast> {
    Unbounded,
    Exclusive(&'ast Expr<'ast>),
    Inclusive(&'ast Expr<'ast>),
}

#[derive(Copy, Clone, PartialEq)]
pub enum BasicType {
    S8,
    S16,
    S32,
    S64,
    Ssize,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F16,
    F32,
    F64,
    Bool,
    Char,
    Rawptr,
    Void,
    Never,
}

#[derive(Copy, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    BitNot,
    LogicNot,
    Deref,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    BitShl,
    BitShr,
    IsEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    LogicAnd,
    LogicOr,
    Range,
    RangeInc,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Bin(BinOp),
}

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
mod size_assert {
    use super::*;
    macro_rules! size_assert {
        ($size:expr, $ty:ty) => {
            const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
        };
    }

    size_assert!(12, Name);
    size_assert!(16, Item);
    size_assert!(16, Path);
    size_assert!(16, Type);
    size_assert!(24, Stmt);
    size_assert!(32, Expr);
}

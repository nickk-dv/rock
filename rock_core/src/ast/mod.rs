use crate::arena::Arena;
use crate::intern::{InternID, InternPool};
use crate::session::FileID;
use crate::text::TextRange;

#[derive(Default)]
pub struct Ast<'ast> {
    pub arena: Arena<'ast>,
    pub intern: InternPool,
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

#[derive(Copy, Clone)]
pub struct ProcItem<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub params: &'ast [ProcParam<'ast>],
    pub is_variadic: bool,
    pub return_ty: Option<Type<'ast>>,
    pub directive_tail: Option<Directive>,
    pub block: Option<&'ast Expr<'ast>>,
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
    pub variants: &'ast [EnumVariant<'ast>],
}

#[derive(Copy, Clone)]
pub struct EnumVariant<'ast> {
    pub name: Name,
    pub value: Option<ConstExpr<'ast>>,
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
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ImportItem<'ast> {
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
pub struct Directive {
    pub name: Name,
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
    ArraySlice(&'ast ArraySlice<'ast>),
    ArrayStatic(&'ast ArrayStatic<'ast>),
}

#[derive(Copy, Clone)]
pub struct ArraySlice<'ast> {
    pub mutt: Mut,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic<'ast> {
    pub size: ConstExpr<'ast>,
    pub ty: Type<'ast>,
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
    Defer(&'ast Expr<'ast>),
    ForLoop(&'ast For<'ast>),
    Local(&'ast Local<'ast>),
    Assign(&'ast Assign<'ast>),
    ExprSemi(&'ast Expr<'ast>),
    ExprTail(&'ast Expr<'ast>),
}

#[derive(Copy, Clone)]
pub struct For<'ast> {
    pub kind: ForKind<'ast>,
    pub block: &'ast Expr<'ast>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ForKind<'ast> {
    Loop,
    While { cond: &'ast Expr<'ast> },
    ForLoop { local: &'ast Local<'ast>, cond: &'ast Expr<'ast>, assign: &'ast Assign<'ast> },
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
pub struct Expr<'ast> {
    pub kind: ExprKind<'ast>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct ConstExpr<'ast>(pub &'ast Expr<'ast>);

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ExprKind<'ast> {
    Unit,
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64 },
    LitFloat    { val: f64 },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: &'ast If<'ast> },
    Block       { stmts: &'ast [Stmt<'ast>] },
    Match       { match_: &'ast Match<'ast> },
    Field       { target: &'ast Expr<'ast>, name: Name },
    Index       { target: &'ast Expr<'ast>, index: &'ast Expr<'ast> },
    Cast        { target: &'ast Expr<'ast>, ty: &'ast Type<'ast> },
    Sizeof      { ty: Type<'ast> },
    Item        { path: &'ast Path<'ast> },
    ProcCall    { proc_call: &'ast ProcCall<'ast> },
    StructInit  { struct_init: &'ast StructInit<'ast> },
    ArrayInit   { input: &'ast [&'ast Expr<'ast>] },
    ArrayRepeat { expr: &'ast Expr<'ast>, size: ConstExpr<'ast> },
    Unary       { op: UnOp, rhs: &'ast Expr<'ast> },
    Binary      { op: BinOp, lhs: &'ast Expr<'ast>, rhs: &'ast Expr<'ast> },
}

#[derive(Copy, Clone)]
pub struct If<'ast> {
    pub entry: Branch<'ast>,
    pub branches: &'ast [Branch<'ast>],
    pub fallback: Option<&'ast Expr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Branch<'ast> {
    pub cond: &'ast Expr<'ast>,
    pub block: &'ast Expr<'ast>,
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
pub struct ProcCall<'ast> {
    pub path: &'ast Path<'ast>,
    pub input: &'ast [&'ast Expr<'ast>],
}

#[derive(Copy, Clone)]
pub struct StructInit<'ast> {
    pub path: &'ast Path<'ast>,
    pub input: &'ast [FieldInit<'ast>],
}

#[derive(Copy, Clone)]
pub struct FieldInit<'ast> {
    pub name: Name,
    pub expr: Option<&'ast Expr<'ast>>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BasicType {
    Unit,
    Bool,
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
    F32,
    F64,
    Char,
    Rawptr,
}

#[derive(Copy, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    BitNot,
    LogicNot,
    Deref,
    Addr(Mut),
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
    CmpIsEq,
    CmpNotEq,
    CmpLt,
    CmpLtEq,
    CmpGt,
    CmpGtEq,
    LogicAnd,
    LogicOr,
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

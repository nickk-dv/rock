use crate::arena::Arena;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};
use crate::vfs;

pub struct Ast<'ast> {
    pub arena: Arena<'ast>,
    pub modules: Vec<Module<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Module<'ast> {
    pub file_id: vfs::FileID,
    pub decls: &'ast [Decl<'ast>],
}

#[derive(Copy, Clone)]
pub enum Decl<'ast> {
    Mod(&'ast ModDecl),
    Use(&'ast UseDecl<'ast>),
    Proc(&'ast ProcDecl<'ast>),
    Enum(&'ast EnumDecl<'ast>),
    Union(&'ast UnionDecl<'ast>),
    Struct(&'ast StructDecl<'ast>),
    Const(&'ast ConstDecl<'ast>),
    Global(&'ast GlobalDecl<'ast>),
}

#[derive(Copy, Clone)]
pub struct ModDecl {
    pub vis: Vis,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct UseDecl<'ast> {
    pub path: &'ast Path<'ast>,
    pub symbols: &'ast [UseSymbol],
}

#[derive(Copy, Clone)]
pub struct UseSymbol {
    pub name: Ident,
    pub alias: Option<Ident>,
}

#[derive(Copy, Clone)]
pub struct ProcDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub params: &'ast [ProcParam<'ast>],
    pub is_variadic: bool,
    pub return_ty: Option<Type<'ast>>,
    pub directive_tail: Option<Directive>,
    pub block: Option<&'ast Expr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam<'ast> {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct EnumDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub variants: &'ast [EnumVariant<'ast>],
}

#[derive(Copy, Clone)]
pub struct EnumVariant<'ast> {
    pub name: Ident,
    pub value: Option<ConstExpr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct UnionDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub members: &'ast [UnionMember<'ast>],
}

#[derive(Copy, Clone)]
pub struct UnionMember<'ast> {
    pub name: Ident,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct StructDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub fields: &'ast [StructField<'ast>],
}

#[derive(Copy, Clone)]
pub struct StructField<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ConstDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct GlobalDecl<'ast> {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
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
pub struct Ident {
    pub id: InternID,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Directive {
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct Path<'ast> {
    pub kind: PathKind,
    pub names: &'ast [Ident],
    pub range_start: TextOffset,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PathKind {
    None,
    Super,
    Package,
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
    VarDecl(&'ast VarDecl<'ast>),
    VarAssign(&'ast VarAssign<'ast>),
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
    ForLoop { var_decl: &'ast VarDecl<'ast>, cond: &'ast Expr<'ast>, var_assign: &'ast VarAssign<'ast> },
}

#[derive(Copy, Clone)]
pub struct VarDecl<'ast> {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Option<Type<'ast>>,
    pub expr: Option<&'ast Expr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct VarAssign<'ast> {
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
    Field       { target: &'ast Expr<'ast>, name: Ident },
    Index       { target: &'ast Expr<'ast>, index: &'ast Expr<'ast> },
    Cast        { target: &'ast Expr<'ast>, ty: &'ast Type<'ast> },
    Sizeof      { ty: Type<'ast> },
    Item        { path: &'ast Path<'ast> },
    ProcCall    { proc_call: &'ast ProcCall<'ast> },
    StructInit  { struct_init: &'ast StructInit<'ast> },
    ArrayInit   { input: &'ast [&'ast Expr<'ast>] },
    ArrayRepeat { expr: &'ast Expr<'ast>, size: ConstExpr<'ast> },
    UnaryExpr   { op: UnOp, rhs: &'ast Expr<'ast> },
    BinaryExpr  { op: BinOp, lhs: &'ast Expr<'ast>, rhs: &'ast Expr<'ast> },
}

#[derive(Copy, Clone)]
pub struct If<'ast> {
    pub cond: &'ast Expr<'ast>,
    pub block: &'ast Expr<'ast>,
    pub else_: Option<Else<'ast>>,
}

#[derive(Copy, Clone)]
pub enum Else<'ast> {
    If { else_if: &'ast If<'ast> },
    Block { block: &'ast Expr<'ast> },
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
    pub name: Ident,
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

    size_assert!(12, Ident);
    size_assert!(16, Decl);
    size_assert!(24, Path);
    size_assert!(16, Type);
    size_assert!(24, Stmt);
    size_assert!(32, Expr);
}

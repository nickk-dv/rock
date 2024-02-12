use super::intern::*;
use super::parser::FileID;
use super::span::Span;
use crate::hir::scope::{EnumID, StructID, UnionID};
use crate::mem::*;

// 50616
// 50576 removed span of mod decl (wasnt used)
// 49120 changed span to span_start in path (only used to report "super.." error)
// 48544 encoded expr_semi & expr_tail into stmt instead of allocating separate 16 byte node
// 48272 store return expr directly in the stmt

pub type ScopeID = u32;

pub struct Ast {
    pub arena: Arena,
    pub modules: Vec<P<Module>>,
}

#[derive(Copy, Clone)]
pub struct Module {
    pub file_id: FileID,
    pub decls: List<Decl>,
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
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct Path {
    pub kind: PathKind,
    pub names: List<Ident>,
    pub span_start: u32,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PathKind {
    None,
    Super,
    Package,
}

#[derive(Copy, Clone)]
pub struct Type {
    pub ptr: PtrLevel,
    pub kind: TypeKind,
}

#[derive(Copy, Clone)]
pub struct PtrLevel {
    level: u8,
    mut_mask: u8,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    Basic(BasicType),
    Custom(P<ItemName>),
    ArraySlice(P<ArraySlice>),
    ArrayStatic(P<ArrayStatic>),
    Enum(EnumID),     //check
    Union(UnionID),   //check
    Struct(StructID), //check
    Poison,           //check
}

#[derive(Copy, Clone)]
pub struct ArraySlice {
    pub mutt: Mut,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic {
    pub size: ConstExpr,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub enum Decl {
    Module(P<ModuleDecl>),
    Import(P<ImportDecl>),
    Global(P<GlobalDecl>),
    Proc(P<ProcDecl>),
    Enum(P<EnumDecl>),
    Union(P<UnionDecl>),
    Struct(P<StructDecl>),
}

#[derive(Copy, Clone)]
pub struct ModuleDecl {
    pub vis: Vis,
    pub name: Ident,
    pub id: Option<ScopeID>, //@ move into "mod_data" when checking?
}

#[derive(Copy, Clone)]
pub struct ImportDecl {
    pub path: Path,
    pub target: ImportTarget,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum ImportTarget {
    GlobAll,
    Symbol(Ident),
    SymbolList(List<Ident>),
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: ConstExpr,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub vis: Vis,
    pub name: Ident,
    pub params: List<ProcParam>,
    pub is_variadic: bool,
    pub return_ty: Option<Type>,
    pub block: Option<P<Block>>, //@ None acts like external c_call
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct EnumDecl {
    pub vis: Vis,
    pub name: Ident,
    pub basic_ty: Option<BasicType>,
    pub variants: List<EnumVariant>,
}

#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub value: Option<ConstExpr>,
}

#[derive(Copy, Clone)]
pub struct UnionDecl {
    pub vis: Vis,
    pub name: Ident,
    pub members: List<UnionMember>,
}

#[derive(Copy, Clone)]
pub struct UnionMember {
    pub name: Ident,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub vis: Vis,
    pub name: Ident,
    pub fields: List<StructField>,
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum StmtKind {
    Break,
    Continue,
    For(P<For>),
    Defer(P<Block>),
    Return(Option<P<Expr>>),
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    ExprSemi(P<Expr>),
    ExprTail(P<Expr>),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<P<VarDecl>>, //@can be encoded using enum with less size
    pub cond: Option<P<Expr>>,
    pub var_assign: Option<P<VarAssign>>,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub mutt: Mut,
    pub name: Option<Ident>,
    pub ty: Option<Type>,
    pub expr: Option<P<Expr>>,
}

#[derive(Copy, Clone)]
pub struct VarAssign {
    pub lhs: P<Expr>,
    pub rhs: P<Expr>,
    pub op: AssignOp,
}

#[derive(Copy, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct ConstExpr(pub P<Expr>);

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ExprKind {
    Unit,
    Discard,
    LitNull,
    LitBool     { val: bool },
    LitUint     { val: u64, ty: Option<BasicType> },
    LitFloat    { val: f64, ty: Option<BasicType> },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: P<If> },
    Block       { block: P<Block> },
    Match       { expr: P<Expr>, arms: List<MatchArm> },
    Field       { target: P<Expr>, name: Ident },
    Index       { target: P<Expr>, index: P<Expr> },
    Cast        { target: P<Expr>, ty: Type },
    Sizeof      { ty: Type },
    Item        { item: P<ItemName> },
    ProcCall    { item: P<ItemName>, input: List<P<Expr>> },
    StructInit  { item: P<ItemName>, input: List<FieldInit> },
    ArrayInit   { input: List<P<Expr>> },
    ArrayRepeat { expr: P<Expr>, size: ConstExpr },
    UnaryExpr   { op: UnOp, rhs: P<Expr> },
    BinaryExpr  { op: BinOp, lhs: P<Expr>, rhs: P<Expr> },
}

#[derive(Copy, Clone)]
pub struct If {
    pub cond: P<Expr>,
    pub block: P<Block>,
    pub else_: Option<Else>,
}

#[derive(Copy, Clone)]
pub enum Else {
    If(P<If>),
    Block(P<Block>),
}

#[derive(Copy, Clone)]
pub struct Block {
    pub stmts: List<Stmt>,
}

#[derive(Copy, Clone)]
pub struct MatchArm {
    pub pat: P<Expr>,
    pub expr: P<Expr>,
}

#[derive(Copy, Clone)]
pub struct ItemName {
    pub path: Path,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct FieldInit {
    pub name: Ident,
    pub expr: Option<P<Expr>>,
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
}

#[derive(Copy, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    BitNot,
    LogicNot,
    Addr(Mut),
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

impl PtrLevel {
    const MAX_LEVEL: u8 = 8;

    pub fn new() -> Self {
        Self {
            level: 0,
            mut_mask: 0,
        }
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn add_level(&mut self, mutt: Mut) -> Result<(), ()> {
        if self.level >= Self::MAX_LEVEL {
            return Err(());
        }
        let mut_bit = 1u8 << (self.level);
        if mutt == Mut::Mutable {
            self.mut_mask |= mut_bit;
        } else {
            self.mut_mask &= !mut_bit;
        }
        self.level += 1;
        Ok(())
    }
}

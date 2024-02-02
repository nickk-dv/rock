use super::span::Span;
use crate::hir::scope::{EnumID, ProcID, StructID, UnionID};
use crate::mem::*;
use std::path::PathBuf;

//@look into parsing .name into the
// ExprKind with (P<Expr> Ident) instead of binary op approach
// same for casts + look into using "as" since its easier syntax
// compared to cast(<type>, <expr>)

pub type ScopeID = u32;

pub struct Ast {
    pub arenas: Vec<Arena>,
    pub modules: Vec<P<Module>>,
    pub intern_pool: P<InternPool>,
}

pub struct Module {
    pub file: SourceFile,
    pub decls: List<Decl>,
}

pub struct SourceFile {
    pub path: PathBuf,
    pub source: String,
    pub line_spans: Vec<Span>, //@not always required, could be generated on demand
    pub lex_strings: Vec<String>, //@never freeing strings after they get interned
}

#[derive(Copy, Clone)]
pub struct Ident {
    pub id: InternID,
    pub span: Span,
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
pub struct Path {
    pub kind: PathKind,
    pub kind_span: Span,
    pub names: List<Ident>,
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
    pub level: u8,
    mut_mask: u8,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    Basic(BasicType),
    Custom(P<CustomType>),
    ArraySlice(P<ArraySlice>),
    ArrayStatic(P<ArrayStatic>),
    Enum(EnumID),     //check
    Union(UnionID),   //check
    Struct(StructID), //check
    Poison,           //check
}

#[derive(Copy, Clone, PartialEq)]
pub enum BasicType {
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

#[derive(Copy, Clone)]
pub struct CustomType {
    pub path: Path,
    pub name: Ident,
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
    pub span: Span,
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
    Return(P<Return>),
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    ExprStmt(P<ExprStmt>),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<P<VarDecl>>,
    pub cond: Option<P<Expr>>,
    pub var_assign: Option<P<VarAssign>>,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Return {
    pub expr: Option<P<Expr>>,
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
pub enum AssignOp {
    Assign,
    BinaryOp(BinaryOp),
}

#[derive(Copy, Clone)]
pub struct ExprStmt {
    pub expr: P<Expr>,
    pub has_semi: bool,
}

#[derive(Copy, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct ConstExpr(pub P<Expr>);

#[derive(Copy, Clone)]
pub enum ExprKind {
    Discard,
    Lit(Lit),
    If(P<If>),
    Block(P<Block>),
    Match(P<Match>),
    Index(P<Index>),
    DotName(Ident),
    Cast(P<Cast>),
    Sizeof(P<Sizeof>),
    Item(P<Item>),
    ProcCall(P<ProcCall>),
    ArrayInit(P<ArrayInit>),
    ArrayRepeat(P<ArrayRepeat>),
    StructInit(P<StructInit>),
    UnaryExpr(P<UnaryExpr>),
    BinaryExpr(P<BinaryExpr>),
}

#[derive(Copy, Clone)]
pub enum Lit {
    Null,
    Bool(bool),
    Uint(u64, Option<BasicType>),
    Float(f64, Option<BasicType>),
    Char(char),
    String(InternID),
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
pub struct Match {
    pub expr: P<Expr>,
    pub arms: List<MatchArm>,
}

#[derive(Copy, Clone)]
pub struct MatchArm {
    pub pat: P<Expr>,
    pub expr: P<Expr>,
}

#[derive(Copy, Clone)]
pub struct Index {
    pub expr: P<Expr>,
}

#[derive(Copy, Clone)]
pub struct Cast {
    pub ty: Type,
    pub expr: P<Expr>,
}

#[derive(Copy, Clone)]
pub struct Sizeof {
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct Item {
    pub path: Path,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct ProcCall {
    pub path: Path,
    pub name: Ident,
    pub input: List<P<Expr>>,
    pub id: Option<ProcID>, //check
}

#[derive(Copy, Clone)]
pub struct ArrayInit {
    pub input: List<P<Expr>>,
}

#[derive(Copy, Clone)]
pub struct ArrayRepeat {
    pub expr: P<Expr>,
    pub size: ConstExpr,
}

#[derive(Copy, Clone)]
pub struct StructInit {
    pub path: Path,
    pub name: Ident,
    pub input: List<FieldInit>,
    pub ty: StructInitResolved, //check
}

#[derive(Copy, Clone)]
pub enum StructInitResolved {
    Union(UnionID),
    Struct(StructID),
    Poison,
}

#[derive(Copy, Clone)]
pub struct FieldInit {
    pub name: Ident,
    pub expr: Option<P<Expr>>,
}

#[derive(Copy, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub rhs: P<Expr>,
}

#[derive(Copy, Clone)]
pub enum UnaryOp {
    Neg,
    BitNot,
    LogicNot,
    Addr(Mut),
    Deref,
}

#[derive(Copy, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: P<Expr>,
    pub rhs: P<Expr>,
}

#[derive(Copy, Clone, PartialEq)]
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
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Deref,
    Index,
}

impl PtrLevel {
    const MAX_LEVEL: u8 = 8;

    pub fn new() -> Self {
        Self {
            level: 0,
            mut_mask: 0,
        }
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

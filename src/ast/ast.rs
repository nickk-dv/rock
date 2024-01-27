use super::span::Span;
use crate::mem::*;
use std::path::PathBuf;

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
    pub line_spans: Vec<Span>,
}

#[derive(Copy, Clone)]
pub struct Ident {
    pub id: InternID,
    pub span: Span,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Mutability {
    Mutable,
    Immutable,
}

#[derive(Copy, Clone)]
pub struct GenericArgs {
    pub types: List<Type>,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct GenericParams {
    pub names: List<Ident>,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct ModulePath {
    pub kind: ModulePathKind,
    pub kind_span: Span,
    pub names: List<Ident>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum ModulePathKind {
    None,
    Super,
    Package,
}

#[derive(Copy, Clone)]
pub struct Type {
    pub pointer_level: u32,
    pub mutt: Mutability,
    pub kind: TypeKind,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    Basic(BasicType),
    Custom(P<CustomType>),
    ArraySlice(P<ArraySlice>),
    ArrayStatic(P<ArrayStatic>),
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
    pub module_path: ModulePath,
    pub name: Ident,
    pub generic_args: Option<GenericArgs>,
}

#[derive(Copy, Clone)]
pub struct ArraySlice {
    pub mutt: Mutability,
    pub element: Type,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic {
    pub size: ConstExpr,
    pub element: Type,
}

#[derive(Copy, Clone)]
pub enum Decl {
    Mod(P<ModDecl>),
    Proc(P<ProcDecl>),
    Impl(P<ImplDecl>),
    Enum(P<EnumDecl>),
    Struct(P<StructDecl>),
    Global(P<GlobalDecl>),
    Import(P<ImportDecl>),
}

#[derive(Copy, Clone)]
pub struct ModDecl {
    pub vis: Visibility,
    pub name: Ident,
    pub id: Option<ScopeID>,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub vis: Visibility,
    pub name: Ident,
    pub generic_params: Option<GenericParams>,
    pub params: List<ProcParam>,
    pub is_variadic: bool,
    pub return_type: Option<Type>,
    pub block: Option<P<Block>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub mutt: Mutability,
    pub name: Ident,
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct ImplDecl {
    pub vis_span: Option<Span>,
    pub name: Ident,
    pub generic_params: Option<GenericParams>,
    pub procs: List<P<ProcDecl>>,
}

#[derive(Copy, Clone)]
pub struct EnumDecl {
    pub vis: Visibility,
    pub name: Ident,
    pub generic_params: Option<GenericParams>,
    pub basic_type: Option<BasicType>,
    pub variants: List<EnumVariant>,
}

#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub expr: Option<ConstExpr>,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub vis: Visibility,
    pub name: Ident,
    pub generic_params: Option<GenericParams>,
    pub fields: List<StructField>,
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub name: Ident,
    pub ty: Type,
    pub default: Option<ConstExpr>,
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub vis: Visibility,
    pub name: Ident,
    pub ty: Option<Type>,
    pub expr: ConstExpr,
}

#[derive(Copy, Clone)]
pub struct ImportDecl {
    pub module_path: ModulePath,
    pub target: ImportTarget,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum ImportTarget {
    AllSymbols,
    Symbol(Ident),
    SymbolList(List<Ident>),
}

#[derive(Copy, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum StmtKind {
    For(P<For>),
    Defer(P<Block>),
    Break,
    Return(P<Return>),
    Continue,
    ExprStmt(P<ExprStmt>),
    Assignment(P<Assignment>),
    VarDecl(P<VarDecl>),
    ProcCall(P<ProcCall>),
}

#[derive(Copy, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub has_semi: bool,
}

#[derive(Copy, Clone)]
pub struct Assignment {
    pub lhs: Expr,
    pub op: AssignOp,
    pub rhs: Expr,
}

#[derive(Copy, Clone)]
pub struct If {
    pub condition: Expr,
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
    pub kind: ForKind,
    pub block: P<Block>,
}

//@mem usage, move to expr as pointer vs as value
#[derive(Copy, Clone)]
pub enum ForKind {
    Loop,
    While(Expr),
    Iter(Option<Ident>, Expr),
    Range(Option<Ident>, Range),
}

#[derive(Copy, Clone)]
pub struct Range {
    pub lhs: Expr,
    pub rhs: Expr,
    pub kind: RangeKind,
}

#[derive(Copy, Clone)]
pub enum RangeKind {
    DotDot,
    DotDotEq,
}

//10..=0
//0..=10
//10..0
//0..10

#[derive(Copy, Clone)]
pub struct Block {
    pub stmts: List<Stmt>,
}

#[derive(Copy, Clone)]
pub struct Match {
    pub expr: Expr,
    pub arms: List<MatchArm>,
}

#[derive(Copy, Clone)]
pub struct MatchArm {
    pub pattern: Expr, //@pattern
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct Return {
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub mutt: Mutability,
    pub name: Option<Ident>,
    pub ty: Option<Type>,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    BinaryOp(BinaryOp),
}

#[derive(Copy, Clone)]
pub struct ConstExpr(pub Expr);

#[derive(Copy, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum ExprKind {
    Discard,
    Lit(Lit),
    If(P<If>),
    Block(P<Block>),
    Match(P<Match>),
    Index(P<Index>),
    DotName(Ident),
    DotCall(P<Call>),
    Item(Type),
    Cast(P<Cast>),
    Sizeof(P<Sizeof>),
    ProcCall(P<ProcCall>),
    ArrayInit(P<ArrayInit>),
    StructInit(P<StructInit>),
    UnaryExpr(P<UnaryExpr>),
    BinaryExpr(P<BinaryExpr>),
}

#[derive(Copy, Clone)]
pub struct Index {
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct Call {
    pub name: Ident,
    pub generic_args: Option<GenericArgs>,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub enum Lit {
    Null,
    Bool(bool),
    Uint(u64, Option<BasicType>),
    Float(f64, Option<BasicType>),
    Char(char),
    String,
}

#[derive(Copy, Clone)]
pub struct Cast {
    pub ty: Type,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct Sizeof {
    pub ty: Type,
}

#[derive(Copy, Clone)]
pub struct ProcCall {
    pub ty: Type,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub struct ArrayInit {
    pub ty: Option<Type>,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub struct StructInit {
    pub ty: Type,
    pub input: List<FieldInit>,
}

#[derive(Copy, Clone)]
pub struct FieldInit {
    pub name: Ident,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub rhs: Expr,
}

#[derive(Copy, Clone)]
pub enum UnaryOp {
    Neg,
    BitNot,
    LogicNot,
    Addr(Mutability),
    Deref,
}

#[derive(Copy, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Expr,
    pub rhs: Expr,
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

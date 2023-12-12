use super::token::Span;
use crate::mem::*;

type SourceID = u32;

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
pub struct Ident {
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct ModuleAccess {
    pub names: List<Ident>,
}

#[derive(Copy, Clone)]
pub struct GenericDeclaration {
    pub names: List<Ident>,
}

#[derive(Copy, Clone)]
pub struct GenericSpecialization {
    pub types: List<Type>,
}

#[derive(Copy, Clone)]
pub struct Type {
    pub pointer_level: u32,
    pub kind: TypeKind,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    Basic(BasicType),
    Custom(P<CustomType>),
    ArraySlice(P<ArraySliceType>),
    ArrayStatic(P<ArrayStaticType>),
}

#[derive(Copy, Clone)]
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
    Void,
}

#[derive(Copy, Clone)]
pub struct CustomType {
    pub module_access: Option<ModuleAccess>,
    pub name: Ident,
    pub generic: Option<GenericSpecialization>,
}

#[derive(Copy, Clone)]
pub struct ArraySliceType {
    pub element: Type,
}

#[derive(Copy, Clone)]
pub struct ArrayStaticType {
    pub size: Expr,
    pub element: Type,
}

#[derive(Copy, Clone)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Copy, Clone)]
pub enum Mutability {
    Mutable,
    Immutable,
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
    pub visibility: Visibility,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub generic: Option<GenericDeclaration>,
    pub params: List<ProcParam>,
    pub is_variadic: bool,
    pub return_type: Option<Type>,
    pub block: Option<P<Block>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub mutability: Mutability,
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
pub struct ImplDecl {
    pub tt: Type,
    pub procs: List<P<ProcDecl>>,
}

#[derive(Copy, Clone)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub generic: Option<GenericDeclaration>,
    pub basic_type: Option<BasicType>,
    pub variants: List<EnumVariant>,
}

#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub generic: Option<GenericDeclaration>,
    pub fields: List<StructField>,
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub visibility: Visibility,
    pub name: Ident,
    pub tt: Type,
    pub default: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct ImportDecl {
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
pub enum Stmt {
    For(P<For>),
    Defer(P<Block>),
    Break,
    Return(P<Return>),
    Continue,
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    Expression(Expr),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<P<VarDecl>>,
    pub condition: Option<Expr>,
    pub var_assign: Option<P<VarAssign>>,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Return {
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub mutability: Mutability,
    pub name: Ident,
    pub tt: Option<Type>,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarAssign {
    pub access: P<AccessChain>,
    pub op: AssignOp,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    BinaryOp(BinaryOp),
}

#[derive(Copy, Clone)]
pub enum Expr {
    If(P<If>),
    Enum(P<Enum>),
    Cast(P<Cast>),
    Block(P<Block>),
    Match(P<Match>),
    Sizeof(P<Sizeof>),
    Literal(P<Literal>),
    ArrayInit(P<ArrayInit>),
    StructInit(P<StructInit>),
    AccessChain(P<AccessChain>),
    UnaryExpr(P<UnaryExpr>),
    BinaryExpr(P<BinaryExpr>),
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
pub struct Enum {
    pub variant_name: Ident,
    pub destructure: Option<EnumDestructure>,
}

#[derive(Copy, Clone)]
pub enum EnumDestructure {
    Ignore,
    Access(Ident),
    ExprPass(Expr),
}

#[derive(Copy, Clone)]
pub struct Cast {
    pub expr: Expr,
    pub into: BasicType,
}

#[derive(Copy, Clone)]
pub struct Block {
    pub is_short: bool,
    pub stmts: List<Stmt>,
}

#[derive(Copy, Clone)]
pub struct Match {
    pub expr: Expr,
    pub arms: List<MatchArm>,
}

#[derive(Copy, Clone)]
pub struct MatchArm {
    pub expr: Expr,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Sizeof {
    pub tt: Type,
}

#[derive(Copy, Clone)]
pub enum Literal {
    Null,
    Bool(bool),
    Uint(u64),
    Float(f64),
    String,
}

#[derive(Copy, Clone)]
pub struct ArrayInit {
    pub tt: Option<Type>,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub struct StructInit {
    pub module_access: Option<ModuleAccess>,
    pub struct_name: Option<Ident>,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub struct AccessChain {
    pub module_access: Option<ModuleAccess>,
    pub access: P<Access>,
}

#[derive(Copy, Clone)]
pub struct Access {
    pub kind: AccessKind,
    pub next: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub enum AccessKind {
    Ident(Ident),
    Array(Expr),
    Call(P<AccessCall>),
}

#[derive(Copy, Clone)]
pub struct AccessCall {
    pub name: Ident,
    pub generic: Option<GenericSpecialization>,
    pub input: List<Expr>,
}

#[derive(Copy, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub rhs: Expr,
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
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Expr,
    pub rhs: Expr,
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

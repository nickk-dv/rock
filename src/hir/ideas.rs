use super::span::Span;
use crate::mem::*;
use std::path::PathBuf;

pub type ScopeID = u32;

pub struct Ast {
    pub arenas: Vec<Arena>,
    pub modules: Vec<P<Module>>,
    pub intern_pool: P<InternPool>,
}

//CONSEPT OF MODULE ISNT REQUIRED ANYMORE
/*
pub struct Module {
    pub file: SourceFile,
    pub decls: List<Decl>,
}
*/

// STILL IS ACCOCIATED WITH SCOPES
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

//ALL MODULES WILL BE RESOLVED
/*
#[derive(Copy, Clone)]
pub struct ModuleAccess {
    pub modifier: ModuleAccessModifier,
    pub modifier_span: Span,
    pub names: List<Ident>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum ModuleAccessModifier {
    None,
    Super,
    Package,
}
*/

#[derive(Copy, Clone)]
pub struct Type {
    pub pointer_level: u32,
    pub kind: TypeKind,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    //Poison                 //poinson / broken type
    Basic(BasicType),
    //Custom(P<CustomType>), //name would be resolved
    //Enum(EnumID)
    //Struct(StructID)
    ArraySlice(P<ArraySliceType>),
    ArrayStatic(P<ArrayStaticType>),
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

/*
#[derive(Copy, Clone)]
pub struct CustomType {
    pub module_access: ModuleAccess,
    pub name: Ident,
}
*/

#[derive(Copy, Clone)]
pub struct ArraySliceType {
    pub element: Type,
}

#[derive(Copy, Clone)]
pub struct ArrayStaticType {
    pub size: Expr,
    pub element: Type,
}

//MIGHT BE STILL NEEDED?
#[derive(Copy, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

//CONCEPT OF DECL DOESNT EXIST ANYMORE
/*
#[derive(Copy, Clone)]
pub enum Decl {
    Mod(P<ModDecl>),
    Proc(P<ProcDecl>),
    Enum(P<EnumDecl>),
    Struct(P<StructDecl>),
    Global(P<GlobalDecl>),
    Import(P<ImportDecl>),
}
*/

//ALL MOD accesses mush have been resolved by now
/*
#[derive(Copy, Clone)]
pub struct ModDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub id: Option<ScopeID>,
}
*/

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub params: List<ProcParam>, //List -> Array Unique nameset
    pub is_variadic: bool,
    pub return_type: Option<Type>,
    pub block: Option<P<Block>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub name: Ident,
    pub tt: Type,
}

#[derive(Copy, Clone)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub basic_type: Option<BasicType>,
    pub variants: List<EnumVariant>, //List -> Array Unique nameset
}

#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub fields: List<StructField>, //List -> Array Unique nameset
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub name: Ident,
    pub tt: Type,
    pub default: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub visibility: Visibility,
    pub name: Ident,
    pub tt: Option<Type>,
    pub expr: Expr,
}

//Imports would be already done
/*
#[derive(Copy, Clone)]
pub struct ImportDecl {
    pub span: Span,
    pub module_access: ModuleAccess,
    pub target: ImportTarget,
}

#[derive(Copy, Clone)]
pub enum ImportTarget {
    AllSymbols,
    Symbol(Ident),
    SymbolList(List<Ident>),
}
*/

#[derive(Copy, Clone)]
pub enum Stmt {
    If(P<If>),
    For(P<For>),
    Block(P<Block>),
    Defer(P<Block>),
    Break,
    Switch(P<Switch>),
    Return(P<Return>),
    Continue,
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    ProcCall(P<ProcCall>),
}

//REPLACED BY CONDITIONAL JUMPS TO BB
/*
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
*/

//FOR WOULD BE DECONSTRUCTED INTO LOOP
//with inner var decl / var assignments
/*
#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<P<VarDecl>>,
    pub condition: Option<Expr>,
    pub var_assign: Option<P<VarAssign>>,
    pub block: P<Block>,
}
*/

//strip empty blocks only containing another block
// Would use BB + Jumps directly
#[derive(Copy, Clone)]
pub struct Block {
    pub stmts: List<Stmt>, //List -> Array, Consept of Stmt still remains like "entity" inside the BB flow
}

#[derive(Copy, Clone)]
pub struct Switch {
    pub expr: Expr,
    pub cases: List<SwitchCase>, //List -> Array
}

#[derive(Copy, Clone)]
pub struct SwitchCase {
    pub expr: Expr,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Return {
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub name: Ident,
    pub tt: Option<Type>,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarAssign {
    pub var: P<Var>,
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
    Var(P<Var>),
    Enum(P<Enum>),
    Cast(P<Cast>),
    Sizeof(P<Sizeof>),
    Literal(P<Literal>),
    ProcCall(P<ProcCall>),
    ArrayInit(P<ArrayInit>),
    StructInit(P<StructInit>),
    UnaryExpr(P<UnaryExpr>),
    BinaryExpr(P<BinaryExpr>),
}

//Some Vars might end up as:
// - Global constants
// - Local variables
// - Enum with Variant represented by single field access
//Can we assign IDs to local variables?
#[derive(Copy, Clone)]
pub struct Var {
    pub module_access: ModuleAccess,
    pub name: Ident,
    pub access: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub struct Access {
    pub kind: AccessKind,
    pub next: Option<P<Access>>,
}

//Even without full constant information
//field ids can be found (or their invalidity)
#[derive(Copy, Clone)]
pub enum AccessKind {
    Field(Ident),
    Array(Expr),
}

//Will be expanded to support Optionally Known EnumID
#[derive(Copy, Clone)]
pub struct Enum {
    pub variant: Ident,
}

#[derive(Copy, Clone)]
pub struct Cast {
    pub tt: Type,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct Sizeof {
    pub tt: Type,
}

#[derive(Copy, Clone)]
pub enum Literal {
    Null,
    Bool(bool),
    Uint(u64, Option<BasicType>),
    Float(f64, Option<BasicType>),
    Char(char),
    String,
}

#[derive(Copy, Clone)]
pub struct ProcCall {
    pub module_access: ModuleAccess, //ID instead
    pub name: Ident,
    pub input: List<Expr>, //Array
    pub access: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub struct ArrayInit {
    pub tt: Option<Type>,
    pub input: List<Expr>, //Array
}

#[derive(Copy, Clone)]
pub struct StructInit {
    pub module_access: ModuleAccess, //Optional StructID would be used instead
    pub name: Option<Ident>,
    pub input: List<Expr>, //Array
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

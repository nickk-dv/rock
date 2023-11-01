use super::token::*;
use std::collections::HashMap;

pub struct AstProgram<'a> {
    pub modules: Vec<&'a mut Ast<'a>>,
    pub module_map: HashMap<String, &'a mut Ast<'a>>,
    pub structs: Vec<AstStructIRInfo<'a>>,
    pub enums: Vec<AstEnumIRInfo<'a>>,
    pub procs: Vec<AstProcIRInfo<'a>>,
    pub globals: Vec<AstGlobalIRInfo<'a>>,
}

pub struct AstStructIRInfo<'a> {
    pub struct_decl: &'a mut AstStructDecl<'a>,
    //todo
}

pub struct AstEnumIRInfo<'a> {
    pub enum_decl: &'a mut AstEnumDecl<'a>,
    //todo
}

pub struct AstProcIRInfo<'a> {
    pub proc_decl: &'a mut AstProcDecl<'a>,
    //todo
}

pub struct AstGlobalIRInfo<'a> {
    pub global_decl: &'a mut AstGlobalDecl<'a>,
    //todo
}

pub struct Ast<'a> {
    pub source: &'a [u8],
    pub filepath: String,
    pub imports: Vec<&'a mut AstImportDecl<'a>>,
    pub uses: Vec<&'a mut AstUseDecl<'a>>,
    pub structs: Vec<&'a mut AstStructDecl<'a>>,
    pub enums: Vec<&'a mut AstEnumDecl<'a>>,
    pub procs: Vec<&'a mut AstProcDecl<'a>>,
    pub globals: Vec<&'a mut AstGlobalDecl<'a>>,
    pub import_map: HashMap<AstIdent<'a>, &'a mut AstImportDecl<'a>>,
    pub struct_map: HashMap<AstIdent<'a>, AstStructInfo<'a>>,
    pub enum_map: HashMap<AstIdent<'a>, AstEnumInfo<'a>>,
    pub proc_map: HashMap<AstIdent<'a>, AstProcInfo<'a>>,
    pub global_map: HashMap<AstIdent<'a>, AstGlobalInfo<'a>>,
}

pub struct AstStructInfo<'a> {
    pub struct_id: u32,
    pub struct_decl: &'a mut AstStructDecl<'a>,
}

pub struct AstEnumInfo<'a> {
    pub enum_id: u32,
    pub enum_decl: &'a mut AstEnumDecl<'a>,
}

pub struct AstProcInfo<'a> {
    pub proc_id: u32,
    pub proc_decl: &'a mut AstProcDecl<'a>,
}

pub struct AstGlobalInfo<'a> {
    pub global_id: u32,
    pub global_decl: &'a mut AstGlobalDecl<'a>,
}

pub struct AstIdent<'a> {
    pub span: Span,
    pub str: &'a str,
}

pub struct AstType<'a> {
    pub span: Span,
    pub pointer_level: u32,
    pub union: AstTypeUnion<'a>,
}

pub enum AstTypeUnion<'a> {
    Basic(BasicType),
    Array(&'a mut AstArrayType<'a>),
    Custom(&'a mut AstCustomType<'a>),
    Struct(AstStructType<'a>),
    Enum(AstEnumType<'a>),
}

pub struct AstArrayType<'a> {
    pub element_type: AstType<'a>,
    pub const_expr: AstConstExpr<'a>,
}

pub struct AstCustomType<'a> {
    pub import: Option<AstIdent<'a>>,
    pub ident: AstIdent<'a>,
}

pub struct AstStructType<'a> {
    pub struct_id: u32,
    pub struct_decl: &'a mut AstStructDecl<'a>,
}

pub struct AstEnumType<'a> {
    pub enum_id: u32,
    pub enum_decl: &'a mut AstEnumDecl<'a>,
}

pub struct AstImportDecl<'a> {
    pub alias: AstIdent<'a>,
    pub filepath: AstLiteral,
    pub import_ast: &'a mut Ast<'a>,
}

pub struct AstUseDecl<'a> {
    pub alias: AstIdent<'a>,
    pub import: AstIdent<'a>,
    pub symbol: AstIdent<'a>,
}

pub struct AstStructDecl<'a> {
    pub ident: AstIdent<'a>,
    pub fields: Vec<AstStructField<'a>>,
}

pub struct AstStructField<'a> {
    pub ident: AstIdent<'a>,
    pub r#type: AstType<'a>,
    pub const_expr: Option<AstConstExpr<'a>>,
}

pub struct AstEnumDecl<'a> {
    pub ident: AstIdent<'a>,
    pub basic_type: BasicType,
    pub variants: Vec<AstEnumVariant<'a>>,
}

pub struct AstEnumVariant<'a> {
    pub ident: AstIdent<'a>,
    pub const_expr: AstConstExpr<'a>,
}

pub struct AstProcDecl<'a> {
    pub ident: AstIdent<'a>,
    pub input_params: Vec<AstProcParam<'a>>,
    pub return_type: Option<AstType<'a>>,
}

pub struct AstProcParam<'a> {
    pub ident: AstIdent<'a>,
    pub r#type: AstType<'a>,
}

pub struct AstGlobalDecl<'a> {
    pub ident: AstIdent<'a>,
    pub const_expr: AstConstExpr<'a>,
    pub r#type: Option<AstType<'a>>,
}

pub struct AstBlock<'a> {
    pub statements: Vec<&'a AstStatement<'a>>,
}

pub enum AstStatement<'a> {
    If(&'a mut AstIf<'a>),
    For(&'a mut AstFor<'a>),
    Block(&'a mut AstBlock<'a>),
    Defer(&'a mut AstDefer<'a>),
    Break(&'a mut AstBreak),
    Return(&'a mut AstReturn<'a>),
    Switch(&'a mut AstSwitch<'a>),
    Continue(&'a mut AstContinue),
    VarDecl(&'a mut AstVarDecl<'a>),
    VarAssign(&'a mut AstVarAssign<'a>),
    ProcCall(&'a mut AstProcCall<'a>),
}

pub struct AstIf<'a> {
    pub span: Span,
    pub condition_expr: AstExpr<'a>,
    pub block: &'a mut AstBlock<'a>,
    pub r#else: Option<AstElse<'a>>,
}

pub struct AstElse<'a> {
    pub span: Span,
    pub union: AstElseUnion<'a>,
}

pub enum AstElseUnion<'a> {
    If(&'a mut AstIf<'a>),
    Block(&'a mut AstBlock<'a>),
}

pub struct AstFor<'a> {
    pub span: Span,
    pub var_decl: Option<&'a mut AstVarDecl<'a>>,
    pub condition_expr: Option<&'a mut AstExpr<'a>>,
    pub var_assign: Option<&'a mut AstVarAssign<'a>>,
}

pub struct AstDefer<'a> {
    pub span: Span,
    pub block: &'a mut AstBlock<'a>,
}

pub struct AstBreak {
    pub span: Span,
}

pub struct AstReturn<'a> {
    pub span: Span,
    pub expr: Option<&'a mut AstExpr<'a>>,
}

pub struct AstSwitch<'a> {
    pub span: Span,
    pub expr: &'a mut AstExpr<'a>,
    pub cases: Vec<AstSwitchCase<'a>>,
}

pub struct AstSwitchCase<'a> {
    pub const_expr: AstConstExpr<'a>,
    pub block: Option<&'a mut AstBlock<'a>>,
}

pub struct AstContinue {
    pub span: Span,
}

pub struct AstVarDecl<'a> {
    pub span: Span,
    pub ident: AstIdent<'a>,
    pub r#type: Option<AstType<'a>>,
    pub expr: Option<&'a mut AstExpr<'a>>,
}

pub struct AstVarAssign<'a> {
    pub span: Span,
    pub var: &'a mut AstVar<'a>,
    pub op: AssignOp,
    pub expr: &'a mut AstExpr<'a>,
}

pub struct AstExpr<'a> {
    pub span: Span,
    pub is_const: bool,
    pub union: AstExprUnion<'a>,
}

pub enum AstExprUnion<'a> {
    Term(&'a mut AstTerm<'a>),
    UnaryExpr(&'a mut AstUnaryExpr<'a>),
    BinaryExpr(&'a mut AstBinaryExpr<'a>),
    FoldedExpr(AstFoldedExpr),
}

pub struct AstUnaryExpr<'a> {
    pub op: UnaryOp,
    pub right: &'a mut AstExpr<'a>,
}

pub struct AstBinaryExpr<'a> {
    pub op: BinaryOp,
    pub left: &'a mut AstExpr<'a>,
    pub right: &'a mut AstExpr<'a>,
}

pub struct AstFoldedExpr {
    pub basic_type: BasicType,
    pub literal: FoldedLiteral,
}

pub enum FoldedLiteral {
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
}

pub struct AstConstExpr<'a> {
    pub expr: AstExpr<'a>,
    pub eval: ConstEval,
}

pub enum ConstEval {
    NotEvaluated,
    Invalid,
    Validm,
}

pub enum AstTerm<'a> {
    Var(&'a mut AstVar<'a>),
    Enum(&'a mut AstEnum<'a>),
    Sizeof(&'a mut AstSizeof<'a>),
    Literal(&'a mut AstLiteral),
    ProcCall(&'a mut AstProcCall<'a>),
    ArrayInit(&'a mut AstArrayInit<'a>),
    StructInit(&'a mut AstStructInit<'a>),
}

pub struct AstVar<'a> {
    pub access: Option<&'a mut AstAccess<'a>>,
    pub union: AstVarUnion<'a>,
}

pub enum AstVarUnion<'a> {
    Unresolved(AstVarUnresolved<'a>),
    Local(AstVarLocal<'a>),
    Global(AstVarGlobal<'a>),
}

pub struct AstVarUnresolved<'a> {
    pub ident: AstIdent<'a>,
}

pub struct AstVarLocal<'a> {
    pub ident: AstIdent<'a>,
}

pub struct AstVarGlobal<'a> {
    pub global_id: u32,
    pub global_decl: &'a mut AstGlobalDecl<'a>,
}

pub enum AstAccess<'a> {
    VarAccess(AstVarAccess<'a>),
    ArrayAccess(AstArrayAccess<'a>),
}

pub struct AstVarAccess<'a> {
    pub ident: AstIdent<'a>,
    pub next: Option<&'a mut AstAccess<'a>>,
    pub field_id: u32,
}

pub struct AstArrayAccess<'a> {
    pub index_expr: &'a mut AstExpr<'a>,
    pub next: Option<&'a mut AstAccess<'a>>,
}

pub enum AstEnum<'a> {
    Unresolved(AstEnumUnresolved<'a>),
    Resolved(AstEnumResolved<'a>),
}

pub struct AstEnumUnresolved<'a> {
    pub import: Option<AstIdent<'a>>,
    pub ident: AstIdent<'a>,
    pub variant: AstIdent<'a>,
}

pub struct AstEnumResolved<'a> {
    pub r#type: AstEnumType<'a>,
    pub variant_id: u32,
}

pub struct AstSizeof<'a> {
    pub r#type: AstType<'a>,
}

pub struct AstLiteral {
    pub token: Token,
}

pub struct AstProcCall<'a> {
    pub span: Span,
    pub input_exprs: Vec<&'a mut AstExpr<'a>>,
    pub access: Option<&'a mut AstAccess<'a>>,
    pub union: AstProcCallUnion<'a>,
}

pub enum AstProcCallUnion<'a> {
    Unresolved(AstProcCallUnresolved<'a>),
    Resolved(AstProcCallResolved<'a>),
}

pub struct AstProcCallUnresolved<'a> {
    pub import: Option<AstIdent<'a>>,
    pub ident: AstIdent<'a>,
}

pub struct AstProcCallResolved<'a> {
    pub proc_id: u32,
    pub proc_decl: &'a mut AstProcDecl<'a>,
}

pub struct AstArrayInit<'a> {
    pub r#type: Option<AstType<'a>>,
    pub input_exprs: Vec<&'a mut AstExpr<'a>>,
}

pub struct AstStructInit<'a> {
    pub input_exprs: Vec<&'a mut AstExpr<'a>>,
    pub union: AstStructInitUnion<'a>,
}

pub enum AstStructInitUnion<'a> {
    Unresolved(AstStructInitUnresolved<'a>),
    Resolved(AstStructInitResolved<'a>),
}

pub struct AstStructInitUnresolved<'a> {
    pub import: Option<AstIdent<'a>>,
    pub ident: Option<AstIdent<'a>>,
}

pub struct AstStructInitResolved<'a> {
    pub r#type: Option<AstStructType<'a>>,
}

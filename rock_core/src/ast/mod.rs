use crate::intern::{LitID, NameID};
use crate::support::{Arena, AsStr};
use crate::text::{TextOffset, TextRange};

pub struct Ast<'ast> {
    pub arena: Arena<'ast>,
    pub items: &'ast [Item<'ast>],
}

#[derive(Copy, Clone)]
pub enum Item<'ast> {
    Proc(&'ast ProcItem<'ast>),
    Enum(&'ast EnumItem<'ast>),
    Struct(&'ast StructItem<'ast>),
    Const(&'ast ConstItem<'ast>),
    Global(&'ast GlobalItem<'ast>),
    Import(&'ast ImportItem<'ast>),
    Directive(&'ast DirectiveList<'ast>),
}

#[derive(Copy, Clone)]
pub struct ProcItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub poly_params: Option<&'ast PolymorphParams<'ast>>,
    pub params: &'ast [Param<'ast>],
    pub is_variadic: bool,
    pub return_ty: Type<'ast>,
    pub block: Option<Block<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Param<'ast> {
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct EnumItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub poly_params: Option<&'ast PolymorphParams<'ast>>,
    pub tag_ty: Option<&'ast EnumTagType>,
    pub variants: &'ast [Variant<'ast>],
}

#[derive(Copy, Clone)]
pub struct EnumTagType {
    pub basic: BasicType,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Variant<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub kind: VariantKind<'ast>,
}

#[derive(Copy, Clone)]
pub enum VariantKind<'ast> {
    Default,
    Constant(ConstExpr<'ast>),
    HasFields(&'ast VariantFieldList<'ast>),
}

#[derive(Copy, Clone)]
pub struct VariantFieldList<'ast> {
    pub types: &'ast [Type<'ast>],
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct StructItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub poly_params: Option<&'ast PolymorphParams<'ast>>,
    pub fields: &'ast [Field<'ast>],
}

#[derive(Copy, Clone)]
pub struct Field<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ConstItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub name: Name,
    pub ty: Option<Type<'ast>>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct GlobalItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
    pub init: GlobalInit<'ast>,
}

#[derive(Copy, Clone)]
pub enum GlobalInit<'ast> {
    Init(ConstExpr<'ast>),
    Zeroed,
}

#[derive(Copy, Clone)]
pub struct ImportItem<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub package: Option<Name>,
    pub import_path: &'ast [Name],
    pub rename: SymbolRename,
    pub symbols: &'ast [ImportSymbol],
}

#[derive(Copy, Clone)]
pub struct ImportSymbol {
    pub name: Name,
    pub rename: SymbolRename,
}

#[derive(Copy, Clone)]
pub enum SymbolRename {
    None,
    Alias(Name),
    Discard(TextRange),
}

//==================== DIRECTIVE ====================

#[derive(Copy, Clone)]
pub struct DirectiveList<'ast> {
    pub directives: &'ast [Directive<'ast>],
}

#[derive(Copy, Clone)]
pub struct Directive<'ast> {
    pub kind: DirectiveKind<'ast>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct DirectiveParam {
    pub name: Name,
    pub value: LitID,
    pub value_range: TextRange,
}

#[derive(Copy, Clone)]
pub enum DirectiveKind<'ast> {
    Error(Name),
    Inline,
    Builtin,
    ScopePublic,
    ScopePackage,
    ScopePrivate,
    CallerLocation,
    SizeOf(&'ast Type<'ast>),
    AlignOf(&'ast Type<'ast>),
    Config(&'ast [DirectiveParam]),
    ConfigAny(&'ast [DirectiveParam]),
    ConfigNot(&'ast [DirectiveParam]),
}

//==================== TYPE ====================

#[derive(Copy, Clone)]
pub struct Type<'ast> {
    pub kind: TypeKind<'ast>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub enum TypeKind<'ast> {
    Basic(BasicType),
    Custom(&'ast Path<'ast>),
    Reference(Mut, &'ast Type<'ast>),
    MultiReference(Mut, &'ast Type<'ast>),
    Procedure(&'ast ProcType<'ast>),
    ArraySlice(&'ast ArraySlice<'ast>),
    ArrayStatic(&'ast ArrayStatic<'ast>),
}

#[derive(Copy, Clone)]
pub struct ProcType<'ast> {
    pub param_types: &'ast [Type<'ast>],
    pub variadic: bool,
    pub return_ty: Type<'ast>,
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

//==================== STMT ====================

#[derive(Copy, Clone)]
pub struct Block<'ast> {
    pub stmts: &'ast [Stmt<'ast>],
    pub range: TextRange,
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
    For(&'ast For<'ast>),
    Local(&'ast Local<'ast>),
    Assign(&'ast Assign<'ast>),
    ExprSemi(&'ast Expr<'ast>),
    ExprTail(&'ast Expr<'ast>),
    WithDirective(&'ast StmtWithDirective<'ast>),
}

#[derive(Copy, Clone)]
pub struct For<'ast> {
    pub header: ForHeader<'ast>,
    pub block: Block<'ast>,
}

#[derive(Copy, Clone)]
pub enum ForHeader<'ast> {
    Loop,
    Cond(&'ast Expr<'ast>),
    Elem(&'ast ForHeaderElem<'ast>),
    Range(&'ast ForHeaderRange<'ast>),
    Pat(&'ast ForHeaderPat<'ast>),
}

#[derive(Copy, Clone)]
pub struct ForHeaderElem<'ast> {
    pub ref_mut: Option<Mut>,
    pub value: Option<Name>,
    pub index: Option<Name>,
    pub reverse: bool,
    pub expr: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ForHeaderRange<'ast> {
    pub ref_start: Option<TextOffset>,
    pub value: Option<Name>,
    pub index: Option<Name>,
    pub reverse_start: Option<TextOffset>,
    pub start: &'ast Expr<'ast>,
    pub end: &'ast Expr<'ast>,
    pub kind: RangeKind,
}

#[derive(Copy, Clone)]
pub struct ForHeaderPat<'ast> {
    pub pat: Pat<'ast>,
    pub expr: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct Local<'ast> {
    pub bind: Binding,
    pub ty: Option<Type<'ast>>,
    pub init: LocalInit<'ast>,
}

#[derive(Copy, Clone)]
pub enum LocalInit<'ast> {
    Init(&'ast Expr<'ast>),
    Zeroed(TextRange),
    Undefined(TextRange),
}

#[derive(Copy, Clone)]
pub struct Assign<'ast> {
    pub op: AssignOp,
    pub op_range: TextRange,
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct StmtWithDirective<'ast> {
    pub dir_list: Option<&'ast DirectiveList<'ast>>,
    pub stmt: Stmt<'ast>,
}

//==================== EXPR ====================

#[derive(Copy, Clone)]
pub struct ConstExpr<'ast>(pub &'ast Expr<'ast>);

#[derive(Copy, Clone)]
pub struct Expr<'ast> {
    pub kind: ExprKind<'ast>,
    pub range: TextRange,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ExprKind<'ast> {
    Lit         { lit: Lit },
    If          { if_: &'ast If<'ast> },
    Block       { block: &'ast Block<'ast> },
    Match       { match_: &'ast Match<'ast> },
    Field       { target: &'ast Expr<'ast>, name: Name },
    Index       { target: &'ast Expr<'ast>, index: &'ast Expr<'ast> },
    Slice       { target: &'ast Expr<'ast>, range: &'ast SliceRange<'ast> },
    Call        { target: &'ast Expr<'ast>, args_list: &'ast ArgumentList<'ast> },
    Cast        { target: &'ast Expr<'ast>, into: &'ast Type<'ast> },
    Sizeof      { ty: &'ast Type<'ast> },
    Directive   { directive: &'ast Directive<'ast> },
    Item        { path: &'ast Path<'ast>, args_list: Option<&'ast ArgumentList<'ast>> },
    Variant     { name: Name, args_list: Option<&'ast ArgumentList<'ast>> },
    StructInit  { struct_init: &'ast StructInit<'ast> },
    ArrayInit   { input: &'ast [&'ast Expr<'ast>] },
    ArrayRepeat { value: &'ast Expr<'ast>, len: ConstExpr<'ast> },
    Deref       { rhs: &'ast Expr<'ast> },
    Address     { mutt: Mut, rhs: &'ast Expr<'ast> },
    Unary       { op: UnOp, op_range: TextRange, rhs: &'ast Expr<'ast> },
    Binary      { op: BinOp, op_start: TextOffset, lhs: &'ast Expr<'ast>, rhs: &'ast Expr<'ast> },
}

#[derive(Copy, Clone)]
pub struct If<'ast> {
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
    pub pat: Pat<'ast>,
    pub expr: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct SliceRange<'ast> {
    pub start: Option<&'ast Expr<'ast>>,
    pub end: Option<(RangeKind, &'ast Expr<'ast>)>,
}

#[derive(Copy, Clone)]
pub enum RangeKind {
    Exclusive,
    Inclusive,
}

#[derive(Copy, Clone)]
pub struct StructInit<'ast> {
    pub path: Option<&'ast Path<'ast>>,
    pub input: &'ast [FieldInit<'ast>],
    pub input_start: TextOffset,
}

#[derive(Copy, Clone)]
pub struct FieldInit<'ast> {
    pub name: Name,
    pub expr: &'ast Expr<'ast>,
}

//==================== PAT ====================

#[derive(Copy, Clone)]
pub struct Pat<'ast> {
    pub kind: PatKind<'ast>,
    pub range: TextRange,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum PatKind<'ast> {
    Wild,
    Lit       { expr: &'ast Expr<'ast> },
    Item      { path: &'ast Path<'ast>, bind_list: Option<&'ast BindingList<'ast>> },
    Variant   { name: Name, bind_list: Option<&'ast BindingList<'ast>> },
    Or        { pats: &'ast [Pat<'ast>] },
}

#[derive(Copy, Clone)]
pub enum Lit {
    Void,
    Null,
    Bool(bool),
    Int(u64),
    Float(f64),
    Char(char),
    String(LitID),
}

//==================== COMMON ====================

#[derive(Copy, Clone)]
pub struct Name {
    pub id: NameID,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub enum Binding {
    Named(Mut, Name),
    Discard(TextRange),
}

#[derive(Copy, Clone)]
pub struct BindingList<'ast> {
    pub binds: &'ast [Binding],
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct ArgumentList<'ast> {
    pub exprs: &'ast [&'ast Expr<'ast>],
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Path<'ast> {
    pub segments: &'ast [PathSegment<'ast>],
}

#[derive(Copy, Clone)]
pub struct PathSegment<'ast> {
    pub name: Name,
    pub poly_args: Option<&'ast PolymorphArgs<'ast>>,
}

#[derive(Copy, Clone)]
pub struct PolymorphArgs<'ast> {
    pub types: &'ast [Type<'ast>],
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct PolymorphParams<'ast> {
    pub names: &'ast [Name],
    pub range: TextRange,
}

//==================== ENUMS ====================

#[derive(Copy, Clone, PartialEq)]
pub enum Mut {
    Mutable,
    Immutable,
}

//@reorder same as hir::Type?
crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum BasicType {
        S8 "s8",
        S16 "s16",
        S32 "s32",
        S64 "s64",
        Ssize "ssize",
        U8 "u8",
        U16 "u16",
        U32 "u32",
        U64 "u64",
        Usize "usize",
        F32 "f32",
        F64 "f64",
        Bool "bool",
        Bool32 "bool32",
        Char "char",
        Rawptr "rawptr",
        Any "any",
        Void "void",
        Never "never",
        String "string",
        CString "cstring",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum UnOp {
        Neg "-",
        BitNot "~",
        LogicNot "!",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum BinOp {
        Add "+",
        Sub "-",
        Mul "*",
        Div "/",
        Rem "%",
        BitAnd "&",
        BitOr "|",
        BitXor "^",
        BitShl "<<",
        BitShr ">>",
        Eq "==",
        NotEq "!=",
        Less "<",
        LessEq "<=",
        Greater ">",
        GreaterEq ">=",
        LogicAnd "&&",
        LogicOr "||",
    }
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Bin(BinOp),
}

//==================== SIZE LOCK ====================

crate::size_lock!(16, Item);
crate::size_lock!(96, ProcItem);
crate::size_lock!(56, EnumItem);
crate::size_lock!(48, StructItem);
crate::size_lock!(56, ConstItem);
crate::size_lock!(56, GlobalItem);
crate::size_lock!(72, ImportItem);

crate::size_lock!(40, Param);
crate::size_lock!(40, Variant);
crate::size_lock!(48, Field);
crate::size_lock!(28, ImportSymbol);

crate::size_lock!(24, Type);
crate::size_lock!(24, Block);
crate::size_lock!(24, Stmt);
crate::size_lock!(32, Expr);
crate::size_lock!(32, Pat);

crate::size_lock!(12, Name);
crate::size_lock!(16, Path);
crate::size_lock!(16, Binding);

//==================== AST IMPL ====================

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicOr => 1,
            BinOp::LogicAnd => 2,
            BinOp::Eq
            | BinOp::NotEq
            | BinOp::Less
            | BinOp::LessEq
            | BinOp::Greater
            | BinOp::GreaterEq => 3,
            BinOp::Add | BinOp::Sub => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::BitShl | BinOp::BitShr => 6,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 7,
        }
    }
}

impl<'ast> DirectiveKind<'ast> {
    pub fn as_str(&self) -> &'static str {
        match self {
            DirectiveKind::Error(_) => "<error>",
            DirectiveKind::Inline => "inline",
            DirectiveKind::Builtin => "builtin",
            DirectiveKind::ScopePublic => "scope_public",
            DirectiveKind::ScopePackage => "scope_package",
            DirectiveKind::ScopePrivate => "scope_private",
            DirectiveKind::CallerLocation => "caller_location",
            DirectiveKind::SizeOf(_) => "size_of",
            DirectiveKind::AlignOf(_) => "align_of",
            DirectiveKind::Config(_) => "config",
            DirectiveKind::ConfigAny(_) => "config_any",
            DirectiveKind::ConfigNot(_) => "config_not",
        }
    }
}

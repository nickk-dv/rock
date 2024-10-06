use crate::intern::{InternLit, InternName};
use crate::support::AsStr;
use crate::support::{Arena, ID};
use crate::text::TextRange;

//@breaking change: removed cstring lit state, cstring lit
// state no longer coresponds with intern pool string lit order
// will always generate zero terminated strings (minor memory waste)
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
}

#[derive(Copy, Clone)]
pub struct Attr<'ast> {
    pub name: Name,
    pub params: Option<(&'ast [AttrParam], TextRange)>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct AttrParam {
    pub name: Name,
    pub value: Option<(ID<InternLit>, TextRange)>, //@problem uses InternLit!
}

#[derive(Copy, Clone)]
pub struct ProcItem<'ast> {
    pub attrs: &'ast [Attr<'ast>],
    pub vis: Vis,
    pub name: Name,
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
    pub attrs: &'ast [Attr<'ast>],
    pub vis: Vis,
    pub name: Name,
    pub variants: &'ast [Variant<'ast>],
}

#[derive(Copy, Clone)]
pub struct Variant<'ast> {
    pub name: Name,
    pub kind: VariantKind<'ast>,
}

#[derive(Copy, Clone)]
pub enum VariantKind<'ast> {
    Default,
    Constant(ConstExpr<'ast>),
    HasValues(&'ast [Type<'ast>]),
}

#[derive(Copy, Clone)]
pub struct StructItem<'ast> {
    pub attrs: &'ast [Attr<'ast>],
    pub vis: Vis,
    pub name: Name,
    pub fields: &'ast [Field<'ast>],
}

#[derive(Copy, Clone)]
pub struct Field<'ast> {
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct ConstItem<'ast> {
    pub attrs: &'ast [Attr<'ast>],
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct GlobalItem<'ast> {
    pub attrs: &'ast [Attr<'ast>],
    pub vis: Vis,
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ImportItem<'ast> {
    pub attrs: &'ast [Attr<'ast>],
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
    Reference(&'ast Type<'ast>, Mut),
    Procedure(&'ast ProcType<'ast>),
    ArraySlice(&'ast ArraySlice<'ast>),
    ArrayStatic(&'ast ArrayStatic<'ast>),
}

#[derive(Copy, Clone)]
pub struct ProcType<'ast> {
    pub param_types: &'ast [Type<'ast>],
    pub is_variadic: bool,
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
    Loop(&'ast Loop<'ast>),
    Local(&'ast Local<'ast>),
    Assign(&'ast Assign<'ast>),
    ExprSemi(&'ast Expr<'ast>),
    ExprTail(&'ast Expr<'ast>),
}

#[derive(Copy, Clone)]
pub struct Loop<'ast> {
    pub kind: LoopKind<'ast>,
    pub block: Block<'ast>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum LoopKind<'ast> {
    Loop,
    While { cond: &'ast Expr<'ast> },
    ForLoop {
        local: &'ast Local<'ast>,
        cond: &'ast Expr<'ast>,
        assign: &'ast Assign<'ast>,
    },
}

#[derive(Copy, Clone)]
pub struct Local<'ast> {
    pub bind: Binding,
    pub ty: Option<Type<'ast>>,
    pub init: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct Assign<'ast> {
    pub op: AssignOp,
    pub op_range: TextRange,
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
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
    Slice       { target: &'ast Expr<'ast>, mutt: Mut, range: &'ast Expr<'ast> },
    Call        { target: &'ast Expr<'ast>, args_list: &'ast ArgumentList<'ast> },
    Cast        { target: &'ast Expr<'ast>, into: &'ast Type<'ast> },
    Sizeof      { ty: &'ast Type<'ast> },
    Item        { path: &'ast Path<'ast>, args_list: Option<&'ast ArgumentList<'ast>> },
    Variant     { name: Name, args_list: Option<&'ast ArgumentList<'ast>> },
    StructInit  { struct_init: &'ast StructInit<'ast> },
    ArrayInit   { input: &'ast [&'ast Expr<'ast>] },
    ArrayRepeat { expr: &'ast Expr<'ast>, len: ConstExpr<'ast> },
    Deref       { rhs: &'ast Expr<'ast> },
    Address     { mutt: Mut, rhs: &'ast Expr<'ast> },
    Range       { range: &'ast Range<'ast> },
    Unary       { op: UnOp, op_range: TextRange, rhs: &'ast Expr<'ast> },
    Binary      { op: BinOp, op_range: TextRange, bin: &'ast BinExpr<'ast> },
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
    pub pat: Pat<'ast>,
    pub expr: &'ast Expr<'ast>,
}

//@move path to expr itself + &field init list
#[derive(Copy, Clone)]
pub struct StructInit<'ast> {
    pub path: Option<&'ast Path<'ast>>,
    pub input: &'ast [FieldInit<'ast>],
}

#[derive(Copy, Clone)]
pub struct FieldInit<'ast> {
    pub name: Name,
    pub expr: &'ast Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct BinExpr<'ast> {
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
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
    Lit       { lit: Lit },
    Item      { path: &'ast Path<'ast>, bind_list: Option<&'ast BindingList<'ast>> },
    Variant   { name: Name, bind_list: Option<&'ast BindingList<'ast>> },
    Or        { patterns: &'ast [Pat<'ast>] },
}

#[derive(Copy, Clone)]
pub enum Lit {
    Null,
    Bool(bool),
    Int(u64),
    Float(f64),
    Char(char),
    String(StringLit),
}

#[derive(Copy, Clone, PartialEq, Hash)]
pub struct StringLit {
    pub id: ID<InternLit>,
    pub c_string: bool,
}

#[derive(Copy, Clone)]
pub enum Range<'ast> {
    Full,                                          // ..
    ToExclusive(&'ast Expr<'ast>),                 // ..<2
    ToInclusive(&'ast Expr<'ast>),                 // ..=2
    From(&'ast Expr<'ast>),                        // 0..
    Exclusive(&'ast Expr<'ast>, &'ast Expr<'ast>), // 0..<2
    Inclusive(&'ast Expr<'ast>, &'ast Expr<'ast>), // 0..=2
}

//==================== COMMON ====================

#[derive(Copy, Clone)]
pub struct Name {
    pub id: ID<InternName>,
    pub range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Path<'ast> {
    pub names: &'ast [Name],
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

//==================== ENUMS ====================

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
        Char "char",
        Rawptr "rawptr",
        Void "void",
        Never "never",
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
        IsEq "==",
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

//@work in progress, some relations might be non-obvious
impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicOr => 1,
            BinOp::LogicAnd => 2,
            BinOp::IsEq
            | BinOp::NotEq
            | BinOp::Less
            | BinOp::LessEq
            | BinOp::Greater
            | BinOp::GreaterEq => 3,
            BinOp::Add | BinOp::Sub => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitShl | BinOp::BitShr => 6,
        }
    }
}

//==================== SIZE LOCK ====================

crate::size_lock!(16, Item);
crate::size_lock!(48, Attr);
crate::size_lock!(28, AttrParam);

crate::size_lock!(96, ProcItem);
crate::size_lock!(48, EnumItem);
crate::size_lock!(48, StructItem);
crate::size_lock!(64, ConstItem);
crate::size_lock!(64, GlobalItem);
crate::size_lock!(80, ImportItem);

crate::size_lock!(24, Type);
crate::size_lock!(24, Block);
crate::size_lock!(24, Stmt);
crate::size_lock!(32, Expr);
crate::size_lock!(32, Pat);

crate::size_lock!(12, Name);
crate::size_lock!(16, Path);
crate::size_lock!(16, Binding);

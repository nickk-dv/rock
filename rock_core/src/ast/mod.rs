use crate::arena::Arena;
use crate::intern::{InternID, InternPool};
use crate::text::TextRange;

pub struct Ast<'ast, 'intern> {
    pub arena: Arena<'ast>,
    pub intern_name: InternPool<'intern>,
    pub intern_string: InternPool<'intern>,
    pub string_is_cstr: Vec<bool>,
    pub modules: Vec<Module<'ast>>,
}

#[derive(Copy, Clone)]
pub struct Module<'ast> {
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

#[derive(Default)]
pub struct ItemCount {
    pub procs: u32,
    pub enums: u32,
    pub structs: u32,
    pub consts: u32,
    pub globals: u32,
}

#[derive(Copy, Clone)]
pub struct ProcItem<'ast> {
    pub attrs: &'ast [Attribute],
    pub vis: Vis,
    pub name: Name,
    pub params: &'ast [ProcParam<'ast>],
    pub is_variadic: bool,
    pub return_ty: Option<Type<'ast>>,
    pub block: Option<Block<'ast>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam<'ast> {
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
}

#[derive(Copy, Clone)]
pub struct EnumItem<'ast> {
    pub attrs: &'ast [Attribute],
    pub vis: Vis,
    pub name: Name,
    pub basic: Option<(BasicType, TextRange)>,
    pub variants: &'ast [EnumVariant<'ast>],
}

#[derive(Copy, Clone)]
pub struct EnumVariant<'ast> {
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
    pub attrs: &'ast [Attribute],
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
    pub attrs: &'ast [Attribute],
    pub vis: Vis,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct GlobalItem<'ast> {
    pub attrs: &'ast [Attribute],
    pub vis: Vis,
    pub mutt: Mut,
    pub name: Name,
    pub ty: Type<'ast>,
    pub value: ConstExpr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ImportItem<'ast> {
    pub attrs: &'ast [Attribute],
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
pub struct Attribute {
    pub kind: AttributeKind,
    pub range: TextRange,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq)]
pub enum AttributeKind {
    Test,
    Builtin,
    Inline,
    Thread_Local,
    Unknown,
}

#[derive(Copy, Clone)]
pub struct Path<'ast> {
    pub names: &'ast [Name],
}

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
    pub return_ty: Option<Type<'ast>>,
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
    pub mutt: Mut,
    pub name: Name,
    pub kind: LocalKind<'ast>,
}

#[derive(Copy, Clone)]
pub enum LocalKind<'ast> {
    Decl(Type<'ast>),
    Init(Option<Type<'ast>>, &'ast Expr<'ast>),
}

#[derive(Copy, Clone)]
pub struct Assign<'ast> {
    pub op: AssignOp,
    pub op_range: TextRange,
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
}

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
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64 },
    LitFloat    { val: f64 },
    LitChar     { val: char },
    LitString   { id: InternID, c_string: bool },
    If          { if_: &'ast If<'ast> },
    Block       { block: &'ast Block<'ast> },
    Match       { match_: &'ast Match<'ast> },
    Field       { target: &'ast Expr<'ast>, name: Name },
    Index       { target: &'ast Expr<'ast>, mutt: Mut, index: &'ast Expr<'ast> },
    Call        { target: &'ast Expr<'ast>, input: &'ast &'ast [&'ast Expr<'ast>] },
    Cast        { target: &'ast Expr<'ast>, into: &'ast Type<'ast> },
    Sizeof      { ty: &'ast Type<'ast> },
    Item        { path: &'ast Path<'ast> },
    Variant     { name: Name },
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
    pub fallback: Option<&'ast Expr<'ast>>,
    pub fallback_range: TextRange,
}

#[derive(Copy, Clone)]
pub struct MatchArm<'ast> {
    pub pat: ConstExpr<'ast>,
    pub expr: &'ast Expr<'ast>,
}

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
pub enum Range<'ast> {
    Full,                                               // ..
    RangeTo(&'ast Expr<'ast>),                          // ..<2
    RangeToInclusive(&'ast Expr<'ast>),                 // ..=2
    RangeFrom(&'ast Expr<'ast>),                        // 0..
    Range(&'ast Expr<'ast>, &'ast Expr<'ast>),          // 0..<2
    RangeInclusive(&'ast Expr<'ast>, &'ast Expr<'ast>), // 0..=2
}

#[derive(Copy, Clone)]
pub struct BinExpr<'ast> {
    pub lhs: &'ast Expr<'ast>,
    pub rhs: &'ast Expr<'ast>,
}

#[derive(Copy, Clone, PartialEq, Hash)]
pub enum BasicType {
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
    Bool,
    Char,
    Rawptr,
    Void,
    Never,
}

#[derive(Copy, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    BitNot,
    LogicNot,
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
    IsEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    LogicAnd,
    LogicOr,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Bin(BinOp),
}

use crate::size_assert;
size_assert!(16, Item);
size_assert!(12, Name);
size_assert!(16, Path);
size_assert!(24, Type);
size_assert!(24, Stmt);
size_assert!(32, Expr);

impl AttributeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AttributeKind::Test => "test",
            AttributeKind::Builtin => "builtin",
            AttributeKind::Inline => "inline",
            AttributeKind::Thread_Local => "thread_local",
            AttributeKind::Unknown => "unknown",
        }
    }

    pub fn from_str(string: &str) -> AttributeKind {
        match string {
            "test" => AttributeKind::Test,
            "builtin" => AttributeKind::Builtin,
            "inline" => AttributeKind::Inline,
            "thread_local" => AttributeKind::Thread_Local,
            _ => AttributeKind::Unknown,
        }
    }
}

impl BasicType {
    pub fn as_str(self) -> &'static str {
        match self {
            BasicType::S8 => "s8",
            BasicType::S16 => "s16",
            BasicType::S32 => "s32",
            BasicType::S64 => "s64",
            BasicType::Ssize => "ssize",
            BasicType::U8 => "u8",
            BasicType::U16 => "u16",
            BasicType::U32 => "u32",
            BasicType::U64 => "u64",
            BasicType::Usize => "usize",
            BasicType::F32 => "f32",
            BasicType::F64 => "f64",
            BasicType::Bool => "bool",
            BasicType::Char => "char",
            BasicType::Rawptr => "rawptr",
            BasicType::Void => "void",
            BasicType::Never => "never",
        }
    }
}

impl UnOp {
    pub fn as_str(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::BitNot => "~",
            UnOp::LogicNot => "!",
        }
    }
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::BitShl => "<<",
            BinOp::BitShr => ">>",
            BinOp::IsEq => "==",
            BinOp::NotEq => "!=",
            BinOp::Less => "<",
            BinOp::LessEq => "<=",
            BinOp::Greater => ">",
            BinOp::GreaterEq => ">=",
            BinOp::LogicAnd => "&&",
            BinOp::LogicOr => "||",
        }
    }
}

use super::intern::*;
use super::span::Span;
use super::FileID;
use crate::check::{EnumID, StructID, UnionID};
use crate::mem::*;

pub struct Ast {
    pub arena: Arena,
    pub modules: Vec<P<Module>>,
}

#[derive(Copy, Clone)]
pub struct Module {
    pub file_id: FileID,
    pub decls: List<Decl>,
}

#[derive(Copy, Clone)]
pub enum Decl {
    Mod(P<ModDecl>),
    Use(P<UseDecl>),
    Proc(P<ProcDecl>),
    Enum(P<EnumDecl>),
    Union(P<UnionDecl>),
    Struct(P<StructDecl>),
    Const(P<ConstDecl>),
    Global(P<GlobalDecl>),
}

#[derive(Copy, Clone)]
pub struct ModDecl {
    pub vis: Vis,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct UseDecl {
    pub path: P<Path>,
    pub symbols: List<UseSymbol>,
}

#[derive(Copy, Clone)]
pub struct UseSymbol {
    pub name: Ident,
    pub alias: Option<Ident>,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub vis: Vis,
    pub name: Ident,
    pub params: List<ProcParam>,
    pub is_variadic: bool,
    pub return_ty: Option<Type>,
    pub block: Option<P<Expr>>,
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
pub struct ConstDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: ConstExpr,
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: ConstExpr,
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
    Custom(P<Path>),
    ArraySlice(P<ArraySlice>),
    ArrayStatic(P<ArrayStatic>),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Poison,
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
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum StmtKind {
    Break,
    Continue,
    Return(Option<P<Expr>>),
    Defer(P<Expr>),
    ForLoop(P<For>),
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    ExprSemi(P<Expr>),
    ExprTail(P<Expr>),
}

#[derive(Copy, Clone)]
pub struct For {
    pub kind: ForKind,
    pub block: P<Expr>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ForKind {
    Loop,
    While { cond: P<Expr> },
    ForLoop { var_decl: P<VarDecl>, cond: P<Expr>, var_assign: P<VarAssign> },
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
    pub op: AssignOp,
    pub lhs: P<Expr>,
    pub rhs: P<Expr>,
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
    LitInt      { val: u64, ty: Option<BasicType> },
    LitFloat    { val: f64, ty: Option<BasicType> },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: P<If> },
    Block       { stmts: List<Stmt> },
    Match       { on_expr: P<Expr>, arms: List<MatchArm> },
    Field       { target: P<Expr>, name: Ident },
    Index       { target: P<Expr>, index: P<Expr> },
    Cast        { target: P<Expr>, ty: P<Type> },
    Sizeof      { ty: P<Type> },
    Item        { path: P<Path> },
    ProcCall    { path: P<Path>, input: List<P<Expr>> },
    StructInit  { path: P<Path>, input: List<FieldInit> },
    ArrayInit   { input: List<P<Expr>> },
    ArrayRepeat { expr: P<Expr>, size: ConstExpr },
    UnaryExpr   { op: UnOp, rhs: P<Expr> },
    BinaryExpr  { op: BinOp, lhs: P<Expr>, rhs: P<Expr> },
}

#[derive(Copy, Clone)]
pub struct If {
    pub cond: P<Expr>,
    pub block: P<Expr>,
    pub else_: Option<Else>,
}

#[derive(Copy, Clone)]
pub enum Else {
    If { else_if: P<If> },
    Block { block: P<Expr> },
}

#[derive(Copy, Clone)]
pub struct MatchArm {
    pub pat: P<Expr>,
    pub expr: P<Expr>,
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
    Rawptr,
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

impl Type {
    pub fn new(kind: TypeKind) -> Self {
        Self {
            ptr: PtrLevel::new(),
            kind,
        }
    }

    pub fn new_ptr(mutt: Mut, kind: TypeKind) -> Self {
        let mut ptr = PtrLevel::new();
        let _ = ptr.add_level(mutt);
        Self { ptr, kind }
    }

    pub fn unit() -> Self {
        Self::new(TypeKind::Basic(BasicType::Unit))
    }

    pub fn basic(basic: BasicType) -> Self {
        Self::new(TypeKind::Basic(basic))
    }

    pub fn poison() -> Self {
        Self::new(TypeKind::Poison)
    }

    pub fn matches(ty: &Type, ty2: &Type) -> bool {
        if ty.ptr.level != ty2.ptr.level {
            return false;
        }
        if ty.ptr.mut_mask != ty2.ptr.mut_mask {
            return false;
        }
        match (ty.kind, ty2.kind) {
            (TypeKind::Basic(basic), TypeKind::Basic(basic2)) => basic == basic2,
            (TypeKind::Custom(_), TypeKind::Custom(_)) => panic!("custom type must be resolved"),
            (TypeKind::ArraySlice(slice), TypeKind::ArraySlice(slice2)) => {
                slice.mutt == slice2.mutt && Self::matches(&slice.ty, &slice2.ty)
            }
            (TypeKind::ArrayStatic(array), TypeKind::ArrayStatic(array2)) => {
                //@size ConstExpr is ignored
                Self::matches(&array.ty, &array2.ty)
            }
            (TypeKind::Enum(id), TypeKind::Enum(id2)) => id == id2,
            (TypeKind::Union(id), TypeKind::Union(id2)) => id == id2,
            (TypeKind::Struct(id), TypeKind::Struct(id2)) => id == id2,
            (TypeKind::Poison, ..) => true,
            (.., TypeKind::Poison) => true,
            _ => false,
        }
    }
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
    size_assert!(16, Path);
    size_assert!(24, Type);
    size_assert!(16, Decl);
    size_assert!(24, Stmt);
    size_assert!(32, Expr);
}

use super::hir_build::{HirData, HirEmit, SymbolKind};
use super::proc_scope::{ProcScope, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct ConstID(u32);

#[rustfmt::skip]
enum ConstValue<'hir> {
    Bool   { val: bool },
    Int    { val: u64, neg: bool },
    Float  { val: f64 },
    Char   { val: char },
    Struct { struct_: &'hir ConstStruct<'hir> },
    Array  { array: &'hir ConstArray<'hir> },
}

struct ConstArray<'hir> {
    len: u64,
    values: &'hir [ConstID],
}

struct ConstStruct<'hir> {
    struct_id: hir::StructID,
    fields: &'hir [ConstID],
}

pub fn run<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.proc_ids() {
        typecheck_proc(hir, emit, id)
    }
}

fn typecheck_proc<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.proc_ast(id);

    match item.block {
        Some(block) => {
            let data = hir.proc_data(id);
            let mut proc = ProcScope::new(data);

            let block_res = typecheck_expr(hir, emit, &mut proc, data.return_ty, block);

            let locals = proc.finish();
            let locals = emit.arena.alloc_slice(&locals);

            let data = hir.proc_data_mut(id);
            data.block = Some(block_res.expr);
            data.body.locals = locals;
        }
        None => {
            let data = hir.proc_data(id);
            //@for now having a tail directive assumes that it must be a #[c_call]
            // and this directory is only present with no block expression
            let directive_tail = item
                .directive_tail
                .expect("directive expected with no proc block");

            let directive_name = hir.name_str(directive_tail.name.id);
            if directive_name != "c_call" {
                emit.error(
                    ErrorComp::error(format!(
                        "expected a `c_call` directive, got `{}`",
                        directive_name
                    ))
                    .context(hir.src(data.origin_id, directive_tail.name.range)),
                )
            }
        }
    }
}

fn type_matches<'hir>(ty: hir::Type<'hir>, ty2: hir::Type<'hir>) -> bool {
    match (ty, ty2) {
        (hir::Type::Error, ..) => true,
        (.., hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (hir::Type::Enum(id), hir::Type::Enum(id2)) => id == id2,
        (hir::Type::Union(id), hir::Type::Union(id2)) => id == id2,
        (hir::Type::Struct(id), hir::Type::Struct(id2)) => id == id2,
        (hir::Type::Reference(ref_ty, mutt), hir::Type::Reference(ref_ty2, mutt2)) => {
            mutt == mutt2 && type_matches(*ref_ty, *ref_ty2)
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            slice.mutt == slice2.mutt && type_matches(slice.ty, slice2.ty)
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            //@size const_expr is ignored
            type_matches(array.ty, array2.ty)
        }
        _ => false,
    }
}

fn type_format<'hir>(hir: &HirData<'hir, '_, '_>, ty: hir::Type<'hir>) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
            BasicType::Unit => "()".into(),
            BasicType::Bool => "bool".into(),
            BasicType::S8 => "s8".into(),
            BasicType::S16 => "s16".into(),
            BasicType::S32 => "s32".into(),
            BasicType::S64 => "s64".into(),
            BasicType::Ssize => "ssize".into(),
            BasicType::U8 => "u8".into(),
            BasicType::U16 => "u16".into(),
            BasicType::U32 => "u32".into(),
            BasicType::U64 => "u64".into(),
            BasicType::Usize => "usize".into(),
            BasicType::F32 => "f32".into(),
            BasicType::F64 => "f64".into(),
            BasicType::Char => "char".into(),
            BasicType::Rawptr => "rawptr".into(),
        },
        hir::Type::Enum(id) => hir.name_str(hir.enum_data(id).name.id).into(),
        hir::Type::Union(id) => hir.name_str(hir.union_data(id).name.id).into(),
        hir::Type::Struct(id) => hir.name_str(hir.struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hir, *ref_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hir, slice.ty))
        }
        hir::Type::ArrayStatic(array) => format!("[<SIZE>]{}", type_format(hir, array.ty)),
    }
}

struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    expr: &'hir hir::Expr<'hir>,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult { ty, expr }
    }
}

//@need type_repr instead of allocating hir types
// and maybe type::unknown, or type::infer type to facilitate better inference
// to better represent partially typed arrays, etc
#[must_use]
fn typecheck_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    expr: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let expr_res = match expr.kind {
        ast::ExprKind::Unit => typecheck_unit(emit),
        ast::ExprKind::LitNull => typecheck_lit_null(emit),
        ast::ExprKind::LitBool { val } => typecheck_lit_bool(emit, val),
        ast::ExprKind::LitInt { val } => typecheck_lit_int(emit, expect, val),
        ast::ExprKind::LitFloat { val } => typecheck_lit_float(emit, expect, val),
        ast::ExprKind::LitChar { val } => typecheck_lit_char(emit, val),
        ast::ExprKind::LitString { id, c_string } => typecheck_lit_string(emit, id, c_string),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_),
        ast::ExprKind::Block { stmts } => typecheck_block(hir, emit, proc, expect, stmts),
        ast::ExprKind::Match { match_ } => typecheck_match(hir, emit, proc, expect, match_),
        ast::ExprKind::Field { target, name } => typecheck_field(hir, emit, proc, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(hir, emit, proc, target, index),
        ast::ExprKind::Cast { target, into } => {
            typecheck_cast(hir, emit, proc, target, into, expr.range)
        }
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, expr.range.start(), ty),
        ast::ExprKind::Item { path } => typecheck_item(hir, emit, proc, path),
        ast::ExprKind::ProcCall { proc_call } => {
            typecheck_proc_call(hir, emit, proc, proc_call, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(hir, emit, proc, expect, input),
        ast::ExprKind::ArrayRepeat { expr, size } => {
            typecheck_array_repeat(hir, emit, proc, expect, expr, size)
        }
        ast::ExprKind::Unary { op, rhs } => typecheck_unary(hir, emit, proc, op, rhs),
        ast::ExprKind::Binary { op, lhs, rhs } => typecheck_binary(hir, emit, proc, op, lhs, rhs),
    };

    if !type_matches(expect, expr_res.ty) {
        let msg: String = format!(
            "type mismatch: expected `{}`, found `{}`",
            type_format(hir, expect),
            type_format(hir, expr_res.ty)
        );
        emit.error(ErrorComp::error(msg).context(hir.src(proc.origin(), expr.range)));
    }

    expr_res
}

fn typecheck_unit<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Unit),
        emit.arena.alloc(hir::Expr::Unit),
    )
}

fn typecheck_lit_null<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Rawptr),
        emit.arena.alloc(hir::Expr::LitNull),
    )
}

fn typecheck_lit_bool<'hir>(emit: &mut HirEmit<'hir>, val: bool) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Bool),
        emit.arena.alloc(hir::Expr::LitBool { val }),
    )
}

fn typecheck_lit_int<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: hir::Type<'hir>,
    val: u64,
) -> TypeResult<'hir> {
    const DEFAULT_INT_TYPE: BasicType = BasicType::S32;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            BasicType::S8
            | BasicType::S16
            | BasicType::S32
            | BasicType::S64
            | BasicType::Ssize
            | BasicType::U8
            | BasicType::U16
            | BasicType::U32
            | BasicType::U64
            | BasicType::Usize => basic,
            _ => DEFAULT_INT_TYPE,
        },
        _ => DEFAULT_INT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::LitInt { val, ty: lit_type }),
    )
}

fn typecheck_lit_float<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: hir::Type<'hir>,
    val: f64,
) -> TypeResult<'hir> {
    const DEFAULT_FLOAT_TYPE: BasicType = BasicType::F64;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            BasicType::F32 | BasicType::F64 => basic,
            _ => DEFAULT_FLOAT_TYPE,
        },
        _ => DEFAULT_FLOAT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::LitFloat { val, ty: lit_type }),
    )
}

fn typecheck_lit_char<'hir>(emit: &mut HirEmit<'hir>, val: char) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Char),
        emit.arena.alloc(hir::Expr::LitChar { val }),
    )
}

fn typecheck_lit_string<'hir>(
    emit: &mut HirEmit<'hir>,
    id: InternID,
    c_string: bool,
) -> TypeResult<'hir> {
    let string_ty = if c_string {
        let byte = emit.arena.alloc(hir::Type::Basic(BasicType::U8));
        hir::Type::Reference(byte, ast::Mut::Immutable)
    } else {
        let slice = emit.arena.alloc(hir::ArraySlice {
            mutt: ast::Mut::Immutable,
            ty: hir::Type::Basic(BasicType::U8),
        });
        hir::Type::ArraySlice(slice)
    };

    let string_expr = hir::Expr::LitString { id, c_string };

    TypeResult::new(string_ty, emit.arena.alloc(string_expr))
}

fn typecheck_if<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    if_: &ast::If<'_>,
) -> TypeResult<'hir> {
    //@remove this when theres TypeRepr with shorthand creation functions eg TypeRepr::bool()
    const BOOL_TYPE: hir::Type = hir::Type::Basic(BasicType::Bool);

    let expect = if if_.fallback.is_some() {
        expect
    } else {
        hir::Type::Error
    };

    let entry_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, if_.entry.cond);
    let entry_block = typecheck_expr(hir, emit, proc, expect, if_.entry.block);
    let entry = hir::Branch {
        cond: entry_cond.expr,
        block: entry_block.expr,
    };

    //@approx esmitation based on first entry block type
    // this is the same typecheck error reporting problem as described below
    let if_type = if if_.fallback.is_some() {
        entry_block.ty
    } else {
        hir::Type::Basic(BasicType::Unit)
    };

    let mut branches = Vec::<hir::Branch>::new();
    for &branch in if_.branches {
        let branch_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, branch.cond);
        let branch_block = typecheck_expr(hir, emit, proc, expect, branch.block);
        branches.push(hir::Branch {
            cond: branch_cond.expr,
            block: branch_block.expr,
        });
    }

    let mut fallback = None;
    if let Some(block) = if_.fallback {
        let fallback_block = typecheck_expr(hir, emit, proc, expect, block);
        fallback = Some(fallback_block.expr);
    }

    let branches = emit.arena.alloc_slice(&branches);
    let if_ = emit.arena.alloc(hir::If {
        entry,
        branches,
        fallback,
    });
    let if_expr = emit.arena.alloc(hir::Expr::If { if_ });

    //@no idea which type to return for the if expression
    // too many different permutations, current expectation model doesnt provide enough information
    // for example caller cannot know if type error occured on that call
    // this problem applies to block, if, match, array expressions. @02.04.24
    TypeResult::new(if_type, if_expr)
}

fn typecheck_match<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    match_: &ast::Match<'_>,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr(hir, emit, proc, hir::Type::Error, match_.on_expr);

    if !verify_can_match(on_res.ty) {
        emit.error(
            ErrorComp::error(format!(
                "cannot match on value of type `{}`",
                type_format(hir, on_res.ty)
            ))
            .context(hir.src(proc.origin(), match_.on_expr.range)),
        );
    }

    //@approx esmitation based on first match block value
    let mut match_type = hir::Type::Error;
    let mut match_arms = Vec::<hir::MatchArm>::with_capacity(match_.arms.len());

    for (idx, arm) in match_.arms.iter().enumerate() {
        //@pat must also be constant value, which isnt checked
        // since it will always compile to a switch primitive
        let pat = if let Some(pat) = arm.pat {
            let pat_res = typecheck_expr(hir, emit, proc, on_res.ty, pat);
            Some(pat_res.expr)
        } else {
            None
        };

        let block_res = typecheck_expr(hir, emit, proc, expect, arm.expr);
        if idx == 0 {
            match_type = block_res.ty;
        }

        match_arms.push(hir::MatchArm {
            pat,
            expr: block_res.expr,
        })
    }

    let match_arms = emit.arena.alloc_slice(&match_arms);
    let match_ = emit.arena.alloc(hir::Match {
        on_expr: on_res.expr,
        arms: match_arms,
    });
    let match_expr = emit.arena.alloc(hir::Expr::Match { match_ });
    TypeResult::new(match_type, match_expr)
}

fn verify_can_match(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => match BasicTypeKind::from_basic(basic) {
            BasicTypeKind::Unit => false,
            BasicTypeKind::Bool => false,
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => true,
            BasicTypeKind::Float => false,
            BasicTypeKind::Char => false,
            BasicTypeKind::Rawptr => false,
        },
        hir::Type::Enum(_) => true,
        _ => false,
    }
}

fn typecheck_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);

    //@auto deref should be included in final hir @07.04.24
    // with current codegen pointer gep will run, without loading from pointer first.
    let (field_ty, kind) = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_type_field(hir, emit, proc, *ref_ty, name),
        _ => verify_type_field(hir, emit, proc, target_res.ty, name),
    };

    match kind {
        FieldExprKind::None => {
            TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
        }
        FieldExprKind::Member(union_id, id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::UnionMember {
                target: target_res.expr,
                union_id,
                id,
            }),
        ),
        FieldExprKind::Field(struct_id, id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::StructField {
                target: target_res.expr,
                struct_id,
                id,
            }),
        ),
    }
}

enum FieldExprKind {
    None,
    Member(hir::UnionID, hir::UnionMemberID),
    Field(hir::StructID, hir::StructFieldID),
}

fn verify_type_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldExprKind) {
    match ty {
        hir::Type::Error => (hir::Type::Error, FieldExprKind::None),
        hir::Type::Union(id) => {
            let data = hir.union_data(id);
            if let Some((member_id, member)) = data.find_member(name.id) {
                (member.ty, FieldExprKind::Member(id, member_id))
            } else {
                emit.error(
                    ErrorComp::error(format!(
                        "no field `{}` exists on union type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ))
                    .context(hir.src(proc.origin(), name.range)),
                );
                (hir::Type::Error, FieldExprKind::None)
            }
        }
        hir::Type::Struct(id) => {
            let data = hir.struct_data(id);
            if let Some((field_id, field)) = data.find_field(name.id) {
                (field.ty, FieldExprKind::Field(id, field_id))
            } else {
                emit.error(
                    ErrorComp::error(format!(
                        "no field `{}` exists on struct type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ))
                    .context(hir.src(proc.origin(), name.range)),
                );
                (hir::Type::Error, FieldExprKind::None)
            }
        }
        _ => {
            let ty_format = type_format(hir, ty);
            emit.error(
                ErrorComp::error(format!(
                    "no field `{}` exists on value of type {}",
                    hir.name_str(name.id),
                    ty_format,
                ))
                .context(hir.src(proc.origin(), name.range)),
            );
            (hir::Type::Error, FieldExprKind::None)
        }
    }
}

fn typecheck_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    index: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let index_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), index);

    let elem_ty = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_elem_type(*ref_ty),
        _ => verify_elem_type(target_res.ty),
    };

    if let Some(elem_ty) = elem_ty {
        let index_expr = emit.arena.alloc(hir::Expr::Index {
            target: target_res.expr,
            index: index_res.expr,
        });
        TypeResult::new(elem_ty, index_expr)
    } else {
        emit.error(
            ErrorComp::error(format!(
                "cannot index value of type {}",
                type_format(hir, target_res.ty)
            ))
            .context(hir.src(proc.origin(), index.range)),
        );
        TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
    }
}

fn verify_elem_type(ty: hir::Type) -> Option<hir::Type> {
    match ty {
        hir::Type::Error => Some(hir::Type::Error),
        hir::Type::ArraySlice(slice) => Some(slice.ty),
        hir::Type::ArrayStatic(array) => Some(array.ty),
        _ => None,
    }
}

fn basic_type_size(basic: BasicType) -> u64 {
    match basic {
        BasicType::Unit => 0,
        BasicType::Bool => 1,
        BasicType::S8 => 1,
        BasicType::S16 => 2,
        BasicType::S32 => 4,
        BasicType::S64 => 8,
        BasicType::Ssize => 8, //@assume 64bit target
        BasicType::U8 => 1,
        BasicType::U16 => 2,
        BasicType::U32 => 4,
        BasicType::U64 => 8,
        BasicType::Usize => 8, //@assume 64bit target
        BasicType::F32 => 4,
        BasicType::F64 => 8,
        BasicType::Char => 4,
        BasicType::Rawptr => 8, //@assume 64bit target
    }
}

enum BasicTypeKind {
    Unit,
    Bool,
    SignedInt,
    UnsignedInt,
    Float,
    Char,
    Rawptr,
}

impl BasicTypeKind {
    fn from_basic(basic: BasicType) -> BasicTypeKind {
        match basic {
            BasicType::Unit => BasicTypeKind::Unit,
            BasicType::Bool => BasicTypeKind::Bool,
            BasicType::S8 | BasicType::S16 | BasicType::S32 | BasicType::S64 | BasicType::Ssize => {
                BasicTypeKind::SignedInt
            }
            BasicType::U8 | BasicType::U16 | BasicType::U32 | BasicType::U64 | BasicType::Usize => {
                BasicTypeKind::UnsignedInt
            }
            BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
            BasicType::Char => BasicTypeKind::Char,
            BasicType::Rawptr => BasicTypeKind::Rawptr,
        }
    }
}

fn typecheck_cast<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    into: &ast::Type<'_>,
    range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let into = super::pass_3::type_resolve(hir, emit, proc.origin(), *into);

    // early return prevents false positives on cast warning & cast error
    if matches!(target_res.ty, hir::Type::Error) || matches!(into, hir::Type::Error) {
        //@this could be skipped by returning reference to same error expression
        // to save memory in error cases and reduce noise in the code itself.

        let cast_expr = hir::Expr::Cast {
            target: target_res.expr,
            into: emit.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult {
            ty: into,
            expr: emit.arena.alloc(cast_expr),
        };
    }

    // invariant: both types are not Error
    // ensured by early return above
    if type_matches(target_res.ty, into) {
        emit.error(
            ErrorComp::warning(format!(
                "redundant cast from `{}` into `{}`",
                type_format(hir, target_res.ty),
                type_format(hir, into)
            ))
            .context(hir.src(proc.origin(), range)),
        );

        let cast_expr = hir::Expr::Cast {
            target: target_res.expr,
            into: emit.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult {
            ty: into,
            expr: emit.arena.alloc(cast_expr),
        };
    }

    // invariant: from_size != into_size
    // ensured by cast redundancy warning above
    let cast_kind = match (target_res.ty, into) {
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            let from_kind = BasicTypeKind::from_basic(from);
            let into_kind = BasicTypeKind::from_basic(into);
            let from_size = basic_type_size(from);
            let into_size = basic_type_size(into);

            match from_kind {
                BasicTypeKind::Unit => hir::CastKind::Error,
                BasicTypeKind::Bool => hir::CastKind::Error,
                BasicTypeKind::SignedInt => match into_kind {
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => {
                        if from_size < into_size {
                            hir::CastKind::Sint_Sign_Extend
                        } else {
                            hir::CastKind::Integer_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::Sint_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::UnsignedInt => match into_kind {
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => {
                        if from_size < into_size {
                            hir::CastKind::Uint_Zero_Extend
                        } else {
                            hir::CastKind::Integer_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::Uint_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::Float => match into_kind {
                    BasicTypeKind::SignedInt => hir::CastKind::Float_to_Sint,
                    BasicTypeKind::UnsignedInt => hir::CastKind::Float_to_Uint,
                    BasicTypeKind::Float => {
                        if from_size < into_size {
                            hir::CastKind::Float_Extend
                        } else {
                            hir::CastKind::Float_Trunc
                        }
                    }
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::Char => hir::CastKind::Error,
                BasicTypeKind::Rawptr => hir::CastKind::Error,
            }
        }
        _ => hir::CastKind::Error,
    };

    if let hir::CastKind::Error = cast_kind {
        //@cast could be primitive but still invalid
        // wording might be improved
        // or have 2 error types for this
        emit.error(
            ErrorComp::error(format!(
                "non primitive cast from `{}` into `{}`",
                type_format(hir, target_res.ty),
                type_format(hir, into)
            ))
            .context(hir.src(proc.origin(), range)),
        );
    }

    let cast_expr = hir::Expr::Cast {
        target: target_res.expr,
        into: emit.arena.alloc(into),
        kind: cast_kind,
    };
    TypeResult {
        ty: into,
        expr: emit.arena.alloc(cast_expr),
    }
}

//@type-sizing not done:
// is complicated due to constant dependency graphs,
// recursive types also not detected yet.
fn typecheck_sizeof<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    ty: ast::Type,
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ty);

    let size = match ty {
        hir::Type::Basic(basic) => Some(basic_type_size(basic)),
        hir::Type::Reference(..) => Some(8), //@assume 64bit target
        hir::Type::ArraySlice(..) => Some(16), //@assume 64bit target
        _ => {
            emit.error(
                ErrorComp::error(
                    "sizeof for user defined or static array types is not yet supported",
                )
                .context(hir.src(proc.origin(), TextRange::new(start, start + 6.into()))),
            );
            None
        }
    };

    //@usize semantics not finalized yet
    // assigning usize type to constant int, since it represents size
    let sizeof_expr = if let Some(size) = size {
        emit.arena.alloc(hir::Expr::LitInt {
            val: size,
            ty: BasicType::Usize,
        })
    } else {
        emit.arena.alloc(hir::Expr::Error)
    };
    TypeResult::new(hir::Type::Basic(BasicType::Usize), sizeof_expr)
}

fn typecheck_item<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    path: &ast::Path,
) -> TypeResult<'hir> {
    let (value_id, field_names) = path_resolve_value(hir, emit, Some(proc), proc.origin(), path);

    let item_res = match value_id {
        ValueID::None => {
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
        ValueID::Enum(enum_id, variant_id) => {
            let enum_variant = hir::Expr::EnumVariant {
                enum_id,
                id: variant_id,
            };
            return TypeResult::new(hir::Type::Enum(enum_id), emit.arena.alloc(enum_variant));
        }
        ValueID::Const(id) => TypeResult::new(
            hir.const_data(id).ty,
            emit.arena.alloc(hir::Expr::ConstVar { const_id: id }),
        ),
        ValueID::Global(id) => TypeResult::new(
            hir.global_data(id).ty,
            emit.arena.alloc(hir::Expr::GlobalVar { global_id: id }),
        ),
        ValueID::Local(id) => TypeResult::new(
            proc.get_local(id).ty, //@type of local var might not be known
            emit.arena.alloc(hir::Expr::LocalVar { local_id: id }),
        ),
        ValueID::Param(id) => TypeResult::new(
            proc.get_param(id).ty,
            emit.arena.alloc(hir::Expr::ParamVar { param_id: id }),
        ),
    };

    let mut target = item_res.expr;
    let mut target_ty = item_res.ty;

    for &name in field_names {
        //@same code duplicated code as typecheck_field
        let (field_ty, kind) = match target_ty {
            hir::Type::Reference(ref_ty, mutt) => verify_type_field(hir, emit, proc, *ref_ty, name),
            _ => verify_type_field(hir, emit, proc, target_ty, name),
        };

        match kind {
            FieldExprKind::None => return TypeResult::new(hir::Type::Error, target),
            FieldExprKind::Member(union_id, id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::UnionMember {
                    target,
                    union_id,
                    id,
                });
            }
            FieldExprKind::Field(struct_id, id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::StructField {
                    target,
                    struct_id,
                    id,
                });
            }
        }
    }

    TypeResult::new(target_ty, target)
}

fn typecheck_proc_call<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    proc_call: &ast::ProcCall<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let proc_id = match path_resolve_proc(hir, emit, Some(proc), proc.origin(), proc_call.path) {
        Some(id) => id,
        None => {
            for &expr in proc_call.input {
                let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
            }
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
    };
    let data = hir.proc_data(proc_id);
    let input_count = proc_call.input.len();
    let expected_count = data.params.len();

    if (data.is_variadic && (input_count < expected_count))
        || (!data.is_variadic && (input_count != expected_count))
    {
        let at_least = if data.is_variadic { " at least" } else { "" };
        //@plular form for argument`s` only needed if != 1
        emit.error(
            ErrorComp::error(format!(
                "expected{at_least} {} input arguments, found {}",
                expected_count, input_count
            ))
            .context(hir.src(proc.origin(), expr_range))
            .context_info(
                "calling this procedure",
                hir.src(data.origin_id, data.name.range),
            ),
        );
    }

    let mut input = Vec::with_capacity(proc_call.input.len());
    for (idx, &expr) in proc_call.input.iter().enumerate() {
        let param = data.params.get(idx);
        let expect = match param {
            Some(param) => param.ty,
            None => hir::Type::Error,
        };
        let input_res = typecheck_expr(hir, emit, proc, expect, expr);
        input.push(input_res.expr);
    }

    let input = emit.arena.alloc_slice(&input);
    let proc_call = hir::Expr::ProcCall { proc_id, input };
    TypeResult::new(data.return_ty, emit.arena.alloc(proc_call))
}

fn typecheck_struct_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    struct_init: &ast::StructInit<'_>,
) -> TypeResult<'hir> {
    let structure_id =
        path_resolve_structure(hir, emit, Some(proc), proc.origin(), struct_init.path);
    let structure_name = *struct_init.path.names.last().expect("non empty path");

    match structure_id {
        StructureID::None => {
            for input in struct_init.input {
                let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
            }
            TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
        }
        StructureID::Union(union_id) => {
            let data = hir.union_data(union_id);

            if let Some((first, other)) = struct_init.input.split_first() {
                let type_res = if let Some((member_id, member)) = data.find_member(first.name.id) {
                    let input_res = typecheck_expr(hir, emit, proc, member.ty, first.expr);

                    let member_init = hir::UnionMemberInit {
                        member_id,
                        expr: input_res.expr,
                    };
                    let union_init = hir::Expr::UnionInit {
                        union_id,
                        input: member_init,
                    };
                    TypeResult::new(hir::Type::Union(union_id), emit.arena.alloc(union_init))
                } else {
                    emit.error(
                        ErrorComp::error(format!(
                            "field `{}` is not found",
                            hir.name_str(first.name.id),
                        ))
                        .context(hir.src(proc.origin(), first.name.range))
                        .context_info(
                            "union defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    );
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, first.expr);

                    TypeResult::new(
                        hir::Type::Union(union_id),
                        emit.arena.alloc(hir::Expr::Error),
                    )
                };

                for input in other {
                    emit.error(
                        ErrorComp::error("union initializer must have exactly one field")
                            .context(hir.src(proc.origin(), input.name.range)),
                    );
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }

                type_res
            } else {
                emit.error(
                    ErrorComp::error("union initializer must have exactly one field")
                        .context(hir.src(proc.origin(), structure_name.range)),
                );

                TypeResult::new(
                    hir::Type::Union(union_id),
                    emit.arena.alloc(hir::Expr::Error),
                )
            }
        }
        StructureID::Struct(struct_id) => {
            let data = hir.struct_data(struct_id);

            enum FieldStatus {
                None,
                Init(TextRange),
            }

            //@potentially a lot of allocations (simple solution), same memory could be re-used
            let mut field_inits = Vec::<hir::StructFieldInit>::with_capacity(data.fields.len());
            let mut field_status = Vec::<FieldStatus>::new();
            field_status.resize_with(data.fields.len(), || FieldStatus::None);
            let mut init_count: usize = 0;

            for input in struct_init.input {
                if let Some((field_id, field)) = data.find_field(input.name.id) {
                    let input_res = typecheck_expr(hir, emit, proc, field.ty, input.expr);

                    if let FieldStatus::Init(range) = field_status[field_id.index()] {
                        emit.error(
                            ErrorComp::error(format!(
                                "field `{}` was already initialized",
                                hir.name_str(input.name.id),
                            ))
                            .context(hir.src(proc.origin(), input.name.range))
                            .context_info("initialized here", hir.src(data.origin_id, range)),
                        );
                    } else {
                        let field_init = hir::StructFieldInit {
                            field_id,
                            expr: input_res.expr,
                        };
                        field_inits.push(field_init);
                        field_status[field_id.index()] = FieldStatus::Init(input.name.range);
                        init_count += 1;
                    }
                } else {
                    emit.error(
                        ErrorComp::error(format!(
                            "field `{}` is not found",
                            hir.name_str(input.name.id),
                        ))
                        .context(hir.src(proc.origin(), input.name.range))
                        .context_info(
                            "struct defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    );
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }
            }

            if init_count < data.fields.len() {
                let mut message = "missing field initializers: ".to_string();

                for (idx, status) in field_status.iter().enumerate() {
                    if let FieldStatus::None = status {
                        let field = data.fields[idx];
                        message.push('`');
                        message.push_str(hir.name_str(field.name.id));
                        if idx + 1 != data.fields.len() {
                            message.push_str("`, ");
                        } else {
                            message.push_str("`");
                        }
                    }
                }

                emit.error(
                    ErrorComp::error(message)
                        .context(hir.src(proc.origin(), structure_name.range))
                        .context_info(
                            "struct defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                );
            }

            let input = emit.arena.alloc_slice(&field_inits);
            let struct_init = hir::Expr::StructInit { struct_id, input };
            TypeResult::new(hir::Type::Struct(struct_id), emit.arena.alloc(struct_init))
        }
    }
}

fn typecheck_array_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    input: &[&ast::Expr<'_>],
) -> TypeResult<'hir> {
    //@unknown type in empty initializers gets ignored
    // same would be a problem for empty slices: `&[]`
    // & will be used as slicing syntax most likely
    // need to properly seaprate Error type and Unknown types
    // and handle them properly (relates to variables and overall inference flow) @06.04.24
    let mut elem_ty = hir::Type::Error;

    let expect = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let mut hir_input = Vec::with_capacity(input.len());
    for (idx, &expr) in input.iter().enumerate() {
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        hir_input.push(expr_res.expr);
        if idx == 0 {
            elem_ty = expr_res.ty;
        }
    }
    let hir_input = emit.arena.alloc_slice(&hir_input);

    //@types used during typechecking shount be a hir::Type
    // allocating const expr to store array size is temporary
    let size = emit.arena.alloc(hir::Expr::LitInt {
        val: input.len() as u64,
        ty: BasicType::Ssize,
    });
    let array_type = emit.arena.alloc(hir::ArrayStatic {
        size: hir::ConstExpr(size),
        ty: elem_ty,
    });

    let array_init = emit.arena.alloc(hir::ArrayInit {
        elem_ty,
        input: hir_input,
    });
    let array_expr = hir::Expr::ArrayInit { array_init };
    TypeResult::new(
        hir::Type::ArrayStatic(array_type),
        emit.arena.alloc(array_expr),
    )
}

fn typecheck_array_repeat<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    expr: &ast::Expr,
    size: ast::ConstExpr,
) -> TypeResult<'hir> {
    let expect = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
    let size_res = super::pass_4::const_expr_resolve(hir, emit, proc.origin(), size);

    let array_type = emit.arena.alloc(hir::ArrayStatic {
        size: size_res,
        ty: expr_res.ty,
    });

    let array_repeat = emit.arena.alloc(hir::ArrayRepeat {
        elem_ty: expr_res.ty,
        expr: expr_res.expr,
        size: size_res,
    });
    let array_expr = hir::Expr::ArrayRepeat { array_repeat };
    TypeResult::new(
        hir::Type::ArrayStatic(array_type),
        emit.arena.alloc(array_expr),
    )
}

fn typecheck_unary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::UnOp,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);
    //@not generating anything if type is invalid
    if let hir::Type::Error = rhs_res.ty {
        return TypeResult::new(hir::Type::Error, rhs_res.expr);
    }

    let unary_ty = match op {
        ast::UnOp::Neg => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::S8
                | BasicType::S16
                | BasicType::S32
                | BasicType::S64
                | BasicType::Ssize
                | BasicType::F32
                | BasicType::F64 => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::BitNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::U8
                | BasicType::U16
                | BasicType::U32
                | BasicType::U64
                | BasicType::Usize => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::LogicNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::Bool => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::Deref => match rhs_res.ty {
            hir::Type::Reference(ref_ty, ..) => *ref_ty,
            _ => hir::Type::Error,
        },
        //@consider mutt and maybe dont allow & of constants, but value could be copied so technically it works
        ast::UnOp::Addr(mutt) => {
            let ref_ty = emit.arena.alloc(rhs_res.ty);
            hir::Type::Reference(ref_ty, mutt)
        }
    };

    if let hir::Type::Error = unary_ty {
        //@unary op &str is same as token.to_str()
        // but those are separate types, this could be de-duplicated
        let op_str = match op {
            ast::UnOp::Neg => "-",
            ast::UnOp::BitNot => "~",
            ast::UnOp::LogicNot => "!",
            ast::UnOp::Deref => "*",
            ast::UnOp::Addr(mutt) => match mutt {
                ast::Mut::Mutable => "&mut",
                ast::Mut::Immutable => "&",
            },
        };
        emit.error(
            ErrorComp::error(format!(
                "unary operator `{op_str}` cannot be applied to `{}`",
                type_format(hir, rhs_res.ty)
            ))
            .context(hir.src(proc.origin(), rhs.range)),
        );
    }

    let unary_expr = hir::Expr::Unary {
        op,
        rhs: rhs_res.expr,
    };
    TypeResult::new(unary_ty, emit.arena.alloc(unary_expr))
}

fn typecheck_binary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    //@remove this when theres TypeRepr with shorthand creation functions eg TypeRepr::bool()
    const BOOL_TYPE: hir::Type = hir::Type::Basic(BasicType::Bool);

    let (binary_ty, lhs_expr, rhs_expr) = match op {
        ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Div => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if !check_type_allow_math_ops(lhs_res.ty) {
                emit.error(
                    ErrorComp::error(format!(
                        "cannot use math operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ))
                    .context(hir.src(proc.origin(), lhs.range)),
                );
                lhs_res.ty
            } else {
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Rem => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if !check_type_allow_remainder(lhs_res.ty) {
                emit.error(
                    ErrorComp::error(format!(
                        "cannot use remainder operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ))
                    .context(hir.src(proc.origin(), lhs.range)),
                );
                lhs_res.ty
            } else {
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::BitAnd
        | ast::BinOp::BitOr
        | ast::BinOp::BitXor
        | ast::BinOp::BitShl
        | ast::BinOp::BitShr => {
            //@binary bit ops are not checked yet
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            (lhs_res.ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::CmpIsEq | ast::BinOp::CmpNotEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_eq(lhs_res.ty) {
                emit.error(
                    ErrorComp::error(format!(
                        "cannot compare equality for value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ))
                    .context(hir.src(proc.origin(), lhs.range)),
                );
            }

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::CmpLt | ast::BinOp::CmpLtEq | ast::BinOp::CmpGt | ast::BinOp::CmpGtEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_ord(lhs_res.ty) {
                emit.error(
                    ErrorComp::error(format!(
                        "cannot compare order for value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ))
                    .context(hir.src(proc.origin(), lhs.range)),
                );
            }

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => {
            let lhs_res = typecheck_expr(hir, emit, proc, BOOL_TYPE, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, BOOL_TYPE, rhs);

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Range | ast::BinOp::RangeInc => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), rhs);

            panic!("pass_5: ranges dont procude a valid typechecked expression")
        }
    };

    let signed_int = match binary_ty {
        hir::Type::Basic(basic) => {
            matches!(BasicTypeKind::from_basic(basic), BasicTypeKind::SignedInt)
        }
        _ => false,
    };
    let binary_expr = hir::Expr::Binary {
        op,
        lhs: lhs_expr,
        rhs: rhs_expr,
        signed_int,
    };
    TypeResult::new(binary_ty, emit.arena.alloc(binary_expr))
}

fn check_type_allow_math_ops(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => match BasicTypeKind::from_basic(basic) {
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float => true,
            _ => false,
        },
        _ => false,
    }
}

fn check_type_allow_remainder(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => match BasicTypeKind::from_basic(basic) {
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => true,
            _ => false,
        },
        _ => false,
    }
}

fn check_type_allow_compare_eq(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => match BasicTypeKind::from_basic(basic) {
            BasicTypeKind::Bool
            | BasicTypeKind::SignedInt
            | BasicTypeKind::UnsignedInt
            | BasicTypeKind::Float
            | BasicTypeKind::Char
            | BasicTypeKind::Rawptr => true,
            _ => false,
        },
        _ => false,
    }
}

fn check_type_allow_compare_ord(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => match BasicTypeKind::from_basic(basic) {
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float => true,
            _ => false,
        },
        _ => false,
    }
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    stmts: &[ast::Stmt],
) -> TypeResult<'hir> {
    proc.push_block();

    let mut block_ty = None;
    let mut hir_stmts = Vec::with_capacity(stmts.len());

    for (idx, stmt) in stmts.iter().enumerate() {
        let hir_stmt = match stmt.kind {
            ast::StmtKind::Break => Some(typecheck_break(hir, emit, proc, stmt.range)),
            ast::StmtKind::Continue => Some(typecheck_continue(hir, emit, proc, stmt.range)),
            ast::StmtKind::Return(expr) => {
                Some(typecheck_return(hir, emit, proc, stmt.range, expr))
            }
            ast::StmtKind::Defer(block) => {
                Some(typecheck_defer(hir, emit, proc, stmt.range.start(), block))
            }
            ast::StmtKind::ForLoop(for_) => {
                let kind = match for_.kind {
                    ast::ForKind::Loop => hir::ForKind::Loop,
                    ast::ForKind::While { cond } => todo!("for_loop while-like is not supported"),
                    ast::ForKind::ForLoop {
                        local,
                        cond,
                        assign,
                    } => todo!("for_loop c-like is not supported"),
                };

                let block_res = typecheck_expr(
                    hir,
                    emit,
                    proc,
                    hir::Type::Basic(BasicType::Unit),
                    for_.block,
                );

                let for_ = hir::For {
                    kind,
                    block: block_res.expr,
                };
                Some(hir::Stmt::ForLoop(emit.arena.alloc(for_))) //@placeholder
            }
            ast::StmtKind::Local(local) => typecheck_local(hir, emit, proc, local),
            ast::StmtKind::Assign(assign) => Some(typecheck_assign(hir, emit, proc, assign)),
            ast::StmtKind::ExprSemi(expr) => {
                let expr_res = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
                Some(hir::Stmt::ExprSemi(expr_res.expr))
            }
            ast::StmtKind::ExprTail(expr) => {
                if idx + 1 == stmts.len() {
                    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                    block_ty = Some(expr_res.ty);
                    Some(hir::Stmt::ExprTail(expr_res.expr))
                } else {
                    //@decide if semi should enforced on parsing
                    // or we just expect a unit from that no-semi expression
                    // when its not a last expression?
                    let expr_res =
                        typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Unit), expr);
                    Some(hir::Stmt::ExprTail(expr_res.expr))
                }
            }
        };
        if let Some(hir_stmt) = hir_stmt {
            hir_stmts.push(hir_stmt);
        }
    }

    proc.pop_block();

    //@when type expectation was passed to tail expr
    // return Type::Error to not trigger duplicate type mismatch errors
    // this propagates typecheck match further down the call stack
    // without triggering error on each nested block level
    let block_ty = if block_ty.is_some() {
        //@this disables other typechecks
        // this hack of disabling type error also has side effect
        // of not producing any usefull errors after
        // this hir::Type::Error value from the block is a lie
        // the actual type is tail returned experrsion type stored in `block_ty`
        //hir::Type::Error

        block_ty.unwrap() //@generating more errors, but handling block type correctly
    } else {
        hir::Type::Basic(BasicType::Unit)
    };

    let stmts = emit.arena.alloc_slice(&hir_stmts);
    let block_expr = emit.arena.alloc(hir::Expr::Block { stmts });
    TypeResult::new(block_ty, block_expr)
}

fn typecheck_break<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.get_block().in_loop {
        emit.error(
            ErrorComp::error("cannot use `break` outside of a loop")
                .context(hir.src(proc.origin(), range)),
        );
    }

    hir::Stmt::Break
}

fn typecheck_continue<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.get_block().in_loop {
        emit.error(
            ErrorComp::error("cannot use `continue` outside of a loop")
                .context(hir.src(proc.origin(), range)),
        );
    }

    hir::Stmt::Continue
}

fn typecheck_return<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
    expr: Option<&ast::Expr>,
) -> hir::Stmt<'hir> {
    let expect = proc.data().return_ty;

    if let Some(expr) = expr {
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        hir::Stmt::ReturnVal(expr_res.expr)
    } else {
        //@this is modified duplicated typecheck error, special case for empty return @07.04.24
        let found = hir::Type::Basic(BasicType::Unit);
        if !type_matches(expect, found) {
            let expect_format = type_format(hir, expect);
            let found_format = type_format(hir, found);
            emit.error(
                ErrorComp::error(format!(
                    "type mismatch: expected `{}`, found `{}`",
                    expect_format, found_format,
                ))
                .context(hir.src(proc.origin(), range))
                .context_info(
                    format!("procedure returns `{expect_format}`"),
                    hir.src(proc.origin(), proc.data().name.range),
                ),
            );
        }
        hir::Stmt::Return
    }
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_defer<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    block: &ast::Expr<'_>,
) -> hir::Stmt<'hir> {
    if proc.get_block().in_defer {
        emit.error(
            ErrorComp::error("`defer` statements cannot be nested")
                .context(hir.src(proc.origin(), TextRange::new(start, start + 5.into()))),
        );
    }

    let block_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Unit), block);
    hir::Stmt::Defer(block_res.expr)
}

fn typecheck_local<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    local: &ast::Local,
) -> Option<hir::Stmt<'hir>> {
    if let Some(existing) = hir.scope_name_defined(proc.origin(), local.name.id) {
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);

        if let Some(ty) = local.ty {
            super::pass_3::type_resolve(hir, emit, proc.origin(), ty);
        };
        if let Some(expr) = local.value {
            let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
        }
        return None;
    }

    //@theres no `nice` way to find both existing name from global (hir) scope
    // and proc_scope, those are so far disconnected,
    // some unified model of symbols might be better in the future
    // this also applies to SymbolKind which is separate from VariableID (leads to some issues in path resolve) @1.04.24
    if let Some(existing_var) = proc.find_variable(local.name.id) {
        let existing = match existing_var {
            VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
            VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
        };
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);

        if let Some(ty) = local.ty {
            super::pass_3::type_resolve(hir, emit, proc.origin(), ty);
        };
        if let Some(expr) = local.value {
            let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
        }
        return None;
    }

    //@local type can be both not specified and not inferred by the expression
    // this is currently being represented as hir::Type::Error
    // instead of having some UnknownType.
    // also proc_scope stores hir::Local which are immutable
    // so type cannot be filled in after the fact.
    //@this could be adressed by requiring type or expression on local
    // this could be a valid design choice:
    // bindings like: `let x; let y;` dont make too much sence, and can be confusing; @1.04.24

    let mut local_ty = match local.ty {
        Some(ty) => super::pass_3::type_resolve(hir, emit, proc.origin(), ty),
        None => hir::Type::Error,
    };

    let local_value = match local.value {
        Some(expr) => {
            let expr_res = typecheck_expr(hir, emit, proc, local_ty, expr);
            if local.ty.is_none() {
                local_ty = expr_res.ty;
            }
            Some(expr_res.expr)
        }
        None => None,
    };

    let local = emit.arena.alloc(hir::Local {
        mutt: local.mutt,
        name: local.name,
        ty: local_ty,
        value: local_value,
    });
    let local_id = proc.push_local(local);
    Some(hir::Stmt::Local(local_id))
}

//@not checking lhs variable mutability
//@not emitting any specific errors like when assigning to constants
//@not checking bin assignment operators (need a good way to do it same in binary expr typecheck)
// clean this up in general
fn typecheck_assign<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    assign: &ast::Assign,
) -> hir::Stmt<'hir> {
    let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, assign.lhs);

    let expect = if matches!(lhs_res.ty, hir::Type::Error) {
        hir::Type::Error
    } else {
        if verify_is_expr_assignable(lhs_res.expr) {
            lhs_res.ty
        } else {
            emit.error(
                ErrorComp::error("cannot assign to this expression")
                    .context(hir.src(proc.origin(), assign.lhs.range)),
            );
            hir::Type::Error
        }
    };

    let rhs_res = typecheck_expr(hir, emit, proc, expect, assign.rhs);

    let assign = hir::Assign {
        op: assign.op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
    };
    hir::Stmt::Assign(emit.arena.alloc(assign))
}

fn verify_is_expr_assignable(expr: &hir::Expr) -> bool {
    match *expr {
        hir::Expr::Error => true,
        hir::Expr::UnionMember { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::StructField { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::Index { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::LocalVar { .. } => true,
        hir::Expr::ParamVar { .. } => true,
        hir::Expr::GlobalVar { .. } => true, //@add mut to globals
        hir::Expr::Unary { op, .. } => matches!(op, ast::UnOp::Deref),
        _ => false,
    }
}

/*

module     -> <first?>
proc       -> [no follow]
enum       -> <follow?> by single enum variant name
union      -> [no follow]
struct     -> [no follow]
const      -> <follow?> by <chained> field access
global     -> <follow?> by <chained> field access
param_var  -> <follow?> by <chained> field access
local_var  -> <follow?> by <chained> field access

*/

enum ResolvedPath {
    None,
    Variable(VariableID),
    Symbol(SymbolKind, SourceRange),
}

fn path_resolve<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> (ResolvedPath, usize) {
    let name = path.names.first().cloned().expect("non empty path");

    if let Some(proc) = proc {
        if let Some(var_id) = proc.find_variable(name.id) {
            return (ResolvedPath::Variable(var_id), 0);
        }
    }

    let (module_id, name) = match hir.symbol_from_scope(emit, origin_id, origin_id, name) {
        Some((kind, source)) => {
            let next_name = path.names.get(1).cloned();
            match (kind, next_name) {
                (SymbolKind::Module(module_id), Some(name)) => (module_id, name),
                _ => return (ResolvedPath::Symbol(kind, source), 0),
            }
        }
        None => return (ResolvedPath::None, 0),
    };

    match hir.symbol_from_scope(emit, origin_id, module_id, name) {
        Some((kind, source)) => (ResolvedPath::Symbol(kind, source), 1),
        None => return (ResolvedPath::None, 1),
    }
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let ty = match resolved {
        ResolvedPath::None => hir::Type::Error,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(
                ErrorComp::error(format!(
                    "expected type, found local `{}`",
                    hir.name_str(name.id)
                ))
                .context(hir.src(origin_id, name.range))
                .context_info("defined here", source),
            );
            return hir::Type::Error;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Union(id) => hir::Type::Union(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(
                    ErrorComp::error(format!(
                        "expected type, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ))
                    .context(hir.src(origin_id, name.range))
                    .context_info("defined here", source),
                );
                return hir::Type::Error;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(
                ErrorComp::error(format!("unexpected path segment",))
                    .context(hir.src(origin_id, range)),
            );
        }
    }

    ty
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
fn path_resolve_proc<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> Option<hir::ProcID> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let proc_id = match resolved {
        ResolvedPath::None => None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(
                ErrorComp::error(format!(
                    "expected procedure, found local `{}`",
                    hir.name_str(name.id)
                ))
                .context(hir.src(origin_id, name.range))
                .context_info("defined here", source),
            );
            return None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Proc(id) => Some(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(
                    ErrorComp::error(format!(
                        "expected procedure, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ))
                    .context(hir.src(origin_id, name.range))
                    .context_info("defined here", source),
                );
                return None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(
                ErrorComp::error(format!("unexpected path segment",))
                    .context(hir.src(origin_id, range)),
            );
        }
    }

    proc_id
}

enum StructureID {
    None,
    Union(hir::UnionID),
    Struct(hir::StructID),
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
fn path_resolve_structure<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> StructureID {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let structure_id = match resolved {
        ResolvedPath::None => StructureID::None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(
                ErrorComp::error(format!(
                    "expected struct or union, found local `{}`",
                    hir.name_str(name.id)
                ))
                .context(hir.src(origin_id, name.range))
                .context_info("defined here", source),
            );
            return StructureID::None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Union(id) => StructureID::Union(id),
            SymbolKind::Struct(id) => StructureID::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(
                    ErrorComp::error(format!(
                        "expected struct or union, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ))
                    .context(hir.src(origin_id, name.range))
                    .context_info("defined here", source),
                );
                return StructureID::None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(
                ErrorComp::error(format!("unexpected path segment",))
                    .context(hir.src(origin_id, range)),
            );
        }
    }

    structure_id
}

enum ValueID {
    None,
    Enum(hir::EnumID, hir::EnumVariantID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

fn path_resolve_value<'hir, 'ast>(
    hir: &HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> (ValueID, &'ast [ast::Name]) {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let value_id = match resolved {
        ResolvedPath::None => ValueID::None,
        ResolvedPath::Variable(var) => match var {
            VariableID::Local(id) => ValueID::Local(id),
            VariableID::Param(id) => ValueID::Param(id),
        },
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => {
                if let Some(variant_name) = path.names.get(name_idx + 1) {
                    let enum_data = hir.enum_data(id);
                    if let Some((variant_id, ..)) = enum_data.find_variant(variant_name.id) {
                        if let Some(remaining) = path.names.get(name_idx + 2..) {
                            if let (Some(first), Some(last)) = (remaining.first(), remaining.last())
                            {
                                let range = TextRange::new(first.range.start(), last.range.end());
                                emit.error(
                                    ErrorComp::error(format!("unexpected path segment",))
                                        .context(hir.src(origin_id, range)),
                                );
                            }
                        }
                        return (ValueID::Enum(id, variant_id), &[]);
                    } else {
                        emit.error(
                            ErrorComp::error(format!(
                                "enum variant `{}` is not found",
                                hir.name_str(variant_name.id)
                            ))
                            .context(hir.src(origin_id, variant_name.range))
                            .context_info("enum defined here", source),
                        );
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    emit.error(
                        ErrorComp::error(format!(
                            "expected value, found {} `{}`",
                            HirData::symbol_kind_name(kind),
                            hir.name_str(name.id)
                        ))
                        .context(hir.src(origin_id, name.range))
                        .context_info("defined here", source),
                    );
                    return (ValueID::None, &[]);
                }
            }
            SymbolKind::Const(id) => ValueID::Const(id),
            SymbolKind::Global(id) => ValueID::Global(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(
                    ErrorComp::error(format!(
                        "expected value, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ))
                    .context(hir.src(origin_id, name.range))
                    .context_info("defined here", source),
                );
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

use super::hir_build::{self, HirData, HirEmit, SymbolKind};
use super::pass_4;
use super::proc_scope::{ProcScope, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};

pub fn check_attribute(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ModuleID,
    attr: Option<ast::Attribute>,
    expected: ast::AttributeKind,
) -> bool {
    let attr = if let Some(attr) = attr {
        attr
    } else {
        return false;
    };

    if attr.kind == expected {
        return true;
    }

    match attr.kind {
        ast::AttributeKind::Unknown => {
            emit.error(ErrorComp::error(
                format!(
                    "unknown attribute, only #[{}] is allowed here",
                    expected.as_str()
                ),
                hir.src(origin_id, attr.range),
                None,
            ));
        }
        _ => {
            emit.error(ErrorComp::error(
                format!(
                    "unexpected #[{}] attribute, only #[{}] is allowed here",
                    attr.kind.as_str(),
                    expected.as_str()
                ),
                hir.src(origin_id, attr.range),
                None,
            ));
        }
    }
    false
}

pub fn typecheck_procedures<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().proc_ids() {
        typecheck_proc(hir, emit, id)
    }
}

fn typecheck_proc<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.registry().proc_item(id);
    let data = hir.registry().proc_data(id);

    let c_call = check_attribute(
        hir,
        emit,
        data.origin_id,
        item.attr_tail,
        ast::AttributeKind::C_Call,
    );

    if c_call {
        if item.block.is_some() {
            emit.error(ErrorComp::error(
                "procedures with #[c_call] attribute cannot have a body",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    } else {
        if item.block.is_none() {
            emit.error(ErrorComp::error(
                "expected tail attribute #[c_call], since procedure has no body",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if data.is_variadic {
            emit.error(ErrorComp::error(
                "procedures without #[c_call] attribute cannot be variadic, remove `..` from parameter list",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    if data.is_test {
        if c_call {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute cannot be external #[c_call]",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if !data.params.is_empty() {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute cannot have any input parameters",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        // not allowing `never` in test procedures,
        // panic or exit in test code doesnt make much sence, or does it? @27.04.24
        if !matches!(data.return_ty, hir::Type::Basic(BasicType::Void)) {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute can only return `void`",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    // main entry point cannot be external or a test (is_main detection isnt done yet) @27.04.24

    if let Some(block) = item.block {
        let mut proc = ProcScope::new(data);

        let block_res = typecheck_block(hir, emit, &mut proc, data.return_ty, block, false, None);

        let locals = proc.finish();
        let locals = emit.arena.alloc_slice(&locals);

        let data = hir.registry_mut().proc_data_mut(id);
        data.block = Some(block_res.expr);
        data.locals = locals;
    }
}

pub fn type_matches<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &HirEmit<'hir>,
    ty: hir::Type<'hir>,
    ty2: hir::Type<'hir>,
) -> bool {
    match (ty, ty2) {
        (hir::Type::Error, ..) => true,
        (.., hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (hir::Type::Enum(id), hir::Type::Enum(id2)) => id == id2,
        (hir::Type::Union(id), hir::Type::Union(id2)) => id == id2,
        (hir::Type::Struct(id), hir::Type::Struct(id2)) => id == id2,
        (hir::Type::Reference(ref_ty, mutt), hir::Type::Reference(ref_ty2, mutt2)) => {
            mutt == mutt2 && type_matches(hir, emit, *ref_ty, *ref_ty2)
        }
        (hir::Type::Procedure(proc_ty), hir::Type::Procedure(proc_ty2)) => {
            (proc_ty.params.len() == proc_ty2.params.len())
                && (proc_ty.is_variadic == proc_ty2.is_variadic)
                && type_matches(hir, emit, proc_ty.return_ty, proc_ty2.return_ty)
                && (0..proc_ty.params.len())
                    .all(|idx| type_matches(hir, emit, proc_ty.params[idx], proc_ty2.params[idx]))
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            (slice.mutt == slice2.mutt) && type_matches(hir, emit, slice.elem_ty, slice2.elem_ty)
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            if let Some(len) = array_static_get_len(hir, emit, array.len) {
                if let Some(len2) = array_static_get_len(hir, emit, array2.len) {
                    return (len == len2) && type_matches(hir, emit, array.elem_ty, array2.elem_ty);
                }
            }
            true
        }
        (hir::Type::ArrayStatic(array), ..) => array_static_get_len(hir, emit, array.len).is_none(),
        (.., hir::Type::ArrayStatic(array2)) => {
            array_static_get_len(hir, emit, array2.len).is_none()
        }
        _ => false,
    }
}

//@can use &'static str often 07.05.24
pub fn type_format<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &HirEmit<'hir>,
    ty: hir::Type<'hir>,
) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
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
            BasicType::F16 => "f16".into(),
            BasicType::F32 => "f32".into(),
            BasicType::F64 => "f64".into(),
            BasicType::Bool => "bool".into(),
            BasicType::Char => "char".into(),
            BasicType::Rawptr => "rawptr".into(),
            BasicType::Void => "void".into(),
            BasicType::Never => "never".into(),
        },
        hir::Type::Enum(id) => hir.name_str(hir.registry().enum_data(id).name.id).into(),
        hir::Type::Union(id) => hir.name_str(hir.registry().union_data(id).name.id).into(),
        hir::Type::Struct(id) => hir.name_str(hir.registry().struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hir, emit, *ref_ty))
        }
        hir::Type::Procedure(proc_ty) => {
            let mut string = String::from("proc(");
            for (idx, param) in proc_ty.params.iter().enumerate() {
                string.push_str(&type_format(hir, emit, *param));
                if proc_ty.params.len() != idx + 1 {
                    string.push_str(", ");
                }
            }
            if proc_ty.is_variadic {
                string.push_str(", ..")
            }
            string.push_str(") -> ");
            string.push_str(&type_format(hir, emit, proc_ty.return_ty));
            string
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hir, emit, slice.elem_ty))
        }
        hir::Type::ArrayStatic(array) => {
            let len = array_static_get_len(hir, emit, array.len);
            let elem_format: String = type_format(hir, emit, array.elem_ty);
            match len {
                Some(len) => format!("[{len}]{elem_format}"),
                None => format!("[<unknown>]{elem_format}"),
            }
        }
    }
}

fn array_static_get_len<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &HirEmit<'hir>,
    len: hir::ArrayStaticLen,
) -> Option<u64> {
    match len {
        hir::ArrayStaticLen::Immediate(len) => len,
        hir::ArrayStaticLen::ConstEval(eval_id) => {
            let (eval, _) = *hir.registry().const_eval(eval_id);
            match eval {
                hir::ConstEval::ResolvedValue(value_id) => {
                    let value = emit.const_intern.get(value_id);
                    match value {
                        hir::ConstValue::Int { val, neg, ty } => {
                            if neg {
                                None
                            } else {
                                Some(val)
                            }
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
    }
}

struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    expr: &'hir hir::Expr<'hir>,
    ignore: bool,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: false,
        }
    }

    fn new_ignore_typecheck(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: true,
        }
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
        ast::ExprKind::LitNull => typecheck_lit_null(emit),
        ast::ExprKind::LitBool { val } => typecheck_lit_bool(emit, val),
        ast::ExprKind::LitInt { val } => typecheck_lit_int(emit, expect, val),
        ast::ExprKind::LitFloat { val } => typecheck_lit_float(emit, expect, val),
        ast::ExprKind::LitChar { val } => typecheck_lit_char(emit, val),
        ast::ExprKind::LitString { id, c_string } => typecheck_lit_string(emit, id, c_string),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_),
        ast::ExprKind::Block { block } => {
            typecheck_block(hir, emit, proc, expect, block, false, None)
        }
        ast::ExprKind::Match { match_ } => typecheck_match(hir, emit, proc, expect, match_),
        ast::ExprKind::Field { target, name } => typecheck_field(hir, emit, proc, target, name),
        ast::ExprKind::Index { target, index } => {
            typecheck_index(hir, emit, proc, target, index, expr.range)
        }
        ast::ExprKind::Slice {
            target,
            mutt,
            slice_range,
        } => typecheck_slice(hir, emit, proc, target, mutt, slice_range, expr.range),
        ast::ExprKind::Call { target, input } => {
            typecheck_call(hir, emit, proc, target, input, expr.range)
        }
        ast::ExprKind::Cast { target, into } => {
            typecheck_cast(hir, emit, proc, target, into, expr.range)
        }
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, ty, expr.range),
        ast::ExprKind::Item { path } => typecheck_item(hir, emit, proc, path),
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(hir, emit, proc, expect, input),
        ast::ExprKind::ArrayRepeat { expr, len } => {
            typecheck_array_repeat(hir, emit, proc, expect, expr, len)
        }
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(hir, emit, proc, mutt, rhs),
        ast::ExprKind::Unary { op, rhs } => typecheck_unary(hir, emit, proc, op, rhs),
        ast::ExprKind::Binary { op, lhs, rhs } => typecheck_binary(hir, emit, proc, op, lhs, rhs),
    };

    if !expr_res.ignore && !type_matches(hir, emit, expect, expr_res.ty) {
        emit.error(ErrorComp::error(
            format!(
                "type mismatch: expected `{}`, found `{}`",
                type_format(hir, emit, expect),
                type_format(hir, emit, expr_res.ty)
            ),
            hir.src(proc.origin(), expr.range),
            None,
        ));
    }

    expr_res
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
    let lit_type = coerce_int_type(expect);
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
    let lit_type = coerce_float_type(expect);
    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::LitFloat { val, ty: lit_type }),
    )
}

pub fn coerce_int_type(expect: hir::Type) -> BasicType {
    const DEFAULT_INT_TYPE: BasicType = BasicType::S32;

    match expect {
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
    }
}

pub fn coerce_float_type(expect: hir::Type) -> BasicType {
    const DEFAULT_FLOAT_TYPE: BasicType = BasicType::F64;

    match expect {
        hir::Type::Basic(basic) => match basic {
            BasicType::F16 | BasicType::F32 | BasicType::F64 => basic,
            _ => DEFAULT_FLOAT_TYPE,
        },
        _ => DEFAULT_FLOAT_TYPE,
    }
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
    let string_ty = alloc_string_lit_type(emit, c_string);
    let string_expr = hir::Expr::LitString { id, c_string };
    TypeResult::new(string_ty, emit.arena.alloc(string_expr))
}

pub fn alloc_string_lit_type<'hir>(emit: &mut HirEmit<'hir>, c_string: bool) -> hir::Type<'hir> {
    if c_string {
        let byte = emit.arena.alloc(hir::Type::Basic(BasicType::U8));
        hir::Type::Reference(byte, ast::Mut::Immutable)
    } else {
        let slice = emit.arena.alloc(hir::ArraySlice {
            mutt: ast::Mut::Immutable,
            elem_ty: hir::Type::Basic(BasicType::U8),
        });
        hir::Type::ArraySlice(slice)
    }
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

    let expect = if if_.else_block.is_some() {
        expect
    } else {
        hir::Type::Error
    };

    let entry_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, if_.entry.cond);
    let entry_block = typecheck_block(hir, emit, proc, expect, if_.entry.block, false, None);
    let entry = hir::Branch {
        cond: entry_cond.expr,
        block: entry_block.expr,
    };

    //@approx esmitation based on first entry block type
    // this is the same typecheck error reporting problem as described below
    let if_type = if if_.else_block.is_some() {
        entry_block.ty
    } else {
        hir::Type::Basic(BasicType::Void)
    };

    let mut branches = Vec::<hir::Branch>::new();
    for &branch in if_.branches {
        let branch_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, branch.cond);
        let branch_block = typecheck_block(hir, emit, proc, expect, branch.block, false, None);
        branches.push(hir::Branch {
            cond: branch_cond.expr,
            block: branch_block.expr,
        });
    }

    let mut fallback = None;
    if let Some(else_block) = if_.else_block {
        let fallback_block = typecheck_block(hir, emit, proc, expect, else_block, false, None);
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
        emit.error(ErrorComp::error(
            format!(
                "cannot match on value of type `{}`",
                type_format(hir, emit, on_res.ty)
            ),
            hir.src(proc.origin(), match_.on_expr.range),
            None,
        ));
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
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
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
    let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_res.ty, name);

    match kind {
        FieldKind::Error => TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error)),
        FieldKind::Member(union_id, member_id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::UnionMember {
                target: target_res.expr,
                union_id,
                member_id,
                deref,
            }),
        ),
        FieldKind::Field(struct_id, field_id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::StructField {
                target: target_res.expr,
                struct_id,
                field_id,
                deref,
            }),
        ),
        FieldKind::Slice { first_ptr } => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::SliceField {
                target: target_res.expr,
                first_ptr,
                deref,
            }),
        ),
    }
}

enum FieldKind {
    Error,
    Member(hir::UnionID, hir::UnionMemberID),
    Field(hir::StructID, hir::StructFieldID),
    Slice { first_ptr: bool },
}

fn check_type_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldKind, bool) {
    let field = match ty {
        hir::Type::Reference(ref_ty, mutt) => {
            (type_get_field(hir, emit, proc, *ref_ty, name), true)
        }
        _ => (type_get_field(hir, emit, proc, ty, name), false),
    };
    (field.0 .0, field.0 .1, field.1)
}

fn type_get_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldKind) {
    match ty {
        hir::Type::Error => (hir::Type::Error, FieldKind::Error),
        hir::Type::Union(id) => {
            let data = hir.registry().union_data(id);
            if let Some((member_id, member)) = data.find_member(name.id) {
                (member.ty, FieldKind::Member(id, member_id))
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "no field `{}` exists on union type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ),
                    hir.src(proc.origin(), name.range),
                    None,
                ));
                (hir::Type::Error, FieldKind::Error)
            }
        }
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            if let Some((field_id, field)) = data.find_field(name.id) {
                (field.ty, FieldKind::Field(id, field_id))
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "no field `{}` exists on struct type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ),
                    hir.src(proc.origin(), name.range),
                    None,
                ));
                (hir::Type::Error, FieldKind::Error)
            }
        }
        hir::Type::ArraySlice(slice) => {
            let field_name = hir.name_str(name.id);
            match field_name {
                "ptr" => (
                    hir::Type::Reference(&slice.elem_ty, ast::Mut::Immutable),
                    FieldKind::Slice { first_ptr: true },
                ),
                "len" => (
                    hir::Type::Basic(BasicType::Usize),
                    FieldKind::Slice { first_ptr: false },
                ),
                _ => {
                    let ty_format = type_format(hir, emit, ty);
                    emit.error(ErrorComp::error(
                        format!(
                            "no field `{}` exists on slice type `{}`\ndid you mean `len` or `ptr`?",
                            hir.name_str(name.id),
                            ty_format,
                        ),
                        hir.src(proc.origin(), name.range),
                        None,
                    ));
                    (hir::Type::Error, FieldKind::Error)
                }
            }
        }
        _ => {
            let ty_format = type_format(hir, emit, ty);
            emit.error(ErrorComp::error(
                format!(
                    "no field `{}` exists on value of type `{}`",
                    hir.name_str(name.id),
                    ty_format,
                ),
                hir.src(proc.origin(), name.range),
                None,
            ));
            (hir::Type::Error, FieldKind::Error)
        }
    }
}

struct CollectionType<'hir> {
    deref: bool,
    elem_ty: hir::Type<'hir>,
    kind: SliceOrArray<'hir>,
}

enum SliceOrArray<'hir> {
    Slice(&'hir hir::ArraySlice<'hir>),
    Array(&'hir hir::ArrayStatic<'hir>),
}

impl<'hir> CollectionType<'hir> {
    fn from(ty: hir::Type<'hir>) -> Result<Option<CollectionType<'hir>>, ()> {
        fn type_collection<'hir>(
            ty: hir::Type<'hir>,
            deref: bool,
        ) -> Result<Option<CollectionType<'hir>>, ()> {
            match ty {
                hir::Type::ArraySlice(slice) => Ok(Some(CollectionType {
                    deref,
                    elem_ty: slice.elem_ty,
                    kind: SliceOrArray::Slice(slice),
                })),
                hir::Type::ArrayStatic(array) => Ok(Some(CollectionType {
                    deref,
                    elem_ty: array.elem_ty,
                    kind: SliceOrArray::Array(array),
                })),
                hir::Type::Error => Ok(None),
                _ => Err(()),
            }
        }

        match ty {
            hir::Type::Reference(ref_ty, _) => type_collection(*ref_ty, true),
            _ => type_collection(ty, false),
        }
    }
}

fn typecheck_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    index: &ast::Expr<'_>,
    expr_range: TextRange, //@use range of brackets? `[]` 08.05.24
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let index_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), index);

    match CollectionType::from(target_res.ty) {
        Ok(Some(collection)) => {
            let access = hir::IndexAccess {
                deref: collection.deref,
                elem_ty: collection.elem_ty,
                kind: match collection.kind {
                    SliceOrArray::Slice(slice) => hir::IndexKind::Slice {
                        elem_size: type_size(
                            hir,
                            emit,
                            slice.elem_ty,
                            hir.src(proc.origin(), expr_range), //@review source range for this type_size error 10.05.24
                        )
                        .unwrap_or(hir::Size::new(0, 1))
                        .size(),
                    },
                    SliceOrArray::Array(array) => hir::IndexKind::Array { array },
                },
                index: index_res.expr,
            };

            let index_expr = hir::Expr::Index {
                target: target_res.expr,
                access: emit.arena.alloc(access),
            };
            TypeResult::new(collection.elem_ty, emit.arena.alloc(index_expr))
        }
        Ok(None) => TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR),
        Err(()) => {
            emit.error(ErrorComp::error(
                format!(
                    "cannot index value of type `{}`",
                    type_format(hir, emit, target_res.ty)
                ),
                hir.src(proc.origin(), expr_range),
                ErrorComp::info(
                    format!("has `{}` type", type_format(hir, emit, target_res.ty)),
                    hir.src(proc.origin(), target.range),
                ),
            ));
            TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR)
        }
    }
}

fn typecheck_slice<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    mutt: ast::Mut,
    slice: &ast::SliceRange<'_>,
    expr_range: TextRange, //@use range of brackets? `[]` 08.05.24
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);

    let lower = slice.lower.map(|lower| {
        let lower_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), lower);
        lower_res.expr
    });
    let upper = match slice.upper {
        ast::SliceRangeEnd::Unbounded => hir::SliceRangeEnd::Unbounded,
        ast::SliceRangeEnd::Exclusive(upper) => {
            let upper_res =
                typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), upper);
            hir::SliceRangeEnd::Exclusive(upper_res.expr)
        }
        ast::SliceRangeEnd::Inclusive(upper) => {
            let upper_res =
                typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), upper);
            hir::SliceRangeEnd::Inclusive(upper_res.expr)
        }
    };

    match CollectionType::from(target_res.ty) {
        Ok(Some(collection)) => {
            let access = hir::SliceAccess {
                deref: collection.deref,
                kind: match collection.kind {
                    SliceOrArray::Slice(slice) => hir::SliceKind::Slice {
                        elem_size: type_size(
                            hir,
                            emit,
                            slice.elem_ty,
                            hir.src(proc.origin(), expr_range), //@review source range for this type_size error 10.05.24
                        )
                        .unwrap_or(hir::Size::new(0, 1))
                        .size(),
                    },
                    SliceOrArray::Array(array) => hir::SliceKind::Array { array },
                },
                range: hir::SliceRange { lower, upper },
            };

            //@mutability not checked, use addressability? 08.05.24
            // or only base type? check different cases (eg:  &slice_var[mut ..] // invalid? )
            let slice_ty = emit.arena.alloc(hir::ArraySlice {
                mutt,
                elem_ty: collection.elem_ty,
            });

            let slice_expr = hir::Expr::Slice {
                target: target_res.expr,
                access: emit.arena.alloc(access),
            };
            TypeResult::new(
                hir::Type::ArraySlice(slice_ty),
                emit.arena.alloc(slice_expr),
            )
        }
        Ok(None) => TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR),
        Err(()) => {
            emit.error(ErrorComp::error(
                format!(
                    "cannot slice value of type `{}`",
                    type_format(hir, emit, target_res.ty)
                ),
                hir.src(proc.origin(), expr_range),
                ErrorComp::info(
                    format!("has `{}` type", type_format(hir, emit, target_res.ty)),
                    hir.src(proc.origin(), target.range),
                ),
            ));
            TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR)
        }
    }
}

fn typecheck_call<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    input: &&[&ast::Expr<'_>],
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);

    match target_res.ty {
        hir::Type::Error => {}
        hir::Type::Procedure(proc_ty) => {
            // both direct and indirect return proc_ty
            // it can be used for input checks

            let direct_id = match target_res.expr {
                hir::Expr::Procedure { proc_id } => Some(*proc_id),
                _ => None,
            };

            let input_count = input.len();
            let expected_count = proc_ty.params.len();

            if (proc_ty.is_variadic && (input_count < expected_count))
                || (!proc_ty.is_variadic && (input_count != expected_count))
            {
                let at_least = if proc_ty.is_variadic { " at least" } else { "" };

                let info = if let Some(proc_id) = direct_id {
                    let data = hir.registry().proc_data(proc_id);
                    ErrorComp::info(
                        "calling this procedure",
                        hir.src(data.origin_id, data.name.range),
                    )
                } else {
                    None
                };

                //@plular form for argument`s` only needed if != 1
                emit.error(ErrorComp::error(
                    format!(
                        "expected{at_least} {} input arguments, found {}",
                        expected_count, input_count
                    ),
                    hir.src(proc.origin(), expr_range),
                    info,
                ));
            }

            let mut hir_input = Vec::with_capacity(input.len());
            for (idx, &expr) in input.iter().enumerate() {
                let expect = match proc_ty.params.get(idx) {
                    Some(param) => *param,
                    None => hir::Type::Error,
                };
                let input_res = typecheck_expr(hir, emit, proc, expect, expr);
                hir_input.push(input_res.expr);
            }
            let hir_input = emit.arena.alloc_slice(&hir_input);

            let call_expr = match direct_id {
                Some(proc_id) => hir::Expr::CallDirect {
                    proc_id,
                    input: hir_input,
                },
                None => hir::Expr::CallIndirect {
                    target: target_res.expr,
                    indirect: emit.arena.alloc(hir::CallIndirect {
                        proc_ty,
                        input: hir_input,
                    }),
                },
            };
            return TypeResult::new(proc_ty.return_ty, emit.arena.alloc(call_expr));
        }
        _ => {
            emit.error(ErrorComp::error(
                format!(
                    "cannot call value of type `{}`",
                    type_format(hir, emit, target_res.ty)
                ),
                hir.src(proc.origin(), target.range),
                None,
            ));
        }
    }

    for &expr in input.iter() {
        let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
    }
    TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
}

pub fn type_size(
    hir: &HirData,
    emit: &mut HirEmit,
    ty: hir::Type,
    source: SourceRange,
) -> Option<hir::Size> {
    match ty {
        hir::Type::Error => None,
        hir::Type::Basic(basic) => Some(basic_type_size(basic)),
        hir::Type::Enum(id) => Some(basic_type_size(hir.registry().enum_data(id).basic)),
        hir::Type::Union(id) => hir.registry().union_data(id).size_eval.get_size(),
        hir::Type::Struct(id) => hir.registry().struct_data(id).size_eval.get_size(),
        hir::Type::Reference(_, _) => Some(hir::Size::new_equal(8)), //@assume 64bit target
        hir::Type::Procedure(_) => Some(hir::Size::new_equal(8)),    //@assume 64bit target
        hir::Type::ArraySlice(_) => Some(hir::Size::new(16, 8)),     //@assume 64bit target
        hir::Type::ArrayStatic(array) => {
            if let (Some(elem_size), Some(len)) = (
                type_size(hir, emit, array.elem_ty, source),
                array_static_get_len(hir, emit, array.len),
            ) {
                if let Some(array_size) = elem_size.size().checked_mul(len) {
                    Some(hir::Size::new(array_size, elem_size.align()))
                } else {
                    emit.error(ErrorComp::error(
                        format!(
                            "array size overflow: `{}` * `{}` (elem_size * array_len)",
                            elem_size.size(),
                            len
                        ),
                        source,
                        None,
                    ));
                    None
                }
            } else {
                None
            }
        }
    }
}

fn basic_type_size(basic: BasicType) -> hir::Size {
    match basic {
        BasicType::S8 => hir::Size::new_equal(1),
        BasicType::S16 => hir::Size::new_equal(2),
        BasicType::S32 => hir::Size::new_equal(4),
        BasicType::S64 => hir::Size::new_equal(8),
        BasicType::Ssize => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::U8 => hir::Size::new_equal(1),
        BasicType::U16 => hir::Size::new_equal(2),
        BasicType::U32 => hir::Size::new_equal(4),
        BasicType::U64 => hir::Size::new_equal(8),
        BasicType::Usize => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::F16 => hir::Size::new_equal(2),
        BasicType::F32 => hir::Size::new_equal(4),
        BasicType::F64 => hir::Size::new_equal(8),
        BasicType::Bool => hir::Size::new_equal(1),
        BasicType::Char => hir::Size::new_equal(4),
        BasicType::Rawptr => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::Void => hir::Size::new(0, 1),
        BasicType::Never => hir::Size::new(0, 1),
    }
}

enum BasicTypeKind {
    SignedInt,
    UnsignedInt,
    Float,
    Bool,
    Char,
    Rawptr,
    Void,
    Never,
}

impl BasicTypeKind {
    fn from_basic(basic: BasicType) -> BasicTypeKind {
        match basic {
            BasicType::S8 | BasicType::S16 | BasicType::S32 | BasicType::S64 | BasicType::Ssize => {
                BasicTypeKind::SignedInt
            }
            BasicType::U8 | BasicType::U16 | BasicType::U32 | BasicType::U64 | BasicType::Usize => {
                BasicTypeKind::UnsignedInt
            }
            BasicType::F16 | BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
            BasicType::Bool => BasicTypeKind::Bool,
            BasicType::Char => BasicTypeKind::Char,
            BasicType::Rawptr => BasicTypeKind::Rawptr,
            BasicType::Void => BasicTypeKind::Void,
            BasicType::Never => BasicTypeKind::Never,
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
        return TypeResult::new(into, emit.arena.alloc(cast_expr));
    }

    // invariant: both types are not Error
    // ensured by early return above
    if type_matches(hir, emit, target_res.ty, into) {
        emit.error(ErrorComp::warning(
            format!(
                "redundant cast from `{}` into `{}`",
                type_format(hir, emit, target_res.ty),
                type_format(hir, emit, into)
            ),
            hir.src(proc.origin(), range),
            None,
        ));

        let cast_expr = hir::Expr::Cast {
            target: target_res.expr,
            into: emit.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult::new(into, emit.arena.alloc(cast_expr));
    }

    // invariant: from_size != into_size
    // ensured by cast redundancy warning above
    let cast_kind = match (target_res.ty, into) {
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            let from_kind = BasicTypeKind::from_basic(from);
            let into_kind = BasicTypeKind::from_basic(into);
            let from_size = basic_type_size(from).size();
            let into_size = basic_type_size(into).size();

            match from_kind {
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
                BasicTypeKind::Bool => hir::CastKind::Error,
                BasicTypeKind::Char => hir::CastKind::Error,
                BasicTypeKind::Rawptr => hir::CastKind::Error,
                BasicTypeKind::Void => hir::CastKind::Error,
                BasicTypeKind::Never => hir::CastKind::Error,
            }
        }
        _ => hir::CastKind::Error,
    };

    if let hir::CastKind::Error = cast_kind {
        //@cast could be primitive but still invalid
        // wording might be improved
        // or have 2 error types for this
        emit.error(ErrorComp::error(
            format!(
                "non primitive cast from `{}` into `{}`",
                type_format(hir, emit, target_res.ty),
                type_format(hir, emit, into)
            ),
            hir.src(proc.origin(), range),
            None,
        ));
    }

    let cast_expr = hir::Expr::Cast {
        target: target_res.expr,
        into: emit.arena.alloc(into),
        kind: cast_kind,
    };
    TypeResult::new(into, emit.arena.alloc(cast_expr))
}

fn typecheck_sizeof<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: ast::Type,
    expr_range: TextRange, //@temp? used for array size overflow error
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ty);

    //@usize semantics not finalized yet
    // assigning usize type to constant int, since it represents size
    //@review source range for this type_size error 10.05.24
    let sizeof_expr = match type_size(hir, emit, ty, hir.src(proc.origin(), expr_range)) {
        Some(size) => emit.arena.alloc(hir::Expr::LitInt {
            val: size.size(),
            ty: BasicType::Usize,
        }),
        None => emit.arena.alloc(hir::Expr::Error),
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
        ValueID::Proc(proc_id) => {
            let data = hir.registry().proc_data(proc_id);

            let mut param_types = Vec::with_capacity(data.params.len());
            for param in data.params {
                param_types.push(param.ty);
            }
            let proc_ty = hir::ProcType {
                params: emit.arena.alloc_slice(&param_types),
                return_ty: data.return_ty,
                is_variadic: data.is_variadic,
            };

            return TypeResult::new(
                hir::Type::Procedure(emit.arena.alloc(proc_ty)), //@create proc type?
                emit.arena.alloc(hir::Expr::Procedure { proc_id }),
            );
        }
        ValueID::Enum(enum_id, variant_id) => {
            let enum_variant = hir::Expr::EnumVariant {
                enum_id,
                variant_id,
            };
            return TypeResult::new(hir::Type::Enum(enum_id), emit.arena.alloc(enum_variant));
        }
        ValueID::Const(id) => TypeResult::new(
            hir.registry().const_data(id).ty,
            emit.arena.alloc(hir::Expr::ConstVar { const_id: id }),
        ),
        ValueID::Global(id) => TypeResult::new(
            hir.registry().global_data(id).ty,
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

    //@everything below is copy-paste from regular typecheck field access 16.05.24
    // de-duplicate later
    let mut target = item_res.expr;
    let mut target_ty = item_res.ty;

    for &name in field_names {
        let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_ty, name);

        match kind {
            FieldKind::Error => return TypeResult::new(hir::Type::Error, target),
            FieldKind::Member(union_id, member_id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::UnionMember {
                    target,
                    union_id,
                    member_id,
                    deref,
                });
            }
            FieldKind::Field(struct_id, field_id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::StructField {
                    target,
                    struct_id,
                    field_id,
                    deref,
                });
            }
            FieldKind::Slice { first_ptr } => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::SliceField {
                    target,
                    first_ptr,
                    deref,
                });
            }
        }
    }

    TypeResult::new(target_ty, target)
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
            let data = hir.registry().union_data(union_id);

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
                    emit.error(ErrorComp::error(
                        format!("field `{}` is not found", hir.name_str(first.name.id),),
                        hir.src(proc.origin(), first.name.range),
                        ErrorComp::info(
                            "union defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, first.expr);

                    TypeResult::new(
                        hir::Type::Union(union_id),
                        emit.arena.alloc(hir::Expr::Error),
                    )
                };

                for input in other {
                    emit.error(ErrorComp::error(
                        "union initializer must have exactly one field",
                        hir.src(proc.origin(), input.name.range),
                        None,
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }

                type_res
            } else {
                emit.error(ErrorComp::error(
                    "union initializer must have exactly one field",
                    hir.src(proc.origin(), structure_name.range),
                    None,
                ));

                TypeResult::new(
                    hir::Type::Union(union_id),
                    emit.arena.alloc(hir::Expr::Error),
                )
            }
        }
        StructureID::Struct(struct_id) => {
            let data = hir.registry().struct_data(struct_id);
            let field_count = data.fields.len();

            enum FieldStatus {
                None,
                Init(TextRange),
            }

            //@potentially a lot of allocations (simple solution), same memory could be re-used
            let mut field_inits = Vec::<hir::StructFieldInit>::with_capacity(field_count);
            let mut field_status = Vec::<FieldStatus>::new();
            field_status.resize_with(field_count, || FieldStatus::None);
            let mut init_count: usize = 0;

            for input in struct_init.input {
                if let Some((field_id, field)) = data.find_field(input.name.id) {
                    let input_res = typecheck_expr(hir, emit, proc, field.ty, input.expr);

                    if let FieldStatus::Init(range) = field_status[field_id.index()] {
                        emit.error(ErrorComp::error(
                            format!(
                                "field `{}` was already initialized",
                                hir.name_str(input.name.id),
                            ),
                            hir.src(proc.origin(), input.name.range),
                            ErrorComp::info("initialized here", hir.src(data.origin_id, range)),
                        ));
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
                    emit.error(ErrorComp::error(
                        format!("field `{}` is not found", hir.name_str(input.name.id),),
                        hir.src(proc.origin(), input.name.range),
                        ErrorComp::info(
                            "struct defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }
            }

            if init_count < field_count {
                let mut message = "missing field initializers: ".to_string();

                for (idx, status) in field_status.iter().enumerate() {
                    if let FieldStatus::None = status {
                        let field = data.field(hir::StructFieldID::new(idx));
                        message.push('`');
                        message.push_str(hir.name_str(field.name.id));
                        if idx + 1 != field_count {
                            message.push_str("`, ");
                        } else {
                            message.push('`');
                        }
                    }
                }

                emit.error(ErrorComp::error(
                    message,
                    hir.src(proc.origin(), structure_name.range),
                    ErrorComp::info(
                        "struct defined here",
                        hir.src(data.origin_id, data.name.range),
                    ),
                ));
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
        hir::Type::ArrayStatic(array) => array.elem_ty,
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

    let array_type: &hir::ArrayStatic = emit.arena.alloc(hir::ArrayStatic {
        len: hir::ArrayStaticLen::Immediate(Some(input.len() as u64)),
        elem_ty,
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
    len: ast::ConstExpr,
) -> TypeResult<'hir> {
    let expect = match expect {
        hir::Type::ArrayStatic(array) => array.elem_ty,
        _ => hir::Type::Error,
    };

    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);

    //@this is duplicated here and in pass_3::type_resolve 09.05.24
    let (value, _) = pass_4::resolve_const_expr(
        hir,
        emit,
        proc.origin(),
        hir::Type::Basic(BasicType::Usize),
        len,
    );
    let len = match value {
        hir::ConstValue::Int { val, ty, neg } => {
            if neg {
                None
            } else {
                Some(val)
            }
        }
        _ => None,
    };

    let array_type = emit.arena.alloc(hir::ArrayStatic {
        len: hir::ArrayStaticLen::Immediate(len),
        elem_ty: expr_res.ty,
    });
    let array_repeat = emit.arena.alloc(hir::ArrayRepeat {
        elem_ty: expr_res.ty,
        expr: expr_res.expr,
        len,
    });
    TypeResult::new(
        hir::Type::ArrayStatic(array_type),
        emit.arena.alloc(hir::Expr::ArrayRepeat { array_repeat }),
    )
}

fn typecheck_address<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    mutt: ast::Mut,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);
    let adressability = get_expr_addressability(hir, proc, rhs_res.expr);

    match adressability {
        Addressability::Unknown => {} //@ & to error should be also Error? 16.05.24
        Addressability::Constant => {
            emit.error(ErrorComp::error(
                "cannot get reference to a constant, you can use `global` instead",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::error(
                "cannot get reference to a slice field, slice itself cannot be modified",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::Temporary => {
            emit.error(ErrorComp::error(
                "cannot get reference to a temporary value",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::TemporaryImmutable => {
            if mutt == ast::Mut::Mutable {
                emit.error(ErrorComp::error(
                    "cannot get mutable reference to this temporary value, only immutable `&` is allowed",
                    hir.src(proc.origin(), rhs.range),
                    None,
                ));
            }
        }
        Addressability::Addressable(rhs_mutt, src) => {
            if mutt == ast::Mut::Mutable && rhs_mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::error(
                    "cannot get mutable reference to an immutable variable",
                    hir.src(proc.origin(), rhs.range),
                    ErrorComp::info("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::error(
                "addressability not implemented for this expression",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
    }

    let ref_ty = hir::Type::Reference(emit.arena.alloc(rhs_res.ty), mutt);
    let address_expr = hir::Expr::Address { rhs: rhs_res.expr };
    TypeResult::new(ref_ty, emit.arena.alloc(address_expr))
}

enum Addressability {
    Unknown,
    Constant,
    SliceField,
    Temporary,
    TemporaryImmutable,
    Addressable(ast::Mut, SourceRange),
    NotImplemented, //@temporary non crashing error 05.05.24
}

fn get_expr_addressability<'hir>(
    hir: &HirData<'hir, '_, '_>,
    proc: &ProcScope<'hir, '_>,
    expr: &'hir hir::Expr<'hir>,
) -> Addressability {
    match *expr {
        hir::Expr::Error => Addressability::Unknown,
        hir::Expr::LitNull => Addressability::Temporary,
        hir::Expr::LitBool { .. } => Addressability::Temporary,
        hir::Expr::LitInt { .. } => Addressability::Temporary,
        hir::Expr::LitFloat { .. } => Addressability::Temporary,
        hir::Expr::LitChar { .. } => Addressability::Temporary,
        hir::Expr::LitString { .. } => Addressability::Temporary,
        hir::Expr::If { if_ } => Addressability::NotImplemented, //@todo 05.05.24
        hir::Expr::Block { stmts } => Addressability::NotImplemented, //@todo 05.05.24
        hir::Expr::Match { match_ } => Addressability::NotImplemented, //@todo 05.05.24
        hir::Expr::UnionMember { target, .. } => get_expr_addressability(hir, proc, target), //@is this correct? even with deref? 05.05.24
        hir::Expr::StructField { target, .. } => get_expr_addressability(hir, proc, target), //@is this correct? even with deref? 05.05.24
        hir::Expr::SliceField { .. } => Addressability::SliceField,
        // in case of slices depends on slice type mutability itself 16.05.24
        // also might depend on any & or &mut in the way?
        hir::Expr::Index { target, .. } => get_expr_addressability(hir, proc, target), //@is this correct? even with deref? 05.05.24
        // mutability of slicing plays are role: eg:  array[2..<4][mut ..] //@cant take mutable slice from immutable slice 05.05.24
        hir::Expr::Slice { target, .. } => get_expr_addressability(hir, proc, target), //@is this correct? even with deref? 05.05.24
        hir::Expr::Cast { .. } => Addressability::Temporary,
        hir::Expr::LocalVar { local_id } => {
            let local = proc.get_local(local_id);
            Addressability::Addressable(local.mutt, hir.src(proc.origin(), local.name.range))
        }
        hir::Expr::ParamVar { param_id } => {
            let param = proc.get_param(param_id);
            Addressability::Addressable(param.mutt, hir.src(proc.origin(), param.name.range))
        }
        hir::Expr::ConstVar { .. } => Addressability::Constant,
        hir::Expr::GlobalVar { global_id } => {
            let data = hir.registry().global_data(global_id);
            Addressability::Addressable(data.mutt, hir.src(data.origin_id, data.name.range))
        }
        hir::Expr::Procedure { .. } => Addressability::Temporary,
        hir::Expr::CallDirect { .. } => Addressability::NotImplemented, //@temporary value, but might be slice-able? 05.05.24
        hir::Expr::CallIndirect { .. } => Addressability::NotImplemented, //@temporary value, but might be slice-able? 05.05.24
        hir::Expr::EnumVariant { .. } => Addressability::Temporary,
        hir::Expr::UnionInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::StructInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayRepeat { .. } => Addressability::TemporaryImmutable,
        hir::Expr::Address { .. } => Addressability::Temporary,
        hir::Expr::Unary { op, .. } => match op {
            ast::UnOp::Deref => Addressability::NotImplemented, //@todo 05.05.24
            _ => Addressability::Temporary,
        },
        hir::Expr::Binary { op, .. } => match op {
            ast::BinOp::Range | ast::BinOp::RangeInc => Addressability::TemporaryImmutable,
            _ => Addressability::Temporary,
        },
    }
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
    };

    if let hir::Type::Error = unary_ty {
        //@unary op &str is same as token.to_str()
        // but those are separate types, this could be de-duplicated
        let op_str = match op {
            ast::UnOp::Neg => "-",
            ast::UnOp::BitNot => "~",
            ast::UnOp::LogicNot => "!",
            ast::UnOp::Deref => "*",
        };
        emit.error(ErrorComp::error(
            format!(
                "unary operator `{op_str}` cannot be applied to `{}`",
                type_format(hir, emit, rhs_res.ty)
            ),
            hir.src(proc.origin(), rhs.range),
            None,
        ));
    }

    let unary_expr = hir::Expr::Unary {
        op,
        rhs: rhs_res.expr,
    };
    TypeResult::new(unary_ty, emit.arena.alloc(unary_expr))
}

// @26.04.24
// operator incompatability messages are bad
// no as_str() for bin_op is available, only for tokens they come from
// add range for un_op bin_op and assign_op in the ast?
// will allow for more precise and clear messages, while using more mem

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

            let binary_ty = if check_type_allow_math_ops(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use math operator on value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Rem => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if check_type_allow_remainder(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use remainder operator on value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::BitAnd | ast::BinOp::BitOr | ast::BinOp::BitXor => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if check_type_allow_and_or_xor(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use bit-wise operator on value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::BitShl | ast::BinOp::BitShr => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            // this expectation should aim to be integer of same size @26.04.24
            // but passing lhs_res.ty would result in hard error, eg: i32 >> u32
            // passing hir::Type::Error will turn literals into default i32 values which isnt always expected
            let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);

            // how to threat arch dependant sizes? @26.04.24
            // during build we know which size we want
            // during check no such info is available
            // same question about folding the sizeof()
            // size only known when target arch is known

            if let (hir::Type::Basic(lhs_basic), hir::Type::Basic(rhs_basic)) =
                (lhs_res.ty, rhs_res.ty)
            {
                // checks that integers types have same size
                if matches!(
                    BasicTypeKind::from_basic(lhs_basic),
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
                ) && matches!(
                    BasicTypeKind::from_basic(rhs_basic),
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
                ) {
                    let lhs_size = basic_type_size(lhs_basic).size();
                    let rhs_size = basic_type_size(rhs_basic).size();
                    if lhs_size != rhs_size {
                        emit.error(ErrorComp::error(
                                format!(
                                    "cannot use bit-shift operator on integers of different sizes\n`{}` has bitwidth of {}, `{}` has bitwidth of {} ",
                                    type_format(hir, emit, lhs_res.ty),
                                    lhs_size * 8,
                                    type_format(hir, emit, rhs_res.ty),
                                    rhs_size * 8,
                                ),
                                hir.src(proc.origin(), lhs.range),
                                None,
                            ));
                    }
                }
            }

            let binary_ty = if check_type_allow_shl_shr(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use bit-shift operator on value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::IsEq | ast::BinOp::NotEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_eq(lhs_res.ty) {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot compare equality for value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
            }

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Less | ast::BinOp::LessEq | ast::BinOp::Greater | ast::BinOp::GreaterEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_ord(lhs_res.ty) {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot compare order for value of type `{}`",
                        type_format(hir, emit, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
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

    let lhs_signed_int = match binary_ty {
        hir::Type::Basic(basic) => {
            matches!(BasicTypeKind::from_basic(basic), BasicTypeKind::SignedInt)
        }
        _ => false,
    };
    let binary_expr = hir::Expr::Binary {
        op,
        lhs: lhs_expr,
        rhs: rhs_expr,
        lhs_signed_int,
    };
    TypeResult::new(binary_ty, emit.arena.alloc(binary_expr))
}

fn check_type_allow_math_ops(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float
        ),
        _ => false,
    }
}

fn check_type_allow_remainder(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_and_or_xor(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_shl_shr(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_compare_eq(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt
                | BasicTypeKind::UnsignedInt
                | BasicTypeKind::Float
                | BasicTypeKind::Bool
                | BasicTypeKind::Char
                | BasicTypeKind::Rawptr
        ),
        _ => false,
    }
}

fn check_type_allow_compare_ord(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float
        ),
        _ => false,
    }
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    block: ast::Block<'_>,
    enter_loop: bool,
    enter_defer: Option<TextRange>,
) -> TypeResult<'hir> {
    proc.push_block(enter_loop, enter_defer);

    let mut block_ty = None;
    let mut hir_stmts = Vec::with_capacity(block.stmts.len());

    for stmt in block.stmts {
        let hir_stmt = match stmt.kind {
            ast::StmtKind::Break => Some(typecheck_break(hir, emit, proc, stmt.range)),
            ast::StmtKind::Continue => Some(typecheck_continue(hir, emit, proc, stmt.range)),
            ast::StmtKind::Return(expr) => {
                Some(typecheck_return(hir, emit, proc, stmt.range, expr))
            }
            ast::StmtKind::Defer(block) => {
                Some(typecheck_defer(hir, emit, proc, stmt.range.start(), *block))
            }
            ast::StmtKind::ForLoop(for_) => {
                // @alloca in loops should be done outside of a loop @13.04.24
                // to not cause memory issues and preserve the stack space?
                // research the topic more
                match for_.kind {
                    ast::ForKind::Loop => {
                        let block_res = typecheck_block(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Void),
                            for_.block,
                            true,
                            None,
                        );

                        let for_ = hir::For {
                            kind: hir::ForKind::Loop,
                            block: block_res.expr,
                        };
                        Some(hir::Stmt::ForLoop(emit.arena.alloc(for_)))
                    }
                    ast::ForKind::While { cond } => {
                        let cond_res = typecheck_expr(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Bool),
                            cond,
                        );
                        let block_res = typecheck_block(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Void),
                            for_.block,
                            true,
                            None,
                        );

                        let for_ = hir::For {
                            kind: hir::ForKind::While {
                                cond: cond_res.expr,
                            },
                            block: block_res.expr,
                        };
                        Some(hir::Stmt::ForLoop(emit.arena.alloc(for_)))
                    }
                    ast::ForKind::ForLoop {
                        local,
                        cond,
                        assign,
                    } => todo!("for_loop c-like is not supported"),
                }
            }
            ast::StmtKind::Local(local) => typecheck_local(hir, emit, proc, local),
            ast::StmtKind::Assign(assign) => Some(typecheck_assign(hir, emit, proc, assign)),
            ast::StmtKind::ExprSemi(expr) => {
                let expect = if matches!(expr.kind, ast::ExprKind::Block { .. }) {
                    hir::Type::Basic(BasicType::Void)
                } else {
                    hir::Type::Error
                };
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                Some(hir::Stmt::ExprSemi(expr_res.expr))
            }
            ast::StmtKind::ExprTail(expr) => {
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                if block_ty.is_none() {
                    block_ty = Some(expr_res.ty);
                }
                Some(hir::Stmt::ExprTail(expr_res.expr))
            }
        };
        if let Some(hir_stmt) = hir_stmt {
            hir_stmts.push(hir_stmt);
        }
    }

    proc.pop_block();

    let stmts = emit.arena.alloc_slice(&hir_stmts);
    let block_expr = emit.arena.alloc(hir::Expr::Block { stmts });

    if let Some(block_ty) = block_ty {
        TypeResult::new_ignore_typecheck(block_ty, block_expr)
    } else {
        TypeResult::new(hir::Type::Basic(BasicType::Void), block_expr)
    }
}

fn typecheck_break<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.is_inside_loop() {
        emit.error(ErrorComp::error(
            "cannot use `break` outside of a loop",
            hir.src(proc.origin(), range),
            None,
        ));
    }

    hir::Stmt::Break
}

fn typecheck_continue<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.is_inside_loop() {
        emit.error(ErrorComp::error(
            "cannot use `continue` outside of a loop",
            hir.src(proc.origin(), range),
            None,
        ));
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

    if let Some(prev_defer) = proc.is_inside_defer() {
        emit.error(ErrorComp::error(
            "cannot use `return` inside `defer`",
            hir.src(proc.origin(), range),
            ErrorComp::info("in this defer", hir.src(proc.origin(), prev_defer)),
        ));
    }

    if let Some(expr) = expr {
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        hir::Stmt::Return(Some(expr_res.expr))
    } else {
        //@this is modified duplicated typecheck error, special case for empty return @07.04.24
        let found = hir::Type::Basic(BasicType::Void);
        if !type_matches(hir, emit, expect, found) {
            let expect_format = type_format(hir, emit, expect);
            let found_format = type_format(hir, emit, found);
            emit.error(ErrorComp::error(
                format!(
                    "type mismatch: expected `{}`, found `{}`",
                    expect_format, found_format,
                ),
                hir.src(proc.origin(), range),
                ErrorComp::info(
                    format!("procedure returns `{expect_format}`"),
                    hir.src(proc.origin(), proc.data().name.range),
                ),
            ));
        }
        hir::Stmt::Return(None)
    }
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_defer<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    block: ast::Block<'_>,
) -> hir::Stmt<'hir> {
    let defer_range = TextRange::new(start, start + 5.into());

    if let Some(prev_defer) = proc.is_inside_defer() {
        emit.error(ErrorComp::error(
            "`defer` statements cannot be nested",
            hir.src(proc.origin(), defer_range),
            ErrorComp::info("already in this defer", hir.src(proc.origin(), prev_defer)),
        ));
    }

    let block_res = typecheck_block(
        hir,
        emit,
        proc,
        hir::Type::Basic(BasicType::Void),
        block,
        false,
        Some(defer_range),
    );
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
    let adressability = get_expr_addressability(hir, proc, lhs_res.expr);

    match adressability {
        Addressability::Unknown => {}
        Addressability::Constant => {
            emit.error(ErrorComp::error(
                "cannot assign to a constant",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::error(
                "cannot assign to a slice field, slice itself cannot be modified",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Temporary | Addressability::TemporaryImmutable => {
            emit.error(ErrorComp::error(
                "cannot assign to a temporary value",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Addressable(mutt, src) => {
            if mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::error(
                    "cannot assign to an immutable variable",
                    hir.src(proc.origin(), assign.lhs.range),
                    ErrorComp::info("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::error(
                "addressability not implemented for this expression",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
    }

    let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, assign.rhs);

    //@binary assignment ops not checked
    let lhs_signed_int = match lhs_res.ty {
        hir::Type::Basic(basic) => {
            matches!(BasicTypeKind::from_basic(basic), BasicTypeKind::SignedInt)
        }
        _ => false,
    };
    let assign = hir::Assign {
        op: assign.op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_ty: lhs_res.ty,
        lhs_signed_int,
    };
    hir::Stmt::Assign(emit.arena.alloc(assign))
}

// these calls are only done for items so far @26.04.24
// locals or input params etc not checked yet (need to find a reasonable strategy)
// proc return type is allowed to be never or void unlike other instances where types are used
pub fn require_value_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    ty: hir::Type,
    source: SourceRange,
) {
    if !type_is_value_type(ty) {
        emit.error(ErrorComp::error(
            format!(
                "expected value type, found `{}`",
                type_format(hir, emit, ty)
            ),
            source,
            None,
        ))
    }
}

pub fn type_is_value_type(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => !matches!(basic, BasicType::Void | BasicType::Never),
        hir::Type::Enum(_) => true,
        hir::Type::Union(_) => true,
        hir::Type::Struct(_) => true,
        hir::Type::Reference(ref_ty, _) => type_is_value_type(*ref_ty),
        hir::Type::Procedure(_) => true,
        hir::Type::ArraySlice(slice) => type_is_value_type(slice.elem_ty),
        hir::Type::ArrayStatic(array) => type_is_value_type(array.elem_ty),
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
    origin_id: hir::ModuleID,
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
        None => (ResolvedPath::None, 1),
    }
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let ty = match resolved {
        ResolvedPath::None => return hir::Type::Error,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::error(
                format!("expected type, found local `{}`", hir.name_str(name.id)),
                hir.src(origin_id, name.range),
                ErrorComp::info("defined here", source),
            ));
            return hir::Type::Error;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Union(id) => hir::Type::Union(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected type, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return hir::Type::Error;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::error(
                "unexpected path segment",
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    ty
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
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> StructureID {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let structure_id = match resolved {
        ResolvedPath::None => return StructureID::None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::error(
                format!(
                    "expected struct or union, found local `{}`",
                    hir.name_str(name.id)
                ),
                hir.src(origin_id, name.range),
                ErrorComp::info("defined here", source),
            ));
            return StructureID::None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Union(id) => StructureID::Union(id),
            SymbolKind::Struct(id) => StructureID::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected struct or union, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return StructureID::None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::error(
                "unexpected path segment",
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    structure_id
}

enum ValueID {
    None,
    Proc(hir::ProcID),
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
    origin_id: hir::ModuleID,
    path: &'ast ast::Path<'ast>,
) -> (ValueID, &'ast [ast::Name]) {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let value_id = match resolved {
        ResolvedPath::None => return (ValueID::None, &[]),
        ResolvedPath::Variable(var) => match var {
            VariableID::Local(id) => ValueID::Local(id),
            VariableID::Param(id) => ValueID::Param(id),
        },
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Proc(id) => {
                if let Some(remaining) = path.names.get(name_idx + 1..) {
                    if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
                        let range = TextRange::new(first.range.start(), last.range.end());
                        emit.error(ErrorComp::error(
                            "unexpected path segment",
                            hir.src(origin_id, range),
                            None,
                        ));
                    }
                }
                ValueID::Proc(id)
            }
            SymbolKind::Enum(id) => {
                if let Some(variant_name) = path.names.get(name_idx + 1) {
                    let enum_data = hir.registry().enum_data(id);
                    if let Some((variant_id, ..)) = enum_data.find_variant(variant_name.id) {
                        if let Some(remaining) = path.names.get(name_idx + 2..) {
                            if let (Some(first), Some(last)) = (remaining.first(), remaining.last())
                            {
                                let range = TextRange::new(first.range.start(), last.range.end());
                                emit.error(ErrorComp::error(
                                    "unexpected path segment",
                                    hir.src(origin_id, range),
                                    None,
                                ));
                            }
                        }
                        return (ValueID::Enum(id, variant_id), &[]);
                    } else {
                        emit.error(ErrorComp::error(
                            format!(
                                "enum variant `{}` is not found",
                                hir.name_str(variant_name.id)
                            ),
                            hir.src(origin_id, variant_name.range),
                            ErrorComp::info("enum defined here", source),
                        ));
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    emit.error(ErrorComp::error(
                        format!(
                            "expected value, found {} `{}`",
                            HirData::symbol_kind_name(kind),
                            hir.name_str(name.id)
                        ),
                        hir.src(origin_id, name.range),
                        ErrorComp::info("defined here", source),
                    ));
                    return (ValueID::None, &[]);
                }
            }
            SymbolKind::Const(id) => ValueID::Const(id),
            SymbolKind::Global(id) => ValueID::Global(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected value, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

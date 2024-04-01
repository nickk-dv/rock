use super::hir_build::{HirData, HirEmit, SymbolKind};
use super::proc_scope::{ProcScope, VariableID};
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};

pub fn run<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.proc_ids() {
        typecheck_proc(hir, emit, id)
    }
}

fn typecheck_proc<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>, id: hir::ProcID) {
    let item = hir.proc_ast(id);

    match item.block {
        Some(block) => {
            let data = hir.proc_data(id);
            let proc = &mut ProcScope::new(data);

            let block_res = typecheck_expr(hir, emit, proc, data.return_ty, block);

            let data = hir.proc_data_mut(id);
            data.block = Some(block_res.expr);
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

fn type_format<'hir>(hir: &HirData<'hir, '_>, ty: hir::Type<'hir>) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
            ast::BasicType::Unit => "()".into(),
            ast::BasicType::Bool => "bool".into(),
            ast::BasicType::S8 => "s8".into(),
            ast::BasicType::S16 => "s16".into(),
            ast::BasicType::S32 => "s32".into(),
            ast::BasicType::S64 => "s64".into(),
            ast::BasicType::Ssize => "ssize".into(),
            ast::BasicType::U8 => "u8".into(),
            ast::BasicType::U16 => "u16".into(),
            ast::BasicType::U32 => "u32".into(),
            ast::BasicType::U64 => "u64".into(),
            ast::BasicType::Usize => "usize".into(),
            ast::BasicType::F32 => "f32".into(),
            ast::BasicType::F64 => "f64".into(),
            ast::BasicType::Char => "char".into(),
            ast::BasicType::Rawptr => "rawptr".into(),
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
    hir: &HirData<'hir, '_>,
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
        ast::ExprKind::LitString { id } => typecheck_lit_string(emit, id),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_),
        ast::ExprKind::Block { stmts } => typecheck_block(hir, emit, proc, expect, stmts),
        ast::ExprKind::Match { match_ } => typecheck_match(hir, emit, proc, expect, match_),
        ast::ExprKind::Field { target, name } => typecheck_field(hir, emit, proc, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(hir, emit, proc, target, index),
        ast::ExprKind::Cast { target, ty } => {
            typecheck_cast(hir, emit, proc, target, ty, expr.range)
        }
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, expr.range.start(), ty),
        ast::ExprKind::Item { path } => typecheck_item(hir, emit, proc, path),
        ast::ExprKind::ProcCall { proc_call } => {
            typecheck_proc_call(hir, emit, proc, proc_call, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init, expr.range)
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
        hir::Type::Basic(ast::BasicType::Unit),
        emit.arena.alloc(hir::Expr::Unit),
    )
}

fn typecheck_lit_null<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Rawptr),
        emit.arena.alloc(hir::Expr::LitNull),
    )
}

fn typecheck_lit_bool<'hir>(emit: &mut HirEmit<'hir>, val: bool) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Bool),
        emit.arena.alloc(hir::Expr::LitBool { val }),
    )
}

fn typecheck_lit_int<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: hir::Type<'hir>,
    val: u64,
) -> TypeResult<'hir> {
    const DEFAULT_INT_TYPE: ast::BasicType = ast::BasicType::S32;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            ast::BasicType::S8
            | ast::BasicType::S16
            | ast::BasicType::S32
            | ast::BasicType::S64
            | ast::BasicType::Ssize
            | ast::BasicType::U8
            | ast::BasicType::U16
            | ast::BasicType::U32
            | ast::BasicType::U64
            | ast::BasicType::Usize => basic,
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
    const DEFAULT_FLOAT_TYPE: ast::BasicType = ast::BasicType::F64;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            ast::BasicType::F32 | ast::BasicType::F64 => basic,
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
        hir::Type::Basic(ast::BasicType::Char),
        emit.arena.alloc(hir::Expr::LitChar { val }),
    )
}

fn typecheck_lit_string<'hir>(emit: &mut HirEmit<'hir>, id: InternID) -> TypeResult<'hir> {
    let slice = emit.arena.alloc(hir::ArraySlice {
        mutt: ast::Mut::Immutable,
        ty: hir::Type::Basic(ast::BasicType::U8),
    });
    TypeResult::new(
        hir::Type::ArraySlice(slice),
        emit.arena.alloc(hir::Expr::LitString { id }),
    )
}

fn typecheck_if<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    if_: &ast::If<'_>,
) -> TypeResult<'hir> {
    let has_fallback = if_.fallback.is_some();

    let entry = if_.entry;
    let _ = typecheck_expr(
        hir,
        emit,
        proc,
        hir::Type::Basic(ast::BasicType::Bool),
        entry.cond,
    );
    let _ = typecheck_expr(hir, emit, proc, expect, entry.block);

    for &branch in if_.branches {
        let _ = typecheck_expr(
            hir,
            emit,
            proc,
            hir::Type::Basic(ast::BasicType::Bool),
            branch.cond,
        );
        let _ = typecheck_expr(hir, emit, proc, expect, branch.block);
    }

    if let Some(block) = if_.fallback {
        let _ = typecheck_expr(hir, emit, proc, expect, block);
    }

    TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
}

fn typecheck_match<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    match_: &ast::Match<'_>,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr(hir, emit, proc, hir::Type::Error, match_.on_expr);
    for arm in match_.arms {
        if let Some(pat) = arm.pat {
            let pat_res = typecheck_expr(hir, emit, proc, on_res.ty, pat);
        }
        //@check match arm expr
    }
    TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
}

fn typecheck_field<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);

    let (field_ty, kind) = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_type_field(hir, emit, proc, *ref_ty, name),
        _ => verify_type_field(hir, emit, proc, target_res.ty, name),
    };

    match kind {
        FieldExprKind::None => {
            TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
        }
        FieldExprKind::Member(id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::UnionMember {
                target: target_res.expr,
                id,
            }),
        ),
        FieldExprKind::Field(id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::StructField {
                target: target_res.expr,
                id,
            }),
        ),
    }
}

enum FieldExprKind {
    None,
    Member(hir::UnionMemberID),
    Field(hir::StructFieldID),
}

fn verify_type_field<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldExprKind) {
    match ty {
        hir::Type::Error => (hir::Type::Error, FieldExprKind::None),
        hir::Type::Union(id) => {
            let data = hir.union_data(id);
            let find = data.members.iter().enumerate().find_map(|(id, member)| {
                (member.name.id == name.id).then(|| (hir::UnionMemberID::new(id), member))
            });
            match find {
                Some((id, member)) => (member.ty, FieldExprKind::Member(id)),
                _ => {
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
        }
        hir::Type::Struct(id) => {
            let data = hir.struct_data(id);
            let find = data.fields.iter().enumerate().find_map(|(id, field)| {
                (field.name.id == name.id).then(|| (hir::StructFieldID::new(id), field))
            });
            match find {
                Some((id, field)) => (field.ty, FieldExprKind::Field(id)),
                _ => {
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
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    index: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let index_res = typecheck_expr(
        hir,
        emit,
        proc,
        hir::Type::Basic(ast::BasicType::Usize),
        index,
    );

    let elem_ty = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_elem_type(*ref_ty),
        _ => verify_elem_type(target_res.ty),
    };

    match elem_ty {
        Some(it) => {
            let hir_expr = emit.arena.alloc(hir::Expr::Index {
                target: target_res.expr,
                index: index_res.expr,
            });
            TypeResult::new(it, hir_expr)
        }
        None => {
            let ty_format = type_format(hir, target_res.ty);
            emit.error(
                ErrorComp::error(format!("cannot index value of type {}", ty_format))
                    .context(hir.src(proc.origin(), index.range)),
            );
            TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
        }
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

fn typecheck_cast<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    ty: &ast::Type<'_>,
    cast_range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let cast_ty = super::pass_3::type_resolve(hir, emit, proc.origin(), *ty);

    match (target_res.ty, cast_ty) {
        (hir::Type::Error, ..) => {}
        (.., hir::Type::Error) => {}
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            //@verify that from into pair is valid
            // determine type of the cast, according to llvm, e.g: fp_trunc, fp_to_int etc.
        }
        _ => {
            let from_format = type_format(hir, target_res.ty);
            let into_format = type_format(hir, cast_ty);
            emit.error(
                ErrorComp::error(format!(
                    "non privitive cast from `{from_format}` into `{into_format}`",
                ))
                .context(hir.src(proc.origin(), cast_range)),
            );
        }
    }

    let hir_ty = emit.arena.alloc(cast_ty);
    TypeResult {
        ty: cast_ty,
        expr: emit.arena.alloc(hir::Expr::Cast {
            target: target_res.expr,
            ty: hir_ty,
        }),
    }
}

//@type-sizing not done:
// is complicated due to constant dependency graphs,
// recursive types also not detected yet.
fn typecheck_sizeof<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    ty: ast::Type,
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ty);

    let size = match ty {
        hir::Type::Basic(basic) => {
            let size: u64 = match basic {
                ast::BasicType::Unit => 0,
                ast::BasicType::Bool => 1,
                ast::BasicType::S8 => 1,
                ast::BasicType::S16 => 2,
                ast::BasicType::S32 => 4,
                ast::BasicType::S64 => 8,
                ast::BasicType::Ssize => 8, //@assuming 64bit target
                ast::BasicType::U8 => 1,
                ast::BasicType::U16 => 2,
                ast::BasicType::U32 => 4,
                ast::BasicType::U64 => 8,
                ast::BasicType::Usize => 8, //@assuming 64bit target
                ast::BasicType::F32 => 4,
                ast::BasicType::F64 => 8,
                ast::BasicType::Char => 4,
                ast::BasicType::Rawptr => 8, //@assuming 64bit target
            };
            Some(size)
        }
        hir::Type::Reference(..) => Some(8), //@assuming 64bit target
        hir::Type::ArraySlice(..) => Some(16), //@assuming 64bit target
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
    let hir_expr = if let Some(size) = size {
        emit.arena.alloc(hir::Expr::LitInt {
            val: size,
            ty: ast::BasicType::Usize,
        })
    } else {
        emit.arena.alloc(hir::Expr::Error)
    };

    TypeResult::new(hir::Type::Basic(ast::BasicType::Usize), hir_expr)
}

fn typecheck_item<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    path: &ast::Path,
) -> TypeResult<'hir> {
    let value_id = path_resolve_value(hir, emit, Some(proc), proc.origin(), path);

    match value_id {
        ValueID::None => {
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
        ValueID::Enum(id) => {
            //@resolve variant, based on remaining path names
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
        ValueID::Const(id) => {
            //@resolve potential field access chain
            return TypeResult::new(
                hir.const_data(id).ty,
                emit.arena.alloc(hir::Expr::ConstVar { const_id: id }),
            );
        }
        ValueID::Global(id) => {
            //@resolve potential field access chain
            return TypeResult::new(
                hir.global_data(id).ty,
                emit.arena.alloc(hir::Expr::GlobalVar { global_id: id }),
            );
        }
        ValueID::Local(id) => {
            //@resolve potential field access chain
            return TypeResult::new(
                proc.get_local(id).ty,
                emit.arena.alloc(hir::Expr::LocalVar { local_id: id }),
            );
        }
        ValueID::Param(id) => {
            //@resolve potential field access chain
            return TypeResult::new(
                proc.get_param(id).ty,
                emit.arena.alloc(hir::Expr::ParamVar { param_id: id }),
            );
        }
    }
}

fn typecheck_proc_call<'hir>(
    hir: &HirData<'hir, '_>,
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

    for (idx, &expr) in proc_call.input.iter().enumerate() {
        let data = hir.proc_data(proc_id);
        let param = data.params.get(idx);

        let expect = match param {
            Some(param) => param.ty,
            None => hir::Type::Error,
        };
        let _ = typecheck_expr(hir, emit, proc, expect, expr);
    }

    //@getting proc data multiple times due to mutable error reporting
    // maybe pass error context to all functions isntead
    let input_count = proc_call.input.len();
    let expected_count = hir.proc_data(proc_id).params.len();

    if input_count != expected_count {
        let data = hir.proc_data(proc_id);
        emit.error(
            ErrorComp::error("unexpected number of input arguments")
                .context(hir.src(proc.origin(), expr_range))
                .context_info(
                    "calling this procedure",
                    hir.src(data.origin_id, data.name.range),
                ),
        );
    }

    let data = hir.proc_data(proc_id);
    TypeResult::new(data.return_ty, emit.arena.alloc(hir::Expr::Error))
}

fn typecheck_struct_init<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    struct_init: &ast::StructInit<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let struct_id =
        match path_resolve_structure(hir, emit, Some(proc), proc.origin(), struct_init.path) {
            StructureID::Struct(id) => id,
            _ => {
                //@union case is ignored for now
                return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
            }
        };

    for (idx, &expr) in struct_init.input.iter().enumerate() {
        let data = hir.struct_data(struct_id);
        let field = data.fields.get(idx);

        let expect_ty = match field {
            Some(field) => field.ty,
            None => hir::Type::Error,
        };
        //@resolve expr & field name or name as expr
        //let _ = typecheck_expr_2(hb, origin_id, block_flags, proc_scope, expect_ty, expr);
    }

    //@getting proc data multiple times due to mutable error reporting
    // maybe pass error context to all functions isntead
    let input_count = struct_init.input.len();
    let expected_count = hir.struct_data(struct_id).fields.len();

    if input_count != expected_count {
        let data = hir.struct_data(struct_id);
        emit.error(
            ErrorComp::error("unexpected number of input fields")
                .context(hir.src(proc.origin(), expr_range))
                .context_info(
                    "calling this procedure",
                    hir.src(data.origin_id, data.name.range),
                ),
        );
    }

    let data = hir.struct_data(struct_id);
    TypeResult::new(
        hir::Type::Struct(struct_id),
        emit.arena.alloc(hir::Expr::Error),
    )
}

fn typecheck_array_init<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    input: &[&ast::Expr<'_>],
) -> TypeResult<'hir> {
    let mut expect_elem = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let mut first_elem = hir::Type::Error;

    let mut input_iter = input.iter().cloned();

    if let Some(expr) = input_iter.next() {
        let expr_res = typecheck_expr(hir, emit, proc, expect_elem, expr);
        first_elem = expr_res.ty;
        if !matches!(expect_elem, hir::Type::Error) {
            expect_elem = expr_res.ty;
        }
    }

    for expr in input_iter {
        let expr_res = typecheck_expr(hir, emit, proc, expect_elem, expr);
    }

    let size = emit.arena.alloc(hir::Expr::LitInt {
        val: input.len() as u64,
        ty: ast::BasicType::Ssize,
    });
    let array_ty = emit.arena.alloc(hir::ArrayStatic {
        size: hir::ConstExpr(size),
        ty: first_elem,
    });
    TypeResult::new(
        hir::Type::ArrayStatic(array_ty),
        emit.arena.alloc(hir::Expr::Error),
    )
}

fn typecheck_array_repeat<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    expr: &ast::Expr,
    size: ast::ConstExpr,
) -> TypeResult<'hir> {
    let expect_elem = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let expr_res = typecheck_expr(hir, emit, proc, expect_elem, expr);
    let size_res = super::pass_4::const_expr_resolve(hir, emit, proc.origin(), size);

    let array = emit.arena.alloc(hir::ArrayStatic {
        size: size_res,
        ty: expr_res.ty,
    });
    let hir_expr = emit.arena.alloc(hir::Expr::ArrayRepeat {
        expr: expr_res.expr,
        size: size_res,
    });
    TypeResult::new(hir::Type::ArrayStatic(array), hir_expr)
}

fn typecheck_unary<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::UnOp,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);

    if let hir::Type::Error = rhs_res.ty {
        return TypeResult::new(hir::Type::Error, rhs_res.expr);
    }

    let unary_ty = match op {
        ast::UnOp::Neg => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::S8
                | ast::BasicType::S16
                | ast::BasicType::S32
                | ast::BasicType::S64
                | ast::BasicType::Ssize
                | ast::BasicType::F32
                | ast::BasicType::F64 => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::BitNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::U8
                | ast::BasicType::U16
                | ast::BasicType::U32
                | ast::BasicType::U64
                | ast::BasicType::Usize => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::LogicNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::Bool => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::Deref => match rhs_res.ty {
            hir::Type::Reference(ref_ty, ..) => *ref_ty,
            _ => hir::Type::Error,
        },
        ast::UnOp::Addr(mutt) => hir::Type::Error, // @todo
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

    TypeResult::new(unary_ty, rhs_res.expr)
}

fn typecheck_binary<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);

    match (lhs_res.ty, rhs_res.ty) {
        (hir::Type::Error, ..) | (.., hir::Type::Error) => {
            //@allocate proper bin_expr here?
            return TypeResult::new(hir::Type::Error, lhs_res.expr);
        }
        _ => {}
    }

    /* @todo
    match op {
        ast::BinOp::Add => todo!(),
        ast::BinOp::Sub => todo!(),
        ast::BinOp::Mul => todo!(),
        ast::BinOp::Div => todo!(),
        ast::BinOp::Rem => todo!(),
        ast::BinOp::BitAnd => todo!(),
        ast::BinOp::BitOr => todo!(),
        ast::BinOp::BitXor => todo!(),
        ast::BinOp::BitShl => todo!(),
        ast::BinOp::BitShr => todo!(),
        ast::BinOp::CmpIsEq => todo!(),
        ast::BinOp::CmpNotEq => todo!(),
        ast::BinOp::CmpLt => todo!(),
        ast::BinOp::CmpLtEq => todo!(),
        ast::BinOp::CmpGt => todo!(),
        ast::BinOp::CmpGtEq => todo!(),
        ast::BinOp::LogicAnd => todo!(),
        ast::BinOp::LogicOr => todo!(),
    }
    */

    TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    stmts: &[ast::Stmt],
) -> TypeResult<'hir> {
    proc.push_block();

    let mut block_ty = None;

    for (idx, stmt) in stmts.iter().enumerate() {
        match stmt.kind {
            ast::StmtKind::Break => typecheck_break(hir, emit, proc, stmt.range),
            ast::StmtKind::Continue => typecheck_continue(hir, emit, proc, stmt.range),
            ast::StmtKind::Return(expr) => typecheck_return(hir, emit, proc, stmt.range, expr),
            ast::StmtKind::Defer(block) => {
                typecheck_defer(hir, emit, proc, stmt.range.start(), block)
            }
            ast::StmtKind::ForLoop(for_) => {}
            ast::StmtKind::Local(local) => typecheck_local(hir, emit, proc, local),
            ast::StmtKind::Assign(assign) => {}
            ast::StmtKind::ExprSemi(expr) => {
                let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
            }
            ast::StmtKind::ExprTail(expr) => {
                if idx + 1 == stmts.len() {
                    let res = typecheck_expr(hir, emit, proc, expect, expr);
                    block_ty = Some(res.ty);
                } else {
                    //@decide if semi should enforced on parsing
                    // or we just expect a unit from that no-semi expression
                    // when its not a last expression?
                    let _ = typecheck_expr(
                        hir,
                        emit,
                        proc,
                        hir::Type::Basic(ast::BasicType::Unit),
                        expr,
                    );
                }
            }
        }
    }

    proc.pop_block();

    // when type expectation was passed to tail expr
    // return Type::Error to not trigger duplicate type mismatch errors
    if block_ty.is_some() {
        TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
    } else {
        TypeResult::new(
            hir::Type::Basic(ast::BasicType::Unit),
            emit.arena.alloc(hir::Expr::Error),
        )
    }
}

fn typecheck_break<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) {
    if !proc.get_block().in_loop {
        emit.error(
            ErrorComp::error("cannot use `break` outside of a loop")
                .context(hir.src(proc.origin(), range)),
        );
    }
}

fn typecheck_continue<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) {
    if !proc.get_block().in_loop {
        emit.error(
            ErrorComp::error("cannot use `continue` outside of a loop")
                .context(hir.src(proc.origin(), range)),
        );
    }
}

fn typecheck_return<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
    expr: Option<&ast::Expr>,
) {
    if let Some(expr) = expr {
        let expect = proc.data().return_ty;
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
    } else {
        //@figure out a way to report return type related errors
        // this also applies to tail return expression in blocks
        //@does empty return need its own unique error?
    }
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_defer<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    block: &ast::Expr<'_>,
) {
    if proc.get_block().in_defer {
        emit.error(
            ErrorComp::error("`defer` statements cannot be nested")
                .context(hir.src(proc.origin(), TextRange::new(start, start + 5.into()))),
        );
    }
    let _ = typecheck_expr(
        hir,
        emit,
        proc,
        hir::Type::Basic(ast::BasicType::Unit),
        block,
    );
}

fn typecheck_local<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    local: &ast::Local,
) {
    if let Some(existing) = hir.scope_name_defined(proc.origin(), local.name.id) {
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);
        return; //@not checking type and expr in case of name duplicate (need to check)
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
        return; //@not checking type and expr in case of name duplicate (need to check)
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

    proc.push_local(local);
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
    hir: &HirData<'hir, '_>,
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

pub fn path_resolve_type<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);
    match resolved {
        ResolvedPath::None => hir::Type::Error,
        ResolvedPath::Variable(_) => hir::Type::Error, //@missing error message
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
                hir::Type::Error
            }
        },
    }
}

fn path_resolve_proc<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> Option<hir::ProcID> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);
    match resolved {
        ResolvedPath::None => None,
        ResolvedPath::Variable(_) => None, //@missing error message
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
                None
            }
        },
    }
}

enum StructureID {
    None,
    Union(hir::UnionID),
    Struct(hir::StructID),
}

fn path_resolve_structure<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> StructureID {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);
    match resolved {
        ResolvedPath::None => StructureID::None,
        ResolvedPath::Variable(_) => StructureID::None, //@missing error message
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
                StructureID::None
            }
        },
    }
}

enum ValueID {
    None,
    Enum(hir::EnumID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

fn path_resolve_value<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ScopeID,
    path: &ast::Path,
) -> ValueID {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);
    match resolved {
        ResolvedPath::None => ValueID::None,
        ResolvedPath::Variable(var) => match var {
            VariableID::Local(id) => ValueID::Local(id),
            VariableID::Param(id) => ValueID::Param(id),
        },
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => ValueID::Enum(id),
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
                ValueID::None
            }
        },
    }
}

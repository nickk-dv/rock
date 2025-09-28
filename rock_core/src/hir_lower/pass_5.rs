use super::check_directive;
use super::check_match;
use super::check_path::{self, ValueID};
use super::context::HirCtx;
use super::layout;
use super::pass_4;
use super::scope::{self, BlockStatus, Diverges, InferContext, PolyScope};
use super::types;
use crate::ast;
use crate::error::{ErrorSink, SourceRange};
use crate::errors as err;
use crate::hir::{self, BoolType, CmpPred, FloatType, IntType, StringType};
use crate::support::{AsStr, TempOffset};
use crate::text::{TextOffset, TextRange};

pub fn typecheck_procedures(ctx: &mut HirCtx) {
    for proc_id in ctx.registry.proc_ids() {
        typecheck_proc(ctx, proc_id)
    }
}

fn typecheck_proc(ctx: &mut HirCtx, proc_id: hir::ProcID) {
    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data(proc_id);
    let Some(block) = item.block else {
        return;
    };

    let expect_src = SourceRange::new(data.origin_id, item.return_ty.range);
    let expect = Expectation::HasType(data.return_ty, Some(expect_src));
    ctx.scope.origin = data.origin_id;
    ctx.scope.poly = PolyScope::Proc(proc_id);
    ctx.scope.local.reset();
    ctx.scope.local.set_proc_context(Some(proc_id), data.params, expect);

    let block_res = typecheck_block(ctx, expect, block, BlockStatus::None);
    let variables = ctx.arena.alloc_slice(ctx.scope.local.end_proc_context());

    let data = ctx.registry.proc_data_mut(proc_id);
    data.block = Some(block_res.block);
    data.variables = variables;

    for (idx, param) in data.params.iter().enumerate() {
        if !ctx.scope.local.params_was_used[idx] {
            let src = ctx.src(param.name.range);
            let name = ctx.name(param.name.id);
            err::scope_unused_symbol(&mut ctx.emit, src, name, "parameter");
        }
    }
    let discard_id = ctx.session.intern_name.intern("_");
    for var in variables {
        if !var.was_used && var.name.id != discard_id {
            let src = ctx.src(var.name.range);
            let name = ctx.name(var.name.id);
            err::scope_unused_symbol(&mut ctx.emit, src, name, "variable");
        }
    }
}

fn type_matches_coerced(ctx: &HirCtx, ty: hir::Type, ty2: hir::Type) -> bool {
    if type_matches(ctx, ty, ty2) {
        return true;
    }
    match (ty, ty2) {
        (hir::Type::Reference(mutt, ref_ty), hir::Type::MultiReference(mutt2, ref_ty2)) => {
            (mutt2 == ast::Mut::Mutable || mutt == mutt2) && type_matches(ctx, *ref_ty, *ref_ty2)
        }
        (hir::Type::MultiReference(mutt, ref_ty), hir::Type::Reference(mutt2, ref_ty2)) => {
            (mutt2 == ast::Mut::Mutable || mutt == mutt2) && type_matches(ctx, *ref_ty, *ref_ty2)
        }
        _ => false,
    }
}

fn type_matches(ctx: &HirCtx, ty: hir::Type, ty2: hir::Type) -> bool {
    if ty.is_error() || ty2.is_error() || ty.is_unknown() {
        return true;
    }

    match (ty, ty2) {
        (hir::Type::Char, hir::Type::Char) => true,
        (hir::Type::Void, hir::Type::Void) => true,
        (hir::Type::Never, hir::Type::Never) => true,
        (hir::Type::Rawptr, hir::Type::Rawptr) => true,
        (hir::Type::UntypedChar, hir::Type::UntypedChar) => true,
        (hir::Type::Int(ty), hir::Type::Int(ty2)) => ty == ty2,
        (hir::Type::Float(ty), hir::Type::Float(ty2)) => ty == ty2,
        (hir::Type::Bool(ty), hir::Type::Bool(ty2)) => ty == ty2,
        (hir::Type::String(ty), hir::Type::String(ty2)) => ty == ty2,
        (hir::Type::PolyProc(id, poly_idx), hir::Type::PolyProc(id2, poly_idx2)) => {
            id == id2 && poly_idx == poly_idx2
        }
        (hir::Type::PolyEnum(id, poly_idx), hir::Type::PolyEnum(id2, poly_idx2)) => {
            id == id2 && poly_idx == poly_idx2
        }
        (hir::Type::PolyStruct(id, poly_idx), hir::Type::PolyStruct(id2, poly_idx2)) => {
            id == id2 && poly_idx == poly_idx2
        }
        (hir::Type::Enum(id, poly_types), hir::Type::Enum(id2, poly_types2)) => {
            id == id2
                && (0..poly_types.len())
                    .all(|idx| type_matches(ctx, poly_types[idx], poly_types2[idx]))
        }
        (hir::Type::Struct(id, poly_types), hir::Type::Struct(id2, poly_types2)) => {
            id == id2
                && (0..poly_types.len())
                    .all(|idx| type_matches(ctx, poly_types[idx], poly_types2[idx]))
        }
        (hir::Type::Reference(mutt, ref_ty), hir::Type::Reference(mutt2, ref_ty2)) => {
            (mutt2 == ast::Mut::Mutable || mutt == mutt2) && type_matches(ctx, *ref_ty, *ref_ty2)
        }
        (hir::Type::MultiReference(mutt, ref_ty), hir::Type::MultiReference(mutt2, ref_ty2)) => {
            (mutt2 == ast::Mut::Mutable || mutt == mutt2) && type_matches(ctx, *ref_ty, *ref_ty2)
        }
        (hir::Type::Procedure(proc_ty), hir::Type::Procedure(proc_ty2)) => {
            proc_ty.flag_set == proc_ty2.flag_set
                && type_matches(ctx, proc_ty.return_ty, proc_ty2.return_ty)
                && (proc_ty.params.len() == proc_ty2.params.len())
                && (0..proc_ty.params.len()).all(|idx| {
                    let param = &proc_ty.params[idx];
                    let param2 = &proc_ty2.params[idx];
                    param.kind == param2.kind && type_matches(ctx, param.ty, param2.ty)
                })
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            (slice2.mutt == ast::Mut::Mutable || slice.mutt == slice2.mutt)
                && type_matches(ctx, slice.elem_ty, slice2.elem_ty)
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            if let Ok(len) = ctx.array_len(array.len) {
                if let Ok(len2) = ctx.array_len(array2.len) {
                    return (len == len2) && type_matches(ctx, array.elem_ty, array2.elem_ty);
                }
            }
            true
        }
        (hir::Type::ArrayEnumerated(array), hir::Type::ArrayEnumerated(array2)) => {
            array.enum_id == array2.enum_id && type_matches(ctx, array.elem_ty, array2.elem_ty)
        }
        //prevent arrays with error sizes from erroring
        (hir::Type::ArrayStatic(array), _) => ctx.array_len(array.len).is_err(),
        (_, hir::Type::ArrayStatic(array2)) => ctx.array_len(array2.len).is_err(),
        _ => false,
    }
}

pub fn type_format(ctx: &HirCtx, ty: hir::Type) -> std::borrow::Cow<'static, str> {
    match ty {
        hir::Type::Error => "<error>".into(),
        hir::Type::Unknown => "<unknown>".into(),
        hir::Type::Char => "char".into(),
        hir::Type::Void => "void".into(),
        hir::Type::Never => "never".into(),
        hir::Type::Rawptr => "rawptr".into(),
        hir::Type::UntypedChar => "untyped char".into(),
        hir::Type::Int(int_ty) => int_ty.as_str().into(),
        hir::Type::Float(float_ty) => float_ty.as_str().into(),
        hir::Type::Bool(bool_ty) => bool_ty.as_str().into(),
        hir::Type::String(string_ty) => string_ty.as_str().into(),
        hir::Type::PolyProc(id, poly_idx) => {
            let name = ctx.registry.proc_data(id).poly_params.unwrap()[poly_idx];
            ctx.name(name.id).to_string().into()
        }
        hir::Type::PolyEnum(id, poly_idx) => {
            let name = ctx.registry.enum_data(id).poly_params.unwrap()[poly_idx];
            ctx.name(name.id).to_string().into()
        }
        hir::Type::PolyStruct(id, poly_idx) => {
            let name = ctx.registry.struct_data(id).poly_params.unwrap()[poly_idx];
            ctx.name(name.id).to_string().into()
        }
        hir::Type::Enum(id, poly_types) => {
            let name = ctx.name(ctx.registry.enum_data(id).name.id);

            if !poly_types.is_empty() {
                let mut format = String::with_capacity(64);
                let mut first = true;
                format.push_str(name);
                format.push('(');
                for gen_type in poly_types {
                    if !first {
                        format.push(',');
                        format.push(' ');
                    }
                    first = false;
                    let gen_fmt = type_format(ctx, *gen_type);
                    format.push_str(&gen_fmt);
                }
                format.push(')');
                format.into()
            } else {
                name.to_string().into()
            }
        }
        hir::Type::Struct(id, poly_types) => {
            let name = ctx.name(ctx.registry.struct_data(id).name.id);

            if !poly_types.is_empty() {
                let mut format = String::with_capacity(64);
                let mut first = true;
                format.push_str(name);
                format.push('(');
                for gen_type in poly_types {
                    if !first {
                        format.push(',');
                        format.push(' ');
                    }
                    first = false;
                    let gen_fmt = type_format(ctx, *gen_type);
                    format.push_str(&gen_fmt);
                }
                format.push(')');
                format.into()
            } else {
                name.to_string().into()
            }
        }
        hir::Type::Reference(mutt, ref_ty) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            let ref_ty_format = type_format(ctx, *ref_ty);
            let format = format!("&{mut_str}{}", ref_ty_format);
            format.into()
        }
        hir::Type::MultiReference(mutt, ref_ty) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            let ref_ty_format = type_format(ctx, *ref_ty);
            let format = format!("[&{mut_str}]{}", ref_ty_format);
            format.into()
        }
        hir::Type::Procedure(proc_ty) => {
            let mut format = String::from("proc");
            if proc_ty.flag_set.contains(hir::ProcFlag::External) {
                format.push_str(" #c_call");
            }
            format.push('(');
            for (idx, param) in proc_ty.params.iter().enumerate() {
                match param.kind {
                    hir::ParamKind::Normal => {
                        let param_ty = type_format(ctx, param.ty);
                        format.push_str(&param_ty);
                    }
                    hir::ParamKind::Variadic => format.push_str("#variadic"),
                    hir::ParamKind::CallerLocation => format.push_str("#caller_location"),
                }
                if proc_ty.params.len() != idx + 1 {
                    format.push_str(", ");
                }
            }
            if proc_ty.flag_set.contains(hir::ProcFlag::CVariadic) {
                format.push_str(", #c_variadic");
            }
            format.push_str(") ");
            let return_ty = type_format(ctx, proc_ty.return_ty);
            format.push_str(&return_ty);
            format.into()
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            let elem_format = type_format(ctx, slice.elem_ty);
            let format = format!("[{}]{}", mut_str, elem_format);
            format.into()
        }
        hir::Type::ArrayStatic(array) => {
            let elem_format = type_format(ctx, array.elem_ty);
            let format = match ctx.array_len(array.len) {
                Ok(len) => format!("[{}]{}", len, elem_format),
                Err(()) => format!("[<unknown>]{}", elem_format),
            };
            format.into()
        }
        hir::Type::ArrayEnumerated(array) => {
            let elem_format = type_format(ctx, array.elem_ty);
            let enum_name = ctx.name(ctx.registry.enum_data(array.enum_id).name.id);
            format!("[{enum_name}]{}", elem_format).into()
        }
    }
}

#[derive(Copy, Clone)]
pub enum Expectation<'hir> {
    None,
    HasType(hir::Type<'hir>, Option<SourceRange>),
}

impl<'hir> Expectation<'hir> {
    pub const VOID: Expectation<'static> = Expectation::HasType(hir::Type::Void, None);
    pub const USIZE: Expectation<'static> =
        Expectation::HasType(hir::Type::Int(IntType::U64), None);
    pub const ERROR: Expectation<'static> = Expectation::HasType(hir::Type::Error, None);

    fn inner_type(&self) -> Option<hir::Type<'hir>> {
        match self {
            Expectation::None => None,
            Expectation::HasType(ty, _) => Some(*ty),
        }
    }
    fn infer_bool(&self) -> BoolType {
        match self {
            Expectation::HasType(hir::Type::Bool(bool_ty), _) => *bool_ty,
            _ => BoolType::Bool,
        }
    }
    fn infer_ref_ty(&self) -> Expectation<'hir> {
        match self {
            Expectation::None => Expectation::None,
            Expectation::HasType(expect_ty, expect_src) => match expect_ty {
                hir::Type::Error => Expectation::ERROR,
                hir::Type::Reference(_, ref_ty) => Expectation::HasType(**ref_ty, *expect_src),
                hir::Type::MultiReference(_, ref_ty) => Expectation::HasType(**ref_ty, *expect_src),
                _ => Expectation::None,
            },
        }
    }
    fn infer_array_elem(&self) -> Expectation<'hir> {
        match self {
            Expectation::None => Expectation::None,
            Expectation::HasType(expect_ty, expect_src) => match expect_ty {
                hir::Type::Error => Expectation::ERROR,
                hir::Type::ArrayStatic(array) => Expectation::HasType(array.elem_ty, *expect_src),
                hir::Type::ArrayEnumerated(array) => {
                    Expectation::HasType(array.elem_ty, *expect_src)
                }
                _ => Expectation::None,
            },
        }
    }
    fn infer_array_enum(&self) -> Expectation<'hir> {
        match self {
            Expectation::None => Expectation::None,
            Expectation::HasType(expect_ty, expect_src) => match expect_ty {
                hir::Type::Error => Expectation::ERROR,
                hir::Type::ArrayEnumerated(array) => {
                    Expectation::HasType(hir::Type::Enum(array.enum_id, &[]), *expect_src)
                }
                _ => Expectation::None,
            },
        }
    }
    fn infer_slicing_array_type(
        &self,
        ctx: &mut HirCtx<'hir, '_, '_>,
        target: &ast::Expr,
    ) -> Expectation<'hir> {
        match self {
            Expectation::None => Expectation::None,
            Expectation::HasType(expect_ty, expect_src) => match expect_ty {
                hir::Type::Error => Expectation::ERROR,
                hir::Type::ArraySlice(slice) => {
                    if let ast::ExprKind::ArrayInit { input } = target.kind {
                        let array_ty = hir::ArrayStatic {
                            len: hir::ArrayStaticLen::Immediate(input.len() as u64),
                            elem_ty: slice.elem_ty,
                        };
                        let array_ty = ctx.arena.alloc(array_ty);
                        Expectation::HasType(hir::Type::ArrayStatic(array_ty), *expect_src)
                    } else {
                        Expectation::None
                    }
                }
                _ => Expectation::None,
            },
        }
    }
}

fn type_substitute<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    is_poly: bool,
    poly_set: &[hir::Type<'hir>],
    ty: hir::Type<'hir>,
) -> hir::Type<'hir> {
    if !is_poly || !types::has_poly_param(ty) {
        return ty;
    }
    types::substitute(ctx, ty, poly_set, None)
}

fn type_substitute_inferred<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    is_poly: bool,
    infer: InferContext,
    ty: hir::Type<'hir>,
) -> hir::Type<'hir> {
    if !is_poly || !types::has_poly_param(ty) {
        return ty;
    }
    //borrow checker forced copy
    let poly_set = ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer));
    types::substitute(ctx, ty, poly_set, None)
}

pub fn type_expectation_check(
    ctx: &mut HirCtx,
    from_range: TextRange,
    found_ty: hir::Type,
    expect: Expectation,
) {
    let Expectation::HasType(expect_ty, expect_src) = expect else {
        return;
    };
    // `never` coerses to any type
    if found_ty.is_never() {
        return;
    }
    if type_matches_coerced(ctx, expect_ty, found_ty) {
        return;
    }
    let src = ctx.src(from_range);
    let expected_ty = type_format(ctx, expect_ty);
    let found_ty = type_format(ctx, found_ty);
    err::tycheck_type_mismatch(&mut ctx.emit, src, expect_src, &expected_ty, &found_ty);
}

fn check_expect_integer(ctx: &mut HirCtx, range: TextRange, ty: hir::Type) {
    match ty {
        hir::Type::Error | hir::Type::Int(_) => {}
        _ => {
            let src = ctx.src(range);
            let ty = type_format(ctx, ty);
            err::tycheck_expected_integer(&mut ctx.emit, src, &ty);
        }
    }
}

fn check_expect_boolean(ctx: &mut HirCtx, range: TextRange, ty: hir::Type) {
    match ty {
        hir::Type::Error | hir::Type::Bool(_) => {}
        _ => {
            let src = ctx.src(range);
            let ty = type_format(ctx, ty);
            err::tycheck_expected_boolean(&mut ctx.emit, src, &ty);
        }
    }
}

#[must_use]
pub struct TypeResult<'hir> {
    pub ty: hir::Type<'hir>,
    expr: hir::Expr<'hir>,
    ignore: bool,
}

#[derive(Clone)]
#[must_use]
pub struct ExprResult<'hir> {
    pub ty: hir::Type<'hir>,
    pub expr: &'hir hir::Expr<'hir>,
}

#[must_use]
struct PatResult<'hir> {
    pat: hir::Pat<'hir>,
    pat_ty: hir::Type<'hir>,
}

#[must_use]
struct BlockResult<'hir> {
    ty: hir::Type<'hir>,
    block: hir::Block<'hir>,
    tail_range: Option<TextRange>,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult { ty, expr, ignore: false }
    }
    fn new_ignore(ty: hir::Type<'hir>, expr: hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult { ty, expr, ignore: true }
    }
    fn error() -> TypeResult<'hir> {
        TypeResult { ty: hir::Type::Error, expr: hir::Expr::Error, ignore: true }
    }
    fn into_expr_result(self, ctx: &mut HirCtx<'hir, '_, '_>) -> ExprResult<'hir> {
        ExprResult::new(self.ty, ctx.arena.alloc(self.expr))
    }
}

impl<'hir> ExprResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> ExprResult<'hir> {
        ExprResult { ty, expr }
    }
}

impl<'hir> PatResult<'hir> {
    fn new(pat: hir::Pat<'hir>, pat_ty: hir::Type<'hir>) -> PatResult<'hir> {
        PatResult { pat, pat_ty }
    }
    fn error() -> PatResult<'hir> {
        PatResult { pat: hir::Pat::Error, pat_ty: hir::Type::Error }
    }
}

impl<'hir> BlockResult<'hir> {
    fn new(
        ty: hir::Type<'hir>,
        block: hir::Block<'hir>,
        tail_range: Option<TextRange>,
    ) -> BlockResult<'hir> {
        BlockResult { ty, block, tail_range }
    }
}

pub fn typecheck_expr<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
) -> ExprResult<'hir> {
    typecheck_expr_impl(ctx, expect, expr, true)
}

pub fn typecheck_expr_untyped<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
) -> ExprResult<'hir> {
    typecheck_expr_impl(ctx, expect, expr, false)
}

fn typecheck_expr_impl<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
    untyped_promote: bool,
) -> ExprResult<'hir> {
    let mut expr_res = match expr.kind {
        ast::ExprKind::Lit { lit } => typecheck_lit(lit),
        ast::ExprKind::If { if_ } => typecheck_if(ctx, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            let block_res = typecheck_block(ctx, expect, *block, BlockStatus::None);
            TypeResult::new_ignore(block_res.ty, hir::Expr::Block { block: block_res.block })
        }
        ast::ExprKind::Match { match_ } => typecheck_match(ctx, expect, match_, expr.range),
        ast::ExprKind::Field { target, name } => typecheck_field(ctx, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(ctx, expr.range, target, index),
        ast::ExprKind::Slice { target, range } => {
            typecheck_slice(ctx, expect, target, range, expr.range)
        }
        ast::ExprKind::Call { target, args_list } => typecheck_call(ctx, target, args_list),
        ast::ExprKind::Cast { target, into } => typecheck_cast(ctx, expr.range, target, into),
        ast::ExprKind::Item { path, args_list } => {
            typecheck_item(ctx, expect, path, args_list, expr.range)
        }
        ast::ExprKind::Variant { name, args_list } => {
            typecheck_variant(ctx, expect, name, args_list, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(ctx, expect, struct_init, expr.range)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(ctx, expect, expr.range, input),
        ast::ExprKind::ArrayRepeat { value, len } => {
            typecheck_array_repeat(ctx, expect, value, len)
        }
        ast::ExprKind::Try { expr: target } => typecheck_try(ctx, target, expr.range),
        ast::ExprKind::Deref { rhs } => typecheck_deref(ctx, rhs, expr.range),
        ast::ExprKind::Address { mutt, rhs } => {
            typecheck_address(ctx, expect, mutt, rhs, expr.range)
        }
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(ctx, expr.range, op, op_range, rhs)
        }
        ast::ExprKind::Binary { op, op_start, lhs, rhs } => {
            typecheck_binary(ctx, expect, expr.range, op, op_start, lhs, rhs)
        }
    };

    if untyped_promote {
        let with = expect.inner_type();
        if let Some(Ok((v, id))) =
            promote_untyped(ctx, expr.range, expr_res.expr, &mut expr_res.ty, with, true)
        {
            expr_res.expr = hir::Expr::Const(v, id);
        }
    }

    if !expr_res.ignore {
        type_expectation_check(ctx, expr.range, expr_res.ty, expect);
    }
    expr_res.into_expr_result(ctx)
}

fn promote_untyped<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    range: TextRange,
    expr: hir::Expr<'hir>,
    expr_ty: &mut hir::Type<'hir>,
    with: Option<hir::Type<'hir>>,
    default: bool,
) -> Option<Result<(hir::ConstValue<'hir>, hir::ConstID), ()>> {
    let (value, const_id) = match expr {
        hir::Expr::Const(value, const_id) => (value, const_id),
        _ => return None,
    };
    let src = ctx.src(range);

    let promoted = match value {
        hir::ConstValue::Null => {
            if let Some(with) = with {
                match with {
                    hir::Type::Reference(_, _)
                    | hir::Type::MultiReference(_, _)
                    | hir::Type::Procedure(_) => *expr_ty = with,
                    _ => (),
                }
            }
            return None; //ConstValue::Null remains the same
        }
        hir::ConstValue::Int { val, neg, int_ty } => {
            if int_ty != IntType::Untyped {
                return None;
            }
            match with {
                Some(hir::Type::Int(with)) if with != IntType::Untyped => {
                    *expr_ty = hir::Type::Int(with);
                    int_range_check(ctx, src, value.into_int(), with)
                }
                Some(hir::Type::Float(with)) => {
                    *expr_ty = hir::Type::Float(with);
                    let val = if neg { -(val as f64) } else { val as f64 };
                    Ok(hir::ConstValue::Float { val, float_ty: with })
                }
                _ if default => {
                    *expr_ty = hir::Type::Int(IntType::S32);
                    int_range_check(ctx, src, value.into_int(), IntType::S32)
                }
                _ => return None,
            }
        }
        hir::ConstValue::Float { val, float_ty } => {
            if float_ty != FloatType::Untyped {
                return None;
            }
            match with {
                Some(hir::Type::Float(with)) if with != FloatType::Untyped => {
                    *expr_ty = hir::Type::Float(with);
                    Ok(hir::ConstValue::Float { val, float_ty: with })
                }
                _ if default => {
                    *expr_ty = hir::Type::Float(FloatType::F64);
                    Ok(hir::ConstValue::Float { val, float_ty: FloatType::F64 })
                }
                _ => return None,
            }
        }
        hir::ConstValue::Bool { val, bool_ty } => {
            if bool_ty != BoolType::Untyped {
                return None;
            }
            match with {
                Some(hir::Type::Bool(with)) if with != BoolType::Untyped => {
                    *expr_ty = hir::Type::Bool(with);
                    Ok(hir::ConstValue::Bool { val, bool_ty: with })
                }
                _ if default => {
                    *expr_ty = hir::Type::Bool(BoolType::Bool);
                    Ok(hir::ConstValue::Bool { val, bool_ty: BoolType::Bool })
                }
                _ => return None,
            }
        }
        hir::ConstValue::Char { val, untyped } => {
            if !untyped {
                return None;
            }
            match with {
                Some(hir::Type::Char) => {
                    *expr_ty = hir::Type::Char;
                    Ok(hir::ConstValue::Char { val, untyped: false })
                }
                Some(hir::Type::Int(with)) if with != IntType::Untyped => {
                    *expr_ty = hir::Type::Int(with);
                    int_range_check(ctx, src, val as i128, with)
                }
                _ if default => {
                    *expr_ty = hir::Type::Char;
                    Ok(hir::ConstValue::Char { val, untyped: false })
                }
                _ => return None,
            }
        }
        hir::ConstValue::String { val, string_ty } => {
            if string_ty != StringType::Untyped {
                return None;
            }
            match with {
                Some(hir::Type::String(with)) if with != StringType::Untyped => {
                    *expr_ty = hir::Type::String(with);
                    Ok(hir::ConstValue::String { val, string_ty: with })
                }
                _ if default => {
                    *expr_ty = hir::Type::String(StringType::String);
                    Ok(hir::ConstValue::String { val, string_ty: StringType::String })
                }
                _ => return None,
            }
        }
        _ => return None,
    };

    Some(promoted.map(|v| (v, const_id)))
}

fn typecheck_lit<'hir>(lit: ast::Lit) -> TypeResult<'hir> {
    let (ty, value) = match lit {
        ast::Lit::Void => (hir::Type::Void, hir::ConstValue::Void),
        ast::Lit::Null => (hir::Type::Rawptr, hir::ConstValue::Null),
        ast::Lit::Bool(val) => (
            hir::Type::Bool(BoolType::Untyped),
            hir::ConstValue::Bool { val, bool_ty: BoolType::Untyped },
        ),
        ast::Lit::Int(val) => (
            hir::Type::Int(IntType::Untyped),
            hir::ConstValue::Int { val, neg: false, int_ty: IntType::Untyped },
        ),
        ast::Lit::Float(val) => (
            hir::Type::Float(FloatType::Untyped),
            hir::ConstValue::Float { val, float_ty: FloatType::Untyped },
        ),
        ast::Lit::Char(val) => {
            (hir::Type::UntypedChar, hir::ConstValue::Char { val, untyped: true })
        }
        ast::Lit::String(val) => (
            hir::Type::String(StringType::Untyped),
            hir::ConstValue::String { val, string_ty: StringType::Untyped },
        ),
    };
    TypeResult::new(ty, hir::Expr::Const(value, hir::ConstID::dummy()))
}

/// unify type across braches:  
/// never -> anything  
/// error -> anything except never
fn type_unify_control_flow<'hir>(ty: &mut hir::Type<'hir>, res_ty: hir::Type<'hir>) {
    if ty.is_never() || (ty.is_error() && !res_ty.is_never()) {
        *ty = res_ty;
    }
}

fn typecheck_if<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    mut expect: Expectation<'hir>,
    if_: &ast::If<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let mut if_type = hir::Type::Never;

    let offset = ctx.cache.branches.start();
    for branch in if_.branches {
        let cond = match branch.kind {
            ast::BranchKind::Cond(cond) => {
                let cond_res = typecheck_expr(ctx, Expectation::None, cond);
                check_expect_boolean(ctx, cond.range, cond_res.ty);
                cond_res.expr
            }
            ast::BranchKind::Pat(pat_ast, expr) => {
                let error_count = ctx.emit.error_count();
                let on_res = typecheck_expr(ctx, Expectation::None, expr);
                let kind = check_match::match_kind(ctx, on_res.ty, expr.range);
                let (pat_expect, ref_mut) = check_match::match_pat_expect(ctx, expr.range, kind);

                ctx.scope.local.start_block(BlockStatus::None);
                let pat = typecheck_pat(ctx, pat_expect, pat_ast, ref_mut, false);

                if let Some(kind) = kind {
                    let arms = [hir::MatchArm { pat, block: hir::Block { stmts: &[] } }];
                    let arms_ast = [ast::MatchArm { pat: *pat_ast, expr }];
                    let check = check_match::CheckContext::new(None, &arms, &arms_ast);
                    check_match::match_cov(ctx, kind, &check, error_count);

                    let any_wild = match pat {
                        hir::Pat::Wild => true,
                        hir::Pat::Or(pats) => pats.iter().any(|p| matches!(p, hir::Pat::Wild)),
                        _ => false,
                    };
                    let value_true = hir::ConstValue::Bool { val: true, bool_ty: BoolType::Bool };
                    let expr_true = hir::Expr::Const(value_true, hir::ConstID::dummy());
                    let stmt_true = hir::Stmt::ExprTail(ctx.arena.alloc(expr_true));
                    let block_true = hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_true]) };
                    let arm_true = hir::MatchArm { pat, block: block_true };

                    let arms = if any_wild {
                        ctx.arena.alloc_slice(&[arm_true])
                    } else {
                        let value_false =
                            hir::ConstValue::Bool { val: false, bool_ty: BoolType::Bool };
                        let expr_false = hir::Expr::Const(value_false, hir::ConstID::dummy());
                        let stmt_false = hir::Stmt::ExprTail(ctx.arena.alloc(expr_false));
                        let block_false =
                            hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_false]) };
                        let arm_false = hir::MatchArm { pat: hir::Pat::Wild, block: block_false };
                        ctx.arena.alloc_slice(&[arm_true, arm_false])
                    };
                    let match_ = hir::Match { kind, on_expr: on_res.expr, arms };
                    emit_match_expr(ctx, match_)
                } else {
                    &hir::Expr::Error
                }
            }
        };

        let block_res = typecheck_block(ctx, expect, branch.block, BlockStatus::None);
        type_unify_control_flow(&mut if_type, block_res.ty);
        if let ast::BranchKind::Pat(_, _) = branch.kind {
            ctx.scope.local.exit_block();
        }

        if let Expectation::None = expect {
            if !block_res.ty.is_error() && !block_res.ty.is_never() {
                let expect_src = block_res.tail_range.map(|range| ctx.src(range));
                expect = Expectation::HasType(block_res.ty, expect_src);
            }
        }
        let branch = hir::Branch { cond, block: block_res.block };
        ctx.cache.branches.push(branch);
    }
    let branches = ctx.cache.branches.take(offset, &mut ctx.arena);

    let else_block = match if_.else_block {
        Some(block) => {
            let block_res = typecheck_block(ctx, expect, block, BlockStatus::None);
            type_unify_control_flow(&mut if_type, block_res.ty);
            Some(block_res.block)
        }
        None => None,
    };

    if else_block.is_none() && if_type.is_never() {
        if_type = hir::Type::Void;
    }
    if else_block.is_none() && !if_type.is_error() && !if_type.is_void() && !if_type.is_never() {
        let src = ctx.src(TextRange::new(expr_range.start(), expr_range.start() + 2.into()));
        err::tycheck_if_missing_else(&mut ctx.emit, src);
    }

    let if_ = ctx.arena.alloc(hir::If { branches, else_block });
    TypeResult::new_ignore(if_type, hir::Expr::If { if_ })
}

fn typecheck_match<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    mut expect: Expectation<'hir>,
    match_: &ast::Match<'ast>,
    match_range: TextRange,
) -> TypeResult<'hir> {
    let mut match_type = hir::Type::Never;
    let error_count = ctx.emit.error_count();
    let on_res = typecheck_expr(ctx, Expectation::None, match_.on_expr);
    let kind = check_match::match_kind(ctx, on_res.ty, match_.on_expr.range);
    let (pat_expect, ref_mut) = check_match::match_pat_expect(ctx, match_.on_expr.range, kind);

    let offset = ctx.cache.match_arms.start();
    for arm in match_.arms {
        ctx.scope.local.start_block(BlockStatus::None);
        let pat = typecheck_pat(ctx, pat_expect, &arm.pat, ref_mut, false);
        let expr_res = typecheck_expr(ctx, expect, arm.expr);
        type_unify_control_flow(&mut match_type, expr_res.ty);
        ctx.scope.local.exit_block();

        if matches!(expect, Expectation::None) && !expr_res.ty.is_error() && !expr_res.ty.is_never()
        {
            let expect_src = ctx.src(arm.expr.range);
            expect = Expectation::HasType(expr_res.ty, Some(expect_src));
        }

        let block = match expr_res.expr {
            hir::Expr::Block { block } => *block,
            _ => {
                let tail_stmt = hir::Stmt::ExprTail(expr_res.expr);
                let stmts = ctx.arena.alloc_slice(&[tail_stmt]);
                hir::Block { stmts }
            }
        };
        ctx.cache.match_arms.push(hir::MatchArm { pat, block });
    }
    let arms = ctx.cache.match_arms.take(offset, &mut ctx.arena);

    if ctx.emit.did_error(error_count) {
        TypeResult::error()
    } else if let Some(kind) = kind {
        let mut match_kw = TextRange::empty_at(match_range.start());
        match_kw.extend_by(5.into());
        let check = check_match::CheckContext::new(Some(match_kw), arms, match_.arms);
        check_match::match_cov(ctx, kind, &check, error_count);

        let match_ = hir::Match { kind, on_expr: on_res.expr, arms };
        TypeResult::new_ignore(match_type, *emit_match_expr(ctx, match_))
    } else {
        TypeResult::error()
    }
}

fn emit_match_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    match_: hir::Match<'hir>,
) -> &'hir hir::Expr<'hir> {
    if match_.arms.is_empty() || !matches!(match_.kind, hir::MatchKind::String) {
        let match_ = ctx.arena.alloc(match_);
        return ctx.arena.alloc(hir::Expr::Match { match_ });
    }

    let discard_id = ctx.session.intern_name.intern("_");
    let name_dummy = ast::Name { id: discard_id, range: TextRange::zero() };
    let value_var = hir::Variable {
        mutt: ast::Mut::Immutable,
        name: name_dummy,
        ty: hir::Type::String(hir::StringType::String),
        was_used: false,
    };
    let var_id = ctx.scope.local.add_variable(value_var);
    let local = hir::Local { var_id, init: hir::LocalInit::Init(match_.on_expr) };
    let stmt_local = hir::Stmt::Local(ctx.arena.alloc(local));
    let lhs = ctx.arena.alloc(hir::Expr::Variable { var_id });

    //single string pattern with `_`:
    if match_.arms.len() == 1 {
        let stmt_offset = ctx.cache.stmts.start();
        ctx.cache.stmts.push(stmt_local);
        let tail = ctx.arena.alloc(hir::Expr::Block { block: match_.arms[0].block });
        ctx.cache.stmts.push(hir::Stmt::ExprTail(tail));
        let stmts = ctx.cache.stmts.take(stmt_offset, &mut ctx.arena);
        return ctx.arena.alloc(hir::Expr::Block { block: hir::Block { stmts } });
    }

    //multiple string patterns compile to if else chain:
    let offset = ctx.cache.branches.start();
    let (last, rest) = match_.arms.split_last().unwrap();

    for arm in rest {
        match arm.pat {
            hir::Pat::Lit(value) => {
                let cond = string_cmp(ctx, lhs, value);
                let br = hir::Branch { cond, block: arm.block };
                ctx.cache.branches.push(br);
            }
            hir::Pat::Or(pats) => {
                let mut lhs_or = string_cmp(ctx, lhs, pat_value(&pats[0]));
                for pat in &pats[1..] {
                    let rhs_or = string_cmp(ctx, lhs, pat_value(pat));
                    let op = hir::BinOp::LogicOr(hir::BoolType::Bool);
                    lhs_or = ctx.arena.alloc(hir::Expr::Binary { op, lhs: lhs_or, rhs: rhs_or });
                }
                let br = hir::Branch { cond: lhs_or, block: arm.block };
                ctx.cache.branches.push(br);
            }
            _ => {}
        }
    }

    fn string_cmp<'hir>(
        ctx: &mut HirCtx<'hir, '_, '_>,
        lhs: &'hir hir::Expr<'hir>,
        value: hir::ConstValue<'hir>,
    ) -> &'hir hir::Expr<'hir> {
        let op =
            hir::BinOp::Cmp_String(hir::CmpPred::Eq, hir::BoolType::Bool, hir::StringType::String);
        let bin = hir::Expr::Binary {
            op,
            lhs,
            rhs: ctx.arena.alloc(hir::Expr::Const(value, hir::ConstID::dummy())),
        };
        ctx.arena.alloc(bin)
    }

    fn pat_value<'hir>(pat: &hir::Pat<'hir>) -> hir::ConstValue<'hir> {
        match pat {
            hir::Pat::Lit(value) => *value,
            _ => hir::ConstValue::Void, //placeholder
        }
    }

    let branches = ctx.cache.branches.take(offset, &mut ctx.arena);
    let else_block = Some(last.block);
    let if_ = ctx.arena.alloc(hir::If { branches, else_block });
    let tail = hir::Stmt::ExprTail(ctx.arena.alloc(hir::Expr::If { if_ }));

    let stmt_offset = ctx.cache.stmts.start();
    ctx.cache.stmts.push(stmt_local);
    ctx.cache.stmts.push(tail);
    let stmts = ctx.cache.stmts.take(stmt_offset, &mut ctx.arena);
    ctx.arena.alloc(hir::Expr::Block { block: hir::Block { stmts } })
}

fn typecheck_pat<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    pat: &ast::Pat<'ast>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
) -> hir::Pat<'hir> {
    let mut pat_res = match pat.kind {
        ast::PatKind::Wild => PatResult::new(hir::Pat::Wild, hir::Type::Error),
        ast::PatKind::Lit { expr } => typecheck_pat_lit(ctx, expect, expr),
        ast::PatKind::Range { range } => unimplemented!(), //@implement
        ast::PatKind::Item { path, bind_list } => {
            typecheck_pat_item(ctx, expect, path, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Variant { name, bind_list } => {
            typecheck_pat_variant(ctx, expect, name, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Or { pats } => typecheck_pat_or(ctx, expect, pats, ref_mut),
    };

    //apply untyped promotion for literal values and constants
    let with = expect.inner_type();
    let expr = match pat_res.pat {
        hir::Pat::Lit(value) => hir::Expr::Const(value, hir::ConstID::dummy()),
        _ => hir::Expr::Error,
    };
    if let Some(Ok((value, _))) =
        promote_untyped(ctx, pat.range, expr, &mut pat_res.pat_ty, with, true)
    {
        pat_res.pat = hir::Pat::Lit(value);
    }

    type_expectation_check(ctx, pat.range, pat_res.pat_ty, expect);
    pat_res.pat
}

fn typecheck_pat_lit<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
) -> PatResult<'hir> {
    let expr_res = typecheck_expr(ctx, expect, expr);
    match expr_res.expr {
        hir::Expr::Error => PatResult::error(),
        hir::Expr::Const(value, _) => PatResult::new(hir::Pat::Lit(*value), expr_res.ty),
        _ => unreachable!(), // literal patterns can only be const or error expressions
    }
}

fn typecheck_pat_item<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    path: &ast::Path<'ast>,
    bind_list: Option<&ast::BindingList>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
    range: TextRange,
) -> PatResult<'hir> {
    match check_path::path_resolve_value(ctx, path, false) {
        ValueID::None => {
            check_variant_bind_list(ctx, expect, bind_list, None, None, in_or_pat);
            PatResult::error()
        }
        ValueID::Enum(enum_id, variant_id, poly_types) => {
            check_variant_bind_count(ctx, bind_list, enum_id, variant_id, range);
            let variant = Some(ctx.registry.enum_data(enum_id).variant(variant_id));
            let bind_ids =
                check_variant_bind_list(ctx, expect, bind_list, variant, ref_mut, in_or_pat);

            let poly = if let Some(poly_types) = poly_types {
                poly_types
            } else if let Expectation::HasType(hir::Type::Enum(_, poly_types), _) = expect {
                poly_types
            } else {
                let data = ctx.registry.enum_data(enum_id);
                let count = data.poly_params.map_or(0, |p| p.len());
                ctx.arena.alloc_slice_with_value(hir::Type::Unknown, count)
            };

            PatResult::new(
                hir::Pat::Variant(enum_id, variant_id, bind_ids),
                hir::Type::Enum(enum_id, poly),
            )
        }
        ValueID::Const(const_id, fields) => {
            if let Some(name) = fields.first() {
                let src = ctx.src(name.name.range);
                err::tycheck_pat_const_field_access(&mut ctx.emit, src);
            }
            if let Some(bind_list) = bind_list {
                let src = ctx.src(bind_list.range);
                err::tycheck_pat_const_with_bindings(&mut ctx.emit, src);
            }
            check_variant_bind_list(ctx, expect, bind_list, None, None, in_or_pat);

            let data = ctx.registry.const_data(const_id);
            let (eval, _, _) = ctx.registry.const_eval(data.value);

            let pat = if let Ok(value) = eval.resolved() {
                if let hir::ConstValue::Variant { .. } = value {
                    let src = ctx.src(range);
                    err::tycheck_pat_const_enum(&mut ctx.emit, src);
                    hir::Pat::Error
                } else {
                    hir::Pat::Lit(value)
                }
            } else {
                hir::Pat::Error
            };
            PatResult::new(pat, data.ty.expect("typed const var"))
        }
        ValueID::Proc(_, _)
        | ValueID::Global(_, _)
        | ValueID::Param(_, _)
        | ValueID::Variable(_, _) => {
            let src = ctx.src(range);
            err::tycheck_pat_runtime_value(&mut ctx.emit, src);
            check_variant_bind_list(ctx, expect, bind_list, None, None, in_or_pat);
            PatResult::new(hir::Pat::Error, hir::Type::Error)
        }
    }
}

fn typecheck_pat_variant<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    bind_list: Option<&ast::BindingList>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
    pat_range: TextRange,
) -> PatResult<'hir> {
    let name_src = ctx.src(name.range);
    let (enum_id, poly_types) = match infer_enum_type(ctx, expect, name_src) {
        Some(found) => found,
        None => {
            check_variant_bind_list(ctx, expect, bind_list, None, None, in_or_pat);
            return PatResult::error();
        }
    };
    let variant_id = match scope::check_find_enum_variant(ctx, enum_id, name) {
        Some(found) => found,
        None => {
            check_variant_bind_list(ctx, expect, bind_list, None, None, in_or_pat);
            return PatResult::error();
        }
    };

    check_variant_bind_count(ctx, bind_list, enum_id, variant_id, pat_range);
    let variant = Some(ctx.registry.enum_data(enum_id).variant(variant_id));
    let bind_ids = check_variant_bind_list(ctx, expect, bind_list, variant, ref_mut, in_or_pat);

    let poly = if let Some(poly_types) = poly_types {
        poly_types
    } else {
        let data = ctx.registry.enum_data(enum_id);
        let count = data.poly_params.map_or(0, |p| p.len());
        ctx.arena.alloc_slice_with_value(hir::Type::Unknown, count)
    };

    PatResult::new(hir::Pat::Variant(enum_id, variant_id, bind_ids), hir::Type::Enum(enum_id, poly))
}

fn typecheck_pat_or<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    pats: &[ast::Pat<'ast>],
    ref_mut: Option<ast::Mut>,
) -> PatResult<'hir> {
    let offset = ctx.cache.patterns.start();
    for pat in pats {
        let pat = typecheck_pat(ctx, expect, pat, ref_mut, true);
        ctx.cache.patterns.push(pat);
    }
    let pats = ctx.cache.patterns.take(offset, &mut ctx.arena);

    PatResult::new(hir::Pat::Or(pats), hir::Type::Error)
}

fn typecheck_field<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let field_res = check_field_from_type(ctx, name, target_res.ty);
    emit_field_expr(ctx, target_res.expr, field_res, name.range.start())
}

struct FieldResult<'hir> {
    deref: Option<ast::Mut>,
    kind: FieldKind<'hir>,
    field_ty: hir::Type<'hir>,
}

#[rustfmt::skip]
enum FieldKind<'hir> {
    Error,
    Struct(hir::StructID, hir::FieldID, &'hir [hir::Type<'hir>]),
    ArraySlice { field: hir::SliceField },
    ArrayStatic { len: hir::ConstValue<'hir> },
    ArrayStaticIndex { index: u64, len: hir::ArrayStaticLen },
}

impl<'hir> FieldResult<'hir> {
    fn new(
        deref: Option<ast::Mut>,
        kind: FieldKind<'hir>,
        field_ty: hir::Type<'hir>,
    ) -> FieldResult<'hir> {
        FieldResult { deref, kind, field_ty }
    }
    fn error() -> FieldResult<'hir> {
        FieldResult { deref: None, kind: FieldKind::Error, field_ty: hir::Type::Error }
    }
}

fn check_field_from_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    name: ast::Name,
    ty: hir::Type<'hir>,
) -> FieldResult<'hir> {
    let (ty, deref) = match ty {
        hir::Type::Reference(mutt, ref_ty) => (*ref_ty, Some(mutt)),
        _ => (ty, None),
    };

    match ty {
        hir::Type::Error => FieldResult::error(),
        hir::Type::String(StringType::String | StringType::Untyped) => {
            let field_name = ctx.name(name.id);
            match field_name {
                "len" => {
                    let kind = FieldKind::ArraySlice { field: hir::SliceField::Len };
                    let field_ty = hir::Type::Int(IntType::U64);
                    FieldResult::new(deref, kind, field_ty)
                }
                _ => {
                    let src = ctx.src(name.range);
                    err::tycheck_field_not_found_string(&mut ctx.emit, src, field_name);
                    FieldResult::error()
                }
            }
        }
        hir::Type::Struct(struct_id, poly_types) => {
            match scope::check_find_struct_field(ctx, struct_id, name) {
                Some(field_id) => {
                    let data = ctx.registry.struct_data(struct_id);
                    let field = data.field(field_id);
                    let kind = FieldKind::Struct(struct_id, field_id, poly_types);
                    let is_poly = data.poly_params.is_some();
                    let field_ty = type_substitute(ctx, is_poly, poly_types, field.ty);
                    FieldResult::new(deref, kind, field_ty)
                }
                None => FieldResult::error(),
            }
        }
        hir::Type::ArraySlice(slice) => match check_field_from_slice(ctx, name) {
            Some(field) => {
                let kind = FieldKind::ArraySlice { field };
                let field_ty = match field {
                    hir::SliceField::Ptr => hir::Type::Reference(slice.mutt, &slice.elem_ty),
                    hir::SliceField::Len => hir::Type::Int(IntType::U64),
                };
                FieldResult::new(deref, kind, field_ty)
            }
            None => FieldResult::error(),
        },
        hir::Type::ArrayStatic(array) => match check_field_from_array(ctx, name, array) {
            Some((kind, field_ty)) => FieldResult::new(deref, kind, field_ty),
            None => FieldResult::error(),
        },
        hir::Type::ArrayEnumerated(array) => match check_field_from_array_enum(ctx, name, array) {
            Some(len) => {
                let kind = FieldKind::ArrayStatic { len };
                let field_ty = hir::Type::Int(IntType::U64);
                FieldResult::new(deref, kind, field_ty)
            }
            None => FieldResult::error(),
        },
        _ => {
            let src = ctx.src(name.range);
            let field_name = ctx.name(name.id);
            let ty_fmt = type_format(ctx, ty);
            err::tycheck_field_not_found_ty(&mut ctx.emit, src, field_name, &ty_fmt);
            FieldResult::error()
        }
    }
}

fn check_field_from_slice(ctx: &mut HirCtx, name: ast::Name) -> Option<hir::SliceField> {
    let field_name = ctx.name(name.id);
    match field_name {
        "ptr" => Some(hir::SliceField::Ptr),
        "len" => Some(hir::SliceField::Len),
        _ => {
            let src = ctx.src(name.range);
            err::tycheck_field_not_found_slice(&mut ctx.emit, src, field_name);
            None
        }
    }
}

fn check_field_from_array<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    name: ast::Name,
    array: &hir::ArrayStatic<'hir>,
) -> Option<(FieldKind<'hir>, hir::Type<'hir>)> {
    let len = ctx.array_len(array.len).ok()?;
    let field = ctx.name(name.id);

    if field == "len" {
        let len = hir::ConstValue::from_u64(len, IntType::U64);
        return Some((FieldKind::ArrayStatic { len }, hir::Type::Int(IntType::U64)));
    }
    let index: u64 = match field {
        "x" | "r" => 0,
        "y" | "g" => 1,
        "z" | "b" => 2,
        "w" | "a" => 3,
        _ => {
            let src = ctx.src(name.range);
            err::tycheck_field_not_found_array(&mut ctx.emit, src, field);
            return None;
        }
    };
    if index >= len {
        let src = ctx.src(name.range);
        err::tycheck_field_not_found_array(&mut ctx.emit, src, field); //@custom error?
        return None;
    }
    Some((FieldKind::ArrayStaticIndex { index, len: array.len }, array.elem_ty))
}

fn check_field_from_array_enum<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    name: ast::Name,
    array: &hir::ArrayEnumerated,
) -> Option<hir::ConstValue<'hir>> {
    let field_name = ctx.name(name.id);
    match field_name {
        "len" => {
            let val = ctx.registry.enum_data(array.enum_id).variants.len() as u64;
            Some(hir::ConstValue::Int { val, neg: false, int_ty: IntType::U64 })
        }
        _ => {
            let src = ctx.src(name.range);
            err::tycheck_field_not_found_array(&mut ctx.emit, src, field_name);
            None
        }
    }
}

fn emit_field_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    target: &'hir hir::Expr<'hir>,
    field_res: FieldResult<'hir>,
    name_start: TextOffset,
) -> TypeResult<'hir> {
    match field_res.kind {
        FieldKind::Error => TypeResult::error(),
        FieldKind::Struct(struct_id, field_id, poly_types) => {
            if let hir::Expr::Const(value, _) = target {
                let field = match value {
                    hir::ConstValue::Struct { struct_, .. } => struct_.values[field_id.index()],
                    _ => unreachable!(),
                };
                return TypeResult::new(
                    field_res.field_ty,
                    hir::Expr::Const(field, hir::ConstID::dummy()),
                );
            }
            let access = hir::StructFieldAccess {
                deref: field_res.deref,
                struct_id,
                field_id,
                field_ty: field_res.field_ty,
                poly_types,
            };
            let access = ctx.arena.alloc(access);
            TypeResult::new(field_res.field_ty, hir::Expr::StructField { target, access })
        }
        FieldKind::ArraySlice { field } => {
            if let hir::Expr::Const(value, _) = target {
                let field = match value {
                    hir::ConstValue::String { val, .. } => {
                        let len = ctx.session.intern_lit.get(*val).len();
                        hir::ConstValue::from_u64(len as u64, IntType::U64)
                    }
                    _ => unreachable!(),
                };
                return TypeResult::new(
                    field_res.field_ty,
                    hir::Expr::Const(field, hir::ConstID::dummy()),
                );
            }
            let access = hir::SliceFieldAccess { deref: field_res.deref, field };
            TypeResult::new(field_res.field_ty, hir::Expr::SliceField { target, access })
        }
        FieldKind::ArrayStatic { len } => {
            TypeResult::new(field_res.field_ty, hir::Expr::Const(len, hir::ConstID::dummy()))
        }
        FieldKind::ArrayStaticIndex { index, len } => {
            if let hir::Expr::Const(value, const_id) = target {
                let elem_value = match value {
                    hir::ConstValue::Array { array } => array.values[index as usize],
                    hir::ConstValue::ArrayRepeat { array } => array.value,
                    _ => unreachable!(),
                };
                TypeResult::new(field_res.field_ty, hir::Expr::Const(elem_value, *const_id))
            } else {
                let access = hir::IndexAccess {
                    deref: field_res.deref,
                    elem_ty: field_res.field_ty,
                    kind: hir::IndexKind::Array(len),
                    index: ctx.arena.alloc(hir::Expr::Const(
                        hir::ConstValue::from_u64(index, IntType::U64),
                        hir::ConstID::dummy(),
                    )),
                    offset: name_start,
                };
                let expr = hir::Expr::Index { target, access: ctx.arena.alloc(access) };
                TypeResult::new(field_res.field_ty, expr)
            }
        }
    }
}

struct CollectionType<'hir> {
    deref: Option<ast::Mut>,
    elem_ty: hir::Type<'hir>,
    string_ty: Option<StringType>,
    kind: CollectionKind<'hir>,
}

enum CollectionKind<'hir> {
    Multi(ast::Mut),
    Slice(&'hir hir::ArraySlice<'hir>),
    Array(&'hir hir::ArrayStatic<'hir>),
    ArrayEnum(&'hir hir::ArrayEnumerated<'hir>),
    ArrayCore,
}

fn type_as_collection<'hir>(
    ctx: &HirCtx<'hir, '_, '_>,
    mut ty: hir::Type<'hir>,
) -> Result<Option<CollectionType<'hir>>, ()> {
    let mut deref = None;
    if let hir::Type::Reference(mutt, ref_ty) = ty {
        ty = *ref_ty;
        deref = Some(mutt);
    }
    match ty {
        hir::Type::Error => Ok(None),
        hir::Type::String(string_ty) => match string_ty {
            StringType::String => Ok(Some(CollectionType {
                deref,
                elem_ty: hir::Type::Int(IntType::U8),
                string_ty: Some(string_ty),
                kind: CollectionKind::Slice(&hir::ArraySlice {
                    mutt: ast::Mut::Immutable,
                    elem_ty: hir::Type::Int(IntType::U8),
                }),
            })),
            StringType::CString => Ok(Some(CollectionType {
                deref,
                elem_ty: hir::Type::Int(IntType::U8),
                string_ty: Some(string_ty),
                kind: CollectionKind::Multi(ast::Mut::Immutable),
            })),
            StringType::Untyped => unreachable!(),
        },
        hir::Type::Struct(struct_id, poly_types) => {
            if struct_id == ctx.core.array {
                Ok(Some(CollectionType {
                    deref,
                    elem_ty: poly_types[0],
                    string_ty: None,
                    kind: CollectionKind::ArrayCore,
                }))
            } else {
                Err(())
            }
        }
        hir::Type::MultiReference(mutt, ref_ty) => Ok(Some(CollectionType {
            deref,
            elem_ty: *ref_ty,
            string_ty: None,
            kind: CollectionKind::Multi(mutt),
        })),
        hir::Type::ArraySlice(slice) => Ok(Some(CollectionType {
            deref,
            elem_ty: slice.elem_ty,
            string_ty: None,
            kind: CollectionKind::Slice(slice),
        })),
        hir::Type::ArrayStatic(array) => Ok(Some(CollectionType {
            deref,
            elem_ty: array.elem_ty,
            string_ty: None,
            kind: CollectionKind::Array(array),
        })),
        hir::Type::ArrayEnumerated(array) => Ok(Some(CollectionType {
            deref,
            elem_ty: array.elem_ty,
            string_ty: None,
            kind: CollectionKind::ArrayEnum(array),
        })),
        _ => Err(()),
    }
}

fn typecheck_index<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: TextRange,
    target: &ast::Expr<'ast>,
    index: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let error_count = ctx.emit.error_count();
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let collection_res = type_as_collection(ctx, target_res.ty);
    let index_expect = if let Ok(Some(collection)) = &collection_res {
        if let CollectionKind::ArrayEnum(array) = collection.kind {
            Expectation::HasType(hir::Type::Enum(array.enum_id, &[]), None)
        } else {
            Expectation::USIZE
        }
    } else {
        Expectation::USIZE
    };
    let index_res = typecheck_expr(ctx, index_expect, index);

    let collection = match collection_res {
        Ok(None) => return TypeResult::error(),
        Ok(Some(value)) => value,
        Err(()) => {
            let src = ctx.src(range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_index_on_ty(&mut ctx.emit, src, &ty_fmt);
            return TypeResult::error();
        }
    };

    let kind = match collection.kind {
        CollectionKind::Multi(mutt) => hir::IndexKind::Multi(mutt),
        CollectionKind::Slice(slice) => hir::IndexKind::Slice(slice.mutt),
        CollectionKind::Array(array) => hir::IndexKind::Array(array.len),
        CollectionKind::ArrayEnum(array) => hir::IndexKind::ArrayEnum(array.enum_id),
        CollectionKind::ArrayCore => hir::IndexKind::ArrayCore,
    };
    let access = hir::IndexAccess {
        deref: collection.deref,
        elem_ty: collection.elem_ty,
        kind,
        index: index_res.expr,
        offset: target.range.end(),
    };

    if !ctx.emit.did_error(error_count) && !matches!(kind, hir::IndexKind::ArrayCore) {
        if let hir::Expr::Const(target, target_id) = target_res.expr {
            if let hir::Expr::Const(index, _) = index_res.expr {
                let index = if let hir::IndexKind::ArrayEnum(_) = kind {
                    index.into_enum().1.index() as u64
                } else {
                    index.into_int_u64()
                };

                let array_len = match target {
                    hir::ConstValue::String { val, .. } => {
                        ctx.session.intern_lit.get(*val).len() as u64
                    }
                    hir::ConstValue::Array { array } => array.values.len() as u64,
                    hir::ConstValue::ArrayRepeat { array } => array.len,
                    hir::ConstValue::ArrayEmpty { .. } => 0,
                    _ => unreachable!(),
                };
                if index >= array_len {
                    let src = ctx.src(range);
                    err::const_index_out_of_bounds(&mut ctx.emit, src, index, array_len);
                    return TypeResult::error();
                }
                let value = match target {
                    hir::ConstValue::String { val, .. } => {
                        let byte = ctx.session.intern_lit.get(*val).as_bytes()[index as usize];
                        hir::ConstValue::Int { val: byte as u64, neg: false, int_ty: IntType::U8 }
                    }
                    hir::ConstValue::Array { array } => array.values[index as usize],
                    hir::ConstValue::ArrayRepeat { array } => array.value,
                    _ => unreachable!(),
                };
                return TypeResult::new(
                    collection.elem_ty,
                    hir::Expr::Const(value, hir::ConstID::dummy()),
                );
            } else if *target_id != hir::ConstID::dummy() {
                let src = ctx.src(range);
                let data = ctx.registry.const_data(*target_id);
                err::tycheck_index_const(&mut ctx.emit, src, data.src());
                return TypeResult::error();
            }
        }
    }

    let index = hir::Expr::Index { target: target_res.expr, access: ctx.arena.alloc(access) };
    TypeResult::new(collection.elem_ty, index)
}

fn typecheck_slice<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    target: &ast::Expr<'ast>,
    range: &ast::SliceRange<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let expect = expect.infer_slicing_array_type(ctx, target);
    let target_res = typecheck_expr(ctx, expect, target);

    let collection = match type_as_collection(ctx, target_res.ty) {
        Ok(None) => return TypeResult::error(),
        Ok(Some(collection)) => match collection.kind {
            CollectionKind::Slice(_)
            | CollectionKind::Array(_)
            | CollectionKind::ArrayEnum(_)
            | CollectionKind::ArrayCore => collection,
            CollectionKind::Multi(_) => {
                let src = ctx.src(target.range);
                let ty_fmt = type_format(ctx, target_res.ty);
                err::tycheck_cannot_slice_on_type(&mut ctx.emit, src, &ty_fmt);
                return TypeResult::error();
            }
        },
        Err(()) => {
            let src = ctx.src(expr_range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_slice_on_type(&mut ctx.emit, src, &ty_fmt);
            return TypeResult::error();
        }
    };

    let addr_res = resolve_expr_addressability(ctx, target_res.expr);
    let slice_mutt = check_slice_addressability(ctx, addr_res, &collection, expr_range);
    let mut expect = Expectation::USIZE;

    let slice = match collection.kind {
        CollectionKind::Multi(_) => {
            unreachable!()
        }
        CollectionKind::Slice(_) => {
            if let hir::Type::Reference(mutt, ref_ty) = target_res.ty {
                let deref = hir::Expr::Deref { rhs: target_res.expr, mutt, ref_ty };
                ctx.arena.alloc(deref)
            } else {
                target_res.expr
            }
        }
        CollectionKind::Array(array) => {
            let ptr = if let hir::Type::Reference(_, _) = target_res.ty {
                target_res.expr
            } else {
                ctx.arena.alloc(hir::Expr::Address { rhs: target_res.expr })
            };
            let len = ctx.array_len(array.len).unwrap_or(0);
            let len = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::from_u64(len, IntType::U64),
                hir::ConstID::dummy(),
            ));
            let proc_id = ctx.core.from_raw_parts;
            let input = ctx.arena.alloc_slice(&[ptr, len]);
            let poly_types = ctx.arena.alloc_slice(&[collection.elem_ty]);
            let input = ctx.arena.alloc((input, poly_types));
            ctx.arena.alloc(hir::Expr::CallDirectPoly { proc_id, input })
        }
        CollectionKind::ArrayEnum(array) => {
            expect = Expectation::HasType(hir::Type::Enum(array.enum_id, &[]), None);
            let ptr = if let hir::Type::Reference(_, _) = target_res.ty {
                target_res.expr
            } else {
                ctx.arena.alloc(hir::Expr::Address { rhs: target_res.expr })
            };
            let len = ctx.registry.enum_data(array.enum_id).variants.len() as u64;
            let len = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::from_u64(len, IntType::U64),
                hir::ConstID::dummy(),
            ));
            let proc_id = ctx.core.from_raw_parts;
            let input = ctx.arena.alloc_slice(&[ptr, len]);
            let poly_types = ctx.arena.alloc_slice(&[collection.elem_ty]);
            let input = ctx.arena.alloc((input, poly_types));
            ctx.arena.alloc(hir::Expr::CallDirectPoly { proc_id, input })
        }
        CollectionKind::ArrayCore => {
            let ptr = if let hir::Type::Reference(_, _) = target_res.ty {
                target_res.expr
            } else {
                ctx.arena.alloc(hir::Expr::Address { rhs: target_res.expr })
            };
            let proc_id = if slice_mutt == ast::Mut::Immutable {
                ctx.core.values
            } else {
                ctx.core.values_mut
            };
            let input = ctx.arena.alloc_slice(&[ptr]);
            let poly_types = ctx.arena.alloc_slice(&[collection.elem_ty]);
            let input = ctx.arena.alloc((input, poly_types));
            ctx.arena.alloc(hir::Expr::CallDirectPoly { proc_id, input })
        }
    };

    let return_ty = if let Some(string_ty) = collection.string_ty {
        hir::Type::String(string_ty)
    } else {
        let slice = hir::ArraySlice { mutt: slice_mutt, elem_ty: collection.elem_ty };
        hir::Type::ArraySlice(ctx.arena.alloc(slice))
    };

    // return the full slice in case of `[..]`
    if range.start.is_none() && range.end.is_none() {
        return TypeResult::new(return_ty, *slice);
    }

    // range start
    let start = if let Some(start) = range.start {
        let mut bound = typecheck_expr(ctx, expect, start).expr;
        if let CollectionKind::ArrayEnum(_) = collection.kind {
            let cast = hir::Expr::Cast {
                target: bound,
                into: &hir::Type::Int(IntType::U64),
                kind: hir::CastKind::EnumU_Extend_to_Int,
            };
            bound = ctx.arena.alloc(cast);
        }
        bound
    } else {
        let zero_usize = hir::ConstValue::Int { val: 0, neg: false, int_ty: IntType::U64 };
        ctx.arena.alloc(hir::Expr::Const(zero_usize, hir::ConstID::dummy()))
    };

    // range bound
    let (variant_id, input) = if let Some((kind, end)) = range.end {
        let variant_id = match kind {
            ast::RangeKind::Exclusive => hir::VariantID::new(1),
            ast::RangeKind::Inclusive => hir::VariantID::new(2),
        };
        let mut bound = typecheck_expr(ctx, expect, end).expr;
        if let CollectionKind::ArrayEnum(_) = collection.kind {
            let cast = hir::Expr::Cast {
                target: bound,
                into: &hir::Type::Int(IntType::U64),
                kind: hir::CastKind::EnumU_Extend_to_Int,
            };
            bound = ctx.arena.alloc(cast);
        }
        (variant_id, ctx.arena.alloc_slice(&[bound]))
    } else {
        (hir::VariantID::new(0), ctx.arena.alloc_slice(&[]))
    };
    let enum_id = ctx.core.range_bound;
    let bound = hir::Expr::Variant { enum_id, variant_id, input: ctx.arena.alloc((input, &[])) };
    let bound = ctx.arena.alloc(bound);

    let proc_id = ctx.core.slice_range;
    let input = ctx.arena.alloc_slice(&[slice, start, bound]);
    let poly_types = ctx.arena.alloc_slice(&[collection.elem_ty]);
    let input = ctx.arena.alloc((input, poly_types));
    let op_call = hir::Expr::CallDirectPoly { proc_id, input };

    let kind = match collection.kind {
        CollectionKind::Multi(_) => unreachable!(),
        CollectionKind::Slice(slice) => hir::SliceKind::Slice(slice.mutt),
        CollectionKind::Array(_) | CollectionKind::ArrayEnum(_) | CollectionKind::ArrayCore => {
            hir::SliceKind::Array
        }
    };
    let access = hir::SliceAccess { deref: collection.deref, kind, op_call };
    let access = ctx.arena.alloc(access);
    let expr = hir::Expr::Slice { target: target_res.expr, access };
    TypeResult::new(return_ty, expr)
}

fn typecheck_call<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    args_list: &ast::ArgumentList<'ast>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    check_call_indirect(ctx, target.range, target_res, args_list)
}

fn typecheck_cast<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: TextRange,
    target: &ast::Expr<'ast>,
    into: &ast::Type<'ast>,
) -> TypeResult<'hir> {
    use hir::CastKind;
    use std::cmp::Ordering;

    let target_res = typecheck_expr_untyped(ctx, Expectation::None, target);
    let from = target_res.ty;
    let into = super::pass_3::type_resolve(ctx, *into, false);

    if from.is_error() || into.is_error() {
        return TypeResult::error();
    }

    let kind = match (from, into) {
        (hir::Type::Char, hir::Type::Int(IntType::U32)) => CastKind::Char_NoOp,
        (hir::Type::UntypedChar, hir::Type::Int(_)) => CastKind::UntypedChar_to_Int,
        (hir::Type::Rawptr, hir::Type::Reference(_, _))
        | (hir::Type::Rawptr, hir::Type::MultiReference(_, _))
        | (hir::Type::Rawptr, hir::Type::Procedure(_))
        | (hir::Type::Reference(_, _), hir::Type::Rawptr)
        | (hir::Type::MultiReference(_, _), hir::Type::Rawptr)
        | (hir::Type::Procedure(_), hir::Type::Rawptr) => CastKind::Rawptr_NoOp,
        (hir::Type::Int(from_ty), hir::Type::Int(into_ty)) => {
            if from_ty != IntType::Untyped && from_ty == into_ty {
                CastKind::Error
            } else {
                integer_cast_kind(from_ty, into_ty)
            }
        }
        (hir::Type::Int(from_ty), hir::Type::Float(_)) => {
            if from_ty.is_signed() {
                CastKind::IntS_to_Float
            } else {
                CastKind::IntU_to_Float
            }
        }
        (hir::Type::Float(from_ty), hir::Type::Float(into_ty)) => {
            if from_ty == FloatType::Untyped {
                CastKind::Float_Trunc
            } else {
                let from_size = layout::float_layout(from_ty).size;
                let into_size = layout::float_layout(into_ty).size;
                match from_size.cmp(&into_size) {
                    Ordering::Less => CastKind::Float_Extend,
                    Ordering::Equal => CastKind::Error,
                    Ordering::Greater => CastKind::Float_Trunc,
                }
            }
        }
        (hir::Type::Float(_), hir::Type::Int(into_ty)) => {
            if into_ty.is_signed() {
                CastKind::Float_to_IntS
            } else {
                CastKind::Float_to_IntU
            }
        }
        (hir::Type::Bool(from_ty), hir::Type::Bool(into_ty)) => {
            if from_ty == BoolType::Untyped {
                CastKind::Bool_Trunc
            } else {
                let from_size = layout::bool_layout(from_ty).size;
                let into_size = layout::bool_layout(into_ty).size;
                match from_size.cmp(&into_size) {
                    Ordering::Less => CastKind::Bool_Extend,
                    Ordering::Equal => CastKind::Error,
                    Ordering::Greater => CastKind::Bool_Trunc,
                }
            }
        }
        (hir::Type::Bool(from_ty), hir::Type::Int(into_ty)) => {
            if from_ty == BoolType::Untyped {
                CastKind::Bool_NoOp_to_Int
            } else {
                let from_size = layout::bool_layout(from_ty).size;
                let into_size = layout::int_layout(into_ty).size;
                match from_size.cmp(&into_size) {
                    Ordering::Less => CastKind::Bool_Extend_to_Int,
                    Ordering::Equal => {
                        if from_size == 1 {
                            // llvm uses i1, emit extend for `bool`
                            CastKind::Bool_Extend_to_Int
                        } else {
                            CastKind::Bool_NoOp_to_Int
                        }
                    }
                    Ordering::Greater => CastKind::Bool_Trunc_to_Int,
                }
            }
        }
        (hir::Type::String(from_ty), hir::Type::String(_)) => {
            if from_ty == StringType::Untyped {
                CastKind::StringUntyped_NoOp
            } else {
                CastKind::Error
            }
        }
        (hir::Type::Enum(enum_id, _), hir::Type::Int(into_ty)) => {
            let enum_data = ctx.registry.enum_data(enum_id);
            let from_ty = if let Ok(tag_ty) = enum_data.tag_ty.resolved() {
                tag_ty
            } else {
                return TypeResult::error();
            };

            let from_size = layout::int_layout(from_ty).size;
            let into_size = layout::int_layout(into_ty).size;

            if enum_data.flag_set.contains(hir::EnumFlag::WithFields) {
                CastKind::Error
            } else {
                match from_size.cmp(&into_size) {
                    Ordering::Less => {
                        if from_ty.is_signed() {
                            CastKind::EnumS_Extend_to_Int
                        } else {
                            CastKind::EnumU_Extend_to_Int
                        }
                    }
                    Ordering::Equal => CastKind::Enum_NoOp_to_Int,
                    Ordering::Greater => CastKind::Enum_Trunc_to_Int,
                }
            }
        }
        (hir::Type::Reference(_, _), hir::Type::MultiReference(_, _))
        | (hir::Type::MultiReference(_, _), hir::Type::Reference(_, _)) => {
            if type_matches_coerced(ctx, into, from) {
                CastKind::Rawptr_NoOp
            } else {
                CastKind::Error
            }
        }
        _ => CastKind::Error,
    };

    if kind == CastKind::Error {
        let src = ctx.src(range);
        let from_ty = type_format(ctx, from);
        let into_ty = type_format(ctx, into);
        err::tycheck_cast_invalid(&mut ctx.emit, src, &from_ty, &into_ty);
        TypeResult::error()
    } else {
        if let hir::Expr::Const(target, _) = *target_res.expr {
            return if let Ok(value) = constfold_cast(ctx, range, target, into, kind) {
                TypeResult::new(into, hir::Expr::Const(value, hir::ConstID::dummy()))
            } else {
                TypeResult::error()
            };
        }
        let cast = hir::Expr::Cast { target: target_res.expr, into: ctx.arena.alloc(into), kind };
        TypeResult::new(into, cast)
    }
}

fn integer_cast_kind(from_ty: IntType, into_ty: IntType) -> hir::CastKind {
    use hir::CastKind;
    use std::cmp::Ordering;

    if from_ty == IntType::Untyped || into_ty == IntType::Untyped {
        return hir::CastKind::Int_NoOp;
    }

    let from_size = layout::int_layout(from_ty).size;
    let into_size = layout::int_layout(into_ty).size;
    match from_size.cmp(&into_size) {
        Ordering::Less => {
            if from_ty.is_signed() {
                CastKind::IntS_Extend
            } else {
                CastKind::IntU_Extend
            }
        }
        Ordering::Equal => CastKind::Int_NoOp,
        Ordering::Greater => CastKind::Int_Trunc,
    }
}

fn constfold_cast<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    range: TextRange,
    target: hir::ConstValue<'hir>,
    into: hir::Type<'hir>,
    kind: hir::CastKind,
) -> Result<hir::ConstValue<'hir>, ()> {
    use hir::CastKind;

    let src = ctx.src(range);
    match kind {
        CastKind::Error => Err(()),
        CastKind::Char_NoOp => {
            Ok(hir::ConstValue::from_u64(target.into_char() as u64, IntType::U32))
        }
        CastKind::UntypedChar_to_Int => {
            int_range_check(ctx, src, target.into_char() as i128, into.unwrap_int())
        }
        CastKind::Rawptr_NoOp => Ok(target),
        CastKind::Int_NoOp
        | CastKind::Int_Trunc
        | CastKind::IntS_Extend
        | CastKind::IntU_Extend => int_range_check(ctx, src, target.into_int(), into.unwrap_int()),
        CastKind::IntS_to_Float | CastKind::IntU_to_Float => Ok(hir::ConstValue::Float {
            val: target.into_int() as f64,
            float_ty: into.unwrap_float(),
        }),
        CastKind::Float_Trunc | CastKind::Float_Extend => {
            Ok(hir::ConstValue::Float { val: target.into_float(), float_ty: into.unwrap_float() })
        }
        CastKind::Float_to_IntS | CastKind::Float_to_IntU => {
            int_range_check(ctx, src, target.into_float() as i128, into.unwrap_int())
        }
        CastKind::Bool_Trunc | CastKind::Bool_Extend => {
            Ok(hir::ConstValue::Bool { val: target.into_bool(), bool_ty: into.unwrap_bool() })
        }
        CastKind::Bool_NoOp_to_Int | CastKind::Bool_Trunc_to_Int | CastKind::Bool_Extend_to_Int => {
            Ok(hir::ConstValue::from_u64(target.into_bool() as u64, into.unwrap_int()))
        }
        CastKind::StringUntyped_NoOp => Ok(hir::ConstValue::String {
            val: target.into_string(),
            string_ty: into.unwrap_string(),
        }),
        CastKind::Enum_NoOp_to_Int
        | CastKind::Enum_Trunc_to_Int
        | CastKind::EnumS_Extend_to_Int
        | CastKind::EnumU_Extend_to_Int => {
            let (enum_id, variant_id) = target.into_enum();
            let enum_data = ctx.registry.enum_data(enum_id);
            let variant = enum_data.variant(variant_id);
            let tag_value = match variant.kind {
                hir::VariantKind::Default(id) => ctx.registry.variant_eval(id).resolved()?,
                hir::VariantKind::Constant(id) => ctx.registry.const_eval(id).0.resolved()?,
            };
            int_range_check(ctx, src, tag_value.into_int(), into.unwrap_int())
        }
    }
}

fn typecheck_item<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    path: &ast::Path<'ast>,
    args_list: Option<&ast::ArgumentList<'ast>>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let (item_res, fields) = match check_path::path_resolve_value(ctx, path, false) {
        ValueID::None => {
            if let Some(arg_list) = args_list {
                default_check_arg_list(ctx, arg_list);
            }
            return TypeResult::error();
        }
        ValueID::Proc(proc_id, poly_types) => {
            if let Some(args_list) = args_list {
                return check_call_direct(ctx, expect, proc_id, poly_types, args_list, expr_range);
            } else {
                return check_item_procedure(ctx, expect, proc_id, poly_types, expr_range);
            }
        }
        ValueID::Enum(enum_id, variant_id, poly_types) => {
            return check_variant_input(
                ctx, expect, enum_id, variant_id, poly_types, args_list, expr_range,
            );
        }
        ValueID::Const(id, fields) => {
            let data = ctx.registry.const_data(id);
            let const_ty = data.ty.expect("typed const var");
            let (eval, _, _) = ctx.registry.const_eval(data.value);

            let res = if let Ok(value) = eval.resolved() {
                TypeResult::new(const_ty, hir::Expr::Const(value, id))
            } else {
                TypeResult::new(const_ty, hir::Expr::Error)
            };
            (res, fields)
        }
        ValueID::Global(id, fields) => (
            TypeResult::new(
                ctx.registry.global_data(id).ty,
                hir::Expr::GlobalVar { global_id: id },
            ),
            fields,
        ),
        ValueID::Param(id, fields) => (
            TypeResult::new(ctx.scope.local.param(id).ty, hir::Expr::ParamVar { param_id: id }),
            fields,
        ),
        ValueID::Variable(id, fields) => (
            TypeResult::new(ctx.scope.local.variable(id).ty, hir::Expr::Variable { var_id: id }),
            fields,
        ),
    };

    let mut target_res = item_res;

    for field in fields {
        let expr_res = target_res.into_expr_result(ctx);
        let field_res = check_field_from_type(ctx, field.name, expr_res.ty);
        target_res = emit_field_expr(ctx, expr_res.expr, field_res, field.name.range.start());
    }

    if let Some(args_list) = args_list {
        let expr_res = target_res.into_expr_result(ctx);
        let target_range =
            TextRange::new(expr_range.start(), path.segments.last().unwrap().name.range.end());
        check_call_indirect(ctx, target_range, expr_res, args_list)
    } else {
        target_res
    }
}

pub fn check_item_procedure<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation<'hir>,
    proc_id: hir::ProcID,
    poly_types: Option<&'hir [hir::Type<'hir>]>,
    range: TextRange,
) -> TypeResult<'hir> {
    //@creating proc type each time its encountered / called, waste of arena memory 25.05.24
    let data = ctx.registry.proc_data(proc_id);
    if data.flag_set.contains(hir::ProcFlag::Intrinsic) {
        let src = ctx.src(range);
        err::tycheck_intrinsic_proc_ptr(&mut ctx.emit, src, data.src());
        return TypeResult::error();
    }

    let (infer, is_poly) = ctx.scope.infer.start_context(data.poly_params);
    if let Some(poly_types) = poly_types {
        for (poly_idx, ty) in poly_types.iter().copied().enumerate() {
            ctx.scope.infer.resolve(infer, poly_idx, ty);
        }
    }
    if is_poly {
        if let Expectation::HasType(hir::Type::Procedure(proc_ty), _) = expect {
            types::apply_inference(
                ctx.scope.infer.inferred_mut(infer),
                proc_ty.return_ty,
                data.return_ty,
            );
            for (idx, param) in data.params.iter().enumerate() {
                if let Some(expect_param) = proc_ty.params.get(idx) {
                    types::apply_inference(
                        ctx.scope.infer.inferred_mut(infer),
                        expect_param.ty,
                        param.ty,
                    );
                }
            }
        }
    }

    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_unknown()) {
        let src = ctx.src(range);
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_error()) {
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    let poly_types = match poly_types {
        Some(poly_types) => poly_types,
        None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
    };

    let offset = ctx.cache.proc_ty_params.start();
    for param in data.params {
        let param_ty = type_substitute_inferred(ctx, is_poly, infer, param.ty);
        let param = hir::ProcTypeParam { ty: param_ty, kind: param.kind };
        ctx.cache.proc_ty_params.push(param);
    }
    let data = ctx.registry.proc_data(proc_id);
    let mut proc_ty = hir::ProcType {
        flag_set: data.flag_set,
        params: ctx.cache.proc_ty_params.take(offset, &mut ctx.arena),
        return_ty: type_substitute_inferred(ctx, is_poly, infer, data.return_ty),
    };
    ctx.scope.infer.end_context(infer);

    //clear unrelated flags
    proc_ty.flag_set.clear(hir::ProcFlag::InlineNever);
    proc_ty.flag_set.clear(hir::ProcFlag::InlineAlways);
    proc_ty.flag_set.clear(hir::ProcFlag::WasUsed);
    proc_ty.flag_set.clear(hir::ProcFlag::EntryPoint);

    let poly_types = if poly_types.is_empty() { None } else { Some(ctx.arena.alloc(poly_types)) };
    let proc_ty = hir::Type::Procedure(ctx.arena.alloc(proc_ty));
    let proc_value = hir::ConstValue::Procedure { proc_id, poly_types };
    let proc_expr = hir::Expr::Const(proc_value, hir::ConstID::dummy());
    TypeResult::new(proc_ty, proc_expr)
}

fn typecheck_variant<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    args_list: Option<&ast::ArgumentList<'ast>>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let name_src = ctx.src(name.range);
    let (enum_id, poly_types) = match infer_enum_type(ctx, expect, name_src) {
        Some(found) => found,
        None => {
            if let Some(arg_list) = args_list {
                default_check_arg_list(ctx, arg_list);
            }
            return TypeResult::error();
        }
    };
    let variant_id = match scope::check_find_enum_variant(ctx, enum_id, name) {
        Some(found) => found,
        None => {
            if let Some(arg_list) = args_list {
                default_check_arg_list(ctx, arg_list);
            }
            return TypeResult::error();
        }
    };

    check_variant_input(ctx, expect, enum_id, variant_id, poly_types, args_list, expr_range)
}

fn typecheck_struct_init<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    struct_init: &ast::StructInit<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    fn error_range(struct_init: &ast::StructInit, expr_range: TextRange) -> TextRange {
        if struct_init.input.is_empty() {
            expr_range
        } else {
            TextRange::new(expr_range.start(), struct_init.input_start)
        }
    }

    let struct_res = match struct_init.path {
        Some(path) => check_path::path_resolve_struct(ctx, path, false),
        None => {
            let src = ctx.src(error_range(struct_init, expr_range));
            infer_struct_type(ctx, expect, src)
        }
    };
    let (struct_id, poly_types) = match struct_res {
        Some(res) => {
            //infering poly types if missing on path
            if res.1.is_none() {
                match infer_struct_poly_types(res.0, expect) {
                    Some(poly_types) => (res.0, Some(poly_types)),
                    None => res,
                }
            } else {
                res
            }
        }
        None => {
            default_check_field_init_list(ctx, struct_init.input);
            return TypeResult::error();
        }
    };

    let data = ctx.registry.struct_data(struct_id);
    let field_count = data.fields.len();
    let (infer, is_poly) = ctx.scope.infer.start_context(data.poly_params);

    if let Some(poly_types) = poly_types {
        for (poly_idx, ty) in poly_types.iter().copied().enumerate() {
            ctx.scope.infer.resolve(infer, poly_idx, ty);
        }
    }

    enum FieldStatus {
        None,
        Init(TextRange),
    }

    //@re-use FieldStatus memory
    let offset = ctx.cache.exprs.start();
    let mut field_status = Vec::<FieldStatus>::new();
    field_status.resize_with(field_count, || FieldStatus::None);
    let mut init_count: usize = 0;

    let error_count = ctx.emit.error_count();
    let mut out_of_order_init = None;

    for (idx, input) in struct_init.input.iter().enumerate() {
        let field_id = match scope::check_find_struct_field(ctx, struct_id, input.name) {
            Some(found) => found,
            None => {
                let _ = typecheck_expr(ctx, Expectation::ERROR, input.expr);
                continue;
            }
        };

        if idx != field_id.index() && out_of_order_init.is_none() {
            out_of_order_init = Some(idx)
        }

        let data = ctx.registry.struct_data(struct_id);
        let field = data.field(field_id);

        let expect_src = SourceRange::new(data.origin_id, field.ty_range);
        let field_ty = type_substitute_inferred(ctx, is_poly, infer, field.ty);
        let expect = Expectation::HasType(field_ty, Some(expect_src));

        let input_res = typecheck_expr(ctx, expect, input.expr);
        if is_poly {
            types::apply_inference(ctx.scope.infer.inferred_mut(infer), input_res.ty, field.ty);
        }

        if let FieldStatus::Init(range) = field_status[field_id.index()] {
            let src = ctx.src(input.name.range);
            let prev_src = ctx.src(range);
            let field_name = ctx.name(input.name.id);
            err::tycheck_field_already_initialized(&mut ctx.emit, src, prev_src, field_name);
        } else {
            ctx.cache.exprs.push(input_res.expr);
            field_status[field_id.index()] = FieldStatus::Init(input.name.range);
            init_count += 1;
        }
    }

    if init_count < field_count {
        let data = ctx.registry.struct_data(struct_id);
        let missing_count = field_count - init_count;
        let mut mentioned = 0_usize;
        let mut message = String::with_capacity(128);
        message.push_str("missing field initializers: ");

        for (idx, status) in field_status.iter().enumerate() {
            if let FieldStatus::None = status {
                let field_id = hir::FieldID::new(idx);
                let field = data.field(field_id);

                message.push('`');
                message.push_str(ctx.name(field.name.id));
                message.push('`');
                mentioned += 1;

                if mentioned >= 5 {
                    let remaining = missing_count - mentioned;
                    if remaining > 0 {
                        use std::fmt::Write;
                        let _ = write!(message, " and {remaining} more...");
                    }
                    break;
                } else if mentioned < missing_count {
                    message.push(',');
                    message.push(' ');
                }
            }
        }

        let src = ctx.src(error_range(struct_init, expr_range));
        let struct_src = SourceRange::new(data.origin_id, data.name.range);
        err::tycheck_missing_field_initializers(&mut ctx.emit, src, struct_src, message);
    } else if !ctx.emit.did_error(error_count) {
        if let Some(idx) = out_of_order_init {
            let data = ctx.registry.struct_data(struct_id);
            let field_init = struct_init.input[idx];
            let field_id = hir::FieldID::new(idx);

            let src = ctx.src(field_init.name.range);
            let init_name = ctx.name(field_init.name.id);
            let expect_name = ctx.name(data.field(field_id).name.id);
            err::tycheck_field_init_out_of_order(&mut ctx.emit, src, init_name, expect_name)
        }
    }

    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_unknown()) {
        let src = ctx.src(error_range(struct_init, expr_range));
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_error()) {
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    let poly_types = match poly_types {
        Some(poly_types) => poly_types,
        None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
    };
    ctx.scope.infer.end_context(infer);

    let expr = if ctx.in_const > 0 {
        constfold_struct_init(ctx, struct_init.input, offset, struct_id, poly_types)
    } else {
        let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
        if poly_types.is_empty() {
            hir::Expr::StructInit { struct_id, input }
        } else {
            hir::Expr::StructInitPoly { struct_id, input: ctx.arena.alloc((input, poly_types)) }
        }
    };
    TypeResult::new(hir::Type::Struct(struct_id, poly_types), expr)
}

fn constfold_struct_init<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    input: &[ast::FieldInit],
    offset: TempOffset<&'hir hir::Expr<'hir>>,
    struct_id: hir::StructID,
    poly_types: &'hir [hir::Type<'hir>],
) -> hir::Expr<'hir> {
    let mut valid = true;
    let const_offset = ctx.cache.const_values.start();

    for (idx, &field) in ctx.cache.exprs.view(offset).iter().enumerate() {
        match field {
            hir::Expr::Error => valid = false,
            hir::Expr::Const(value, _) => ctx.cache.const_values.push(*value),
            _ => {
                valid = false;
                let src = ctx.src(input[idx].expr.range);
                err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
            }
        }
    }
    ctx.cache.exprs.pop_view(offset);

    if valid {
        let values = ctx.cache.const_values.take(const_offset, &mut ctx.arena);
        let struct_ = hir::ConstStruct { values, poly_types };
        let struct_ = hir::ConstValue::Struct { struct_id, struct_: ctx.arena.alloc(struct_) };
        hir::Expr::Const(struct_, hir::ConstID::dummy())
    } else {
        ctx.cache.const_values.pop_view(const_offset);
        hir::Expr::Error
    }
}

fn typecheck_array_init<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    range: TextRange,
    input: &[ast::ArrayInit<'ast>],
) -> TypeResult<'hir> {
    let error_count = ctx.emit.error_count();
    let mut elem_ty = None;
    let mut arr_enum_id = None;
    let mut expect_enum = expect.infer_array_enum();
    let mut expect = expect.infer_array_elem();

    let offset = ctx.cache.exprs.start();
    for (idx, init) in input.iter().copied().enumerate() {
        if let Some(variant) = init.variant {
            check_array_variant(ctx, &mut arr_enum_id, &mut expect_enum, idx, variant);
        }
        let expr_res = typecheck_expr(ctx, expect, init.expr);
        ctx.cache.exprs.push(expr_res.expr);

        // stop expecting when errored
        let did_error = ctx.emit.did_error(error_count);
        if did_error {
            expect = Expectation::ERROR;
        }
        // set elem_ty and expect to first non-error type
        if elem_ty.is_none() && !expr_res.ty.is_error() {
            elem_ty = Some(expr_res.ty);
            if matches!(expect, Expectation::None) && !did_error {
                expect = Expectation::HasType(expr_res.ty, Some(ctx.src(init.expr.range)));
            }
        }
    }

    if elem_ty.is_none() {
        elem_ty = expect.inner_type();
    }
    let elem_ty = match elem_ty {
        Some(elem_ty) => elem_ty,
        None => {
            if input.is_empty() {
                let src = ctx.src(range);
                err::tycheck_cannot_infer_empty_array(&mut ctx.emit, src);
            }
            ctx.cache.exprs.pop_view(offset);
            return TypeResult::error();
        }
    };

    if let Some(enum_id) = arr_enum_id {
        if !super::pass_3::check_enumerated_array_type(ctx, enum_id, range) {
            return TypeResult::error();
        }
        let init_count = input.iter().filter(|init| init.variant.is_some()).count() as isize;
        let expected = ctx.registry.enum_data(enum_id).variants.len() as isize;
        let missing = expected - init_count;
        if missing > 0 {
            let src = ctx.src(TextRange::new(range.end() - 1.into(), range.end()));
            err::tycheck_missing_variant_inits(&mut ctx.emit, src, missing as usize);
            return TypeResult::error();
        }
    }

    let expr = if ctx.in_const > 0 {
        constfold_array_init(ctx, input, offset, elem_ty)
    } else {
        let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
        let array = ctx.arena.alloc(hir::ArrayInit { elem_ty, input });
        hir::Expr::ArrayInit { array }
    };

    let array_ty = if let Some(enum_id) = arr_enum_id {
        let array = ctx.arena.alloc(hir::ArrayEnumerated { enum_id, elem_ty });
        hir::Type::ArrayEnumerated(array)
    } else {
        let len = hir::ArrayStaticLen::Immediate(input.len() as u64);
        let array = ctx.arena.alloc(hir::ArrayStatic { len, elem_ty });
        hir::Type::ArrayStatic(array)
    };
    TypeResult::new(array_ty, expr)
}

fn check_array_variant<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    arr_enum_id: &mut Option<hir::EnumID>,
    expect_enum: &mut Expectation<'hir>,
    init_idx: usize,
    variant: ast::ConstExpr<'ast>,
) {
    let (variant_res, variant_ty) = pass_4::resolve_const_expr(ctx, *expect_enum, variant);

    if let hir::Type::Enum(enum_id, _) = variant_ty {
        if arr_enum_id.is_none() {
            *arr_enum_id = Some(enum_id);
        }
        if matches!(expect_enum, Expectation::None) {
            *expect_enum = Expectation::HasType(variant_ty, Some(ctx.src(variant.0.range)))
        }
    }
    let Ok(value) = variant_res else {
        return;
    };
    let (enum_id, variant_id) = match value {
        hir::ConstValue::Variant { enum_id, variant_id } => (enum_id, variant_id),
        hir::ConstValue::VariantPoly { enum_id, variant } => (enum_id, variant.variant_id),
        _ => {
            let src = ctx.src(variant.0.range);
            err::tycheck_expected_variant_value(&mut ctx.emit, src);
            return;
        }
    };
    if arr_enum_id.unwrap() != enum_id {
        return;
    }
    let data = ctx.registry.enum_data(enum_id);
    if init_idx >= data.variants.len() {
        let src = ctx.src(variant.0.range);
        err::tycheck_unexpected_variant_value(&mut ctx.emit, src);
    } else if init_idx != variant_id.index() {
        let src = ctx.src(variant.0.range);
        let name = ctx.name(data.variant(hir::VariantID::new(init_idx)).name.id);
        err::tycheck_expected_variant(&mut ctx.emit, src, name);
    }
}

fn constfold_array_init<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    input: &[ast::ArrayInit],
    offset: TempOffset<&'hir hir::Expr<'hir>>,
    elem_ty: hir::Type<'hir>,
) -> hir::Expr<'hir> {
    if ctx.cache.exprs.view(offset).is_empty() {
        let array = hir::ConstValue::ArrayEmpty { elem_ty: ctx.arena.alloc(elem_ty) };
        return hir::Expr::Const(array, hir::ConstID::dummy());
    }

    let mut valid = true;
    let const_offset = ctx.cache.const_values.start();

    for (idx, value) in ctx.cache.exprs.view(offset).iter().enumerate() {
        match value {
            hir::Expr::Error => valid = false,
            hir::Expr::Const(value, _) => ctx.cache.const_values.push(*value),
            _ => {
                valid = false;
                let src = ctx.src(input[idx].expr.range);
                err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
            }
        }
    }
    ctx.cache.exprs.pop_view(offset);

    if valid {
        let values = ctx.cache.const_values.take(const_offset, &mut ctx.arena);
        let array = hir::ConstArray { values };
        let array = hir::ConstValue::Array { array: ctx.arena.alloc(array) };
        hir::Expr::Const(array, hir::ConstID::dummy())
    } else {
        ctx.cache.const_values.pop_view(const_offset);
        hir::Expr::Error
    }
}

fn typecheck_array_repeat<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    value: &ast::Expr<'ast>,
    len: ast::ConstExpr<'ast>,
) -> TypeResult<'hir> {
    let expect = expect.infer_array_elem();
    let value_res = typecheck_expr(ctx, expect, value);
    let (len_res, _) = pass_4::resolve_const_expr(ctx, Expectation::USIZE, len);

    let len = match len_res {
        Ok(value) => value.into_int_u64(),
        Err(_) => return TypeResult::error(),
    };

    let expr = if let hir::Expr::Const(value, _) = *value_res.expr {
        let array = hir::ConstArrayRepeat { len, value };
        let value = hir::ConstValue::ArrayRepeat { array: ctx.arena.alloc(array) };
        hir::Expr::Const(value, hir::ConstID::dummy())
    } else {
        let array = hir::ArrayRepeat { elem_ty: value_res.ty, value: value_res.expr, len };
        hir::Expr::ArrayRepeat { array: ctx.arena.alloc(array) }
    };

    let len = hir::ArrayStaticLen::Immediate(len);
    let array_ty = hir::ArrayStatic { len, elem_ty: value_res.ty };
    let array_ty = ctx.arena.alloc(array_ty);
    TypeResult::new(hir::Type::ArrayStatic(array_ty), expr)
}

//@fix errors, fix edge cases, when new ir is ready.
fn typecheck_try<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expr: &ast::Expr<'ast>,
    range: TextRange,
) -> TypeResult<'hir> {
    let return_type = ctx.scope.local.return_expect.inner_type().unwrap();
    let try_enum_id;
    let ret_enum_poly;

    let try_expect = match return_type {
        hir::Type::Error => return TypeResult::error(),
        hir::Type::Enum(enum_id, poly) => {
            if enum_id == ctx.core.result {
                try_enum_id = enum_id;
                ret_enum_poly = poly;
                match ctx.scope.local.return_expect {
                    Expectation::None => unreachable!(),
                    Expectation::HasType(_, ret_src) => {
                        let expect_poly = ctx.arena.alloc_slice(&[hir::Type::Unknown, poly[1]]);
                        Expectation::HasType(hir::Type::Enum(ctx.core.result, expect_poly), ret_src)
                    }
                }
            } else if enum_id == ctx.core.option {
                try_enum_id = enum_id;
                ret_enum_poly = poly;
                match ctx.scope.local.return_expect {
                    Expectation::None => unreachable!(),
                    Expectation::HasType(_, ret_src) => {
                        let expect_poly = ctx.arena.alloc_slice(&[hir::Type::Unknown]);
                        Expectation::HasType(hir::Type::Enum(ctx.core.option, expect_poly), ret_src)
                    }
                }
            } else {
                let src = ctx.src(range);
                err::tycheck_type_mismatch(
                    &mut ctx.emit,
                    src,
                    None,
                    "try: Result",
                    "something else",
                );
                return TypeResult::error();
            }
        }
        _ => {
            let src = ctx.src(range);
            err::tycheck_type_mismatch(&mut ctx.emit, src, None, "try: Result", "something else");
            return TypeResult::error();
        }
    };

    let expr_res = typecheck_expr(ctx, try_expect, expr);
    let ok_ty = match expr_res.ty {
        hir::Type::Enum(enum_id, poly) => {
            if enum_id == ctx.core.option || enum_id == ctx.core.result {
                poly.first().copied().unwrap_or(hir::Type::Error)
            } else {
                hir::Type::Error
            }
        }
        _ => hir::Type::Error,
    };

    let ok_arm = {
        let ok_var = hir::Variable {
            mutt: ast::Mut::Immutable,
            name: ast::Name { id: ctx.session.intern_name.intern("_"), range },
            ty: ok_ty,
            was_used: true,
        };
        let ok_id = ctx.scope.local.add_variable(ok_var);
        let binds = ctx.arena.alloc_slice(&[ok_id]);
        let pat = if try_enum_id == ctx.core.result {
            hir::Pat::Variant(try_enum_id, hir::VariantID::new(0), binds)
        } else {
            hir::Pat::Variant(try_enum_id, hir::VariantID::new(1), binds)
        };

        let ok_expr = ctx.arena.alloc(hir::Expr::Variable { var_id: ok_id });
        let stmts = ctx.arena.alloc_slice(&[hir::Stmt::ExprTail(ok_expr)]);

        hir::MatchArm { pat, block: hir::Block { stmts } }
    };

    let err_arm = {
        let (pat, ret_expr) = if try_enum_id == ctx.core.result {
            let var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: ast::Name { id: ctx.session.intern_name.intern("_"), range },
                ty: ret_enum_poly[1],
                was_used: true,
            };
            let var_id = ctx.scope.local.add_variable(var);
            let binds = ctx.arena.alloc_slice(&[var_id]);

            let var_expr = ctx.arena.alloc(hir::Expr::Variable { var_id });
            let inputs = ctx.arena.alloc_slice(&[var_expr]);
            let input = ctx.arena.alloc((inputs, ret_enum_poly));
            let variant = hir::Expr::Variant {
                enum_id: try_enum_id,
                variant_id: hir::VariantID::new(1),
                input,
            };
            let pat = hir::Pat::Variant(try_enum_id, hir::VariantID::new(1), binds);
            (pat, ctx.arena.alloc(variant))
        } else {
            let input = ctx.arena.alloc((&([][..]), ret_enum_poly));
            let variant = hir::Expr::Variant {
                enum_id: try_enum_id,
                variant_id: hir::VariantID::new(0),
                input,
            };
            let pat = hir::Pat::Variant(try_enum_id, hir::VariantID::new(0), &[]);
            (pat, ctx.arena.alloc(variant))
        };

        let offset = ctx.cache.stmts.start();
        for block in ctx.scope.local.defer_blocks_all().iter().copied().rev() {
            let block_expr = ctx.arena.alloc(hir::Expr::Block { block });
            ctx.cache.stmts.push(hir::Stmt::ExprSemi(block_expr));
        }
        ctx.cache.stmts.push(hir::Stmt::Return(Some(ret_expr)));
        let stmts = ctx.cache.stmts.take(offset, &mut ctx.arena);

        hir::MatchArm { pat, block: hir::Block { stmts } }
    };

    let poly_types = match expr_res.ty {
        hir::Type::Enum(_, poly) => poly,
        _ => &[],
    };
    let match_ = hir::Match {
        kind: hir::MatchKind::Enum {
            enum_id: try_enum_id,
            ref_mut: None,
            poly_types: Some(ctx.arena.alloc(poly_types)),
        },
        on_expr: expr_res.expr,
        arms: ctx.arena.alloc_slice(&[ok_arm, err_arm]),
    };
    let match_ = ctx.arena.alloc(match_);
    TypeResult::new(ok_ty, hir::Expr::Match { match_ })
}

fn typecheck_deref<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    rhs: &ast::Expr<'ast>,
    range: TextRange,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, Expectation::None, rhs);

    match rhs_res.ty {
        hir::Type::Error => TypeResult::error(),
        hir::Type::Reference(mutt, ref_ty) => {
            TypeResult::new(*ref_ty, hir::Expr::Deref { rhs: rhs_res.expr, mutt, ref_ty })
        }
        _ => {
            let src = ctx.src(range);
            let ty_fmt = type_format(ctx, rhs_res.ty);
            err::tycheck_cannot_deref_on_ty(&mut ctx.emit, src, &ty_fmt);
            TypeResult::error()
        }
    }
}

fn typecheck_address<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    mutt: ast::Mut,
    rhs: &ast::Expr<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, expect.infer_ref_ty(), rhs);
    let addr_res = resolve_expr_addressability(ctx, rhs_res.expr);
    check_address_addressability(ctx, mutt, &addr_res, expr_range);

    let ref_ty = ctx.arena.alloc(rhs_res.ty);
    let ref_ty = hir::Type::Reference(mutt, ref_ty);
    TypeResult::new(ref_ty, hir::Expr::Address { rhs: rhs_res.expr })
}

fn typecheck_unary<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: TextRange,
    op: ast::UnOp,
    op_range: TextRange,
    rhs: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr_untyped(ctx, Expectation::None, rhs);

    if rhs_res.ty.is_error() {
        return TypeResult::error();
    }

    let hir_op = match op {
        ast::UnOp::Neg => match rhs_res.ty {
            hir::Type::Int(int_ty) if int_ty.is_signed() || int_ty == IntType::Untyped => {
                Ok(hir::UnOp::Neg_Int(int_ty))
            }
            hir::Type::Float(float_ty) => Ok(hir::UnOp::Neg_Float(float_ty)),
            _ => Err(()),
        },
        ast::UnOp::BitNot => match rhs_res.ty {
            hir::Type::Int(IntType::Untyped) => Err(()),
            hir::Type::Int(int_ty) => Ok(hir::UnOp::BitNot(int_ty)),
            _ => Err(()),
        },
        ast::UnOp::LogicNot => match rhs_res.ty {
            hir::Type::Bool(bool_ty) => Ok(hir::UnOp::LogicNot(bool_ty)),
            _ => Err(()),
        },
    };

    if let Ok(hir_op) = hir_op {
        if let hir::Expr::Const(rhs, _) = *rhs_res.expr {
            return if let Ok(value) = constfold_unary(ctx, range, hir_op, rhs) {
                TypeResult::new(rhs_res.ty, hir::Expr::Const(value, hir::ConstID::dummy()))
            } else {
                TypeResult::error()
            };
        }
        let unary = hir::Expr::Unary { op: hir_op, rhs: rhs_res.expr };
        TypeResult::new(rhs_res.ty, unary)
    } else {
        let src = ctx.src(op_range);
        let rhs_ty = type_format(ctx, rhs_res.ty);
        err::tycheck_un_op_cannot_apply(&mut ctx.emit, src, op.as_str(), &rhs_ty);
        TypeResult::error()
    }
}

fn constfold_unary<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    range: TextRange,
    op: hir::UnOp,
    rhs: hir::ConstValue<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let src = ctx.src(range);
    match op {
        hir::UnOp::Neg_Int(int_ty) => {
            let val = -rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::UnOp::Neg_Float(float_ty) => {
            let val = -rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::UnOp::BitNot(int_ty) => {
            if int_ty.is_signed() {
                Ok(hir::ConstValue::from_i64(!rhs.into_int_i64(), int_ty))
            } else {
                let value_bits = layout::int_layout(int_ty).size * 8;
                let mask = if value_bits == 64 { u64::MAX } else { (1 << value_bits) - 1 };
                Ok(hir::ConstValue::from_u64(mask & !rhs.into_int_u64(), int_ty))
            }
        }
        hir::UnOp::LogicNot(bool_ty) => {
            let (val, _) = rhs.expect_bool();
            Ok(hir::ConstValue::Bool { val: !val, bool_ty })
        }
    }
}

fn typecheck_binary<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    range: TextRange,
    op: ast::BinOp,
    op_start: TextOffset,
    lhs: &ast::Expr<'ast>,
    rhs: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    //@bidirectional enum / struct type inference doesnt work, lhs biased.
    let mut lhs_res = typecheck_expr_untyped(ctx, Expectation::None, lhs);
    let rhs_expect = match lhs_res.ty {
        hir::Type::Enum(..) => Expectation::HasType(lhs_res.ty, Some(ctx.src(lhs.range))),
        _ => Expectation::None,
    };
    let mut rhs_res = typecheck_expr_untyped(ctx, rhs_expect, rhs);

    if lhs_res.ty.is_error() || rhs_res.ty.is_error() {
        return TypeResult::error();
    }

    let lhs_promote =
        promote_untyped(ctx, lhs.range, *lhs_res.expr, &mut lhs_res.ty, Some(rhs_res.ty), false);
    let rhs_promote =
        promote_untyped(ctx, rhs.range, *rhs_res.expr, &mut rhs_res.ty, Some(lhs_res.ty), false);

    match lhs_promote {
        Some(Ok((v, id))) => lhs_res.expr = ctx.arena.alloc(hir::Expr::Const(v, id)),
        Some(Err(())) => return TypeResult::error(),
        None => {}
    }
    match rhs_promote {
        Some(Ok((v, id))) => rhs_res.expr = ctx.arena.alloc(hir::Expr::Const(v, id)),
        Some(Err(())) => return TypeResult::error(),
        None => {}
    }

    let (hir_op, array) =
        match check_binary_op(ctx, expect, op, op_start, lhs_res.clone(), rhs_res.clone()) {
            Ok(hir_op) => hir_op,
            Err(()) => return TypeResult::error(),
        };
    let res_ty = match op {
        ast::BinOp::Eq
        | ast::BinOp::NotEq
        | ast::BinOp::Less
        | ast::BinOp::LessEq
        | ast::BinOp::Greater
        | ast::BinOp::GreaterEq => hir::Type::Bool(expect.infer_bool()),
        _ => lhs_res.ty,
    };

    if let hir::Expr::Const(rhsv, _) = *rhs_res.expr {
        if check_binary_const_rhs(ctx, rhs.range, hir_op, rhsv).is_err() {
            return TypeResult::new(res_ty, hir::Expr::Error);
        };
        if let hir::Expr::Const(lhsv, _) = *lhs_res.expr {
            return if let Ok(value) = constfold_binary(ctx, range, hir_op, lhsv, rhsv) {
                TypeResult::new(res_ty, hir::Expr::Const(value, hir::ConstID::dummy()))
            } else {
                TypeResult::new(res_ty, hir::Expr::Error)
            };
        }
    }

    let binary = if let Some(array) = array {
        hir::Expr::ArrayBinary { op: hir_op, array }
    } else {
        hir::Expr::Binary { op: hir_op, lhs: lhs_res.expr, rhs: rhs_res.expr }
    };
    TypeResult::new(res_ty, binary)
}

fn check_binary_op<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation,
    op: ast::BinOp,
    op_start: TextOffset,
    lhs: ExprResult<'hir>,
    rhs: ExprResult<'hir>,
) -> Result<(hir::BinOp, Option<&'hir hir::ArrayBinary<'hir>>), ()> {
    if lhs.ty.is_error() || rhs.ty.is_error() {
        return Err(());
    }
    if op != ast::BinOp::BitShl
        && op != ast::BinOp::BitShr
        && !type_matches_coerced(ctx, lhs.ty, rhs.ty)
    {
        let op_len = op.as_str().len() as u32;
        let src = ctx.src(TextRange::new(op_start, op_start + op_len.into()));
        let lhs_ty = type_format(ctx, lhs.ty);
        let rhs_ty = type_format(ctx, rhs.ty);
        err::tycheck_bin_type_mismatch(&mut ctx.emit, src, &lhs_ty, &rhs_ty);
        return Err(());
    }

    let lhs_ty = match lhs.ty {
        hir::Type::ArrayStatic(array) => array.elem_ty,
        hir::Type::ArrayEnumerated(array) => array.elem_ty,
        other => other,
    };
    let rhs_ty = match rhs.ty {
        hir::Type::ArrayStatic(array) => array.elem_ty,
        hir::Type::ArrayEnumerated(array) => array.elem_ty,
        other => other,
    };

    let hir_op = match op {
        ast::BinOp::Add => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Add_Int(int_ty)),
            hir::Type::Float(float_ty) => Ok(hir::BinOp::Add_Float(float_ty)),
            _ => Err(()),
        },
        ast::BinOp::Sub => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Sub_Int(int_ty)),
            hir::Type::Float(float_ty) => Ok(hir::BinOp::Sub_Float(float_ty)),
            _ => Err(()),
        },
        ast::BinOp::Mul => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Mul_Int(int_ty)),
            hir::Type::Float(float_ty) => Ok(hir::BinOp::Mul_Float(float_ty)),
            _ => Err(()),
        },
        ast::BinOp::Div => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Div_Int(int_ty)),
            hir::Type::Float(float_ty) => Ok(hir::BinOp::Div_Float(float_ty)),
            _ => Err(()),
        },
        ast::BinOp::Rem => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Rem_Int(int_ty)),
            _ => Err(()),
        },
        ast::BinOp::BitAnd => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::BitAnd(int_ty)),
            _ => Err(()),
        },
        ast::BinOp::BitOr => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::BitOr(int_ty)),
            _ => Err(()),
        },
        ast::BinOp::BitXor => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::BitXor(int_ty)),
            _ => Err(()),
        },
        ast::BinOp::BitShl => match (lhs_ty, rhs_ty) {
            (hir::Type::Int(lhs_ty), hir::Type::Int(rhs_ty)) => {
                let cast = integer_cast_kind(rhs_ty, lhs_ty);
                Ok(hir::BinOp::BitShl(lhs_ty, cast))
            }
            _ => Err(()),
        },
        ast::BinOp::BitShr => match (lhs_ty, rhs_ty) {
            (hir::Type::Int(lhs_ty), hir::Type::Int(rhs_ty)) => {
                let cast = integer_cast_kind(rhs_ty, lhs_ty);
                Ok(hir::BinOp::BitShr(lhs_ty, cast))
            }
            _ => Err(()),
        },
        ast::BinOp::Eq => match lhs_ty {
            hir::Type::Char
            | hir::Type::UntypedChar
            | hir::Type::Rawptr
            | hir::Type::Bool(_)
            | hir::Type::Reference(_, _)
            | hir::Type::MultiReference(_, _)
            | hir::Type::Procedure(_) => Ok(hir::BinOp::Eq_Int_Other(expect.infer_bool())),
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::Eq, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::Eq, expect.infer_bool(), float_ty))
            }
            hir::Type::String(string_ty) => {
                Ok(hir::BinOp::Cmp_String(CmpPred::Eq, expect.infer_bool(), string_ty))
            }
            hir::Type::Enum(enum_id, _) => {
                let data = ctx.registry.enum_data(enum_id);
                if !data.flag_set.contains(hir::EnumFlag::WithFields) {
                    Ok(hir::BinOp::Eq_Int_Other(expect.infer_bool()))
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        },
        ast::BinOp::NotEq => match lhs_ty {
            hir::Type::Char
            | hir::Type::UntypedChar
            | hir::Type::Rawptr
            | hir::Type::Bool(_)
            | hir::Type::Reference(_, _)
            | hir::Type::MultiReference(_, _)
            | hir::Type::Procedure(_) => Ok(hir::BinOp::NotEq_Int_Other(expect.infer_bool())),
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::NotEq, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::NotEq, expect.infer_bool(), float_ty))
            }
            hir::Type::String(string_ty) => {
                Ok(hir::BinOp::Cmp_String(CmpPred::NotEq, expect.infer_bool(), string_ty))
            }
            hir::Type::Enum(enum_id, _) => {
                let data = ctx.registry.enum_data(enum_id);
                if !data.flag_set.contains(hir::EnumFlag::WithFields) {
                    Ok(hir::BinOp::NotEq_Int_Other(expect.infer_bool()))
                } else {
                    Err(())
                }
            }
            _ => Err(()),
        },
        ast::BinOp::Less => match lhs_ty {
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::Less, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::Less, expect.infer_bool(), float_ty))
            }
            _ => Err(()),
        },
        ast::BinOp::LessEq => match lhs_ty {
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::LessEq, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::LessEq, expect.infer_bool(), float_ty))
            }
            _ => Err(()),
        },
        ast::BinOp::Greater => match lhs_ty {
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::Greater, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::Greater, expect.infer_bool(), float_ty))
            }
            _ => Err(()),
        },
        ast::BinOp::GreaterEq => match lhs_ty {
            hir::Type::Int(int_ty) => {
                Ok(hir::BinOp::Cmp_Int(CmpPred::GreaterEq, expect.infer_bool(), int_ty))
            }
            hir::Type::Float(float_ty) => {
                Ok(hir::BinOp::Cmp_Float(CmpPred::GreaterEq, expect.infer_bool(), float_ty))
            }
            _ => Err(()),
        },
        ast::BinOp::LogicAnd => match lhs_ty {
            hir::Type::Bool(bool_ty) => Ok(hir::BinOp::LogicAnd(bool_ty)),
            _ => Err(()),
        },
        ast::BinOp::LogicOr => match lhs_ty {
            hir::Type::Bool(bool_ty) => Ok(hir::BinOp::LogicOr(bool_ty)),
            _ => Err(()),
        },
    };

    let array_info = match lhs.ty {
        hir::Type::ArrayStatic(array) => Some(ctx.array_len(array.len).unwrap_or(0)),
        hir::Type::ArrayEnumerated(array) => {
            Some(ctx.registry.enum_data(array.enum_id).variants.len() as u64)
        }
        _ => None,
    };
    let array = if let Some(len) = array_info {
        let array = hir::ArrayBinary { len, lhs: lhs.expr, rhs: rhs.expr };
        Some(ctx.arena.alloc(array))
    } else {
        None
    };

    let array_op_banned = array.is_some()
        && !matches!(
            op,
            ast::BinOp::Add
                | ast::BinOp::Sub
                | ast::BinOp::Mul
                | ast::BinOp::Div
                | ast::BinOp::Rem
                | ast::BinOp::BitAnd
                | ast::BinOp::BitOr
                | ast::BinOp::BitXor,
        );

    if hir_op.is_err() || array_op_banned {
        let op_len = op.as_str().len() as u32;
        let src = ctx.src(TextRange::new(op_start, op_start + op_len.into()));
        let lhs_ty = type_format(ctx, lhs.ty);
        let rhs_ty = type_format(ctx, rhs.ty);
        err::tycheck_bin_cannot_apply(&mut ctx.emit, src, op.as_str(), &lhs_ty, &rhs_ty);
    }

    hir_op.map(|hir_op| (hir_op, array))
}

fn check_binary_const_rhs(
    ctx: &mut HirCtx,
    range: TextRange,
    op: hir::BinOp,
    rhs: hir::ConstValue,
) -> Result<(), ()> {
    if let hir::BinOp::Div_Int(_) | hir::BinOp::Rem_Int(_) = op {
        if rhs.into_int() == 0 {
            let src = ctx.src(range);
            err::const_int_div_by_zero(&mut ctx.emit, src);
            return Err(());
        }
    }

    let shift_ty = match op {
        hir::BinOp::BitShl(int_ty, _) => Some(int_ty),
        hir::BinOp::BitShr(int_ty, _) => Some(int_ty),
        _ => None,
    };

    if let Some(int_ty) = shift_ty {
        let val = rhs.into_int();
        let layout = if int_ty == IntType::Untyped {
            hir::Layout::equal(8)
        } else {
            layout::int_layout(int_ty)
        };
        let max = (layout.size * 8 - 1) as i128;
        if val < 0 || val > max {
            let src = ctx.src(range);
            err::const_shift_out_of_range(&mut ctx.emit, src, op.as_str(), val, 0, max);
            return Err(());
        }
    }

    Ok(())
}

fn constfold_binary<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    range: TextRange,
    op: hir::BinOp,
    lhs: hir::ConstValue<'hir>,
    rhs: hir::ConstValue<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let src = ctx.src(range);
    match op {
        hir::BinOp::Add_Int(int_ty) => {
            let val = lhs.into_int() + rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::Add_Float(float_ty) => {
            let val = lhs.into_float() + rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Sub_Int(int_ty) => {
            let val = lhs.into_int() - rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::Sub_Float(float_ty) => {
            let val = lhs.into_float() - rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Mul_Int(int_ty) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_mul(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Mul_Float(float_ty) => {
            let val = lhs.into_float() * rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Div_Int(int_ty) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_div(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Div_Float(float_ty) => {
            let val = lhs.into_float() / rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Rem_Int(int_ty) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_rem(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::BitAnd(int_ty) => {
            if int_ty.is_signed() {
                let val = lhs.into_int_i64() & rhs.into_int_i64();
                Ok(hir::ConstValue::from_i64(val, int_ty))
            } else {
                let val = lhs.into_int_u64() & rhs.into_int_u64();
                Ok(hir::ConstValue::from_u64(val, int_ty))
            }
        }
        hir::BinOp::BitOr(int_ty) => {
            if int_ty.is_signed() {
                let val = lhs.into_int_i64() | rhs.into_int_i64();
                Ok(hir::ConstValue::from_i64(val, int_ty))
            } else {
                let val = lhs.into_int_u64() | rhs.into_int_u64();
                Ok(hir::ConstValue::from_u64(val, int_ty))
            }
        }
        hir::BinOp::BitXor(int_ty) => {
            if int_ty.is_signed() {
                let val = lhs.into_int_i64() ^ rhs.into_int_i64();
                Ok(hir::ConstValue::from_i64(val, int_ty))
            } else {
                let val = lhs.into_int_u64() ^ rhs.into_int_u64();
                Ok(hir::ConstValue::from_u64(val, int_ty))
            }
        }
        hir::BinOp::BitShl(int_ty, _) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int() as u32;

            let layout = if int_ty == IntType::Untyped {
                hir::Layout::equal(8)
            } else {
                layout::int_layout(int_ty)
            };

            let val = if int_ty == IntType::Untyped || int_ty.is_signed() {
                match layout.size {
                    1 => ((lhs as i8) << rhs) as i128,
                    2 => ((lhs as i16) << rhs) as i128,
                    4 => ((lhs as i32) << rhs) as i128,
                    _ => ((lhs as i64) << rhs) as i128,
                }
            } else {
                match layout.size {
                    1 => ((lhs as u8) << rhs) as i128,
                    2 => ((lhs as u16) << rhs) as i128,
                    4 => ((lhs as u32) << rhs) as i128,
                    _ => ((lhs as u64) << rhs) as i128,
                }
            };
            let src = ctx.src(range);
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::BitShr(int_ty, _) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int() as u32;

            let val = lhs >> rhs;
            let src = ctx.src(range);
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::Eq_Int_Other(bool_ty) => {
            let val = match lhs {
                hir::ConstValue::Bool { val, .. } => val == rhs.into_bool(),
                hir::ConstValue::Char { val, .. } => val == rhs.into_char(),
                hir::ConstValue::Variant { variant_id, .. } => variant_id == rhs.into_enum().1,
                hir::ConstValue::Null => match rhs {
                    hir::ConstValue::Null => true,
                    hir::ConstValue::Procedure { .. } => false,
                    _ => unreachable!(),
                },
                hir::ConstValue::Procedure { proc_id, .. } => match rhs {
                    hir::ConstValue::Null => false,
                    hir::ConstValue::Procedure { proc_id: rhs_id, .. } => proc_id == rhs_id,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::NotEq_Int_Other(bool_ty) => {
            let val = match lhs {
                hir::ConstValue::Bool { val, .. } => val != rhs.into_bool(),
                hir::ConstValue::Char { val, .. } => val != rhs.into_char(),
                hir::ConstValue::Variant { variant_id, .. } => variant_id != rhs.into_enum().1,
                hir::ConstValue::Null => match rhs {
                    hir::ConstValue::Null => false,
                    hir::ConstValue::Procedure { .. } => true,
                    _ => unreachable!(),
                },
                hir::ConstValue::Procedure { proc_id, .. } => match rhs {
                    hir::ConstValue::Null => true,
                    hir::ConstValue::Procedure { proc_id: rhs_id, .. } => proc_id != rhs_id,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::Cmp_Int(pred, bool_ty, _) => {
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();
            let val = match pred {
                CmpPred::Eq => lhs == rhs,
                CmpPred::NotEq => lhs != rhs,
                CmpPred::Less => lhs < rhs,
                CmpPred::LessEq => lhs <= rhs,
                CmpPred::Greater => lhs > rhs,
                CmpPred::GreaterEq => lhs >= rhs,
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::Cmp_Float(pred, bool_ty, _) => {
            let lhs = lhs.into_float();
            let rhs = rhs.into_float();
            let val = match pred {
                CmpPred::Eq => lhs == rhs,
                CmpPred::NotEq => lhs != rhs,
                CmpPred::Less => lhs < rhs,
                CmpPred::LessEq => lhs <= rhs,
                CmpPred::Greater => lhs > rhs,
                CmpPred::GreaterEq => lhs >= rhs,
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::Cmp_String(pred, bool_ty, _) => {
            let lhs = lhs.into_string();
            let rhs = rhs.into_string();
            let val = match pred {
                CmpPred::Eq => lhs == rhs,
                CmpPred::NotEq => lhs != rhs,
                _ => unreachable!(),
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::LogicAnd(bool_ty) => {
            let val = lhs.into_bool() && rhs.into_bool();
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::LogicOr(bool_ty) => {
            let val = lhs.into_bool() || rhs.into_bool();
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
    }
}

fn typecheck_block<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    block: ast::Block<'ast>,
    status: BlockStatus,
) -> BlockResult<'hir> {
    ctx.scope.local.start_block(status);
    let offset = ctx.cache.stmts.start();
    let mut block_tail_ty: Option<hir::Type> = None;
    let mut block_tail_range: Option<TextRange> = None;

    for stmt in block.stmts.iter().copied() {
        let stmt = match stmt.kind {
            ast::StmtKind::WithDirective(stmt_dir) => {
                let config =
                    check_directive::check_expect_config(ctx, stmt_dir.dir_list, "statements");
                if config.disabled() {
                    continue;
                }
                stmt_dir.stmt
            }
            _ => stmt,
        };

        let stmt_res = match stmt.kind {
            ast::StmtKind::Break => {
                if let Some(stmt_res) = typecheck_break(ctx, stmt.range) {
                    check_stmt_diverges(ctx, true, stmt.range);
                    stmt_res
                } else {
                    continue;
                }
            }
            ast::StmtKind::Continue => {
                if let Some(stmt_res) = typecheck_continue(ctx, stmt.range) {
                    check_stmt_diverges(ctx, true, stmt.range);
                    stmt_res
                } else {
                    continue;
                }
            }
            ast::StmtKind::Return(expr) => {
                if let Some(stmt_res) = typecheck_return(ctx, expr, stmt.range) {
                    check_stmt_diverges(ctx, true, stmt.range);
                    stmt_res
                } else {
                    continue;
                }
            }
            //@currently not considering diverging blocks in defer (can happen from `never` calls)
            ast::StmtKind::Defer(block) => {
                if typecheck_defer(ctx, *block, stmt.range) {
                    check_stmt_diverges(ctx, false, stmt.range);
                }
                continue;
            }
            ast::StmtKind::For(for_) => {
                if let Some(loop_stmt) = typecheck_for(ctx, for_) {
                    //@can diverge (inf loop, return, panic)
                    check_stmt_diverges(ctx, false, stmt.range);
                    loop_stmt
                } else {
                    continue;
                }
            }
            ast::StmtKind::Local(local) => {
                let mut will_diverge = false;
                let local_res = typecheck_local(ctx, local, &mut will_diverge);
                check_stmt_diverges(ctx, will_diverge, stmt.range);
                match local_res {
                    LocalResult::Error => continue,
                    LocalResult::Local(local) => hir::Stmt::Local(local),
                    LocalResult::Discard(None) => continue,
                    LocalResult::Discard(Some(expr)) => hir::Stmt::ExprSemi(expr),
                }
            }
            ast::StmtKind::Assign(assign) => {
                let mut will_diverge = false;
                let assign = typecheck_assign(ctx, assign, &mut will_diverge);
                check_stmt_diverges(ctx, will_diverge, stmt.range);
                hir::Stmt::Assign(assign)
            }
            ast::StmtKind::ExprSemi(expr) => {
                let expect = match expr.kind {
                    ast::ExprKind::If { .. }
                    | ast::ExprKind::Block { .. }
                    | ast::ExprKind::Match { .. } => Expectation::VOID,
                    _ => Expectation::None,
                };
                let error_count = ctx.emit.error_count();
                let expr_res = typecheck_expr(ctx, expect, expr);
                if !ctx.emit.did_error(error_count) {
                    check_unused_expr_semi(ctx, expr_res.expr, expr.range);
                }
                check_stmt_diverges(ctx, expr_res.ty.is_never(), stmt.range);
                hir::Stmt::ExprSemi(expr_res.expr)
            }
            ast::StmtKind::ExprTail(expr) => {
                //type expectation is delegated to tail expression, instead of the block itself
                let expr_res = typecheck_expr(ctx, expect, expr);
                block_tail_ty = Some(expr_res.ty);
                block_tail_range = Some(expr.range);
                check_stmt_diverges(ctx, expr_res.ty.is_never(), stmt.range);
                hir::Stmt::ExprTail(expr_res.expr)
            }
            ast::StmtKind::WithDirective(_) => unreachable!(),
        };

        match ctx.scope.local.diverges() {
            Diverges::Maybe | Diverges::Always(_) => {
                match stmt_res {
                    hir::Stmt::Break | hir::Stmt::Continue => {
                        for block in ctx.scope.local.defer_blocks_loop().iter().copied().rev() {
                            let expr = hir::Expr::Block { block };
                            ctx.cache.stmts.push(hir::Stmt::ExprSemi(ctx.arena.alloc(expr)));
                        }
                    }
                    hir::Stmt::Return(_) => {
                        for block in ctx.scope.local.defer_blocks_all().iter().copied().rev() {
                            let expr = hir::Expr::Block { block };
                            ctx.cache.stmts.push(hir::Stmt::ExprSemi(ctx.arena.alloc(expr)));
                        }
                    }
                    _ => {}
                }

                if let hir::Stmt::Continue = stmt_res {
                    let curr_block = ctx.scope.local.current_block();
                    if let Some((var_id, int_ty, op)) = curr_block.for_idx_change {
                        let expr_var = ctx.arena.alloc(hir::Expr::Variable { var_id });
                        let expr_one = ctx.arena.alloc(hir::Expr::Const(
                            hir::ConstValue::Int { val: 1, neg: false, int_ty },
                            hir::ConstID::dummy(),
                        ));
                        let index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                            op: hir::AssignOp::Bin(op, None),
                            lhs: expr_var,
                            rhs: expr_one,
                        }));
                        ctx.cache.stmts.push(index_change);
                    }
                    if let Some((var_id, int_ty)) = curr_block.for_value_change {
                        let expr_var = ctx.arena.alloc(hir::Expr::Variable { var_id });
                        let expr_one = ctx.arena.alloc(hir::Expr::Const(
                            hir::ConstValue::Int { val: 1, neg: false, int_ty },
                            hir::ConstID::dummy(),
                        ));
                        let index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                            op: hir::AssignOp::Bin(hir::BinOp::Add_Int(int_ty), None),
                            lhs: expr_var,
                            rhs: expr_one,
                        }));
                        ctx.cache.stmts.push(index_change);
                    }
                }

                ctx.cache.stmts.push(stmt_res);
            }
            Diverges::AlwaysWarned => {}
        }
    }

    let diverges = match ctx.scope.local.diverges() {
        Diverges::Maybe => false,
        Diverges::Always(_) | Diverges::AlwaysWarned => true,
    };

    //@can affect diverge status? "defer { panic(); }"
    // generate defer blocks on exit from non-diverging block
    if let Diverges::Maybe = ctx.scope.local.diverges() {
        for block in ctx.scope.local.defer_blocks_last().iter().copied().rev() {
            let expr = hir::Expr::Block { block };
            ctx.cache.stmts.push(hir::Stmt::ExprSemi(ctx.arena.alloc(expr)));
        }
    }

    let stmts = ctx.cache.stmts.take(offset, &mut ctx.arena);
    let hir_block = hir::Block { stmts };

    //type expectation is delegated to tail expression, instead of the block itself
    let block_result = if let Some(block_ty) = block_tail_ty {
        BlockResult::new(block_ty, hir_block, block_tail_range)
    } else {
        let range = if block.stmts.is_empty() {
            block.range
        } else {
            TextRange::new(block.range.end() - 1.into(), block.range.end())
        };
        let block_ty = if diverges { hir::Type::Never } else { hir::Type::Void };
        type_expectation_check(ctx, range, block_ty, expect);
        BlockResult::new(block_ty, hir_block, block_tail_range)
    };

    ctx.scope.local.exit_block();
    block_result
}

fn check_stmt_diverges(ctx: &mut HirCtx, will_diverge: bool, stmt_range: TextRange) {
    match ctx.scope.local.diverges() {
        Diverges::Maybe => {
            if will_diverge {
                ctx.scope.local.diverges_set(Diverges::Always(stmt_range));
            }
        }
        Diverges::Always(range) => {
            let src = ctx.src(stmt_range);
            let after = ctx.src(range);
            err::tycheck_unreachable_stmt(&mut ctx.emit, src, after);
            ctx.scope.local.diverges_set(Diverges::AlwaysWarned);
        }
        Diverges::AlwaysWarned => {}
    }
}

fn typecheck_break<'hir>(ctx: &mut HirCtx, stmt_range: TextRange) -> Option<hir::Stmt<'hir>> {
    let (loop_found, defer) = ctx.scope.local.find_prev_loop_before_defer();
    if !loop_found {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
        let src = ctx.src(kw_range);
        err::tycheck_break_outside_loop(&mut ctx.emit, src);
        None
    } else if let Some(defer) = defer {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
        let src = ctx.src(kw_range);
        let defer_src = ctx.src(defer);
        err::tycheck_break_in_defer(&mut ctx.emit, src, defer_src);
        None
    } else {
        Some(hir::Stmt::Break)
    }
}

fn typecheck_continue<'hir>(ctx: &mut HirCtx, stmt_range: TextRange) -> Option<hir::Stmt<'hir>> {
    let (loop_found, defer) = ctx.scope.local.find_prev_loop_before_defer();
    if !loop_found {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
        let src = ctx.src(kw_range);
        err::tycheck_continue_outside_loop(&mut ctx.emit, src);
        None
    } else if let Some(defer) = defer {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
        let src = ctx.src(kw_range);
        let defer_src = ctx.src(defer);
        err::tycheck_continue_in_defer(&mut ctx.emit, src, defer_src);
        None
    } else {
        Some(hir::Stmt::Continue)
    }
}

fn typecheck_return<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expr: Option<&ast::Expr<'ast>>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let valid = if let Some(defer) = ctx.scope.local.find_prev_defer() {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
        let src = ctx.src(kw_range);
        let defer_src = ctx.src(defer);
        err::tycheck_return_in_defer(&mut ctx.emit, src, defer_src);
        false
    } else {
        true
    };

    let expect = ctx.scope.local.return_expect;
    let expr = match expr {
        Some(expr) => {
            let expr_res = typecheck_expr(ctx, expect, expr);
            Some(expr_res.expr)
        }
        None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
            type_expectation_check(ctx, kw_range, hir::Type::Void, expect);
            None
        }
    };

    valid.then_some(hir::Stmt::Return(expr))
}

fn typecheck_defer<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    block: ast::Block<'ast>,
    stmt_range: TextRange,
) -> bool {
    let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
    let valid = if let Some(defer) = ctx.scope.local.find_prev_defer() {
        let src = ctx.src(kw_range);
        let defer_src = ctx.src(defer);
        err::tycheck_defer_in_defer(&mut ctx.emit, src, defer_src);
        false
    } else {
        true
    };

    let block_res = typecheck_block(ctx, Expectation::VOID, block, BlockStatus::Defer(kw_range));
    ctx.scope.local.add_defer_block(block_res.block);
    valid
}

fn typecheck_for<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    for_: &ast::For<'ast>,
) -> Option<hir::Stmt<'hir>> {
    match for_.header {
        ast::ForHeader::Loop => {
            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);

            let block = ctx.arena.alloc(block_res.block);
            Some(hir::Stmt::Loop(block))
        }
        ast::ForHeader::Cond(cond) => {
            let cond_res = typecheck_expr(ctx, Expectation::None, cond);
            check_expect_boolean(ctx, cond.range, cond_res.ty);
            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);

            let branch_cond =
                hir::Expr::Unary { op: hir::UnOp::LogicNot(BoolType::Bool), rhs: cond_res.expr };
            let branch_block = hir::Block { stmts: ctx.arena.alloc_slice(&[hir::Stmt::Break]) };
            let branch = hir::Branch { cond: ctx.arena.alloc(branch_cond), block: branch_block };
            let expr_if = hir::If { branches: ctx.arena.alloc_slice(&[branch]), else_block: None };
            let expr_if = hir::Expr::If { if_: ctx.arena.alloc(expr_if) };
            let expr_block = hir::Expr::Block { block: block_res.block };

            let stmt_cond = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_if));
            let stmt_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_block));
            let block = hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_cond, stmt_block]) };
            let block = ctx.arena.alloc(block);
            Some(hir::Stmt::Loop(block))
        }
        ast::ForHeader::Elem(header) => {
            let expr_res = typecheck_expr(ctx, Expectation::None, header.expr);

            let collection = match type_as_collection(ctx, expr_res.ty) {
                Ok(None) => return None,
                Ok(Some(collection)) => match collection.kind {
                    CollectionKind::Slice(_)
                    | CollectionKind::Array(_)
                    | CollectionKind::ArrayEnum(_) => collection,
                    CollectionKind::Multi(_) | CollectionKind::ArrayCore => {
                        let src = ctx.src(header.expr.range);
                        let ty_fmt = type_format(ctx, expr_res.ty);
                        err::tycheck_cannot_iter_on_type(&mut ctx.emit, src, &ty_fmt);
                        return None;
                    }
                },
                Err(()) => {
                    let src = ctx.src(header.expr.range);
                    let ty_fmt = type_format(ctx, expr_res.ty);
                    err::tycheck_cannot_iter_on_type(&mut ctx.emit, src, &ty_fmt);
                    return None;
                }
            };

            let addr_res = resolve_expr_addressability(ctx, expr_res.expr);
            check_for_elem_addressability(
                ctx,
                header.ref_mut,
                addr_res,
                &collection,
                header.expr.range,
            );

            let value_ty = if let Some(mutt) = header.ref_mut {
                let ref_ty = ctx.arena.alloc(collection.elem_ty);
                hir::Type::Reference(mutt, ref_ty)
            } else {
                collection.elem_ty
            };

            let discard_id = ctx.session.intern_name.intern("_");
            let name_dummy = ast::Name { id: discard_id, range: TextRange::zero() };
            let value_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: header.value.unwrap_or(name_dummy),
                ty: value_ty,
                was_used: false,
            };
            let index_ty = match collection.kind {
                CollectionKind::ArrayEnum(array) => hir::Type::Enum(array.enum_id, &[]),
                _ => hir::Type::Int(IntType::U64),
            };
            let index_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: header.index.unwrap_or(name_dummy),
                ty: index_ty,
                was_used: false,
            };

            ctx.scope.local.start_block(BlockStatus::None);

            let value_already_defined = match header.value {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let value_id = if value_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(value_var)
            };

            let index_already_defined = match header.index {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let index_id = if index_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(index_var)
            };

            let curr_block = ctx.scope.local.current_block_mut();
            let index_int_ty = match collection.kind {
                CollectionKind::ArrayEnum(array) => {
                    let data = ctx.registry.enum_data(array.enum_id);
                    data.tag_ty.resolved().unwrap_or(IntType::Untyped)
                }
                _ => IntType::U64,
            };
            let index_change_op = if header.reverse {
                hir::BinOp::Sub_Int(index_int_ty)
            } else {
                hir::BinOp::Add_Int(index_int_ty)
            };
            curr_block.for_idx_change = Some((index_id, index_int_ty, index_change_op)); //@use defer instead of this hack?

            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);
            ctx.scope.local.exit_block();

            // iteration local:
            // let iter = &collection;

            //always storing a `&` to array or slice
            let iter_var_ty = if collection.deref.is_some() {
                expr_res.ty
            } else {
                hir::Type::Reference(ast::Mut::Immutable, ctx.arena.alloc(expr_res.ty))
            };
            let iter_var = hir::Variable {
                mutt: ast::Mut::Mutable,
                name: name_dummy,
                ty: iter_var_ty,
                was_used: false,
            };
            let iter_id = ctx.scope.local.add_variable(iter_var);

            let iter_init = if collection.deref.is_some() {
                expr_res.expr
            } else {
                ctx.arena.alloc(hir::Expr::Address { rhs: expr_res.expr })
            };
            let iter_local = hir::Local { var_id: iter_id, init: hir::LocalInit::Init(iter_init) };
            let stmt_iter = hir::Stmt::Local(ctx.arena.alloc(iter_local));

            let expr_iter_var = ctx.arena.alloc(hir::Expr::Variable { var_id: iter_id });
            let expr_iter_len = match collection.kind {
                CollectionKind::Array(array) => {
                    let len_value = hir::ConstValue::Int {
                        val: ctx.array_len(array.len).unwrap_or(0),
                        neg: false,
                        int_ty: index_int_ty,
                    };
                    ctx.arena.alloc(hir::Expr::Const(len_value, hir::ConstID::dummy()))
                }
                CollectionKind::ArrayEnum(array) => {
                    let val = ctx.registry.enum_data(array.enum_id).variants.len();
                    let len_value =
                        hir::ConstValue::Int { val: val as u64, neg: false, int_ty: index_int_ty };
                    ctx.arena.alloc(hir::Expr::Const(len_value, hir::ConstID::dummy()))
                }
                //always doing deref since `iter` stores &slice
                CollectionKind::Slice(_) => ctx.arena.alloc(hir::Expr::SliceField {
                    target: expr_iter_var,
                    access: hir::SliceFieldAccess {
                        deref: Some(ast::Mut::Immutable),
                        field: hir::SliceField::Len,
                    },
                }),
                CollectionKind::Multi(_) | CollectionKind::ArrayCore => unreachable!(),
            };
            let expr_zero = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 0, neg: false, int_ty: index_int_ty },
                hir::ConstID::dummy(),
            ));
            let expr_one = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 1, neg: false, int_ty: index_int_ty },
                hir::ConstID::dummy(),
            ));

            // index local:
            // forward: let idx = 0;
            // reverse: let idx = iter.len;
            let index_init = if header.reverse { expr_iter_len } else { expr_zero };
            let index_local =
                hir::Local { var_id: index_id, init: hir::LocalInit::Init(index_init) };
            let stmt_index = hir::Stmt::Local(ctx.arena.alloc(index_local));

            // loop statement:
            let expr_index_var = ctx.arena.alloc(hir::Expr::Variable { var_id: index_id });

            // conditional loop break:
            let cond_rhs = if header.reverse { expr_zero } else { expr_iter_len };
            let cond_op = if header.reverse {
                hir::BinOp::Cmp_Int(CmpPred::Eq, BoolType::Bool, IntType::U64)
            } else {
                hir::BinOp::Cmp_Int(CmpPred::GreaterEq, BoolType::Bool, IntType::U64)
            };
            let branch_cond = hir::Expr::Binary { op: cond_op, lhs: expr_index_var, rhs: cond_rhs };
            let branch_block = hir::Block { stmts: ctx.arena.alloc_slice(&[hir::Stmt::Break]) };
            let branch = hir::Branch { cond: ctx.arena.alloc(branch_cond), block: branch_block };
            let expr_if = hir::If { branches: ctx.arena.alloc_slice(&[branch]), else_block: None };
            let expr_if = hir::Expr::If { if_: ctx.arena.alloc(expr_if) };
            let stmt_cond = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_if));

            // index expression:
            let index_kind = match collection.kind {
                CollectionKind::Slice(slice) => hir::IndexKind::Slice(slice.mutt),
                CollectionKind::Array(array) => hir::IndexKind::Array(array.len),
                CollectionKind::ArrayEnum(array) => hir::IndexKind::ArrayEnum(array.enum_id),
                CollectionKind::Multi(_) | CollectionKind::ArrayCore => unreachable!(),
            };
            let index_access = hir::IndexAccess {
                //always doing deref since `iter` stores &collection
                deref: Some(ast::Mut::Immutable),
                elem_ty: collection.elem_ty,
                kind: index_kind,
                index: expr_index_var,
                offset: header.expr.range.end(),
            };
            let access = ctx.arena.alloc(index_access);
            let iter_index_expr =
                ctx.arena.alloc(hir::Expr::Index { target: expr_iter_var, access });
            let value_initializer = if header.ref_mut.is_some() {
                ctx.arena.alloc(hir::Expr::Address { rhs: iter_index_expr })
            } else {
                iter_index_expr
            };

            let stmt_value_local = hir::Stmt::Local(ctx.arena.alloc(hir::Local {
                var_id: value_id,
                init: hir::LocalInit::Init(value_initializer),
            }));

            let stmt_index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                op: hir::AssignOp::Bin(index_change_op, None),
                lhs: expr_index_var,
                rhs: expr_one,
            }));

            let expr_for_block = hir::Expr::Block { block: block_res.block };
            let stmt_for_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_for_block));

            let loop_block = if header.reverse {
                hir::Block {
                    stmts: ctx.arena.alloc_slice(&[
                        stmt_cond,
                        stmt_index_change,
                        stmt_value_local,
                        stmt_for_block,
                    ]),
                }
            } else if block_res.ty.is_never() {
                hir::Block {
                    stmts: ctx.arena.alloc_slice(&[stmt_cond, stmt_value_local, stmt_for_block]),
                }
            } else {
                hir::Block {
                    stmts: ctx.arena.alloc_slice(&[
                        stmt_cond,
                        stmt_value_local,
                        stmt_for_block,
                        stmt_index_change,
                    ]),
                }
            };
            let stmt_loop = hir::Stmt::Loop(ctx.arena.alloc(loop_block));

            let offset = ctx.cache.stmts.start();
            ctx.cache.stmts.push(stmt_iter);
            ctx.cache.stmts.push(stmt_index);
            ctx.cache.stmts.push(stmt_loop);
            let stmts = ctx.cache.stmts.take(offset, &mut ctx.arena);

            let expr_overall_block = hir::Expr::Block { block: hir::Block { stmts } };
            let overall_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_overall_block));
            Some(overall_block)
        }
        ast::ForHeader::Range(header) => {
            if let Some(start) = header.ref_start {
                let src = ctx.src(TextRange::new(start, start + 1.into()));
                err::tycheck_for_range_ref(&mut ctx.emit, src);
            }
            if let Some(start) = header.reverse_start {
                let src = ctx.src(TextRange::new(start, start + 2.into()));
                err::tycheck_for_range_reverse(&mut ctx.emit, src);
            }

            let (start, end, int_ty) =
                typecheck_range(ctx, Expectation::None, &header.range, false);

            let discard_id = ctx.session.intern_name.intern("_");
            let name_dummy = ast::Name { id: discard_id, range: TextRange::zero() };

            let start_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: header.value.unwrap_or(name_dummy),
                ty: hir::Type::Int(int_ty),
                was_used: false,
            };
            let end_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: name_dummy,
                ty: hir::Type::Int(int_ty),
                was_used: false,
            };
            let index_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: header.index.unwrap_or(name_dummy),
                ty: hir::Type::Int(IntType::U64),
                was_used: false,
            };

            ctx.scope.local.start_block(BlockStatus::None);

            let value_already_defined = match header.value {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let start_id = if value_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(start_var)
            };
            let index_already_defined = match header.index {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let index_id = if index_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(index_var)
            };
            let end_id = ctx.scope.local.add_variable(end_var);

            let curr_block = ctx.scope.local.current_block_mut();
            curr_block.for_idx_change =
                Some((index_id, IntType::U64, hir::BinOp::Add_Int(IntType::U64)));
            curr_block.for_value_change = Some((start_id, int_ty));

            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);
            ctx.scope.local.exit_block();

            // start, end, index locals:
            let start_local = hir::Local { var_id: start_id, init: hir::LocalInit::Init(start) };
            let stmt_start = hir::Stmt::Local(ctx.arena.alloc(start_local));

            let end_local = hir::Local { var_id: end_id, init: hir::LocalInit::Init(end) };
            let stmt_end = hir::Stmt::Local(ctx.arena.alloc(end_local));

            let zero = hir::ConstValue::Int { val: 0, neg: false, int_ty: IntType::U64 };
            let zero = ctx.arena.alloc(hir::Expr::Const(zero, hir::ConstID::dummy()));
            let index_local = hir::Local { var_id: index_id, init: hir::LocalInit::Init(zero) };
            let stmt_index = hir::Stmt::Local(ctx.arena.alloc(index_local));

            // loop body block:
            let expr_start_var = ctx.arena.alloc(hir::Expr::Variable { var_id: start_id });
            let expr_end_var = ctx.arena.alloc(hir::Expr::Variable { var_id: end_id });
            let expr_index_var = ctx.arena.alloc(hir::Expr::Variable { var_id: index_id });

            let cond_op = match header.range.kind {
                ast::RangeKind::Exclusive => {
                    hir::BinOp::Cmp_Int(CmpPred::Less, BoolType::Bool, int_ty)
                }
                ast::RangeKind::Inclusive => {
                    hir::BinOp::Cmp_Int(CmpPred::LessEq, BoolType::Bool, int_ty)
                }
            };
            let continue_cond = ctx.arena.alloc(hir::Expr::Binary {
                op: cond_op,
                lhs: expr_start_var,
                rhs: expr_end_var,
            });
            let break_cond =
                hir::Expr::Unary { op: hir::UnOp::LogicNot(BoolType::Bool), rhs: continue_cond };
            let branch_block = hir::Block { stmts: ctx.arena.alloc_slice(&[hir::Stmt::Break]) };
            let branch = hir::Branch { cond: ctx.arena.alloc(break_cond), block: branch_block };
            let expr_if = hir::If { branches: ctx.arena.alloc_slice(&[branch]), else_block: None };
            let expr_if = hir::Expr::If { if_: ctx.arena.alloc(expr_if) };
            let stmt_cond = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_if));

            let expr_one_iter = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 1, neg: false, int_ty },
                hir::ConstID::dummy(),
            ));
            let stmt_value_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                op: hir::AssignOp::Bin(hir::BinOp::Add_Int(int_ty), None),
                lhs: expr_start_var,
                rhs: expr_one_iter,
            }));
            let expr_one_usize = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 1, neg: false, int_ty: IntType::U64 },
                hir::ConstID::dummy(),
            ));
            let stmt_index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                op: hir::AssignOp::Bin(hir::BinOp::Add_Int(IntType::U64), None),
                lhs: expr_index_var,
                rhs: expr_one_usize,
            }));

            let expr_for_block = hir::Expr::Block { block: block_res.block };
            let stmt_for_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_for_block));

            let loop_block = if block_res.ty.is_never() {
                hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_cond, stmt_for_block]) }
            } else {
                hir::Block {
                    stmts: ctx.arena.alloc_slice(&[
                        stmt_cond,
                        stmt_for_block,
                        stmt_value_change,
                        stmt_index_change,
                    ]),
                }
            };
            let stmt_loop = hir::Stmt::Loop(ctx.arena.alloc(loop_block));

            // overall block for entire loop:
            let stmts = ctx.arena.alloc_slice(&[stmt_start, stmt_end, stmt_index, stmt_loop]);
            let expr_overall_block = hir::Expr::Block { block: hir::Block { stmts } };
            let overall_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_overall_block));
            Some(overall_block)
        }
        ast::ForHeader::Pat(header) => {
            let error_count = ctx.emit.error_count();
            let on_res = typecheck_expr(ctx, Expectation::None, header.expr);
            let kind = check_match::match_kind(ctx, on_res.ty, header.expr.range);
            let (pat_expect, ref_mut) = check_match::match_pat_expect(ctx, header.expr.range, kind);

            ctx.scope.local.start_block(BlockStatus::None);
            let pat = typecheck_pat(ctx, pat_expect, &header.pat, ref_mut, false);
            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);
            ctx.scope.local.exit_block();

            let kind = kind?;
            let arms = [hir::MatchArm { pat, block: block_res.block }];
            let arms_ast = [ast::MatchArm { pat: header.pat, expr: header.expr }];
            let check = check_match::CheckContext::new(None, &arms, &arms_ast);
            check_match::match_cov(ctx, kind, &check, error_count);

            let any_wild = match pat {
                hir::Pat::Wild => true,
                hir::Pat::Or(pats) => pats.iter().any(|p| matches!(p, hir::Pat::Wild)),
                _ => false,
            };
            let arms = if any_wild {
                let arm = hir::MatchArm { pat, block: block_res.block };
                ctx.arena.alloc_slice(&[arm])
            } else {
                let arm = hir::MatchArm { pat, block: block_res.block };
                let block = hir::Block { stmts: ctx.arena.alloc_slice(&[hir::Stmt::Break]) };
                let break_arm = hir::MatchArm { pat: hir::Pat::Wild, block };
                ctx.arena.alloc_slice(&[arm, break_arm])
            };

            let match_ = hir::Match { kind, on_expr: on_res.expr, arms };
            let stmt_match = hir::Stmt::ExprSemi(emit_match_expr(ctx, match_));
            let block = hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_match]) };
            let block = ctx.arena.alloc(block);
            Some(hir::Stmt::Loop(block))
        }
    }
}

fn typecheck_range<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    range: &ast::Range<'ast>,
    constant: bool,
) -> (&'hir hir::Expr<'hir>, &'hir hir::Expr<'hir>, hir::IntType) {
    let mut start_res = typecheck_expr_untyped(ctx, expect, range.start);
    let mut end_res = typecheck_expr_untyped(ctx, expect, range.end);

    let start_promote = promote_untyped(
        ctx,
        range.start.range,
        *start_res.expr,
        &mut start_res.ty,
        Some(end_res.ty),
        true,
    );
    let end_promote = promote_untyped(
        ctx,
        range.end.range,
        *end_res.expr,
        &mut end_res.ty,
        Some(start_res.ty),
        true,
    );

    if let Some(Ok((v, id))) = start_promote {
        start_res.expr = ctx.arena.alloc(hir::Expr::Const(v, id));
    }
    if let Some(Ok((v, id))) = end_promote {
        end_res.expr = ctx.arena.alloc(hir::Expr::Const(v, id));
    }

    check_expect_integer(ctx, range.start.range, start_res.ty);
    check_expect_integer(ctx, range.end.range, end_res.ty);

    if constant {
        if !matches!(start_res.expr, hir::Expr::Error | hir::Expr::Const(_, _)) {
            let src = ctx.src(range.start.range);
            err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
        }
        if !matches!(end_res.expr, hir::Expr::Error | hir::Expr::Const(_, _)) {
            let src = ctx.src(range.end.range);
            err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
        }
    }

    let int_ty = if let (hir::Type::Int(lhs), hir::Type::Int(rhs)) = (start_res.ty, end_res.ty) {
        if lhs != rhs {
            let range = TextRange::new(range.start.range.start(), range.end.range.end());
            let src = ctx.src(range);
            err::tycheck_for_range_type_mismatch(&mut ctx.emit, src, lhs.as_str(), rhs.as_str());
        }
        lhs
    } else {
        IntType::S32
    };

    (start_res.expr, end_res.expr, int_ty)
}

enum LocalResult<'hir> {
    Error,
    Local(&'hir hir::Local<'hir>),
    Discard(Option<&'hir hir::Expr<'hir>>),
}

fn typecheck_local<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    local: &ast::Local<'ast>,
    will_diverge: &mut bool,
) -> LocalResult<'hir> {
    let already_defined = match local.bind {
        ast::Binding::Named(_, name) => ctx
            .scope
            .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err(),
        ast::Binding::Discard(_) => false,
    };

    let (mut local_ty, expect) = match local.ty {
        Some(ty) => {
            let local_ty = super::pass_3::type_resolve(ctx, ty, false);
            let expect_src = ctx.src(ty.range);
            (Some(local_ty), Expectation::HasType(local_ty, Some(expect_src)))
        }
        None => (None, Expectation::None),
    };

    let init = match local.init {
        ast::LocalInit::Init(expr) => {
            let init_res = typecheck_expr(ctx, expect, expr);
            local_ty = local_ty.or(Some(init_res.ty));
            *will_diverge = init_res.ty.is_never();
            hir::LocalInit::Init(init_res.expr)
        }
        ast::LocalInit::Zeroed(_) => hir::LocalInit::Zeroed,
        ast::LocalInit::Undefined(_) => hir::LocalInit::Undefined,
    };

    match local.bind {
        ast::Binding::Named(mutt, name) => {
            let ty = if let Some(ty) = local_ty {
                ty
            } else {
                let src = ctx.src(name.range);
                err::tycheck_cannot_infer_local_type(&mut ctx.emit, src);
                hir::Type::Error
            };

            if already_defined {
                return LocalResult::Error;
            }

            let var = hir::Variable { mutt, name, ty, was_used: false };
            let var_id = ctx.scope.local.add_variable(var);
            let local = hir::Local { var_id, init };
            LocalResult::Local(ctx.arena.alloc(local))
        }
        ast::Binding::Discard(_) => match init {
            hir::LocalInit::Init(expr) => LocalResult::Discard(Some(expr)),
            hir::LocalInit::Zeroed => LocalResult::Discard(None),
            hir::LocalInit::Undefined => LocalResult::Discard(None),
        },
    }
}

fn typecheck_assign<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    assign: &ast::Assign<'ast>,
    will_diverge: &mut bool,
) -> &'hir hir::Assign<'hir> {
    let lhs_res = typecheck_expr(ctx, Expectation::None, assign.lhs);
    let addr_res = resolve_expr_addressability(ctx, lhs_res.expr);
    check_assign_addressability(ctx, &addr_res, assign.lhs.range);

    let expect = Expectation::HasType(lhs_res.ty, Some(ctx.src(assign.lhs.range)));
    let rhs_res = typecheck_expr(ctx, expect, assign.rhs);
    *will_diverge = rhs_res.ty.is_never(); //when lhs is `never` typecheck will require rhs `never`

    let assign_op = match assign.op {
        ast::AssignOp::Assign => hir::AssignOp::Assign,
        ast::AssignOp::Bin(op) => {
            let op_start = assign.op_range.start();
            let op_res = check_binary_op(
                ctx,
                Expectation::None,
                op,
                op_start,
                lhs_res.clone(),
                rhs_res.clone(),
            );
            op_res.map(|op| hir::AssignOp::Bin(op.0, op.1)).unwrap_or(hir::AssignOp::Assign)
        }
    };

    let assign = hir::Assign { op: assign_op, lhs: lhs_res.expr, rhs: rhs_res.expr };
    ctx.arena.alloc(assign)
}

pub fn int_range_check<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    val: i128,
    int_ty: IntType,
) -> Result<hir::ConstValue<'hir>, ()> {
    let min = int_ty.min_128();
    let max = int_ty.max_128();

    if val < min || val > max {
        let int_ty = int_ty.as_str();
        err::const_int_out_of_range(&mut ctx.emit, src, int_ty, val, min, max);
        Err(())
    } else if val >= 0 {
        let val: u64 = val.try_into().unwrap();
        Ok(hir::ConstValue::Int { val, neg: false, int_ty })
    } else {
        let val: u64 = (-val).try_into().unwrap();
        Ok(hir::ConstValue::Int { val, neg: true, int_ty })
    }
}

//==================== UNUSED ====================

fn check_unused_expr_semi(ctx: &mut HirCtx, expr: &hir::Expr, expr_range: TextRange) {
    fn unused_return_type(ty: hir::Type, kind: &'static str) -> Option<&'static str> {
        match ty {
            hir::Type::Error | hir::Type::Void | hir::Type::Never => None,
            _ => Some(kind),
        }
    }

    let unused = match *expr {
        hir::Expr::Error => None, // already errored
        hir::Expr::Const { .. } => Some("constant value"),
        hir::Expr::If { .. } => None,    // expected `void`
        hir::Expr::Block { .. } => None, // expected `void`
        hir::Expr::Match { .. } => None, // expected `void`
        hir::Expr::StructField { .. } => Some("field access"),
        hir::Expr::SliceField { .. } => Some("field access"),
        hir::Expr::Index { .. } => Some("index access"),
        hir::Expr::Slice { .. } => Some("slice value"),
        hir::Expr::Cast { .. } => Some("cast value"),
        hir::Expr::ParamVar { .. } => Some("parameter value"),
        hir::Expr::Variable { .. } => Some("variable value"),
        hir::Expr::GlobalVar { .. } => Some("global value"),
        hir::Expr::Variant { .. } => Some("variant value"),
        hir::Expr::CallDirect { proc_id, .. } => {
            let ty = ctx.registry.proc_data(proc_id).return_ty;
            unused_return_type(ty, "procedure return value")
        }
        hir::Expr::CallDirectPoly { proc_id, .. } => {
            let ty = ctx.registry.proc_data(proc_id).return_ty;
            unused_return_type(ty, "procedure return value")
        }
        hir::Expr::CallIndirect { indirect, .. } => {
            let ty = indirect.proc_ty.return_ty;
            unused_return_type(ty, "indirect call return value")
        }
        hir::Expr::VariadicArg { .. } => None,
        hir::Expr::StructInit { .. } | hir::Expr::StructInitPoly { .. } => Some("struct value"),
        hir::Expr::ArrayInit { .. } => Some("array value"),
        hir::Expr::ArrayRepeat { .. } => Some("array value"),
        hir::Expr::Deref { .. } => Some("dereference"),
        hir::Expr::Address { .. } => Some("address value"),
        hir::Expr::Unary { .. } => Some("unary operation"),
        hir::Expr::Binary { .. } | hir::Expr::ArrayBinary { .. } => Some("binary operation"),
    };

    if let Some(kind) = unused {
        let src = ctx.src(expr_range);
        err::tycheck_unused_expr(&mut ctx.emit, src, kind);
    }
}

//==================== INFER ====================

fn infer_enum_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation<'hir>,
    error_src: SourceRange,
) -> Option<(hir::EnumID, Option<&'hir [hir::Type<'hir>]>)> {
    let enum_id = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            hir::Type::Enum(enum_id, poly_types) => Some((enum_id, Some(poly_types))),
            hir::Type::Reference(_, hir::Type::Enum(enum_id, poly_types)) => {
                Some((*enum_id, Some(*poly_types)))
            }
            _ => None,
        },
    };
    if enum_id.is_none() {
        err::tycheck_cannot_infer_enum_type(&mut ctx.emit, error_src);
    }
    enum_id
}

fn infer_enum_poly_types(enum_id: hir::EnumID, expect: Expectation) -> Option<&[hir::Type]> {
    match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => None,
            hir::Type::Enum(id, poly_types) => {
                if id == enum_id {
                    Some(poly_types)
                } else {
                    None
                }
            }
            _ => None,
        },
    }
}

fn infer_struct_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation<'hir>,
    error_src: SourceRange,
) -> Option<(hir::StructID, Option<&'hir [hir::Type<'hir>]>)> {
    let struct_res = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            hir::Type::Struct(struct_id, poly_types) => Some((struct_id, Some(poly_types))),
            _ => None,
        },
    };
    if struct_res.is_none() {
        err::tycheck_cannot_infer_struct_type(&mut ctx.emit, error_src);
    }
    struct_res
}

fn infer_struct_poly_types(struct_id: hir::StructID, expect: Expectation) -> Option<&[hir::Type]> {
    match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => None,
            hir::Type::Struct(id, poly_types) => {
                if id == struct_id {
                    Some(poly_types)
                } else {
                    None
                }
            }
            _ => None,
        },
    }
}

//==================== DEFAULT CHECK ====================

fn default_check_arg_list<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    arg_list: &ast::ArgumentList<'ast>,
) {
    for &expr in arg_list.exprs.iter() {
        let _ = typecheck_expr(ctx, Expectation::ERROR, expr);
    }
}

fn default_check_field_init_list<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    input: &[ast::FieldInit<'ast>],
) {
    for field in input.iter() {
        let _ = typecheck_expr(ctx, Expectation::ERROR, field.expr);
    }
}

//==================== CALL & INPUT ====================

fn arg_list_range(arg_list: &ast::ArgumentList) -> TextRange {
    if arg_list.exprs.is_empty() {
        arg_list.range
    } else {
        let end = arg_list.range.end();
        TextRange::new(end - 1.into(), end)
    }
}

fn arg_list_opt_range(arg_list: Option<&ast::ArgumentList>, default: TextRange) -> TextRange {
    if let Some(arg_list) = arg_list {
        arg_list_range(arg_list)
    } else {
        default
    }
}

fn bind_list_range(bind_list: &ast::BindingList) -> TextRange {
    if bind_list.binds.is_empty() {
        bind_list.range
    } else {
        let end = bind_list.range.end();
        TextRange::new(end - 1.into(), end)
    }
}

fn bind_list_opt_range(bind_list: Option<&ast::BindingList>, default: TextRange) -> TextRange {
    if let Some(bind_list) = bind_list {
        bind_list_range(bind_list)
    } else {
        default
    }
}

fn check_call_direct<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    proc_id: hir::ProcID,
    poly_types: Option<&'hir [hir::Type<'hir>]>,
    arg_list: &ast::ArgumentList<'ast>,
    range: TextRange,
) -> TypeResult<'hir> {
    let data = ctx.registry.proc_data(proc_id);
    let origin_id = data.origin_id;
    let return_ty = data.return_ty;

    let (infer, is_poly) = ctx.scope.infer.start_context(data.poly_params);
    if let Some(poly_types) = poly_types {
        for (poly_idx, ty) in poly_types.iter().copied().enumerate() {
            ctx.scope.infer.resolve(infer, poly_idx, ty);
        }
    }
    if is_poly {
        if let Expectation::HasType(expect_ty, _) = expect {
            types::apply_inference(ctx.scope.infer.inferred_mut(infer), expect_ty, return_ty);
        }
    }

    let expected_count = data.params.iter().filter(|p| p.kind == hir::ParamKind::Normal).count();
    let is_variadic = data.flag_set.contains(hir::ProcFlag::Variadic)
        || data.flag_set.contains(hir::ProcFlag::CVariadic);
    check_call_arg_count(ctx, arg_list, Some(data.src()), expected_count, is_variadic);

    let data = ctx.registry.proc_data(proc_id);
    let offset = ctx.cache.exprs.start();
    let mut args = arg_list.exprs.iter().copied();

    for param in data.params {
        match param.kind {
            hir::ParamKind::Normal => {
                if let Some(expr) = args.next() {
                    let expect_src = SourceRange::new(origin_id, param.ty_range);
                    let param_ty = type_substitute_inferred(ctx, is_poly, infer, param.ty);
                    let expect = Expectation::HasType(param_ty, Some(expect_src));

                    let expr_res = typecheck_expr(ctx, expect, expr);
                    if is_poly {
                        types::apply_inference(
                            ctx.scope.infer.inferred_mut(infer),
                            expr_res.ty,
                            param.ty,
                        );
                    }
                    ctx.cache.exprs.push(expr_res.expr);
                } else {
                    ctx.cache.exprs.push(&hir::Expr::Error);
                }
            }
            hir::ParamKind::Variadic => {
                let offset = ctx.cache.exprs.start();
                let offset_ty = ctx.cache.types.start();
                for expr in args.by_ref() {
                    let expr_res = typecheck_expr(ctx, Expectation::None, expr);
                    ctx.cache.exprs.push(expr_res.expr);
                    ctx.cache.types.push(expr_res.ty);
                }
                let exprs = ctx.cache.exprs.take(offset, &mut ctx.arena);
                let types = ctx.cache.types.take(offset_ty, &mut ctx.arena);
                let arg = ctx.arena.alloc(hir::VariadicArg { exprs, types });
                let variadic = ctx.arena.alloc(hir::Expr::VariadicArg { arg });
                ctx.cache.exprs.push(variadic);
                break;
            }
            hir::ParamKind::CallerLocation => {
                let values = hir::source_location(ctx.session, ctx.scope.origin, range.start());
                let values = ctx.arena.alloc_slice(&values);
                let struct_ = hir::ConstStruct { values, poly_types: &[] };
                let struct_ = ctx.arena.alloc(struct_);
                let value = hir::ConstValue::Struct { struct_id: ctx.core.source_loc, struct_ };
                let expr = ctx.arena.alloc(hir::Expr::Const(value, hir::ConstID::dummy()));
                ctx.cache.exprs.push(expr);
            }
        }
    }

    let data = ctx.registry.proc_data(proc_id);
    if data.flag_set.contains(hir::ProcFlag::CVariadic) {
        for expr in args {
            let expr_res = typecheck_expr(ctx, Expectation::ERROR, expr);
            ctx.cache.exprs.push(expr_res.expr);
        }
    } else {
        for expr in args {
            let _ = typecheck_expr(ctx, Expectation::ERROR, expr);
        }
    }

    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_unknown()) {
        let src = ctx.src(TextRange::new(range.start(), arg_list.range.start()));
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
        ctx.cache.exprs.pop_view(offset);
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_error()) {
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    let poly_types = match poly_types {
        Some(poly_types) => poly_types,
        None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
    };

    let return_ty = type_substitute_inferred(ctx, is_poly, infer, return_ty);
    let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
    ctx.scope.infer.end_context(infer);

    let data = ctx.registry.proc_data(proc_id);
    if data.flag_set.contains(hir::ProcFlag::Intrinsic) {
        if let Some(result) = check_call_intrinsic(ctx, proc_id, poly_types, range) {
            return result;
        }
    }

    let expr = if is_poly {
        hir::Expr::CallDirectPoly { proc_id, input: ctx.arena.alloc((input, poly_types)) }
    } else {
        hir::Expr::CallDirect { proc_id, input }
    };
    TypeResult::new(return_ty, expr)
}

fn check_call_indirect<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target_range: TextRange,
    target_res: ExprResult<'hir>,
    arg_list: &ast::ArgumentList<'ast>,
) -> TypeResult<'hir> {
    let proc_ty = match target_res.ty {
        hir::Type::Error => {
            default_check_arg_list(ctx, arg_list);
            return TypeResult::error();
        }
        hir::Type::Procedure(proc_ty) => proc_ty,
        _ => {
            let src = ctx.src(target_range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_call_value_of_type(&mut ctx.emit, src, &ty_fmt);
            default_check_arg_list(ctx, arg_list);
            return TypeResult::error();
        }
    };

    let expected_count = proc_ty.params.iter().filter(|p| p.kind == hir::ParamKind::Normal).count();
    let is_variadic = proc_ty.flag_set.contains(hir::ProcFlag::Variadic)
        || proc_ty.flag_set.contains(hir::ProcFlag::CVariadic);
    check_call_arg_count(ctx, arg_list, None, expected_count, is_variadic);

    let offset = ctx.cache.exprs.start();
    let mut args = arg_list.exprs.iter().copied();

    for param in proc_ty.params {
        match param.kind {
            hir::ParamKind::Normal => {
                if let Some(expr) = args.next() {
                    let expect = Expectation::HasType(param.ty, None);
                    let expr_res = typecheck_expr(ctx, expect, expr);
                    ctx.cache.exprs.push(expr_res.expr);
                } else {
                    ctx.cache.exprs.push(&hir::Expr::Error);
                }
            }
            hir::ParamKind::Variadic => {
                let offset = ctx.cache.exprs.start();
                let offset_ty = ctx.cache.types.start();
                for expr in args.by_ref() {
                    let expr_res = typecheck_expr(ctx, Expectation::None, expr);
                    ctx.cache.exprs.push(expr_res.expr);
                    ctx.cache.types.push(expr_res.ty);
                }
                let exprs = ctx.cache.exprs.take(offset, &mut ctx.arena);
                let types = ctx.cache.types.take(offset_ty, &mut ctx.arena);
                let arg = ctx.arena.alloc(hir::VariadicArg { exprs, types });
                let variadic = ctx.arena.alloc(hir::Expr::VariadicArg { arg });
                ctx.cache.exprs.push(variadic);
                break;
            }
            hir::ParamKind::CallerLocation => {
                let values =
                    hir::source_location(ctx.session, ctx.scope.origin, target_range.start());
                let values = ctx.arena.alloc_slice(&values);
                let struct_ = hir::ConstStruct { values, poly_types: &[] };
                let struct_ = ctx.arena.alloc(struct_);
                let value = hir::ConstValue::Struct { struct_id: ctx.core.source_loc, struct_ };
                let expr = ctx.arena.alloc(hir::Expr::Const(value, hir::ConstID::dummy()));
                ctx.cache.exprs.push(expr);
            }
        }
    }

    if proc_ty.flag_set.contains(hir::ProcFlag::CVariadic) {
        for expr in args {
            let expr_res = typecheck_expr(ctx, Expectation::ERROR, expr);
            ctx.cache.exprs.push(expr_res.expr);
        }
    } else {
        for expr in args {
            let _ = typecheck_expr(ctx, Expectation::ERROR, expr);
        }
    }

    let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
    let indirect = hir::CallIndirect { proc_ty, input };
    let expr =
        hir::Expr::CallIndirect { target: target_res.expr, indirect: ctx.arena.alloc(indirect) };
    TypeResult::new(proc_ty.return_ty, expr)
}

fn check_call_intrinsic<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    proc_id: hir::ProcID,
    poly_types: &'hir [hir::Type<'hir>],
    range: TextRange,
) -> Option<TypeResult<'hir>> {
    let data = ctx.registry.proc_data(proc_id);
    let src = ctx.src(range);

    match ctx.name(data.name.id) {
        "size_of" => {
            if types::has_poly_layout_dep(poly_types[0]) {
                return None;
            }
            let layout = layout::type_layout(ctx, poly_types[0], &[], src);
            let expr = layout
                .map(|layout| {
                    int_range_check(ctx, src, layout.size as i128, IntType::U64)
                        .map(|value| hir::Expr::Const(value, hir::ConstID::dummy()))
                        .unwrap_or(hir::Expr::Error)
                })
                .unwrap_or(hir::Expr::Error);
            Some(TypeResult::new(hir::Type::Int(IntType::U64), expr))
        }
        "align_of" => {
            if types::has_poly_layout_dep(poly_types[0]) {
                return None;
            }
            let layout = layout::type_layout(ctx, poly_types[0], &[], src);
            let expr = layout
                .map(|layout| {
                    int_range_check(ctx, src, layout.align as i128, IntType::U64)
                        .map(|value| hir::Expr::Const(value, hir::ConstID::dummy()))
                        .unwrap_or(hir::Expr::Error)
                })
                .unwrap_or(hir::Expr::Error);
            Some(TypeResult::new(hir::Type::Int(IntType::U64), expr))
        }
        "transmute" => {
            let from_ty = poly_types[0];
            let into_ty = poly_types[1];

            if types::has_poly_layout_dep(from_ty) {
                let ty = type_format(ctx, from_ty);
                err::tycheck_transmute_poly_dep(&mut ctx.emit, src, &ty);
                return Some(TypeResult::error());
            }
            if types::has_poly_layout_dep(into_ty) {
                let ty = type_format(ctx, into_ty);
                err::tycheck_transmute_poly_dep(&mut ctx.emit, src, &ty);
                return Some(TypeResult::error());
            }

            let from_res = layout::type_layout(ctx, from_ty, &[], src);
            let into_res = layout::type_layout(ctx, into_ty, &[], src);
            let (Ok(from_layout), Ok(into_layout)) = (from_res, into_res) else {
                return Some(TypeResult::error());
            };

            let (subject, from_val, into_val) = if from_layout.size != into_layout.size {
                ("size", from_layout.size, into_layout.size)
            } else {
                return None;
            };

            let from_ty = type_format(ctx, from_ty);
            let into_ty = type_format(ctx, into_ty);
            err::tycheck_transmute_mismatch(
                &mut ctx.emit,
                src,
                subject,
                &from_ty,
                from_val,
                &into_ty,
                into_val,
            );
            Some(TypeResult::error())
        }
        _ => None,
    }
}

fn check_call_arg_count(
    ctx: &mut HirCtx,
    arg_list: &ast::ArgumentList,
    proc_src: Option<SourceRange>,
    expected_count: usize,
    variadic: bool,
) -> bool {
    let input_count = arg_list.exprs.len();
    let wrong_count =
        if variadic { input_count < expected_count } else { input_count != expected_count };

    if wrong_count {
        let src = ctx.src(arg_list_range(arg_list));
        err::tycheck_unexpected_proc_arg_count(
            &mut ctx.emit,
            src,
            proc_src,
            variadic,
            input_count,
            expected_count,
        );
    }
    !wrong_count
}

fn check_variant_input<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    mut poly_types: Option<&'hir [hir::Type<'hir>]>,
    arg_list: Option<&ast::ArgumentList<'ast>>,
    range: TextRange,
) -> TypeResult<'hir> {
    let data = ctx.registry.enum_data(enum_id);
    let variant = data.variant(variant_id);
    let origin_id = data.origin_id;
    let with_fields = data.flag_set.contains(hir::EnumFlag::WithFields);

    let (infer, is_poly) = ctx.scope.infer.start_context(data.poly_params);
    if is_poly && poly_types.is_none() {
        poly_types = infer_enum_poly_types(enum_id, expect);
    }
    if let Some(poly_types) = poly_types {
        for (poly_idx, ty) in poly_types.iter().copied().enumerate() {
            ctx.scope.infer.resolve(infer, poly_idx, ty);
        }
    }

    let input_count = arg_list.map(|arg_list| arg_list.exprs.len()).unwrap_or(0);
    let expected_count = variant.fields.len();

    if expected_count == 0 && arg_list.is_some() {
        let src = ctx.src(arg_list_opt_range(arg_list, range));
        let variant_src = SourceRange::new(origin_id, variant.name.range);
        err::tycheck_unexpected_variant_arg_list(&mut ctx.emit, src, variant_src);
    } else if input_count != expected_count {
        let src = ctx.src(arg_list_opt_range(arg_list, range));
        let variant_src = SourceRange::new(origin_id, variant.name.range);
        err::tycheck_unexpected_variant_arg_count(
            &mut ctx.emit,
            src,
            variant_src,
            input_count,
            expected_count,
        );
    }

    let mut input = &[][..];
    if let Some(arg_list) = arg_list {
        let offset = ctx.cache.exprs.start();
        for (idx, &expr) in arg_list.exprs.iter().enumerate() {
            let expect = match variant.fields.get(idx) {
                Some(field) => {
                    let expect_src = SourceRange::new(origin_id, field.ty_range);
                    let field_ty = type_substitute_inferred(ctx, is_poly, infer, field.ty);
                    Expectation::HasType(field_ty, Some(expect_src))
                }
                None => Expectation::ERROR,
            };

            let expr_res = typecheck_expr(ctx, expect, expr);
            ctx.cache.exprs.push(expr_res.expr);

            if is_poly {
                if let Some(field) = variant.fields.get(idx) {
                    types::apply_inference(
                        ctx.scope.infer.inferred_mut(infer),
                        expr_res.ty,
                        field.ty,
                    );
                }
            }
        }
        input = ctx.cache.exprs.take(offset, &mut ctx.arena);
    }

    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_unknown()) {
        let range =
            TextRange::new(range.start(), arg_list.map(|l| l.range.start()).unwrap_or(range.end()));
        let src = ctx.src(range);
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    if ctx.scope.infer.inferred(infer).iter().any(|t| t.is_error()) {
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    let poly_types = match poly_types {
        Some(poly_types) => poly_types,
        None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
    };
    ctx.scope.infer.end_context(infer);

    if !is_poly && !with_fields {
        let value = hir::ConstValue::Variant { enum_id, variant_id };
        let expr = hir::Expr::Const(value, hir::ConstID::dummy());
        return TypeResult::new(hir::Type::Enum(enum_id, &[]), expr);
    }

    let expr = if ctx.in_const > 0 {
        let mut valid = true;
        let offset = ctx.cache.const_values.start();
        for (idx, field) in input.iter().copied().enumerate() {
            match field {
                hir::Expr::Error => valid = false,
                hir::Expr::Const(value, _) => ctx.cache.const_values.push(*value),
                _ => {
                    valid = false;
                    let src = ctx.src(arg_list.unwrap().exprs[idx].range);
                    err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
                }
            }
        }
        if valid {
            let values = ctx.cache.const_values.take(offset, &mut ctx.arena);
            let variant = hir::ConstVariant { variant_id, values, poly_types };
            let value = hir::ConstValue::VariantPoly { enum_id, variant: ctx.arena.alloc(variant) };
            hir::Expr::Const(value, hir::ConstID::dummy())
        } else {
            ctx.cache.const_values.pop_view(offset);
            hir::Expr::Error
        }
    } else {
        let input = ctx.arena.alloc((input, poly_types));
        hir::Expr::Variant { enum_id, variant_id, input }
    };

    TypeResult::new(hir::Type::Enum(enum_id, poly_types), expr)
}

fn check_variant_bind_count(
    ctx: &mut HirCtx,
    bind_list: Option<&ast::BindingList>,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    default: TextRange,
) {
    let enum_data = ctx.registry.enum_data(enum_id);
    let variant = enum_data.variant(variant_id);

    let input_count = bind_list.map(|bl| bl.binds.len()).unwrap_or(0);
    let expected_count = variant.fields.len();

    if expected_count == 0 && bind_list.is_some() {
        let src = ctx.src(bind_list.unwrap().range);
        let variant_src = SourceRange::new(enum_data.origin_id, variant.name.range);
        err::tycheck_unexpected_variant_bind_list(&mut ctx.emit, src, variant_src);
    } else if input_count != expected_count {
        let src = ctx.src(bind_list_opt_range(bind_list, default));
        let variant_src = SourceRange::new(enum_data.origin_id, variant.name.range);
        err::tycheck_unexpected_variant_bind_count(
            &mut ctx.emit,
            src,
            variant_src,
            input_count,
            expected_count,
        );
    }
}

fn check_variant_bind_list<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation<'hir>,
    bind_list: Option<&ast::BindingList>,
    variant: Option<&hir::Variant<'hir>>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
) -> &'hir [hir::VariableID] {
    let bind_list = match bind_list {
        Some(bind_list) => bind_list,
        None => return &[],
    };

    if in_or_pat && bind_list.binds.iter().any(|bind| matches!(bind, ast::Binding::Named(_, _))) {
        let src = ctx.src(bind_list.range);
        err::tycheck_pat_in_or_bindings(&mut ctx.emit, src);
        return &[];
    }

    let offset = ctx.cache.var_ids.start();
    for (idx, bind) in bind_list.binds.iter().enumerate() {
        let (mutt, name) = match *bind {
            ast::Binding::Named(mutt, name) => (mutt, name),
            ast::Binding::Discard(_) => {
                ctx.cache.var_ids.push(hir::VariableID::dummy());
                continue; //discarded binding
            }
        };

        let mut ty = hir::Type::Error;
        if let Some(variant) = variant {
            if let Some(field_id) = variant.field_id(idx) {
                let field = variant.field(field_id);
                if let Expectation::HasType(hir::Type::Enum(_, poly_types), _) = expect {
                    let field_ty =
                        type_substitute(ctx, !poly_types.is_empty(), poly_types, field.ty);
                    ty = match ref_mut {
                        Some(ref_mut) => {
                            if field_ty.is_error() {
                                hir::Type::Error
                            } else {
                                hir::Type::Reference(ref_mut, ctx.arena.alloc(field_ty))
                            }
                        }
                        None => field_ty,
                    }
                }
            }
        }

        if ctx.scope.check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit).is_ok()
        {
            let var = hir::Variable { mutt, name, ty, was_used: false };
            let var_id = ctx.scope.local.add_variable(var);
            ctx.cache.var_ids.push(var_id);
        }
    }

    ctx.cache.var_ids.take(offset, &mut ctx.arena)
}

//==================== ADDRESSABILITY ====================
// Addressability allows to check whether expression
// can be addressed (in <address> expr or <assign> stmt).
// The mutability rules and constraints are also checked here.

struct AddrResult {
    base: AddrBase,
    constraint: AddrConstraint,
}

enum AddrBase {
    Unknown,
    Temporary,
    SliceField,
    Constant(SourceRange),
    Variable(ast::Mut, SourceRange),
}

enum AddrConstraint {
    None,
    AllowMut,
    ImmutRef,
    ImmutMulti,
    ImmutSlice,
}

impl AddrConstraint {
    #[inline]
    fn set(&mut self, new: AddrConstraint) {
        match self {
            AddrConstraint::None => *self = new,
            AddrConstraint::AllowMut => {
                if !matches!(new, AddrConstraint::None | AddrConstraint::AllowMut) {
                    *self = new;
                }
            }
            AddrConstraint::ImmutRef => {}
            AddrConstraint::ImmutMulti => {}
            AddrConstraint::ImmutSlice => {}
        }
    }
}

fn resolve_expr_addressability(ctx: &HirCtx, expr: &hir::Expr) -> AddrResult {
    let mut expr = expr;
    let mut constraint = AddrConstraint::None;

    loop {
        let base = match *expr {
            hir::Expr::Error => AddrBase::Unknown,
            hir::Expr::Const(_, const_id) => {
                if const_id != hir::ConstID::dummy() {
                    let data = ctx.registry.const_data(const_id);
                    AddrBase::Constant(data.src())
                } else {
                    AddrBase::Temporary
                }
            }
            hir::Expr::SliceField { .. } => AddrBase::SliceField,
            // addr_base: variable
            hir::Expr::ParamVar { param_id } => {
                let param = ctx.scope.local.param(param_id);
                let src = ctx.src(param.name.range);
                AddrBase::Variable(param.mutt, src)
            }
            hir::Expr::Variable { var_id } => {
                let var = ctx.scope.local.variable(var_id);
                let src = ctx.src(var.name.range);
                AddrBase::Variable(var.mutt, src)
            }
            hir::Expr::GlobalVar { global_id } => {
                let data = ctx.registry.global_data(global_id);
                AddrBase::Variable(data.mutt, data.src())
            }
            // access chains before addr_base
            hir::Expr::StructField { target, access } => {
                match access.deref {
                    Some(ast::Mut::Mutable) => constraint.set(AddrConstraint::AllowMut),
                    Some(ast::Mut::Immutable) => constraint.set(AddrConstraint::ImmutRef),
                    None => {}
                }
                expr = target;
                continue;
            }
            hir::Expr::Index { target, access } => {
                match access.deref {
                    Some(ast::Mut::Mutable) => constraint.set(AddrConstraint::AllowMut),
                    Some(ast::Mut::Immutable) => constraint.set(AddrConstraint::ImmutRef),
                    None => {}
                }
                match access.kind {
                    hir::IndexKind::Multi(ast::Mut::Mutable) => {
                        constraint.set(AddrConstraint::AllowMut)
                    }
                    hir::IndexKind::Slice(ast::Mut::Mutable) => {
                        constraint.set(AddrConstraint::AllowMut)
                    }
                    hir::IndexKind::Multi(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutMulti)
                    }
                    hir::IndexKind::Slice(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutSlice)
                    }
                    hir::IndexKind::Array(_) => {}
                    hir::IndexKind::ArrayEnum(_) => {}
                    hir::IndexKind::ArrayCore => {}
                }
                expr = target;
                continue;
            }
            hir::Expr::Slice { target, access } => {
                match access.deref {
                    Some(ast::Mut::Mutable) => constraint.set(AddrConstraint::AllowMut),
                    Some(ast::Mut::Immutable) => constraint.set(AddrConstraint::ImmutRef),
                    None => {}
                }
                match access.kind {
                    hir::SliceKind::Slice(ast::Mut::Mutable) => {
                        constraint.set(AddrConstraint::AllowMut)
                    }
                    hir::SliceKind::Slice(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutSlice)
                    }
                    hir::SliceKind::Array => {}
                }
                expr = target;
                continue;
            }
            hir::Expr::Deref { rhs, mutt, .. } => {
                match mutt {
                    ast::Mut::Mutable => constraint.set(AddrConstraint::AllowMut),
                    ast::Mut::Immutable => constraint.set(AddrConstraint::ImmutRef),
                }
                expr = rhs;
                continue;
            }
            _ => AddrBase::Temporary,
        };

        return AddrResult { base, constraint };
    }
}

fn check_slice_addressability(
    ctx: &mut HirCtx,
    mut addr_res: AddrResult,
    collection: &CollectionType,
    expr_range: TextRange,
) -> ast::Mut {
    let src = ctx.src(expr_range);
    let action = "slice";

    if let Some(deref) = collection.deref {
        match deref {
            ast::Mut::Mutable => addr_res.constraint.set(AddrConstraint::AllowMut),
            ast::Mut::Immutable => addr_res.constraint.set(AddrConstraint::ImmutRef),
        }
    }
    if let CollectionKind::Slice(slice) = collection.kind {
        match slice.mutt {
            ast::Mut::Mutable => addr_res.constraint.set(AddrConstraint::AllowMut),
            ast::Mut::Immutable => addr_res.constraint.set(AddrConstraint::ImmutSlice),
        }
    }

    if let AddrBase::Constant(const_src) = addr_res.base {
        err::tycheck_addr_const(&mut ctx.emit, src, const_src, action);
    }
    match addr_res.constraint {
        AddrConstraint::None => {
            if let AddrBase::Variable(var_mutt, _) = addr_res.base {
                var_mutt
            } else {
                ast::Mut::Mutable
            }
        }
        AddrConstraint::AllowMut => ast::Mut::Mutable,
        AddrConstraint::ImmutRef | AddrConstraint::ImmutMulti | AddrConstraint::ImmutSlice => {
            ast::Mut::Immutable
        }
    }
}

fn check_address_addressability(
    ctx: &mut HirCtx,
    mutt: ast::Mut,
    addr_res: &AddrResult,
    expr_range: TextRange,
) {
    let src = ctx.src(expr_range);
    let action = match mutt {
        ast::Mut::Mutable => "get &mut to",
        ast::Mut::Immutable => "get & to",
    };

    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => err::tycheck_addr(&mut ctx.emit, src, action, "slice field"),
        AddrBase::Temporary => {
            if mutt == ast::Mut::Mutable {
                check_address_mut_constraint(ctx, addr_res, src, action)
            }
        }
        AddrBase::Constant(const_src) => {
            err::tycheck_addr_const(&mut ctx.emit, src, const_src, action)
        }
        AddrBase::Variable(var_mutt, var_src) => {
            if mutt == ast::Mut::Mutable {
                if let AddrConstraint::None = addr_res.constraint {
                    if var_mutt == ast::Mut::Immutable {
                        err::tycheck_addr_variable(&mut ctx.emit, src, var_src, action);
                    }
                } else {
                    check_address_mut_constraint(ctx, addr_res, src, action);
                }
            }
        }
    }
}

fn check_for_elem_addressability(
    ctx: &mut HirCtx,
    ref_mut: Option<ast::Mut>,
    mut addr_res: AddrResult,
    collection: &CollectionType,
    expr_range: TextRange,
) {
    if let AddrBase::Constant(const_src) = addr_res.base {
        let src = ctx.src(expr_range);
        err::tycheck_addr_const(&mut ctx.emit, src, const_src, "iterate on");
        return;
    }
    let mutt = match ref_mut {
        Some(mutt) => mutt,
        None => return,
    };
    if let Some(deref) = collection.deref {
        match deref {
            ast::Mut::Mutable => addr_res.constraint.set(AddrConstraint::AllowMut),
            ast::Mut::Immutable => addr_res.constraint.set(AddrConstraint::ImmutRef),
        }
    }
    if let CollectionKind::Slice(slice) = collection.kind {
        match slice.mutt {
            ast::Mut::Mutable => addr_res.constraint.set(AddrConstraint::AllowMut),
            ast::Mut::Immutable => addr_res.constraint.set(AddrConstraint::ImmutSlice),
        }
    }
    check_address_addressability(ctx, mutt, &addr_res, expr_range);
}

fn check_assign_addressability(ctx: &mut HirCtx, addr_res: &AddrResult, expr_range: TextRange) {
    let src = ctx.src(expr_range);
    let action = "assign to";

    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => err::tycheck_addr(&mut ctx.emit, src, action, "slice field"),
        AddrBase::Temporary => err::tycheck_addr(&mut ctx.emit, src, action, "temporary value"),
        AddrBase::Constant(const_src) => {
            err::tycheck_addr_const(&mut ctx.emit, src, const_src, action);
        }
        AddrBase::Variable(var_mutt, var_src) => {
            if let AddrConstraint::None = addr_res.constraint {
                if var_mutt == ast::Mut::Immutable {
                    err::tycheck_addr_variable(&mut ctx.emit, src, var_src, action);
                }
            } else {
                check_address_mut_constraint(ctx, addr_res, src, action);
            }
        }
    }
}

fn check_address_mut_constraint(
    ctx: &mut HirCtx,
    addr_res: &AddrResult,
    src: SourceRange,
    action: &'static str,
) {
    match addr_res.constraint {
        AddrConstraint::None => {}
        AddrConstraint::AllowMut => {}
        AddrConstraint::ImmutRef => {
            let subject = "value behind an immutable reference";
            err::tycheck_addr(&mut ctx.emit, src, action, subject);
        }
        AddrConstraint::ImmutMulti => {
            let subject = "value behind an immutable multi-reference";
            err::tycheck_addr(&mut ctx.emit, src, action, subject);
        }
        AddrConstraint::ImmutSlice => {
            let subject = "value behind an immutable slice";
            err::tycheck_addr(&mut ctx.emit, src, action, subject);
        }
    }
}

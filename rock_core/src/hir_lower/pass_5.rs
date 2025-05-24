use super::check_directive;
use super::check_match;
use super::check_path::{self, ValueID};
use super::context::HirCtx;
use super::layout;
use super::pass_4;
use super::scope::{self, BlockStatus, Diverges, InferContext, PolyScope};
use super::types;
use crate::ast;
use crate::error::{ErrorSink, SourceRange, StringOrStr};
use crate::errors as err;
use crate::hir::{self, BoolType, CmpPred, FloatType, IntType, StringType};
use crate::support::AsStr;
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
            err::scope_symbol_unused(&mut ctx.emit, src, name, "parameter");
        }
    }
    let discard_id = ctx.session.intern_name.intern("_");
    for var in variables {
        if !var.was_used && var.name.id != discard_id {
            let src = ctx.src(var.name.range);
            let name = ctx.name(var.name.id);
            err::scope_symbol_unused(&mut ctx.emit, src, name, "variable");
        }
    }
}

//@remove ref / multi ref coercion for polymorphic types
// make coercion behavior conditional, right now T(&N) and T([&]N) is considered same.
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
        // implicit conversion: [&]T -> &T
        (hir::Type::Reference(mutt, ref_ty), hir::Type::MultiReference(mutt2, ref_ty2)) => {
            (mutt2 == ast::Mut::Mutable || mutt == mutt2) && type_matches(ctx, *ref_ty, *ref_ty2)
        }
        // implicit conversion: &T -> [&]T
        (hir::Type::MultiReference(mutt, ref_ty), hir::Type::Reference(mutt2, ref_ty2)) => {
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
        //prevent arrays with error sizes from erroring
        (hir::Type::ArrayStatic(array), _) => ctx.array_len(array.len).is_err(),
        (_, hir::Type::ArrayStatic(array2)) => ctx.array_len(array2.len).is_err(),
        _ => false,
    }
}

pub fn type_format(ctx: &HirCtx, ty: hir::Type) -> StringOrStr {
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
                    format.push_str(gen_fmt.as_str());
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
                    format.push_str(gen_fmt.as_str());
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
            let format = format!("&{mut_str}{}", ref_ty_format.as_str());
            format.into()
        }
        hir::Type::MultiReference(mutt, ref_ty) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            let ref_ty_format = type_format(ctx, *ref_ty);
            let format = format!("[&{mut_str}]{}", ref_ty_format.as_str());
            format.into()
        }
        hir::Type::Procedure(proc_ty) => {
            let mut format = String::from("proc");
            if proc_ty.flag_set.contains(hir::ProcFlag::External) {
                format.push_str(" #c_call");
            }
            format.push_str("(");
            for (idx, param) in proc_ty.params.iter().enumerate() {
                match param.kind {
                    hir::ParamKind::Normal => {
                        let param_ty = type_format(ctx, param.ty);
                        format.push_str(param_ty.as_str());
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
            format.push_str(return_ty.as_str());
            format.into()
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            let elem_format = type_format(ctx, slice.elem_ty);
            let format = format!("[{}]{}", mut_str, elem_format.as_str());
            format.into()
        }
        hir::Type::ArrayStatic(array) => {
            let elem_format = type_format(ctx, array.elem_ty);
            let format = match ctx.array_len(array.len) {
                Ok(len) => format!("[{}]{}", len, elem_format.as_str()),
                Err(()) => format!("[<unknown>]{}", elem_format.as_str()),
            };
            format.into()
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
        Expectation::HasType(hir::Type::Int(IntType::Usize), None);
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
    fn infer_array_elem(&self) -> Expectation<'hir> {
        match self {
            Expectation::None => Expectation::None,
            Expectation::HasType(expect_ty, expect_src) => match expect_ty {
                hir::Type::Error => Expectation::ERROR,
                hir::Type::ArrayStatic(array) => Expectation::HasType(array.elem_ty, *expect_src),
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
    if type_matches(ctx, expect_ty, found_ty) {
        return;
    }
    let src = ctx.src(from_range);
    let expected_ty = type_format(ctx, expect_ty);
    let found_ty = type_format(ctx, found_ty);
    err::tycheck_type_mismatch(
        &mut ctx.emit,
        src,
        expect_src,
        expected_ty.as_str(),
        found_ty.as_str(),
    );
}

fn check_expect_integer(ctx: &mut HirCtx, range: TextRange, ty: hir::Type) {
    match ty {
        hir::Type::Error | hir::Type::Int(_) => {}
        _ => {
            let src = ctx.src(range);
            let ty = type_format(ctx, ty);
            err::tycheck_expected_integer(&mut ctx.emit, src, ty.as_str());
        }
    }
}

fn check_expect_boolean(ctx: &mut HirCtx, range: TextRange, ty: hir::Type) {
    match ty {
        hir::Type::Error | hir::Type::Bool(_) => {}
        _ => {
            let src = ctx.src(range);
            let ty = type_format(ctx, ty);
            err::tycheck_expected_boolean(&mut ctx.emit, src, ty.as_str());
        }
    }
}

#[must_use]
pub struct TypeResult<'hir> {
    pub ty: hir::Type<'hir>,
    expr: hir::Expr<'hir>,
    ignore: bool,
}

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
    diverges: bool, //@temp fix: used in for loop gen to not generate incrementing code
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
        diverges: bool,
    ) -> BlockResult<'hir> {
        BlockResult { ty, block, tail_range, diverges }
    }

    fn into_type_result(self) -> TypeResult<'hir> {
        TypeResult::new(self.ty, hir::Expr::Block { block: self.block })
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
        ast::ExprKind::Lit { lit } => typecheck_lit(expect, lit),
        ast::ExprKind::If { if_ } => typecheck_if(ctx, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            typecheck_block(ctx, expect, *block, BlockStatus::None).into_type_result()
        }
        ast::ExprKind::Match { match_ } => typecheck_match(ctx, expect, match_, expr.range),
        ast::ExprKind::Field { target, name } => typecheck_field(ctx, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(ctx, expr.range, target, index),
        ast::ExprKind::Slice { target, range } => typecheck_slice(ctx, target, range, expr.range),
        ast::ExprKind::Call { target, args_list } => typecheck_call(ctx, target, args_list),
        ast::ExprKind::Cast { target, into } => typecheck_cast(ctx, expr.range, target, into),
        ast::ExprKind::Builtin { builtin } => typecheck_builtin(ctx, expr.range, builtin),
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
        ast::ExprKind::Deref { rhs } => typecheck_deref(ctx, rhs, expr.range),
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(ctx, mutt, rhs, expr.range),
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(ctx, expr.range, op, op_range, rhs)
        }
        ast::ExprKind::Binary { op, op_start, lhs, rhs } => {
            typecheck_binary(ctx, expect, expr.range, op, op_start, lhs, rhs)
        }
    };

    if untyped_promote {
        let with = expect.inner_type();
        let promoted =
            promote_untyped(ctx, expr.range, expr_res.expr, &mut expr_res.ty, with, true);
        match promoted {
            Some(Ok((v, id))) => expr_res.expr = hir::Expr::Const(v, id),
            Some(Err(())) => {} //@handle differently?
            None => {}
        }
    }

    if !expr_res.ignore {
        type_expectation_check(ctx, expr.range, expr_res.ty, expect);
    }
    expr_res.into_expr_result(ctx)
}

fn typecheck_lit<'hir>(expect: Expectation<'hir>, lit: ast::Lit) -> TypeResult<'hir> {
    let (ty, value) = match lit {
        ast::Lit::Void => (hir::Type::Void, hir::ConstValue::Void),
        ast::Lit::Null => {
            //coerce `null` to all other pointer types
            let ptr_ty = if let Some(expect_ty) = expect.inner_type() {
                match expect_ty {
                    hir::Type::Reference(_, _)
                    | hir::Type::MultiReference(_, _)
                    | hir::Type::Procedure(_) => expect_ty,
                    _ => hir::Type::Rawptr,
                }
            } else {
                hir::Type::Rawptr
            };
            (ptr_ty, hir::ConstValue::Null)
        }
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
                    let match_ = ctx.arena.alloc(match_);
                    ctx.arena.alloc(hir::Expr::Match { match_ })
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
        let src = ctx.src(expr_range);
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
        let match_ = ctx.arena.alloc(match_);
        TypeResult::new_ignore(match_type, hir::Expr::Match { match_ })
    } else {
        TypeResult::error()
    }
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
        ast::PatKind::Item { path, bind_list } => {
            typecheck_pat_item(ctx, expect, path, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Variant { name, bind_list } => {
            typecheck_pat_variant(ctx, expect, name, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Or { pats } => typecheck_pat_or(ctx, expect, pats, ref_mut),
    };

    //@temp hacky fix, using expression based promotion, refactor const value promotion
    let with = expect.inner_type();
    let expr = match pat_res.pat {
        hir::Pat::Lit(value) => hir::Expr::Const(value, hir::ConstID::dummy()),
        _ => hir::Expr::Error,
    };
    let promoted = promote_untyped(ctx, pat.range, expr, &mut pat_res.pat_ty, with, true);
    match promoted {
        Some(Ok((value, _))) => pat_res.pat = hir::Pat::Lit(value),
        Some(Err(())) => {} //@handle differently?
        None => {}
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
    //@pass correct in_definition
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
    emit_field_expr(ctx, target_res.expr, field_res)
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
        hir::Type::String(StringType::String) => {
            let field_name = ctx.name(name.id);
            match field_name {
                "len" => {
                    let kind = FieldKind::ArraySlice { field: hir::SliceField::Len };
                    let field_ty = hir::Type::Int(IntType::Usize);
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
                    hir::SliceField::Len => hir::Type::Int(IntType::Usize),
                };
                FieldResult::new(deref, kind, field_ty)
            }
            None => FieldResult::error(),
        },
        hir::Type::ArrayStatic(array) => match check_field_from_array(ctx, name, array) {
            Some(len) => {
                let kind = FieldKind::ArrayStatic { len };
                let field_ty = hir::Type::Int(IntType::Usize);
                FieldResult::new(deref, kind, field_ty)
            }
            None => FieldResult::error(),
        },
        _ => {
            let src = ctx.src(name.range);
            let field_name = ctx.name(name.id);
            let ty_fmt = type_format(ctx, ty);
            err::tycheck_field_not_found_ty(&mut ctx.emit, src, field_name, ty_fmt.as_str());
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
    array: &hir::ArrayStatic,
) -> Option<hir::ConstValue<'hir>> {
    let field_name = ctx.name(name.id);
    match field_name {
        "len" => {
            let len = match array.len {
                hir::ArrayStaticLen::Immediate(len) => {
                    hir::ConstValue::from_u64(len, IntType::Usize)
                }
                hir::ArrayStaticLen::ConstEval(eval_id) => {
                    let (eval, _, _) = ctx.registry.const_eval(eval_id);
                    eval.resolved().ok()?
                }
            };
            Some(len)
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
                        hir::ConstValue::from_u64(len as u64, IntType::Usize)
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
}

fn type_as_collection(mut ty: hir::Type) -> Result<Option<CollectionType>, ()> {
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
        _ => Err(()),
    }
}

fn typecheck_index<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: TextRange,
    target: &ast::Expr<'ast>,
    index: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let index_res = typecheck_expr(ctx, Expectation::USIZE, index);

    let collection = match type_as_collection(target_res.ty) {
        Ok(None) => return TypeResult::error(),
        Ok(Some(value)) => value,
        Err(()) => {
            let src = ctx.src(range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_index_on_ty(&mut ctx.emit, src, ty_fmt.as_str());
            return TypeResult::error();
        }
    };

    let kind = match collection.kind {
        CollectionKind::Multi(mutt) => hir::IndexKind::Multi(mutt),
        CollectionKind::Slice(slice) => hir::IndexKind::Slice(slice.mutt),
        CollectionKind::Array(array) => hir::IndexKind::Array(array.len),
    };
    let access = hir::IndexAccess {
        deref: collection.deref,
        elem_ty: collection.elem_ty,
        kind,
        index: index_res.expr,
        offset: target.range.end(),
    };

    if let hir::Expr::Const(target, target_id) = target_res.expr {
        if let hir::Expr::Const(index, _) = index_res.expr {
            let index = index.into_int_u64();
            let array_len = match target {
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

    let index = hir::Expr::Index { target: target_res.expr, access: ctx.arena.alloc(access) };
    TypeResult::new(collection.elem_ty, index)
}

fn typecheck_slice<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    range: &ast::SliceRange<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let addr_res = resolve_expr_addressability(ctx, target_res.expr);
    //@check that expression value is addressable

    let collection = match type_as_collection(target_res.ty) {
        Ok(None) => return TypeResult::error(),
        Ok(Some(collection)) => match collection.kind {
            CollectionKind::Slice(_) | CollectionKind::Array(_) => collection,
            CollectionKind::Multi(_) => {
                let src = ctx.src(target.range);
                let ty_fmt = type_format(ctx, target_res.ty);
                err::tycheck_cannot_slice_on_type(&mut ctx.emit, src, ty_fmt.as_str());
                return TypeResult::error();
            }
        },
        Err(()) => {
            let src = ctx.src(expr_range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_slice_on_type(&mut ctx.emit, src, ty_fmt.as_str());
            return TypeResult::error();
        }
    };

    //@always allowing mutable access for now, fix!
    let return_ty = if let Some(string_ty) = collection.string_ty {
        hir::Type::String(string_ty)
    } else {
        let slice = hir::ArraySlice { mutt: ast::Mut::Mutable, elem_ty: collection.elem_ty };
        hir::Type::ArraySlice(ctx.arena.alloc(slice))
    };

    let slice = match collection.kind {
        CollectionKind::Multi(_) => unreachable!(),
        CollectionKind::Slice(_) => {
            if let hir::Type::Reference(mutt, ref_ty) = target_res.ty {
                let deref = hir::Expr::Deref { rhs: target_res.expr, mutt, ref_ty };
                ctx.arena.alloc(deref)
            } else {
                target_res.expr
            }
        }
        CollectionKind::Array(array) => {
            let ptr = if let hir::Type::Reference(mutt, ref_ty) = target_res.ty {
                let deref = hir::Expr::Deref { rhs: target_res.expr, mutt, ref_ty };
                ctx.arena.alloc(deref)
            } else {
                ctx.arena.alloc(hir::Expr::Address { rhs: target_res.expr })
            };
            let len = ctx.array_len(array.len).unwrap_or(0);
            let builtin = hir::Builtin::RawSlice(ptr, len);
            let builtin = hir::Expr::Builtin { builtin: ctx.arena.alloc(builtin) };
            ctx.arena.alloc(builtin)
        }
    };

    // return the full slice in case of `[..]`
    if range.start.is_none() && range.end.is_none() {
        return TypeResult::new(return_ty, *slice);
    }

    // range start
    let start = if let Some(start) = range.start {
        typecheck_expr(ctx, Expectation::USIZE, start).expr
    } else {
        let zero_usize = hir::ConstValue::Int { val: 0, neg: false, int_ty: IntType::Usize };
        ctx.arena.alloc(hir::Expr::Const(zero_usize, hir::ConstID::dummy()))
    };

    // range bound
    let enum_id = ctx.core.range_bound.unwrap_or(hir::EnumID::dummy());
    let (variant_id, input) = if let Some((kind, end)) = range.end {
        let variant_id = match kind {
            ast::RangeKind::Exclusive => hir::VariantID::new(1),
            ast::RangeKind::Inclusive => hir::VariantID::new(2),
        };
        let idx = typecheck_expr(ctx, Expectation::USIZE, end).expr;
        (variant_id, ctx.arena.alloc_slice(&[idx]))
    } else {
        (hir::VariantID::new(0), ctx.arena.alloc_slice(&[]))
    };
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
        CollectionKind::Array(_) => hir::SliceKind::Array,
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
                integer_cast_kind(ctx, from_ty, into_ty)
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
                let into_size = layout::int_layout(ctx, into_ty).size;
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

            let from_size = layout::int_layout(ctx, from_ty).size;
            let into_size = layout::int_layout(ctx, into_ty).size;

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
            if type_matches(ctx, into, from) {
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
        err::tycheck_cast_invalid(&mut ctx.emit, src, from_ty.as_str(), into_ty.as_str());
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

fn integer_cast_kind(ctx: &HirCtx, from_ty: IntType, into_ty: IntType) -> hir::CastKind {
    use hir::CastKind;
    use std::cmp::Ordering;

    if from_ty == IntType::Untyped || into_ty == IntType::Untyped {
        return hir::CastKind::Int_NoOp;
    }

    let from_size = layout::int_layout(ctx, from_ty).size;
    let into_size = layout::int_layout(ctx, into_ty).size;
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

fn typecheck_builtin<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: TextRange,
    builtin: &ast::Builtin<'ast>,
) -> TypeResult<'hir> {
    let src = ctx.src(range);
    match *builtin {
        ast::Builtin::Error(name) => {
            let name = ctx.name(name.id);
            err::tycheck_builtin_unknown(&mut ctx.emit, src, name);
            TypeResult::error()
        }
        ast::Builtin::SizeOf(ty) => {
            let ty = super::pass_3::type_resolve(ctx, ty, false); //@in def?
            let expr = if types::has_poly_layout_dep(ty) {
                let builtin = hir::Builtin::SizeOf(ty);
                hir::Expr::Builtin { builtin: ctx.arena.alloc(builtin) }
            } else {
                layout::type_layout(ctx, ty, &[], src)
                    .map(|layout| {
                        int_range_check(ctx, src, layout.size as i128, IntType::Usize)
                            .map(|value| hir::Expr::Const(value, hir::ConstID::dummy()))
                            .unwrap_or(hir::Expr::Error)
                    })
                    .unwrap_or(hir::Expr::Error)
            };
            TypeResult::new(hir::Type::Int(IntType::Usize), expr)
        }
        ast::Builtin::AlignOf(ty) => {
            let ty = super::pass_3::type_resolve(ctx, ty, false); //@in def?
            let expr = if types::has_poly_layout_dep(ty) {
                let builtin = hir::Builtin::AlignOf(ty);
                hir::Expr::Builtin { builtin: ctx.arena.alloc(builtin) }
            } else {
                layout::type_layout(ctx, ty, &[], src)
                    .map(|layout| {
                        int_range_check(ctx, src, layout.align as i128, IntType::Usize)
                            .map(|value| hir::Expr::Const(value, hir::ConstID::dummy()))
                            .unwrap_or(hir::Expr::Error)
                    })
                    .unwrap_or(hir::Expr::Error)
            };
            TypeResult::new(hir::Type::Int(IntType::Usize), expr)
        }
        ast::Builtin::Transmute(expr, into) => {
            let from_src = ctx.src(expr.range);
            let into_src = ctx.src(into.range);
            let expr_res = typecheck_expr(ctx, Expectation::None, expr);
            let into_ty = super::pass_3::type_resolve(ctx, into, false);

            if types::has_poly_layout_dep(expr_res.ty) {
                let ty = type_format(ctx, expr_res.ty);
                err::tycheck_transumute_poly_dep(&mut ctx.emit, from_src, ty.as_str());
                return TypeResult::error();
            }
            if types::has_poly_layout_dep(into_ty) {
                let ty = type_format(ctx, into_ty);
                err::tycheck_transumute_poly_dep(&mut ctx.emit, into_src, ty.as_str());
                return TypeResult::error();
            }

            let from_res = layout::type_layout(ctx, expr_res.ty, &[], from_src);
            let into_res = layout::type_layout(ctx, into_ty, &[], into_src);

            if let (Ok(from_layout), Ok(into_layout)) = (from_res, into_res) {
                let subject = if from_layout.size != into_layout.size {
                    "size"
                } else if from_layout.align != into_layout.align {
                    "alignment"
                } else {
                    let builtin = hir::Builtin::Transmute(expr_res.expr, into_ty);
                    let expr = hir::Expr::Builtin { builtin: ctx.arena.alloc(builtin) };
                    return TypeResult::new(into_ty, expr);
                };

                let from_ty = type_format(ctx, expr_res.ty);
                let into_ty = type_format(ctx, into_ty);
                err::tycheck_transmute_mismatch(
                    &mut ctx.emit,
                    src,
                    subject,
                    from_ty.as_str(),
                    into_ty.as_str(),
                );
                TypeResult::error()
            } else {
                TypeResult::error()
            }
        }
        ast::Builtin::AtomicLoad(args) => {
            if !check_call_arg_count(ctx, &args, None, 2, false) {
                return TypeResult::error();
            }
            let src = ctx.src(range);
            err::internal_not_implemented(&mut ctx.emit, src, "@atomic_load");
            TypeResult::error()
        }
        ast::Builtin::AtomicStore(args) => {
            if !check_call_arg_count(ctx, &args, None, 3, false) {
                return TypeResult::error();
            }
            let src = ctx.src(range);
            err::internal_not_implemented(&mut ctx.emit, src, "@atomic_store");
            TypeResult::error()
        }
        ast::Builtin::AtomicOp(args) => {
            if !check_call_arg_count(ctx, &args, None, 4, false) {
                return TypeResult::error();
            }
            let src = ctx.src(range);
            err::internal_not_implemented(&mut ctx.emit, src, "@atomic_op");
            TypeResult::error()
        }
        ast::Builtin::AtomicCompareSwap(weak, args) => {
            if !check_call_arg_count(ctx, &args, None, 5, false) {
                return TypeResult::error();
            }
            let src = ctx.src(range);
            if weak {
                err::internal_not_implemented(&mut ctx.emit, src, "@atomic_compare_swap_weak");
            } else {
                err::internal_not_implemented(&mut ctx.emit, src, "@atomic_compare_swap");
            }
            TypeResult::error()
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
    //@pass correct in_definition
    let (item_res, fields) = match check_path::path_resolve_value(ctx, path, false) {
        ValueID::None => {
            if let Some(arg_list) = args_list {
                default_check_arg_list(ctx, arg_list);
            }
            return TypeResult::error();
        }
        ValueID::Proc(proc_id, poly_types) => {
            if let Some(args_list) = args_list {
                return check_call_direct(
                    ctx,
                    expect,
                    proc_id,
                    poly_types,
                    args_list,
                    expr_range.start(),
                );
            } else {
                return check_item_procedure(ctx, expect, proc_id, poly_types, expr_range);
            }
        }
        ValueID::Enum(enum_id, variant_id, poly_types) => {
            return check_variant_input_opt(
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

    //@check segments for having poly types in them + rename
    for &name in fields {
        let expr_res = target_res.into_expr_result(ctx);
        let field_res = check_field_from_type(ctx, name.name, expr_res.ty);
        target_res = emit_field_expr(ctx, expr_res.expr, field_res);
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

pub fn check_item_procedure<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
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
    proc_ty.flag_set.clear(hir::ProcFlag::Inline);
    proc_ty.flag_set.clear(hir::ProcFlag::WasUsed);
    proc_ty.flag_set.clear(hir::ProcFlag::EntryPoint);

    let poly_types = if poly_types.is_empty() { None } else { Some(ctx.arena.alloc(poly_types)) };
    let proc_ty = hir::Type::Procedure(ctx.arena.alloc(proc_ty));
    let proc_value = hir::ConstValue::Procedure { proc_id, poly_types };
    let proc_expr = hir::Expr::Const(proc_value, hir::ConstID::dummy());
    return TypeResult::new(proc_ty, proc_expr);
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

    check_variant_input_opt(ctx, expect, enum_id, variant_id, poly_types, args_list, expr_range)
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
        //@pass correct in_definition
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
    let offset_init = ctx.cache.field_inits.start();
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
            let field_init = hir::FieldInit { field_id, expr: input_res.expr };
            ctx.cache.field_inits.push(field_init);
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
    let poly_types = match poly_types {
        Some(poly_types) => poly_types,
        None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
    };
    ctx.scope.infer.end_context(infer);

    let expr = if ctx.in_const {
        if ctx.emit.did_error(error_count) {
            ctx.cache.field_inits.pop_view(offset_init);
            return TypeResult::error();
        }
        let const_offset = ctx.cache.const_values.start();
        let fields = ctx.cache.field_inits.view(offset_init);
        for field in fields {
            match field.expr {
                hir::Expr::Error => return TypeResult::error(),
                hir::Expr::Const(value, _) => ctx.cache.const_values.push(*value),
                _ => {
                    let src = ctx.src(struct_init.input[field.field_id.index()].expr.range);
                    err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
                    return TypeResult::error();
                }
            }
        }
        ctx.cache.field_inits.pop_view(offset_init);
        let values = ctx.cache.const_values.take(const_offset, &mut ctx.arena);

        let struct_ = hir::ConstStruct { values, poly_types };
        let struct_ = hir::ConstValue::Struct { struct_id, struct_: ctx.arena.alloc(struct_) };
        hir::Expr::Const(struct_, hir::ConstID::dummy())
    } else {
        let input = ctx.cache.field_inits.take(offset_init, &mut ctx.arena);
        if poly_types.is_empty() {
            hir::Expr::StructInit { struct_id, input }
        } else {
            hir::Expr::StructInitPoly { struct_id, input: ctx.arena.alloc((input, poly_types)) }
        }
    };

    TypeResult::new(hir::Type::Struct(struct_id, poly_types), expr)
}

fn typecheck_array_init<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    range: TextRange,
    input: &[&ast::Expr<'ast>],
) -> TypeResult<'hir> {
    let error_count = ctx.emit.error_count();
    let mut elem_ty = None;
    let mut expect = expect.infer_array_elem();

    let offset = ctx.cache.exprs.start();
    for expr in input.iter().copied() {
        let expr_res = typecheck_expr(ctx, expect, expr);
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
                expect = Expectation::HasType(expr_res.ty, Some(ctx.src(expr.range)));
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

    let array_expr = if ctx.in_const {
        // cannot constfold if any value is error
        if ctx.emit.did_error(error_count) {
            ctx.cache.exprs.pop_view(offset);
            return TypeResult::error();
        }

        if input.is_empty() {
            ctx.cache.exprs.pop_view(offset);
            let array = hir::ConstValue::ArrayEmpty { elem_ty: ctx.arena.alloc(elem_ty) };
            hir::Expr::Const(array, hir::ConstID::dummy())
        } else {
            let const_offset = ctx.cache.const_values.start();
            let values = ctx.cache.exprs.view(offset);
            for (idx, value) in values.iter().enumerate() {
                match value {
                    hir::Expr::Error => return TypeResult::error(),
                    hir::Expr::Const(value, _) => ctx.cache.const_values.push(*value),
                    _ => {
                        let src = ctx.src(input[idx].range);
                        err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
                        return TypeResult::error();
                    }
                }
            }
            ctx.cache.exprs.pop_view(offset);
            let const_values = ctx.cache.const_values.take(const_offset, &mut ctx.arena);

            let array = hir::ConstArray { values: const_values };
            let array = hir::ConstValue::Array { array: ctx.arena.alloc(array) };
            hir::Expr::Const(array, hir::ConstID::dummy())
        }
    } else {
        let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
        let array = ctx.arena.alloc(hir::ArrayInit { elem_ty, input });
        hir::Expr::ArrayInit { array }
    };

    let len = hir::ArrayStaticLen::Immediate(input.len() as u64);
    let array_ty = ctx.arena.alloc(hir::ArrayStatic { len, elem_ty });
    TypeResult::new(hir::Type::ArrayStatic(array_ty), array_expr)
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

fn typecheck_deref<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    rhs: &ast::Expr<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, Expectation::None, rhs);

    match rhs_res.ty {
        hir::Type::Error => TypeResult::error(),
        hir::Type::Reference(mutt, ref_ty) => {
            TypeResult::new(*ref_ty, hir::Expr::Deref { rhs: rhs_res.expr, mutt, ref_ty })
        }
        _ => {
            let src = ctx.src(expr_range);
            let ty_fmt = type_format(ctx, rhs_res.ty);
            err::tycheck_cannot_deref_on_ty(&mut ctx.emit, src, ty_fmt.as_str());
            TypeResult::error()
        }
    }
}

fn typecheck_address<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    mutt: ast::Mut,
    rhs: &ast::Expr<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, Expectation::None, rhs);
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
                Ok(hir::UnOp::Neg_Int)
            }
            hir::Type::Float(_) => Ok(hir::UnOp::Neg_Float),
            _ => Err(()),
        },
        ast::UnOp::BitNot => match rhs_res.ty {
            hir::Type::Int(IntType::Untyped) => Err(()),
            hir::Type::Int(_) => Ok(hir::UnOp::BitNot),
            _ => Err(()),
        },
        ast::UnOp::LogicNot => match rhs_res.ty {
            hir::Type::Bool(_) => Ok(hir::UnOp::LogicNot),
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
        err::tycheck_un_op_cannot_apply(&mut ctx.emit, src, op.as_str(), rhs_ty.as_str());
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
        hir::UnOp::Neg_Int => {
            let int_ty = rhs.into_int_ty();
            let val = -rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::UnOp::Neg_Float => {
            let float_ty = rhs.into_float_ty();
            let val = -rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::UnOp::BitNot => {
            let int_ty = rhs.into_int_ty();
            if int_ty.is_signed() {
                Ok(hir::ConstValue::from_i64(!rhs.into_int_i64(), int_ty))
            } else {
                let value_bits = layout::int_layout(ctx, int_ty).size * 8;
                let mask = if value_bits == 64 { u64::MAX } else { (1 << value_bits) - 1 };
                Ok(hir::ConstValue::from_u64(mask & !rhs.into_int_u64(), int_ty))
            }
        }
        hir::UnOp::LogicNot => {
            let (val, bool_ty) = rhs.expect_bool();
            Ok(hir::ConstValue::Bool { val: !val, bool_ty })
        }
    }
}

#[must_use]
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

fn typecheck_binary<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    range: TextRange,
    op: ast::BinOp,
    op_start: TextOffset,
    lhs: &ast::Expr<'ast>,
    rhs: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    //@expectation model doesnt work for binary expr
    // allow `null` literals to coerce, allow enum inference for lhs and rhs, etc..
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

    let hir_op = match check_binary_op(ctx, expect, op, op_start, lhs_res.ty, rhs_res.ty) {
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

    let binary = hir::Expr::Binary { op: hir_op, lhs: lhs_res.expr, rhs: rhs_res.expr };
    TypeResult::new(res_ty, binary)
}

fn check_binary_op<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    expect: Expectation,
    op: ast::BinOp,
    op_start: TextOffset,
    lhs_ty: hir::Type<'hir>,
    rhs_ty: hir::Type,
) -> Result<hir::BinOp, ()> {
    if lhs_ty.is_error() || rhs_ty.is_error() {
        return Err(());
    }
    if op != ast::BinOp::BitShl && op != ast::BinOp::BitShr && !type_matches(ctx, lhs_ty, rhs_ty) {
        let op_len = op.as_str().len() as u32;
        let src = ctx.src(TextRange::new(op_start, op_start + op_len.into()));
        let lhs_ty = type_format(ctx, lhs_ty);
        let rhs_ty = type_format(ctx, rhs_ty);
        err::tycheck_bin_type_mismatch(&mut ctx.emit, src, lhs_ty.as_str(), rhs_ty.as_str());
        return Err(());
    }

    let hir_op = match op {
        ast::BinOp::Add => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::Add_Int),
            hir::Type::Float(_) => Ok(hir::BinOp::Add_Float),
            _ => Err(()),
        },
        ast::BinOp::Sub => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::Sub_Int),
            hir::Type::Float(_) => Ok(hir::BinOp::Sub_Float),
            _ => Err(()),
        },
        ast::BinOp::Mul => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::Mul_Int),
            hir::Type::Float(_) => Ok(hir::BinOp::Mul_Float),
            _ => Err(()),
        },
        ast::BinOp::Div => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Div_Int(int_ty)),
            hir::Type::Float(_) => Ok(hir::BinOp::Div_Float),
            _ => Err(()),
        },
        ast::BinOp::Rem => match lhs_ty {
            hir::Type::Int(int_ty) => Ok(hir::BinOp::Rem_Int(int_ty)),
            _ => Err(()),
        },
        ast::BinOp::BitAnd => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::BitAnd),
            _ => Err(()),
        },
        ast::BinOp::BitOr => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::BitOr),
            _ => Err(()),
        },
        ast::BinOp::BitXor => match lhs_ty {
            hir::Type::Int(_) => Ok(hir::BinOp::BitXor),
            _ => Err(()),
        },
        ast::BinOp::BitShl => match (lhs_ty, rhs_ty) {
            (hir::Type::Int(lhs_ty), hir::Type::Int(rhs_ty)) => {
                let cast = integer_cast_kind(ctx, rhs_ty, lhs_ty);
                Ok(hir::BinOp::BitShl(lhs_ty, cast))
            }
            _ => Err(()),
        },
        ast::BinOp::BitShr => match (lhs_ty, rhs_ty) {
            (hir::Type::Int(lhs_ty), hir::Type::Int(rhs_ty)) => {
                let cast = integer_cast_kind(ctx, rhs_ty, lhs_ty);
                Ok(hir::BinOp::BitShr(lhs_ty, cast))
            }
            _ => Err(()),
        },
        ast::BinOp::Eq => match lhs_ty {
            hir::Type::Char | hir::Type::UntypedChar | hir::Type::Rawptr | hir::Type::Bool(_) => {
                Ok(hir::BinOp::Eq_Int_Other(expect.infer_bool()))
            }
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
            hir::Type::Char | hir::Type::UntypedChar | hir::Type::Rawptr | hir::Type::Bool(_) => {
                Ok(hir::BinOp::NotEq_Int_Other(expect.infer_bool()))
            }
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

    if hir_op.is_err() {
        let op_len = op.as_str().len() as u32;
        let src = ctx.src(TextRange::new(op_start, op_start + op_len.into()));
        let lhs_ty = type_format(ctx, lhs_ty);
        let rhs_ty = type_format(ctx, rhs_ty);
        err::tycheck_bin_cannot_apply(
            &mut ctx.emit,
            src,
            op.as_str(),
            lhs_ty.as_str(),
            rhs_ty.as_str(),
        );
    }

    hir_op
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
            layout::int_layout(ctx, int_ty)
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
        hir::BinOp::Add_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() + rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::Add_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() + rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Sub_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() - rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::BinOp::Sub_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() - rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Mul_Int => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_mul(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Mul_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() * rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Div_Int(_) => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_div(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Div_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() / rhs.into_float();
            Ok(hir::ConstValue::Float { val, float_ty })
        }
        hir::BinOp::Rem_Int(_) => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_rem(rhs) {
                int_range_check(ctx, src, val, int_ty)
            } else {
                err::const_int_overflow(&mut ctx.emit, src, op.as_str(), lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::BitAnd => {
            let int_ty = lhs.into_int_ty();
            if int_ty.is_signed() {
                let val = lhs.into_int_i64() & rhs.into_int_i64();
                Ok(hir::ConstValue::from_i64(val, int_ty))
            } else {
                let val = lhs.into_int_u64() & rhs.into_int_u64();
                Ok(hir::ConstValue::from_u64(val, int_ty))
            }
        }
        hir::BinOp::BitOr => {
            let int_ty = lhs.into_int_ty();
            if int_ty.is_signed() {
                let val = lhs.into_int_i64() | rhs.into_int_i64();
                Ok(hir::ConstValue::from_i64(val, int_ty))
            } else {
                let val = lhs.into_int_u64() | rhs.into_int_u64();
                Ok(hir::ConstValue::from_u64(val, int_ty))
            }
        }
        hir::BinOp::BitXor => {
            let int_ty = lhs.into_int_ty();
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
                layout::int_layout(ctx, int_ty)
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
                hir::ConstValue::Char { val, .. } => val == rhs.into_char(),
                hir::ConstValue::Null => true, //only value: null == null
                hir::ConstValue::Bool { val, .. } => val == rhs.into_bool(),
                hir::ConstValue::Variant { variant_id, .. } => variant_id == rhs.into_enum().1,
                _ => unreachable!(),
            };
            Ok(hir::ConstValue::Bool { val, bool_ty })
        }
        hir::BinOp::NotEq_Int_Other(bool_ty) => {
            let val = match lhs {
                hir::ConstValue::Char { val, .. } => val != rhs.into_char(),
                hir::ConstValue::Null => false, //only value: null != null
                hir::ConstValue::Bool { val, .. } => val != rhs.into_bool(),
                hir::ConstValue::Variant { variant_id, .. } => variant_id != rhs.into_enum().1,
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
                //@can diverge any diverging expr inside
                check_stmt_diverges(ctx, false, stmt.range);
                let local_res = typecheck_local(ctx, local);
                match local_res {
                    LocalResult::Error => continue,
                    LocalResult::Local(local) => hir::Stmt::Local(local),
                    LocalResult::Discard(None) => continue,
                    LocalResult::Discard(Some(expr)) => hir::Stmt::Discard(expr),
                }
            }
            ast::StmtKind::Assign(assign) => {
                //@can diverge any diverging expr inside
                check_stmt_diverges(ctx, false, stmt.range);
                hir::Stmt::Assign(typecheck_assign(ctx, assign))
            }
            ast::StmtKind::ExprSemi(expr) => {
                //@can diverge but expression divergence isnt implemented (if, match, explicit `never` calls like panic)
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

                let will_diverge = expr_res.ty.is_never();
                check_stmt_diverges(ctx, will_diverge, stmt.range);
                hir::Stmt::ExprSemi(expr_res.expr)
            }
            ast::StmtKind::ExprTail(expr) => {
                // type expectation is delegated to tail expression, instead of the block itself
                let expr_res = typecheck_expr(ctx, expect, expr);
                let stmt_res = hir::Stmt::ExprTail(expr_res.expr);
                // @seems to fix the problem (still a hack) - will_diverge was `true`

                let will_diverge = expr_res.ty.is_never();
                check_stmt_diverges(ctx, will_diverge, stmt.range);
                block_tail_ty = Some(expr_res.ty);
                block_tail_range = Some(expr.range);
                stmt_res
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
                    if let Some((var_id, op)) = curr_block.for_idx_change {
                        let expr_var = ctx.arena.alloc(hir::Expr::Variable { var_id });
                        let expr_one = ctx.arena.alloc(hir::Expr::Const(
                            hir::ConstValue::Int { val: 1, neg: false, int_ty: IntType::Usize },
                            hir::ConstID::dummy(),
                        ));
                        let index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                            op: hir::AssignOp::Bin(op),
                            lhs: expr_var,
                            rhs: expr_one,
                            lhs_ty: hir::Type::Int(IntType::Usize),
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
                            op: hir::AssignOp::Bin(hir::BinOp::Add_Int),
                            lhs: expr_var,
                            rhs: expr_one,
                            lhs_ty: hir::Type::Int(int_ty),
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

    //@wip approach, will change 03.07.24
    let block_result = if let Some(block_ty) = block_tail_ty {
        BlockResult::new(block_ty, hir_block, block_tail_range, diverges)
    } else {
        //@potentially incorrect aproach, verify that `void`
        // as the expectation and block result ty are valid 29.05.24

        //@change to last `}` range?
        // verify that all block are actual blocks in that case
        if !diverges {
            type_expectation_check(ctx, block.range, hir::Type::Void, expect);
        }
        //@hack but should be correct
        let block_ty = if diverges { hir::Type::Never } else { hir::Type::Void };
        BlockResult::new(block_ty, hir_block, block_tail_range, diverges)
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

            let branch_cond = hir::Expr::Unary { op: hir::UnOp::LogicNot, rhs: cond_res.expr };
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
            let value_already_defined = match header.value {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let index_already_defined = match header.index {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };

            let expr_res = typecheck_expr(ctx, Expectation::None, header.expr);

            //@not checking mutability in cases of & or &mut iteration
            //@not checking runtime indexing (constants cannot be indexed at runtime)
            //@dont instantly return here, check the block also!
            let collection = match type_as_collection(expr_res.ty) {
                Ok(None) => return None,
                Ok(Some(collection)) => match collection.kind {
                    CollectionKind::Slice(_) | CollectionKind::Array(_) => collection,
                    CollectionKind::Multi(_) => {
                        let src = ctx.src(header.expr.range);
                        let ty_fmt = type_format(ctx, expr_res.ty);
                        err::tycheck_cannot_iter_on_type(&mut ctx.emit, src, ty_fmt.as_str());
                        return None;
                    }
                },
                Err(()) => {
                    let src = ctx.src(header.expr.range);
                    let ty_fmt = type_format(ctx, expr_res.ty);
                    err::tycheck_cannot_iter_on_type(&mut ctx.emit, src, ty_fmt.as_str());
                    return None;
                }
            };

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
            let index_var = hir::Variable {
                mutt: ast::Mut::Immutable,
                name: header.index.unwrap_or(name_dummy),
                ty: hir::Type::Int(IntType::Usize),
                was_used: false,
            };

            ctx.scope.local.start_block(BlockStatus::None);
            let value_id = if value_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(value_var)
            };
            let index_id = if index_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(index_var)
            };

            let curr_block = ctx.scope.local.current_block_mut();
            let index_change_op =
                if header.reverse { hir::BinOp::Sub_Int } else { hir::BinOp::Add_Int };
            curr_block.for_idx_change = Some((index_id, index_change_op));

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
                        int_ty: IntType::Usize,
                    };
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
                CollectionKind::Multi(_) => unreachable!(),
            };
            let expr_zero_usize = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 0, neg: false, int_ty: IntType::Usize },
                hir::ConstID::dummy(),
            ));
            let expr_one_usize = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 1, neg: false, int_ty: IntType::Usize },
                hir::ConstID::dummy(),
            ));

            // index local:
            // forward: let idx = 0;
            // reverse: let idx = iter.len;
            let index_init = if header.reverse { expr_iter_len } else { expr_zero_usize };
            let index_local =
                hir::Local { var_id: index_id, init: hir::LocalInit::Init(index_init) };
            let stmt_index = hir::Stmt::Local(ctx.arena.alloc(index_local));

            // loop statement:
            let expr_index_var = ctx.arena.alloc(hir::Expr::Variable { var_id: index_id });

            // conditional loop break:
            let cond_rhs = if header.reverse { expr_zero_usize } else { expr_iter_len };
            let cond_op = if header.reverse {
                hir::BinOp::Cmp_Int(CmpPred::Eq, BoolType::Bool, IntType::Usize)
            } else {
                hir::BinOp::Cmp_Int(CmpPred::GreaterEq, BoolType::Bool, IntType::Usize)
            };
            let branch_cond = hir::Expr::Binary { op: cond_op, lhs: expr_index_var, rhs: cond_rhs };
            let branch_block = hir::Block { stmts: ctx.arena.alloc_slice(&[hir::Stmt::Break]) };
            let branch = hir::Branch { cond: ctx.arena.alloc(branch_cond), block: branch_block };
            let expr_if = hir::If { branches: ctx.arena.alloc_slice(&[branch]), else_block: None };
            let expr_if = hir::Expr::If { if_: ctx.arena.alloc(expr_if) };
            let stmt_cond = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_if));

            // index expression:
            let index_kind = match collection.kind {
                CollectionKind::Array(array) => hir::IndexKind::Array(array.len),
                CollectionKind::Slice(slice) => hir::IndexKind::Slice(slice.mutt),
                CollectionKind::Multi(_) => unreachable!(),
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
                op: hir::AssignOp::Bin(index_change_op),
                lhs: expr_index_var,
                rhs: expr_one_usize,
                lhs_ty: hir::Type::Int(IntType::Usize),
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
            } else if block_res.diverges {
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
            let value_already_defined = match header.value {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };
            let index_already_defined = match header.index {
                Some(name) => ctx
                    .scope
                    .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
                    .is_err(),
                _ => false,
            };

            if let Some(start) = header.ref_start {
                let src = ctx.src(TextRange::new(start, start + 1.into()));
                err::tycheck_for_range_ref(&mut ctx.emit, src);
            }
            if let Some(start) = header.reverse_start {
                let src = ctx.src(TextRange::new(start, start + 2.into()));
                err::tycheck_for_range_reverse(&mut ctx.emit, src);
            }

            //@use untyped and unify the integer types
            let mut start_res = typecheck_expr_untyped(ctx, Expectation::None, header.start);
            check_expect_integer(ctx, header.start.range, start_res.ty);
            let mut end_res = typecheck_expr_untyped(ctx, Expectation::None, header.end);
            check_expect_integer(ctx, header.end.range, end_res.ty);

            let start_promote = promote_untyped(
                ctx,
                header.start.range,
                *start_res.expr,
                &mut start_res.ty,
                Some(end_res.ty),
                true,
            );
            let end_promote = promote_untyped(
                ctx,
                header.end.range,
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

            let int_ty = if let (hir::Type::Int(lhs), hir::Type::Int(rhs)) =
                (start_res.ty, end_res.ty)
            {
                if lhs != rhs {
                    let range = TextRange::new(header.start.range.start(), header.end.range.end());
                    let src = ctx.src(range);
                    err::tycheck_for_range_type_mismatch(
                        &mut ctx.emit,
                        src,
                        lhs.as_str(),
                        rhs.as_str(),
                    );
                }
                lhs //@could be wrong to always use lhs
            } else {
                IntType::S32 //default
            };

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
                ty: hir::Type::Int(IntType::Usize),
                was_used: false,
            };

            ctx.scope.local.start_block(BlockStatus::None);
            let start_id = if value_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(start_var)
            };
            let index_id = if index_already_defined {
                hir::VariableID::dummy()
            } else {
                ctx.scope.local.add_variable(index_var)
            };
            let end_id = ctx.scope.local.add_variable(end_var);

            let curr_block = ctx.scope.local.current_block_mut();
            curr_block.for_idx_change = Some((index_id, hir::BinOp::Add_Int));
            curr_block.for_value_change = Some((start_id, int_ty));

            let block_res = typecheck_block(ctx, Expectation::VOID, for_.block, BlockStatus::Loop);
            ctx.scope.local.exit_block();

            // start, end, index locals:
            let start_local =
                hir::Local { var_id: start_id, init: hir::LocalInit::Init(start_res.expr) };
            let stmt_start = hir::Stmt::Local(ctx.arena.alloc(start_local));

            let end_local = hir::Local { var_id: end_id, init: hir::LocalInit::Init(end_res.expr) };
            let stmt_end = hir::Stmt::Local(ctx.arena.alloc(end_local));

            let zero = hir::ConstValue::Int { val: 0, neg: false, int_ty: IntType::Usize };
            let zero = ctx.arena.alloc(hir::Expr::Const(zero, hir::ConstID::dummy()));
            let index_local = hir::Local { var_id: index_id, init: hir::LocalInit::Init(zero) };
            let stmt_index = hir::Stmt::Local(ctx.arena.alloc(index_local));

            // loop body block:
            let expr_start_var = ctx.arena.alloc(hir::Expr::Variable { var_id: start_id });
            let expr_end_var = ctx.arena.alloc(hir::Expr::Variable { var_id: end_id });
            let expr_index_var = ctx.arena.alloc(hir::Expr::Variable { var_id: index_id });

            let cond_op = match header.kind {
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
            let break_cond = hir::Expr::Unary { op: hir::UnOp::LogicNot, rhs: continue_cond };
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
                op: hir::AssignOp::Bin(hir::BinOp::Add_Int),
                lhs: expr_start_var,
                rhs: expr_one_iter,
                lhs_ty: hir::Type::Int(int_ty),
            }));
            let expr_one_usize = ctx.arena.alloc(hir::Expr::Const(
                hir::ConstValue::Int { val: 1, neg: false, int_ty: IntType::Usize },
                hir::ConstID::dummy(),
            ));
            let stmt_index_change = hir::Stmt::Assign(ctx.arena.alloc(hir::Assign {
                op: hir::AssignOp::Bin(hir::BinOp::Add_Int),
                lhs: expr_index_var,
                rhs: expr_one_usize,
                lhs_ty: hir::Type::Int(IntType::Usize),
            }));

            let expr_for_block = hir::Expr::Block { block: block_res.block };
            let stmt_for_block = hir::Stmt::ExprSemi(ctx.arena.alloc(expr_for_block));

            let loop_block = if block_res.diverges {
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
            let match_ = ctx.arena.alloc(match_);
            let match_ = ctx.arena.alloc(hir::Expr::Match { match_ });
            let stmt_match = hir::Stmt::ExprSemi(match_);
            let block = hir::Block { stmts: ctx.arena.alloc_slice(&[stmt_match]) };
            let block = ctx.arena.alloc(block);
            Some(hir::Stmt::Loop(block))
        }
    }
}

enum LocalResult<'hir> {
    Error,
    Local(&'hir hir::Local<'hir>),
    Discard(Option<&'hir hir::Expr<'hir>>),
}

fn typecheck_local<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    local: &ast::Local<'ast>,
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
) -> &'hir hir::Assign<'hir> {
    let lhs_res = typecheck_expr(ctx, Expectation::None, assign.lhs);
    let addr_res = resolve_expr_addressability(ctx, lhs_res.expr);
    check_assign_addressability(ctx, &addr_res, assign.lhs.range);

    let expect = Expectation::HasType(lhs_res.ty, Some(ctx.src(assign.lhs.range)));
    let rhs_res = typecheck_expr(ctx, expect, assign.rhs);

    let assign_op = match assign.op {
        ast::AssignOp::Assign => hir::AssignOp::Assign,
        ast::AssignOp::Bin(op) => {
            let op_start = assign.op_range.start();
            let op_res =
                check_binary_op(ctx, Expectation::None, op, op_start, lhs_res.ty, rhs_res.ty);
            op_res.map(hir::AssignOp::Bin).unwrap_or(hir::AssignOp::Assign)
        }
    };

    let assign =
        hir::Assign { op: assign_op, lhs: lhs_res.expr, rhs: rhs_res.expr, lhs_ty: lhs_res.ty };
    ctx.arena.alloc(assign)
}

pub fn int_range_check<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    val: i128,
    int_ty: hir::IntType,
) -> Result<hir::ConstValue<'hir>, ()> {
    let ptr_width = ctx.session.config.target_ptr_width;
    let min = int_ty.min_128(ptr_width);
    let max = int_ty.max_128(ptr_width);

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
        hir::Expr::Builtin { .. } => Some("builtin value"),
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
        hir::Expr::Variadics { .. } => None,
        hir::Expr::StructInit { .. } | hir::Expr::StructInitPoly { .. } => Some("struct value"),
        hir::Expr::ArrayInit { .. } => Some("array value"),
        hir::Expr::ArrayRepeat { .. } => Some("array value"),
        hir::Expr::Deref { .. } => Some("dereference"),
        hir::Expr::Address { .. } => Some("address value"),
        hir::Expr::Unary { .. } => Some("unary operation"),
        hir::Expr::Binary { .. } => Some("binary operation"),
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

fn infer_enum_poly_types<'hir>(
    enum_id: hir::EnumID,
    expect: Expectation<'hir>,
) -> Option<&'hir [hir::Type<'hir>]> {
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

fn infer_struct_poly_types<'hir>(
    struct_id: hir::StructID,
    expect: Expectation<'hir>,
) -> Option<&'hir [hir::Type<'hir>]> {
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
    start: TextOffset,
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
                let offset = ctx.cache.variadics.start();
                while let Some(expr) = args.next() {
                    let expr_res = typecheck_expr(ctx, Expectation::None, expr);
                    let arg = hir::Variadic { ty: expr_res.ty, expr: expr_res.expr };
                    ctx.cache.variadics.push(arg);
                }
                let args = ctx.cache.variadics.take(offset, &mut ctx.arena);
                let variadics = ctx.arena.alloc(hir::Expr::Variadics { args });
                ctx.cache.exprs.push(variadics);
                break;
            }
            hir::ParamKind::CallerLocation => {
                let expr = if let Some(struct_id) = ctx.core.source_loc {
                    let values = hir::source_location(ctx.session, ctx.scope.origin, start);
                    let values = ctx.arena.alloc_slice(&values);
                    let struct_ = hir::ConstStruct { values, poly_types: &[] };
                    let struct_ = ctx.arena.alloc(struct_);
                    let value = hir::ConstValue::Struct { struct_id, struct_ };
                    ctx.arena.alloc(hir::Expr::Const(value, hir::ConstID::dummy()))
                } else {
                    &hir::Expr::Error
                };
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
        let src = ctx.src(TextRange::new(start, arg_list.range.start()));
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
        ctx.cache.exprs.pop_view(offset);
        ctx.scope.infer.end_context(infer);
        return TypeResult::error();
    }
    let return_ty = type_substitute_inferred(ctx, is_poly, infer, return_ty);

    let input = ctx.cache.exprs.take(offset, &mut ctx.arena);
    let expr = if is_poly {
        let poly_types = match poly_types {
            Some(poly_types) => poly_types,
            None => ctx.arena.alloc_slice(ctx.scope.infer.inferred(infer)),
        };
        hir::Expr::CallDirectPoly { proc_id, input: ctx.arena.alloc((input, poly_types)) }
    } else {
        hir::Expr::CallDirect { proc_id, input }
    };

    ctx.scope.infer.end_context(infer);
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
            err::tycheck_cannot_call_value_of_type(&mut ctx.emit, src, ty_fmt.as_str());
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
                let offset = ctx.cache.variadics.start();
                while let Some(expr) = args.next() {
                    let expr_res = typecheck_expr(ctx, Expectation::None, expr);
                    let arg = hir::Variadic { ty: expr_res.ty, expr: expr_res.expr };
                    ctx.cache.variadics.push(arg);
                }
                let args = ctx.cache.variadics.take(offset, &mut ctx.arena);
                let variadics = ctx.arena.alloc(hir::Expr::Variadics { args });
                ctx.cache.exprs.push(variadics);
                break;
            }
            hir::ParamKind::CallerLocation => {
                let expr = if let Some(struct_id) = ctx.core.source_loc {
                    let values =
                        hir::source_location(ctx.session, ctx.scope.origin, target_range.start());
                    let values = ctx.arena.alloc_slice(&values);
                    let struct_ = hir::ConstStruct { values, poly_types: &[] };
                    let struct_ = ctx.arena.alloc(struct_);
                    let value = hir::ConstValue::Struct { struct_id, struct_ };
                    ctx.arena.alloc(hir::Expr::Const(value, hir::ConstID::dummy()))
                } else {
                    &hir::Expr::Error
                };
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

fn check_variant_input_opt<'hir, 'ast>(
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
        let src = ctx.src(range); //@improve range to include everything up to arg list same as struct_init and proc call
        err::tycheck_cannot_infer_poly_params(&mut ctx.emit, src);
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

    let expr = if ctx.in_const {
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
            ast::Binding::Discard(_) => continue,
        };

        //@total mess, cleanup
        let ty = if let Some(variant) = variant {
            if let Some(field_id) = variant.field_id(idx) {
                let field = variant.field(field_id);
                if let Expectation::HasType(hir::Type::Enum(_, poly_types), _) = expect {
                    let field_ty =
                        type_substitute(ctx, !poly_types.is_empty(), poly_types, field.ty);
                    match ref_mut {
                        Some(ref_mut) => {
                            if field_ty.is_error() {
                                hir::Type::Error
                            } else {
                                hir::Type::Reference(ref_mut, ctx.arena.alloc(field_ty))
                            }
                        }
                        None => field_ty,
                    }
                } else {
                    hir::Type::Error
                }
            } else {
                hir::Type::Error
            }
        } else {
            hir::Type::Error
        };

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
    SliceField,
    Temporary,
    TemporaryImmut,
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
        if matches!(self, AddrConstraint::None) {
            *self = new;
        }
    }
}

fn resolve_expr_addressability(ctx: &HirCtx, expr: &hir::Expr) -> AddrResult {
    let mut expr = expr;
    let mut constraint = AddrConstraint::None;

    loop {
        let base = match *expr {
            // addr_base: simple
            hir::Expr::Error => AddrBase::Unknown,
            hir::Expr::Const(value, const_id) => {
                if const_id != hir::ConstID::dummy() {
                    let data = ctx.registry.const_data(const_id);
                    AddrBase::Constant(data.src())
                } else {
                    match value {
                        hir::ConstValue::Variant { .. }
                        | hir::ConstValue::Struct { .. }
                        | hir::ConstValue::Array { .. }
                        | hir::ConstValue::ArrayRepeat { .. }
                        | hir::ConstValue::ArrayEmpty { .. } => AddrBase::TemporaryImmut,
                        _ => AddrBase::Temporary,
                    }
                }
            }
            hir::Expr::If { .. } => AddrBase::Temporary,
            hir::Expr::Block { .. } => AddrBase::Temporary,
            hir::Expr::Match { .. } => AddrBase::Temporary,
            hir::Expr::SliceField { .. } => AddrBase::SliceField,
            hir::Expr::Cast { .. } => AddrBase::Temporary,
            hir::Expr::Builtin { .. } => AddrBase::Temporary,
            hir::Expr::Variant { .. } => AddrBase::TemporaryImmut,
            hir::Expr::CallDirect { .. } => AddrBase::Temporary,
            hir::Expr::CallDirectPoly { .. } => AddrBase::Temporary,
            hir::Expr::CallIndirect { .. } => AddrBase::Temporary,
            hir::Expr::Variadics { .. } => AddrBase::Unknown,
            hir::Expr::StructInit { .. } => AddrBase::TemporaryImmut,
            hir::Expr::StructInitPoly { .. } => AddrBase::TemporaryImmut,
            hir::Expr::ArrayInit { .. } => AddrBase::TemporaryImmut,
            hir::Expr::ArrayRepeat { .. } => AddrBase::TemporaryImmut,
            hir::Expr::Address { .. } => AddrBase::Temporary,
            hir::Expr::Unary { .. } => AddrBase::Temporary,
            hir::Expr::Binary { .. } => AddrBase::Temporary,
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
        };

        return AddrResult { base, constraint };
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
        ast::Mut::Mutable => "get &mut",
        ast::Mut::Immutable => "get &",
    };

    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => err::tycheck_addr(&mut ctx.emit, src, action, "slice field"),
        AddrBase::Temporary => err::tycheck_addr(&mut ctx.emit, src, action, "temporary value"),
        AddrBase::TemporaryImmut => {}
        AddrBase::Constant(const_src) => {
            err::tycheck_addr_const(&mut ctx.emit, src, const_src, action);
        }
        AddrBase::Variable(var_mutt, var_src) => {
            if mutt == ast::Mut::Immutable {
                return;
            }
            match addr_res.constraint {
                AddrConstraint::None => {
                    if var_mutt == ast::Mut::Immutable {
                        err::tycheck_addr_variable(&mut ctx.emit, src, var_src, action);
                    }
                }
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
    }
}

fn check_assign_addressability(ctx: &mut HirCtx, addr_res: &AddrResult, expr_range: TextRange) {
    let src = ctx.src(expr_range);
    let action = "assign";

    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => err::tycheck_addr(&mut ctx.emit, src, action, "slice field"),
        AddrBase::Temporary | AddrBase::TemporaryImmut => {
            err::tycheck_addr(&mut ctx.emit, src, action, "temporary value")
        }
        AddrBase::Constant(const_src) => {
            err::tycheck_addr_const(&mut ctx.emit, src, const_src, action);
        }
        AddrBase::Variable(var_mutt, var_src) => match addr_res.constraint {
            AddrConstraint::None => {
                if var_mutt == ast::Mut::Immutable {
                    err::tycheck_addr_variable(&mut ctx.emit, src, var_src, action);
                }
            }
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
        },
    }
}

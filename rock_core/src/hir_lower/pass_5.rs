use super::constant;
use super::context::{HirCtx, HirEmit, SymbolKind};
use super::proc_scope::Diverges;
use super::proc_scope::{BlockEnter, DeferStatus, LoopStatus, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, ErrorSink, Info, SourceRange, StringOrStr, WarningComp};
use crate::errors as err;
use crate::hir::{self, BasicFloat, BasicInt};
use crate::session::ModuleID;
use crate::support::ID;
use crate::text::TextRange;

pub fn typecheck_procedures(ctx: &mut HirCtx) {
    for proc_id in ctx.registry.proc_ids() {
        typecheck_proc(ctx, proc_id)
    }
}

fn typecheck_proc<'hir>(ctx: &mut HirCtx<'hir, '_>, proc_id: hir::ProcID<'hir>) {
    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data(proc_id);

    if data.attr_set.contains(hir::ProcFlag::Variadic) {
        if data.params.is_empty() {
            ctx.emit.error(ErrorComp::new(
                "variadic procedures must have at least one named parameter",
                SourceRange::new(data.origin_id, data.name.range),
                None,
            ));
        }
    }

    if data.attr_set.contains(hir::ProcFlag::Test) {
        if !data.params.is_empty() {
            ctx.emit.error(ErrorComp::new(
                "procedures with #[test] attribute cannot have any input parameters",
                SourceRange::new(data.origin_id, data.name.range),
                None,
            ));
        }
        //@allow never
        if !data.return_ty.is_void() {
            ctx.emit.error(ErrorComp::new(
                "procedures with #[test] attribute can only return `void`",
                SourceRange::new(data.origin_id, item.return_ty.range),
                None,
            ));
        }
    }

    if let Some(block) = item.block {
        let expect_src = SourceRange::new(data.origin_id, item.return_ty.range);
        let expect = Expectation::HasType(data.return_ty, Some(expect_src));
        ctx.proc.reset(data.origin_id, data.params, expect);

        let block_res = typecheck_block(ctx, expect, block, BlockEnter::None);
        let locals = ctx.arena.alloc_slice(ctx.proc.finish_locals());

        let data = ctx.registry.proc_data_mut(proc_id);
        data.block = Some(block_res.block);
        data.locals = locals;
    }
}

pub fn type_matches(ctx: &HirCtx, ty: hir::Type, ty2: hir::Type) -> bool {
    match (ty, ty2) {
        (hir::Type::Error, _) => true,
        (_, hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (hir::Type::Enum(id), hir::Type::Enum(id2)) => id == id2,
        (hir::Type::Struct(id), hir::Type::Struct(id2)) => id == id2,
        (hir::Type::Reference(ref_ty, mutt), hir::Type::Reference(ref_ty2, mutt2)) => {
            if mutt2 == ast::Mut::Mutable {
                type_matches(ctx, *ref_ty, *ref_ty2)
            } else {
                mutt == mutt2 && type_matches(ctx, *ref_ty, *ref_ty2)
            }
        }
        (hir::Type::Procedure(proc_ty), hir::Type::Procedure(proc_ty2)) => {
            (proc_ty.param_types.len() == proc_ty2.param_types.len())
                && (proc_ty.is_variadic == proc_ty2.is_variadic)
                && type_matches(ctx, proc_ty.return_ty, proc_ty2.return_ty)
                && (0..proc_ty.param_types.len()).all(|idx| {
                    type_matches(ctx, proc_ty.param_types[idx], proc_ty2.param_types[idx])
                })
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            if slice2.mutt == ast::Mut::Mutable {
                type_matches(ctx, slice.elem_ty, slice2.elem_ty)
            } else {
                slice.mutt == slice2.mutt && type_matches(ctx, slice.elem_ty, slice2.elem_ty)
            }
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            if let Ok(len) = array.len.get_resolved(ctx) {
                if let Ok(len2) = array2.len.get_resolved(ctx) {
                    return (len == len2) && type_matches(ctx, array.elem_ty, array2.elem_ty);
                }
            }
            true
        }
        (hir::Type::ArrayStatic(array), _) => array.len.get_resolved(ctx).is_err(),
        (_, hir::Type::ArrayStatic(array2)) => array2.len.get_resolved(ctx).is_err(),
        _ => false,
    }
}

pub fn type_format(ctx: &HirCtx, ty: hir::Type) -> StringOrStr {
    match ty {
        hir::Type::Error => "<unknown>".into(),
        hir::Type::Basic(basic) => basic.as_str().into(),
        hir::Type::Enum(id) => {
            let name = ctx.name_str(ctx.registry.enum_data(id).name.id);
            name.to_string().into()
        }
        hir::Type::Struct(id) => {
            let name = ctx.name_str(ctx.registry.struct_data(id).name.id);
            name.to_string().into()
        }
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            let ref_ty_format = type_format(ctx, *ref_ty);
            let format = format!("&{}{}", mut_str, ref_ty_format.as_str());
            format.into()
        }
        hir::Type::Procedure(proc_ty) => {
            let mut format = String::from("proc(");
            for (idx, param_ty) in proc_ty.param_types.iter().enumerate() {
                let param_ty_format = type_format(ctx, *param_ty);
                format.push_str(param_ty_format.as_str());
                if proc_ty.param_types.len() != idx + 1 {
                    format.push_str(", ");
                }
            }
            if proc_ty.is_variadic {
                format.push_str(", ..")
            }
            format.push_str(") -> ");
            let return_format = type_format(ctx, proc_ty.return_ty);
            format.push_str(return_format.as_str());
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
            let len = array.len.get_resolved(ctx);
            let elem_format = type_format(ctx, array.elem_ty);
            let format = match len {
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

//@verify how this works, `false` is good outcome?
pub fn check_type_expectation(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    from_range: TextRange,
    expect: Expectation,
    found_ty: hir::Type,
) -> bool {
    if let Expectation::HasType(expect_ty, expect_src) = expect {
        if type_matches(ctx, expect_ty, found_ty) {
            return false;
        }

        let info = if let Some(source) = expect_src {
            Info::new("expected due to this", source)
        } else {
            None
        };

        ctx.emit.error(ErrorComp::new(
            format!(
                "type mismatch: expected `{}`, found `{}`",
                type_format(ctx, expect_ty).as_str(),
                type_format(ctx, found_ty).as_str()
            ),
            SourceRange::new(origin_id, from_range),
            info,
        ));
        true
    } else {
        false
    }
}

pub struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    kind: hir::ExprKind<'hir>,
}

pub struct ExprResult<'hir> {
    ty: hir::Type<'hir>,
    pub expr: &'hir hir::Expr<'hir>,
}

struct BlockResult<'hir> {
    ty: hir::Type<'hir>,
    block: hir::Block<'hir>,
    tail_range: Option<TextRange>,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, kind: hir::ExprKind<'hir>) -> TypeResult<'hir> {
        TypeResult { ty, kind }
    }
    fn error() -> TypeResult<'hir> {
        TypeResult {
            ty: hir::Type::Error,
            kind: hir::ExprKind::Error,
        }
    }
    fn into_expr_result(self, ctx: &mut HirCtx<'hir, '_>, range: TextRange) -> ExprResult<'hir> {
        let expr = hir::Expr {
            kind: self.kind,
            range,
        };
        let expr = ctx.arena.alloc(expr);
        ExprResult::new(self.ty, expr)
    }
}

impl<'hir> ExprResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> ExprResult<'hir> {
        ExprResult { ty, expr }
    }
}

impl<'hir> BlockResult<'hir> {
    fn new(
        ty: hir::Type<'hir>,
        block: hir::Block<'hir>,
        tail_range: Option<TextRange>,
    ) -> BlockResult<'hir> {
        BlockResult {
            ty,
            block,
            tail_range,
        }
    }

    //@prev ignore behavior was always `ignore: true`
    //@ignoring with manual expectation check in blocks, might change 03.07.24
    fn into_type_result(self) -> TypeResult<'hir> {
        let kind = hir::ExprKind::Block { block: self.block };
        TypeResult::new(self.ty, kind)
    }
}

//@Error exprs are still allocated, before they were replaced
// with constant reference defined in hir_build.
// `if` check performance versus memory usage tradeoff.
// It might be still good to include errored expressions with correct range.
#[must_use]
pub fn typecheck_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr,
) -> ExprResult<'hir> {
    let expr_res = match expr.kind {
        ast::ExprKind::Lit { lit } => typecheck_lit(ctx, expect, lit),
        ast::ExprKind::If { if_ } => typecheck_if(ctx, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            typecheck_block(ctx, expect, *block, BlockEnter::None).into_type_result()
        }
        ast::ExprKind::Match { match_ } => typecheck_match(ctx, expect, match_, expr.range),
        ast::ExprKind::Field { target, name } => typecheck_field(ctx, target, name),
        ast::ExprKind::Index {
            target,
            mutt,
            index,
        } => typecheck_index(ctx, target, mutt, index, expr.range),
        ast::ExprKind::Call { target, args_list } => typecheck_call(ctx, target, args_list),
        ast::ExprKind::Cast { target, into } => typecheck_cast(ctx, target, into, expr.range),
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(ctx, *ty, expr.range),
        ast::ExprKind::Item { path, args_list } => typecheck_item(ctx, path, args_list, expr.range),
        ast::ExprKind::Variant { name, args_list } => {
            typecheck_variant(ctx, expect, name, args_list, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(ctx, expect, struct_init, expr.range)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(ctx, expect, input, expr.range),
        ast::ExprKind::ArrayRepeat { expr, len } => typecheck_array_repeat(ctx, expect, expr, len),
        ast::ExprKind::Deref { rhs } => typecheck_deref(ctx, rhs),
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(ctx, mutt, rhs),
        ast::ExprKind::Range { range } => todo!("typecheck range"),
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(ctx, expect, op, op_range, rhs)
        }
        ast::ExprKind::Binary { op, op_range, bin } => {
            typecheck_binary(ctx, expect, op, op_range, bin)
        }
    };

    //@if `errored` is usefull it can be done via emit error count api
    //@removed `ignored` from `TypeResult`, verify ignore behaviors for block returns
    check_type_expectation(ctx, ctx.proc.origin(), expr.range, expect, expr_res.ty);
    expr_res.into_expr_result(ctx, expr.range)
}

fn typecheck_lit<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation,
    lit: ast::Lit,
) -> TypeResult<'hir> {
    let (value, ty) = match lit {
        ast::Lit::Null => {
            let value = hir::ConstValue::Null;
            (value, hir::Type::Basic(BasicType::Rawptr))
        }
        ast::Lit::Bool(val) => {
            let value = hir::ConstValue::Bool { val };
            (value, hir::Type::Basic(BasicType::Bool))
        }
        ast::Lit::Int(val) => {
            let int_ty = coerce_int_type(expect);
            let value = hir::ConstValue::Int {
                val,
                neg: false,
                int_ty,
            };
            (value, hir::Type::Basic(int_ty.into_basic()))
        }
        ast::Lit::Float(val) => {
            let float_ty = coerce_float_type(expect);
            let value = hir::ConstValue::Float { val, float_ty };
            (value, hir::Type::Basic(float_ty.into_basic()))
        }
        ast::Lit::Char(val) => {
            let value = hir::ConstValue::Char { val };
            (value, hir::Type::Basic(BasicType::Char))
        }
        ast::Lit::String(string_lit) => {
            let value = hir::ConstValue::String { string_lit };
            let string_ty = alloc_string_lit_type(ctx, string_lit);
            (value, string_ty)
        }
    };

    let kind = hir::ExprKind::Const { value };
    TypeResult::new(ty, kind)
}

pub fn coerce_int_type(expect: Expectation) -> BasicInt {
    const DEFAULT_INT_TYPE: BasicInt = BasicInt::S32;

    match expect {
        Expectation::None => DEFAULT_INT_TYPE,
        Expectation::HasType(expect_ty, _) => match expect_ty {
            hir::Type::Basic(basic) => BasicInt::from_basic(basic).unwrap_or(DEFAULT_INT_TYPE),
            _ => DEFAULT_INT_TYPE,
        },
    }
}

pub fn coerce_float_type(expect: Expectation) -> BasicFloat {
    const DEFAULT_FLOAT_TYPE: BasicFloat = BasicFloat::F64;

    match expect {
        Expectation::None => DEFAULT_FLOAT_TYPE,
        Expectation::HasType(expect_ty, _) => match expect_ty {
            hir::Type::Basic(basic) => BasicFloat::from_basic(basic).unwrap_or(DEFAULT_FLOAT_TYPE),
            _ => DEFAULT_FLOAT_TYPE,
        },
    }
}

pub fn alloc_string_lit_type<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    string_lit: ast::StringLit,
) -> hir::Type<'hir> {
    if string_lit.c_string {
        let byte = ctx.arena.alloc(hir::Type::Basic(BasicType::U8));
        hir::Type::Reference(byte, ast::Mut::Immutable)
    } else {
        let slice = ctx.arena.alloc(hir::ArraySlice {
            mutt: ast::Mut::Immutable,
            elem_ty: hir::Type::Basic(BasicType::U8),
        });
        hir::Type::ArraySlice(slice)
    }
}

fn typecheck_if<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    mut expect: Expectation<'hir>,
    if_: &ast::If,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let mut if_type = hir::Type::Basic(BasicType::Never);
    let entry = typecheck_branch(ctx, &mut expect, &mut if_type, &if_.entry);

    let mut branches = Vec::with_capacity(if_.branches.len());
    for branch in if_.branches {
        let branch = typecheck_branch(ctx, &mut expect, &mut if_type, branch);
        branches.push(branch);
    }
    let branches = ctx.arena.alloc_slice(&branches);

    let else_block = match if_.else_block {
        Some(block) => {
            let block_res = typecheck_block(ctx, expect, block, BlockEnter::None);

            // never -> anything
            // error -> anything except never
            if if_type.is_never() || (if_type.is_error() && !block_res.ty.is_never()) {
                if_type = block_res.ty;
            }

            Some(block_res.block)
        }
        None => None,
    };

    if else_block.is_none() && if_type.is_never() {
        if_type = hir::Type::VOID;
    }

    if else_block.is_none() && !if_type.is_error() && !if_type.is_void() && !if_type.is_never() {
        ctx.emit.error(ErrorComp::new(
            "`if` expression is missing an `else` block\n`if` without `else` evaluates to `void` and cannot return a value",
            SourceRange::new(ctx.proc.origin(), expr_range),
            None,
        ));
    }

    let if_ = hir::If {
        entry,
        branches,
        else_block,
    };
    let if_ = ctx.arena.alloc(if_);
    let kind = hir::ExprKind::If { if_ };
    //@cannot tell when to ignore typecheck or not
    TypeResult::new(if_type, kind)
}

fn typecheck_branch<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: &mut Expectation<'hir>,
    if_type: &mut hir::Type<'hir>,
    branch: &ast::Branch,
) -> hir::Branch<'hir> {
    let expect_bool = Expectation::HasType(hir::Type::BOOL, None);
    let cond_res = typecheck_expr(ctx, expect_bool, branch.cond);
    let block_res = typecheck_block(ctx, *expect, branch.block, BlockEnter::None);

    // never -> anything
    // error -> anything except never
    if if_type.is_never() || (if_type.is_error() && !block_res.ty.is_never()) {
        *if_type = block_res.ty;
    }

    if let Expectation::None = expect {
        if !block_res.ty.is_error() && !block_res.ty.is_never() {
            let expect_src = block_res
                .tail_range
                .map(|range| SourceRange::new(ctx.proc.origin(), range));
            *expect = Expectation::HasType(block_res.ty, expect_src);
        }
    }

    hir::Branch {
        cond: cond_res.expr,
        block: block_res.block,
    }
}

//@different enum variants 01.06.24
// could have same value and result in
// error in llvm ir generation, not checked currently
fn typecheck_match<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    mut expect: Expectation<'hir>,
    match_: &ast::Match,
    match_range: TextRange,
) -> TypeResult<'hir> {
    let mut match_type = hir::Type::Basic(BasicType::Never);
    let error_count = ctx.emit.error_count();

    let on_res = typecheck_expr(ctx, Expectation::None, match_.on_expr);
    let pat_expect_src = SourceRange::new(ctx.proc.origin(), match_.on_expr.range);
    let pat_expect = Expectation::HasType(on_res.ty, Some(pat_expect_src));
    check_match_compatibility(ctx, ctx.proc.origin(), on_res.ty, match_.on_expr.range);

    let mut arms = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms {
        let pat = typecheck_pat(ctx, pat_expect, &arm.pat);
        let expr_res = typecheck_expr(ctx, expect, arm.expr);

        // never -> anything
        // error -> anything except never
        if match_type.is_never() || (match_type.is_error() && !expr_res.ty.is_never()) {
            match_type = expr_res.ty;
        }
        if let Expectation::None = expect {
            if !expr_res.ty.is_error() && !expr_res.ty.is_never() {
                let expect_src = SourceRange::new(ctx.proc.origin(), arm.expr.range);
                expect = Expectation::HasType(expr_res.ty, Some(expect_src));
            }
        }

        let tail_stmt = hir::Stmt::ExprTail(expr_res.expr);
        let stmts = ctx.arena.alloc_slice(&[tail_stmt]);
        let block = hir::Block { stmts };

        let arm = hir::MatchArm {
            pat,
            block,
            unreachable: false,
        };
        arms.push(arm);
    }

    // no errors for entire match
    // all constant values resolved
    if !ctx.emit.did_error(error_count) && match_const_pats_resolved(ctx, &arms) {
        let mut match_range = TextRange::empty_at(match_range.start());
        match_range.extend_by(5.into());
        super::match_check::match_cov(
            ctx,
            on_res.ty,
            arms.as_mut_slice(),
            match_.arms,
            match_range,
        );
    }

    let arms = ctx.arena.alloc_slice(&arms);
    let match_ = hir::Match {
        on_expr: on_res.expr,
        arms,
    };
    let match_ = ctx.arena.alloc(match_);
    let kind = hir::ExprKind::Match { match_ };
    //@cannot tell when to ignore typecheck or not
    TypeResult::new(match_type, kind)
}

fn match_const_pats_resolved(ctx: &HirCtx, arms: &[hir::MatchArm]) -> bool {
    fn const_value_resolved_ok(ctx: &HirCtx, const_id: ID<hir::ConstData>) -> bool {
        let data = ctx.registry.const_data(const_id);
        let (eval, _) = ctx.registry.const_eval(data.value);
        eval.is_resolved_ok()
    }

    for arm in arms.iter() {
        match arm.pat {
            hir::Pat::Const(const_id) => {
                if !const_value_resolved_ok(ctx, const_id) {
                    return false;
                }
            }
            hir::Pat::Or(patterns) => {
                for pat in patterns {
                    match pat {
                        hir::Pat::Const(const_id) => {
                            if !const_value_resolved_ok(ctx, *const_id) {
                                return false;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    true
}

struct PatResult<'hir> {
    pat: hir::Pat<'hir>,
    pat_ty: hir::Type<'hir>,
}

impl<'hir> PatResult<'hir> {
    fn new(pat: hir::Pat<'hir>, pat_ty: hir::Type<'hir>) -> PatResult<'hir> {
        PatResult { pat, pat_ty }
    }
}

fn typecheck_pat<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    pat: &ast::Pat,
) -> hir::Pat<'hir> {
    let pat_res = match pat.kind {
        ast::PatKind::Wild => PatResult::new(hir::Pat::Wild, hir::Type::Error),
        ast::PatKind::Lit { lit } => typecheck_pat_lit(ctx, expect, lit),
        ast::PatKind::Item { path, bind_list } => {
            typecheck_pat_item(ctx, path, bind_list, pat.range)
        }
        ast::PatKind::Variant { name, bind_list } => {
            typecheck_pat_variant(ctx, expect, name, bind_list, pat.range)
        }
        ast::PatKind::Or { patterns } => typecheck_pat_or(ctx, expect, patterns),
    };

    check_type_expectation(ctx, ctx.proc.origin(), pat.range, expect, pat_res.pat_ty);
    pat_res.pat
}

fn typecheck_pat_lit<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    lit: ast::Lit,
) -> PatResult<'hir> {
    let lit_res = typecheck_lit(ctx, expect, lit);
    let value = match lit_res.kind {
        hir::ExprKind::Const { value } => value,
        _ => unreachable!(),
    };
    PatResult::new(hir::Pat::Lit(value), lit_res.ty)
}

fn typecheck_pat_item<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    path: &ast::Path,
    binds: Option<&ast::BindingList>,
    pat_range: TextRange,
) -> PatResult<'hir> {
    let (value_id, field_names) = path_resolve_value(ctx, ctx.proc.origin(), path);

    match value_id {
        ValueID::None => PatResult::new(hir::Pat::Error, hir::Type::Error),
        ValueID::Enum(enum_id, variant_id) => {
            //@`field_names` are guaranteed to be empty (solve by making path resolve more clear)
            let data = ctx.registry.enum_data(enum_id);
            let variant = data.variant(variant_id);

            let input_count = binds.map(|list| list.binds.len()).unwrap_or(0);
            let expected_count = variant.fields.len();

            if input_count != expected_count {
                //@error src can be improved to not include variant itself if possible (if needed)
                error_unexpected_variant_bind_count(
                    &mut ctx.emit,
                    expected_count,
                    input_count,
                    SourceRange::new(ctx.proc.origin(), pat_range),
                    SourceRange::new(data.origin_id, variant.name.range),
                );
            }

            PatResult::new(
                hir::Pat::Variant(enum_id, variant_id),
                hir::Type::Enum(enum_id),
            )
        }
        ValueID::Const(const_id) => {
            if let Some(name) = field_names.first() {
                ctx.emit.error(ErrorComp::new(
                    "cannot access fields in patterns",
                    SourceRange::new(ctx.proc.origin(), name.range),
                    None,
                ));
            }

            let input_count = binds.map(|list| list.binds.len()).unwrap_or(0);
            if input_count > 0 {
                //@rephrase error
                ctx.emit.error(ErrorComp::new(
                    "cannot destructure constants in patterns",
                    SourceRange::new(ctx.proc.origin(), pat_range),
                    None,
                ));
            }

            let data = ctx.registry.const_data(const_id);
            PatResult::new(hir::Pat::Const(const_id), data.ty)
        }
        ValueID::Proc(_) | ValueID::Global(_) | ValueID::Local(_) | ValueID::Param(_) => {
            ctx.emit.error(ErrorComp::new(
                "cannot use runtime values in patterns",
                SourceRange::new(ctx.proc.origin(), pat_range),
                None,
            ));
            PatResult::new(hir::Pat::Error, hir::Type::Error)
        }
    }
}

fn typecheck_pat_variant<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    binds: Option<&ast::BindingList>,
    pat_range: TextRange,
) -> PatResult<'hir> {
    let enum_id = infer_enum_type(
        &mut ctx.emit,
        expect,
        SourceRange::new(ctx.proc.origin(), name.range),
    );
    let enum_id = match enum_id {
        Some(enum_id) => enum_id,
        None => return PatResult::new(hir::Pat::Wild, hir::Type::Error),
    };

    let data = ctx.registry.enum_data(enum_id);
    if let Some((variant_id, variant)) = data.find_variant(name.id) {
        //@duplicate codeblock, same as ast::PatKind::Item
        let input_count = binds.map(|list| list.binds.len()).unwrap_or(0);
        let expected_count = variant.fields.len();

        if input_count != expected_count {
            //@error src can be improved to not include variant itself if possible (if needed)
            error_unexpected_variant_bind_count(
                &mut ctx.emit,
                expected_count,
                input_count,
                SourceRange::new(ctx.proc.origin(), pat_range),
                SourceRange::new(data.origin_id, variant.name.range),
            );
        }

        PatResult::new(
            hir::Pat::Variant(enum_id, variant_id),
            hir::Type::Enum(enum_id),
        )
    } else {
        //@duplicate error, same as path resolve
        ctx.emit.error(ErrorComp::new(
            format!("enum variant `{}` is not found", ctx.name_str(name.id)),
            SourceRange::new(ctx.proc.origin(), name.range),
            Info::new(
                "enum defined here",
                SourceRange::new(data.origin_id, data.name.range),
            ),
        ));
        PatResult::new(hir::Pat::Wild, hir::Type::Error)
    }
}

fn typecheck_pat_or<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    patterns: &[ast::Pat],
) -> PatResult<'hir> {
    let mut patterns_res = Vec::with_capacity(patterns.len());
    for pat in patterns {
        let pat = typecheck_pat(ctx, expect, pat);
        patterns_res.push(pat);
    }
    let patterns = ctx.arena.alloc_slice(&patterns_res);

    PatResult::new(hir::Pat::Or(patterns), hir::Type::Error)
}

fn typecheck_field<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    target: &ast::Expr,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let field_result = check_field_from_type(ctx, ctx.proc.origin(), name, target_res.ty);
    emit_field_expr(target_res.expr, field_result)
}

struct FieldResult<'hir> {
    deref: bool,
    kind: FieldKind<'hir>,
    field_ty: hir::Type<'hir>,
}

#[rustfmt::skip]
enum FieldKind<'hir> {
    Struct(hir::StructID<'hir>, hir::FieldID<'hir>),
    ArraySlice { field: hir::SliceField },
    ArrayStatic { len: hir::ConstValue<'hir> },
}

impl<'hir> FieldResult<'hir> {
    fn new(deref: bool, kind: FieldKind<'hir>, field_ty: hir::Type<'hir>) -> FieldResult<'hir> {
        FieldResult {
            deref,
            kind,
            field_ty,
        }
    }
}

fn check_field_from_type<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    name: ast::Name,
    ty: hir::Type<'hir>,
) -> Option<FieldResult<'hir>> {
    let (ty, deref) = match ty {
        hir::Type::Reference(ref_ty, _) => (*ref_ty, true),
        _ => (ty, false),
    };

    match ty {
        hir::Type::Error => None,
        hir::Type::Struct(struct_id) => {
            match check_field_from_struct(ctx, origin_id, name, struct_id) {
                Some((field_id, field)) => {
                    let kind = FieldKind::Struct(struct_id, field_id);
                    let field_ty = field.ty;
                    Some(FieldResult::new(deref, kind, field_ty))
                }
                None => None,
            }
        }
        hir::Type::ArraySlice(slice) => match check_field_from_slice(ctx, origin_id, name, slice) {
            Some(field) => {
                let kind = FieldKind::ArraySlice { field };
                let field_ty = match field {
                    hir::SliceField::Ptr => hir::Type::Reference(&slice.elem_ty, slice.mutt),
                    hir::SliceField::Len => hir::Type::USIZE,
                };
                Some(FieldResult::new(deref, kind, field_ty))
            }
            None => None,
        },
        hir::Type::ArrayStatic(array) => {
            match check_field_from_array(ctx, origin_id, name, array) {
                Some(len) => {
                    let kind = FieldKind::ArrayStatic { len };
                    let field_ty = hir::Type::USIZE;
                    Some(FieldResult::new(deref, kind, field_ty))
                }
                None => None,
            }
        }
        _ => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on value of type `{}`",
                    ctx.name_str(name.id),
                    type_format(ctx, ty).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn check_field_from_struct<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    name: ast::Name,
    struct_id: hir::StructID<'hir>,
) -> Option<(hir::FieldID<'hir>, &'hir hir::Field<'hir>)> {
    let data = ctx.registry.struct_data(struct_id);
    match data.find_field(name.id) {
        Some((field_id, field)) => {
            if origin_id != data.origin_id && field.vis == ast::Vis::Private {
                ctx.emit.error(ErrorComp::new(
                    format!("field `{}` is private", ctx.name_str(name.id)),
                    SourceRange::new(origin_id, name.range),
                    Info::new(
                        "defined here",
                        SourceRange::new(data.origin_id, field.name.range),
                    ),
                ));
            }
            Some((field_id, field))
        }
        None => {
            ctx.emit.error(ErrorComp::new(
                format!("field `{}` is not found", ctx.name_str(name.id)),
                SourceRange::new(origin_id, name.range),
                Info::new(
                    "struct defined here",
                    SourceRange::new(data.origin_id, data.name.range),
                ),
            ));
            None
        }
    }
}

fn check_field_from_slice<'hir>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    name: ast::Name,
    slice: &hir::ArraySlice,
) -> Option<hir::SliceField> {
    let field_name = ctx.name_str(name.id);
    match field_name {
        "ptr" => Some(hir::SliceField::Ptr),
        "len" => Some(hir::SliceField::Len),
        _ => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on slice type `{}`\ndid you mean `len` or `ptr`?",
                    field_name,
                    type_format(ctx, hir::Type::ArraySlice(slice)).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn check_field_from_array<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    name: ast::Name,
    array: &hir::ArrayStatic,
) -> Option<hir::ConstValue<'hir>> {
    let field_name = ctx.name_str(name.id);
    match field_name {
        "len" => {
            let len = match array.len {
                hir::ArrayStaticLen::Immediate(len) => match len {
                    Some(value) => hir::ConstValue::Int {
                        val: value,
                        neg: false,
                        int_ty: hir::BasicInt::Usize,
                    },
                    None => return None, //@field result doesnt communicate usize type if this is None
                },
                hir::ArrayStaticLen::ConstEval(eval_id) => {
                    let (eval, _) = ctx.registry.const_eval(eval_id);
                    match eval.get_resolved() {
                        Ok(value_id) => ctx.const_intern.get(value_id),
                        Err(()) => return None, //@field result doesnt communicate usize type if this is None
                    }
                }
            };
            Some(len)
        }
        _ => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on array type `{}`\ndid you mean `len`?",
                    field_name,
                    type_format(ctx, hir::Type::ArrayStatic(array)).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn emit_field_expr<'hir>(
    target: &'hir hir::Expr<'hir>,
    field_result: Option<FieldResult<'hir>>,
) -> TypeResult<'hir> {
    let result = match field_result {
        Some(result) => result,
        None => return TypeResult::error(),
    };

    let kind = match result.kind {
        FieldKind::Struct(struct_id, field_id) => hir::ExprKind::StructField {
            target,
            struct_id,
            field_id,
            deref: result.deref,
        },
        FieldKind::ArraySlice { field } => hir::ExprKind::SliceField {
            target,
            field,
            deref: result.deref,
        },
        FieldKind::ArrayStatic { len } => hir::ExprKind::Const { value: len },
    };

    TypeResult::new(result.field_ty, kind)
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
        fn type_collection(ty: hir::Type, deref: bool) -> Result<Option<CollectionType>, ()> {
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

//@index or slice, desugar correctly
fn typecheck_index<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    target: &ast::Expr,
    mutt: ast::Mut,
    index: &ast::Expr,
    expr_range: TextRange, //@use range of brackets? `[]` 08.05.24
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let expect_usize = Expectation::HasType(hir::Type::USIZE, None);
    let index_res = typecheck_expr(ctx, expect_usize, index);

    match CollectionType::from(target_res.ty) {
        Ok(Some(collection)) => {
            let access = hir::IndexAccess {
                deref: collection.deref,
                elem_ty: collection.elem_ty,
                kind: match collection.kind {
                    SliceOrArray::Slice(slice) => hir::IndexKind::Slice {
                        elem_size: constant::type_layout(
                            ctx,
                            slice.elem_ty,
                            SourceRange::new(ctx.proc.origin(), expr_range), //@review source range for this type_size error 10.05.24
                        )
                        .unwrap_or(hir::Layout::new(0, 1))
                        .size(),
                    },
                    SliceOrArray::Array(array) => hir::IndexKind::Array { array },
                },
                index: index_res.expr,
            };

            let kind = hir::ExprKind::Index {
                target: target_res.expr,
                access: ctx.arena.alloc(access),
            };
            TypeResult::new(collection.elem_ty, kind)
        }
        Ok(None) => TypeResult::error(),
        Err(()) => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "cannot index value of type `{}`",
                    type_format(ctx, target_res.ty).as_str()
                ),
                SourceRange::new(ctx.proc.origin(), expr_range),
                // is this info needed? test if its useful
                Info::new(
                    format!("has `{}` type", type_format(ctx, target_res.ty).as_str()),
                    SourceRange::new(ctx.proc.origin(), target.range),
                ),
            ));
            TypeResult::error()
        }
    }
}

fn typecheck_call<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    target: &ast::Expr,
    args_list: &ast::ArgumentList,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    check_call_indirect(ctx, target_res, args_list)
}

#[derive(Copy, Clone)]
enum BasicTypeKind {
    IntS,
    IntU,
    Float,
    Bool,
    Char,
    Rawptr,
    Void,
    Never,
}

impl BasicTypeKind {
    fn new(basic: BasicType) -> BasicTypeKind {
        match basic {
            BasicType::S8 | BasicType::S16 | BasicType::S32 | BasicType::S64 | BasicType::Ssize => {
                BasicTypeKind::IntS
            }
            BasicType::U8 | BasicType::U16 | BasicType::U32 | BasicType::U64 | BasicType::Usize => {
                BasicTypeKind::IntU
            }
            BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
            BasicType::Bool => BasicTypeKind::Bool,
            BasicType::Char => BasicTypeKind::Char,
            BasicType::Rawptr => BasicTypeKind::Rawptr,
            BasicType::Void => BasicTypeKind::Void,
            BasicType::Never => BasicTypeKind::Never,
        }
    }
}

fn typecheck_cast<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    target: &ast::Expr,
    into: &ast::Type,
    range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let into = super::pass_3::type_resolve(ctx, ctx.proc.origin(), *into);

    // early return prevents false positives on cast warning & cast error
    if matches!(target_res.ty, hir::Type::Error) || matches!(into, hir::Type::Error) {
        //@this could be skipped by returning reference to same error expression
        // to save memory in error cases and reduce noise in the code itself.

        let kind = hir::ExprKind::Cast {
            target: target_res.expr,
            into: ctx.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult::new(into, kind);
    }

    // invariant: both types are not Error
    // ensured by early return above
    if type_matches(ctx, target_res.ty, into) {
        ctx.emit.warning(WarningComp::new(
            format!(
                "redundant cast from `{}` into `{}`",
                type_format(ctx, target_res.ty).as_str(),
                type_format(ctx, into).as_str()
            ),
            SourceRange::new(ctx.proc.origin(), range),
            None,
        ));

        let kind = hir::ExprKind::Cast {
            target: target_res.expr,
            into: ctx.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult::new(into, kind);
    }

    // invariant: from_size != into_size
    // ensured by cast redundancy warning above
    let cast_kind = match (target_res.ty, into) {
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            let from_kind = BasicTypeKind::new(from);
            let into_kind = BasicTypeKind::new(into);
            let from_size = constant::basic_layout(ctx, from).size();
            let into_size = constant::basic_layout(ctx, into).size();

            match from_kind {
                BasicTypeKind::IntS => match into_kind {
                    BasicTypeKind::IntS | BasicTypeKind::IntU => {
                        if from_size < into_size {
                            hir::CastKind::IntS_Sign_Extend
                        } else {
                            hir::CastKind::Int_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::IntS_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::IntU => match into_kind {
                    BasicTypeKind::IntS | BasicTypeKind::IntU => {
                        if from_size < into_size {
                            hir::CastKind::IntU_Zero_Extend
                        } else {
                            hir::CastKind::Int_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::IntU_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::Float => match into_kind {
                    BasicTypeKind::IntS => hir::CastKind::Float_to_IntS,
                    BasicTypeKind::IntU => hir::CastKind::Float_to_IntU,
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
        ctx.emit.error(ErrorComp::new(
            format!(
                "non primitive cast from `{}` into `{}`",
                type_format(ctx, target_res.ty).as_str(),
                type_format(ctx, into).as_str()
            ),
            SourceRange::new(ctx.proc.origin(), range),
            None,
        ));
    }

    let kind = hir::ExprKind::Cast {
        target: target_res.expr,
        into: ctx.arena.alloc(into),
        kind: cast_kind,
    };
    TypeResult::new(into, kind)
}

//@resulting layout sizes are not checked to fit in usize
// this can be partially adressed if each hir::Expr
// is folded, thus range being range checked for `usize`
fn typecheck_sizeof<'hir>(
    ctx: &mut HirCtx,
    ty: ast::Type,
    expr_range: TextRange, //@temp? used for array size overflow error
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(ctx, ctx.proc.origin(), ty);

    //@usize semantics not finalized yet
    // assigning usize type to constant int, since it represents size
    //@review source range for this type_size error 10.05.24
    let kind = match constant::type_layout(ctx, ty, SourceRange::new(ctx.proc.origin(), expr_range))
    {
        Ok(layout) => {
            let value = hir::ConstValue::Int {
                val: layout.size(),
                neg: false,
                int_ty: BasicInt::Usize,
            };
            hir::ExprKind::Const { value }
        }
        Err(()) => hir::ExprKind::Error,
    };

    TypeResult::new(hir::Type::Basic(BasicType::Usize), kind)
}

fn typecheck_item<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    path: &ast::Path,
    input: Option<&ast::ArgumentList>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let (value_id, field_names) = path_resolve_value(ctx, ctx.proc.origin(), path);

    let item_res = match value_id {
        ValueID::None => {
            return error_result_default_check_input_opt(ctx, input);
        }
        ValueID::Proc(proc_id) => {
            //@where do fields go? none expected?
            // ValueID or something like it should provide
            // remaining fields in each variant so its known when none can exist
            // instead of hadling the empty slice and assuming its correct
            if let Some(input) = input {
                return check_call_direct(ctx, proc_id, input);
            } else {
                let data = ctx.registry.proc_data(proc_id);
                //@creating proc type each time its encountered / called, waste of arena memory 25.05.24
                let mut param_types = Vec::with_capacity(data.params.len());
                for param in data.params {
                    param_types.push(param.ty);
                }
                let proc_ty = hir::ProcType {
                    param_types: ctx.arena.alloc_slice(&param_types),
                    is_variadic: data.attr_set.contains(hir::ProcFlag::Variadic),
                    return_ty: data.return_ty,
                };

                let proc_ty = hir::Type::Procedure(ctx.arena.alloc(proc_ty));
                let proc_value = hir::ConstValue::Procedure { proc_id };
                let kind = hir::ExprKind::Const { value: proc_value };
                return TypeResult::new(proc_ty, kind);
            }
        }
        ValueID::Enum(enum_id, variant_id) => {
            //@no fields is guaranteed by path resolve
            // remove assert when stable, or path resolve is reworked
            assert!(field_names.is_empty());
            return check_variant_input_opt(ctx, enum_id, variant_id, input, expr_range);
        }
        ValueID::Const(id) => TypeResult::new(
            ctx.registry.const_data(id).ty,
            hir::ExprKind::ConstVar { const_id: id },
        ),
        ValueID::Global(id) => TypeResult::new(
            ctx.registry.global_data(id).ty,
            hir::ExprKind::GlobalVar { global_id: id },
        ),
        ValueID::Local(id) => TypeResult::new(
            ctx.proc.get_local(id).ty, //@type of local var might not be known
            hir::ExprKind::LocalVar { local_id: id },
        ),
        ValueID::Param(id) => TypeResult::new(
            ctx.proc.get_param(id).ty,
            hir::ExprKind::ParamVar { param_id: id },
        ),
    };

    let mut target_res = item_res;
    let mut target_range = expr_range;

    for &name in field_names {
        let expr_res = target_res.into_expr_result(ctx, target_range);
        let field_result = check_field_from_type(ctx, ctx.proc.origin(), name, expr_res.ty);
        target_res = emit_field_expr(expr_res.expr, field_result);
        target_range = TextRange::new(expr_range.start(), name.range.end());
    }

    if let Some(input) = input {
        let expr_res = target_res.into_expr_result(ctx, target_range);
        check_call_indirect(ctx, expr_res, input)
    } else {
        target_res
    }
}

fn typecheck_variant<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    input: Option<&ast::ArgumentList>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let enum_id = infer_enum_type(
        &mut ctx.emit,
        expect,
        SourceRange::new(ctx.proc.origin(), expr_range),
    );
    let enum_id = match enum_id {
        Some(enum_id) => enum_id,
        None => return error_result_default_check_input_opt(ctx, input),
    };

    let data = ctx.registry.enum_data(enum_id);
    let variant_id = match data.find_variant(name.id) {
        Some((variant_id, _)) => variant_id,
        None => {
            //@duplicate error, same as path resolve 1.07.24
            ctx.emit.error(ErrorComp::new(
                format!("enum variant `{}` is not found", ctx.name_str(name.id)),
                SourceRange::new(ctx.proc.origin(), name.range),
                Info::new(
                    "enum defined here",
                    SourceRange::new(data.origin_id, data.name.range),
                ),
            ));
            return error_result_default_check_input_opt(ctx, input);
        }
    };

    check_variant_input_opt(ctx, enum_id, variant_id, input, expr_range)
}

//@still used in pass4
pub fn error_cannot_infer_struct_type(emit: &mut HirEmit, src: SourceRange) {
    emit.error(ErrorComp::new("cannot infer struct type", src, None))
}

fn typecheck_struct_init<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    struct_init: &ast::StructInit,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let struct_id = match struct_init.path {
        Some(path) => path_resolve_struct(ctx, ctx.proc.origin(), path),
        None => infer_struct_type(
            &mut ctx.emit,
            expect,
            SourceRange::new(ctx.proc.origin(), expr_range),
        ),
    };
    let struct_id = match struct_id {
        Some(struct_id) => struct_id,
        None => return error_result_default_check_field_init(ctx, struct_init.input),
    };

    let data = ctx.registry.struct_data(struct_id);
    let origin_id = data.origin_id;
    let data_name = data.name;
    let field_count = data.fields.len();

    enum FieldStatus {
        None,
        Init(TextRange),
    }

    //@potentially a lot of allocations (simple solution), same memory could be re-used
    let mut field_inits = Vec::<hir::FieldInit>::with_capacity(field_count);
    let mut field_status = Vec::<FieldStatus>::new();
    field_status.resize_with(field_count, || FieldStatus::None);
    let mut init_count: usize = 0;

    for input in struct_init.input {
        //@reborrow bullshit
        let data = ctx.registry.struct_data(struct_id);

        match data.find_field(input.name.id) {
            Some((field_id, field)) => {
                let expect = Expectation::HasType(field.ty, None);
                let input_res = typecheck_expr(ctx, expect, input.expr);

                if let FieldStatus::Init(range) = field_status[field_id.raw_index()] {
                    ctx.emit.error(ErrorComp::new(
                        format!(
                            "field `{}` was already initialized",
                            ctx.name_str(input.name.id),
                        ),
                        SourceRange::new(ctx.proc.origin(), input.name.range),
                        Info::new("initialized here", SourceRange::new(origin_id, range)),
                    ));
                } else {
                    if ctx.proc.origin() != origin_id {
                        if field.vis == ast::Vis::Private {
                            ctx.emit.error(ErrorComp::new(
                                format!("field `{}` is private", ctx.name_str(field.name.id),),
                                SourceRange::new(ctx.proc.origin(), input.name.range),
                                Info::new(
                                    "defined here",
                                    SourceRange::new(origin_id, field.name.range),
                                ),
                            ));
                        }
                    }

                    let field_init = hir::FieldInit {
                        field_id,
                        expr: input_res.expr,
                    };
                    field_inits.push(field_init);
                    field_status[field_id.raw_index()] = FieldStatus::Init(input.name.range);
                    init_count += 1;
                }
            }
            None => {
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "field `{}` is not found in `{}`",
                        ctx.name_str(input.name.id),
                        ctx.name_str(data_name.id)
                    ),
                    SourceRange::new(ctx.proc.origin(), input.name.range),
                    Info::new(
                        "struct defined here",
                        SourceRange::new(origin_id, data_name.range),
                    ),
                ));
                let _ = typecheck_expr(ctx, Expectation::None, input.expr);
            }
        }
    }

    let data = ctx.registry.struct_data(struct_id);

    if init_count < field_count {
        //@change message to list limited number of fields based on their name len()
        let mut message = "missing field initializers: ".to_string();

        for (idx, status) in field_status.iter().enumerate() {
            if let FieldStatus::None = status {
                let field = data.field(hir::FieldID::new_raw(idx));
                message.push('`');
                message.push_str(ctx.name_str(field.name.id));
                if idx + 1 != field_count {
                    message.push_str("`, ");
                } else {
                    message.push('`');
                }
            }
        }

        ctx.emit.error(ErrorComp::new(
            message,
            SourceRange::new(ctx.proc.origin(), expr_range),
            Info::new(
                "struct defined here",
                SourceRange::new(data.origin_id, data.name.range),
            ),
        ));
    }

    let input = ctx.arena.alloc_slice(&field_inits);
    let kind = hir::ExprKind::StructInit { struct_id, input };
    TypeResult::new(hir::Type::Struct(struct_id), kind)
}

fn typecheck_array_init<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    mut expect: Expectation<'hir>,
    input: &[&ast::Expr<'_>],
    array_range: TextRange,
) -> TypeResult<'hir> {
    let mut expect_array_ty = None;

    expect = match expect {
        Expectation::None => Expectation::None,
        Expectation::HasType(expect_ty, expect_src) => match expect_ty {
            hir::Type::ArrayStatic(array) => {
                expect_array_ty = Some(array);
                Expectation::HasType(array.elem_ty, expect_src)
            }
            _ => Expectation::None,
        },
    };

    //@fix elem_ty inference same as `if`, `match`
    let mut elem_ty = hir::Type::Error;

    let input = {
        let mut input_res = Vec::with_capacity(input.len());
        for &expr in input.iter() {
            let expr_res = typecheck_expr(ctx, expect, expr);
            input_res.push(expr_res.expr);

            //@temp old version
            if elem_ty.is_error() {
                elem_ty = expr_res.ty;
            }
            //@restore expect update
            /*
            if expect.ty.is_error() {
                expect = TypeExpectation::new(
                    expr_res.ty,
                    Some(SourceRange::new(ctx.proc.origin(), expr.range)),
                )
            }
            */
        }
        ctx.arena.alloc_slice(&input_res)
    };

    let elem_ty = if input.is_empty() {
        if let Some(array_ty) = expect_array_ty {
            array_ty.elem_ty
        } else {
            ctx.emit.error(ErrorComp::new(
                "cannot infer type of empty array",
                SourceRange::new(ctx.proc.origin(), array_range),
                None,
            ));
            hir::Type::Error
        }
    } else {
        elem_ty
    };

    let array_type: &hir::ArrayStatic = ctx.arena.alloc(hir::ArrayStatic {
        len: hir::ArrayStaticLen::Immediate(Some(input.len() as u64)),
        elem_ty,
    });
    let array_init = ctx.arena.alloc(hir::ArrayInit { elem_ty, input });
    let kind = hir::ExprKind::ArrayInit { array_init };
    TypeResult::new(hir::Type::ArrayStatic(array_type), kind)
}

fn typecheck_array_repeat<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    mut expect: Expectation<'hir>,
    expr: &ast::Expr,
    len: ast::ConstExpr,
) -> TypeResult<'hir> {
    expect = match expect {
        Expectation::None => Expectation::None,
        Expectation::HasType(expect_ty, expect_src) => match expect_ty {
            hir::Type::ArrayStatic(array) => Expectation::HasType(array.elem_ty, expect_src),
            _ => Expectation::None,
        },
    };

    let expr_res = typecheck_expr(ctx, expect, expr);

    //@this is duplicated here and in pass_3::type_resolve 09.05.24
    let value = constant::resolve_const_expr(
        ctx,
        ctx.proc.origin(),
        Expectation::HasType(hir::Type::USIZE, None),
        len,
    );
    let len = match value {
        Ok(hir::ConstValue::Int { val, .. }) => Some(val),
        _ => None,
    };

    if let Some(len) = len {
        let array_type = ctx.arena.alloc(hir::ArrayStatic {
            len: hir::ArrayStaticLen::Immediate(Some(len)), //@move to always specified size? else error 09.06.24
            elem_ty: expr_res.ty,
        });
        let array_repeat = ctx.arena.alloc(hir::ArrayRepeat {
            elem_ty: expr_res.ty,
            expr: expr_res.expr,
            len,
        });
        let kind = hir::ExprKind::ArrayRepeat { array_repeat };
        TypeResult::new(hir::Type::ArrayStatic(array_type), kind)
    } else {
        TypeResult::error()
    }
}

fn typecheck_deref<'hir>(ctx: &mut HirCtx<'hir, '_>, rhs: &ast::Expr) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, Expectation::None, rhs);

    let ptr_ty = match rhs_res.ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::Reference(ref_ty, _) => *ref_ty,
        _ => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "cannot dereference value of type `{}`",
                    type_format(ctx, rhs_res.ty).as_str()
                ),
                SourceRange::new(ctx.proc.origin(), rhs.range),
                None,
            ));
            hir::Type::Error
        }
    };

    let kind = hir::ExprKind::Deref {
        rhs: rhs_res.expr,
        ptr_ty: ctx.arena.alloc(ptr_ty),
    };
    TypeResult::new(ptr_ty, kind)
}

fn typecheck_address<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    mutt: ast::Mut,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(ctx, Expectation::None, rhs);
    let adressability = get_expr_addressability(ctx, rhs_res.expr);

    match adressability {
        Addressability::Unknown => {} //@ & to error should be also Error? 16.05.24
        Addressability::Constant => {
            ctx.emit.error(ErrorComp::new(
                "cannot get reference to a constant, you can use `global` instead",
                SourceRange::new(ctx.proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            ctx.emit.error(ErrorComp::new(
                "cannot get reference to a slice field, slice itself cannot be modified",
                SourceRange::new(ctx.proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::Temporary => {
            ctx.emit.error(ErrorComp::new(
                "cannot get reference to a temporary value",
                SourceRange::new(ctx.proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::TemporaryImmutable => {
            if mutt == ast::Mut::Mutable {
                ctx.emit.error(ErrorComp::new(
                    "cannot get mutable reference to this temporary value, only immutable `&` is allowed",
                    SourceRange::new(ctx.proc.origin(), rhs.range),
                    None,
                ));
            }
        }
        Addressability::Addressable(rhs_mutt, src) => {
            if mutt == ast::Mut::Mutable && rhs_mutt == ast::Mut::Immutable {
                ctx.emit.error(ErrorComp::new(
                    "cannot get mutable reference to an immutable variable",
                    SourceRange::new(ctx.proc.origin(), rhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            ctx.emit.error(ErrorComp::new(
                "addressability not implemented for this expression",
                SourceRange::new(ctx.proc.origin(), rhs.range),
                None,
            ));
        }
    }

    let ref_ty = ctx.arena.alloc(rhs_res.ty);
    let ref_ty = hir::Type::Reference(ref_ty, mutt);
    let kind = hir::ExprKind::Address { rhs: rhs_res.expr };
    TypeResult::new(ref_ty, kind)
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

fn get_expr_addressability<'hir>(ctx: &HirCtx, expr: &'hir hir::Expr<'hir>) -> Addressability {
    match expr.kind {
        hir::ExprKind::Error => Addressability::Unknown,
        hir::ExprKind::Const { .. } => Addressability::Temporary, //@TemporaryImmutable for struct / array? and alloca them
        hir::ExprKind::If { .. } => Addressability::Temporary,
        hir::ExprKind::Block { .. } => Addressability::Temporary,
        hir::ExprKind::Match { .. } => Addressability::Temporary,
        hir::ExprKind::StructField { target, .. } => get_expr_addressability(ctx, target),
        hir::ExprKind::SliceField { .. } => Addressability::SliceField,
        hir::ExprKind::Index { target, .. } => get_expr_addressability(ctx, target),
        hir::ExprKind::Slice { target, .. } => get_expr_addressability(ctx, target),
        hir::ExprKind::Cast { .. } => Addressability::Temporary,
        hir::ExprKind::LocalVar { local_id } => {
            let local = ctx.proc.get_local(local_id);
            Addressability::Addressable(
                local.mutt,
                SourceRange::new(ctx.proc.origin(), local.name.range),
            )
        }
        hir::ExprKind::ParamVar { param_id } => {
            let param = ctx.proc.get_param(param_id);
            Addressability::Addressable(
                param.mutt,
                SourceRange::new(ctx.proc.origin(), param.name.range),
            )
        }
        hir::ExprKind::ConstVar { .. } => Addressability::Constant,
        hir::ExprKind::GlobalVar { global_id } => {
            let data = ctx.registry.global_data(global_id);
            Addressability::Addressable(
                data.mutt,
                SourceRange::new(data.origin_id, data.name.range),
            )
        }
        hir::ExprKind::Variant { .. } => Addressability::NotImplemented, //@todo
        hir::ExprKind::CallDirect { .. } => Addressability::Temporary,
        hir::ExprKind::CallIndirect { .. } => Addressability::Temporary,
        hir::ExprKind::StructInit { .. } => Addressability::TemporaryImmutable,
        hir::ExprKind::ArrayInit { .. } => Addressability::TemporaryImmutable,
        hir::ExprKind::ArrayRepeat { .. } => Addressability::TemporaryImmutable,
        hir::ExprKind::Deref { rhs, .. } => get_expr_addressability(ctx, rhs),
        hir::ExprKind::Address { .. } => Addressability::Temporary,
        hir::ExprKind::Unary { .. } => Addressability::Temporary,
        hir::ExprKind::Binary { .. } => Addressability::Temporary,
    }
}

fn typecheck_unary<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    op: ast::UnOp,
    op_range: TextRange,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_expect = unary_rhs_expect(op, expect);
    let rhs_res = typecheck_expr(ctx, rhs_expect, rhs);
    let un_op = unary_op_check(ctx, ctx.proc.origin(), rhs_res.ty, op, op_range);

    if let Some(un_op) = un_op {
        let unary_type = unary_output_type(op, rhs_res.ty);
        let kind = hir::ExprKind::Unary {
            op: un_op,
            rhs: rhs_res.expr,
        };
        TypeResult::new(unary_type, kind)
    } else {
        TypeResult::error()
    }
}

//@experiment with rules for expectation
// use binary expr expect optionally? when not compatible
// different rules for cmp and arith?
//@look into peer type resolution: (this should work)
// just using lhs leaves default s32 that doesnt match rhs
// let x = 5 + 10 as u16;
// let y = 5 as u16 + 10;
fn typecheck_binary<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    op: ast::BinOp,
    op_range: TextRange,
    bin: &ast::BinExpr<'_>,
) -> TypeResult<'hir> {
    let lhs_expect = binary_lhs_expect(op, expect);
    let lhs_res = typecheck_expr(ctx, lhs_expect, bin.lhs);
    let bin_op = binary_op_check(ctx, ctx.proc.origin(), lhs_res.ty, op, op_range);

    let expect_src = SourceRange::new(ctx.proc.origin(), bin.lhs.range);
    let rhs_expect = binary_rhs_expect(op, lhs_res.ty, bin_op.is_some(), expect_src);
    let rhs_res = typecheck_expr(ctx, rhs_expect, bin.rhs);

    if let Some(bin_op) = bin_op {
        let binary_ty = binary_output_type(op, lhs_res.ty);
        let kind = hir::ExprKind::Binary {
            op: bin_op,
            lhs: lhs_res.expr,
            rhs: rhs_res.expr,
        };
        TypeResult::new(binary_ty, kind)
    } else {
        TypeResult::error()
    }
}

fn check_match_compatibility<'hir>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    ty: hir::Type,
    range: TextRange,
) {
    let compatible = match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::new(basic),
            BasicTypeKind::IntS | BasicTypeKind::IntU | BasicTypeKind::Bool | BasicTypeKind::Char
        ),
        hir::Type::Enum(_) => true,
        hir::Type::ArraySlice(slice) => {
            // allow []u8 assuming string literal (&u8 cstring literals not allowed)
            // @add str basic type for semantic clarity?
            // @same for cstr (which is &u8 currently)
            if slice.elem_ty.is_error() {
                true
            } else {
                matches!(slice.elem_ty, hir::Type::Basic(BasicType::U8))
            }
        }
        _ => false,
    };

    if !compatible {
        ctx.emit.error(ErrorComp::new(
            format!(
                "cannot match on value of type `{}`",
                type_format(ctx, ty).as_str()
            ),
            SourceRange::new(origin_id, range),
            None,
        ));
    }
}

fn typecheck_block<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expect: Expectation<'hir>,
    block: ast::Block,
    enter: BlockEnter,
) -> BlockResult<'hir> {
    ctx.proc.push_block(enter);

    let mut block_stmts: Vec<hir::Stmt> = Vec::with_capacity(block.stmts.len());
    let mut block_ty: Option<hir::Type> = None;
    let mut tail_range: Option<TextRange> = None;

    for stmt in block.stmts {
        let (hir_stmt, diverges) = match stmt.kind {
            ast::StmtKind::Break => {
                if let Some(stmt_res) = typecheck_break(ctx, stmt.range) {
                    let diverges = ctx.proc.check_stmt_diverges(true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Continue => {
                if let Some(stmt_res) = typecheck_continue(ctx, stmt.range) {
                    let diverges = ctx.proc.check_stmt_diverges(true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Return(expr) => {
                if let Some(stmt_res) = typecheck_return(ctx, expr, stmt.range) {
                    let diverges = ctx.proc.check_stmt_diverges(true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            //@defer block divergence should be simulated to correctly handle block_ty and divergence warnings 03.07.24
            ast::StmtKind::Defer(block) => {
                if let Some(stmt_res) = typecheck_defer(ctx, *block, stmt.range) {
                    let diverges = ctx.proc.check_stmt_diverges(false, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Loop(loop_) => {
                //@can diverge (inf loop, return, panic)
                let diverges = ctx.proc.check_stmt_diverges(false, stmt.range);
                let stmt_res = hir::Stmt::Loop(typecheck_loop(ctx, loop_));
                (stmt_res, diverges)
            }
            ast::StmtKind::Local(local) => {
                //@can diverge any diverging expr inside
                let diverges = ctx.proc.check_stmt_diverges(false, stmt.range);
                let local_res = typecheck_local(ctx, local);
                let stmt_res = match local_res {
                    LocalResult::Error => continue,
                    LocalResult::Local(local_id) => hir::Stmt::Local(local_id),
                    LocalResult::Discard(value) => hir::Stmt::Discard(value),
                };
                (stmt_res, diverges)
            }
            ast::StmtKind::Assign(assign) => {
                //@can diverge any diverging expr inside
                let diverges = ctx.proc.check_stmt_diverges(false, stmt.range);
                let stmt_res = hir::Stmt::Assign(typecheck_assign(ctx, assign));
                (stmt_res, diverges)
            }
            ast::StmtKind::ExprSemi(expr) => {
                //@can diverge but expression divergence isnt implemented (if, match, explicit `never` calls like panic)
                //@error or warn on expressions that arent used? 29.05.24
                // `arent used` would mean that result isnt stored anywhere?
                // but proc calls might have side effects and should always be allowed
                // eg: `20 + some_var;` `10.0 != 21.2;` `something[0] + something[1];` `call() + 10;`
                let expect = match expr.kind {
                    ast::ExprKind::If { .. }
                    | ast::ExprKind::Block { .. }
                    | ast::ExprKind::Match { .. } => Expectation::HasType(hir::Type::VOID, None),
                    _ => Expectation::None,
                };

                let error_count = ctx.emit.error_count();
                let expr_res = typecheck_expr(ctx, expect, expr);
                if !ctx.emit.did_error(error_count) {
                    check_unused_expr_semi(ctx, ctx.proc.origin(), expr_res.expr, expr.range);
                }

                //@migrate to using never type instead of diverges bool flags
                let will_diverge = expr_res.ty.is_never();
                let diverges = ctx.proc.check_stmt_diverges(will_diverge, stmt.range);

                let stmt_res = hir::Stmt::ExprSemi(expr_res.expr);
                (stmt_res, diverges)
            }
            ast::StmtKind::ExprTail(expr) => {
                /*
                //@30.05.24
                proc example() -> s32 {
                    // incorrect diverges warning since
                    // block isnt entered until typecheck_expr is called
                    -> { // after this
                        // warning for this
                        -> 10;
                    };
                }
                 */
                // type expectation is delegated to tail expression, instead of the block itself
                let expr_res = typecheck_expr(ctx, expect, expr);
                let stmt_res = hir::Stmt::ExprTail(expr_res.expr);
                // @seems to fix the problem (still a hack)
                let diverges = ctx.proc.check_stmt_diverges(true, stmt.range);

                // only assigned once, any further `ExprTail` are unreachable
                if block_ty.is_none() {
                    block_ty = Some(expr_res.ty);
                    tail_range = Some(expr.range);
                }
                (stmt_res, diverges)
            }
        };

        match diverges {
            Diverges::Maybe => block_stmts.push(hir_stmt),
            Diverges::Always(range) => {
                ctx.emit.warning(WarningComp::new(
                    "unreachable statement",
                    SourceRange::new(ctx.proc.origin(), stmt.range),
                    Info::new(
                        "all statements after this are unreachable",
                        SourceRange::new(ctx.proc.origin(), range),
                    ),
                ));
            }
            Diverges::AlwaysWarned => {}
        }
    }

    let stmts = ctx.arena.alloc_slice(&block_stmts);
    let hir_block = hir::Block { stmts };

    //@wip approach, will change 03.07.24
    let block_result = if let Some(block_ty) = block_ty {
        BlockResult::new(block_ty, hir_block, tail_range)
    } else {
        //@potentially incorrect aproach, verify that `void`
        // as the expectation and block result ty are valid 29.05.24
        let diverges = ctx.proc.diverges().is_always();
        if !diverges {
            check_type_expectation(ctx, ctx.proc.origin(), block.range, expect, hir::Type::VOID);
        }
        //@hack but should be correct
        let block_ty = if diverges {
            hir::Type::Basic(BasicType::Never)
        } else {
            hir::Type::VOID
        };
        BlockResult::new(block_ty, hir_block, tail_range)
    };

    ctx.proc.pop_block();
    block_result
}

/// returns `None` on invalid use of `break`
fn typecheck_break<'hir>(ctx: &mut HirCtx, stmt_range: TextRange) -> Option<hir::Stmt<'hir>> {
    match ctx.proc.loop_status() {
        LoopStatus::None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
            ctx.emit.error(ErrorComp::new(
                "cannot use `break` outside of loop",
                SourceRange::new(ctx.proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
            ctx.emit.error(ErrorComp::new(
                "cannot use `break` in loop started outside of `defer`",
                SourceRange::new(ctx.proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside => Some(hir::Stmt::Break),
    }
}

/// returns `None` on invalid use of `continue`
fn typecheck_continue<'hir>(ctx: &mut HirCtx, stmt_range: TextRange) -> Option<hir::Stmt<'hir>> {
    match ctx.proc.loop_status() {
        LoopStatus::None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
            ctx.emit.error(ErrorComp::new(
                "cannot use `continue` outside of loop",
                SourceRange::new(ctx.proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
            ctx.emit.error(ErrorComp::new(
                "cannot use `continue` in loop started outside of `defer`",
                SourceRange::new(ctx.proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside => Some(hir::Stmt::Continue),
    }
}

/// returns `None` on invalid use of `return`
fn typecheck_return<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    expr: Option<&ast::Expr>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let valid = if let DeferStatus::Inside(prev_defer) = ctx.proc.defer_status() {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
        ctx.emit.error(ErrorComp::new(
            "cannot use `return` inside `defer`",
            SourceRange::new(ctx.proc.origin(), kw_range),
            Info::new(
                "in this defer",
                SourceRange::new(ctx.proc.origin(), prev_defer),
            ),
        ));
        false
    } else {
        true
    };

    let expr = match expr {
        Some(expr) => {
            let expr_res = typecheck_expr(ctx, ctx.proc.return_expect(), expr);
            Some(expr_res.expr)
        }
        None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
            check_type_expectation(
                ctx,
                ctx.proc.origin(),
                kw_range,
                ctx.proc.return_expect(),
                hir::Type::VOID,
            );
            None
        }
    };

    if valid {
        Some(hir::Stmt::Return(expr))
    } else {
        None
    }
}

/// returns `None` on invalid use of `defer`
fn typecheck_defer<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    block: ast::Block,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());

    let valid = if let DeferStatus::Inside(prev_defer) = ctx.proc.defer_status() {
        ctx.emit.error(ErrorComp::new(
            "`defer` statements cannot be nested",
            SourceRange::new(ctx.proc.origin(), kw_range),
            Info::new(
                "already in this defer",
                SourceRange::new(ctx.proc.origin(), prev_defer),
            ),
        ));
        false
    } else {
        true
    };

    let block_res = typecheck_block(
        ctx,
        Expectation::HasType(hir::Type::VOID, None),
        block,
        BlockEnter::Defer(kw_range),
    );

    if valid {
        let block = ctx.arena.alloc(block_res.block);
        Some(hir::Stmt::Defer(block))
    } else {
        None
    }
}

fn typecheck_loop<'hir>(ctx: &mut HirCtx<'hir, '_>, loop_: &ast::Loop) -> &'hir hir::Loop<'hir> {
    let kind = match loop_.kind {
        ast::LoopKind::Loop => hir::LoopKind::Loop,
        ast::LoopKind::While { cond } => {
            let cond_res = typecheck_expr(ctx, Expectation::HasType(hir::Type::BOOL, None), cond);

            hir::LoopKind::While {
                cond: cond_res.expr,
            }
        }
        ast::LoopKind::ForLoop {
            local,
            cond,
            assign,
        } => {
            let local_res = typecheck_local(ctx, local);
            let cond_res = typecheck_expr(ctx, Expectation::HasType(hir::Type::BOOL, None), cond);
            let assign = typecheck_assign(ctx, assign);

            let bind = match local_res {
                LocalResult::Error => hir::ForLoopBind::Error,
                LocalResult::Local(local_id) => hir::ForLoopBind::Local(local_id),
                LocalResult::Discard(value) => hir::ForLoopBind::Discard(value),
            };
            hir::LoopKind::ForLoop {
                bind,
                cond: cond_res.expr,
                assign,
            }
        }
    };

    let block_res = typecheck_block(
        ctx,
        Expectation::HasType(hir::Type::VOID, None),
        loop_.block,
        BlockEnter::Loop,
    );

    let loop_ = hir::Loop {
        kind,
        block: block_res.block,
    };
    ctx.arena.alloc(loop_)
}

enum LocalResult<'hir> {
    Error,
    Local(hir::LocalID<'hir>),
    Discard(&'hir hir::Expr<'hir>),
}

fn typecheck_local<'hir>(ctx: &mut HirCtx<'hir, '_>, local: &ast::Local) -> LocalResult<'hir> {
    //@theres no `nice` way to find both existing name from global (hir) scope
    // and proc_scope, those are so far disconnected,
    // some unified model of symbols might be better in the future
    // this also applies to SymbolKind which is separate from VariableID (leads to some issues in path resolve) @1.04.24

    let already_defined = match local.bind {
        ast::Binding::Named(name) => {
            if let Err(error) =
                ctx.scope
                    .already_defined_check(&ctx.registry, ctx.proc.origin(), name)
            {
                error.emit(ctx);
                true
            } else if let Some(existing_var) = ctx.proc.find_variable(name.id) {
                let existing = match existing_var {
                    VariableID::Local(id) => {
                        SourceRange::new(ctx.proc.origin(), ctx.proc.get_local(id).name.range)
                    }
                    VariableID::Param(id) => {
                        SourceRange::new(ctx.proc.origin(), ctx.proc.get_param(id).name.range)
                    }
                };
                let name_src = SourceRange::new(ctx.proc.origin(), name.range);
                let name = ctx.name_str(name.id);
                err::scope_name_already_defined(&mut ctx.emit, name_src, existing, name);
                true
            } else {
                false
            }
        }
        ast::Binding::Discard(_) => false,
    };

    let (local_ty, expect) = match local.ty {
        Some(ty) => {
            let local_ty = super::pass_3::type_resolve(ctx, ctx.proc.origin(), ty);
            let expect_src = SourceRange::new(ctx.proc.origin(), ty.range);
            let expect = Expectation::HasType(local_ty, Some(expect_src));
            (Some(local_ty), expect)
        }
        None => (None, Expectation::None),
    };
    let init_res = typecheck_expr(ctx, expect, local.init);
    let local_ty = local_ty.unwrap_or(init_res.ty);

    match local.bind {
        ast::Binding::Named(name) => {
            if already_defined {
                LocalResult::Error
            } else {
                //@check for `never`, `void` to prevent panic during codegen
                let local = hir::Local {
                    mutt: local.mutt,
                    name,
                    ty: local_ty,
                    init: init_res.expr,
                };
                let local_id = ctx.proc.push_local(local);
                LocalResult::Local(local_id)
            }
        }
        ast::Binding::Discard(_) => LocalResult::Discard(init_res.expr),
    }
}

//@not checking bin assignment operators (need a good way to do it same in binary expr typecheck)
fn typecheck_assign<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    assign: &ast::Assign,
) -> &'hir hir::Assign<'hir> {
    let lhs_res = typecheck_expr(ctx, Expectation::None, assign.lhs);

    let adressability = get_expr_addressability(ctx, lhs_res.expr);
    match adressability {
        Addressability::Unknown => {}
        Addressability::Constant => {
            ctx.emit.error(ErrorComp::new(
                "cannot assign to a constant",
                SourceRange::new(ctx.proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            ctx.emit.error(ErrorComp::new(
                "cannot assign to a slice field, slice itself cannot be modified",
                SourceRange::new(ctx.proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Temporary | Addressability::TemporaryImmutable => {
            ctx.emit.error(ErrorComp::new(
                "cannot assign to a temporary value",
                SourceRange::new(ctx.proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Addressable(mutt, src) => {
            if mutt == ast::Mut::Immutable {
                ctx.emit.error(ErrorComp::new(
                    "cannot assign to an immutable variable",
                    SourceRange::new(ctx.proc.origin(), assign.lhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            ctx.emit.error(ErrorComp::new(
                "addressability not implemented for this expression",
                SourceRange::new(ctx.proc.origin(), assign.lhs.range),
                None,
            ));
        }
    }

    let assign_op = match assign.op {
        ast::AssignOp::Assign => hir::AssignOp::Assign,
        ast::AssignOp::Bin(op) => {
            binary_op_check(ctx, ctx.proc.origin(), lhs_res.ty, op, assign.op_range)
                .map(hir::AssignOp::Bin)
                .unwrap_or(hir::AssignOp::Assign)
        }
    };

    let expect_src = SourceRange::new(ctx.proc.origin(), assign.lhs.range);
    let rhs_expect = Expectation::HasType(lhs_res.ty, Some(expect_src));
    let rhs_res = typecheck_expr(ctx, rhs_expect, assign.rhs);

    let assign = hir::Assign {
        op: assign_op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_ty: lhs_res.ty,
    };
    ctx.arena.alloc(assign)
}

// these calls are only done for items so far @26.04.24
// locals or input params etc not checked yet (need to find a reasonable strategy)
// proc return type is allowed to be never or void unlike other instances where types are used
pub fn require_value_type(ctx: &mut HirCtx, ty: hir::Type, src: SourceRange) {
    if !type_is_value_type(ty) {
        ctx.emit.error(ErrorComp::new(
            format!(
                "expected value type, found `{}`",
                type_format(ctx, ty).as_str()
            ),
            src,
            None,
        ))
    }
}

pub fn type_is_value_type(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => !matches!(basic, BasicType::Void | BasicType::Never),
        hir::Type::Enum(_) => true,
        hir::Type::Struct(_) => true,
        hir::Type::Reference(ref_ty, _) => type_is_value_type(*ref_ty),
        hir::Type::Procedure(_) => true,
        hir::Type::ArraySlice(slice) => type_is_value_type(slice.elem_ty),
        hir::Type::ArrayStatic(array) => type_is_value_type(array.elem_ty),
    }
}

//==================== UNUSED CHECK ====================

fn check_unused_expr_semi(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    expr: &hir::Expr,
    expr_range: TextRange,
) {
    enum UnusedExpr {
        No,
        Maybe,
        Yes(&'static str),
    }

    fn return_type(ty: hir::Type) -> UnusedExpr {
        match ty {
            hir::Type::Error => UnusedExpr::No,
            hir::Type::Basic(BasicType::Void) => UnusedExpr::No,
            hir::Type::Basic(BasicType::Never) => UnusedExpr::No,
            _ => UnusedExpr::Yes("procedure return value"),
        }
    }

    let unused = match expr.kind {
        // errored expressions are not allowed to be checked
        hir::ExprKind::Error => unreachable!(),
        hir::ExprKind::Const { .. } => UnusedExpr::Yes("constant value"),
        hir::ExprKind::If { .. } => UnusedExpr::Maybe,
        hir::ExprKind::Block { .. } => UnusedExpr::Maybe,
        hir::ExprKind::Match { .. } => UnusedExpr::Maybe,
        hir::ExprKind::StructField { .. } => UnusedExpr::Yes("field access"),
        hir::ExprKind::SliceField { .. } => UnusedExpr::Yes("field access"),
        hir::ExprKind::Index { .. } => UnusedExpr::Yes("index access"),
        hir::ExprKind::Slice { .. } => UnusedExpr::Yes("slice value"),
        hir::ExprKind::Cast { .. } => UnusedExpr::Yes("cast value"),
        hir::ExprKind::LocalVar { .. } => UnusedExpr::Yes("local value"),
        hir::ExprKind::ParamVar { .. } => UnusedExpr::Yes("parameter value"),
        hir::ExprKind::ConstVar { .. } => UnusedExpr::Yes("constant value"),
        hir::ExprKind::GlobalVar { .. } => UnusedExpr::Yes("global value"),
        hir::ExprKind::Variant { .. } => UnusedExpr::Yes("variant value"),
        hir::ExprKind::CallDirect { proc_id, .. } => {
            let ty = ctx.registry.proc_data(proc_id).return_ty;
            return_type(ty)
        }
        hir::ExprKind::CallIndirect { indirect, .. } => {
            let ty = indirect.proc_ty.return_ty;
            return_type(ty)
        }
        hir::ExprKind::StructInit { .. } => UnusedExpr::Yes("struct value"),
        hir::ExprKind::ArrayInit { .. } => UnusedExpr::Yes("array value"),
        hir::ExprKind::ArrayRepeat { .. } => UnusedExpr::Yes("array value"),
        hir::ExprKind::Deref { .. } => UnusedExpr::Yes("dereference"),
        hir::ExprKind::Address { .. } => UnusedExpr::Yes("address value"),
        hir::ExprKind::Unary { .. } => UnusedExpr::Yes("unary operation"),
        hir::ExprKind::Binary { .. } => UnusedExpr::Yes("binary operation"),
    };

    if let UnusedExpr::Yes(kind) = unused {
        ctx.emit.warning(WarningComp::new(
            format!("unused {}", kind),
            SourceRange::new(origin_id, expr_range),
            None,
        ));
    }
}

//==================== INFERENCE ====================
//@check if reference expectations need to be accounted for

fn infer_enum_type<'hir>(
    emit: &mut HirEmit,
    expect: Expectation<'hir>,
    error_src: SourceRange,
) -> Option<hir::EnumID<'hir>> {
    let enum_id = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            hir::Type::Enum(enum_id) => Some(enum_id),
            _ => None,
        },
    };
    if enum_id.is_none() {
        emit.error(ErrorComp::new("cannot infer enum type", error_src, None));
    }
    enum_id
}

fn infer_struct_type<'hir>(
    emit: &mut HirEmit,
    expect: Expectation<'hir>,
    error_src: SourceRange,
) -> Option<hir::StructID<'hir>> {
    let struct_id = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            hir::Type::Struct(struct_id) => Some(struct_id),
            _ => None,
        },
    };
    if struct_id.is_none() {
        emit.error(ErrorComp::new("cannot infer struct type", error_src, None));
    }
    struct_id
}

//==================== DEFAULT CHECK ====================

fn error_result_default_check_input<'hir>(
    ctx: &mut HirCtx,
    input: &ast::ArgumentList,
) -> TypeResult<'hir> {
    for &expr in input.exprs.iter() {
        let _ = typecheck_expr(ctx, Expectation::None, expr);
    }
    TypeResult::error()
}

fn error_result_default_check_input_opt<'hir>(
    ctx: &mut HirCtx,
    input: Option<&ast::ArgumentList>,
) -> TypeResult<'hir> {
    if let Some(input) = input {
        for &expr in input.exprs.iter() {
            let _ = typecheck_expr(ctx, Expectation::None, expr);
        }
    }
    TypeResult::error()
}

fn error_result_default_check_field_init<'hir>(
    ctx: &mut HirCtx,
    input: &[ast::FieldInit],
) -> TypeResult<'hir> {
    for field in input.iter() {
        let _ = typecheck_expr(ctx, Expectation::None, field.expr);
    }
    TypeResult::error()
}

//==================== CALL-LIKE INPUT CHECK ====================

//@fix names for ArgumentList / BindingList related code
fn input_range(input: &ast::ArgumentList) -> TextRange {
    if input.exprs.is_empty() {
        input.range
    } else {
        let end = input.range.end();
        TextRange::new(end - 1.into(), end)
    }
}

fn input_opt_range(input: Option<&ast::ArgumentList>, default: TextRange) -> TextRange {
    if let Some(input) = input {
        input_range(input)
    } else {
        default
    }
}

fn error_unexpected_variant_arg_count(
    emit: &mut HirEmit,
    expected_count: usize,
    input_count: usize,
    error_src: SourceRange,
    variant_src: SourceRange,
) {
    let plural_end = if expected_count == 1 { "" } else { "s" };
    emit.error(ErrorComp::new(
        format!("expected {expected_count} argument{plural_end}, found {input_count}"),
        error_src,
        Info::new("variant defined here", variant_src),
    ));
}

fn error_unexpected_variant_bind_count(
    emit: &mut HirEmit,
    expected_count: usize,
    input_count: usize,
    error_src: SourceRange,
    variant_src: SourceRange,
) {
    let plural_end = if expected_count == 1 { "" } else { "s" };
    emit.error(ErrorComp::new(
        format!("expected {expected_count} binding{plural_end}, found {input_count}"),
        error_src,
        Info::new("variant defined here", variant_src),
    ));
}

fn error_unexpected_call_arg_count(
    emit: &mut HirEmit,
    expected_count: usize,
    input_count: usize,
    is_variadic: bool,
    error_src: SourceRange,
    info: Option<SourceRange>,
) {
    let at_least = if is_variadic { " at least" } else { "" };
    let plural_end = if expected_count == 1 { "" } else { "s" };
    let info = match info {
        Some(proc_src) => Info::new("procedure defined here", proc_src),
        None => None,
    };
    emit.error(ErrorComp::new(
        format!("expected{at_least} {expected_count} argument{plural_end}, found {input_count}"),
        error_src,
        info,
    ));
}

fn check_call_direct<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    proc_id: hir::ProcID<'hir>,
    input: &ast::ArgumentList,
) -> TypeResult<'hir> {
    let data = ctx.registry.proc_data(proc_id);
    let return_ty = data.return_ty;

    let input_count = input.exprs.len();
    let expected_count = data.params.len();
    let is_variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
    let wrong_count = (is_variadic && (input_count < expected_count))
        || (!is_variadic && (input_count != expected_count));

    if wrong_count {
        let input_range = input_range(input);
        error_unexpected_call_arg_count(
            &mut ctx.emit,
            expected_count,
            input_count,
            is_variadic,
            SourceRange::new(ctx.proc.origin(), input_range),
            Some(SourceRange::new(data.origin_id, data.name.range)),
        );
    }

    let mut values = Vec::with_capacity(data.params.len());
    for (idx, &expr) in input.exprs.iter().enumerate() {
        //@reborrow
        let data = ctx.registry.proc_data(proc_id);
        //@expect src, id with duplicates problem
        let expect = match data.params.get(idx) {
            Some(param) => Expectation::HasType(param.ty, None),
            None => Expectation::None,
        };
        let expr_res = typecheck_expr(ctx, expect, expr);
        values.push(expr_res.expr);
    }
    let values = ctx.arena.alloc_slice(&values);

    let kind = hir::ExprKind::CallDirect {
        proc_id,
        input: values,
    };
    TypeResult::new(return_ty, kind)
}

fn check_call_indirect<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    target_res: ExprResult<'hir>,
    input: &ast::ArgumentList,
) -> TypeResult<'hir> {
    let proc_ty = match target_res.ty {
        hir::Type::Error => return error_result_default_check_input(ctx, input),
        hir::Type::Procedure(proc_ty) => proc_ty,
        _ => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "cannot call value of type `{}`",
                    type_format(ctx, target_res.ty).as_str()
                ),
                SourceRange::new(ctx.proc.origin(), target_res.expr.range),
                None,
            ));
            return error_result_default_check_input(ctx, input);
        }
    };

    let input_count = input.exprs.len();
    let expected_count = proc_ty.param_types.len();
    let is_variadic = proc_ty.is_variadic;
    let wrong_count = (is_variadic && (input_count < expected_count))
        || (!is_variadic && (input_count != expected_count));

    if wrong_count {
        let input_range = input_range(input);
        error_unexpected_call_arg_count(
            &mut ctx.emit,
            expected_count,
            input_count,
            is_variadic,
            SourceRange::new(ctx.proc.origin(), input_range),
            None,
        );
    }

    let mut values = Vec::with_capacity(proc_ty.param_types.len());
    for (idx, &expr) in input.exprs.iter().enumerate() {
        let expect = match proc_ty.param_types.get(idx) {
            Some(param_ty) => Expectation::HasType(*param_ty, None),
            None => Expectation::None,
        };
        let expr_res = typecheck_expr(ctx, expect, expr);
        values.push(expr_res.expr);
    }
    let values = ctx.arena.alloc_slice(&values);

    let indirect = hir::CallIndirect {
        proc_ty,
        input: values,
    };
    let indirect = ctx.arena.alloc(indirect);
    let kind = hir::ExprKind::CallIndirect {
        target: target_res.expr,
        indirect,
    };
    TypeResult::new(proc_ty.return_ty, kind)
}

fn check_variant_input_opt<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    enum_id: hir::EnumID<'hir>,
    variant_id: hir::VariantID<'hir>,
    input: Option<&ast::ArgumentList>,
    error_range: TextRange,
) -> TypeResult<'hir> {
    let data = ctx.registry.enum_data(enum_id);
    let variant = data.variant(variant_id);

    let input_count = input.map(|i| i.exprs.len()).unwrap_or(0);
    let expected_count = variant.fields.len();

    if input_count != expected_count {
        let input_range = input_opt_range(input, error_range);
        error_unexpected_variant_arg_count(
            &mut ctx.emit,
            expected_count,
            input_count,
            SourceRange::new(ctx.proc.origin(), input_range),
            SourceRange::new(data.origin_id, variant.name.range),
        );
    }

    let input = if let Some(input) = input {
        let mut values = Vec::with_capacity(input.exprs.len());
        for (idx, &expr) in input.exprs.iter().enumerate() {
            //@expect src, id with duplicates problem
            let expect = match variant.fields.get(idx) {
                Some(ty) => Expectation::HasType(*ty, None),
                None => Expectation::None,
            };
            let expr_res = typecheck_expr(ctx, expect, expr);
            values.push(expr_res.expr);
        }
        let values = ctx.arena.alloc_slice(&values);
        let values = ctx.arena.alloc(values);
        Some(values)
    } else {
        None
    };

    //@deal with empty value or type list like () both on definition & expression
    let kind = hir::ExprKind::Variant {
        enum_id,
        variant_id,
        input,
    };
    TypeResult::new(hir::Type::Enum(enum_id), kind)
}

//==================== PATH RESOLVE ====================
//@refactor redudant duplication + more type safe leftover fields

/*
module     -> <first?>
proc       -> [no follow]
enum       -> <follow?> by single enum variant name
struct     -> [no follow]
const      -> <follow?> by <chained> field access
global     -> <follow?> by <chained> field access
param_var  -> <follow?> by <chained> field access
local_var  -> <follow?> by <chained> field access
*/

enum ResolvedPath<'hir> {
    None,
    Variable(VariableID<'hir>),
    Symbol(SymbolKind<'hir>),
}

fn path_resolve<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> (ResolvedPath<'hir>, usize) {
    let name = path.names.first().cloned().expect("non empty path");

    //@cannot tell in whitch context this body is
    // contant exprs, type defs dont have a `proc`
    if false {
        if let Some(var_id) = ctx.proc.find_variable(name.id) {
            return (ResolvedPath::Variable(var_id), 0);
        }
    }

    let (target_id, name) =
        match ctx
            .scope
            .symbol_from_scope(&ctx.registry, origin_id, origin_id, name)
        {
            Ok(kind) => {
                let next_name = path.names.get(1).cloned();
                match (kind, next_name) {
                    (SymbolKind::Module(module_id), Some(name)) => (module_id, name),
                    _ => return (ResolvedPath::Symbol(kind), 0),
                }
            }
            Err(error) => {
                error.emit(ctx);
                return (ResolvedPath::None, 0);
            }
        };

    match ctx
        .scope
        .symbol_from_scope(&ctx.registry, origin_id, target_id, name)
    {
        Ok(kind) => (ResolvedPath::Symbol(kind), 1),
        Err(error) => {
            error.emit(ctx);
            (ResolvedPath::None, 1)
        }
    }
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_type<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(ctx, origin_id, path);

    let ty = match resolved {
        ResolvedPath::None => return hir::Type::Error,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let source = match variable {
                VariableID::Local(id) => {
                    SourceRange::new(ctx.proc.origin(), ctx.proc.get_local(id).name.range)
                }
                VariableID::Param(id) => {
                    SourceRange::new(ctx.proc.origin(), ctx.proc.get_param(id).name.range)
                }
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            ctx.emit.error(ErrorComp::new(
                format!("expected type, found local `{}`", ctx.name_str(name.id)),
                SourceRange::new(origin_id, name.range),
                Info::new("defined here", source),
            ));
            return hir::Type::Error;
        }
        ResolvedPath::Symbol(kind) => match kind {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                let name = path.names[name_idx];
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "expected type, found {} `{}`",
                        kind.kind_name(),
                        ctx.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", kind.src(&ctx.registry)),
                ));
                return hir::Type::Error;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            ctx.emit.error(ErrorComp::new(
                "unexpected path segment",
                SourceRange::new(origin_id, range),
                None,
            ));
        }
    }

    ty
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_struct<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> Option<hir::StructID<'hir>> {
    let (resolved, name_idx) = path_resolve(ctx, origin_id, path);

    let struct_id = match resolved {
        ResolvedPath::None => return None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let source = match variable {
                VariableID::Local(id) => {
                    SourceRange::new(ctx.proc.origin(), ctx.proc.get_local(id).name.range)
                }
                VariableID::Param(id) => {
                    SourceRange::new(ctx.proc.origin(), ctx.proc.get_param(id).name.range)
                }
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            ctx.emit.error(ErrorComp::new(
                format!(
                    "expected struct type, found local `{}`",
                    ctx.name_str(name.id)
                ),
                SourceRange::new(origin_id, name.range),
                Info::new("defined here", source),
            ));
            return None;
        }
        ResolvedPath::Symbol(kind) => match kind {
            SymbolKind::Struct(id) => Some(id),
            _ => {
                let name = path.names[name_idx];
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "expected struct type, found {} `{}`",
                        kind.kind_name(),
                        ctx.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", kind.src(&ctx.registry)),
                ));
                return None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            ctx.emit.error(ErrorComp::new(
                "unexpected path segment",
                SourceRange::new(origin_id, range),
                None,
            ));
        }
    }
    struct_id
}

pub enum ValueID<'hir> {
    None,
    Proc(hir::ProcID<'hir>),
    Enum(hir::EnumID<'hir>, hir::VariantID<'hir>),
    Const(hir::ConstID<'hir>),
    Global(hir::GlobalID<'hir>),
    Local(hir::LocalID<'hir>),
    Param(hir::ParamID<'hir>),
}

pub fn path_resolve_value<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    path: &ast::Path<'ast>,
) -> (ValueID<'hir>, &'ast [ast::Name]) {
    let (resolved, name_idx) = path_resolve(ctx, origin_id, path);

    let value_id = match resolved {
        ResolvedPath::None => return (ValueID::None, &[]),
        ResolvedPath::Variable(var) => match var {
            VariableID::Local(id) => ValueID::Local(id),
            VariableID::Param(id) => ValueID::Param(id),
        },
        ResolvedPath::Symbol(kind) => match kind {
            SymbolKind::Proc(id) => {
                if let Some(remaining) = path.names.get(name_idx + 1..) {
                    if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
                        let range = TextRange::new(first.range.start(), last.range.end());
                        ctx.emit.error(ErrorComp::new(
                            "unexpected path segment",
                            SourceRange::new(origin_id, range),
                            None,
                        ));
                    }
                }
                ValueID::Proc(id)
            }
            SymbolKind::Enum(id) => {
                if let Some(variant_name) = path.names.get(name_idx + 1) {
                    let enum_data = ctx.registry.enum_data(id);
                    if let Some((variant_id, ..)) = enum_data.find_variant(variant_name.id) {
                        if let Some(remaining) = path.names.get(name_idx + 2..) {
                            if let (Some(first), Some(last)) = (remaining.first(), remaining.last())
                            {
                                let range = TextRange::new(first.range.start(), last.range.end());
                                ctx.emit.error(ErrorComp::new(
                                    "unexpected path segment",
                                    SourceRange::new(origin_id, range),
                                    None,
                                ));
                            }
                        }
                        return (ValueID::Enum(id, variant_id), &[]);
                    } else {
                        ctx.emit.error(ErrorComp::new(
                            format!(
                                "enum variant `{}` is not found",
                                ctx.name_str(variant_name.id)
                            ),
                            SourceRange::new(origin_id, variant_name.range),
                            Info::new("enum defined here", kind.src(&ctx.registry)),
                        ));
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    ctx.emit.error(ErrorComp::new(
                        format!(
                            "expected value, found {} `{}`",
                            kind.kind_name(),
                            ctx.name_str(name.id)
                        ),
                        SourceRange::new(origin_id, name.range),
                        Info::new("defined here", kind.src(&ctx.registry)),
                    ));
                    return (ValueID::None, &[]);
                }
            }
            SymbolKind::Const(id) => ValueID::Const(id),
            SymbolKind::Global(id) => ValueID::Global(id),
            _ => {
                let name = path.names[name_idx];
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "expected value, found {} `{}`",
                        kind.kind_name(),
                        ctx.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", kind.src(&ctx.registry)),
                ));
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

//==================== UNARY EXPR ====================

fn unary_rhs_expect(op: ast::UnOp, expect: Expectation) -> Expectation {
    match op {
        ast::UnOp::Neg | ast::UnOp::BitNot => expect,
        ast::UnOp::LogicNot => Expectation::HasType(hir::Type::BOOL, None),
    }
}

fn unary_output_type(op: ast::UnOp, rhs_ty: hir::Type) -> hir::Type {
    match op {
        ast::UnOp::Neg | ast::UnOp::BitNot => rhs_ty,
        ast::UnOp::LogicNot => hir::Type::BOOL,
    }
}

fn unary_op_check(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    rhs_ty: hir::Type,
    op: ast::UnOp,
    op_range: TextRange,
) -> Option<hir::UnOp> {
    if rhs_ty.is_error() {
        return None;
    }

    let un_op = match op {
        ast::UnOp::Neg => match rhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::UnOp::Neg_Int),
                BasicTypeKind::Float => Some(hir::UnOp::Neg_Float),
                _ => None,
            },
            _ => None,
        },
        ast::UnOp::BitNot => match rhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::UnOp::BitNot),
                _ => None,
            },
            _ => None,
        },
        ast::UnOp::LogicNot => match rhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::Bool => Some(hir::UnOp::LogicNot),
                _ => None,
            },
            _ => None,
        },
    };

    if un_op.is_none() {
        ctx.emit.error(ErrorComp::new(
            format!(
                "cannot apply unary operator `{}` on value of type `{}`",
                op.as_str(),
                type_format(ctx, rhs_ty).as_str()
            ),
            SourceRange::new(origin_id, op_range),
            None,
        ));
    }
    un_op
}

//==================== BINARY EXPR ====================

fn binary_lhs_expect(op: ast::BinOp, expect: Expectation) -> Expectation {
    match op {
        ast::BinOp::Add
        | ast::BinOp::Sub
        | ast::BinOp::Mul
        | ast::BinOp::Div
        | ast::BinOp::Rem
        | ast::BinOp::BitAnd
        | ast::BinOp::BitOr
        | ast::BinOp::BitXor
        | ast::BinOp::BitShl
        | ast::BinOp::BitShr => expect,
        ast::BinOp::IsEq
        | ast::BinOp::NotEq
        | ast::BinOp::Less
        | ast::BinOp::LessEq
        | ast::BinOp::Greater
        | ast::BinOp::GreaterEq => Expectation::None,
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => Expectation::HasType(hir::Type::BOOL, None),
    }
}

fn binary_rhs_expect(
    op: ast::BinOp,
    lhs_ty: hir::Type,
    compatible: bool,
    expect_src: SourceRange,
) -> Expectation {
    match op {
        ast::BinOp::Add
        | ast::BinOp::Sub
        | ast::BinOp::Mul
        | ast::BinOp::Div
        | ast::BinOp::Rem
        | ast::BinOp::BitAnd
        | ast::BinOp::BitOr
        | ast::BinOp::BitXor
        | ast::BinOp::BitShl
        | ast::BinOp::BitShr
        | ast::BinOp::IsEq
        | ast::BinOp::NotEq
        | ast::BinOp::Less
        | ast::BinOp::LessEq
        | ast::BinOp::Greater
        | ast::BinOp::GreaterEq => {
            if compatible {
                Expectation::HasType(lhs_ty, Some(expect_src))
            } else {
                Expectation::None
            }
        }
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => Expectation::HasType(hir::Type::BOOL, None),
    }
}

fn binary_output_type(op: ast::BinOp, lhs_ty: hir::Type) -> hir::Type {
    match op {
        ast::BinOp::Add
        | ast::BinOp::Sub
        | ast::BinOp::Mul
        | ast::BinOp::Div
        | ast::BinOp::Rem
        | ast::BinOp::BitAnd
        | ast::BinOp::BitOr
        | ast::BinOp::BitXor
        | ast::BinOp::BitShl
        | ast::BinOp::BitShr => lhs_ty,
        ast::BinOp::IsEq
        | ast::BinOp::NotEq
        | ast::BinOp::Less
        | ast::BinOp::LessEq
        | ast::BinOp::Greater
        | ast::BinOp::GreaterEq
        | ast::BinOp::LogicAnd
        | ast::BinOp::LogicOr => hir::Type::BOOL,
    }
}

fn binary_op_check(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    lhs_ty: hir::Type,
    op: ast::BinOp,
    op_range: TextRange,
) -> Option<hir::BinOp> {
    if lhs_ty.is_error() {
        return None;
    }

    let bin_op = match op {
        ast::BinOp::Add => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::Add_Int),
                BasicTypeKind::Float => Some(hir::BinOp::Add_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Sub => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::Sub_Int),
                BasicTypeKind::Float => Some(hir::BinOp::Sub_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Mul => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::Mul_Int),
                BasicTypeKind::Float => Some(hir::BinOp::Mul_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Div => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::Div_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::Div_IntU),
                BasicTypeKind::Float => Some(hir::BinOp::Div_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Rem => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::Rem_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::Rem_IntU),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::BitAnd => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::BitAnd),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::BitOr => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::BitOr),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::BitXor => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::BitXor),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::BitShl => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS | BasicTypeKind::IntU => Some(hir::BinOp::BitShl),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::BitShr => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::BitShr_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::BitShr_IntU),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::IsEq => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS
                | BasicTypeKind::IntU
                | BasicTypeKind::Bool
                | BasicTypeKind::Char
                | BasicTypeKind::Rawptr => Some(hir::BinOp::IsEq_Int),
                BasicTypeKind::Float => Some(hir::BinOp::NotEq_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::NotEq => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS
                | BasicTypeKind::IntU
                | BasicTypeKind::Bool
                | BasicTypeKind::Char
                | BasicTypeKind::Rawptr => Some(hir::BinOp::NotEq_Int),
                BasicTypeKind::Float => Some(hir::BinOp::NotEq_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Less => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::Less_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::Less_IntU),
                BasicTypeKind::Float => Some(hir::BinOp::Less_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::LessEq => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::LessEq_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::LessEq_IntU),
                BasicTypeKind::Float => Some(hir::BinOp::LessEq_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::Greater => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::Greater_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::Greater_IntU),
                BasicTypeKind::Float => Some(hir::BinOp::Greater_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::GreaterEq => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::IntS => Some(hir::BinOp::GreaterEq_IntS),
                BasicTypeKind::IntU => Some(hir::BinOp::GreaterEq_IntU),
                BasicTypeKind::Float => Some(hir::BinOp::GreaterEq_Float),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::LogicAnd => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::Bool => Some(hir::BinOp::LogicAnd),
                _ => None,
            },
            _ => None,
        },
        ast::BinOp::LogicOr => match lhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::Bool => Some(hir::BinOp::LogicOr),
                _ => None,
            },
            _ => None,
        },
    };

    if bin_op.is_none() {
        ctx.emit.error(ErrorComp::new(
            format!(
                "cannot apply binary operator `{}` on value of type `{}`",
                op.as_str(),
                type_format(ctx, lhs_ty).as_str()
            ),
            SourceRange::new(origin_id, op_range),
            None,
        ));
    }
    bin_op
}

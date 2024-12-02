use super::attr_check;
use super::check_path::{self, ValueID};
use super::constant;
use super::context::scope::{self, BlockStatus, Diverges};
use super::context::HirCtx;
use crate::ast::{self, BasicType};
use crate::error::{Error, ErrorSink, SourceRange, StringOrStr};
use crate::errors as err;
use crate::hir::{self, BasicFloat, BasicInt};
use crate::intern::NameID;
use crate::session::{self, ModuleID};
use crate::support::AsStr;
use crate::text::{TextOffset, TextRange};

pub fn typecheck_procedures(ctx: &mut HirCtx) {
    for proc_id in ctx.registry.proc_ids() {
        typecheck_proc(ctx, proc_id)
    }
}

fn typecheck_proc<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, proc_id: hir::ProcID) {
    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data(proc_id);

    if let Some(block) = item.block {
        let expect_src = SourceRange::new(data.origin_id, item.return_ty.range);
        let expect = Expectation::HasType(data.return_ty, Some(expect_src));

        ctx.scope.set_origin(data.origin_id);
        ctx.scope.set_poly(Some(hir::PolymorphDefID::Proc(proc_id)));
        ctx.scope.local.reset();
        ctx.scope.local.set_proc_context(data.params, expect); //shadowing params still addeded here
        let block_res = typecheck_block(ctx, expect, block, BlockStatus::None);

        let (locals, binds, for_binds) = ctx.scope.local.finish_proc_context();
        let locals = ctx.arena.alloc_slice(locals);
        let binds = ctx.arena.alloc_slice(binds);
        let for_binds = ctx.arena.alloc_slice(for_binds);

        let data = ctx.registry.proc_data_mut(proc_id);
        data.block = Some(block_res.block);
        data.locals = locals;
        data.local_binds = binds;
        data.for_binds = for_binds;

        for local in locals {
            if !local.was_used {
                let src = ctx.src(local.name.range);
                let name = ctx.name(local.name.id);
                err::scope_unused_variable(&mut ctx.emit, src, name);
            }
        }
        for bind in binds {
            if !bind.was_used {
                let src = ctx.src(bind.name.range);
                let name = ctx.name(bind.name.id);
                err::scope_unused_binding(&mut ctx.emit, src, name);
            }
        }
    }
}

pub fn type_matches(ctx: &HirCtx, ty: hir::Type, ty2: hir::Type) -> bool {
    match (ty, ty2) {
        (hir::Type::Error, _) => true,
        (_, hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (
            hir::Type::InferDef(poly_def_id, poly_param_idx),
            hir::Type::InferDef(poly_def_id2, poly_param_idx2),
        ) => poly_def_id == poly_def_id2 && poly_param_idx == poly_param_idx2,
        (hir::Type::Enum(id, poly_types), hir::Type::Enum(id2, poly_types2)) => {
            if id != id2 {
                return false;
            }
            //@not stable will always panic
            //@add deep error ty search to return `true`
            //when something errored (preserve false on different id?)
            //assert_eq!(poly_types.len(), poly_types2.len());
            //for idx in 0..poly_types.len() {
            //    if !type_matches(ctx, poly_types[idx], poly_types2[idx]) {
            //        return false;
            //    }
            //}
            true
        }
        (hir::Type::Struct(id, poly_types), hir::Type::Struct(id2, poly_types2)) => {
            if id != id2 {
                return false;
            }
            //@not stable will always panic
            //@add deep error ty search to return `true`
            //when something errored (preserve false on different id?)
            //assert_eq!(poly_types.len(), poly_types2.len());
            //for idx in 0..poly_types.len() {
            //    if !type_matches(ctx, poly_types[idx], poly_types2[idx]) {
            //        return false;
            //    }
            //}
            true
        }
        (hir::Type::Reference(mutt, ref_ty), hir::Type::Reference(mutt2, ref_ty2)) => {
            if mutt2 == ast::Mut::Mutable {
                type_matches(ctx, *ref_ty, *ref_ty2)
            } else {
                mutt == mutt2 && type_matches(ctx, *ref_ty, *ref_ty2)
            }
        }
        (hir::Type::MultiReference(mutt, ref_ty), hir::Type::MultiReference(mutt2, ref_ty2)) => {
            if mutt2 == ast::Mut::Mutable {
                type_matches(ctx, *ref_ty, *ref_ty2)
            } else {
                mutt == mutt2 && type_matches(ctx, *ref_ty, *ref_ty2)
            }
        }
        // [&]T -> &T (@does apply recursively!)
        (hir::Type::Reference(mutt, ref_ty), hir::Type::MultiReference(mutt2, ref_ty2)) => {
            if mutt2 == ast::Mut::Mutable {
                type_matches(ctx, *ref_ty, *ref_ty2)
            } else {
                mutt == mutt2 && type_matches(ctx, *ref_ty, *ref_ty2)
            }
        }
        // &T -> [&]T (@does apply recursively!)
        (hir::Type::MultiReference(mutt, ref_ty), hir::Type::Reference(mutt2, ref_ty2)) => {
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
        hir::Type::InferDef(poly_def_id, poly_param_idx) => {
            let name = ctx.poly_param_name(poly_def_id, poly_param_idx);
            ctx.name(name.id).to_string().into()
        }
        hir::Type::Enum(id, poly_types) => {
            let name = ctx.name(ctx.registry.enum_data(id).name.id);

            if !poly_types.is_empty() {
                let mut format = String::with_capacity(64);
                let mut first = false;
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
                let mut first = false;
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
            format.push_str(") ");
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

pub fn type_expectation_check(
    ctx: &mut HirCtx,
    from_range: TextRange,
    found_ty: hir::Type,
    expect: Expectation,
) {
    match expect {
        Expectation::None => {}
        Expectation::HasType(expect_ty, expect_src) => {
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
    }
}

pub struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    kind: hir::ExprKind<'hir>,
    ignore: bool,
}

pub struct ExprResult<'hir> {
    ty: hir::Type<'hir>,
    pub expr: &'hir hir::Expr<'hir>,
}

struct PatResult<'hir> {
    pat: hir::Pat<'hir>,
    pat_ty: hir::Type<'hir>,
}

struct BlockResult<'hir> {
    ty: hir::Type<'hir>,
    block: hir::Block<'hir>,
    tail_range: Option<TextRange>,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, kind: hir::ExprKind<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            kind,
            ignore: false,
        }
    }
    fn new_ignore(ty: hir::Type<'hir>, kind: hir::ExprKind<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            kind,
            ignore: true,
        }
    }
    fn error() -> TypeResult<'hir> {
        TypeResult {
            ty: hir::Type::Error,
            kind: hir::ExprKind::Error,
            ignore: true,
        }
    }
    fn into_expr_result(
        self,
        ctx: &mut HirCtx<'hir, '_, '_>,
        range: TextRange,
    ) -> ExprResult<'hir> {
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

impl<'hir> PatResult<'hir> {
    fn new(pat: hir::Pat<'hir>, pat_ty: hir::Type<'hir>) -> PatResult<'hir> {
        PatResult { pat, pat_ty }
    }
    fn error() -> PatResult<'hir> {
        PatResult {
            pat: hir::Pat::Error,
            pat_ty: hir::Type::Error,
        }
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

    fn into_type_result(self) -> TypeResult<'hir> {
        let kind = hir::ExprKind::Block { block: self.block };
        TypeResult::new(self.ty, kind)
    }
}

#[must_use]
pub fn typecheck_expr<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
) -> ExprResult<'hir> {
    let expr_res = match expr.kind {
        ast::ExprKind::Lit { lit } => typecheck_lit(expect, lit),
        ast::ExprKind::If { if_ } => typecheck_if(ctx, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            typecheck_block(ctx, expect, *block, BlockStatus::None).into_type_result()
        }
        ast::ExprKind::Match { match_ } => typecheck_match(ctx, expect, match_, expr.range),
        ast::ExprKind::Field { target, name } => typecheck_field(ctx, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(ctx, target, index, expr.range),
        ast::ExprKind::Slice {
            target,
            mutt,
            range,
        } => typecheck_slice(ctx, target, mutt, range, expr.range),
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
        ast::ExprKind::ArrayRepeat { value, len } => {
            typecheck_array_repeat(ctx, expect, value, len)
        }
        ast::ExprKind::Deref { rhs } => typecheck_deref(ctx, rhs, expr.range),
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(ctx, mutt, rhs, expr.range),
        ast::ExprKind::Range { range } => typecheck_range(ctx, range, expr.range),
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(ctx, expect, op, op_range, rhs)
        }
        ast::ExprKind::Binary {
            op,
            op_start,
            lhs,
            rhs,
        } => typecheck_binary(ctx, expect, op, op_start, lhs, rhs),
    };

    if !expr_res.ignore {
        type_expectation_check(ctx, expr.range, expr_res.ty, expect);
    }
    expr_res.into_expr_result(ctx, expr.range)
}

//@not range checked
fn typecheck_lit<'hir>(expect: Expectation, lit: ast::Lit) -> TypeResult<'hir> {
    let (value, ty) = match lit {
        ast::Lit::Void => {
            let value = hir::ConstValue::Void;
            (value, hir::Type::Basic(BasicType::Void))
        }
        ast::Lit::Null => {
            let value = hir::ConstValue::Null;
            (value, hir::Type::Basic(BasicType::Rawptr))
        }
        ast::Lit::Bool(val) => {
            let value = hir::ConstValue::Bool { val };
            (value, hir::Type::Basic(BasicType::Bool))
        }
        ast::Lit::Int(val) => {
            let int_ty = infer_int_type(expect);
            let value = hir::ConstValue::Int {
                val,
                neg: false,
                int_ty,
            };
            (value, hir::Type::Basic(int_ty.into_basic()))
        }
        ast::Lit::Float(val) => {
            let float_ty = infer_float_type(expect);
            let value = hir::ConstValue::Float { val, float_ty };
            (value, hir::Type::Basic(float_ty.into_basic()))
        }
        ast::Lit::Char(val) => {
            let value = hir::ConstValue::Char { val };
            (value, hir::Type::Basic(BasicType::Char))
        }
        ast::Lit::String(string_lit) => {
            const REF_U8: hir::Type =
                hir::Type::Reference(ast::Mut::Immutable, &hir::Type::Basic(BasicType::U8));
            const SLICE_U8: hir::Type = hir::Type::ArraySlice(&hir::ArraySlice {
                mutt: ast::Mut::Immutable,
                elem_ty: hir::Type::Basic(BasicType::U8),
            });

            let value = hir::ConstValue::String { string_lit };
            let string_ty = if string_lit.c_string {
                REF_U8
            } else {
                SLICE_U8
            };
            (value, string_ty)
        }
    };

    let kind = hir::ExprKind::Const { value };
    TypeResult::new(ty, kind)
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
    let mut if_type = hir::Type::NEVER;

    let mut branches = Vec::with_capacity(if_.branches.len());
    for branch in if_.branches {
        let branch = typecheck_branch(ctx, &mut expect, &mut if_type, branch);
        branches.push(branch);
    }
    let branches = ctx.arena.alloc_slice(&branches);

    let else_block = match if_.else_block {
        Some(block) => {
            let block_res = typecheck_block(ctx, expect, block, BlockStatus::None);
            type_unify_control_flow(&mut if_type, block_res.ty);
            Some(block_res.block)
        }
        None => None,
    };

    if else_block.is_none() && if_type.is_never() {
        if_type = hir::Type::VOID;
    }

    if else_block.is_none() && !if_type.is_error() && !if_type.is_void() && !if_type.is_never() {
        let src = ctx.src(expr_range);
        err::tycheck_if_missing_else(&mut ctx.emit, src);
    }

    let if_ = hir::If {
        branches,
        else_block,
    };
    let if_ = ctx.arena.alloc(if_);
    let kind = hir::ExprKind::If { if_ };
    TypeResult::new_ignore(if_type, kind)
}

fn typecheck_branch<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: &mut Expectation<'hir>,
    if_type: &mut hir::Type<'hir>,
    branch: &ast::Branch<'ast>,
) -> hir::Branch<'hir> {
    let expect_bool = Expectation::HasType(hir::Type::BOOL, None);
    let cond_res = typecheck_expr(ctx, expect_bool, branch.cond);
    let block_res = typecheck_block(ctx, *expect, branch.block, BlockStatus::None);
    type_unify_control_flow(if_type, block_res.ty);

    if let Expectation::None = expect {
        if !block_res.ty.is_error() && !block_res.ty.is_never() {
            let expect_src = block_res.tail_range.map(|range| ctx.src(range));
            *expect = Expectation::HasType(block_res.ty, expect_src);
        }
    }

    hir::Branch {
        cond: cond_res.expr,
        block: block_res.block,
    }
}

fn typecheck_match<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    mut expect: Expectation<'hir>,
    match_: &ast::Match<'ast>,
    match_range: TextRange,
) -> TypeResult<'hir> {
    let mut match_type = hir::Type::NEVER;
    let error_count = ctx.emit.error_count();

    let on_res = typecheck_expr(ctx, Expectation::None, match_.on_expr);
    let kind_res = super::match_check::match_kind(on_res.ty);

    if let Err(true) = kind_res {
        let src = ctx.src(on_res.expr.range);
        let ty_fmt = type_format(ctx, on_res.ty);
        err::tycheck_cannot_match_on_ty(&mut ctx.emit, src, ty_fmt.as_str());
    }

    let (pat_expect, ref_mut) = match kind_res {
        Ok(hir::MatchKind::Enum { enum_id, ref_mut }) => {
            let expect_src = ctx.src(on_res.expr.range);
            //@gen types not handled
            let enum_ty = hir::Type::Enum(enum_id, &[]);
            (Expectation::HasType(enum_ty, Some(expect_src)), ref_mut)
        }
        Ok(_) => {
            let expect_src = ctx.src(on_res.expr.range);
            (Expectation::HasType(on_res.ty, Some(expect_src)), None)
        }
        Err(_) => (Expectation::HasType(hir::Type::Error, None), None),
    };

    let mut arms = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms {
        ctx.scope.local.start_block(BlockStatus::None);
        let pat = typecheck_pat(ctx, pat_expect, &arm.pat, ref_mut, false);
        let expr_res = typecheck_expr(ctx, expect, arm.expr);
        type_unify_control_flow(&mut match_type, expr_res.ty);
        ctx.scope.local.exit_block();

        if matches!(expect, Expectation::None) {
            if !expr_res.ty.is_error() && !expr_res.ty.is_never() {
                let expect_src = ctx.src(arm.expr.range);
                expect = Expectation::HasType(expr_res.ty, Some(expect_src));
            }
        }

        let block = match expr_res.expr.kind {
            hir::ExprKind::Block { block } => block,
            _ => {
                let tail_stmt = hir::Stmt::ExprTail(expr_res.expr);
                let stmts = ctx.arena.alloc_slice(&[tail_stmt]);
                hir::Block { stmts }
            }
        };
        arms.push(hir::MatchArm { pat, block });
    }

    let kind = match kind_res {
        Ok(kind) => kind,
        Err(_) => return TypeResult::error(),
    };
    if ctx.emit.did_error(error_count) {
        return TypeResult::error();
    }
    if !match_const_pats_resolved(ctx, &arms) {
        return TypeResult::error();
    }

    let mut match_kw = TextRange::empty_at(match_range.start());
    match_kw.extend_by(5.into());
    super::match_check::match_cov(ctx, kind, arms.as_mut_slice(), match_.arms, match_kw);

    let arms = ctx.arena.alloc_slice(&arms);
    let match_ = hir::Match {
        on_expr: on_res.expr,
        arms,
    };
    let match_ = ctx.arena.alloc(match_);
    let kind = hir::ExprKind::Match { kind, match_ };
    TypeResult::new_ignore(match_type, kind)
}

fn match_const_pats_resolved(ctx: &HirCtx, arms: &[hir::MatchArm]) -> bool {
    fn const_value_resolved_ok(ctx: &HirCtx, const_id: hir::ConstID) -> bool {
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

fn typecheck_pat<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    pat: &ast::Pat<'ast>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
) -> hir::Pat<'hir> {
    let pat_res = match pat.kind {
        ast::PatKind::Wild => PatResult::new(hir::Pat::Wild, hir::Type::Error),
        ast::PatKind::Lit { expr } => typecheck_pat_lit(ctx, expect, expr),
        ast::PatKind::Item { path, bind_list } => {
            typecheck_pat_item(ctx, path, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Variant { name, bind_list } => {
            typecheck_pat_variant(ctx, expect, name, bind_list, ref_mut, in_or_pat, pat.range)
        }
        ast::PatKind::Or { pats } => typecheck_pat_or(ctx, expect, pats, ref_mut),
    };

    type_expectation_check(ctx, pat.range, pat_res.pat_ty, expect);
    pat_res.pat
}

fn typecheck_pat_lit<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'ast>,
) -> PatResult<'hir> {
    let expr_res = typecheck_expr(ctx, expect, expr);
    let src = ctx.src(expr_res.expr.range);

    match constant::fold_const_expr(ctx, src, expr_res.expr) {
        Ok(value) => PatResult::new(hir::Pat::Lit(value), expr_res.ty),
        Err(()) => PatResult::error(),
    }
}

fn typecheck_pat_item<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    bind_list: Option<&ast::BindingList>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
    pat_range: TextRange,
) -> PatResult<'hir> {
    match check_path::path_resolve_value(ctx, path) {
        ValueID::None => {
            check_variant_bind_list(ctx, bind_list, None, None, in_or_pat);
            PatResult::error()
        }
        ValueID::Enum(enum_id, variant_id) => {
            check_variant_bind_count(ctx, bind_list, enum_id, variant_id, pat_range);
            let variant = Some(ctx.registry.enum_data(enum_id).variant(variant_id));
            let bind_ids = check_variant_bind_list(ctx, bind_list, variant, ref_mut, in_or_pat);

            PatResult::new(
                hir::Pat::Variant(enum_id, variant_id, bind_ids),
                //@gen types not handled
                hir::Type::Enum(enum_id, &[]),
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
            check_variant_bind_list(ctx, bind_list, None, None, in_or_pat);
            let data = ctx.registry.const_data(const_id);
            PatResult::new(hir::Pat::Const(const_id), data.ty)
        }
        ValueID::Proc(_)
        | ValueID::Global(_, _)
        | ValueID::Param(_, _)
        | ValueID::Local(_, _)
        | ValueID::LocalBind(_, _)
        | ValueID::ForBind(_, _) => {
            let src = ctx.src(pat_range);
            err::tycheck_pat_runtime_value(&mut ctx.emit, src);
            check_variant_bind_list(ctx, bind_list, None, None, in_or_pat);
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
    let enum_id = match infer_enum_type(ctx, expect, name_src) {
        Some(found) => found,
        None => {
            check_variant_bind_list(ctx, bind_list, None, None, in_or_pat);
            return PatResult::error();
        }
    };
    let variant_id = match scope::check_find_enum_variant(ctx, enum_id, name) {
        Some(found) => found,
        None => {
            check_variant_bind_list(ctx, bind_list, None, None, in_or_pat);
            return PatResult::error();
        }
    };

    check_variant_bind_count(ctx, bind_list, enum_id, variant_id, pat_range);
    let variant = Some(ctx.registry.enum_data(enum_id).variant(variant_id));
    let bind_ids = check_variant_bind_list(ctx, bind_list, variant, ref_mut, in_or_pat);

    PatResult::new(
        hir::Pat::Variant(enum_id, variant_id, bind_ids),
        //@gen types not handled
        hir::Type::Enum(enum_id, &[]),
    )
}

fn typecheck_pat_or<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    pats: &[ast::Pat<'ast>],
    ref_mut: Option<ast::Mut>,
) -> PatResult<'hir> {
    let mut patterns = Vec::with_capacity(pats.len());
    for pat in pats {
        let pat = typecheck_pat(ctx, expect, pat, ref_mut, true);
        patterns.push(pat);
    }
    let pats = ctx.arena.alloc_slice(&patterns);

    PatResult::new(hir::Pat::Or(pats), hir::Type::Error)
}

fn typecheck_field<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let field_result = check_field_from_type(ctx, name, target_res.ty);
    emit_field_expr(target_res.expr, field_result)
}

struct FieldResult<'hir> {
    deref: Option<ast::Mut>,
    kind: FieldKind<'hir>,
    field_ty: hir::Type<'hir>,
}

#[rustfmt::skip]
enum FieldKind<'hir> {
    Struct(hir::StructID, hir::FieldID),
    ArraySlice { field: hir::SliceField },
    ArrayStatic { len: hir::ConstValue<'hir> },
}

impl<'hir> FieldResult<'hir> {
    fn new(
        deref: Option<ast::Mut>,
        kind: FieldKind<'hir>,
        field_ty: hir::Type<'hir>,
    ) -> FieldResult<'hir> {
        FieldResult {
            deref,
            kind,
            field_ty,
        }
    }
}

fn check_field_from_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    name: ast::Name,
    ty: hir::Type<'hir>,
) -> Option<FieldResult<'hir>> {
    let (ty, deref) = match ty {
        hir::Type::Reference(mutt, ref_ty) => (*ref_ty, Some(mutt)),
        _ => (ty, None),
    };

    match ty {
        hir::Type::Error => None,
        //@gen types not handled
        hir::Type::Struct(struct_id, _) => match check_field_from_struct(ctx, struct_id, name) {
            Some((field_id, field)) => {
                let kind = FieldKind::Struct(struct_id, field_id);
                let field_ty = field.ty;
                Some(FieldResult::new(deref, kind, field_ty))
            }
            None => None,
        },
        hir::Type::ArraySlice(slice) => match check_field_from_slice(ctx, name) {
            Some(field) => {
                let kind = FieldKind::ArraySlice { field };
                let field_ty = match field {
                    hir::SliceField::Ptr => hir::Type::Reference(slice.mutt, &slice.elem_ty),
                    hir::SliceField::Len => hir::Type::USIZE,
                };
                Some(FieldResult::new(deref, kind, field_ty))
            }
            None => None,
        },
        hir::Type::ArrayStatic(array) => match check_field_from_array(ctx, name, array) {
            Some(len) => {
                let kind = FieldKind::ArrayStatic { len };
                let field_ty = hir::Type::USIZE;
                Some(FieldResult::new(deref, kind, field_ty))
            }
            None => None,
        },
        _ => {
            let src = ctx.src(name.range);
            let field_name = ctx.name(name.id);
            let ty_fmt = type_format(ctx, ty);
            err::tycheck_field_not_found_ty(&mut ctx.emit, src, field_name, ty_fmt.as_str());
            None
        }
    }
}

fn check_field_from_struct<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    struct_id: hir::StructID,
    name: ast::Name,
) -> Option<(hir::FieldID, &'hir hir::Field<'hir>)> {
    if let Some(field_id) = scope::check_find_struct_field(ctx, struct_id, name) {
        let data = ctx.registry.struct_data(struct_id);
        let field = data.field(field_id);

        if ctx.scope.origin() != data.origin_id && field.vis == ast::Vis::Private {
            let src = ctx.src(name.range);
            let field_name = ctx.name(name.id);
            let field_src = SourceRange::new(data.origin_id, field.name.range);
            err::tycheck_field_is_private(&mut ctx.emit, src, field_name, field_src);
        }
        Some((field_id, field))
    } else {
        None
    }
}

fn check_field_from_slice<'hir>(ctx: &mut HirCtx, name: ast::Name) -> Option<hir::SliceField> {
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
                hir::ArrayStaticLen::Immediate(len) => hir::ConstValue::Int {
                    val: len,
                    neg: false,
                    int_ty: hir::BasicInt::Usize,
                },
                hir::ArrayStaticLen::ConstEval(eval_id) => {
                    let (eval, _) = ctx.registry.const_eval(eval_id);
                    match eval.resolved() {
                        Ok(value_id) => ctx.const_intern.get(value_id),
                        Err(()) => return None,
                    }
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
            access: hir::StructFieldAccess {
                deref: result.deref,
                struct_id,
                field_id,
            },
        },
        FieldKind::ArraySlice { field } => hir::ExprKind::SliceField {
            target,
            access: hir::SliceFieldAccess {
                deref: result.deref,
                field,
            },
        },
        FieldKind::ArrayStatic { len } => hir::ExprKind::Const { value: len },
    };

    TypeResult::new(result.field_ty, kind)
}

struct CollectionType<'hir> {
    deref: Option<ast::Mut>,
    elem_ty: hir::Type<'hir>,
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
        hir::Type::MultiReference(mutt, ref_ty) => Ok(Some(CollectionType {
            deref,
            elem_ty: *ref_ty,
            kind: CollectionKind::Multi(mutt),
        })),
        hir::Type::ArraySlice(slice) => Ok(Some(CollectionType {
            deref,
            elem_ty: slice.elem_ty,
            kind: CollectionKind::Slice(slice),
        })),
        hir::Type::ArrayStatic(array) => Ok(Some(CollectionType {
            deref,
            elem_ty: array.elem_ty,
            kind: CollectionKind::Array(array),
        })),
        _ => Err(()),
    }
}

fn typecheck_index<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    index: &ast::Expr<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let index_res = typecheck_expr(ctx, Expectation::HasType(hir::Type::USIZE, None), index);

    match type_as_collection(target_res.ty) {
        Ok(None) => TypeResult::error(),
        Ok(Some(collection)) => {
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
            };
            let kind = hir::ExprKind::Index {
                target: target_res.expr,
                access: ctx.arena.alloc(access),
            };
            TypeResult::new(collection.elem_ty, kind)
        }
        Err(()) => {
            let src = ctx.src(expr_range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_index_on_ty(&mut ctx.emit, src, ty_fmt.as_str());
            TypeResult::error()
        }
    }
}

fn typecheck_slice<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    mutt: ast::Mut,
    range: &ast::Expr<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let src = ctx.src(expr_range);
    err::internal_slice_expr_not_implemented(&mut ctx.emit, src);
    TypeResult::error()
}

fn typecheck_call<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    args_list: &ast::ArgumentList<'ast>,
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

fn typecheck_cast<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    target: &ast::Expr<'ast>,
    into: &ast::Type<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    use hir::CastKind;
    let target_res = typecheck_expr(ctx, Expectation::None, target);
    let from = target_res.ty;
    let into = super::pass_3::type_resolve(ctx, *into, false);

    if from.is_error() || into.is_error() {
        return TypeResult::error();
    }

    let cast_kind = match (from, into) {
        (hir::Type::Basic(from), hir::Type::Basic(into)) => cast_basic_into_basic(ctx, from, into),
        (hir::Type::Enum(from, _), hir::Type::Basic(into)) => cast_enum_into_basic(ctx, from, into),
        (hir::Type::Basic(BasicType::Rawptr), hir::Type::Reference(_, _))
        | (hir::Type::Basic(BasicType::Rawptr), hir::Type::MultiReference(_, _))
        | (hir::Type::Basic(BasicType::Rawptr), hir::Type::Procedure(_))
        | (hir::Type::Reference(_, _), hir::Type::Basic(BasicType::Rawptr))
        | (hir::Type::MultiReference(_, _), hir::Type::Basic(BasicType::Rawptr))
        | (hir::Type::Procedure(_), hir::Type::Basic(BasicType::Rawptr)) => CastKind::NoOp,
        (hir::Type::Reference(_, _), hir::Type::MultiReference(_, _))
        | (hir::Type::MultiReference(_, _), hir::Type::Reference(_, _)) => {
            type_matches(ctx, into, from)
                .then_some(CastKind::NoOpUnchecked)
                .unwrap_or(CastKind::Error)
        }
        _ => CastKind::Error,
    };

    if let CastKind::Error = cast_kind {
        let src = ctx.src(expr_range);
        let from_ty = type_format(ctx, from);
        let into_ty = type_format(ctx, into);
        err::tycheck_cast_invalid(&mut ctx.emit, src, from_ty.as_str(), into_ty.as_str());
        return TypeResult::error();
    }

    if let CastKind::NoOp = cast_kind {
        if type_matches(ctx, into, from) {
            let src = ctx.src(expr_range);
            let from_ty = type_format(ctx, from);
            let into_ty = type_format(ctx, into);
            err::tycheck_cast_redundant(&mut ctx.emit, src, from_ty.as_str(), into_ty.as_str());
        }
    }

    let kind = hir::ExprKind::Cast {
        target: target_res.expr,
        into: ctx.arena.alloc(into),
        kind: cast_kind,
    };
    TypeResult::new(into, kind)
}

fn cast_basic_into_basic(ctx: &HirCtx, from: BasicType, into: BasicType) -> hir::CastKind {
    use hir::CastKind;
    let from_kind = BasicTypeKind::new(from);
    let into_kind = BasicTypeKind::new(into);
    let from_size = constant::basic_layout(ctx, from).size();
    let into_size = constant::basic_layout(ctx, into).size();

    match from_kind {
        BasicTypeKind::IntS => match into_kind {
            BasicTypeKind::IntS | BasicTypeKind::IntU => {
                if from_size < into_size {
                    CastKind::IntS_Sign_Extend
                } else if from_size > into_size {
                    CastKind::Int_Trunc
                } else {
                    CastKind::NoOp
                }
            }
            BasicTypeKind::Float => CastKind::IntS_to_Float,
            _ => CastKind::Error,
        },
        BasicTypeKind::IntU => match into_kind {
            BasicTypeKind::IntS | BasicTypeKind::IntU => {
                if from_size < into_size {
                    CastKind::IntU_Zero_Extend
                } else if from_size > into_size {
                    CastKind::Int_Trunc
                } else {
                    CastKind::NoOp
                }
            }
            BasicTypeKind::Float => CastKind::IntU_to_Float,
            _ => CastKind::Error,
        },
        BasicTypeKind::Float => match into_kind {
            BasicTypeKind::IntS => CastKind::Float_to_IntS,
            BasicTypeKind::IntU => CastKind::Float_to_IntU,
            BasicTypeKind::Float => {
                if from_size < into_size {
                    CastKind::Float_Extend
                } else if from_size > into_size {
                    CastKind::Float_Trunc
                } else {
                    CastKind::NoOp
                }
            }
            _ => CastKind::Error,
        },
        BasicTypeKind::Bool => match into_kind {
            BasicTypeKind::Bool => CastKind::NoOp,
            BasicTypeKind::IntS | BasicTypeKind::IntU => CastKind::Bool_to_Int,
            _ => CastKind::Error,
        },
        BasicTypeKind::Char => match into {
            BasicType::Char => CastKind::NoOp,
            BasicType::U32 => CastKind::Char_to_U32,
            _ => CastKind::Error,
        },
        BasicTypeKind::Rawptr => match into {
            BasicType::Rawptr => CastKind::NoOp,
            _ => CastKind::Error,
        },
        BasicTypeKind::Void => match into {
            BasicType::Void => CastKind::NoOp,
            _ => CastKind::Error,
        },
        BasicTypeKind::Never => CastKind::Error,
    }
}

fn cast_enum_into_basic(ctx: &HirCtx, from: hir::EnumID, into: BasicType) -> hir::CastKind {
    use hir::CastKind;
    let enum_data = ctx.registry.enum_data(from);
    let into_kind = BasicTypeKind::new(into);

    if enum_data.attr_set.contains(hir::EnumFlag::WithFields) {
        return CastKind::Error;
    }

    if let BasicTypeKind::IntS | BasicTypeKind::IntU = into_kind {
        if let Ok(tag_ty) = enum_data.tag_ty.resolved() {
            cast_basic_into_basic(ctx, tag_ty.into_basic(), into)
        } else {
            CastKind::NoOpUnchecked
        }
    } else {
        CastKind::Error
    }
}

//@resulting layout sizes are not checked to fit in usize
// this can be partially adressed if each hir::Expr
// is folded, thus range being range checked for `usize`
fn typecheck_sizeof<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    ty: ast::Type<'ast>,
    expr_range: TextRange, //@temp? used for array size overflow error
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(ctx, ty, false);

    //@review source range for this type_size error 10.05.24
    let kind = match constant::type_layout(ctx, ty, ctx.src(expr_range)) {
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

fn typecheck_item<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    args_list: Option<&ast::ArgumentList<'ast>>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let (item_res, fields) = match check_path::path_resolve_value(ctx, path) {
        ValueID::None => {
            args_list.map(|al| default_check_arg_list(ctx, al));
            return TypeResult::error();
        }
        ValueID::Proc(proc_id) => {
            if let Some(args_list) = args_list {
                return check_call_direct(ctx, proc_id, args_list);
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
            return check_variant_input_opt(ctx, enum_id, variant_id, args_list, expr_range);
        }
        ValueID::Const(id, fields) => (
            TypeResult::new(
                ctx.registry.const_data(id).ty,
                hir::ExprKind::ConstVar { const_id: id },
            ),
            fields,
        ),
        ValueID::Global(id, fields) => (
            TypeResult::new(
                ctx.registry.global_data(id).ty,
                hir::ExprKind::GlobalVar { global_id: id },
            ),
            fields,
        ),
        ValueID::Param(id, fields) => (
            TypeResult::new(
                ctx.scope.local.param(id).ty,
                hir::ExprKind::ParamVar { param_id: id },
            ),
            fields,
        ),
        ValueID::Local(id, fields) => (
            TypeResult::new(
                ctx.scope.local.local(id).ty,
                hir::ExprKind::LocalVar { local_id: id },
            ),
            fields,
        ),
        ValueID::LocalBind(id, fields) => (
            TypeResult::new(
                ctx.scope.local.bind(id).ty,
                hir::ExprKind::LocalBind { local_bind_id: id },
            ),
            fields,
        ),
        ValueID::ForBind(id, fields) => (
            TypeResult::new(
                ctx.scope.local.for_bind(id).ty,
                hir::ExprKind::ForBind { for_bind_id: id },
            ),
            fields,
        ),
    };

    let mut target_res = item_res;
    let mut target_range = expr_range;

    for &name in fields {
        let expr_res = target_res.into_expr_result(ctx, target_range);
        let field_result = check_field_from_type(ctx, name.name, expr_res.ty);
        target_res = emit_field_expr(expr_res.expr, field_result);
        target_range = TextRange::new(expr_range.start(), name.name.range.end());
    }

    if let Some(args_list) = args_list {
        let expr_res = target_res.into_expr_result(ctx, target_range);
        check_call_indirect(ctx, expr_res, args_list)
    } else {
        target_res
    }
}

fn typecheck_variant<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    args_list: Option<&ast::ArgumentList<'ast>>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let name_src = ctx.src(name.range);
    let enum_id = match infer_enum_type(ctx, expect, name_src) {
        Some(found) => found,
        None => {
            args_list.map(|al| default_check_arg_list(ctx, al));
            return TypeResult::error();
        }
    };
    let variant_id = match scope::check_find_enum_variant(ctx, enum_id, name) {
        Some(found) => found,
        None => {
            args_list.map(|al| default_check_arg_list(ctx, al));
            return TypeResult::error();
        }
    };

    check_variant_input_opt(ctx, enum_id, variant_id, args_list, expr_range)
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

    let struct_id = match struct_init.path {
        Some(path) => check_path::path_resolve_struct(ctx, path),
        None => {
            let src = ctx.src(error_range(struct_init, expr_range));
            infer_struct_type(ctx, expect, src)
        }
    };
    let struct_id = match struct_id {
        Some(found) => found,
        None => {
            default_check_field_init_list(ctx, struct_init.input);
            return TypeResult::error();
        }
    };

    let data = ctx.registry.struct_data(struct_id);
    let struct_origin_id = data.origin_id;
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

    let error_count = ctx.emit.error_count();
    let mut out_of_order_init = None;

    for (idx, input) in struct_init.input.iter().enumerate() {
        let field_id = match scope::check_find_struct_field(ctx, struct_id, input.name) {
            Some(found) => found,
            None => {
                let _ = typecheck_expr(
                    ctx,
                    Expectation::HasType(hir::Type::Error, None),
                    input.expr,
                );
                continue;
            }
        };

        if idx != field_id.index() && out_of_order_init.is_none() {
            out_of_order_init = Some(idx)
        }

        let data = ctx.registry.struct_data(struct_id);
        let field = data.field(field_id);

        let expect_src = SourceRange::new(struct_origin_id, field.ty_range);
        let expect = Expectation::HasType(field.ty, Some(expect_src));
        let input_res = typecheck_expr(ctx, expect, input.expr);

        if let FieldStatus::Init(range) = field_status[field_id.index()] {
            let src = ctx.src(input.name.range);
            let prev_src = ctx.src(range);
            let field_name = ctx.name(input.name.id);
            err::tycheck_field_already_initialized(&mut ctx.emit, src, prev_src, field_name);
        } else {
            if ctx.scope.origin() != struct_origin_id && field.vis == ast::Vis::Private {
                let src = ctx.src(input.name.range);
                let field_name = ctx.name(input.name.id);
                let field_src = SourceRange::new(struct_origin_id, field.name.range);
                err::tycheck_field_is_private(&mut ctx.emit, src, field_name, field_src);
            }

            let field_init = hir::FieldInit {
                field_id,
                expr: input_res.expr,
            };
            field_inits.push(field_init);
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

    let input = ctx.arena.alloc_slice(&field_inits);
    let kind = hir::ExprKind::StructInit { struct_id, input };
    //@ignored poly_types
    TypeResult::new(hir::Type::Struct(struct_id, &[]), kind)
}

fn typecheck_array_init<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    input: &[&ast::Expr<'ast>],
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let mut expect = match expect {
        Expectation::None => Expectation::None,
        Expectation::HasType(expect_ty, expect_src) => match expect_ty {
            hir::Type::Error => Expectation::HasType(hir::Type::Error, None),
            hir::Type::ArrayStatic(array) => Expectation::HasType(array.elem_ty, expect_src),
            _ => Expectation::None,
        },
    };

    let mut elem_ty = None;
    let mut did_error = false;
    let error_count = ctx.emit.error_count();

    let mut input_res = Vec::with_capacity(input.len());
    for expr in input.iter().copied() {
        let expr_res = typecheck_expr(ctx, expect, expr);
        input_res.push(expr_res.expr);
        did_error = ctx.emit.did_error(error_count);

        // stop expecting when errored
        if did_error {
            expect = Expectation::None; //@expect error type?
        }
        // elem_ty is first non-error type
        if elem_ty.is_none() && !expr_res.ty.is_error() {
            elem_ty = Some(expr_res.ty);

            // update expect with first non-error type
            if matches!(expect, Expectation::None) && !did_error {
                let expect_src = ctx.src(expr.range);
                expect = Expectation::HasType(expr_res.ty, Some(expect_src));
            }
        }
    }
    let input = ctx.arena.alloc_slice(&input_res);

    if elem_ty.is_none() {
        match expect {
            Expectation::None => {}
            Expectation::HasType(expect_ty, _) => {
                elem_ty = Some(expect_ty);
            }
        }
    }

    //@should return error? or result with type and Expr::Error?
    if did_error {
        TypeResult::error()
    } else if let Some(elem_ty) = elem_ty {
        let len = hir::ArrayStaticLen::Immediate(input.len() as u64);
        let array_ty = hir::ArrayStatic { len, elem_ty };
        let array_ty = ctx.arena.alloc(array_ty);

        let array_init = hir::ArrayInit { elem_ty, input };
        let array_init = ctx.arena.alloc(array_init);
        let kind = hir::ExprKind::ArrayInit { array_init };
        TypeResult::new(hir::Type::ArrayStatic(array_ty), kind)
    } else if input.is_empty() {
        let src = ctx.src(expr_range);
        err::tycheck_cannot_infer_empty_array(&mut ctx.emit, src);
        TypeResult::error()
    } else {
        TypeResult::error()
    }
}

fn typecheck_array_repeat<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    mut expect: Expectation<'hir>,
    value: &ast::Expr<'ast>,
    len: ast::ConstExpr<'ast>,
) -> TypeResult<'hir> {
    expect = match expect {
        Expectation::None => Expectation::None,
        Expectation::HasType(expect_ty, expect_src) => match expect_ty {
            hir::Type::Error => Expectation::HasType(hir::Type::Error, None),
            hir::Type::ArrayStatic(array) => Expectation::HasType(array.elem_ty, expect_src),
            _ => Expectation::None,
        },
    };

    let expr_res = typecheck_expr(ctx, expect, value);

    //@this is duplicated here and in pass_3::type_resolve 09.05.24
    let value =
        constant::resolve_const_expr(ctx, Expectation::HasType(hir::Type::USIZE, None), len);
    let len = match value {
        Ok(hir::ConstValue::Int { val, .. }) => Some(val),
        _ => None,
    };

    if let Some(len) = len {
        let array_type = ctx.arena.alloc(hir::ArrayStatic {
            len: hir::ArrayStaticLen::Immediate(len),
            elem_ty: expr_res.ty,
        });
        let array_repeat = ctx.arena.alloc(hir::ArrayRepeat {
            elem_ty: expr_res.ty,
            value: expr_res.expr,
            len,
        });
        let kind = hir::ExprKind::ArrayRepeat { array_repeat };
        TypeResult::new(hir::Type::ArrayStatic(array_type), kind)
    } else {
        TypeResult::error()
    }
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
            let kind = hir::ExprKind::Deref {
                rhs: rhs_res.expr,
                mutt,
                ref_ty,
            };
            TypeResult::new(*ref_ty, kind)
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
    let kind = hir::ExprKind::Address { rhs: rhs_res.expr };
    TypeResult::new(ref_ty, kind)
}

fn typecheck_range<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    range: &ast::Range<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let struct_name = match range {
        ast::Range::Full => "RangeFull",
        ast::Range::ToExclusive(_) => "RangeTo",
        ast::Range::ToInclusive(_) => "RangeToInclusive",
        ast::Range::From(_) => "RangeFrom",
        ast::Range::Exclusive(_, _) => "Range",
        ast::Range::Inclusive(_, _) => "RangeInclusive",
    };

    //@improve facilities and error handling of getting things from core library
    let (struct_id, range_id) = match core_find_struct(ctx, "range", struct_name) {
        Some(value) => value,
        None => {
            let msg = format!("failed to locate struct `{struct_name}` in `core:range`");
            let src = ctx.src(expr_range);
            ctx.emit.error(Error::new(msg, src, None));
            return TypeResult::error();
        }
    };

    macro_rules! range_full {
        () => {{
            let kind = hir::ExprKind::StructInit {
                struct_id,
                input: &[],
            };
            TypeResult::new(hir::Type::Struct(struct_id, &[]), kind)
        }};
    }

    macro_rules! range_single {
        ($one:expr) => {{
            let one_src = ctx
                .registry
                .struct_data(struct_id)
                .fields
                .get(0)
                .map(|f| SourceRange::new(range_id, f.ty_range));
            let one_res =
                typecheck_expr(ctx, Expectation::HasType(hir::Type::USIZE, one_src), $one);

            let input = [hir::FieldInit {
                field_id: hir::FieldID::new(0),
                expr: one_res.expr,
            }];
            let kind = hir::ExprKind::StructInit {
                struct_id,
                input: ctx.arena.alloc_slice(&input),
            };
            TypeResult::new(hir::Type::Struct(struct_id, &[]), kind)
        }};
    }

    macro_rules! range_double {
        ($one:expr, $two:expr) => {{
            let one_src = ctx
                .registry
                .struct_data(struct_id)
                .fields
                .get(0)
                .map(|f| SourceRange::new(range_id, f.ty_range));
            let two_src = ctx
                .registry
                .struct_data(struct_id)
                .fields
                .get(1)
                .map(|f| SourceRange::new(range_id, f.ty_range));
            let one_res =
                typecheck_expr(ctx, Expectation::HasType(hir::Type::USIZE, one_src), $one);
            let two_res =
                typecheck_expr(ctx, Expectation::HasType(hir::Type::USIZE, two_src), $two);

            let input = [
                hir::FieldInit {
                    field_id: hir::FieldID::new(0),
                    expr: one_res.expr,
                },
                hir::FieldInit {
                    field_id: hir::FieldID::new(1),
                    expr: two_res.expr,
                },
            ];
            let kind = hir::ExprKind::StructInit {
                struct_id,
                input: ctx.arena.alloc_slice(&input),
            };
            TypeResult::new(hir::Type::Struct(struct_id, &[]), kind)
        }};
    }

    match *range {
        ast::Range::Full => range_full!(),
        ast::Range::ToExclusive(end) => range_single!(end),
        ast::Range::ToInclusive(end) => range_single!(end),
        ast::Range::From(start) => range_single!(start),
        ast::Range::Exclusive(start, end) => range_double!(start, end),
        ast::Range::Inclusive(start, end) => range_double!(start, end),
    }
}

fn core_find_struct<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    module_name: &'static str,
    struct_name: &'static str,
) -> Option<(hir::StructID, ModuleID)> {
    let module_name = ctx.session.intern_name.get_id(module_name)?;
    let struct_name = ctx.session.intern_name.get_id(struct_name)?;

    let core_package = ctx.session.graph.package(session::CORE_PACKAGE_ID);
    let target_id = match core_package.src().find(ctx.session, module_name) {
        session::ModuleOrDirectory::None => return None,
        session::ModuleOrDirectory::Module(module_id) => module_id,
        session::ModuleOrDirectory::Directory(_) => return None,
    };

    let struct_id = ctx.scope.global.find_defined_struct(target_id, struct_name);
    struct_id.map(|id| (id, target_id))
}

fn typecheck_unary<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    op: ast::UnOp,
    op_range: TextRange,
    rhs: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let rhs_expect = unary_rhs_expect(ctx, op, op_range, expect);
    let rhs_res = typecheck_expr(ctx, rhs_expect, rhs);
    let un_op = unary_op_check(ctx, op, op_range, rhs_res.ty);

    if let Some(un_op) = un_op {
        let unary_type = unary_output_type(op, rhs_res.ty);
        let kind = hir::ExprKind::Unary {
            op: un_op,
            rhs: rhs_res.expr,
        };
        TypeResult::new_ignore(unary_type, kind)
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
fn typecheck_binary<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    op: ast::BinOp,
    op_start: TextOffset,
    lhs: &ast::Expr<'ast>,
    rhs: &ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let op_offset = op.as_str().len() as u32;
    let op_range = TextRange::new(op_start, op_start + op_offset.into());

    let lhs_expect = binary_lhs_expect(ctx, op, op_range, expect);
    let lhs_res = typecheck_expr(ctx, lhs_expect, lhs);
    let bin_op = binary_op_check(ctx, op, op_range, lhs_res.ty);

    let expect_src = ctx.src(lhs.range);
    let rhs_expect = binary_rhs_expect(op, lhs_res.ty, bin_op.is_some(), expect_src);
    let rhs_res = typecheck_expr(ctx, rhs_expect, rhs);
    let _ = binary_op_check(ctx, op, op_range, rhs_res.ty);

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

fn typecheck_block<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    block: ast::Block<'ast>,
    status: BlockStatus,
) -> BlockResult<'hir> {
    ctx.scope.local.start_block(status);

    let mut block_stmts: Vec<hir::Stmt> = Vec::with_capacity(block.stmts.len());
    let mut block_ty: Option<hir::Type> = None;
    let mut tail_range: Option<TextRange> = None;

    for stmt in block.stmts.iter().copied() {
        let stmt = match stmt.kind {
            ast::StmtKind::AttrStmt(attr) => {
                let feedback = attr_check::check_attrs_stmt(ctx, attr.attrs);
                if feedback.cfg_state.disabled() {
                    continue;
                }
                attr.stmt
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
            //@defer block divergence should be simulated to correctly handle block_ty and divergence warnings 03.07.24
            ast::StmtKind::Defer(block) => {
                if let Some(stmt_res) = typecheck_defer(ctx, *block, stmt.range) {
                    check_stmt_diverges(ctx, false, stmt.range);
                    stmt_res
                } else {
                    continue;
                }
            }
            ast::StmtKind::Loop(loop_) => {
                //@can diverge (inf loop, return, panic)
                check_stmt_diverges(ctx, false, stmt.range);
                hir::Stmt::Loop(typecheck_loop(ctx, loop_))
            }
            ast::StmtKind::For(for_) => {
                if let Some(for_) = typecheck_for(ctx, for_) {
                    check_stmt_diverges(ctx, false, stmt.range);
                    hir::Stmt::For(for_)
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
                    LocalResult::Local(local_id) => hir::Stmt::Local(local_id),
                    LocalResult::Discard(value) => hir::Stmt::Discard(value),
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
                    | ast::ExprKind::Match { .. } => Expectation::HasType(hir::Type::VOID, None),
                    _ => Expectation::None,
                };

                let error_count = ctx.emit.error_count();
                let expr_res = typecheck_expr(ctx, expect, expr);
                if !ctx.emit.did_error(error_count) {
                    check_unused_expr_semi(ctx, expr_res.expr, expr.range);
                }

                //@migrate to using never type instead of diverges bool flags
                let will_diverge = expr_res.ty.is_never();
                check_stmt_diverges(ctx, will_diverge, stmt.range);
                hir::Stmt::ExprSemi(expr_res.expr)
            }
            ast::StmtKind::ExprTail(expr) => {
                // type expectation is delegated to tail expression, instead of the block itself
                let expr_res = typecheck_expr(ctx, expect, expr);
                let stmt_res = hir::Stmt::ExprTail(expr_res.expr);
                // @seems to fix the problem (still a hack)
                check_stmt_diverges(ctx, true, stmt.range);

                // only assigned once, any further `ExprTail` are unreachable
                if block_ty.is_none() {
                    block_ty = Some(expr_res.ty);
                    tail_range = Some(expr.range);
                }
                stmt_res
            }
            ast::StmtKind::AttrStmt(_) => unreachable!(),
        };

        match ctx.scope.local.diverges() {
            Diverges::Maybe | Diverges::Always(_) => block_stmts.push(stmt_res),
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
        let diverges = match ctx.scope.local.diverges() {
            Diverges::Maybe => false,
            Diverges::Always(_) => true,
            Diverges::AlwaysWarned => true,
        };
        //@change to last `}` range?
        // verify that all block are actual blocks in that case
        if !diverges {
            type_expectation_check(ctx, block.range, hir::Type::VOID, expect);
        }
        //@hack but should be correct
        let block_ty = if diverges {
            hir::Type::NEVER
        } else {
            hir::Type::VOID
        };
        BlockResult::new(block_ty, hir_block, tail_range)
    };

    ctx.scope.local.exit_block();
    block_result
}

pub fn check_stmt_diverges(ctx: &mut HirCtx, will_diverge: bool, stmt_range: TextRange) {
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

    let expect = ctx.scope.local.return_expect();
    let expr = match expr {
        Some(expr) => {
            let expr_res = typecheck_expr(ctx, expect, expr);
            Some(expr_res.expr)
        }
        None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
            type_expectation_check(ctx, kw_range, hir::Type::VOID, expect);
            None
        }
    };

    valid.then_some(hir::Stmt::Return(expr))
}

fn typecheck_defer<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    block: ast::Block<'ast>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
    let valid = if let Some(defer) = ctx.scope.local.find_prev_defer() {
        let src = ctx.src(kw_range);
        let defer_src = ctx.src(defer);
        err::tycheck_defer_in_defer(&mut ctx.emit, src, defer_src);
        false
    } else {
        true
    };

    let expect = Expectation::HasType(hir::Type::VOID, None);
    let block_res = typecheck_block(ctx, expect, block, BlockStatus::Defer(kw_range));

    if valid {
        let block = ctx.arena.alloc(block_res.block);
        Some(hir::Stmt::Defer(block))
    } else {
        None
    }
}

fn typecheck_loop<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    loop_: &ast::Loop<'ast>,
) -> &'hir hir::Loop<'hir> {
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
            ctx.scope.local.start_block(BlockStatus::None);
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
        BlockStatus::Loop,
    );

    if let ast::LoopKind::ForLoop { .. } = loop_.kind {
        ctx.scope.local.exit_block();
    }

    let loop_ = hir::Loop {
        kind,
        block: block_res.block,
    };
    ctx.arena.alloc(loop_)
}

fn typecheck_for<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    for_: &ast::For<'ast>,
) -> Option<&'hir hir::For<'hir>> {
    let kind = match for_.header {
        ast::ForHeader::Loop => hir::ForKind::Loop,
        ast::ForHeader::Cond(cond) => {
            let expect_bool = Expectation::HasType(hir::Type::BOOL, None);
            let expr_res = typecheck_expr(ctx, expect_bool, cond);
            hir::ForKind::Cond(expr_res.expr)
        }
        ast::ForHeader::Elem(header) => {
            let expr_res = typecheck_expr(ctx, Expectation::None, header.expr);

            //@ignore mutability for now
            let (collection, elem_kind) = match type_as_collection(expr_res.ty) {
                Ok(None) => return None,
                Ok(Some(collection)) => match collection.kind {
                    CollectionKind::Slice(_) => (collection, hir::ForElemKind::Slice),
                    CollectionKind::Array(array) => {
                        (collection, hir::ForElemKind::Array(array.len))
                    }
                    CollectionKind::Multi(_) => {
                        let src = ctx.src(expr_res.expr.range);
                        let ty_fmt = type_format(ctx, expr_res.ty);
                        err::tycheck_cannot_iter_on_type(&mut ctx.emit, src, ty_fmt.as_str());
                        return None;
                    }
                },
                Err(()) => {
                    let src = ctx.src(expr_res.expr.range);
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
            let value_bind = hir::ForBind {
                mutt: ast::Mut::Immutable,
                name: header.value,
                ty: value_ty,
            };
            let index_bind = hir::ForBind {
                mutt: ast::Mut::Immutable,
                name: header.index.unwrap_or(ast::Name {
                    id: NameID::dummy(),
                    range: TextRange::zero(),
                }),
                ty: hir::Type::USIZE,
            };

            ctx.scope.local.start_block(BlockStatus::None);
            let value_id = ctx.scope.local.add_for_bind(value_bind, true);
            let index_id = ctx
                .scope
                .local
                .add_for_bind(index_bind, header.index.is_some());

            let for_elem = hir::ForElem {
                value_id,
                index_id,
                deref: collection.deref.is_some(),
                by_pointer: header.ref_mut.is_some(),
                elem_ty: collection.elem_ty,
                kind: elem_kind,
                expr: expr_res.expr,
            };
            hir::ForKind::Elem(ctx.arena.alloc(for_elem))
        }
        ast::ForHeader::Pat(header) => {
            let on_res = typecheck_expr(ctx, Expectation::None, header.expr);
            let kind_res = super::match_check::match_kind(on_res.ty);

            //@duplicate code for pattern handling same as `match`
            if let Err(true) = kind_res {
                let src = ctx.src(on_res.expr.range);
                let ty_fmt = type_format(ctx, on_res.ty);
                err::tycheck_cannot_match_on_ty(&mut ctx.emit, src, ty_fmt.as_str());
            }

            let (pat_expect, ref_mut) = match kind_res {
                Ok(hir::MatchKind::Enum { enum_id, ref_mut }) => {
                    let expect_src = ctx.src(on_res.expr.range);
                    //@ignored poly_types
                    let enum_ty = hir::Type::Enum(enum_id, &[]);
                    (Expectation::HasType(enum_ty, Some(expect_src)), ref_mut)
                }
                Ok(_) => {
                    let expect_src = ctx.src(on_res.expr.range);
                    (Expectation::HasType(on_res.ty, Some(expect_src)), None)
                }
                Err(_) => (Expectation::HasType(hir::Type::Error, None), None),
            };

            ctx.scope.local.start_block(BlockStatus::None);
            let pat = typecheck_pat(ctx, pat_expect, &header.pat, ref_mut, false);

            let for_pat = hir::ForPat {
                pat,
                expr: on_res.expr,
            };
            hir::ForKind::Pat(ctx.arena.alloc(for_pat))
        }
    };

    let expect = Expectation::HasType(hir::Type::VOID, None);
    let block_res = typecheck_block(ctx, expect, for_.block, BlockStatus::Loop);

    // exit binding local scope
    match for_.header {
        ast::ForHeader::Loop => {}
        ast::ForHeader::Cond(_) => {}
        ast::ForHeader::Elem(_) => ctx.scope.local.exit_block(),
        ast::ForHeader::Pat(_) => ctx.scope.local.exit_block(),
    }

    let for_ = hir::For {
        kind,
        block: block_res.block,
    };
    Some(ctx.arena.alloc(for_))
}

enum LocalResult<'hir> {
    Error,
    Local(hir::LocalID),
    Discard(Option<&'hir hir::Expr<'hir>>),
}

fn typecheck_local<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    local: &ast::Local<'ast>,
) -> LocalResult<'hir> {
    let already_defined = match local.bind {
        ast::Binding::Named(_, name) => ctx
            .scope
            .check_already_defined(name, &ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err(),
        ast::Binding::Discard(_) => false,
    };

    let (mut local_ty, expect) = match local.ty.clone() {
        Some(ty) => {
            let local_ty = super::pass_3::type_resolve(ctx, ty, false);
            let expect_src = ctx.src(ty.range);
            (
                Some(local_ty),
                Expectation::HasType(local_ty, Some(expect_src)),
            )
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

            let local = hir::Local {
                mutt,
                name,
                ty,
                init,
                was_used: false,
            };
            let local_id = ctx.scope.local.add_local(local);
            LocalResult::Local(local_id)
        }
        //allowing discard locals to have no type
        //@emit a warning for useless variables? eg: let _ = zeroed;
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

    let assign_op = match assign.op {
        ast::AssignOp::Assign => hir::AssignOp::Assign,
        ast::AssignOp::Bin(op) => binary_op_check(ctx, op, assign.op_range, lhs_res.ty)
            .map(hir::AssignOp::Bin)
            .unwrap_or(hir::AssignOp::Assign),
    };

    let rhs_expect = Expectation::HasType(lhs_res.ty, Some(ctx.src(assign.lhs.range)));
    let rhs_res = typecheck_expr(ctx, rhs_expect, assign.rhs);

    let assign = hir::Assign {
        op: assign_op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_ty: lhs_res.ty,
    };
    ctx.arena.alloc(assign)
}

//==================== UNUSED ====================

fn check_unused_expr_semi(ctx: &mut HirCtx, expr: &hir::Expr, expr_range: TextRange) {
    enum UnusedExpr {
        No,
        Yes(&'static str),
    }

    fn unused_return_type(ty: hir::Type, kind: &'static str) -> UnusedExpr {
        match ty {
            hir::Type::Error => UnusedExpr::No,
            hir::Type::Basic(BasicType::Void) => UnusedExpr::No,
            hir::Type::Basic(BasicType::Never) => UnusedExpr::No,
            _ => UnusedExpr::Yes(kind),
        }
    }

    let unused = match expr.kind {
        hir::ExprKind::Error => UnusedExpr::No, // already errored
        hir::ExprKind::Const { .. } => UnusedExpr::Yes("constant value"),
        hir::ExprKind::If { .. } => UnusedExpr::No, // expected `void`
        hir::ExprKind::Block { .. } => UnusedExpr::No, // expected `void`
        hir::ExprKind::Match { .. } => UnusedExpr::No, // expected `void`
        hir::ExprKind::StructField { .. } => UnusedExpr::Yes("field access"),
        hir::ExprKind::SliceField { .. } => UnusedExpr::Yes("field access"),
        hir::ExprKind::Index { .. } => UnusedExpr::Yes("index access"),
        hir::ExprKind::Slice { .. } => UnusedExpr::Yes("slice value"),
        hir::ExprKind::Cast { .. } => UnusedExpr::Yes("cast value"),
        hir::ExprKind::ParamVar { .. } => UnusedExpr::Yes("parameter value"),
        hir::ExprKind::LocalVar { .. } => UnusedExpr::Yes("local value"),
        hir::ExprKind::LocalBind { .. } => UnusedExpr::Yes("local binding value"),
        hir::ExprKind::ForBind { .. } => UnusedExpr::Yes("for loop binding value"), //@change naming everywhere
        hir::ExprKind::ConstVar { .. } => UnusedExpr::Yes("constant value"),
        hir::ExprKind::GlobalVar { .. } => UnusedExpr::Yes("global value"),
        hir::ExprKind::Variant { .. } => UnusedExpr::Yes("variant value"),
        hir::ExprKind::CallDirect { proc_id, .. } => {
            let ty = ctx.registry.proc_data(proc_id).return_ty;
            unused_return_type(ty, "procedure return value")
        }
        hir::ExprKind::CallIndirect { indirect, .. } => {
            let ty = indirect.proc_ty.return_ty;
            unused_return_type(ty, "indirect call return value")
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
        let src = ctx.src(expr_range);
        err::tycheck_unused_expr(&mut ctx.emit, src, kind);
    }
}

//==================== INFER ====================

fn infer_int_type(expect: Expectation) -> BasicInt {
    const DEFAULT_INT_TYPE: BasicInt = BasicInt::S32;
    match expect {
        Expectation::None => DEFAULT_INT_TYPE,
        Expectation::HasType(expect_ty, _) => match expect_ty {
            hir::Type::Basic(basic) => BasicInt::from_basic(basic).unwrap_or(DEFAULT_INT_TYPE),
            _ => DEFAULT_INT_TYPE,
        },
    }
}

fn infer_float_type(expect: Expectation) -> BasicFloat {
    const DEFAULT_FLOAT_TYPE: BasicFloat = BasicFloat::F64;
    match expect {
        Expectation::None => DEFAULT_FLOAT_TYPE,
        Expectation::HasType(expect_ty, _) => match expect_ty {
            hir::Type::Basic(basic) => BasicFloat::from_basic(basic).unwrap_or(DEFAULT_FLOAT_TYPE),
            _ => DEFAULT_FLOAT_TYPE,
        },
    }
}

fn infer_enum_type(
    ctx: &mut HirCtx,
    expect: Expectation,
    error_src: SourceRange,
) -> Option<hir::EnumID> {
    let enum_id = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            //@ignored poly_types
            hir::Type::Enum(enum_id, _) => Some(enum_id),
            hir::Type::Reference(_, ref_ty) => match *ref_ty {
                //@ignored poly_types
                hir::Type::Enum(enum_id, _) => Some(enum_id),
                _ => None,
            },
            _ => None,
        },
    };
    if enum_id.is_none() {
        err::tycheck_cannot_infer_enum_type(&mut ctx.emit, error_src);
    }
    enum_id
}

fn infer_struct_type(
    ctx: &mut HirCtx,
    expect: Expectation,
    error_src: SourceRange,
) -> Option<hir::StructID> {
    let struct_id = match expect {
        Expectation::None => None,
        Expectation::HasType(ty, _) => match ty {
            hir::Type::Error => return None,
            //@ignored poly_types
            hir::Type::Struct(struct_id, _) => Some(struct_id),
            _ => None,
        },
    };
    if struct_id.is_none() {
        err::tycheck_cannot_infer_struct_type(&mut ctx.emit, error_src);
    }
    struct_id
}

//==================== DEFAULT CHECK ====================

fn default_check_arg_list<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    arg_list: &ast::ArgumentList<'ast>,
) {
    for &expr in arg_list.exprs.iter() {
        let _ = typecheck_expr(ctx, Expectation::HasType(hir::Type::Error, None), expr);
    }
}

fn default_check_field_init_list<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    input: &[ast::FieldInit<'ast>],
) {
    for field in input.iter() {
        let _ = typecheck_expr(
            ctx,
            Expectation::HasType(hir::Type::Error, None),
            field.expr,
        );
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
    proc_id: hir::ProcID,
    arg_list: &ast::ArgumentList<'ast>,
) -> TypeResult<'hir> {
    let data = ctx.registry.proc_data(proc_id);
    let return_ty = data.return_ty;

    let proc_src = Some(data.src());
    let expected_count = data.params.len();
    let is_variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
    check_call_arg_count(ctx, arg_list, proc_src, expected_count, is_variadic);

    let mut values = Vec::with_capacity(expected_count);
    for (idx, expr) in arg_list.exprs.iter().copied().enumerate() {
        let data = ctx.registry.proc_data(proc_id);
        let expect = match data.params.get(idx) {
            Some(param) => {
                let expect_src = SourceRange::new(data.origin_id, param.ty_range);
                Expectation::HasType(param.ty, Some(expect_src))
            }
            None => Expectation::HasType(hir::Type::Error, None),
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

fn check_call_indirect<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
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
            let src = ctx.src(target_res.expr.range);
            let ty_fmt = type_format(ctx, target_res.ty);
            err::tycheck_cannot_call_value_of_type(&mut ctx.emit, src, ty_fmt.as_str());
            default_check_arg_list(ctx, arg_list);
            return TypeResult::error();
        }
    };

    let proc_src = None;
    let expected_count = proc_ty.param_types.len();
    let is_variadic = proc_ty.is_variadic;
    check_call_arg_count(ctx, arg_list, proc_src, expected_count, is_variadic);

    let mut values = Vec::with_capacity(expected_count);
    for (idx, expr) in arg_list.exprs.iter().copied().enumerate() {
        let expect = match proc_ty.param_types.get(idx) {
            Some(param_ty) => Expectation::HasType(*param_ty, None),
            None => Expectation::HasType(hir::Type::Error, None),
        };
        let expr_res = typecheck_expr(ctx, expect, expr);
        values.push(expr_res.expr);
    }
    let values = ctx.arena.alloc_slice(&values);

    let indirect = hir::CallIndirect {
        proc_ty,
        input: values,
    };
    let kind = hir::ExprKind::CallIndirect {
        target: target_res.expr,
        indirect: ctx.arena.alloc(indirect),
    };
    TypeResult::new(proc_ty.return_ty, kind)
}

fn check_call_arg_count(
    ctx: &mut HirCtx,
    arg_list: &ast::ArgumentList,
    proc_src: Option<SourceRange>,
    expected_count: usize,
    is_variadic: bool,
) {
    let input_count = arg_list.exprs.len();
    let wrong_count = if is_variadic {
        input_count < expected_count
    } else {
        input_count != expected_count
    };

    if wrong_count {
        let src = ctx.src(arg_list_range(arg_list));
        err::tycheck_unexpected_proc_arg_count(
            &mut ctx.emit,
            src,
            proc_src,
            is_variadic,
            input_count,
            expected_count,
        );
    }
}

fn check_variant_input_opt<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    arg_list: Option<&ast::ArgumentList<'ast>>,
    error_range: TextRange,
) -> TypeResult<'hir> {
    let enum_data = ctx.registry.enum_data(enum_id);
    let variant = enum_data.variant(variant_id);
    let origin_id = enum_data.origin_id;

    let input_count = arg_list.map(|arg_list| arg_list.exprs.len()).unwrap_or(0);
    let expected_count = variant.fields.len();

    if expected_count == 0 && arg_list.is_some() {
        let src = ctx.src(arg_list_opt_range(arg_list, error_range));
        let variant_src = SourceRange::new(origin_id, variant.name.range);
        err::tycheck_unexpected_variant_arg_list(&mut ctx.emit, src, variant_src);
    } else if input_count != expected_count {
        let src = ctx.src(arg_list_opt_range(arg_list, error_range));
        let variant_src = SourceRange::new(origin_id, variant.name.range);
        err::tycheck_unexpected_variant_arg_count(
            &mut ctx.emit,
            src,
            variant_src,
            input_count,
            expected_count,
        );
    }

    let input = if let Some(arg_list) = arg_list {
        let mut values = Vec::with_capacity(arg_list.exprs.len());
        for (idx, &expr) in arg_list.exprs.iter().enumerate() {
            let expect = match variant.fields.get(idx) {
                Some(field) => {
                    let expect_src = SourceRange::new(origin_id, field.ty_range);
                    Expectation::HasType(field.ty, Some(expect_src))
                }
                None => Expectation::HasType(hir::Type::Error, None),
            };
            let expr_res = typecheck_expr(ctx, expect, expr);
            values.push(expr_res.expr);
        }
        let values = ctx.arena.alloc_slice(&values);
        ctx.arena.alloc(values)
    } else {
        let empty: &[&hir::Expr] = &[];
        ctx.arena.alloc(empty)
    };

    let kind = hir::ExprKind::Variant {
        enum_id,
        variant_id,
        input,
    };
    //@ignored poly_types
    TypeResult::new(hir::Type::Enum(enum_id, &[]), kind)
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
    bind_list: Option<&ast::BindingList>,
    variant: Option<&hir::Variant<'hir>>,
    ref_mut: Option<ast::Mut>,
    in_or_pat: bool,
) -> &'hir [hir::LocalBindID] {
    let bind_list = match bind_list {
        Some(bind_list) => bind_list,
        None => return &[],
    };

    if in_or_pat {
        if bind_list
            .binds
            .iter()
            .any(|bind| matches!(bind, ast::Binding::Named(_, _)))
        {
            let src = ctx.src(bind_list.range);
            err::tycheck_pat_in_or_bindings(&mut ctx.emit, src);
            return &[];
        }
    }

    let expected_count = if let Some(variant) = variant {
        variant.fields.len()
    } else {
        bind_list.binds.len()
    };
    let mut bind_ids = Vec::with_capacity(expected_count);

    for (idx, bind) in bind_list.binds.iter().enumerate() {
        let (mutt, name) = match *bind {
            ast::Binding::Named(mutt, name) => (mutt, name),
            ast::Binding::Discard(_) => continue,
        };

        let (ty, field_id) = if let Some(variant) = variant {
            if let Some(field_id) = variant.field_id(idx) {
                let field = variant.field(field_id);

                let ty = match ref_mut {
                    Some(ref_mut) => {
                        if field.ty.is_error() {
                            hir::Type::Error
                        } else {
                            hir::Type::Reference(ref_mut, &field.ty)
                        }
                    }
                    None => field.ty,
                };

                (ty, Some(field_id))
            } else {
                (hir::Type::Error, None)
            }
        } else {
            (hir::Type::Error, None)
        };

        if ctx
            .scope
            .check_already_defined(name, ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err()
        {
            continue;
        }

        let local_bind = hir::LocalBind {
            mutt,
            name,
            ty,
            field_id,
            was_used: false,
        };
        let bind_id = ctx.scope.local.add_bind(local_bind);
        bind_ids.push(bind_id);
    }

    ctx.arena.alloc_slice(&bind_ids)
}

//==================== OPERATOR ====================

fn unary_rhs_expect<'hir>(
    ctx: &HirCtx,
    op: ast::UnOp,
    op_range: TextRange,
    expect: Expectation<'hir>,
) -> Expectation<'hir> {
    match op {
        ast::UnOp::Neg | ast::UnOp::BitNot => expect,
        ast::UnOp::LogicNot => {
            let expect_src = ctx.src(op_range);
            Expectation::HasType(hir::Type::BOOL, Some(expect_src))
        }
    }
}

fn unary_output_type(op: ast::UnOp, rhs_ty: hir::Type) -> hir::Type {
    match op {
        ast::UnOp::Neg | ast::UnOp::BitNot => rhs_ty,
        ast::UnOp::LogicNot => hir::Type::BOOL,
    }
}

//@make error better (reference the expr)
fn unary_op_check(
    ctx: &mut HirCtx,
    op: ast::UnOp,
    op_range: TextRange,
    rhs_ty: hir::Type,
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
        let src = ctx.src(op_range);
        let rhs_ty = type_format(ctx, rhs_ty);
        err::tycheck_un_op_cannot_apply(&mut ctx.emit, src, op.as_str(), rhs_ty.as_str());
    }
    un_op
}

fn binary_lhs_expect<'hir>(
    ctx: &HirCtx,
    op: ast::BinOp,
    op_range: TextRange,
    expect: Expectation<'hir>,
) -> Expectation<'hir> {
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
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => {
            let expect_src = ctx.src(op_range);
            Expectation::HasType(hir::Type::BOOL, Some(expect_src))
        }
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

//@make error better (reference the expr)
fn binary_op_check(
    ctx: &mut HirCtx,
    op: ast::BinOp,
    op_range: TextRange,
    lhs_ty: hir::Type,
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
                BasicTypeKind::Float => Some(hir::BinOp::IsEq_Float),
                _ => None,
            },
            hir::Type::Enum(enum_id, _) => {
                let enum_data = ctx.registry.enum_data(enum_id);
                if enum_data.attr_set.contains(hir::EnumFlag::WithFields) {
                    None
                } else {
                    Some(hir::BinOp::IsEq_Int)
                }
            }
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
            hir::Type::Enum(enum_id, _) => {
                let enum_data = ctx.registry.enum_data(enum_id);
                if enum_data.attr_set.contains(hir::EnumFlag::WithFields) {
                    None
                } else {
                    Some(hir::BinOp::NotEq_Int)
                }
            }
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

    //@have specific error for enums with fields?
    if bin_op.is_none() {
        let src = ctx.src(op_range);
        let lhs_ty = type_format(ctx, lhs_ty);
        err::tycheck_bin_op_cannot_apply(&mut ctx.emit, src, op.as_str(), lhs_ty.as_str());
    }
    bin_op
}

//==================== ADDRESSABILITY ====================
// Addressability allows us to check whether expression
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
    ImmutRef(SourceRange),
    ImmutMulti(SourceRange),
    ImmutSlice(SourceRange),
}

impl AddrConstraint {
    #[inline]
    fn set(&mut self, new: AddrConstraint) {
        if matches!(self, AddrConstraint::None) {
            *self = new;
        }
    }
}

//@for index access store brackets range?
// unclear which access is the issue since entire index expr range is used
// same bracket range is needed for typecheck_index errors + backend maybe
fn resolve_expr_addressability(ctx: &HirCtx, expr: &hir::Expr) -> AddrResult {
    let mut expr = expr;
    let mut constraint = AddrConstraint::None;

    loop {
        let base = match expr.kind {
            // addr_base: simple
            hir::ExprKind::Error => AddrBase::Unknown,
            hir::ExprKind::Const { value } => match value {
                hir::ConstValue::Variant { .. } => AddrBase::TemporaryImmut,
                hir::ConstValue::Struct { .. } => AddrBase::TemporaryImmut,
                hir::ConstValue::Array { .. } => AddrBase::TemporaryImmut,
                hir::ConstValue::ArrayRepeat { .. } => AddrBase::TemporaryImmut,
                _ => AddrBase::Temporary,
            },
            hir::ExprKind::If { .. } => AddrBase::Temporary,
            hir::ExprKind::Block { .. } => AddrBase::Temporary,
            hir::ExprKind::Match { .. } => AddrBase::Temporary,
            hir::ExprKind::SliceField { .. } => AddrBase::SliceField,
            hir::ExprKind::Cast { .. } => AddrBase::Temporary,
            hir::ExprKind::Variant { .. } => AddrBase::TemporaryImmut,
            hir::ExprKind::CallDirect { .. } => AddrBase::Temporary,
            hir::ExprKind::CallIndirect { .. } => AddrBase::Temporary,
            hir::ExprKind::StructInit { .. } => AddrBase::TemporaryImmut,
            hir::ExprKind::ArrayInit { .. } => AddrBase::TemporaryImmut,
            hir::ExprKind::ArrayRepeat { .. } => AddrBase::TemporaryImmut,
            hir::ExprKind::Address { .. } => AddrBase::Temporary,
            hir::ExprKind::Unary { .. } => AddrBase::Temporary,
            hir::ExprKind::Binary { .. } => AddrBase::Temporary,
            // addr_base: variable
            hir::ExprKind::ParamVar { param_id } => {
                let param = ctx.scope.local.param(param_id);
                let src = ctx.src(param.name.range);
                AddrBase::Variable(param.mutt, src)
            }
            hir::ExprKind::LocalVar { local_id } => {
                let local = ctx.scope.local.local(local_id);
                let src = ctx.src(local.name.range);
                AddrBase::Variable(local.mutt, src)
            }
            hir::ExprKind::LocalBind { local_bind_id } => {
                let local_bind = ctx.scope.local.bind(local_bind_id);
                let src = ctx.src(local_bind.name.range);
                AddrBase::Variable(local_bind.mutt, src)
            }
            hir::ExprKind::ForBind { for_bind_id } => {
                let for_bind = ctx.scope.local.for_bind(for_bind_id);
                let src = ctx.src(for_bind.name.range);
                AddrBase::Variable(for_bind.mutt, src)
            }
            hir::ExprKind::ConstVar { const_id } => {
                let const_data = ctx.registry.const_data(const_id);
                AddrBase::Constant(const_data.src())
            }
            hir::ExprKind::GlobalVar { global_id } => {
                let global_data = ctx.registry.global_data(global_id);
                AddrBase::Variable(global_data.mutt, global_data.src())
            }
            // access chains before addr_base
            hir::ExprKind::StructField { target, access } => {
                match access.deref {
                    Some(ast::Mut::Mutable) => constraint.set(AddrConstraint::AllowMut),
                    Some(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutRef(ctx.src(expr.range)))
                    }
                    None => {}
                }
                expr = target;
                continue;
            }
            hir::ExprKind::Index { target, access } => {
                match access.deref {
                    Some(ast::Mut::Mutable) => constraint.set(AddrConstraint::AllowMut),
                    Some(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutRef(ctx.src(expr.range)))
                    }
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
                        constraint.set(AddrConstraint::ImmutMulti(ctx.src(expr.range)))
                    }
                    hir::IndexKind::Slice(ast::Mut::Immutable) => {
                        constraint.set(AddrConstraint::ImmutSlice(ctx.src(expr.range)))
                    }
                    hir::IndexKind::Array(_) => {}
                }
                expr = target;
                continue;
            }
            hir::ExprKind::Slice { .. } => {
                unimplemented!("slice expression addressability");
            }
            hir::ExprKind::Deref { rhs, mutt, .. } => {
                match mutt {
                    ast::Mut::Mutable => constraint.set(AddrConstraint::AllowMut),
                    ast::Mut::Immutable => {
                        constraint.set(AddrConstraint::ImmutRef(ctx.src(expr.range)))
                    }
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
    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => {
            err::tycheck_cannot_ref_slice_field(&mut ctx.emit, src);
        }
        AddrBase::Temporary => {
            err::tycheck_cannot_ref_temporary(&mut ctx.emit, src);
        }
        AddrBase::TemporaryImmut => {
            if mutt == ast::Mut::Mutable {
                err::tycheck_cannot_ref_temporary_immut(&mut ctx.emit, src);
            }
        }
        AddrBase::Constant(const_src) => {
            err::tycheck_cannot_ref_constant(&mut ctx.emit, src, const_src)
        }
        AddrBase::Variable(var_mutt, var_src) => {
            if mutt == ast::Mut::Immutable {
                return;
            }
            match addr_res.constraint {
                AddrConstraint::None => {
                    if var_mutt == ast::Mut::Immutable {
                        err::tycheck_cannot_ref_var_immut(&mut ctx.emit, src, var_src);
                    }
                }
                AddrConstraint::AllowMut => {}
                AddrConstraint::ImmutRef(deref_src) => {
                    err::tycheck_cannot_ref_val_behind_ref(&mut ctx.emit, src, deref_src);
                }
                AddrConstraint::ImmutMulti(multi_src) => {
                    err::tycheck_cannot_ref_val_behind_multi_ref(&mut ctx.emit, src, multi_src);
                }
                AddrConstraint::ImmutSlice(slice_src) => {
                    err::tycheck_cannot_ref_val_behind_slice(&mut ctx.emit, src, slice_src);
                }
            }
        }
    }
}

fn check_assign_addressability(ctx: &mut HirCtx, addr_res: &AddrResult, expr_range: TextRange) {
    let src = ctx.src(expr_range);
    match addr_res.base {
        AddrBase::Unknown => {}
        AddrBase::SliceField => {
            err::tycheck_cannot_assign_slice_field(&mut ctx.emit, src);
        }
        AddrBase::Temporary | AddrBase::TemporaryImmut => {
            err::tycheck_cannot_assign_temporary(&mut ctx.emit, src)
        }
        AddrBase::Constant(const_src) => {
            err::tycheck_cannot_assign_constant(&mut ctx.emit, src, const_src);
        }
        AddrBase::Variable(var_mutt, var_src) => match addr_res.constraint {
            AddrConstraint::None => {
                if var_mutt == ast::Mut::Immutable {
                    err::tycheck_cannot_assign_var_immut(&mut ctx.emit, src, var_src);
                }
            }
            AddrConstraint::AllowMut => {}
            AddrConstraint::ImmutRef(deref_src) => {
                err::tycheck_cannot_assign_val_behind_ref(&mut ctx.emit, src, deref_src);
            }
            AddrConstraint::ImmutMulti(multi_src) => {
                err::tycheck_cannot_assign_val_behind_multi_ref(&mut ctx.emit, src, multi_src);
            }
            AddrConstraint::ImmutSlice(slice_src) => {
                err::tycheck_cannot_assign_val_behind_slice(&mut ctx.emit, src, slice_src);
            }
        },
    }
}

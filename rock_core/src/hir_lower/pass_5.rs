use super::hir_build::{self, HirData, HirEmit, SymbolKind};
use super::proc_scope::{BlockEnter, DeferStatus, LoopStatus, ProcScope, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, Info, SourceRange, StringOrStr, WarningComp};
use crate::hir::{self, BasicFloat, BasicInt};
use crate::session::ModuleID;
use crate::text::TextRange;

pub fn typecheck_procedures<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for proc_id in hir.registry().proc_ids() {
        typecheck_proc(hir, emit, proc_id)
    }
}

fn typecheck_proc<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc_id: hir::ProcID,
) {
    let item = hir.registry().proc_item(proc_id);
    let data = hir.registry().proc_data(proc_id);

    if data.attr_set.contains(hir::ProcFlag::Variadic) {
        if data.params.is_empty() {
            emit.error(ErrorComp::new(
                "variadic procedures must have at least one named parameter",
                SourceRange::new(data.origin_id, data.name.range),
                None,
            ));
        }
    }

    if data.attr_set.contains(hir::ProcFlag::Test) {
        if !data.params.is_empty() {
            emit.error(ErrorComp::new(
                "procedures with #[test] attribute cannot have any input parameters",
                SourceRange::new(data.origin_id, data.name.range),
                None,
            ));
        }
        if !data.return_ty.is_void() {
            if let Some(return_ty) = item.return_ty {
                emit.error(ErrorComp::new(
                    "procedures with #[test] attribute can only return `void`",
                    SourceRange::new(data.origin_id, return_ty.range),
                    None,
                ));
            }
        }
    }

    if let Some(block) = item.block {
        let expect_src = match item.return_ty {
            Some(return_ty) => SourceRange::new(data.origin_id, return_ty.range),
            None => SourceRange::new(data.origin_id, data.name.range),
        };
        let expect = Expectation::HasType(data.return_ty, Some(expect_src));

        let mut proc = ProcScope::new(data, expect);
        let block_res = typecheck_block(hir, emit, &mut proc, expect, block, BlockEnter::None);
        let locals = emit.arena.alloc_slice(proc.finish_locals());

        let data = hir.registry_mut().proc_data_mut(proc_id);
        data.block = Some(block_res.block);
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
        (hir::Type::Error, _) => true,
        (_, hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (hir::Type::Enum(id), hir::Type::Enum(id2)) => id == id2,
        (hir::Type::Struct(id), hir::Type::Struct(id2)) => id == id2,
        (hir::Type::Reference(ref_ty, mutt), hir::Type::Reference(ref_ty2, mutt2)) => {
            if mutt2 == ast::Mut::Mutable {
                type_matches(hir, emit, *ref_ty, *ref_ty2)
            } else {
                mutt == mutt2 && type_matches(hir, emit, *ref_ty, *ref_ty2)
            }
        }
        (hir::Type::Procedure(proc_ty), hir::Type::Procedure(proc_ty2)) => {
            (proc_ty.param_types.len() == proc_ty2.param_types.len())
                && (proc_ty.is_variadic == proc_ty2.is_variadic)
                && type_matches(hir, emit, proc_ty.return_ty, proc_ty2.return_ty)
                && (0..proc_ty.param_types.len()).all(|idx| {
                    type_matches(
                        hir,
                        emit,
                        proc_ty.param_types[idx],
                        proc_ty2.param_types[idx],
                    )
                })
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            if slice2.mutt == ast::Mut::Mutable {
                type_matches(hir, emit, slice.elem_ty, slice2.elem_ty)
            } else {
                slice.mutt == slice2.mutt && type_matches(hir, emit, slice.elem_ty, slice2.elem_ty)
            }
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            if let Some(len) = array_static_len(hir, emit, array.len) {
                if let Some(len2) = array_static_len(hir, emit, array2.len) {
                    return (len == len2) && type_matches(hir, emit, array.elem_ty, array2.elem_ty);
                }
            }
            true
        }
        (hir::Type::ArrayStatic(array), _) => array_static_len(hir, emit, array.len).is_none(),
        (_, hir::Type::ArrayStatic(array2)) => array_static_len(hir, emit, array2.len).is_none(),
        _ => false,
    }
}

pub fn type_format<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &HirEmit<'hir>,
    ty: hir::Type<'hir>,
) -> StringOrStr {
    match ty {
        hir::Type::Error => "<unknown>".into(),
        hir::Type::Basic(basic) => basic.as_str().into(),
        hir::Type::Enum(id) => {
            let name = hir.name_str(hir.registry().enum_data(id).name.id);
            name.to_string().into()
        }
        hir::Type::Struct(id) => {
            let name = hir.name_str(hir.registry().struct_data(id).name.id);
            name.to_string().into()
        }
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            let ref_ty_format = type_format(hir, emit, *ref_ty);
            let format = format!("&{}{}", mut_str, ref_ty_format.as_str());
            format.into()
        }
        hir::Type::Procedure(proc_ty) => {
            let mut format = String::from("proc(");
            for (idx, param_ty) in proc_ty.param_types.iter().enumerate() {
                let param_ty_format = type_format(hir, emit, *param_ty);
                format.push_str(param_ty_format.as_str());
                if proc_ty.param_types.len() != idx + 1 {
                    format.push_str(", ");
                }
            }
            if proc_ty.is_variadic {
                format.push_str(", ..")
            }
            format.push_str(") -> ");
            let return_format = type_format(hir, emit, proc_ty.return_ty);
            format.push_str(return_format.as_str());
            format.into()
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            let elem_format = type_format(hir, emit, slice.elem_ty);
            let format = format!("[{}]{}", mut_str, elem_format.as_str());
            format.into()
        }
        hir::Type::ArrayStatic(array) => {
            let len = array_static_len(hir, emit, array.len);
            let elem_format = type_format(hir, emit, array.elem_ty);
            let format = match len {
                Some(len) => format!("[{}]{}", len, elem_format.as_str()),
                None => format!("[<unknown>]{}", elem_format.as_str()),
            };
            format.into()
        }
    }
}

fn array_static_len<'hir>(
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
                        hir::ConstValue::Int { val, neg, int_ty } => {
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

#[derive(Copy, Clone)]
pub enum Expectation<'hir> {
    None,
    HasType(hir::Type<'hir>, Option<SourceRange>),
}

//@verify how this works, `false` is good outcome?
pub fn check_type_expectation<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    from_range: TextRange,
    expect: Expectation<'hir>,
    found_ty: hir::Type<'hir>,
) -> bool {
    if let Expectation::HasType(expect_ty, expect_src) = expect {
        if type_matches(hir, emit, expect_ty, found_ty) {
            return false;
        }

        let info = if let Some(source) = expect_src {
            Info::new("expected due to this", source)
        } else {
            None
        };

        emit.error(ErrorComp::new(
            format!(
                "type mismatch: expected `{}`, found `{}`",
                type_format(hir, emit, expect_ty).as_str(),
                type_format(hir, emit, found_ty).as_str()
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
    pub expr: &'hir hir::Expr<'hir>,
    ignore: bool,
}

struct BlockResult<'hir> {
    ty: hir::Type<'hir>,
    block: hir::Block<'hir>,
    tail_range: Option<TextRange>,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: false,
        }
    }

    fn new_ignore(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: true,
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

    fn into_type_result(self, emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
        let block_expr = hir::Expr::Block { block: self.block };
        let block_expr = emit.arena.alloc(block_expr);
        TypeResult {
            ty: self.ty,
            expr: block_expr,
            ignore: true, //@ignoring with manual expectation check in blocks, might change 03.07.24
        }
    }
}

#[must_use]
pub fn typecheck_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    expr: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let expr_res = match expr.kind {
        ast::ExprKind::Lit(literal) => typecheck_lit(emit, expect, literal),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            typecheck_block(hir, emit, proc, expect, *block, BlockEnter::None)
                .into_type_result(emit)
        }
        ast::ExprKind::Match { match_ } => {
            typecheck_match(hir, emit, proc, expect, match_, expr.range)
        }
        ast::ExprKind::Match2 { .. } => todo!("match2 typecheck_expr"),
        ast::ExprKind::Field { target, name } => typecheck_field(hir, emit, proc, target, name),
        ast::ExprKind::Index {
            target,
            mutt,
            index,
        } => typecheck_index(hir, emit, proc, target, mutt, index, expr.range),
        ast::ExprKind::Call { target, input } => typecheck_call(hir, emit, proc, target, input),
        ast::ExprKind::Cast { target, into } => {
            typecheck_cast(hir, emit, proc, target, into, expr.range)
        }
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, *ty, expr.range),
        ast::ExprKind::Item { path, input } => {
            typecheck_item(hir, emit, proc, path, input, expr.range)
        }
        ast::ExprKind::Variant { name, input } => {
            typecheck_variant(hir, emit, proc, expect, name, input, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, expect, struct_init, expr.range)
        }
        ast::ExprKind::ArrayInit { input } => {
            typecheck_array_init(hir, emit, proc, expect, input, expr.range)
        }
        ast::ExprKind::ArrayRepeat { expr, len } => {
            typecheck_array_repeat(hir, emit, proc, expect, expr, len)
        }
        ast::ExprKind::Deref { rhs } => typecheck_deref(hir, emit, proc, rhs),
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(hir, emit, proc, mutt, rhs),
        ast::ExprKind::Range { range } => todo!("range feature"),
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(hir, emit, proc, expect, op, op_range, rhs)
        }
        ast::ExprKind::Binary { op, op_range, bin } => {
            typecheck_binary(hir, emit, proc, expect, op, op_range, bin)
        }
    };

    //@if `errored` is usefull it can be done via emit error count api
    if !expr_res.ignore {
        check_type_expectation(hir, emit, proc.origin(), expr.range, expect, expr_res.ty);
    }

    expr_res
}

fn typecheck_lit<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: Expectation<'hir>,
    literal: ast::Literal,
) -> TypeResult<'hir> {
    let (value, ty) = match literal {
        ast::Literal::Null => {
            let value = hir::ConstValue::Null;
            (value, hir::Type::Basic(BasicType::Rawptr))
        }
        ast::Literal::Bool(val) => {
            let value = hir::ConstValue::Bool { val };
            (value, hir::Type::Basic(BasicType::Bool))
        }
        ast::Literal::Int(val) => {
            let int_ty = coerce_int_type(expect);
            let value = hir::ConstValue::Int {
                val,
                neg: false,
                int_ty,
            };
            (value, hir::Type::Basic(int_ty.into_basic()))
        }
        ast::Literal::Float(val) => {
            let float_ty = coerce_float_type(expect);
            let value = hir::ConstValue::Float { val, float_ty };
            (value, hir::Type::Basic(float_ty.into_basic()))
        }
        ast::Literal::Char(val) => {
            let value = hir::ConstValue::Char { val };
            (value, hir::Type::Basic(BasicType::Char))
        }
        ast::Literal::String { id, c_string } => {
            let value = hir::ConstValue::String { id, c_string };
            let string_ty = alloc_string_lit_type(emit, c_string);
            (value, string_ty)
        }
    };

    let expr = hir::Expr::Const { value };
    let expr = emit.arena.alloc(expr);
    TypeResult::new(ty, expr)
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
    mut expect: Expectation<'hir>,
    if_: &ast::If<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let mut if_type = hir::Type::Basic(BasicType::Never);
    let entry = typecheck_branch(hir, emit, proc, &mut expect, &mut if_type, &if_.entry);

    let mut branches = Vec::with_capacity(if_.branches.len());
    for branch in if_.branches {
        let branch = typecheck_branch(hir, emit, proc, &mut expect, &mut if_type, branch);
        branches.push(branch);
    }
    let branches = emit.arena.alloc_slice(&branches);

    let else_block = match if_.else_block {
        Some(block) => {
            let block_res = typecheck_block(hir, emit, proc, expect, block, BlockEnter::None);

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
        emit.error(ErrorComp::new(
            "`if` expression is missing an `else` block\n`if` without `else` evaluates to `void` and cannot return a value",
            SourceRange::new(proc.origin(), expr_range),
            None,
        ));
    }

    let if_ = hir::If {
        entry,
        branches,
        else_block,
    };
    let if_ = emit.arena.alloc(if_);
    let if_expr = hir::Expr::If { if_ };
    let if_expr = emit.arena.alloc(if_expr);
    //@cannot tell when to ignore typecheck or not
    TypeResult::new(if_type, if_expr)
}

fn typecheck_branch<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: &mut Expectation<'hir>,
    if_type: &mut hir::Type<'hir>,
    branch: &ast::Branch<'_>,
) -> hir::Branch<'hir> {
    let expect_bool = Expectation::HasType(hir::Type::BOOL, None);
    let cond_res = typecheck_expr(hir, emit, proc, expect_bool, branch.cond);
    let block_res = typecheck_block(hir, emit, proc, *expect, branch.block, BlockEnter::None);

    // never -> anything
    // error -> anything except never
    if if_type.is_never() || (if_type.is_error() && !block_res.ty.is_never()) {
        *if_type = block_res.ty;
    }

    if let Expectation::None = expect {
        if !block_res.ty.is_error() && !block_res.ty.is_never() {
            let expect_src = block_res
                .tail_range
                .map(|range| SourceRange::new(proc.origin(), range));
            *expect = Expectation::HasType(block_res.ty, expect_src);
        }
    }

    hir::Branch {
        cond: cond_res.expr,
        block: block_res.block,
    }
}

fn typecheck_match<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    mut expect: Expectation<'hir>,
    match_: &ast::Match<'_>,
    match_range: TextRange,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr(hir, emit, proc, Expectation::None, match_.on_expr);
    check_match_compatibility(hir, emit, proc.origin(), on_res.ty, match_.on_expr.range);

    let mut match_type = hir::Type::Basic(BasicType::Never);
    let mut check_exaust = true;

    let pat_expect_src = SourceRange::new(proc.origin(), match_.on_expr.range);
    let pat_expect = Expectation::HasType(on_res.ty, Some(pat_expect_src));

    let mut arms = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms {
        let value =
            super::pass_4::resolve_const_expr(hir, emit, proc.origin(), pat_expect, arm.pat);
        let value_res = typecheck_expr(hir, emit, proc, expect, arm.expr);

        // never -> anything
        // error -> anything except never
        if match_type.is_never() || (match_type.is_error() && !value_res.ty.is_never()) {
            match_type = value_res.ty;
        }

        if let Expectation::None = expect {
            if !value_res.ty.is_error() && !value_res.ty.is_never() {
                let expect_src = SourceRange::new(proc.origin(), arm.expr.range);
                expect = Expectation::HasType(value_res.ty, Some(expect_src));
            }
        }

        if value == hir::ConstValue::Error {
            check_exaust = false;
        }

        let pat_value_id = emit.const_intern.intern(value);
        let tail_stmt = hir::Stmt::ExprTail(value_res.expr);
        let stmts = emit.arena.alloc_slice(&[tail_stmt]);

        let arm = hir::MatchArm {
            pat: pat_value_id,
            block: hir::Block { stmts },
            unreachable: false,
        };
        arms.push(arm);
    }

    let mut fallback = if let Some(fallback) = match_.fallback {
        let value_res = typecheck_expr(hir, emit, proc, expect, fallback);

        // never -> anything
        // error -> anything except never
        if match_type.is_never() || (match_type.is_error() && !value_res.ty.is_never()) {
            match_type = value_res.ty;
        }

        let tail_stmt = hir::Stmt::ExprTail(value_res.expr);
        let stmts = emit.arena.alloc_slice(&[tail_stmt]);
        Some(hir::Block { stmts })
    } else {
        None
    };

    if check_exaust {
        check_match_exhaust(
            hir,
            emit,
            proc,
            &mut arms,
            &mut fallback,
            match_,
            match_range,
            on_res.ty,
        );
    }

    let arms = emit.arena.alloc_slice(&arms);
    let match_ = hir::Match {
        on_expr: on_res.expr,
        arms,
        fallback,
    };
    let match_ = emit.arena.alloc(match_);
    let match_expr = hir::Expr::Match { match_ };
    let match_expr = emit.arena.alloc(match_expr);
    //@cannot tell when to ignore typecheck or not
    TypeResult::new(match_type, match_expr)
}

//@different enum variants 01.06.24
// could have same value and result in
// error in llvm ir generation, not checked currently
fn check_match_exhaust<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    arms: &mut [hir::MatchArm<'hir>],
    fallback: &mut Option<hir::Block<'hir>>,
    match_ast: &ast::Match<'_>,
    match_range: TextRange,
    on_ty: hir::Type<'hir>,
) {
    //@todo int
    match on_ty {
        hir::Type::Basic(BasicType::Bool) => {
            let mut cover_true = false;
            let mut cover_false = false;

            for (idx, arm) in arms.iter_mut().enumerate() {
                let value = emit.const_intern.get(arm.pat);
                match value {
                    hir::ConstValue::Bool { val } => {
                        if val {
                            if cover_true {
                                let ast_arm = match_ast.arms[idx];
                                arm.unreachable = true;
                                emit.warning(WarningComp::new(
                                    "unreachable pattern",
                                    SourceRange::new(proc.origin(), ast_arm.pat.0.range),
                                    None,
                                ));
                            } else {
                                cover_true = true;
                            }
                        } else {
                            if cover_false {
                                let ast_arm = match_ast.arms[idx];
                                arm.unreachable = true;
                                emit.warning(WarningComp::new(
                                    "unreachable pattern",
                                    SourceRange::new(proc.origin(), ast_arm.pat.0.range),
                                    None,
                                ));
                            } else {
                                cover_false = true;
                            }
                        }
                    }
                    _ => {}
                }
            }

            if fallback.is_some() {
                let all_covered = cover_true && cover_false;
                if all_covered {
                    *fallback = None;
                    emit.warning(WarningComp::new(
                        "unreachable pattern",
                        SourceRange::new(proc.origin(), match_ast.fallback_range),
                        None,
                    ));
                }
            } else {
                let missing = match (cover_true, cover_false) {
                    (true, true) => return,
                    (true, false) => "`false`",
                    (false, true) => "`true`",
                    (false, false) => "`true`, `false`",
                };
                emit.error(ErrorComp::new(
                    format!("non-exhaustive match patterns\nmissing: {}", missing),
                    SourceRange::new(
                        proc.origin(),
                        TextRange::new(match_range.start(), match_range.start() + 5.into()),
                    ),
                    None,
                ));
            }
        }
        hir::Type::Basic(BasicType::Char) => {
            //
        }
        hir::Type::Enum(enum_id) => {
            let data = hir.registry().enum_data(enum_id);

            let variant_count = data.variants.len();
            let mut variants_covered = Vec::new();
            variants_covered.resize(variant_count, false);

            for (idx, arm) in arms.iter_mut().enumerate() {
                let value = emit.const_intern.get(arm.pat);
                match value {
                    //@consider typecheck result of patterns to make sure this is same type 01.06.24
                    // (dont check when any error were raised or value is Error)
                    hir::ConstValue::EnumVariant { enum_ } => {
                        //@match patterns dont support enum values
                        // and pat wont be a ConstExpr in the future probably
                        let variant_id = enum_.variant_id;
                        if !variants_covered[variant_id.index()] {
                            variants_covered[variant_id.index()] = true;
                        } else {
                            let ast_arm = match_ast.arms[idx];
                            arm.unreachable = true;
                            emit.warning(WarningComp::new(
                                "unreachable pattern",
                                SourceRange::new(proc.origin(), ast_arm.pat.0.range),
                                None,
                            ));
                        }
                    }
                    _ => {}
                }
            }

            if fallback.is_some() {
                let all_covered = variants_covered.iter().copied().all(|v| v);
                if all_covered {
                    *fallback = None;
                    emit.warning(WarningComp::new(
                        "unreachable pattern",
                        SourceRange::new(proc.origin(), match_ast.fallback_range),
                        None,
                    ));
                }
            } else {
                //@simplify message with a lot of remaining patterns 01.06.24
                // eg: variants `Thing`, `Kind` and 18 more not covered
                let mut missing = String::new();
                let mut missing_count: u32 = 0;

                for idx in 0..data.variants.len() {
                    let covered = variants_covered[idx];
                    if !covered {
                        let variant = data.variant(hir::EnumVariantID::new(idx));
                        let comma = if missing_count != 0 { ", " } else { "" };
                        missing.push_str(&format!("{comma}`{}`", hir.name_str(variant.name.id)));
                        missing_count += 1;
                    }
                }

                if missing_count > 0 {
                    emit.error(ErrorComp::new(
                        format!(
                            "non-exhaustive match patterns\nmissing variants: {}",
                            missing
                        ),
                        SourceRange::new(
                            proc.origin(),
                            TextRange::new(match_range.start(), match_range.start() + 5.into()),
                        ),
                        None,
                    ));
                }
            }
        }
        _ => {}
    }
}

fn typecheck_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, Expectation::None, target);
    let field_result = check_field_from_type(hir, emit, proc.origin(), name, target_res.ty);
    emit_field_expr(emit, target_res.expr, field_result)
}

struct FieldResult<'hir> {
    deref: bool,
    kind: FieldKind<'hir>,
    field_ty: hir::Type<'hir>,
}

#[rustfmt::skip]
enum FieldKind<'hir> {
    Struct(hir::StructID, hir::StructFieldID),
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
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
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
            match check_field_from_struct(hir, emit, origin_id, name, struct_id) {
                Some((field_id, field)) => {
                    let kind = FieldKind::Struct(struct_id, field_id);
                    let field_ty = field.ty;
                    Some(FieldResult::new(deref, kind, field_ty))
                }
                None => None,
            }
        }
        hir::Type::ArraySlice(slice) => {
            match check_field_from_slice(hir, emit, origin_id, name, slice) {
                Some(field) => {
                    let kind = FieldKind::ArraySlice { field };
                    let field_ty = match field {
                        hir::SliceField::Ptr => hir::Type::Reference(&slice.elem_ty, slice.mutt),
                        hir::SliceField::Len => hir::Type::USIZE,
                    };
                    Some(FieldResult::new(deref, kind, field_ty))
                }
                None => None,
            }
        }
        hir::Type::ArrayStatic(array) => {
            match check_field_from_array(hir, emit, origin_id, name, array) {
                Some(len) => {
                    let kind = FieldKind::ArrayStatic { len };
                    let field_ty = hir::Type::USIZE;
                    Some(FieldResult::new(deref, kind, field_ty))
                }
                None => None,
            }
        }
        _ => {
            emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on value of type `{}`",
                    hir.name_str(name.id),
                    type_format(hir, emit, ty).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn check_field_from_struct<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    name: ast::Name,
    struct_id: hir::StructID,
) -> Option<(hir::StructFieldID, &'hir hir::StructField<'hir>)> {
    let data = hir.registry().struct_data(struct_id);
    match data.find_field(name.id) {
        Some((field_id, field)) => {
            if origin_id != data.origin_id && field.vis == ast::Vis::Private {
                emit.error(ErrorComp::new(
                    format!("field `{}` is private", hir.name_str(name.id)),
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
            emit.error(ErrorComp::new(
                format!("field `{}` is not found", hir.name_str(name.id)),
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
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    name: ast::Name,
    slice: &hir::ArraySlice,
) -> Option<hir::SliceField> {
    let field_name = hir.name_str(name.id);
    match field_name {
        "ptr" => Some(hir::SliceField::Ptr),
        "len" => Some(hir::SliceField::Len),
        _ => {
            emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on slice type `{}`\ndid you mean `len` or `ptr`?",
                    field_name,
                    type_format(hir, emit, hir::Type::ArraySlice(slice)).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn check_field_from_array<'hir>(
    hir: &HirData,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    name: ast::Name,
    array: &hir::ArrayStatic,
) -> Option<hir::ConstValue<'hir>> {
    let field_name = hir.name_str(name.id);
    match field_name {
        "len" => {
            let len = match array.len {
                hir::ArrayStaticLen::Immediate(len) => match len {
                    Some(value) => hir::ConstValue::Int {
                        val: value,
                        neg: false,
                        int_ty: hir::BasicInt::Usize,
                    },
                    None => hir::ConstValue::Error,
                },
                hir::ArrayStaticLen::ConstEval(eval_id) => {
                    match hir.registry().const_eval(eval_id).0 {
                        hir::ConstEval::Unresolved(_) => unreachable!(),
                        hir::ConstEval::ResolvedError => hir::ConstValue::Error,
                        hir::ConstEval::ResolvedValue(value_id) => emit.const_intern.get(value_id),
                    }
                }
            };
            Some(len)
        }
        _ => {
            emit.error(ErrorComp::new(
                format!(
                    "no field `{}` exists on array type `{}`\ndid you mean `len`?",
                    field_name,
                    type_format(hir, emit, hir::Type::ArrayStatic(array)).as_str(),
                ),
                SourceRange::new(origin_id, name.range),
                None,
            ));
            None
        }
    }
}

fn emit_field_expr<'hir>(
    emit: &mut HirEmit<'hir>,
    target: &'hir hir::Expr<'hir>,
    field_result: Option<FieldResult<'hir>>,
) -> TypeResult<'hir> {
    let result = match field_result {
        Some(result) => result,
        None => return TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR),
    };

    let expr = match result.kind {
        FieldKind::Struct(struct_id, field_id) => hir::Expr::StructField {
            target,
            struct_id,
            field_id,
            deref: result.deref,
        },
        FieldKind::ArraySlice { field } => hir::Expr::SliceField {
            target,
            field,
            deref: result.deref,
        },
        FieldKind::ArrayStatic { len } => hir::Expr::Const { value: len },
    };

    let expr = emit.arena.alloc(expr);
    TypeResult::new(result.field_ty, expr)
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
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    mutt: ast::Mut,
    index: &ast::Expr<'_>,
    expr_range: TextRange, //@use range of brackets? `[]` 08.05.24
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, Expectation::None, target);
    let expect_usize = Expectation::HasType(hir::Type::USIZE, None);
    let index_res = typecheck_expr(hir, emit, proc, expect_usize, index);

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
                            SourceRange::new(proc.origin(), expr_range), //@review source range for this type_size error 10.05.24
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
        Ok(None) => TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR),
        Err(()) => {
            emit.error(ErrorComp::new(
                format!(
                    "cannot index value of type `{}`",
                    type_format(hir, emit, target_res.ty).as_str()
                ),
                SourceRange::new(proc.origin(), expr_range),
                // is this info needed? test if its useful
                Info::new(
                    format!(
                        "has `{}` type",
                        type_format(hir, emit, target_res.ty).as_str()
                    ),
                    SourceRange::new(proc.origin(), target.range),
                ),
            ));
            TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR)
        }
    }
}

fn typecheck_call<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    input: &ast::Input<'_>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, Expectation::None, target);
    check_call_indirect(hir, emit, proc, target_res, target.range, input)
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
        hir::Type::Enum(id) => {
            let data = hir.registry().enum_data(id);
            if data.variants.is_empty() {
                Some(hir::Size::new(0, 1))
            } else {
                Some(basic_type_size(data.int_ty.into_basic()))
            }
        }
        hir::Type::Struct(id) => hir.registry().struct_data(id).size_eval.get_size(),
        hir::Type::Reference(_, _) => Some(hir::Size::new_equal(8)), //@assume 64bit target
        hir::Type::Procedure(_) => Some(hir::Size::new_equal(8)),    //@assume 64bit target
        hir::Type::ArraySlice(_) => Some(hir::Size::new(16, 8)),     //@assume 64bit target
        hir::Type::ArrayStatic(array) => {
            if let (Some(elem_size), Some(len)) = (
                type_size(hir, emit, array.elem_ty, source),
                array_static_len(hir, emit, array.len),
            ) {
                if let Some(array_size) = elem_size.size().checked_mul(len) {
                    Some(hir::Size::new(array_size, elem_size.align()))
                } else {
                    emit.error(ErrorComp::new(
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
        BasicType::F32 => hir::Size::new_equal(4),
        BasicType::F64 => hir::Size::new_equal(8),
        BasicType::Bool => hir::Size::new_equal(1),
        BasicType::Char => hir::Size::new_equal(4),
        BasicType::Rawptr => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::Void => hir::Size::new(0, 1),
        BasicType::Never => hir::Size::new(0, 1),
    }
}

#[derive(Copy, Clone)]
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
    fn new(basic: BasicType) -> BasicTypeKind {
        match basic {
            BasicType::S8 | BasicType::S16 | BasicType::S32 | BasicType::S64 | BasicType::Ssize => {
                BasicTypeKind::SignedInt
            }
            BasicType::U8 | BasicType::U16 | BasicType::U32 | BasicType::U64 | BasicType::Usize => {
                BasicTypeKind::UnsignedInt
            }
            BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
            BasicType::Bool => BasicTypeKind::Bool,
            BasicType::Char => BasicTypeKind::Char,
            BasicType::Rawptr => BasicTypeKind::Rawptr,
            BasicType::Void => BasicTypeKind::Void,
            BasicType::Never => BasicTypeKind::Never,
        }
    }

    fn is_integer(self) -> bool {
        matches!(self, BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt)
    }

    fn is_signed_integer(self) -> bool {
        matches!(self, BasicTypeKind::SignedInt)
    }

    fn is_number(self) -> bool {
        matches!(
            self,
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float
        )
    }

    fn is_any_value_type(self) -> bool {
        !matches!(self, BasicTypeKind::Void | BasicTypeKind::Never)
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
    let target_res = typecheck_expr(hir, emit, proc, Expectation::None, target);
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
        emit.warning(WarningComp::new(
            format!(
                "redundant cast from `{}` into `{}`",
                type_format(hir, emit, target_res.ty).as_str(),
                type_format(hir, emit, into).as_str()
            ),
            SourceRange::new(proc.origin(), range),
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
            let from_kind = BasicTypeKind::new(from);
            let into_kind = BasicTypeKind::new(into);
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
        emit.error(ErrorComp::new(
            format!(
                "non primitive cast from `{}` into `{}`",
                type_format(hir, emit, target_res.ty).as_str(),
                type_format(hir, emit, into).as_str()
            ),
            SourceRange::new(proc.origin(), range),
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
    let sizeof_expr = match type_size(hir, emit, ty, SourceRange::new(proc.origin(), expr_range)) {
        Some(size) => {
            let value = hir::ConstValue::Int {
                val: size.size(),
                neg: false,
                int_ty: BasicInt::Usize,
            };
            emit.arena.alloc(hir::Expr::Const { value })
        }
        None => hir_build::EXPR_ERROR,
    };

    TypeResult::new(hir::Type::Basic(BasicType::Usize), sizeof_expr)
}

fn typecheck_item<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    path: &ast::Path,
    input: Option<&ast::Input>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let (value_id, field_names) = path_resolve_value(hir, emit, Some(proc), proc.origin(), path);

    let item_res = match value_id {
        ValueID::None => {
            return error_result_default_check_input_opt(hir, emit, proc, input);
        }
        ValueID::Proc(proc_id) => {
            //@where do fields go? none expected?
            // ValueID or something like it should provide
            // remaining fields in each variant so its known when none can exist
            // instead of hadling the empty slice and assuming its correct
            if let Some(input) = input {
                return check_call_direct(hir, emit, proc, proc_id, input);
            } else {
                let data = hir.registry().proc_data(proc_id);
                //@creating proc type each time its encountered / called, waste of arena memory 25.05.24
                let mut param_types = Vec::with_capacity(data.params.len());
                for param in data.params {
                    param_types.push(param.ty);
                }
                let proc_ty = hir::ProcType {
                    param_types: emit.arena.alloc_slice(&param_types),
                    is_variadic: data.attr_set.contains(hir::ProcFlag::Variadic),
                    return_ty: data.return_ty,
                };

                let proc_ty = hir::Type::Procedure(emit.arena.alloc(proc_ty));
                let proc_value = hir::ConstValue::Procedure { proc_id };
                let proc_expr = hir::Expr::Const { value: proc_value };
                let proc_expr = emit.arena.alloc(proc_expr);
                return TypeResult::new(proc_ty, proc_expr);
            }
        }
        ValueID::Enum(enum_id, variant_id) => {
            //@no fields is guaranteed by path resolve
            // remove assert when stable, or path resolve is reworked
            assert!(field_names.is_empty());
            return check_variant_input_opt(
                hir, emit, proc, enum_id, variant_id, input, expr_range,
            );
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

    let mut target_res = item_res;
    for &name in field_names {
        let field_result = check_field_from_type(hir, emit, proc.origin(), name, target_res.ty);
        target_res = emit_field_expr(emit, target_res.expr, field_result);
    }

    if let Some(input) = input {
        check_call_indirect(hir, emit, proc, target_res, expr_range, input)
    } else {
        target_res
    }
}

fn typecheck_variant<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    name: ast::Name,
    input: Option<&ast::Input>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let enum_id = infer_enum_type(emit, expect, SourceRange::new(proc.origin(), expr_range));
    let enum_id = match enum_id {
        Some(enum_id) => enum_id,
        None => return error_result_default_check_input_opt(hir, emit, proc, input),
    };

    let data = hir.registry().enum_data(enum_id);
    let variant_id = match data.find_variant(name.id) {
        Some((variant_id, _)) => variant_id,
        None => {
            //@duplicate error, same as path resolve 1.07.24
            emit.error(ErrorComp::new(
                format!("enum variant `{}` is not found", hir.name_str(name.id)),
                SourceRange::new(proc.origin(), name.range),
                Info::new(
                    "enum defined here",
                    SourceRange::new(data.origin_id, data.name.range),
                ),
            ));
            return error_result_default_check_input_opt(hir, emit, proc, input);
        }
    };

    check_variant_input_opt(hir, emit, proc, enum_id, variant_id, input, expr_range)
}

//@still used in pass4
pub fn error_cannot_infer_struct_type(emit: &mut HirEmit, src: SourceRange) {
    emit.error(ErrorComp::new("cannot infer struct type", src, None))
}

fn typecheck_struct_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    struct_init: &ast::StructInit<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let struct_id = match struct_init.path {
        Some(path) => path_resolve_struct(hir, emit, Some(proc), proc.origin(), path),
        None => infer_struct_type(emit, expect, SourceRange::new(proc.origin(), expr_range)),
    };
    let struct_id = match struct_id {
        Some(struct_id) => struct_id,
        None => return error_result_default_check_field_init(hir, emit, proc, struct_init.input),
    };

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
            //@get expect source?
            let expect = Expectation::HasType(field.ty, None);
            let input_res = typecheck_expr(hir, emit, proc, expect, input.expr);

            if let FieldStatus::Init(range) = field_status[field_id.index()] {
                emit.error(ErrorComp::new(
                    format!(
                        "field `{}` was already initialized",
                        hir.name_str(input.name.id),
                    ),
                    SourceRange::new(proc.origin(), input.name.range),
                    Info::new("initialized here", SourceRange::new(data.origin_id, range)),
                ));
            } else {
                if proc.origin() != data.origin_id {
                    if field.vis == ast::Vis::Private {
                        emit.error(ErrorComp::new(
                            format!("field `{}` is private", hir.name_str(field.name.id),),
                            SourceRange::new(proc.origin(), input.name.range),
                            Info::new(
                                "defined here",
                                SourceRange::new(data.origin_id, field.name.range),
                            ),
                        ));
                    }
                }

                let field_init = hir::StructFieldInit {
                    field_id,
                    expr: input_res.expr,
                };
                field_inits.push(field_init);
                field_status[field_id.index()] = FieldStatus::Init(input.name.range);
                init_count += 1;
            }
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "field `{}` is not found in `{}`",
                    hir.name_str(input.name.id),
                    hir.name_str(data.name.id)
                ),
                SourceRange::new(proc.origin(), input.name.range),
                Info::new(
                    "struct defined here",
                    SourceRange::new(data.origin_id, data.name.range),
                ),
            ));
            let _ = typecheck_expr(hir, emit, proc, Expectation::None, input.expr);
        }
    }

    if init_count < field_count {
        //@change message to list limited number of fields based on their name len()
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

        emit.error(ErrorComp::new(
            message,
            SourceRange::new(proc.origin(), expr_range),
            Info::new(
                "struct defined here",
                SourceRange::new(data.origin_id, data.name.range),
            ),
        ));
    }

    let input = emit.arena.alloc_slice(&field_inits);
    let struct_init = hir::Expr::StructInit { struct_id, input };
    TypeResult::new(hir::Type::Struct(struct_id), emit.arena.alloc(struct_init))
}

fn typecheck_array_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
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
            let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
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
                    Some(SourceRange::new(proc.origin(), expr.range)),
                )
            }
            */
        }
        emit.arena.alloc_slice(&input_res)
    };

    let elem_ty = if input.is_empty() {
        if let Some(array_ty) = expect_array_ty {
            array_ty.elem_ty
        } else {
            emit.error(ErrorComp::new(
                "cannot infer type of empty array",
                SourceRange::new(proc.origin(), array_range),
                None,
            ));
            hir::Type::Error
        }
    } else {
        elem_ty
    };

    let array_type: &hir::ArrayStatic = emit.arena.alloc(hir::ArrayStatic {
        len: hir::ArrayStaticLen::Immediate(Some(input.len() as u64)),
        elem_ty,
    });
    let array_init = emit.arena.alloc(hir::ArrayInit { elem_ty, input });
    let array_expr = emit.arena.alloc(hir::Expr::ArrayInit { array_init });
    TypeResult::new(hir::Type::ArrayStatic(array_type), array_expr)
}

fn typecheck_array_repeat<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
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

    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);

    //@this is duplicated here and in pass_3::type_resolve 09.05.24
    let value = super::pass_4::resolve_const_expr(
        hir,
        emit,
        proc.origin(),
        Expectation::HasType(hir::Type::USIZE, None),
        len,
    );
    let len = match value {
        hir::ConstValue::Int { val, neg, int_ty } => {
            if neg {
                None
            } else {
                Some(val)
            }
        }
        _ => None,
    };

    if let Some(len) = len {
        let array_type = emit.arena.alloc(hir::ArrayStatic {
            len: hir::ArrayStaticLen::Immediate(Some(len)), //@move to always specified size? else error 09.06.24
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
    } else {
        TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR)
    }
}

fn typecheck_deref<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, Expectation::None, rhs);

    let ptr_ty = match rhs_res.ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::Reference(ref_ty, _) => *ref_ty,
        _ => {
            emit.error(ErrorComp::new(
                format!(
                    "cannot dereference value of type `{}`",
                    type_format(hir, emit, rhs_res.ty).as_str()
                ),
                SourceRange::new(proc.origin(), rhs.range),
                None,
            ));
            hir::Type::Error
        }
    };

    let deref_expr = hir::Expr::Deref {
        rhs: rhs_res.expr,
        ptr_ty: emit.arena.alloc(ptr_ty),
    };
    TypeResult::new(ptr_ty, emit.arena.alloc(deref_expr))
}

fn typecheck_address<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    mutt: ast::Mut,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, Expectation::None, rhs);
    let adressability = get_expr_addressability(hir, proc, rhs_res.expr);

    match adressability {
        Addressability::Unknown => {} //@ & to error should be also Error? 16.05.24
        Addressability::Constant => {
            emit.error(ErrorComp::new(
                "cannot get reference to a constant, you can use `global` instead",
                SourceRange::new(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::new(
                "cannot get reference to a slice field, slice itself cannot be modified",
                SourceRange::new(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::Temporary => {
            emit.error(ErrorComp::new(
                "cannot get reference to a temporary value",
                SourceRange::new(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::TemporaryImmutable => {
            if mutt == ast::Mut::Mutable {
                emit.error(ErrorComp::new(
                    "cannot get mutable reference to this temporary value, only immutable `&` is allowed",
                    SourceRange::new(proc.origin(), rhs.range),
                    None,
                ));
            }
        }
        Addressability::Addressable(rhs_mutt, src) => {
            if mutt == ast::Mut::Mutable && rhs_mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::new(
                    "cannot get mutable reference to an immutable variable",
                    SourceRange::new(proc.origin(), rhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::new(
                "addressability not implemented for this expression",
                SourceRange::new(proc.origin(), rhs.range),
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
        hir::Expr::Const { .. } => Addressability::Temporary, //@TemporaryImmutable for struct / array? and alloca them
        hir::Expr::If { .. } => Addressability::Temporary,
        hir::Expr::Block { .. } => Addressability::Temporary,
        hir::Expr::Match { .. } => Addressability::Temporary,
        hir::Expr::StructField { target, .. } => get_expr_addressability(hir, proc, target),
        hir::Expr::SliceField { .. } => Addressability::SliceField,
        hir::Expr::Index { target, .. } => get_expr_addressability(hir, proc, target),
        hir::Expr::Slice { target, .. } => get_expr_addressability(hir, proc, target),
        hir::Expr::Cast { .. } => Addressability::Temporary,
        hir::Expr::LocalVar { local_id } => {
            let local = proc.get_local(local_id);
            Addressability::Addressable(
                local.mutt,
                SourceRange::new(proc.origin(), local.name.range),
            )
        }
        hir::Expr::ParamVar { param_id } => {
            let param = proc.get_param(param_id);
            Addressability::Addressable(
                param.mutt,
                SourceRange::new(proc.origin(), param.name.range),
            )
        }
        hir::Expr::ConstVar { .. } => Addressability::Constant,
        hir::Expr::GlobalVar { global_id } => {
            let data = hir.registry().global_data(global_id);
            Addressability::Addressable(
                data.mutt,
                SourceRange::new(data.origin_id, data.name.range),
            )
        }
        hir::Expr::EnumVariant { .. } => Addressability::NotImplemented, //@todo
        hir::Expr::CallDirect { .. } => Addressability::Temporary,
        hir::Expr::CallIndirect { .. } => Addressability::Temporary,
        hir::Expr::StructInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayRepeat { .. } => Addressability::TemporaryImmutable,
        hir::Expr::Deref { rhs, .. } => get_expr_addressability(hir, proc, rhs),
        hir::Expr::Address { .. } => Addressability::Temporary,
        hir::Expr::Unary { .. } => Addressability::Temporary,
        hir::Expr::Binary { .. } => Addressability::Temporary,
    }
}

fn typecheck_unary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    op: ast::UnOp,
    op_range: TextRange,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_expect = match op {
        ast::UnOp::Neg => expect,
        ast::UnOp::BitNot => expect,
        ast::UnOp::LogicNot => Expectation::HasType(hir::Type::BOOL, None),
    };
    let rhs_res = typecheck_expr(hir, emit, proc, rhs_expect, rhs);

    let un_op = check_un_op_compatibility(hir, emit, proc.origin(), rhs_res.ty, op, op_range);

    if let Some(un_op) = un_op {
        let unary_type = match op {
            ast::UnOp::Neg => rhs_res.ty,
            ast::UnOp::BitNot => rhs_res.ty,
            ast::UnOp::LogicNot => hir::Type::BOOL,
        };
        let unary_expr = hir::Expr::Unary {
            op: un_op,
            rhs: rhs_res.expr,
        };
        let unary_expr = emit.arena.alloc(unary_expr);
        TypeResult::new(unary_type, unary_expr)
    } else {
        TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR)
    }
}

fn check_un_op_compatibility(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    rhs_ty: hir::Type,
    op: ast::UnOp,
    op_range: TextRange,
) -> Option<hir::UnOp> {
    let basic = match rhs_ty {
        hir::Type::Error => return None,
        hir::Type::Basic(basic) => basic,
        _ => {
            error_cannot_apply_unary_op(hir, emit, origin_id, rhs_ty, op, op_range);
            return None;
        }
    };

    let un_op = match op {
        ast::UnOp::Neg => {
            if let Some(sint_ty) = hir::BasicIntSigned::from_basic(basic) {
                Some(hir::UnOp::Neg_Int(sint_ty))
            } else if let Some(float_ty) = hir::BasicFloat::from_basic(basic) {
                Some(hir::UnOp::Neg_Float(float_ty))
            } else {
                None
            }
        }
        ast::UnOp::BitNot => {
            if let Some(int_ty) = hir::BasicInt::from_basic(basic) {
                Some(hir::UnOp::BitNot(int_ty))
            } else {
                None
            }
        }
        ast::UnOp::LogicNot => {
            if let ast::BasicType::Bool = basic {
                Some(hir::UnOp::LogicNot)
            } else {
                None
            }
        }
    };

    if un_op.is_none() {
        error_cannot_apply_unary_op(hir, emit, origin_id, rhs_ty, op, op_range);
    }

    un_op
}

fn error_cannot_apply_unary_op(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    rhs_ty: hir::Type,
    op: ast::UnOp,
    op_range: TextRange,
) {
    emit.error(ErrorComp::new(
        format!(
            "cannot apply unary operator `{}` on value of type `{}`",
            op.as_str(),
            type_format(hir, emit, rhs_ty).as_str()
        ),
        SourceRange::new(origin_id, op_range),
        None,
    ));
}

//@bin << >> should allow any integer type on the right, same sized int?
// no type expectation for this is possible 25.05.24
fn typecheck_binary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    op: ast::BinOp,
    op_range: TextRange,
    bin: &ast::BinExpr<'_>,
) -> TypeResult<'hir> {
    let lhs_expect = match op {
        ast::BinOp::IsEq
        | ast::BinOp::NotEq
        | ast::BinOp::Less
        | ast::BinOp::LessEq
        | ast::BinOp::Greater
        | ast::BinOp::GreaterEq => Expectation::None,
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => Expectation::HasType(hir::Type::BOOL, None),
        _ => expect,
    };
    let lhs_res = typecheck_expr(hir, emit, proc, lhs_expect, bin.lhs);

    let compatible = check_bin_op_compatibility(hir, emit, proc.origin(), lhs_res.ty, op, op_range);

    let rhs_expect = match op {
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => Expectation::HasType(hir::Type::BOOL, None),
        _ => {
            let rhs_expect_src = SourceRange::new(proc.origin(), bin.lhs.range);
            Expectation::HasType(lhs_res.ty, Some(rhs_expect_src))
        }
    };
    let rhs_res = typecheck_expr(hir, emit, proc, rhs_expect, bin.rhs);

    let binary_ty = if compatible {
        match op {
            ast::BinOp::IsEq
            | ast::BinOp::NotEq
            | ast::BinOp::Less
            | ast::BinOp::LessEq
            | ast::BinOp::Greater
            | ast::BinOp::GreaterEq
            | ast::BinOp::LogicAnd
            | ast::BinOp::LogicOr => hir::Type::BOOL,
            _ => lhs_res.ty,
        }
    } else {
        hir::Type::Error
    };

    let lhs_signed_int = match binary_ty {
        hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_signed_integer(),
        _ => false,
    };
    let binary_expr = hir::Expr::Binary {
        op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_signed_int,
    };
    TypeResult::new(binary_ty, emit.arena.alloc(binary_expr))
}

fn check_match_compatibility<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    ty: hir::Type,
    range: TextRange,
) {
    let compatible = match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::new(basic),
            BasicTypeKind::SignedInt
                | BasicTypeKind::UnsignedInt
                | BasicTypeKind::Bool
                | BasicTypeKind::Char
        ),
        hir::Type::Enum(_) => true,
        _ => false,
    };

    if !compatible {
        emit.error(ErrorComp::new(
            format!(
                "cannot match on value of type `{}`",
                type_format(hir, emit, ty).as_str()
            ),
            SourceRange::new(origin_id, range),
            None,
        ));
    }
}

fn check_bin_op_compatibility<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    lhs_ty: hir::Type,
    op: ast::BinOp,
    op_range: TextRange,
) -> bool {
    if lhs_ty.is_error() {
        return false;
    }

    let compatible = match op {
        ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Div => match lhs_ty {
            hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_number(),
            _ => false,
        },
        ast::BinOp::Rem
        | ast::BinOp::BitAnd
        | ast::BinOp::BitOr
        | ast::BinOp::BitXor
        | ast::BinOp::BitShl
        | ast::BinOp::BitShr => match lhs_ty {
            hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_integer(),
            _ => false,
        },
        ast::BinOp::IsEq | ast::BinOp::NotEq => match lhs_ty {
            hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_any_value_type(),
            hir::Type::Enum(_) => true,
            _ => false,
        },
        ast::BinOp::Less | ast::BinOp::LessEq | ast::BinOp::Greater | ast::BinOp::GreaterEq => {
            match lhs_ty {
                hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_number(),
                _ => false,
            }
        }
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => {
            matches!(lhs_ty, hir::Type::Basic(BasicType::Bool))
        }
    };

    if !compatible {
        emit.error(ErrorComp::new(
            format!(
                "cannot apply binary operator `{}` on value of type `{}`",
                op.as_str(),
                type_format(hir, emit, lhs_ty).as_str()
            ),
            SourceRange::new(origin_id, op_range),
            None,
        ));
    }
    compatible
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: Expectation<'hir>,
    block: ast::Block<'_>,
    enter: BlockEnter,
) -> BlockResult<'hir> {
    proc.push_block(enter);

    let mut block_stmts: Vec<hir::Stmt> = Vec::with_capacity(block.stmts.len());
    let mut block_ty: Option<hir::Type> = None;
    let mut tail_range: Option<TextRange> = None;

    for stmt in block.stmts {
        let (hir_stmt, diverges) = match stmt.kind {
            ast::StmtKind::Break => {
                if let Some(stmt_res) = typecheck_break(emit, proc, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Continue => {
                if let Some(stmt_res) = typecheck_continue(emit, proc, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Return(expr) => {
                if let Some(stmt_res) = typecheck_return(hir, emit, proc, expr, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            //@defer block divergence should be simulated to correctly handle block_ty and divergence warnings 03.07.24
            ast::StmtKind::Defer(block) => {
                if let Some(stmt_res) = typecheck_defer(hir, emit, proc, *block, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Loop(loop_) => {
                //@can diverge (inf loop, return, panic)
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Loop(typecheck_loop(hir, emit, proc, loop_));
                (stmt_res, diverges)
            }
            ast::StmtKind::Local(local) => {
                //@can diverge any diverging expr inside
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Local(typecheck_local(hir, emit, proc, local));
                (stmt_res, diverges)
            }
            ast::StmtKind::Assign(assign) => {
                //@can diverge any diverging expr inside
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Assign(typecheck_assign(hir, emit, proc, assign));
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

                let error_count = emit.error_count();
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                if !emit.did_error(error_count) {
                    check_unused_expr_semi(emit, proc, expr_res.expr, expr.range);
                }

                //@migrate to using never type instead of diverges bool flags
                let will_diverge = expr_res.ty.is_never();
                let diverges = proc.check_stmt_diverges(hir, emit, will_diverge, stmt.range);

                let stmt_res = hir::Stmt::ExprSemi(expr_res.expr);
                (stmt_res, diverges)
            }
            ast::StmtKind::ExprTail(expr) => {
                /*
                //@30.05.24
                proc example() -> s32 {
                    // incorrect diverges warning since
                    // block isnt entered untill  typecheck_expr is called
                    -> { // after this
                        // warning for this
                        -> 10;
                    };
                }
                 */
                // type expectation is delegated to tail expression, instead of the block itself
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                let stmt_res = hir::Stmt::ExprTail(expr_res.expr);
                // @seems to fix the problem (still a hack)
                let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);

                // only assigned once, any further `ExprTail` are unreachable
                if block_ty.is_none() {
                    block_ty = Some(expr_res.ty);
                    tail_range = Some(expr.range);
                }
                (stmt_res, diverges)
            }
        };

        if !diverges {
            block_stmts.push(hir_stmt);
        }
    }

    let stmts = emit.arena.alloc_slice(&block_stmts);
    let hir_block = hir::Block { stmts };

    //@wip approach, will change 03.07.24
    let block_result = if let Some(block_ty) = block_ty {
        BlockResult::new(block_ty, hir_block, tail_range)
    } else {
        //@potentially incorrect aproach, verify that `void`
        // as the expectation and block result ty are valid 29.05.24
        let diverges = proc.diverges().is_always();
        if !diverges {
            check_type_expectation(
                hir,
                emit,
                proc.origin(),
                block.range,
                expect,
                hir::Type::VOID,
            );
        }
        //@hack but should be correct
        let block_ty = if diverges {
            hir::Type::Basic(BasicType::Never)
        } else {
            hir::Type::VOID
        };
        BlockResult::new(block_ty, hir_block, tail_range)
    };

    proc.pop_block();
    block_result
}

fn check_unused_expr_semi(
    emit: &mut HirEmit,
    proc: &ProcScope,
    expr: &hir::Expr,
    expr_range: TextRange,
) {
    enum UnusedExpr {
        No,
        Maybe,
        Yes(&'static str),
    }

    let unused = match *expr {
        // errored expressions are not allowed to be checked
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { .. } => UnusedExpr::Yes("constant value"),
        hir::Expr::If { .. } => UnusedExpr::Maybe,
        hir::Expr::Block { .. } => UnusedExpr::Maybe,
        hir::Expr::Match { .. } => UnusedExpr::Maybe,
        hir::Expr::StructField { .. } => UnusedExpr::Yes("field access"),
        hir::Expr::SliceField { .. } => UnusedExpr::Yes("field access"),
        hir::Expr::Index { .. } => UnusedExpr::Yes("index access"),
        hir::Expr::Slice { .. } => UnusedExpr::Yes("slice value"),
        hir::Expr::Cast { .. } => UnusedExpr::Yes("cast value"),
        hir::Expr::LocalVar { .. } => UnusedExpr::Yes("local value"),
        hir::Expr::ParamVar { .. } => UnusedExpr::Yes("parameter value"),
        hir::Expr::ConstVar { .. } => UnusedExpr::Yes("constant value"),
        hir::Expr::GlobalVar { .. } => UnusedExpr::Yes("global value"),
        hir::Expr::EnumVariant { .. } => UnusedExpr::Yes("variant value"),
        hir::Expr::CallDirect { .. } => UnusedExpr::No, //@only if #[must_use] (not implemented)
        hir::Expr::CallIndirect { .. } => UnusedExpr::No, //@only if #[must_use] (not implemented)
        hir::Expr::StructInit { .. } => UnusedExpr::Yes("struct value"),
        hir::Expr::ArrayInit { .. } => UnusedExpr::Yes("array value"),
        hir::Expr::ArrayRepeat { .. } => UnusedExpr::Yes("array value"),
        hir::Expr::Deref { .. } => UnusedExpr::Yes("dereference"),
        hir::Expr::Address { .. } => UnusedExpr::Yes("address value"),
        hir::Expr::Unary { .. } => UnusedExpr::Yes("unary operation"),
        hir::Expr::Binary { .. } => UnusedExpr::Yes("binary operation"),
    };

    if let UnusedExpr::Yes(kind) = unused {
        emit.warning(WarningComp::new(
            format!("unused {}", kind),
            SourceRange::new(proc.origin(), expr_range),
            None,
        ));
    }
}

/// returns `None` on invalid use of `break`
fn typecheck_break<'hir>(
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    match proc.loop_status() {
        LoopStatus::None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
            emit.error(ErrorComp::new(
                "cannot use `break` outside of loop",
                SourceRange::new(proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());
            emit.error(ErrorComp::new(
                "cannot use `break` in loop started outside of `defer`",
                SourceRange::new(proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside => Some(hir::Stmt::Break),
    }
}

/// returns `None` on invalid use of `continue`
fn typecheck_continue<'hir>(
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    match proc.loop_status() {
        LoopStatus::None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
            emit.error(ErrorComp::new(
                "cannot use `continue` outside of loop",
                SourceRange::new(proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 8.into());
            emit.error(ErrorComp::new(
                "cannot use `continue` in loop started outside of `defer`",
                SourceRange::new(proc.origin(), kw_range),
                None,
            ));
            None
        }
        LoopStatus::Inside => Some(hir::Stmt::Continue),
    }
}

/// returns `None` on invalid use of `return`
fn typecheck_return<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expr: Option<&ast::Expr>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let valid = if let DeferStatus::Inside(prev_defer) = proc.defer_status() {
        let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
        emit.error(ErrorComp::new(
            "cannot use `return` inside `defer`",
            SourceRange::new(proc.origin(), kw_range),
            Info::new("in this defer", SourceRange::new(proc.origin(), prev_defer)),
        ));
        false
    } else {
        true
    };

    let expr = match expr {
        Some(expr) => {
            let expr_res = typecheck_expr(hir, emit, proc, proc.return_expect(), expr);
            Some(expr_res.expr)
        }
        None => {
            let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 6.into());
            check_type_expectation(
                hir,
                emit,
                proc.origin(),
                kw_range,
                proc.return_expect(),
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
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    block: ast::Block<'_>,
    stmt_range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    let kw_range = TextRange::new(stmt_range.start(), stmt_range.start() + 5.into());

    let valid = if let DeferStatus::Inside(prev_defer) = proc.defer_status() {
        emit.error(ErrorComp::new(
            "`defer` statements cannot be nested",
            SourceRange::new(proc.origin(), kw_range),
            Info::new(
                "already in this defer",
                SourceRange::new(proc.origin(), prev_defer),
            ),
        ));
        false
    } else {
        true
    };

    let block_res = typecheck_block(
        hir,
        emit,
        proc,
        Expectation::HasType(hir::Type::VOID, None),
        block,
        BlockEnter::Defer(kw_range),
    );

    if valid {
        let block = emit.arena.alloc(block_res.block);
        Some(hir::Stmt::Defer(block))
    } else {
        None
    }
}

fn typecheck_loop<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    loop_: &ast::Loop<'_>,
) -> &'hir hir::Loop<'hir> {
    let kind = match loop_.kind {
        ast::LoopKind::Loop => hir::LoopKind::Loop,
        ast::LoopKind::While { cond } => {
            let cond_res = typecheck_expr(
                hir,
                emit,
                proc,
                Expectation::HasType(hir::Type::BOOL, None),
                cond,
            );
            hir::LoopKind::While {
                cond: cond_res.expr,
            }
        }
        ast::LoopKind::ForLoop {
            local,
            cond,
            assign,
        } => {
            let local_id = typecheck_local(hir, emit, proc, local);
            let cond_res = typecheck_expr(
                hir,
                emit,
                proc,
                Expectation::HasType(hir::Type::BOOL, None),
                cond,
            );
            let assign = typecheck_assign(hir, emit, proc, assign);
            hir::LoopKind::ForLoop {
                local_id,
                cond: cond_res.expr,
                assign,
            }
        }
    };

    let block_res = typecheck_block(
        hir,
        emit,
        proc,
        Expectation::HasType(hir::Type::VOID, None),
        loop_.block,
        BlockEnter::Loop,
    );

    let loop_ = hir::Loop {
        kind,
        block: block_res.block,
    };
    emit.arena.alloc(loop_)
}

fn typecheck_local<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    local: &ast::Local,
) -> hir::LocalID {
    //@theres no `nice` way to find both existing name from global (hir) scope
    // and proc_scope, those are so far disconnected,
    // some unified model of symbols might be better in the future
    // this also applies to SymbolKind which is separate from VariableID (leads to some issues in path resolve) @1.04.24
    let already_defined = if let Some(existing) =
        hir.symbol_in_scope_source(proc.origin(), local.name.id)
    {
        super::pass_1::error_name_already_defined(hir, emit, proc.origin(), local.name, existing);
        true
    } else if let Some(existing_var) = proc.find_variable(local.name.id) {
        let existing = match existing_var {
            VariableID::Local(id) => SourceRange::new(proc.origin(), proc.get_local(id).name.range),
            VariableID::Param(id) => SourceRange::new(proc.origin(), proc.get_param(id).name.range),
        };
        super::pass_1::error_name_already_defined(hir, emit, proc.origin(), local.name, existing);
        true
    } else {
        false
    };

    let (local_ty, local_value) = match local.kind {
        ast::LocalKind::Decl(ast_ty) => {
            let hir_ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ast_ty);
            (hir_ty, None)
        }
        ast::LocalKind::Init(ast_ty, value) => {
            let expect = if let Some(ast_ty) = ast_ty {
                let hir_ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ast_ty);
                let expect_src = SourceRange::new(proc.origin(), ast_ty.range);
                Expectation::HasType(hir_ty, Some(expect_src))
            } else {
                Expectation::None
            };

            let value_res = typecheck_expr(hir, emit, proc, expect, value);

            if ast_ty.is_some() {
                match expect {
                    Expectation::None => unreachable!(),
                    Expectation::HasType(expect_ty, _) => (expect_ty, Some(value_res.expr)),
                }
            } else {
                (value_res.ty, Some(value_res.expr))
            }
        }
    };

    if already_defined {
        hir::LocalID::dummy()
    } else {
        //@check for `never`, `void` to prevent panic during codegen
        let local = emit.arena.alloc(hir::Local {
            mutt: local.mutt,
            name: local.name,
            ty: local_ty,
            value: local_value,
        });
        proc.push_local(local)
    }
}

//@not checking bin assignment operators (need a good way to do it same in binary expr typecheck)
fn typecheck_assign<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    assign: &ast::Assign,
) -> &'hir hir::Assign<'hir> {
    let lhs_res = typecheck_expr(hir, emit, proc, Expectation::None, assign.lhs);
    let adressability = get_expr_addressability(hir, proc, lhs_res.expr);

    match adressability {
        Addressability::Unknown => {}
        Addressability::Constant => {
            emit.error(ErrorComp::new(
                "cannot assign to a constant",
                SourceRange::new(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::new(
                "cannot assign to a slice field, slice itself cannot be modified",
                SourceRange::new(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Temporary | Addressability::TemporaryImmutable => {
            emit.error(ErrorComp::new(
                "cannot assign to a temporary value",
                SourceRange::new(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Addressable(mutt, src) => {
            if mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::new(
                    "cannot assign to an immutable variable",
                    SourceRange::new(proc.origin(), assign.lhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::new(
                "addressability not implemented for this expression",
                SourceRange::new(proc.origin(), assign.lhs.range),
                None,
            ));
        }
    }

    let rhs_expect_src = SourceRange::new(proc.origin(), assign.lhs.range);
    let rhs_expect = Expectation::HasType(lhs_res.ty, Some(rhs_expect_src));
    let rhs_res = typecheck_expr(hir, emit, proc, rhs_expect, assign.rhs);

    //@binary assignment ops not checked
    let lhs_signed_int = match lhs_res.ty {
        hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_signed_integer(),
        _ => false,
    };
    let assign = hir::Assign {
        op: assign.op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_ty: lhs_res.ty,
        lhs_signed_int,
    };
    emit.arena.alloc(assign)
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
        emit.error(ErrorComp::new(
            format!(
                "expected value type, found `{}`",
                type_format(hir, emit, ty).as_str()
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
        hir::Type::Struct(_) => true,
        hir::Type::Reference(ref_ty, _) => type_is_value_type(*ref_ty),
        hir::Type::Procedure(_) => true,
        hir::Type::ArraySlice(slice) => type_is_value_type(slice.elem_ty),
        hir::Type::ArrayStatic(array) => type_is_value_type(array.elem_ty),
    }
}

//==================== INFERENCE ====================
//@check if reference expectations need to be accounted for

fn infer_enum_type(
    emit: &mut HirEmit,
    expect: Expectation,
    error_src: SourceRange,
) -> Option<hir::EnumID> {
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

fn infer_struct_type(
    emit: &mut HirEmit,
    expect: Expectation,
    error_src: SourceRange,
) -> Option<hir::StructID> {
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
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    input: &ast::Input,
) -> TypeResult<'hir> {
    for &expr in input.exprs.iter() {
        let _ = typecheck_expr(hir, emit, proc, Expectation::None, expr);
    }
    return TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR);
}

fn error_result_default_check_input_opt<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    input: Option<&ast::Input>,
) -> TypeResult<'hir> {
    if let Some(input) = input {
        for &expr in input.exprs.iter() {
            let _ = typecheck_expr(hir, emit, proc, Expectation::None, expr);
        }
    }
    return TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR);
}

fn error_result_default_check_field_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    input: &[ast::FieldInit],
) -> TypeResult<'hir> {
    for field in input.iter() {
        let _ = typecheck_expr(hir, emit, proc, Expectation::None, field.expr);
    }
    return TypeResult::new(hir::Type::Error, hir_build::EXPR_ERROR);
}

//==================== CALL-LIKE INPUT CHECK ====================

fn input_range(input: &ast::Input) -> TextRange {
    if input.exprs.is_empty() {
        input.range
    } else {
        let end = input.range.end();
        TextRange::new(end - 1.into(), end)
    }
}

fn input_opt_range(input: Option<&ast::Input>, default: TextRange) -> TextRange {
    if let Some(input) = input {
        input_range(input)
    } else {
        default
    }
}

fn error_unexpected_variant_arg_count(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    expected_count: usize,
    input_count: usize,
    error_src: SourceRange,
    info_src: SourceRange,
    info_msg: &'static str,
) {
    let plural_end = if expected_count == 1 { "" } else { "s" };
    emit.error(ErrorComp::new(
        format!("expected {expected_count} argument{plural_end}, found {input_count}"),
        error_src,
        Info::new(info_msg, info_src),
    ));
}

fn error_unexpected_call_arg_count(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    expected_count: usize,
    input_count: usize,
    is_variadic: bool,
    error_src: SourceRange,
    info: Option<(SourceRange, &'static str)>,
) {
    let at_least = if is_variadic { " at least" } else { "" };
    let plural_end = if expected_count == 1 { "" } else { "s" };
    let info = match info {
        Some((src, msg)) => Info::new(msg, src),
        None => None,
    };
    emit.error(ErrorComp::new(
        format!("expected{at_least} {expected_count} argument{plural_end}, found {input_count}"),
        error_src,
        info,
    ));
}

fn check_call_direct<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    proc_id: hir::ProcID,
    input: &ast::Input,
) -> TypeResult<'hir> {
    let data = hir.registry().proc_data(proc_id);

    let input_count = input.exprs.len();
    let expected_count = data.params.len();
    let is_variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
    let wrong_count = (is_variadic && (input_count < expected_count))
        || (!is_variadic && (input_count != expected_count));

    if wrong_count {
        let input_range = input_range(input);
        error_unexpected_call_arg_count(
            emit,
            proc.origin(),
            expected_count,
            input_count,
            is_variadic,
            SourceRange::new(proc.origin(), input_range),
            Some((
                SourceRange::new(data.origin_id, data.name.range),
                "procedure defined here",
            )),
        );
    }

    let mut values = Vec::with_capacity(data.params.len());
    for (idx, &expr) in input.exprs.iter().enumerate() {
        //@expect src, id with duplicates problem
        let expect = match data.params.get(idx) {
            Some(param) => Expectation::HasType(param.ty, None),
            None => Expectation::None,
        };
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        values.push(expr_res.expr);
    }
    let values = emit.arena.alloc_slice(&values);

    let call_direct = hir::Expr::CallDirect {
        proc_id,
        input: values,
    };
    let call_direct = emit.arena.alloc(call_direct);
    TypeResult::new(data.return_ty, call_direct)
}

fn check_call_indirect<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target_res: TypeResult<'hir>,
    target_range: TextRange,
    input: &ast::Input,
) -> TypeResult<'hir> {
    let proc_ty = match target_res.ty {
        hir::Type::Error => return error_result_default_check_input(hir, emit, proc, input),
        hir::Type::Procedure(proc_ty) => proc_ty,
        _ => {
            emit.error(ErrorComp::new(
                format!(
                    "cannot call value of type `{}`",
                    type_format(hir, emit, target_res.ty).as_str()
                ),
                SourceRange::new(proc.origin(), target_range),
                None,
            ));
            return error_result_default_check_input(hir, emit, proc, input);
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
            emit,
            proc.origin(),
            expected_count,
            input_count,
            is_variadic,
            SourceRange::new(proc.origin(), input_range),
            None,
        );
    }

    let mut values = Vec::with_capacity(proc_ty.param_types.len());
    for (idx, &expr) in input.exprs.iter().enumerate() {
        let expect = match proc_ty.param_types.get(idx) {
            Some(param_ty) => Expectation::HasType(*param_ty, None),
            None => Expectation::None,
        };
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        values.push(expr_res.expr);
    }
    let values = emit.arena.alloc_slice(&values);

    let indirect = hir::CallIndirect {
        proc_ty,
        input: values,
    };
    let indirect = emit.arena.alloc(indirect);
    let call_indirect = hir::Expr::CallIndirect {
        target: target_res.expr,
        indirect,
    };
    let call_indirect = emit.arena.alloc(call_indirect);
    TypeResult::new(proc_ty.return_ty, call_indirect)
}

fn check_variant_input_opt<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    enum_id: hir::EnumID,
    variant_id: hir::EnumVariantID,
    input: Option<&ast::Input>,
    error_range: TextRange,
) -> TypeResult<'hir> {
    let data = hir.registry().enum_data(enum_id);
    let variant = data.variant(variant_id);

    let value_types = match variant.kind {
        hir::VariantKind::Default(_) => &[],
        hir::VariantKind::Constant(_) => &[],
        hir::VariantKind::HasValues(types) => types,
    };

    let input_count = input.map(|i| i.exprs.len()).unwrap_or(0);
    let expected_count = value_types.len();

    if input_count != expected_count {
        let input_range = input_opt_range(input, error_range);
        error_unexpected_variant_arg_count(
            emit,
            proc.origin(),
            expected_count,
            input_count,
            SourceRange::new(proc.origin(), input_range),
            SourceRange::new(data.origin_id, variant.name.range),
            "variant defined here",
        );
    }

    let input = if let Some(input) = input {
        let mut values = Vec::with_capacity(input.exprs.len());
        for (idx, &expr) in input.exprs.iter().enumerate() {
            //@expect src, id with duplicates problem
            let expect = match value_types.get(idx) {
                Some(ty) => Expectation::HasType(*ty, None),
                None => Expectation::None,
            };
            let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
            values.push(expr_res.expr);
        }
        let values = emit.arena.alloc_slice(&values);
        let values = emit.arena.alloc(values);
        Some(values)
    } else {
        None
    };

    //@deal with empty value or type list like () both on definition & expression
    //@rename to hir::Expr::Variant? mass rename pass later on, also VariantID and FieldID etc..
    let variant_expr = hir::Expr::EnumVariant {
        enum_id,
        variant_id,
        input,
    };
    let variant_expr = emit.arena.alloc(variant_expr);
    TypeResult::new(hir::Type::Enum(enum_id), variant_expr)
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

enum ResolvedPath {
    None,
    Variable(VariableID),
    Symbol(SymbolKind, SourceRange),
}

fn path_resolve<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> (ResolvedPath, usize) {
    let name = path.names.first().cloned().expect("non empty path");

    if let Some(proc) = proc {
        if let Some(var_id) = proc.find_variable(name.id) {
            return (ResolvedPath::Variable(var_id), 0);
        }
    }

    let (module_id, name) = match hir.symbol_from_scope(origin_id, origin_id, name) {
        Ok((kind, source)) => {
            let next_name = path.names.get(1).cloned();
            match (kind, next_name) {
                (SymbolKind::Module(module_id), Some(name)) => (module_id, name),
                _ => return (ResolvedPath::Symbol(kind, source), 0),
            }
        }
        Err(error) => {
            emit.error(error);
            return (ResolvedPath::None, 0);
        }
    };

    match hir.symbol_from_scope(origin_id, module_id, name) {
        Ok((kind, source)) => (ResolvedPath::Symbol(kind, source), 1),
        Err(error) => {
            emit.error(error);
            (ResolvedPath::None, 1)
        }
    }
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let ty = match resolved {
        ResolvedPath::None => return hir::Type::Error,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => {
                    SourceRange::new(proc.origin(), proc.get_local(id).name.range)
                }
                VariableID::Param(id) => {
                    SourceRange::new(proc.origin(), proc.get_param(id).name.range)
                }
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::new(
                format!("expected type, found local `{}`", hir.name_str(name.id)),
                SourceRange::new(origin_id, name.range),
                Info::new("defined here", source),
            ));
            return hir::Type::Error;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::new(
                    format!(
                        "expected type, found {} `{}`",
                        kind.kind_name(),
                        hir.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", source),
                ));
                return hir::Type::Error;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::new(
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
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: ModuleID,
    path: &ast::Path,
) -> Option<hir::StructID> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let struct_id = match resolved {
        ResolvedPath::None => return None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => {
                    SourceRange::new(proc.origin(), proc.get_local(id).name.range)
                }
                VariableID::Param(id) => {
                    SourceRange::new(proc.origin(), proc.get_param(id).name.range)
                }
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::new(
                format!(
                    "expected struct type, found local `{}`",
                    hir.name_str(name.id)
                ),
                SourceRange::new(origin_id, name.range),
                Info::new("defined here", source),
            ));
            return None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Struct(id) => Some(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::new(
                    format!(
                        "expected struct type, found {} `{}`",
                        kind.kind_name(),
                        hir.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", source),
                ));
                return None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::new(
                "unexpected path segment",
                SourceRange::new(origin_id, range),
                None,
            ));
        }
    }
    struct_id
}

pub enum ValueID {
    None,
    Proc(hir::ProcID),
    Enum(hir::EnumID, hir::EnumVariantID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

pub fn path_resolve_value<'hir, 'ast>(
    hir: &HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: ModuleID,
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
                        emit.error(ErrorComp::new(
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
                    let enum_data = hir.registry().enum_data(id);
                    if let Some((variant_id, ..)) = enum_data.find_variant(variant_name.id) {
                        if let Some(remaining) = path.names.get(name_idx + 2..) {
                            if let (Some(first), Some(last)) = (remaining.first(), remaining.last())
                            {
                                let range = TextRange::new(first.range.start(), last.range.end());
                                emit.error(ErrorComp::new(
                                    "unexpected path segment",
                                    SourceRange::new(origin_id, range),
                                    None,
                                ));
                            }
                        }
                        return (ValueID::Enum(id, variant_id), &[]);
                    } else {
                        emit.error(ErrorComp::new(
                            format!(
                                "enum variant `{}` is not found",
                                hir.name_str(variant_name.id)
                            ),
                            SourceRange::new(origin_id, variant_name.range),
                            Info::new("enum defined here", source),
                        ));
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    emit.error(ErrorComp::new(
                        format!(
                            "expected value, found {} `{}`",
                            kind.kind_name(),
                            hir.name_str(name.id)
                        ),
                        SourceRange::new(origin_id, name.range),
                        Info::new("defined here", source),
                    ));
                    return (ValueID::None, &[]);
                }
            }
            SymbolKind::Const(id) => ValueID::Const(id),
            SymbolKind::Global(id) => ValueID::Global(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::new(
                    format!(
                        "expected value, found {} `{}`",
                        kind.kind_name(),
                        hir.name_str(name.id)
                    ),
                    SourceRange::new(origin_id, name.range),
                    Info::new("defined here", source),
                ));
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

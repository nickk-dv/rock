use super::hir_build::{self, HirData, HirEmit, SymbolKind};
use super::pass_4;
use super::proc_scope::{BlockEnter, DeferStatus, LoopStatus, ProcScope, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, Info, SourceRange, WarningComp};
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
            emit.error(ErrorComp::new(
                format!(
                    "unknown attribute, only #[{}] is allowed here",
                    expected.as_str()
                ),
                hir.src(origin_id, attr.range),
                None,
            ));
        }
        _ => {
            emit.error(ErrorComp::new(
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

    //@procedure attibutes and flag checks are fragile, rework and stabilize 06.06.24
    // think about #[builtin] will it require empty block? probably to differentiate from external
    if item.block.is_some() {
        if data.is_variadic {
            emit.error(ErrorComp::new(
                "only external procedures can be variadic, remove `..` from parameter list",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    if data.is_test {
        if item.block.is_none() {
            emit.error(ErrorComp::new(
                "procedures with #[test] attribute cannot be external",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if !data.params.is_empty() {
            emit.error(ErrorComp::new(
                "procedures with #[test] attribute cannot have any input parameters",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if !matches!(data.return_ty, hir::Type::Basic(BasicType::Void)) {
            emit.error(ErrorComp::new(
                "procedures with #[test] attribute can only return `void`",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    // main entry point cannot be external or a test (is_main detection isnt done yet) @27.04.24
    if let Some(block) = item.block {
        let file_id = hir.registry().module_data(data.origin_id).file_id;
        let expect_source = match item.return_ty {
            Some(return_ty) => SourceRange::new(return_ty.range, file_id),
            None => SourceRange::new(data.name.range, file_id),
        };
        let return_expect = TypeExpectation::new(data.return_ty, Some(expect_source));

        let mut proc = ProcScope::new(data, return_expect);
        let block_res =
            typecheck_block(hir, emit, &mut proc, return_expect, block, BlockEnter::None);
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
        (hir::Type::Error, ..) => true,
        (.., hir::Type::Error) => true,
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
            (proc_ty.params.len() == proc_ty2.params.len())
                && (proc_ty.is_variadic == proc_ty2.is_variadic)
                && type_matches(hir, emit, proc_ty.return_ty, proc_ty2.return_ty)
                && (0..proc_ty.params.len())
                    .all(|idx| type_matches(hir, emit, proc_ty.params[idx], proc_ty2.params[idx]))
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            if slice2.mutt == ast::Mut::Mutable {
                type_matches(hir, emit, slice.elem_ty, slice2.elem_ty)
            } else {
                slice.mutt == slice2.mutt && type_matches(hir, emit, slice.elem_ty, slice2.elem_ty)
            }
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
        hir::Type::Error => "<unknown>".into(),
        hir::Type::Basic(basic) => basic.as_str().to_string(),
        hir::Type::Enum(id) => hir.name_str(hir.registry().enum_data(id).name.id).into(),
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

#[derive(Copy, Clone)]
pub struct TypeExpectation<'hir> {
    pub ty: hir::Type<'hir>,
    source: Option<SourceRange>,
}

impl<'hir> TypeExpectation<'hir> {
    pub const VOID: TypeExpectation<'static> = TypeExpectation::new(hir::Type::VOID, None);
    pub const BOOL: TypeExpectation<'static> = TypeExpectation::new(hir::Type::BOOL, None);
    pub const USIZE: TypeExpectation<'static> = TypeExpectation::new(hir::Type::USIZE, None);
    pub const NOTHING: TypeExpectation<'static> = TypeExpectation::new(hir::Type::Error, None);

    pub const fn new(ty: hir::Type<'hir>, source: Option<SourceRange>) -> TypeExpectation {
        TypeExpectation { ty, source }
    }
}

pub fn check_type_expectation<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    from_range: TextRange,
    expect: TypeExpectation<'hir>,
    found_ty: hir::Type<'hir>,
) -> bool {
    if type_matches(hir, emit, expect.ty, found_ty) {
        return false;
    }

    let info = if let Some(source) = expect.source {
        Info::new("expected due to this", source)
    } else {
        None
    };

    emit.error(ErrorComp::new(
        format!(
            "type mismatch: expected `{}`, found `{}`",
            type_format(hir, emit, expect.ty),
            type_format(hir, emit, found_ty)
        ),
        hir.src(origin_id, from_range),
        info,
    ));
    return true;
}

pub struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    pub expr: &'hir hir::Expr<'hir>,
    ignore: bool,
    errored: bool,
    diverges: bool,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: false,
            errored: false,
            diverges: false,
        }
    }

    fn new_ignore_typecheck(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: true,
            errored: false,
            diverges: false,
        }
    }

    fn new_div(
        ty: hir::Type<'hir>,
        expr: &'hir hir::Expr<'hir>,
        diverges: bool,
    ) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: false,
            errored: false,
            diverges,
        }
    }

    fn new_ignore_typecheck_div(
        ty: hir::Type<'hir>,
        expr: &'hir hir::Expr<'hir>,
        diverges: bool,
    ) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: true,
            errored: false,
            diverges,
        }
    }
}

struct BlockResult<'hir> {
    ty: hir::Type<'hir>,
    block: hir::Block<'hir>,
    tail_range: Option<TextRange>,
    diverges: bool,
}

impl<'hir> BlockResult<'hir> {
    fn new(
        ty: hir::Type<'hir>,
        block: hir::Block<'hir>,
        tail_range: Option<TextRange>,
        diverges: bool,
    ) -> BlockResult<'hir> {
        BlockResult {
            ty,
            block,
            tail_range,
            diverges,
        }
    }

    fn into_type_result(self, emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty: self.ty,
            expr: emit.arena.alloc(hir::Expr::Block { block: self.block }),
            ignore: true,
            errored: false, //@not used
            diverges: self.diverges,
        }
    }
}

#[must_use]
pub fn typecheck_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: TypeExpectation<'hir>,
    expr: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let mut expr_res = match expr.kind {
        ast::ExprKind::LitNull => typecheck_lit_null(emit),
        ast::ExprKind::LitBool { val } => typecheck_lit_bool(emit, val),
        ast::ExprKind::LitInt { val } => typecheck_lit_int(emit, expect, val),
        ast::ExprKind::LitFloat { val } => typecheck_lit_float(emit, expect, val),
        ast::ExprKind::LitChar { val } => typecheck_lit_char(emit, val),
        ast::ExprKind::LitString { id, c_string } => typecheck_lit_string(emit, id, c_string),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_, expr.range),
        ast::ExprKind::Block { block } => {
            typecheck_block(hir, emit, proc, expect, *block, BlockEnter::None)
                .into_type_result(emit)
        }
        ast::ExprKind::Match { match_ } => {
            typecheck_match(hir, emit, proc, expect, match_, expr.range)
        }
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
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, *ty, expr.range),
        ast::ExprKind::Item { path } => typecheck_item(hir, emit, proc, path),
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init)
        }
        ast::ExprKind::ArrayInit { input } => {
            typecheck_array_init(hir, emit, proc, expect, input, expr.range)
        }
        ast::ExprKind::ArrayRepeat { expr, len } => {
            typecheck_array_repeat(hir, emit, proc, expect, expr, len)
        }
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(hir, emit, proc, mutt, rhs),
        ast::ExprKind::Unary { op, op_range, rhs } => {
            typecheck_unary(hir, emit, proc, expect, op, op_range, rhs)
        }
        ast::ExprKind::Binary { op, op_range, bin } => {
            typecheck_binary(hir, emit, proc, expect, op, op_range, bin)
        }
    };

    if !expr_res.ignore {
        expr_res.errored =
            check_type_expectation(hir, emit, proc.origin(), expr.range, expect, expr_res.ty);
    }

    expr_res
}

fn typecheck_lit_null<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    let value = hir::ConstValue::Null;
    TypeResult::new(
        hir::Type::Basic(BasicType::Rawptr),
        emit.arena.alloc(hir::Expr::Const { value }),
    )
}

fn typecheck_lit_bool<'hir>(emit: &mut HirEmit<'hir>, val: bool) -> TypeResult<'hir> {
    let value = hir::ConstValue::Bool { val };
    TypeResult::new(
        hir::Type::Basic(BasicType::Bool),
        emit.arena.alloc(hir::Expr::Const { value }),
    )
}

fn typecheck_lit_int<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: TypeExpectation<'hir>,
    val: u64,
) -> TypeResult<'hir> {
    let lit_type = coerce_int_type(expect.ty);
    let value = hir::ConstValue::Int {
        val,
        neg: false,
        ty: Some(lit_type),
    };
    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::Const { value }),
    )
}

fn typecheck_lit_float<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: TypeExpectation<'hir>,
    val: f64,
) -> TypeResult<'hir> {
    let lit_type = coerce_float_type(expect.ty);
    let value = hir::ConstValue::Float {
        val,
        ty: Some(lit_type),
    };
    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::Const { value }),
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
    let value = hir::ConstValue::Char { val };
    TypeResult::new(
        hir::Type::Basic(BasicType::Char),
        emit.arena.alloc(hir::Expr::Const { value }),
    )
}

fn typecheck_lit_string<'hir>(
    emit: &mut HirEmit<'hir>,
    id: InternID,
    c_string: bool,
) -> TypeResult<'hir> {
    let value = hir::ConstValue::String { id, c_string };
    TypeResult::new(
        alloc_string_lit_type(emit, c_string),
        emit.arena.alloc(hir::Expr::Const { value }),
    )
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
    mut expect: TypeExpectation<'hir>,
    if_: &ast::If<'_>,
    if_range: TextRange,
) -> TypeResult<'hir> {
    let mut if_type: hir::Type;

    let entry = {
        let cond_res = typecheck_expr(hir, emit, proc, TypeExpectation::BOOL, if_.entry.cond);
        let block_res = typecheck_block(hir, emit, proc, expect, if_.entry.block, BlockEnter::None);
        if_type = block_res.ty;
        if expect.ty.is_error() {
            expect = TypeExpectation::new(
                block_res.ty,
                block_res
                    .tail_range
                    .map(|range| hir.src(proc.origin(), range)),
            );
        }
        hir::Branch {
            cond: cond_res.expr,
            block: block_res.block,
        }
    };

    let branches = {
        let mut branches = Vec::with_capacity(if_.branches.len());
        for &branch in if_.branches {
            let cond_res = typecheck_expr(hir, emit, proc, TypeExpectation::BOOL, branch.cond);
            let block_res =
                typecheck_block(hir, emit, proc, expect, branch.block, BlockEnter::None);

            branches.push(hir::Branch {
                cond: cond_res.expr,
                block: block_res.block,
            });

            if if_type.is_error() {
                if_type = block_res.ty;
            }
            if expect.ty.is_error() {
                expect = TypeExpectation::new(
                    block_res.ty,
                    block_res
                        .tail_range
                        .map(|range| hir.src(proc.origin(), range)),
                );
            }
        }
        emit.arena.alloc_slice(&branches)
    };

    let else_block = if let Some(else_block) = if_.else_block {
        let block_res = typecheck_block(hir, emit, proc, expect, else_block, BlockEnter::None);

        if if_type.is_error() {
            if_type = block_res.ty;
        }
        Some(block_res.block)
    } else {
        None
    };

    if else_block.is_none() && !if_type.is_error() && !if_type.is_void() {
        emit.error(ErrorComp::new(
            "`if` expression is missing an `else` block\n`if` without `else` evaluates to `void` and cannot return a value",
            hir.src(proc.origin(), if_range),
            None,
        ));
    }

    let if_ = emit.arena.alloc(hir::If {
        entry,
        branches,
        else_block,
    });
    let if_expr = emit.arena.alloc(hir::Expr::If { if_ });
    TypeResult::new_ignore_typecheck(if_type, if_expr)
}

fn typecheck_match<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    mut expect: TypeExpectation<'hir>,
    match_: &ast::Match<'_>,
    match_range: TextRange,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, match_.on_expr);
    check_match_compatibility(hir, emit, proc.origin(), on_res.ty, match_.on_expr.range);

    let mut match_type = hir::Type::Error;
    let mut diverges = true;
    let mut check_exaust = true;
    let pat_expect = TypeExpectation::new(
        on_res.ty,
        Some(hir.src(proc.origin(), match_.on_expr.range)),
    );

    let mut arms = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms.iter() {
        let (value, value_id) =
            pass_4::resolve_const_expr(hir, emit, proc.origin(), pat_expect, arm.pat);
        let value_res = typecheck_expr(hir, emit, proc, expect, arm.expr);

        //@check if anything in pattern errored?
        if value == hir::ConstValue::Error {
            check_exaust = false;
        }
        if !value_res.diverges {
            diverges = false;
        }

        arms.push(hir::MatchArm {
            pat: value_id,
            block: hir::Block {
                stmts: emit
                    .arena
                    .alloc_slice(&[hir::Stmt::ExprTail(value_res.expr)]),
            },
            unreachable: false,
        });

        if match_type.is_error() {
            match_type = value_res.ty;
        }
        if expect.ty.is_error() {
            expect =
                TypeExpectation::new(value_res.ty, Some(hir.src(proc.origin(), arm.expr.range)))
        }
    }

    let mut fallback = if let Some(fallback) = match_.fallback {
        let value_res = typecheck_expr(hir, emit, proc, expect, fallback);

        if match_type.is_error() {
            match_type = value_res.ty;
        }

        Some(hir::Block {
            stmts: emit
                .arena
                .alloc_slice(&[hir::Stmt::ExprTail(value_res.expr)]),
        })
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

    let match_hir = emit.arena.alloc(hir::Match {
        on_expr: on_res.expr,
        arms,
        fallback,
    });
    let match_expr = emit.arena.alloc(hir::Expr::Match { match_: match_hir });

    if match_.arms.is_empty() && match_.fallback.is_none() {
        TypeResult::new_ignore_typecheck_div(
            hir::Type::Basic(BasicType::Never),
            match_expr,
            diverges,
        )
    } else {
        TypeResult::new_ignore_typecheck_div(match_type, match_expr, diverges)
    }
}

//@different enum variants 01.06.24
// could have same value and result in
// error in llvm ir generation, not checked currently
fn check_match_exhaust<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    arms: &mut Vec<hir::MatchArm<'hir>>,
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
                                    hir.src(proc.origin(), ast_arm.pat.0.range),
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
                                    hir.src(proc.origin(), ast_arm.pat.0.range),
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
                        hir.src(proc.origin(), match_ast.fallback_range),
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
                    hir.src(
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
                    hir::ConstValue::EnumVariant { variant_id, .. } => {
                        if !variants_covered[variant_id.index()] {
                            variants_covered[variant_id.index()] = true;
                        } else {
                            let ast_arm = match_ast.arms[idx];
                            arm.unreachable = true;
                            emit.warning(WarningComp::new(
                                "unreachable pattern",
                                hir.src(proc.origin(), ast_arm.pat.0.range),
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
                        hir.src(proc.origin(), match_ast.fallback_range),
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
                        hir.src(
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
    let target_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, target);
    let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_res.ty, name);

    match kind {
        FieldKind::Error => TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR),
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
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            if let Some((field_id, field)) = data.find_field(name.id) {
                (field.ty, FieldKind::Field(id, field_id))
            } else {
                emit.error(ErrorComp::new(
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
                    emit.error(ErrorComp::new(
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
            emit.error(ErrorComp::new(
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
    let target_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, target);
    let index_res = typecheck_expr(hir, emit, proc, TypeExpectation::USIZE, index);

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
            emit.error(ErrorComp::new(
                format!(
                    "cannot index value of type `{}`",
                    type_format(hir, emit, target_res.ty)
                ),
                hir.src(proc.origin(), expr_range),
                Info::new(
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
    let target_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, target);

    let lower = slice.lower.map(|lower| {
        let lower_res = typecheck_expr(hir, emit, proc, TypeExpectation::USIZE, lower);
        lower_res.expr
    });
    let upper = match slice.upper {
        ast::SliceRangeEnd::Unbounded => hir::SliceRangeEnd::Unbounded,
        ast::SliceRangeEnd::Exclusive(upper) => {
            let upper_res = typecheck_expr(hir, emit, proc, TypeExpectation::USIZE, upper);
            hir::SliceRangeEnd::Exclusive(upper_res.expr)
        }
        ast::SliceRangeEnd::Inclusive(upper) => {
            let upper_res = typecheck_expr(hir, emit, proc, TypeExpectation::USIZE, upper);
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
            emit.error(ErrorComp::new(
                format!(
                    "cannot slice value of type `{}`",
                    type_format(hir, emit, target_res.ty)
                ),
                hir.src(proc.origin(), expr_range),
                Info::new(
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
    let target_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, target);

    match target_res.ty {
        hir::Type::Error => {}
        hir::Type::Procedure(proc_ty) => {
            // both direct and indirect return proc_ty
            // it can be used for input checks

            let direct_id = match *target_res.expr {
                hir::Expr::Const { value } => match value {
                    hir::ConstValue::Procedure { proc_id } => Some(proc_id),
                    _ => None,
                },
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
                    Info::new(
                        "calling this procedure",
                        hir.src(data.origin_id, data.name.range),
                    )
                } else {
                    None
                };

                //@plular form for argument`s` only needed if != 1
                emit.error(ErrorComp::new(
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
                    Some(param) => TypeExpectation::new(*param, None),
                    None => TypeExpectation::NOTHING,
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

            return TypeResult::new_div(
                proc_ty.return_ty,
                emit.arena.alloc(call_expr),
                proc_ty.return_ty.is_never(),
            );
        }
        _ => {
            emit.error(ErrorComp::new(
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
        let _ = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, expr);
    }
    TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR)
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
            BasicType::F16 | BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
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
    let target_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, target);
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
        Some(size) => {
            let value = hir::ConstValue::Int {
                val: size.size(),
                neg: false,
                ty: Some(BasicType::Usize),
            };
            emit.arena.alloc(hir::Expr::Const { value })
        }
        None => hir_build::ERROR_EXPR,
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
            return TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR);
        }
        ValueID::Proc(proc_id) => {
            let data = hir.registry().proc_data(proc_id);

            //@creating proc type each time its encountered / called, waste of arena memory 25.05.24
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
                hir::Type::Procedure(emit.arena.alloc(proc_ty)),
                emit.arena.alloc(hir::Expr::Const {
                    value: hir::ConstValue::Procedure { proc_id },
                }),
            );
        }
        ValueID::Enum(enum_id, variant_id) => {
            let value = hir::ConstValue::EnumVariant {
                enum_id,
                variant_id,
            };
            return TypeResult::new(
                hir::Type::Enum(enum_id),
                emit.arena.alloc(hir::Expr::Const { value }),
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

    //@everything below is copy-paste from regular typecheck field access 16.05.24
    // de-duplicate later
    let mut target = item_res.expr;
    let mut target_ty = item_res.ty;

    for &name in field_names {
        let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_ty, name);

        match kind {
            FieldKind::Error => return TypeResult::new(hir::Type::Error, target),
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
    let struct_id = path_resolve_struct(hir, emit, Some(proc), proc.origin(), struct_init.path);
    let struct_name = *struct_init.path.names.last().expect("non empty path");

    match struct_id {
        None => {
            for input in struct_init.input {
                let _ = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, input.expr);
            }
            TypeResult::new(hir::Type::Error, hir_build::ERROR_EXPR)
        }
        Some(struct_id) => {
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
                    let expect = TypeExpectation::new(field.ty, None);
                    let input_res = typecheck_expr(hir, emit, proc, expect, input.expr);

                    if let FieldStatus::Init(range) = field_status[field_id.index()] {
                        emit.error(ErrorComp::new(
                            format!(
                                "field `{}` was already initialized",
                                hir.name_str(input.name.id),
                            ),
                            hir.src(proc.origin(), input.name.range),
                            Info::new("initialized here", hir.src(data.origin_id, range)),
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
                    emit.error(ErrorComp::new(
                        format!("field `{}` is not found", hir.name_str(input.name.id),),
                        hir.src(proc.origin(), input.name.range),
                        Info::new(
                            "struct defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    ));
                    let _ = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, input.expr);
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

                emit.error(ErrorComp::new(
                    message,
                    hir.src(proc.origin(), struct_name.range),
                    Info::new(
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
    mut expect: TypeExpectation<'hir>,
    input: &[&ast::Expr<'_>],
    array_range: TextRange,
) -> TypeResult<'hir> {
    let mut expect_array_ty = None;
    expect = match expect.ty {
        hir::Type::ArrayStatic(array) => {
            expect_array_ty = Some(array);
            TypeExpectation::new(array.elem_ty, expect.source)
        }
        _ => TypeExpectation::new(hir::Type::Error, None),
    };
    let mut elem_ty = hir::Type::Error;

    let input = {
        let mut input_res = Vec::with_capacity(input.len());
        for &expr in input.iter() {
            let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
            input_res.push(expr_res.expr);

            if elem_ty.is_error() {
                elem_ty = expr_res.ty;
            }
            if expect.ty.is_error() {
                expect = TypeExpectation::new(expr_res.ty, Some(hir.src(proc.origin(), expr.range)))
            }
        }
        emit.arena.alloc_slice(&input_res)
    };

    let elem_ty = if input.is_empty() {
        if let Some(array_ty) = expect_array_ty {
            array_ty.elem_ty
        } else {
            emit.error(ErrorComp::new(
                "cannot infer type of empty array",
                hir.src(proc.origin(), array_range),
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
    mut expect: TypeExpectation<'hir>,
    expr: &ast::Expr,
    len: ast::ConstExpr,
) -> TypeResult<'hir> {
    expect = match expect.ty {
        hir::Type::ArrayStatic(array) => TypeExpectation::new(array.elem_ty, expect.source),
        _ => TypeExpectation::new(hir::Type::Error, None),
    };

    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);

    //@this is duplicated here and in pass_3::type_resolve 09.05.24
    let (value, _) =
        pass_4::resolve_const_expr(hir, emit, proc.origin(), TypeExpectation::USIZE, len);

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
    let rhs_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, rhs);
    let adressability = get_expr_addressability(hir, proc, rhs_res.expr);

    match adressability {
        Addressability::Unknown => {} //@ & to error should be also Error? 16.05.24
        Addressability::Constant => {
            emit.error(ErrorComp::new(
                "cannot get reference to a constant, you can use `global` instead",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::new(
                "cannot get reference to a slice field, slice itself cannot be modified",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::Temporary => {
            emit.error(ErrorComp::new(
                "cannot get reference to a temporary value",
                hir.src(proc.origin(), rhs.range),
                None,
            ));
        }
        Addressability::TemporaryImmutable => {
            if mutt == ast::Mut::Mutable {
                emit.error(ErrorComp::new(
                    "cannot get mutable reference to this temporary value, only immutable `&` is allowed",
                    hir.src(proc.origin(), rhs.range),
                    None,
                ));
            }
        }
        Addressability::Addressable(rhs_mutt, src) => {
            if mutt == ast::Mut::Mutable && rhs_mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::new(
                    "cannot get mutable reference to an immutable variable",
                    hir.src(proc.origin(), rhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::new(
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
        hir::Expr::CallDirect { .. } => Addressability::Temporary,
        hir::Expr::CallIndirect { .. } => Addressability::Temporary,
        hir::Expr::StructInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayInit { .. } => Addressability::TemporaryImmutable,
        hir::Expr::ArrayRepeat { .. } => Addressability::TemporaryImmutable,
        hir::Expr::Address { .. } => Addressability::Temporary,
        hir::Expr::Unary { op, rhs } => match op {
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
    expect: TypeExpectation<'hir>,
    op: ast::UnOp,
    op_range: TextRange,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_expect = match op {
        ast::UnOp::Neg => expect,
        ast::UnOp::BitNot => expect,
        ast::UnOp::LogicNot => TypeExpectation::BOOL,
        ast::UnOp::Deref => TypeExpectation::NOTHING,
    };
    let rhs_res = typecheck_expr(hir, emit, proc, rhs_expect, rhs);

    let compatable = check_un_op_compatibility(hir, emit, proc.origin(), rhs_res.ty, op, op_range);

    let unary_ty = if compatable {
        match op {
            ast::UnOp::Neg => rhs_res.ty,
            ast::UnOp::BitNot => rhs_res.ty,
            ast::UnOp::LogicNot => hir::Type::BOOL,
            ast::UnOp::Deref => match rhs_res.ty {
                hir::Type::Reference(ref_ty, _) => *ref_ty,
                _ => hir::Type::Error,
            },
        }
    } else {
        hir::Type::Error
    };

    let unary_expr = hir::Expr::Unary {
        op,
        rhs: rhs_res.expr,
    };
    TypeResult::new(unary_ty, emit.arena.alloc(unary_expr))
}

//@bin << >> should allow any integer type on the right, same sized int?
// no type expectation for this is possible 25.05.24
fn typecheck_binary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: TypeExpectation<'hir>,
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
        | ast::BinOp::GreaterEq => TypeExpectation::NOTHING,
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => TypeExpectation::BOOL,
        ast::BinOp::Range | ast::BinOp::RangeInc => TypeExpectation::USIZE,
        _ => expect,
    };
    let lhs_res = typecheck_expr(hir, emit, proc, lhs_expect, bin.lhs);

    let compatible = check_bin_op_compatibility(hir, emit, proc.origin(), lhs_res.ty, op, op_range);

    let rhs_expect = match op {
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => TypeExpectation::BOOL,
        ast::BinOp::Range | ast::BinOp::RangeInc => TypeExpectation::USIZE,
        _ => TypeExpectation::new(lhs_res.ty, Some(hir.src(proc.origin(), bin.lhs.range))),
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
            ast::BinOp::Range | ast::BinOp::RangeInc => {
                panic!("pass5 bin_op range doesnt produce Range struct type yet")
            }
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
    origin_id: hir::ModuleID,
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
                type_format(hir, emit, ty)
            ),
            hir.src(origin_id, range),
            None,
        ));
    }
}

fn check_un_op_compatibility<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    rhs_ty: hir::Type,
    op: ast::UnOp,
    op_range: TextRange,
) -> bool {
    if rhs_ty.is_error() {
        return false;
    }

    let compatible = match op {
        ast::UnOp::Neg => match rhs_ty {
            hir::Type::Basic(basic) => match BasicTypeKind::new(basic) {
                BasicTypeKind::SignedInt | BasicTypeKind::Float => true,
                _ => false,
            },
            _ => false,
        },
        ast::UnOp::BitNot => match rhs_ty {
            hir::Type::Basic(basic) => BasicTypeKind::new(basic).is_integer(),
            _ => false,
        },
        ast::UnOp::LogicNot => matches!(rhs_ty, hir::Type::Basic(BasicType::Bool)),
        ast::UnOp::Deref => matches!(rhs_ty, hir::Type::Reference(..)),
    };

    if !compatible {
        emit.error(ErrorComp::new(
            format!(
                "cannot apply unary operator `{}` on value of type `{}`",
                op.as_str(),
                type_format(hir, emit, rhs_ty)
            ),
            hir.src(origin_id, op_range),
            None,
        ));
    }
    compatible
}

fn check_bin_op_compatibility<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
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
        ast::BinOp::Range | ast::BinOp::RangeInc => {
            matches!(lhs_ty, hir::Type::Basic(BasicType::Usize))
        }
    };

    if !compatible {
        emit.error(ErrorComp::new(
            format!(
                "cannot apply binary operator `{}` on value of type `{}`",
                op.as_str(),
                type_format(hir, emit, lhs_ty)
            ),
            hir.src(origin_id, op_range),
            None,
        ));
    }
    compatible
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: TypeExpectation<'hir>,
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
                if let Some(stmt_res) = typecheck_break(hir, emit, proc, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Continue => {
                if let Some(stmt_res) = typecheck_continue(hir, emit, proc, stmt.range) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Return(expr) => {
                if let Some(stmt_res) = typecheck_return(hir, emit, proc, stmt.range, expr) {
                    let diverges = proc.check_stmt_diverges(hir, emit, true, stmt.range);
                    (stmt_res, diverges)
                } else {
                    continue;
                }
            }
            ast::StmtKind::Defer(block) => {
                //@defer can behave strangely with diverges checks since it inherits diverges 29.05.24
                // from currently top block, while defer itself can be triggered multiple times in different locations
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = typecheck_defer(hir, emit, proc, stmt.range.start(), *block);
                (stmt_res, diverges)
            }
            ast::StmtKind::Loop(loop_) => {
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Loop(typecheck_loop(hir, emit, proc, loop_));
                (stmt_res, diverges)
            }
            ast::StmtKind::Local(local) => {
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Local(typecheck_local(hir, emit, proc, local));
                (stmt_res, diverges)
            }
            ast::StmtKind::Assign(assign) => {
                let diverges = proc.check_stmt_diverges(hir, emit, false, stmt.range);
                let stmt_res = hir::Stmt::Assign(typecheck_assign(hir, emit, proc, assign));
                (stmt_res, diverges)
            }
            ast::StmtKind::ExprSemi(expr) => {
                //@error or want on expressions that arent used? 29.05.24
                // `arent used` would mean that result isnt stored anywhere?
                // but proc calls might have side effects and should always be allowed
                // eg: `20 + some_var;` `10.0 != 21.2;` `something[0] + something[1];` `call() + 10;`
                let expect = match expr.kind {
                    ast::ExprKind::If { .. }
                    | ast::ExprKind::Block { .. }
                    | ast::ExprKind::Match { .. } => TypeExpectation::VOID,
                    _ => TypeExpectation::NOTHING,
                };
                //@can diverge but expression divergence isnt implemented (if, match, explicit `never` calls like panic)
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                let stmt_res = hir::Stmt::ExprSemi(expr_res.expr);
                let diverges = proc.check_stmt_diverges(hir, emit, expr_res.diverges, stmt.range);
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

    let block_result = if let Some(block_ty) = block_ty {
        BlockResult::new(block_ty, hir_block, tail_range, proc.diverges().is_always())
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
        BlockResult::new(hir::Type::VOID, hir_block, tail_range, diverges)
    };

    proc.pop_block();
    block_result
}

/// returns `None` on invalid use of `break`
fn typecheck_break<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    match proc.loop_status() {
        LoopStatus::None => {
            emit.error(ErrorComp::new(
                "cannot use `break` outside of a loop",
                hir.src(proc.origin(), range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            emit.error(ErrorComp::new(
                "cannot use `break` in a loop that is outside of `defer`",
                hir.src(proc.origin(), range),
                None,
            ));
            None
        }
        LoopStatus::Inside => Some(hir::Stmt::Break),
    }
}

/// returns `None` on invalid use of `continue`
fn typecheck_continue<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> Option<hir::Stmt<'hir>> {
    match proc.loop_status() {
        LoopStatus::None => {
            emit.error(ErrorComp::new(
                "cannot use `continue` outside of a loop",
                hir.src(proc.origin(), range),
                None,
            ));
            None
        }
        LoopStatus::Inside_WithDefer => {
            emit.error(ErrorComp::new(
                "cannot use `continue` in a loop thats started outside of `defer`",
                hir.src(proc.origin(), range),
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
    range: TextRange,
    expr: Option<&ast::Expr>,
) -> Option<hir::Stmt<'hir>> {
    match proc.defer_status() {
        DeferStatus::None => {
            if let Some(expr) = expr {
                let expr_res = typecheck_expr(hir, emit, proc, proc.return_expect(), expr);
                Some(hir::Stmt::Return(Some(expr_res.expr)))
            } else {
                check_type_expectation(
                    hir,
                    emit,
                    proc.origin(),
                    range,
                    proc.return_expect(),
                    hir::Type::VOID,
                );
                Some(hir::Stmt::Return(None))
            }
        }
        DeferStatus::Inside(prev_defer) => {
            //@still check whats being returned? for coverage 29.05.24
            emit.error(ErrorComp::new(
                "cannot use `return` inside `defer`",
                hir.src(proc.origin(), range),
                Info::new("in this defer", hir.src(proc.origin(), prev_defer)),
            ));
            None
        }
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

    match proc.defer_status() {
        DeferStatus::None => {}
        DeferStatus::Inside(prev_defer) => {
            emit.error(ErrorComp::new(
                "`defer` statements cannot be nested",
                hir.src(proc.origin(), defer_range),
                Info::new("already in this defer", hir.src(proc.origin(), prev_defer)),
            ));
        }
    }

    let block_res = typecheck_block(
        hir,
        emit,
        proc,
        TypeExpectation::VOID,
        block,
        BlockEnter::Defer(defer_range),
    );

    let block_ref = emit.arena.alloc(block_res.block);
    hir::Stmt::Defer(block_ref)
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
            let cond_res = typecheck_expr(hir, emit, proc, TypeExpectation::BOOL, cond);
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
            let cond_res = typecheck_expr(hir, emit, proc, TypeExpectation::BOOL, cond);
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
        TypeExpectation::VOID,
        loop_.block,
        BlockEnter::Loop,
    );

    emit.arena.alloc(hir::Loop {
        kind,
        block: block_res.block,
    })
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
        hir.scope_name_defined(proc.origin(), local.name.id)
    {
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);
        true
    } else if let Some(existing_var) = proc.find_variable(local.name.id) {
        let existing = match existing_var {
            VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
            VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
        };
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);
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
                TypeExpectation::new(hir_ty, Some(hir.src(proc.origin(), ast_ty.range)))
            } else {
                TypeExpectation::NOTHING
            };

            let value_res = typecheck_expr(hir, emit, proc, expect, value);

            if ast_ty.is_some() {
                (expect.ty, Some(value_res.expr))
            } else {
                (value_res.ty, Some(value_res.expr))
            }
        }
    };

    if already_defined {
        hir::LocalID::dummy()
    } else {
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
    let lhs_res = typecheck_expr(hir, emit, proc, TypeExpectation::NOTHING, assign.lhs);
    let adressability = get_expr_addressability(hir, proc, lhs_res.expr);

    match adressability {
        Addressability::Unknown => {}
        Addressability::Constant => {
            emit.error(ErrorComp::new(
                "cannot assign to a constant",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::SliceField => {
            emit.error(ErrorComp::new(
                "cannot assign to a slice field, slice itself cannot be modified",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Temporary | Addressability::TemporaryImmutable => {
            emit.error(ErrorComp::new(
                "cannot assign to a temporary value",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
        Addressability::Addressable(mutt, src) => {
            if mutt == ast::Mut::Immutable {
                emit.error(ErrorComp::new(
                    "cannot assign to an immutable variable",
                    hir.src(proc.origin(), assign.lhs.range),
                    Info::new("variable defined here", src),
                ));
            }
        }
        Addressability::NotImplemented => {
            emit.error(ErrorComp::new(
                "addressability not implemented for this expression",
                hir.src(proc.origin(), assign.lhs.range),
                None,
            ));
        }
    }

    let rhs_expect =
        TypeExpectation::new(lhs_res.ty, Some(hir.src(proc.origin(), assign.lhs.range)));
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
            emit.error(ErrorComp::new(
                format!("expected type, found local `{}`", hir.name_str(name.id)),
                hir.src(origin_id, name.range),
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
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
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
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    ty
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
fn path_resolve_struct<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> Option<hir::StructID> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let struct_id = match resolved {
        ResolvedPath::None => return None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::new(
                format!(
                    "expected struct type, found local `{}`",
                    hir.name_str(name.id)
                ),
                hir.src(origin_id, name.range),
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
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
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
                hir.src(origin_id, range),
                None,
            ));
        }
    }
    struct_id
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
                        emit.error(ErrorComp::new(
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
                                emit.error(ErrorComp::new(
                                    "unexpected path segment",
                                    hir.src(origin_id, range),
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
                            hir.src(origin_id, variant_name.range),
                            Info::new("enum defined here", source),
                        ));
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    emit.error(ErrorComp::new(
                        format!(
                            "expected value, found {} `{}`",
                            HirData::symbol_kind_name(kind),
                            hir.name_str(name.id)
                        ),
                        hir.src(origin_id, name.range),
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
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    Info::new("defined here", source),
                ));
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

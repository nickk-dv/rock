use super::hir_builder as hb;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let item = hb.proc_ast(id);
    let data = hb.proc_data(id);

    match item.block {
        Some(block) => {
            let _ = typecheck_expr_2(
                hb,
                data.origin_id,
                BlockFlags::entry(),
                &mut ProcScope::new(id),
                data.return_ty,
                block,
            );
        }
        None => {
            //@for now having a tail directive assumes that it must be a #[c_call]
            // and this directory is only present with no block expression
            let directive_tail = item
                .directive_tail
                .expect("directive expected with no proc block");

            let directive_name = hb.name_str(directive_tail.name.id);
            if directive_name != "c_call" {
                hb.error(
                    ErrorComp::error(format!(
                        "expected a `c_call` directive, got `{}`",
                        directive_name
                    ))
                    .context(hb.src(data.origin_id, directive_tail.name.range)),
                )
            }
        }
    }
}

fn type_format(hb: &mut hb::HirBuilder, ty: hir::Type) -> String {
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
        hir::Type::Enum(id) => hb.name_str(hb.enum_data(id).name.id).into(),
        hir::Type::Union(id) => hb.name_str(hb.union_data(id).name.id).into(),
        hir::Type::Struct(id) => hb.name_str(hb.struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hb, *ref_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hb, slice.ty))
        }
        hir::Type::ArrayStatic(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
        hir::Type::ArrayStaticDecl(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
    }
}

#[derive(Copy, Clone)]
struct BlockFlags {
    in_loop: bool,
    in_defer: bool,
}

impl BlockFlags {
    fn entry() -> BlockFlags {
        BlockFlags {
            in_loop: false,
            in_defer: false,
        }
    }
    //@need some way to recognize if loop was started while in defer
    // since break / continue cannot be used in defer in loops that originate
    // outside of that defer (defer is by design simple to think about)
    fn enter_defer(self) -> BlockFlags {
        BlockFlags {
            in_loop: self.in_loop,
            in_defer: true,
        }
    }

    fn enter_loop(self) -> BlockFlags {
        BlockFlags {
            in_loop: true,
            in_defer: self.in_defer,
        }
    }
}

#[derive(Copy, Clone)]
enum VariableID {
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

//@locals in scope needs to be popped
// on stack exit
struct ProcScope<'hir> {
    proc_id: hir::ProcID,
    locals: Vec<&'hir hir::Local<'hir>>,
    locals_in_scope: Vec<hir::LocalID>,
}

impl<'hir> ProcScope<'hir> {
    fn new(proc_id: hir::ProcID) -> ProcScope<'hir> {
        ProcScope {
            proc_id,
            locals: Vec::new(),
            locals_in_scope: Vec::new(),
        }
    }

    fn get_local(&self, id: hir::LocalID) -> &'hir hir::Local<'hir> {
        self.locals.get(id.index()).unwrap()
    }

    fn get_param<'ast>(
        &self,
        hb: &hb::HirBuilder<'hir, 'ast>,
        id: hir::ProcParamID,
    ) -> &'hir hir::ProcParam<'hir> {
        let data = hb.proc_data(self.proc_id);
        data.params.get(id.index()).unwrap()
    }

    fn push_local(&mut self, local: &'hir hir::Local<'hir>) {
        let id = hir::LocalID::new(self.locals.len());
        self.locals.push(local);
        self.locals_in_scope.push(id);
    }

    fn find_variable(&self, hb: &hb::HirBuilder, id: InternID) -> Option<VariableID> {
        let data = hb.proc_data(self.proc_id);
        for (idx, param) in data.params.iter().enumerate() {
            if param.name.id == id {
                let id = hir::ProcParamID::new(idx);
                return Some(VariableID::Param(id));
            }
        }
        for local_id in self.locals_in_scope.iter().cloned() {
            if self.get_local(local_id).name.id == id {
                return Some(VariableID::Local(local_id));
            }
        }
        None
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

pub fn type_matches<'hir>(ty: hir::Type<'hir>, ty2: hir::Type<'hir>) -> bool {
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
        //@division of 2 kinds of static sized arrays
        // makes this eq check totally incorrect, for now
        // or theres needs to be a 4 cases to compare them all
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            //@size const_expr is ignored
            type_matches(array.ty, array2.ty)
        }
        (hir::Type::ArrayStaticDecl(array), hir::Type::ArrayStaticDecl(array2)) => {
            //@size const_expr is ignored
            type_matches(array.ty, array2.ty)
        }
        _ => false,
    }
}

//@need type_repr instead of allocating hir types
// and maybe type::unknown, to facilitate better inference
// to better represent partially typed arrays, etc
#[must_use]
fn typecheck_expr_2<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    expect_ty: hir::Type<'hir>,
    expr: &'ast ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let type_result = match expr.kind {
        ast::ExprKind::Unit => typecheck_unit(hb),
        ast::ExprKind::LitNull => typecheck_lit_null(hb),
        ast::ExprKind::LitBool { val } => typecheck_lit_bool(hb, val),
        ast::ExprKind::LitInt { val } => typecheck_lit_int(hb, expect_ty, val),
        ast::ExprKind::LitFloat { val } => typecheck_lit_float(hb, expect_ty, val),
        ast::ExprKind::LitChar { val } => typecheck_lit_char(hb, val),
        ast::ExprKind::LitString { id } => typecheck_lit_string(hb, id),
        ast::ExprKind::If { if_ } => {
            typecheck_if(hb, origin_id, block_flags, proc_scope, expect_ty, if_)
        }
        ast::ExprKind::Block { stmts } => {
            typecheck_block(hb, origin_id, block_flags, proc_scope, expect_ty, stmts)
        }
        ast::ExprKind::Match { match_ } => {
            typecheck_match(hb, origin_id, block_flags, proc_scope, expect_ty, match_)
        }
        ast::ExprKind::Field { target, name } => {
            typecheck_field(hb, origin_id, block_flags, proc_scope, target, name)
        }
        ast::ExprKind::Index { target, index } => {
            typecheck_index(hb, origin_id, block_flags, proc_scope, target, index)
        }
        ast::ExprKind::Cast { target, ty } => typecheck_cast(
            hb,
            origin_id,
            block_flags,
            proc_scope,
            target,
            ty,
            expr.range,
        ),
        ast::ExprKind::Sizeof { ty } => typecheck_placeholder(hb),
        ast::ExprKind::Item { path } => typecheck_placeholder(hb),
        ast::ExprKind::ProcCall { proc_call } => typecheck_proc_call(
            hb,
            origin_id,
            block_flags,
            proc_scope,
            proc_call,
            expr.range,
        ),
        ast::ExprKind::StructInit { struct_init } => typecheck_struct_init(
            hb,
            origin_id,
            block_flags,
            proc_scope,
            struct_init,
            expr.range,
        ),
        ast::ExprKind::ArrayInit { input } => typecheck_placeholder(hb),
        ast::ExprKind::ArrayRepeat { expr, size } => typecheck_placeholder(hb),
        ast::ExprKind::UnaryExpr { op, rhs } => typecheck_placeholder(hb),
        ast::ExprKind::BinaryExpr { op, lhs, rhs } => typecheck_placeholder(hb),
    };

    if !type_matches(expect_ty, type_result.ty) {
        let msg: String = format!(
            "type mismatch: expected `{}`, found `{}`",
            type_format(hb, expect_ty),
            type_format(hb, type_result.ty)
        );
        hb.error(ErrorComp::error(msg).context(hb.src(origin_id, expr.range)));
    }

    type_result
}

fn typecheck_placeholder<'hir, 'ast>(hb: &mut hb::HirBuilder<'hir, 'ast>) -> TypeResult<'hir> {
    TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error))
}

fn typecheck_unit<'hir, 'ast>(hb: &mut hb::HirBuilder<'hir, 'ast>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Unit),
        hb.arena().alloc(hir::Expr::Unit),
    )
}

fn typecheck_lit_null<'hir, 'ast>(hb: &mut hb::HirBuilder<'hir, 'ast>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Rawptr),
        hb.arena().alloc(hir::Expr::LitNull),
    )
}

fn typecheck_lit_bool<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    val: bool,
) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Bool),
        hb.arena().alloc(hir::Expr::LitBool { val }),
    )
}

fn typecheck_lit_int<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    expect_ty: hir::Type<'hir>,
    val: u64,
) -> TypeResult<'hir> {
    const DEFAULT_INT_TYPE: ast::BasicType = ast::BasicType::S32;

    let lit_type = match expect_ty {
        hir::Type::Basic(expect) => match expect {
            ast::BasicType::Unit => DEFAULT_INT_TYPE,
            ast::BasicType::Bool => DEFAULT_INT_TYPE,
            ast::BasicType::S8
            | ast::BasicType::S16
            | ast::BasicType::S32
            | ast::BasicType::S64
            | ast::BasicType::Ssize
            | ast::BasicType::U8
            | ast::BasicType::U16
            | ast::BasicType::U32
            | ast::BasicType::U64
            | ast::BasicType::Usize => expect,
            ast::BasicType::F32 => DEFAULT_INT_TYPE,
            ast::BasicType::F64 => DEFAULT_INT_TYPE,
            ast::BasicType::Char => DEFAULT_INT_TYPE,
            ast::BasicType::Rawptr => DEFAULT_INT_TYPE,
        },
        _ => DEFAULT_INT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        hb.arena().alloc(hir::Expr::LitInt { val, ty: lit_type }),
    )
}

fn typecheck_lit_float<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    expect_ty: hir::Type<'hir>,
    val: f64,
) -> TypeResult<'hir> {
    const DEFAULT_FLOAT_TYPE: ast::BasicType = ast::BasicType::F64;

    let lit_type = match expect_ty {
        hir::Type::Basic(expect) => match expect {
            ast::BasicType::Unit => DEFAULT_FLOAT_TYPE,
            ast::BasicType::Bool => DEFAULT_FLOAT_TYPE,
            ast::BasicType::S8 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::S16 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::S32 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::S64 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::Ssize => DEFAULT_FLOAT_TYPE,
            ast::BasicType::U8 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::U16 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::U32 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::U64 => DEFAULT_FLOAT_TYPE,
            ast::BasicType::Usize => DEFAULT_FLOAT_TYPE,
            ast::BasicType::F32 | ast::BasicType::F64 => expect,
            ast::BasicType::Char => DEFAULT_FLOAT_TYPE,
            ast::BasicType::Rawptr => DEFAULT_FLOAT_TYPE,
        },
        _ => DEFAULT_FLOAT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        hb.arena().alloc(hir::Expr::LitFloat { val, ty: lit_type }),
    )
}

fn typecheck_lit_char<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    val: char,
) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(ast::BasicType::Char),
        hb.arena().alloc(hir::Expr::LitChar { val }),
    )
}

fn typecheck_lit_string<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    id: InternID,
) -> TypeResult<'hir> {
    let slice = hb.arena().alloc(hir::ArraySlice {
        mutt: ast::Mut::Immutable,
        ty: hir::Type::Basic(ast::BasicType::U8),
    });

    TypeResult::new(
        hir::Type::ArraySlice(slice),
        hb.arena().alloc(hir::Expr::LitString { id }),
    )
}

fn typecheck_if<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    expect_ty: hir::Type<'hir>,
    if_: &'ast ast::If<'ast>,
) -> TypeResult<'hir> {
    let has_fallback = if_.fallback.is_some();

    let entry = if_.entry;
    let _ = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Basic(ast::BasicType::Bool),
        entry.cond,
    );
    let _ = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        expect_ty,
        entry.block,
    );

    for &branch in if_.branches {
        let _ = typecheck_expr_2(
            hb,
            origin_id,
            block_flags,
            proc_scope,
            hir::Type::Basic(ast::BasicType::Bool),
            branch.cond,
        );
        let _ = typecheck_expr_2(
            hb,
            origin_id,
            block_flags,
            proc_scope,
            expect_ty,
            branch.block,
        );
    }

    if let Some(block) = if_.fallback {
        let _ = typecheck_expr_2(hb, origin_id, block_flags, proc_scope, expect_ty, block);
    }

    typecheck_placeholder(hb)
}

fn typecheck_match<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    expect_ty: hir::Type<'hir>,
    match_: &'ast ast::Match<'ast>,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Error, // no expectation
        match_.on_expr,
    );
    for arm in match_.arms {
        if let Some(pat) = arm.pat {
            let pat_res = typecheck_expr_2(hb, origin_id, block_flags, proc_scope, on_res.ty, pat);
        }
        //@check match arm expr
    }
    typecheck_placeholder(hb)
}

fn typecheck_field<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    target: &'ast ast::Expr,
    name: ast::Ident,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Error, // no expectation
        target,
    );

    let (field_ty, kind) = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_type_field(hb, origin_id, *ref_ty, name),
        _ => verify_type_field(hb, origin_id, target_res.ty, name),
    };

    match kind {
        FieldExprKind::None => {
            TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error))
        }
        FieldExprKind::Member(id) => TypeResult::new(
            field_ty,
            hb.arena().alloc(hir::Expr::UnionMember {
                target: target_res.expr,
                id,
            }),
        ),
        FieldExprKind::Field(id) => TypeResult::new(
            field_ty,
            hb.arena().alloc(hir::Expr::StructField {
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

fn verify_type_field<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    ty: hir::Type<'hir>,
    name: ast::Ident,
) -> (hir::Type<'hir>, FieldExprKind) {
    match ty {
        hir::Type::Error => (hir::Type::Error, FieldExprKind::None),
        hir::Type::Union(id) => {
            let data = hb.union_data(id);
            let find = data.members.iter().enumerate().find_map(|(id, member)| {
                (member.name.id == name.id).then(|| (hir::UnionMemberID::new(id), member))
            });
            match find {
                Some((id, member)) => (member.ty, FieldExprKind::Member(id)),
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on union type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.src(origin_id, name.range)),
                    );
                    (hir::Type::Error, FieldExprKind::None)
                }
            }
        }
        hir::Type::Struct(id) => {
            let data = hb.struct_data(id);
            let find = data.fields.iter().enumerate().find_map(|(id, field)| {
                (field.name.id == name.id).then(|| (hir::StructFieldID::new(id), field))
            });
            match find {
                Some((id, field)) => (field.ty, FieldExprKind::Field(id)),
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on struct type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.src(origin_id, name.range)),
                    );
                    (hir::Type::Error, FieldExprKind::None)
                }
            }
        }
        _ => {
            let ty_format = type_format(hb, ty);
            hb.error(
                ErrorComp::error(format!(
                    "no field `{}` exists on value of type {}",
                    hb.name_str(name.id),
                    ty_format,
                ))
                .context(hb.src(origin_id, name.range)),
            );
            (hir::Type::Error, FieldExprKind::None)
        }
    }
}

fn typecheck_index<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    target: &'ast ast::Expr<'ast>,
    index: &'ast ast::Expr<'ast>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Error, // no expectation
        target,
    );
    let index_res = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Basic(ast::BasicType::Usize),
        index,
    );

    let elem_ty = match target_res.ty {
        hir::Type::Reference(ref_ty, mutt) => verify_elem_type(*ref_ty),
        _ => verify_elem_type(target_res.ty),
    };

    match elem_ty {
        Some(it) => {
            let hir_expr = hb.arena().alloc(hir::Expr::Index {
                target: target_res.expr,
                index: index_res.expr,
            });
            TypeResult::new(it, hir_expr)
        }
        None => {
            let ty_format = type_format(hb, target_res.ty);
            hb.error(
                ErrorComp::error(format!("cannot index value of type {}", ty_format))
                    .context(hb.src(origin_id, index.range)),
            );
            TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error))
        }
    }
}

fn verify_elem_type(ty: hir::Type) -> Option<hir::Type> {
    match ty {
        hir::Type::Error => Some(hir::Type::Error),
        hir::Type::ArraySlice(slice) => Some(slice.ty),
        hir::Type::ArrayStatic(array) => Some(array.ty),
        hir::Type::ArrayStaticDecl(array) => Some(array.ty),
        _ => None,
    }
}

fn typecheck_cast<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    target: &'ast ast::Expr<'ast>,
    ty: &'ast ast::Type<'ast>,
    cast_range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr_2(
        hb,
        origin_id,
        block_flags,
        proc_scope,
        hir::Type::Error, // no expectation
        target,
    );
    let cast_ty = super::pass_3::resolve_type(hb, origin_id, *ty, true);

    match (target_res.ty, cast_ty) {
        (hir::Type::Error, ..) => {}
        (.., hir::Type::Error) => {}
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            //@verify that from into pair is valid
            // determine type of the cast, according to llvm, e.g: fp_trunc, fp_to_int etc.
        }
        _ => {
            let from_format = type_format(hb, target_res.ty);
            let into_format = type_format(hb, cast_ty);
            hb.error(
                ErrorComp::error(format!(
                    "non privitive cast from `{from_format}` into `{into_format}`",
                ))
                .context(hb.src(origin_id, cast_range)),
            );
        }
    }

    let hir_ty = hb.arena().alloc(cast_ty);
    TypeResult {
        ty: cast_ty,
        expr: hb.arena().alloc(hir::Expr::Cast {
            target: target_res.expr,
            ty: hir_ty,
        }),
    }
}

fn typecheck_proc_call<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    proc_call: &'ast ast::ProcCall<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let proc_id = match path_resolve_as_proc(hb, origin_id, proc_call.path) {
        Some(it) => it,
        None => {
            for &expr in proc_call.input {
                let _ = typecheck_expr_2(
                    hb,
                    origin_id,
                    block_flags,
                    proc_scope,
                    hir::Type::Error,
                    expr,
                );
            }
            return TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error));
        }
    };

    for (idx, &expr) in proc_call.input.iter().enumerate() {
        let data = hb.proc_data(proc_id);
        let param = data.params.get(idx);

        let expect_ty = match param {
            Some(param) => param.ty,
            None => hir::Type::Error,
        };
        let _ = typecheck_expr_2(hb, origin_id, block_flags, proc_scope, expect_ty, expr);
    }

    //@getting proc data multiple times due to mutable error reporting
    // maybe pass error context to all functions isntead
    let input_count = proc_call.input.len();
    let expected_count = hb.proc_data(proc_id).params.len();

    if input_count != expected_count {
        let data = hb.proc_data(proc_id);
        hb.error(
            ErrorComp::error("unexpected number of input arguments")
                .context(hb.src(origin_id, expr_range))
                .context_info(
                    "calling this procedure",
                    hb.src(data.origin_id, data.name.range),
                ),
        );
    }

    let data = hb.proc_data(proc_id);
    TypeResult::new(data.return_ty, hb.arena().alloc(hir::Expr::Error))
}

fn typecheck_struct_init<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    struct_init: &'ast ast::StructInit<'ast>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let struct_id = match path_resolve_as_struct(hb, origin_id, struct_init.path) {
        Some(it) => it,
        None => {
            for &expr in struct_init.input {
                //@resolve expr & field name or name as expr
                //let _ = typecheck_expr_2(
                //    hb,
                //    origin_id,
                //    block_flags,
                //    proc_scope,
                //    hir::Type::Error,
                //    expr,
                //);
            }
            return TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error));
        }
    };

    for (idx, &expr) in struct_init.input.iter().enumerate() {
        let data = hb.struct_data(struct_id);
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
    let expected_count = hb.struct_data(struct_id).fields.len();

    if input_count != expected_count {
        let data = hb.struct_data(struct_id);
        hb.error(
            ErrorComp::error("unexpected number of input fields")
                .context(hb.src(origin_id, expr_range))
                .context_info(
                    "calling this procedure",
                    hb.src(data.origin_id, data.name.range),
                ),
        );
    }

    let data = hb.struct_data(struct_id);
    TypeResult::new(
        hir::Type::Struct(struct_id),
        hb.arena().alloc(hir::Expr::Error),
    )
}

fn typecheck_block<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    expect_ty: hir::Type<'hir>,
    stmts: &'ast [ast::Stmt<'ast>],
) -> TypeResult<'hir> {
    let mut block_ty = None;

    for (idx, stmt) in stmts.iter().enumerate() {
        match stmt.kind {
            ast::StmtKind::Break => typecheck_stmt_break(hb, origin_id, block_flags, stmt.range),
            ast::StmtKind::Continue => {
                typecheck_stmt_continue(hb, origin_id, block_flags, stmt.range);
            }
            ast::StmtKind::Return(ret_expr) => {}
            ast::StmtKind::Defer(block) => typecheck_stmt_defer(
                hb,
                origin_id,
                block_flags,
                proc_scope,
                stmt.range.start(),
                block,
            ),
            ast::StmtKind::ForLoop(for_) => {}
            ast::StmtKind::Local(local) => {}
            ast::StmtKind::Assign(assign) => {}
            ast::StmtKind::ExprSemi(expr) => {
                let _ = typecheck_expr_2(
                    hb,
                    origin_id,
                    block_flags,
                    proc_scope,
                    hir::Type::Error, // no expectation
                    expr,
                );
            }
            ast::StmtKind::ExprTail(expr) => {
                if idx + 1 == stmts.len() {
                    let res =
                        typecheck_expr_2(hb, origin_id, block_flags, proc_scope, expect_ty, expr);
                    block_ty = Some(res.ty);
                } else {
                    //@decide if semi should enforced on parsing
                    // of we just expect a unit from that no-semi expression
                    // when its not a last expression?
                    let _ = typecheck_expr_2(
                        hb,
                        origin_id,
                        block_flags,
                        proc_scope,
                        hir::Type::Basic(ast::BasicType::Unit),
                        expr,
                    );
                }
            }
        }
    }

    // when type expectation was passed to tail expr
    // return Type::Error to not trigger duplicate type mismatch errors
    if block_ty.is_some() {
        TypeResult::new(hir::Type::Error, hb.arena().alloc(hir::Expr::Error))
    } else {
        TypeResult::new(
            hir::Type::Basic(ast::BasicType::Unit),
            hb.arena().alloc(hir::Expr::Error),
        )
    }
}

fn typecheck_stmt_break<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    stmt_range: TextRange,
) {
    if !block_flags.in_loop {
        hb.error(
            ErrorComp::error("cannot use `break` outside of a loop")
                .context(hb.src(origin_id, stmt_range)),
        );
    }
}

fn typecheck_stmt_continue<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    stmt_range: TextRange,
) {
    if !block_flags.in_loop {
        hb.error(
            ErrorComp::error("cannot use `continue` outside of a loop")
                .context(hb.src(origin_id, stmt_range)),
        );
    }
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_stmt_defer<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    block_flags: BlockFlags,
    proc_scope: &mut ProcScope<'hir>,
    stmt_start: TextOffset,
    block: &'ast ast::Expr<'ast>,
) {
    if block_flags.in_defer {
        hb.error(
            ErrorComp::error("`defer` statement cannot be nested")
                .context(hb.src(origin_id, TextRange::new(stmt_start, stmt_start + 5.into()))),
        );
    }
    let _ = typecheck_expr_2(
        hb,
        origin_id,
        block_flags.enter_defer(),
        proc_scope,
        hir::Type::Basic(ast::BasicType::Unit),
        block,
    );
}

/*
path syntax resolution:
prefix.name.name.name ...

prefix:
super.   -> start at scope_id
package. -> start at scope_id

items:
mod        -> <chained> will change target scope of an item
proc       -> [no follow]
enum       -> <follow?> by single enum variant name
union      -> [no follow]
struct     -> [no follow]
const      -> <follow?> by <chained> field access
global     -> <follow?> by <chained> field access
param_var  -> <follow?> by <chained> field access
local_var  -> <follow?> by <chained> field access

*/

//@this needs to be re-thinked
// to allow more general use
// to get first item of any kind
// and reduce checks for missing item etc
// change api, maybe expect an item and emit if path is modules only
struct PathResult<'ast> {
    target_id: hir::ScopeID,
    symbol: Option<(hb::SymbolKind, SourceRange, ast::Ident)>,
    remaining: &'ast [ast::Ident],
}

fn path_resolve_target_scope<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<PathResult<'ast>> {
    let mut target_id = match path.kind {
        ast::PathKind::None => origin_id,
        ast::PathKind::Super => match hb.scope_parent(origin_id) {
            Some(it) => it,
            None => {
                let range = TextRange::new(path.range_start, path.range_start + 5.into());
                hb.error(
                    ErrorComp::error("parent module `super` cannot be used from the root module")
                        .context(hb.src(origin_id, range)),
                );
                return None;
            }
        },
        ast::PathKind::Package => hb::ROOT_SCOPE_ID,
    };

    let mut step_count: usize = 0;
    for name in path.names {
        match hb.symbol_from_scope(origin_id, target_id, path.kind, name.id) {
            Some((symbol, source)) => match symbol {
                hb::SymbolKind::Mod(id) => {
                    let data = hb.get_mod(id);
                    step_count += 1;

                    if let Some(new_target) = data.target {
                        target_id = new_target;
                    } else {
                        hb.error(
                            ErrorComp::error(format!(
                                "module `{}` does not have its associated file",
                                hb.name_str(name.id)
                            ))
                            .context(hb.src(origin_id, name.range))
                            .context_info("defined here", source),
                        );
                        return None;
                    }
                }
                _ => {
                    step_count += 1;
                    return Some(PathResult {
                        target_id,
                        symbol: Some((symbol, source, *name)),
                        remaining: &path.names[step_count..],
                    });
                }
            },
            None => {
                hb.error(
                    ErrorComp::error(format!("name `{}` is not found", hb.name_str(name.id)))
                        .context(hb.src(origin_id, name.range)),
                );
                return None;
            }
        }
    }

    Some(PathResult {
        target_id,
        symbol: None,
        remaining: &path.names[step_count..],
    })
}

pub fn path_resolve_as_type<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> hir::Type<'hir> {
    let path_res = match path_resolve_target_scope(hb, origin_id, path) {
        Some(it) => it,
        None => return hir::Type::Error,
    };

    let type_res = match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            hb::SymbolKind::Enum(id) => hir::Type::Enum(id),
            hb::SymbolKind::Union(id) => hir::Type::Union(id),
            hb::SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                hb.error(
                    ErrorComp::error("expected custom type item")
                        .context(hb.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                return hir::Type::Error;
            }
        },
        None => {
            let path_range = TextRange::new(
                path.range_start,
                path.names.last().expect("non empty path").range.end(), //@just store path range in ast?
            );
            hb.error(
                ErrorComp::error("module path does not lead to an item")
                    .context(hb.src(origin_id, path_range)),
            );
            return hir::Type::Error;
        }
    };

    if !path_res.remaining.is_empty() {
        let start = path_res.remaining.first().unwrap().range.start();
        let end = path_res.remaining.last().unwrap().range.end();
        hb.error(
            ErrorComp::error("type name cannot be accessed further")
                .context(hb.src(origin_id, TextRange::new(start, end))),
        );
    }

    type_res
}

pub fn path_resolve_as_proc<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<hir::ProcID> {
    let path_res = match path_resolve_target_scope(hb, origin_id, path) {
        Some(it) => it,
        None => return None,
    };

    let proc_id = match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            hb::SymbolKind::Proc(id) => Some(id),
            _ => {
                hb.error(
                    ErrorComp::error("expected procedure item")
                        .context(hb.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                return None;
            }
        },
        None => {
            //@repeating with similar as_type
            let path_range = TextRange::new(
                path.range_start,
                path.names.last().expect("non empty path").range.end(), //@just store path range in ast?
            );
            hb.error(
                ErrorComp::error(format!("module path does not lead to an item",))
                    .context(hb.src(origin_id, path_range)),
            );
            return None;
        }
    };

    //@repeating with similar as_type
    if !path_res.remaining.is_empty() {
        let start = path_res.remaining.first().unwrap().range.start();
        let end = path_res.remaining.last().unwrap().range.end();
        hb.error(
            ErrorComp::error("procedure name cannot be accessed further")
                .context(hb.src(origin_id, TextRange::new(start, end))),
        );
    }

    proc_id
}

pub fn path_resolve_as_struct<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<hir::StructID> {
    let path_res = match path_resolve_target_scope(hb, origin_id, path) {
        Some(it) => it,
        None => return None,
    };

    let struct_id = match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            hb::SymbolKind::Struct(id) => Some(id),
            _ => {
                hb.error(
                    ErrorComp::error("expected struct item")
                        .context(hb.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                return None;
            }
        },
        None => {
            //@repeating with similar as_type
            let path_range = TextRange::new(
                path.range_start,
                path.names.last().expect("non empty path").range.end(), //@just store path range in ast?
            );
            hb.error(
                ErrorComp::error(format!("module path does not lead to an item",))
                    .context(hb.src(origin_id, path_range)),
            );
            return None;
        }
    };

    //@repeating with similar as_type
    if !path_res.remaining.is_empty() {
        let start = path_res.remaining.first().unwrap().range.start();
        let end = path_res.remaining.last().unwrap().range.end();
        hb.error(
            ErrorComp::error("struct name cannot be accessed further")
                .context(hb.src(origin_id, TextRange::new(start, end))),
        );
    }

    struct_id
}

pub fn path_resolve_as_module_path<'hir, 'ast>(
    hb: &mut hb::HirBuilder<'hir, 'ast>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<hir::ScopeID> {
    let path_res = match path_resolve_target_scope(hb, origin_id, path) {
        Some(it) => it,
        None => return None,
    };

    match path_res.symbol {
        Some((_, _, name)) => {
            hb.error(
                ErrorComp::error(format!("`{}` is not a module", hb.name_str(name.id)))
                    .context(hb.src(origin_id, name.range)),
            );
            None
        }
        _ => Some(path_res.target_id),
    }
}

use super::hir_build::{self as hb, HirData, HirEmit, SymbolKind};
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

struct ProcScope<'hir, 'check> {
    data: &'check hir::ProcData<'hir>,
    blocks: Vec<BlockData>,
    locals: Vec<&'hir hir::Local<'hir>>,
    locals_in_scope: Vec<hir::LocalID>,
}

struct BlockData {
    in_loop: bool,
    in_defer: bool,
    local_count: u32,
}

enum VariableID {
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

impl<'hir, 'check> ProcScope<'hir, 'check> {
    fn new(data: &'check hir::ProcData<'hir>) -> Self {
        ProcScope {
            data,
            blocks: Vec::new(),
            locals: Vec::new(),
            locals_in_scope: Vec::new(),
        }
    }

    fn push_block(&mut self) {
        self.blocks.push(BlockData {
            in_loop: false,
            in_defer: false,
            local_count: 0,
        });
    }

    fn push_local(&mut self, local: &'hir hir::Local<'hir>) {
        let local_id = hir::LocalID::new(self.locals.len());
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.blocks.last_mut().expect("block exists").local_count += 1;
    }

    fn pop_block(&mut self) {
        let block = self.blocks.pop().expect("block exists");
        for _ in 0..block.local_count {
            self.locals_in_scope.pop();
        }
    }

    fn origin_id(&self) -> hir::ScopeID {
        self.data.origin_id
    }
    fn get_local(&self, id: hir::LocalID) -> &'hir hir::Local<'hir> {
        self.locals[id.index()]
    }
    fn get_param(&self, id: hir::ProcParamID) -> &'hir hir::ProcParam<'hir> {
        &self.data.params[id.index()]
    }

    fn find_variable(&self, id: InternID) -> Option<VariableID> {
        for (idx, param) in self.data.params.iter().enumerate() {
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
        ast::ExprKind::Item { path } => typecheck_placeholder(emit),
        ast::ExprKind::ProcCall { proc_call } => {
            typecheck_proc_call(hir, emit, proc, proc_call, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init, expr.range)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(hir, emit, proc, expect, input),
        ast::ExprKind::ArrayRepeat { expr, size } => typecheck_placeholder(emit),
        ast::ExprKind::Unary { op, rhs } => typecheck_placeholder(emit),
        ast::ExprKind::Binary { op, lhs, rhs } => typecheck_placeholder(emit),
    };

    if !type_matches(expect, expr_res.ty) {
        let msg: String = format!(
            "type mismatch: expected `{}`, found `{}`",
            type_format(hir, expect),
            type_format(hir, expr_res.ty)
        );
        emit.error(ErrorComp::error(msg).context(hir.src(proc.origin_id(), expr.range)));
    }

    expr_res
}

fn typecheck_placeholder<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
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
            | ast::BasicType::Usize => basic,
            ast::BasicType::F32 => DEFAULT_INT_TYPE,
            ast::BasicType::F64 => DEFAULT_INT_TYPE,
            ast::BasicType::Char => DEFAULT_INT_TYPE,
            ast::BasicType::Rawptr => DEFAULT_INT_TYPE,
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
            ast::BasicType::F32 | ast::BasicType::F64 => basic,
            ast::BasicType::Char => DEFAULT_FLOAT_TYPE,
            ast::BasicType::Rawptr => DEFAULT_FLOAT_TYPE,
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

    typecheck_placeholder(emit)
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
    typecheck_placeholder(emit)
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
                        .context(hir.src(proc.origin_id(), name.range)),
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
                        .context(hir.src(proc.origin_id(), name.range)),
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
                .context(hir.src(proc.origin_id(), name.range)),
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
                    .context(hir.src(proc.origin_id(), index.range)),
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
    let cast_ty = super::pass_3::type_resolve(hir, emit, proc.origin_id(), *ty);

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
                .context(hir.src(proc.origin_id(), cast_range)),
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
    let ty = super::pass_3::type_resolve(hir, emit, proc.origin_id(), ty);

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
                .context(hir.src(proc.origin_id(), TextRange::new(start, start + 6.into()))),
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

fn typecheck_proc_call<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    proc_call: &ast::ProcCall<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let proc_id = match path_resolve_as_proc(hir, emit, proc.origin_id(), proc_call.path) {
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
                .context(hir.src(proc.origin_id(), expr_range))
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
    let struct_id = match path_resolve_as_struct(hir, emit, proc.origin_id(), struct_init.path) {
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
                .context(hir.src(proc.origin_id(), expr_range))
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
    let mut first_elem = hir::Type::Error;
    let mut expect_elem = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

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

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    stmts: &[ast::Stmt<'_>],
) -> TypeResult<'hir> {
    let mut block_ty = None;

    for (idx, stmt) in stmts.iter().enumerate() {
        match stmt.kind {
            ast::StmtKind::Break => typecheck_stmt_break(hir, emit, proc, stmt.range),
            ast::StmtKind::Continue => {
                typecheck_stmt_continue(hir, emit, proc, stmt.range);
            }
            ast::StmtKind::Return(ret_expr) => {}
            ast::StmtKind::Defer(block) => {
                typecheck_stmt_defer(hir, emit, proc, stmt.range.start(), block)
            }
            ast::StmtKind::ForLoop(for_) => {}
            ast::StmtKind::Local(local) => {}
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

fn typecheck_stmt_break<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) {
    //@proc block flags
    /*
    if !block_flags.in_loop {
        emit.error(
            ErrorComp::error("cannot use `break` outside of a loop")
                .context(hir.src(proc.origin_id(), range)),
        );
    }
    */
}

fn typecheck_stmt_continue<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) {
    //@proc block flags
    /*
    if !block_flags.in_loop {
        emit.error(
            ErrorComp::error("cannot use `continue` outside of a loop")
                .context(hir.src(proc.origin_id(), range)),
        );
    }
    */
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_stmt_defer<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    block: &ast::Expr<'_>,
) {
    //@proc block flags
    /*
    if block_flags.in_defer {
        emit.error(
            ErrorComp::error("`defer` statement cannot be nested")
                .context(hir.src(proc.origin_id(), TextRange::new(start, start + 5.into()))),
        );
    }
    */
    let _ = typecheck_expr(
        hir,
        emit,
        proc,
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
    symbol: Option<(SymbolKind, SourceRange, ast::Name)>,
    remaining: &'ast [ast::Name],
}

fn path_resolve_target_scope<'hir, 'ast>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path,
    expect_item: bool,
    expect_remaining: bool,
) -> Option<PathResult<'ast>> {
    let mut target_id = origin_id;

    let mut step_count: usize = 0;

    if let Some(name) = path.names.first().cloned() {
        match hir.symbol_from_scope(origin_id, target_id, name.id) {
            Some((symbol, vis, source)) => match symbol {
                SymbolKind::Module(id) => {
                    step_count += 1;
                    target_id = id;
                }
                _ => {
                    //@duplicate error same as in import pass
                    // api for getting things from scopes is bad
                    if vis == ast::Vis::Private {
                        emit.error(
                            ErrorComp::error(format!(
                                "item `{}` is private",
                                hir.name_str(name.id)
                            ))
                            .context(hir.src(origin_id, name.range))
                            .context_info("defined here", source),
                        );
                        return None;
                    }
                    step_count += 1;
                    return Some(PathResult {
                        target_id,
                        symbol: Some((symbol, source, name)),
                        remaining: &path.names[step_count..],
                    });
                }
            },
            None => {
                emit.error(
                    ErrorComp::error(format!("name `{}` is not found", hir.name_str(name.id)))
                        .context(hir.src(origin_id, name.range)),
                );
                return None;
            }
        }
    }

    //@change this error to something like:
    // "expected {item_name} found module {name}"
    // defined here / imported here
    if expect_item {
        //@unwrapping, assuming 1+ exists (enforced by the parser)
        let path_range = TextRange::new(
            path.names.first().unwrap().range.start(),
            path.names.last().unwrap().range.end(),
        );
        emit.error(
            ErrorComp::error("path does not lead to an item")
                .context(hir.src(origin_id, path_range)),
        );
        return None;
    }

    let mut path_res = PathResult {
        target_id,
        symbol: None,
        remaining: &path.names[step_count..],
    };

    if !expect_remaining && !path_res.remaining.is_empty() {
        let start = path_res.remaining.first().unwrap().range.start();
        let end = path_res.remaining.last().unwrap().range.end();
        path_res.remaining = &[];
        emit.error(
            ErrorComp::error("this item cannot be accessed further")
                .context(hir.src(origin_id, TextRange::new(start, end))),
        );
    }

    Some(path_res)
}

pub fn path_resolve_as_type<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    path: &ast::Path<'_>,
) -> hir::Type<'hir> {
    let path_res = match path_resolve_target_scope(hir, emit, origin_id, path, true, false) {
        Some(it) => it,
        None => return hir::Type::Error,
    };

    match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Union(id) => hir::Type::Union(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                emit.error(
                    ErrorComp::error("expected type item")
                        .context(hir.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                hir::Type::Error
            }
        },
        None => hir::Type::Error,
    }
}

pub fn path_resolve_as_proc<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    path: &ast::Path<'_>,
) -> Option<hir::ProcID> {
    let path_res = match path_resolve_target_scope(hir, emit, origin_id, path, true, false) {
        Some(it) => it,
        None => return None,
    };

    match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            SymbolKind::Proc(id) => Some(id),
            _ => {
                emit.error(
                    ErrorComp::error("expected procedure item")
                        .context(hir.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                None
            }
        },
        None => None,
    }
}

pub fn path_resolve_as_struct<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    path: &ast::Path<'_>,
) -> Option<hir::StructID> {
    let path_res = match path_resolve_target_scope(hir, emit, origin_id, path, true, false) {
        Some(it) => it,
        None => return None,
    };

    match path_res.symbol {
        Some((symbol, source, name)) => match symbol {
            SymbolKind::Struct(id) => Some(id),
            _ => {
                emit.error(
                    ErrorComp::error("expected struct item")
                        .context(hir.src(origin_id, name.range))
                        .context_info("found this", source),
                );
                None
            }
        },
        None => None,
    }
}

pub fn path_resolve_as_module_path<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    path: &ast::Path<'_>,
) -> Option<hir::ScopeID> {
    let path_res = match path_resolve_target_scope(hir, emit, origin_id, path, false, false) {
        Some(it) => it,
        None => return None,
    };

    match path_res.symbol {
        Some((_, _, name)) => {
            emit.error(
                ErrorComp::error(format!("`{}` is not a module", hir.name_str(name.id)))
                    .context(hir.src(origin_id, name.range)),
            );
            None
        }
        _ => Some(path_res.target_id),
    }
}

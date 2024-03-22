use super::hir_builder as hb;
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use crate::text::TextRange;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let decl = hb.proc_ast(id);
    let data = hb.proc_data(id);

    match decl.block {
        Some(block) => {
            let ty = typecheck_expr(hb, data.from_id, block);
        }
        None => {
            //@for now having a tail directive assumes that it must be a #[c_call]
            // and this directory is only present with no block expression
            let directive_tail = decl
                .directive_tail
                .expect("directive expected with no proc block");

            let directive_name = hb.name_str(directive_tail.name.id);
            if directive_name != "c_call" {
                hb.error(
                    ErrorComp::error(format!(
                        "expected a `c_call` directive, got `{}`",
                        directive_name
                    ))
                    .context(hb.get_scope(data.from_id).source(directive_tail.name.range)),
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

//@better idea would be to return type repr that is not allocated via arena
// and will be fast to construct and compare
#[must_use]
fn typecheck_expr<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    origin_id: hir::ScopeID,
    checked_expr: &ast::Expr<'ast>,
) -> hir::Type<'hir> {
    match checked_expr.kind {
        ast::ExprKind::Unit => hir::Type::Basic(ast::BasicType::Unit),
        ast::ExprKind::LitNull => hir::Type::Basic(ast::BasicType::Rawptr),
        ast::ExprKind::LitBool { val } => hir::Type::Basic(ast::BasicType::Bool),
        ast::ExprKind::LitInt { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::S32)),
        ast::ExprKind::LitFloat { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::F64)),
        ast::ExprKind::LitChar { val } => hir::Type::Basic(ast::BasicType::Char),
        ast::ExprKind::LitString { id } => {
            let slice = hb.arena().alloc(hir::ArraySlice {
                mutt: ast::Mut::Immutable,
                ty: hir::Type::Basic(ast::BasicType::U8),
            });
            hir::Type::ArraySlice(slice)
        }
        ast::ExprKind::If { if_ } => {
            for arm in if_ {
                if let Some(cond) = arm.cond {
                    let _ = typecheck_expr(hb, origin_id, cond); //@expect bool
                }
                let _ = typecheck_expr(hb, origin_id, arm.expr);
            }
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::Block { stmts } => {
            for (idx, stmt) in stmts.iter().enumerate() {
                let last = idx + 1 == stmts.len();
                let stmt_ty = match stmt.kind {
                    ast::StmtKind::Break => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Continue => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Return(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Defer(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::ForLoop(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::VarDecl(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::VarAssign(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::ExprSemi(expr) => typecheck_expr(hb, origin_id, expr),
                    ast::StmtKind::ExprTail(expr) => typecheck_expr(hb, origin_id, expr),
                };
                if last {
                    return stmt_ty;
                }
            }
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Match { match_ } => {
            for arm in match_.arms {
                //
            }
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::Field { target, name } => {
            //@allowing only single reference access of the field, no automatic derefencing is done automatically
            let ty = typecheck_expr(hb, origin_id, target);
            match ty {
                hir::Type::Reference(ref_ty, mutt) => check_field_ty(hb, origin_id, *ref_ty, name),
                _ => check_field_ty(hb, origin_id, ty, name),
            }
        }
        ast::ExprKind::Index { target, index } => {
            //@allowing only single reference access of the field, no automatic derefencing is done automatically
            let ty = typecheck_expr(hb, origin_id, target);
            let _ = typecheck_expr(hb, origin_id, index); //@expect usize
            match ty {
                hir::Type::Reference(ref_ty, mutt) => {
                    check_index_ty(hb, origin_id, *ref_ty, index.range)
                }
                _ => check_index_ty(hb, origin_id, ty, index.range),
            }
        }
        ast::ExprKind::Cast { target, ty } => typecheck_todo(hb, origin_id, checked_expr),
        ast::ExprKind::Sizeof { ty } => {
            //@check ast type
            hir::Type::Basic(ast::BasicType::Usize)
        }
        ast::ExprKind::Item { path } => {
            //@nameresolve item path
            let target_id = path_resolve_target_scope(hb, origin_id, path);
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::ProcCall { proc_call } => {
            //@nameresolve proc_call.path
            let target_id = path_resolve_target_scope(hb, origin_id, proc_call.path);
            for &expr in proc_call.input {
                let _ = typecheck_expr(hb, origin_id, expr);
            }
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::StructInit { struct_init } => {
            //@nameresolve struct_init.path
            let target_id = path_resolve_target_scope(hb, origin_id, struct_init.path);

            // @first find if field name exists, only then handle
            // name and expr or just a name
            for init in struct_init.input {
                match init.expr {
                    Some(expr) => {
                        //@field name and the expression
                        let _ = typecheck_expr(hb, origin_id, expr);
                    }
                    None => {
                        //@field name that must match some named value in scope
                    }
                }
            }
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::ArrayInit { input } => {
            //@expected type should make empty array to be of that type
            // since type of empty array literal is anything otherwise
            // if we work with non hir types directly representing the Unknown type is possible
            // and mutation is fine

            // rust example:
            // let empty: [unknown; 0] = [];
            // let slice: &[u32] = &empty; //now array type is known

            //@try to design a better method that might check a procedure body
            // without starting to produce a new hir right-away
            // since the series of Hir::Expr are not usefull if this code
            // wont be compiled any further, we dont need to allocate anything in that case

            let mut elem_ty = hir::Type::Error;
            for (idx, &expr) in input.iter().enumerate() {
                let ty = typecheck_expr(hb, origin_id, expr);
                if idx == 0 {
                    elem_ty = ty;
                }
            }
            let size = hb.arena().alloc(hir::Expr {
                kind: hir::ExprKind::LitInt {
                    val: input.len() as u64,
                    ty: ast::BasicType::Usize,
                },
                range: checked_expr.range, //@this size doesnt have a explicit range
            });
            let array = hb.arena().alloc(hir::ArrayStatic { size, ty: elem_ty });
            hir::Type::ArrayStatic(array)
        }
        ast::ExprKind::ArrayRepeat { expr, size } => typecheck_todo(hb, origin_id, checked_expr),
        ast::ExprKind::UnaryExpr { op, rhs } => {
            let rhs = typecheck_expr(hb, origin_id, rhs);
            typecheck_todo(hb, origin_id, checked_expr)
        }
        ast::ExprKind::BinaryExpr { op, lhs, rhs } => {
            let lhs = typecheck_expr(hb, origin_id, lhs);
            let rhs = typecheck_expr(hb, origin_id, rhs);
            typecheck_todo(hb, origin_id, checked_expr)
        }
    }
}

fn typecheck_todo<'hir>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    checked_expr: &ast::Expr,
) -> hir::Type<'hir> {
    //hb.error(
    //    ErrorComp::warning("this expression is not yet typechecked")
    //        .context(hb.get_scope(from_id).source(checked_expr.range)),
    //);
    hir::Type::Error
}

fn check_field_ty<'hir>(
    hb: &mut hb::HirBuilder<'_, '_, 'hir>,
    from_id: hir::ScopeID,
    ty: hir::Type<'hir>,
    name: ast::Ident,
) -> hir::Type<'hir> {
    match ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::Union(id) => {
            let data = hb.union_data(id);
            let find = data
                .members
                .iter()
                .enumerate()
                .find_map(|(id, member)| (member.name.id == name.id).then(|| member));
            match find {
                Some(member) => member.ty,
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on union type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.get_scope(from_id).source(name.range)),
                    );
                    hir::Type::Error
                }
            }
        }
        hir::Type::Struct(id) => {
            let data = hb.struct_data(id);
            let find = data
                .fields
                .iter()
                .enumerate()
                .find_map(|(id, field)| (field.name.id == name.id).then(|| field));
            match find {
                Some(field) => field.ty,
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on struct type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.get_scope(from_id).source(name.range)),
                    );
                    hir::Type::Error
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
                .context(hb.get_scope(from_id).source(name.range)),
            );
            hir::Type::Error
        }
    }
}

fn check_index_ty<'hir>(
    hb: &mut hb::HirBuilder<'_, '_, 'hir>,
    from_id: hir::ScopeID,
    ty: hir::Type<'hir>,
    index_range: TextRange,
) -> hir::Type<'hir> {
    match ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::ArraySlice(slice) => slice.ty,
        hir::Type::ArrayStatic(array) => array.ty,
        hir::Type::ArrayStaticDecl(array) => array.ty,
        _ => {
            let ty_format = type_format(hb, ty);
            hb.error(
                ErrorComp::error(format!("cannot index value of type {}", ty_format,))
                    .context(hb.get_scope(from_id).source(index_range)),
            );
            hir::Type::Error
        }
    }
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

fn path_resolve_target_scope<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<(hir::ScopeID, &'ast [ast::Ident])> {
    let mut target_id = match path.kind {
        ast::PathKind::None => origin_id,
        ast::PathKind::Super => match hb.get_scope(origin_id).parent() {
            Some(it) => it,
            None => {
                let range = TextRange::new(path.range_start, path.range_start + 5.into());
                hb.error(
                    ErrorComp::error("parent module `super` cannot be used from the root module")
                        .context(hb.get_scope(origin_id).source(range)),
                );
                return None;
            }
        },
        ast::PathKind::Package => hb::ROOT_SCOPE_ID,
    };

    let mut mod_count: usize = 0;
    for name in path.names {
        match hb.symbol_from_scope(origin_id, target_id, name.id) {
            Some((symbol, source)) => match symbol {
                hb::SymbolKind::Mod(id) => {
                    let data = hb.get_mod(id);
                    if let Some(new_target) = data.target {
                        mod_count += 1;
                        target_id = new_target;
                    } else {
                        hb.error(
                            ErrorComp::error(format!(
                                "module `{}` does not have its associated file",
                                hb.name_str(name.id)
                            ))
                            .context(hb.get_scope(origin_id).source(name.range))
                            .context_info("defined here", source),
                        );
                        return None;
                    }
                }
                _ => break,
            },
            None => {
                hb.error(
                    ErrorComp::error(format!("name `{}` is not found", hb.name_str(name.id)))
                        .context(hb.get_scope(origin_id).source(name.range)),
                );
                return None;
            }
        }
    }

    Some((target_id, &path.names[mod_count..]))
}

pub fn path_resolve_as_module_path<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> Option<hir::ScopeID> {
    let (target_id, names) = path_resolve_target_scope(hb, origin_id, path)?;

    match names.first() {
        Some(name) => {
            hb.error(
                ErrorComp::error(format!("`{}` is not a module", hb.name_str(name.id)))
                    .context(hb.get_scope(origin_id).source(name.range)),
            );
            None
        }
        _ => Some(target_id),
    }
}

pub fn path_resolve_as_type<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    origin_id: hir::ScopeID,
    path: &'ast ast::Path<'ast>,
) -> hir::Type<'hir> {
    let (target_id, names) = match path_resolve_target_scope(hb, origin_id, path) {
        Some(it) => it,
        None => return hir::Type::Error,
    };
    let mut names = names.iter();

    match names.next() {
        Some(name) => match hb.symbol_from_scope(origin_id, target_id, name.id) {
            Some((kind, source)) => {
                let ty = match kind {
                    hb::SymbolKind::Enum(id) => hir::Type::Enum(id),
                    hb::SymbolKind::Union(id) => hir::Type::Union(id),
                    hb::SymbolKind::Struct(id) => hir::Type::Struct(id),
                    _ => {
                        hb.error(
                            ErrorComp::error(format!("expected type, got other item",))
                                .context(hb.get_scope(origin_id).source(name.range))
                                .context_info("defined here", source),
                        );
                        return hir::Type::Error;
                    }
                };
                if let Some(next_name) = names.next() {
                    hb.error(
                        ErrorComp::error(format!("type cannot be accessed further",))
                            .context(hb.get_scope(origin_id).source(next_name.range))
                            .context_info("defined here", source),
                    );
                    return hir::Type::Error;
                }
                ty
            }
            None => {
                //@is a duplicate check
                // maybe module resolver can return a Option<(SymbolKind, SourceRange)>
                // which was seen before breaking
                hb.error(
                    ErrorComp::error(format!("name `{}` is not found", hb.name_str(name.id)))
                        .context(hb.get_scope(origin_id).source(name.range)),
                );
                hir::Type::Error
            }
        },
        None => {
            let path_range = TextRange::new(
                path.range_start,
                path.names.last().expect("non empty path").range.end(), //@just store path range in ast?
            );
            hb.error(
                ErrorComp::error(format!("expected type, got module path",))
                    .context(hb.get_scope(origin_id).source(path_range)),
            );
            hir::Type::Error
        }
    }
}

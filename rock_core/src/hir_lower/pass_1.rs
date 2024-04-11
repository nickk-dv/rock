use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;

pub fn run<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    hir.add_ast_modules();
    for origin_id in hir.scope_ids() {
        add_module_scope(hir, emit, origin_id);
    }
}

fn add_module_scope<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
) {
    for item in hir.scope_ast_items(origin_id) {
        match item {
            ast::Item::Proc(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => hir.add_proc(origin_id, item),
            },
            ast::Item::Enum(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => hir.add_enum(origin_id, item),
            },
            ast::Item::Union(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => hir.add_union(origin_id, item),
            },
            ast::Item::Struct(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing)
                }
                None => hir.add_struct(origin_id, item),
            },
            ast::Item::Const(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let value = super::pass_4::const_expr_resolve(hir, emit, origin_id, item.value);
                    let data = hir::ConstData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        ty: hir::Type::Error,
                        value,
                    };
                    hir.add_const(origin_id, item, data);
                }
            },
            ast::Item::Global(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let value = super::pass_4::const_expr_resolve(hir, emit, origin_id, item.value);
                    let data = hir::GlobalData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        ty: hir::Type::Error,
                        value,
                    };
                    hir.add_global(origin_id, item, data);
                }
            },
            ast::Item::Import(..) => {}
        }
    }
}

pub fn name_already_defined_error(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ScopeID,
    name: ast::Name,
    existing: SourceRange,
) {
    emit.error(ErrorComp::error(
        format!("name `{}` is defined multiple times", hir.name_str(name.id)),
        hir.src(origin_id, name.range),
        ErrorComp::info("existing definition", existing),
    ));
}

use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;

pub fn run<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for origin_id in hir.module_ids() {
        add_module_scope(hir, emit, origin_id);
    }
}

fn add_module_scope<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
) {
    for item in hir.module_ast_items(origin_id) {
        match item {
            ast::Item::Proc(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let id = hir.registry_mut().add_proc(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Proc(id),
                        },
                    );
                }
            },
            ast::Item::Enum(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let id = hir.registry_mut().add_enum(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Enum(id),
                        },
                    );
                }
            },
            ast::Item::Union(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let id = hir.registry_mut().add_union(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Union(id),
                        },
                    );
                }
            },
            ast::Item::Struct(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing)
                }
                None => {
                    let id = hir.registry_mut().add_struct(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Struct(id),
                        },
                    );
                }
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
                    let id = hir.registry_mut().add_const(item, data);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Const(id),
                        },
                    );
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
                    let id = hir.registry_mut().add_global(item, data);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Global(id),
                        },
                    );
                }
            },
            ast::Item::Import(..) => {}
        }
    }
}

pub fn name_already_defined_error(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ModuleID,
    name: ast::Name,
    existing: SourceRange,
) {
    emit.error(ErrorComp::error(
        format!("name `{}` is defined multiple times", hir.name_str(name.id)),
        hir.src(origin_id, name.range),
        ErrorComp::info("existing definition", existing),
    ));
}

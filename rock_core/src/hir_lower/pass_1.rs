use super::attr_check;
use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::error::{ErrorComp, Info, SourceRange};
use crate::hir;
use crate::session::{ModuleID, Session};

pub fn populate_scopes<'hir>(
    hir: &mut HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
) {
    for origin_id in session.module_ids() {
        add_module_items(hir, emit, session, origin_id);
    }
}

fn add_module_items<'hir>(
    hir: &mut HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
) {
    let module = hir.ast_module(origin_id);
    for item in module.items.iter().copied() {
        match item {
            ast::Item::Proc(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_proc_item(hir, emit, session, origin_id, item),
            },
            ast::Item::Enum(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_enum_item(hir, emit, session, origin_id, item),
            },
            ast::Item::Struct(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_struct_item(hir, emit, session, origin_id, item),
            },
            ast::Item::Const(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_const_item(hir, emit, session, origin_id, item),
            },
            ast::Item::Global(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_global_item(hir, emit, session, origin_id, item),
            },
            ast::Item::Import(item) => check_import_item(hir, emit, session, origin_id, item),
        }
    }
}

fn add_proc_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ProcItem<'ast>,
) {
    let feedback = attr_check::check_attrs_proc(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let data = hir::ProcData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        params: &[],
        return_ty: hir::Type::Error,
        block: None,
        locals: &[],
    };

    let proc_id = hir.registry_mut().add_proc(item, data);
    let symbol = Symbol::Defined(SymbolKind::Proc(proc_id));
    hir.add_symbol(origin_id, item.name.id, symbol);
}

fn add_enum_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::EnumItem<'ast>,
) {
    let feedback = attr_check::check_attrs_enum(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let data = hir::EnumData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        variants: &[],
        tag_ty: feedback.tag_ty,
        layout: hir::Eval::Unresolved(()),
    };

    let enum_id = hir.registry_mut().add_enum(item, data);
    let symbol = Symbol::Defined(SymbolKind::Enum(enum_id));
    hir.add_symbol(origin_id, item.name.id, symbol);
}

fn add_struct_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::StructItem<'ast>,
) {
    let feedback = attr_check::check_attrs_struct(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let data = hir::StructData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        fields: &[],
        layout: hir::Eval::Unresolved(()),
    };

    let struct_id = hir.registry_mut().add_struct(item, data);
    let symbol = Symbol::Defined(SymbolKind::Struct(struct_id));
    hir.add_symbol(origin_id, item.name.id, symbol);
}

fn add_const_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ConstItem<'ast>,
) {
    let feedback = attr_check::check_attrs_const(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let registry = hir.registry_mut();
    let eval_id = registry.add_const_eval(item.value, origin_id);

    let data = hir::ConstData {
        origin_id,
        vis: item.vis,
        name: item.name,
        ty: hir::Type::Error,
        value: eval_id,
    };

    let const_id = registry.add_const(item, data);
    let symbol = Symbol::Defined(SymbolKind::Const(const_id));
    hir.add_symbol(origin_id, item.name.id, symbol);
}

fn add_global_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::GlobalItem<'ast>,
) {
    let feedback = attr_check::check_attrs_global(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let registry = hir.registry_mut();
    let eval_id = registry.add_const_eval(item.value, origin_id);

    let data = hir::GlobalData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        mutt: item.mutt,
        name: item.name,
        ty: hir::Type::Error,
        value: eval_id,
    };

    let global_id = registry.add_global(item, data);
    let symbol = Symbol::Defined(SymbolKind::Global(global_id));
    hir.add_symbol(origin_id, item.name.id, symbol);
}

fn check_import_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ImportItem<'ast>,
) {
    let feedback = attr_check::check_attrs_import(hir, emit, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let data = hir::ImportData { origin_id };
    let _ = hir.registry_mut().add_import(item, data);
}

//@move to `::errors`
pub fn error_name_already_defined(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    name: ast::Name,
    existing: SourceRange,
) {
    emit.error(ErrorComp::new(
        format!("name `{}` is defined multiple times", hir.name_str(name.id)),
        SourceRange::new(origin_id, name.range),
        Info::new("existing definition", existing),
    ));
}

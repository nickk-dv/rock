use super::attr_check;
use super::context::{HirCtx, Symbol, SymbolKind};
use crate::ast;
use crate::hir;
use crate::session::{ModuleID, Session};

pub fn populate_scopes(ctx: &mut HirCtx, session: &Session) {
    for origin_id in session.module_ids() {
        add_module_items(ctx, session, origin_id);
    }
}

fn add_module_items(ctx: &mut HirCtx, session: &Session, origin_id: ModuleID) {
    let module = ctx.ast_module(origin_id);
    for item in module.items.iter().copied() {
        match item {
            ast::Item::Proc(item) => add_proc_item(ctx, session, origin_id, item),
            ast::Item::Enum(item) => add_enum_item(ctx, session, origin_id, item),
            ast::Item::Struct(item) => add_struct_item(ctx, session, origin_id, item),
            ast::Item::Const(item) => add_const_item(ctx, session, origin_id, item),
            ast::Item::Global(item) => add_global_item(ctx, session, origin_id, item),
            ast::Item::Import(item) => check_import_item(ctx, session, origin_id, item),
        }
    }
}

fn add_proc_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ProcItem,
) {
    let feedback = attr_check::check_attrs_proc(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, item.name)
    {
        error.emit(ctx);
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

    let proc_id = ctx.registry.add_proc(item, data);
    let symbol = Symbol::Defined(SymbolKind::Proc(proc_id));
    ctx.scope.add_symbol(origin_id, item.name.id, symbol);
}

fn add_enum_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::EnumItem,
) {
    let feedback = attr_check::check_attrs_enum(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, item.name)
    {
        error.emit(ctx);
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

    let enum_id = ctx.registry.add_enum(item, data);
    let symbol = Symbol::Defined(SymbolKind::Enum(enum_id));
    ctx.scope.add_symbol(origin_id, item.name.id, symbol);
}

fn add_struct_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::StructItem,
) {
    let feedback = attr_check::check_attrs_struct(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, item.name)
    {
        error.emit(ctx);
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

    let struct_id = ctx.registry.add_struct(item, data);
    let symbol = Symbol::Defined(SymbolKind::Struct(struct_id));
    ctx.scope.add_symbol(origin_id, item.name.id, symbol);
}

fn add_const_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ConstItem,
) {
    let feedback = attr_check::check_attrs_const(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, item.name)
    {
        error.emit(ctx);
        return;
    }

    let eval_id = ctx.registry.add_const_eval(item.value, origin_id);
    let data = hir::ConstData {
        origin_id,
        vis: item.vis,
        name: item.name,
        ty: hir::Type::Error,
        value: eval_id,
    };

    let const_id = ctx.registry.add_const(item, data);
    let symbol = Symbol::Defined(SymbolKind::Const(const_id));
    ctx.scope.add_symbol(origin_id, item.name.id, symbol);
}

fn add_global_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::GlobalItem,
) {
    let feedback = attr_check::check_attrs_global(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, item.name)
    {
        error.emit(ctx);
        return;
    }

    let eval_id = ctx.registry.add_const_eval(item.value, origin_id);
    let data = hir::GlobalData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        mutt: item.mutt,
        name: item.name,
        ty: hir::Type::Error,
        value: eval_id,
    };

    let global_id = ctx.registry.add_global(item, data);
    let symbol = Symbol::Defined(SymbolKind::Global(global_id));
    ctx.scope.add_symbol(origin_id, item.name.id, symbol);
}

fn check_import_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast>,
    session: &Session,
    origin_id: ModuleID,
    item: &'ast ast::ImportItem,
) {
    let feedback = attr_check::check_attrs_import(ctx, session, origin_id, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let data = hir::ImportData { origin_id };
    let _ = ctx.registry.add_import(item, data);
}

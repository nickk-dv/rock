use super::attr_check;
use super::context::scope::{Symbol, SymbolID};
use super::context::HirCtx;
use crate::ast;
use crate::hir;

pub fn populate_scopes(ctx: &mut HirCtx) {
    for module_id in ctx.session.module.ids() {
        ctx.scope.set_origin(module_id);
        let module = ctx.session.module.get(module_id);
        let items = module.ast_expect().items;

        for item in items.iter().copied() {
            match item {
                ast::Item::Proc(item) => add_proc_item(ctx, item),
                ast::Item::Enum(item) => add_enum_item(ctx, item),
                ast::Item::Struct(item) => add_struct_item(ctx, item),
                ast::Item::Const(item) => add_const_item(ctx, item),
                ast::Item::Global(item) => add_global_item(ctx, item),
                ast::Item::Import(item) => add_import_item(ctx, item),
            }
        }
    }
}

fn add_proc_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::ProcItem) {
    let feedback = attr_check::check_attrs_proc(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin();
    let data = hir::ProcData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        params: &[],
        return_ty: hir::Type::Error,
        block: None,
        locals: &[],
        local_binds: &[],
        for_binds: &[],
    };

    let proc_id = ctx.registry.add_proc(item, data);
    let symbol = Symbol::Defined(SymbolID::Proc(proc_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_enum_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::EnumItem) {
    let feedback = attr_check::check_attrs_enum(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin();
    let data = hir::EnumData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        variants: &[],
        tag_ty: hir::Eval::Unresolved(()),
        layout: hir::Eval::Unresolved(()),
    };

    let enum_id = ctx.registry.add_enum(item, data);
    let symbol = Symbol::Defined(SymbolID::Enum(enum_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_struct_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::StructItem) {
    let feedback = attr_check::check_attrs_struct(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin();
    let data = hir::StructData {
        origin_id,
        attr_set: feedback.attr_set,
        vis: item.vis,
        name: item.name,
        fields: &[],
        layout: hir::Eval::Unresolved(()),
    };

    let struct_id = ctx.registry.add_struct(item, data);
    let symbol = Symbol::Defined(SymbolID::Struct(struct_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_const_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::ConstItem) {
    let feedback = attr_check::check_attrs_const(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin();
    let eval_id = ctx.registry.add_const_eval(item.value, origin_id);
    let data = hir::ConstData {
        origin_id,
        vis: item.vis,
        name: item.name,
        ty: hir::Type::Error,
        value: eval_id,
    };

    let const_id = ctx.registry.add_const(item, data);
    let symbol = Symbol::Defined(SymbolID::Const(const_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_global_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::GlobalItem) {
    let feedback = attr_check::check_attrs_global(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin();
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
    let symbol = Symbol::Defined(SymbolID::Global(global_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_import_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::ImportItem) {
    let feedback = attr_check::check_attrs_import(ctx, item);
    if feedback.cfg_state.disabled() {
        return;
    }

    let origin_id = ctx.scope.origin();
    let data = hir::ImportData { origin_id };
    let _ = ctx.registry.add_import(item, data);
}

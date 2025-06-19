use super::check_directive;
use super::context::HirCtx;
use super::scope::{self, PolyScope, Symbol, SymbolID};
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::support::{AsStr, BitSet};

pub fn populate_scopes(ctx: &mut HirCtx) {
    for module_id in ctx.session.module.ids() {
        ctx.scope.origin = module_id;
        let module = ctx.session.module.get(module_id);
        let items = module.ast_expect().items;
        let mut scope_vis = hir::Vis::Public;
        let mut prev_scope: Option<&ast::Directive> = None;

        for item in items.iter().copied() {
            match item {
                ast::Item::Proc(item) => add_proc_item(ctx, item, scope_vis),
                ast::Item::Enum(item) => add_enum_item(ctx, item, scope_vis),
                ast::Item::Struct(item) => add_struct_item(ctx, item, scope_vis),
                ast::Item::Const(item) => add_const_item(ctx, item, scope_vis),
                ast::Item::Global(item) => add_global_item(ctx, item, scope_vis),
                ast::Item::Import(item) => add_import_item(ctx, item),
                ast::Item::Directive(item) => {
                    check_directive_item(ctx, item, &mut scope_vis, &mut prev_scope)
                }
            }
        }
    }

    ctx.core.start = scope::find_core_proc(ctx, "runtime", "start");
    ctx.core.index_out_of_bounds = scope::find_core_proc(ctx, "runtime", "index_out_of_bounds");
    ctx.core.slice_range = scope::find_core_proc(ctx, "ops", "slice_range");
    ctx.core.string_equals = scope::find_core_proc(ctx, "ops", "string_equals");
    ctx.core.cstring_equals = scope::find_core_proc(ctx, "ops", "cstring_equals");
    ctx.core.from_raw_parts = scope::find_core_proc(ctx, "mem", "from_raw_parts");
    ctx.core.range_bound = scope::find_core_enum_opt(ctx, "ops", "RangeBound");
    ctx.core.array = scope::find_core_struct(ctx, "array", "Array");

    ctx.core.any = scope::find_core_struct_opt(ctx, "type", "Any");
    ctx.core.type_info = scope::find_core_enum(ctx, "type", "TypeInfo");
    ctx.core.int_ty = scope::find_core_enum(ctx, "type", "IntType");
    ctx.core.float_ty = scope::find_core_enum(ctx, "type", "FloatType");
    ctx.core.bool_ty = scope::find_core_enum(ctx, "type", "BoolType");
    ctx.core.string_ty = scope::find_core_enum(ctx, "type", "StringType");
    ctx.core.type_info_enum = scope::find_core_struct(ctx, "type", "TypeInfo_Enum");
    ctx.core.type_info_variant = scope::find_core_struct(ctx, "type", "TypeInfo_Variant");
    ctx.core.type_info_variant_field =
        scope::find_core_struct(ctx, "type", "TypeInfo_VariantField");
    ctx.core.type_info_struct = scope::find_core_struct(ctx, "type", "TypeInfo_Struct");
    ctx.core.type_info_field = scope::find_core_struct(ctx, "type", "TypeInfo_Field");
    ctx.core.type_info_reference = scope::find_core_struct(ctx, "type", "TypeInfo_Reference");
    ctx.core.type_info_procedure = scope::find_core_struct(ctx, "type", "TypeInfo_Procedure");
    ctx.core.type_info_array_slice = scope::find_core_struct(ctx, "type", "TypeInfo_ArraySlice");
    ctx.core.type_info_array_static = scope::find_core_struct(ctx, "type", "TypeInfo_ArrayStatic");

    ctx.core.source_loc = scope::find_core_struct_opt(ctx, "runtime", "SourceLocation");
}

fn add_proc_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    item: &'ast ast::ProcItem,
    scope_vis: hir::Vis,
) {
    let (config, flag_set) = check_directive::check_proc_directives(ctx, item);
    if config.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin;
    let data = hir::ProcData {
        origin_id,
        flag_set,
        vis: scope_vis,
        name: item.name,
        poly_params: None,
        params: &[],
        return_ty: hir::Type::Error,
        block: None,
        variables: &[],
    };

    let proc_id = ctx.registry.add_proc(item, data);
    let symbol = Symbol::Defined(SymbolID::Proc(proc_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_enum_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    item: &'ast ast::EnumItem,
    scope_vis: hir::Vis,
) {
    let (config, flag_set) = check_directive::check_enum_directives(ctx, item);
    if config.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin;
    let data = hir::EnumData {
        origin_id,
        flag_set,
        vis: scope_vis,
        name: item.name,
        poly_params: None,
        variants: &[],
        tag_ty: hir::Eval::Unresolved(()),
        layout: hir::Eval::Unresolved(()),
        type_usage: hir::Eval::Unresolved(()),
    };

    let enum_id = ctx.registry.add_enum(item, data);
    let symbol = Symbol::Defined(SymbolID::Enum(enum_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_struct_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    item: &'ast ast::StructItem,
    scope_vis: hir::Vis,
) {
    let config = check_directive::check_expect_config(ctx, item.dir_list, "structs");
    if config.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin;
    let data = hir::StructData {
        origin_id,
        flag_set: BitSet::empty(),
        vis: scope_vis,
        name: item.name,
        poly_params: None,
        fields: &[],
        layout: hir::Eval::Unresolved(()),
        type_usage: hir::Eval::Unresolved(()),
    };

    let struct_id = ctx.registry.add_struct(item, data);
    let symbol = Symbol::Defined(SymbolID::Struct(struct_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_const_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    item: &'ast ast::ConstItem,
    scope_vis: hir::Vis,
) {
    let config = check_directive::check_expect_config(ctx, item.dir_list, "constants");
    if config.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin;
    let eval_id = ctx.registry.add_const_eval(item.value, origin_id, PolyScope::None);

    let data = hir::ConstData {
        origin_id,
        flag_set: BitSet::empty(),
        vis: scope_vis,
        name: item.name,
        ty: None,
        value: eval_id,
    };

    let const_id = ctx.registry.add_const(item, data);
    let symbol = Symbol::Defined(SymbolID::Const(const_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_global_item<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    item: &'ast ast::GlobalItem,
    scope_vis: hir::Vis,
) {
    let config = check_directive::check_expect_config(ctx, item.dir_list, "globals");
    if config.disabled() {
        return;
    }

    if ctx
        .scope
        .check_already_defined_global(item.name, ctx.session, &ctx.registry, &mut ctx.emit)
        .is_err()
    {
        return;
    }

    let origin_id = ctx.scope.origin;
    let init = match item.init {
        ast::GlobalInit::Init(value) => {
            let eval_id = ctx.registry.add_const_eval(value, origin_id, PolyScope::None);
            hir::GlobalInit::Init(eval_id)
        }
        ast::GlobalInit::Zeroed => hir::GlobalInit::Zeroed,
    };

    let data = hir::GlobalData {
        origin_id,
        flag_set: BitSet::empty(),
        vis: scope_vis,
        mutt: item.mutt,
        name: item.name,
        ty: hir::Type::Error,
        init,
    };

    let global_id = ctx.registry.add_global(item, data);
    let symbol = Symbol::Defined(SymbolID::Global(global_id));
    ctx.scope.global.add_symbol(origin_id, item.name.id, symbol);
}

fn add_import_item<'ast>(ctx: &mut HirCtx<'_, 'ast, '_>, item: &'ast ast::ImportItem) {
    let config = check_directive::check_expect_config(ctx, item.dir_list, "imports");
    if config.disabled() {
        return;
    }

    let origin_id = ctx.scope.origin;
    let data = hir::ImportData { origin_id };
    let _ = ctx.registry.add_import(item, data);
}

fn check_directive_item<'ast>(
    ctx: &mut HirCtx,
    item: &ast::DirectiveList<'ast>,
    scope_vis: &mut hir::Vis,
    prev_scope: &mut Option<&ast::Directive<'ast>>,
) {
    let (scope_dir, first) = match item.directives.split_last() {
        Some(value) => value,
        None => return,
    };
    for directive in first {
        if check_directive::try_check_error_directive(ctx, directive) {
            continue;
        }
        let src = ctx.src(directive.range);
        let name = directive.kind.as_str();
        err::directive_cannot_apply(&mut ctx.emit, src, name, "directives");
    }
    let new_vis = match scope_dir.kind {
        ast::DirectiveKind::ScopePublic => hir::Vis::Public,
        ast::DirectiveKind::ScopePackage => hir::Vis::Package,
        ast::DirectiveKind::ScopePrivate => hir::Vis::Private,
        _ => unreachable!(),
    };
    if *scope_vis == new_vis {
        let src = ctx.src(scope_dir.range);
        let prev_src = prev_scope.map(|s| ctx.src(s.range));
        err::directive_scope_vis_redundant(&mut ctx.emit, src, prev_src, scope_vis.as_str());
    }
    *scope_vis = new_vis;
    *prev_scope = Some(scope_dir);
}

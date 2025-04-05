use super::check_directive;
use super::context::HirCtx;
use crate::errors as err;
use crate::hir;
use crate::package::manifest::PackageKind;
use crate::session::{ModuleID, ModuleOrDirectory};

pub fn check_entry_point(ctx: &mut HirCtx) {
    let root_package = ctx.session.graph.package(ctx.session.root_id);
    if root_package.manifest().package.kind != PackageKind::Bin {
        return;
    }

    let main_id = match ctx.session.intern_name.get_id("main") {
        Some(main_id) => main_id,
        None => {
            err::entry_main_mod_not_found(&mut ctx.emit);
            return;
        }
    };

    let module_or_directory = root_package.src().find(ctx.session, main_id);
    let target_id = match module_or_directory {
        ModuleOrDirectory::Module(module_id) => module_id,
        _ => {
            err::entry_main_mod_not_found(&mut ctx.emit);
            return;
        }
    };

    if let Some(proc_id) = ctx.scope.global.find_defined_proc(target_id, main_id) {
        check_main_procedure(ctx, proc_id);
    } else {
        err::entry_main_proc_not_found(&mut ctx.emit);
    }
}

fn check_main_procedure(ctx: &mut HirCtx, proc_id: hir::ProcID) {
    let data = ctx.registry.proc_data_mut(proc_id);
    let mut flag_set = data.flag_set;
    let main_src = data.src();
    ctx.scope.set_origin(data.origin_id);

    check_directive::apply_item_flag(
        ctx,
        &mut flag_set,
        hir::ProcFlag::EntryPoint,
        main_src,
        None,
        "procedures",
    );

    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data_mut(proc_id);
    data.flag_set = flag_set;

    if !data.params.is_empty() {
        err::entry_main_with_parameters(&mut ctx.emit, main_src);
    }
    if !data.return_ty.is_error()
        && !data.return_ty.is_never()
        && !matches!(data.return_ty, hir::Type::Int(hir::IntType::S32))
    {
        let ret_src = ctx.src(item.return_ty.range);
        err::entry_main_wrong_return_ty(&mut ctx.emit, ret_src);
    }
}

pub fn check_unused_items(ctx: &mut HirCtx) {
    for proc_id in ctx.registry.proc_ids() {
        let data = ctx.registry.proc_data(proc_id);
        if !data.flag_set.contains(hir::ProcFlag::WasUsed)
            && !data.flag_set.contains(hir::ProcFlag::EntryPoint)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_symbol_unused(&mut ctx.emit, data.src(), name, "procedure");
        }
    }

    for enum_id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(enum_id);
        if !data.flag_set.contains(hir::EnumFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_symbol_unused(&mut ctx.emit, data.src(), name, "enum");
        }
    }

    for struct_id in ctx.registry.struct_ids() {
        let data = ctx.registry.struct_data(struct_id);
        if !data.flag_set.contains(hir::StructFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_symbol_unused(&mut ctx.emit, data.src(), name, "struct");
        }
    }

    for const_id in ctx.registry.const_ids() {
        let data = ctx.registry.const_data(const_id);
        if !data.flag_set.contains(hir::ConstFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_symbol_unused(&mut ctx.emit, data.src(), name, "const");
        }
    }

    for global_id in ctx.registry.global_ids() {
        let data = ctx.registry.global_data(global_id);
        if !data.flag_set.contains(hir::GlobalFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_symbol_unused(&mut ctx.emit, data.src(), name, "global");
        }
    }
}

#[inline(always)]
fn module_is_library(ctx: &HirCtx, origin_id: ModuleID) -> bool {
    let module = ctx.session.module.get(origin_id);
    let package = ctx.session.graph.package(module.origin());
    package.manifest().package.kind == PackageKind::Lib
}

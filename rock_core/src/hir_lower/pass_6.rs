use super::context::HirCtx;
use super::pass_5::{self, Expectation};
use super::scope::Symbol;
use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::session::{manifest::PackageKind, ModuleID, ModuleOrDirectory};
use crate::support::BitSet;

pub fn check_entry_point(ctx: &mut HirCtx) {
    let root_package = ctx.session.graph.package(ctx.session.root_id);
    if root_package.manifest.package.kind != PackageKind::Bin {
        return;
    }

    let name_id = ctx.session.intern_name.intern("main");
    let ModuleOrDirectory::Module(target_id) = root_package.src.find(ctx.session, name_id) else {
        err::entry_main_mod_not_found(&mut ctx.emit);
        return;
    };
    let Some(proc_id) = ctx.scope.global.find_defined_proc(target_id, name_id) else {
        err::entry_main_proc_not_found(&mut ctx.emit);
        return;
    };

    ctx.entry_point = Some(proc_id);
    let data = ctx.registry.proc_data(proc_id);
    let range = data.name.range;
    ctx.scope.origin = data.origin_id;

    let ty = hir::ProcType { flag_set: BitSet::empty(), params: &[], return_ty: hir::Type::Void };
    let ty = hir::Type::Procedure(ctx.arena.alloc(ty));
    let expect = Expectation::HasType(ty, None);

    let type_res = pass_5::check_item_procedure(ctx, expect, proc_id, None, range);
    pass_5::type_expectation_check(ctx, range, type_res.ty, expect);
    ctx.registry.proc_data_mut(proc_id).flag_set.set(hir::ProcFlag::EntryPoint);
}

pub fn check_unused_items(ctx: &mut HirCtx) {
    for proc_id in ctx.registry.proc_ids() {
        let data = ctx.registry.proc_data(proc_id);
        if !data.flag_set.contains(hir::ProcFlag::WasUsed)
            && !data.flag_set.contains(hir::ProcFlag::EntryPoint)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_unused_symbol(&mut ctx.emit, data.src(), name, "procedure");
        }
    }

    for enum_id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(enum_id);
        if !data.flag_set.contains(hir::EnumFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_unused_symbol(&mut ctx.emit, data.src(), name, "enum");
        }
    }

    for struct_id in ctx.registry.struct_ids() {
        let data = ctx.registry.struct_data(struct_id);
        if !data.flag_set.contains(hir::StructFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_unused_symbol(&mut ctx.emit, data.src(), name, "struct");
        }
    }

    for const_id in ctx.registry.const_ids() {
        let data = ctx.registry.const_data(const_id);
        if !data.flag_set.contains(hir::ConstFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_unused_symbol(&mut ctx.emit, data.src(), name, "const");
        }
    }

    for global_id in ctx.registry.global_ids() {
        let data = ctx.registry.global_data(global_id);
        if !data.flag_set.contains(hir::GlobalFlag::WasUsed)
            && !(data.vis == hir::Vis::Public && module_is_library(ctx, data.origin_id))
        {
            let name = ctx.name(data.name.id);
            err::scope_unused_symbol(&mut ctx.emit, data.src(), name, "global");
        }
    }

    let mut buf = String::with_capacity(64);
    for module_id in ctx.session.module.ids() {
        let scope = ctx.scope.global.module(module_id);

        for (import_name, symbol) in scope.symbols.iter() {
            match symbol {
                Symbol::Defined(_) => continue,
                Symbol::Imported(symbol_id, range, was_used) => {
                    if !*was_used {
                        buf.clear();
                        buf.push_str("imported");
                        buf.push(' ');
                        buf.push_str(symbol_id.desc());
                        let name = ctx.name(*import_name);
                        let src = SourceRange::new(module_id, *range);
                        err::scope_unused_symbol(&mut ctx.emit, src, name, &buf)
                    }
                }
                Symbol::ImportedModule(_, range, was_used) => {
                    if !*was_used {
                        let name = ctx.name(*import_name);
                        let src = SourceRange::new(module_id, *range);
                        err::scope_unused_symbol(&mut ctx.emit, src, name, "imported module");
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn module_is_library(ctx: &HirCtx, origin_id: ModuleID) -> bool {
    let module = ctx.session.module.get(origin_id);
    let package = ctx.session.graph.package(module.origin);
    package.manifest.package.kind == PackageKind::Lib
}

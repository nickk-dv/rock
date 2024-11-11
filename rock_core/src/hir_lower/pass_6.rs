use super::attr_check;
use super::context::HirCtx;
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::package::manifest::PackageKind;
use crate::session::ModuleOrDirectory;

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

    let module_or_directory = root_package.src().find(&ctx.session, main_id);
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

fn check_main_procedure<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, proc_id: hir::ProcID) {
    let data = ctx.registry.proc_data_mut(proc_id);
    let main_src = data.src();
    ctx.scope.set_origin(data.origin_id);

    attr_check::apply_item_flag(
        &mut ctx.emit,
        &mut data.attr_set,
        hir::ProcFlag::Main,
        None,
        main_src,
        "procedures",
    );

    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data(proc_id);

    if !data.params.is_empty() {
        err::entry_main_with_parameters(&mut ctx.emit, main_src);
    }

    if !data.return_ty.is_error()
        && !data.return_ty.is_never()
        && !matches!(data.return_ty, hir::Type::Basic(ast::BasicType::S32))
    {
        let ret_src = ctx.src(item.return_ty.range);
        err::entry_main_wrong_return_ty(&mut ctx.emit, ret_src);
    }
}

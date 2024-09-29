use super::attr_check;
use super::context::{HirCtx, SymbolKind};
use crate::ast;
use crate::error::{Error, ErrorSink, SourceRange};
use crate::hir;
use crate::package::manifest::PackageKind;
use crate::session::{ModuleOrDirectory, Session};
use crate::text::TextRange;

pub fn check_entry_point(ctx: &mut HirCtx) {
    let root_package = ctx.session.pkg_storage.package(Session::ROOT_ID);
    let root_manifest = root_package.manifest();
    if root_manifest.package.kind != PackageKind::Bin {
        return;
    }

    let main_id = match ctx.intern_name().get_id("main") {
        Some(main_id) => main_id,
        None => {
            ctx.emit.error(Error::message(
                "could not find `main` module, expected `src/main.rock` to exist",
            ));
            return;
        }
    };

    let module_or_directory = root_package.src.find(&ctx.session.pkg_storage, main_id);

    let origin_id = match module_or_directory {
        ModuleOrDirectory::Module(module_id) => module_id,
        _ => {
            ctx.emit.error(Error::message(
                "could not find `main` module, expected `src/main.rock` to exist",
            ));
            return;
        }
    };

    let main_name = ast::Name {
        id: main_id,
        range: TextRange::zero(),
    };
    let defined = ctx
        .scope
        .symbol_from_scope(&ctx.registry, origin_id, origin_id, main_name);

    if let Ok(SymbolKind::Proc(proc_id)) = defined {
        check_main_procedure(ctx, proc_id);
    } else {
        ctx.emit.error(Error::message(
            "could not find entry point in `src/main.rock`\ndefine it like this: `proc main() -> s32 { return 0; }`",
        ));
    }
}

pub fn check_main_procedure<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, proc_id: hir::ProcID<'hir>) {
    let data = ctx.registry.proc_data_mut(proc_id);
    let flag = hir::ProcFlag::Main;
    let item_src = SourceRange::new(data.origin_id, data.name.range);

    attr_check::check_attr_flag(
        &mut ctx.emit,
        flag,
        &mut data.attr_set,
        None,
        item_src,
        "procedures",
    );

    let item = ctx.registry.proc_item(proc_id);
    let data = ctx.registry.proc_data(proc_id);

    if !data.params.is_empty() {
        ctx.emit.error(Error::new(
            "`main` procedure cannot have any parameters",
            item_src,
            None,
        ));
    }

    //@allow `never`?
    if !matches!(
        data.return_ty,
        hir::Type::Error | hir::Type::Basic(ast::BasicType::S32)
    ) {
        ctx.emit.error(Error::new(
            "`main` procedure must return `s32`",
            SourceRange::new(data.origin_id, item.return_ty.range),
            None,
        ));
    }
}

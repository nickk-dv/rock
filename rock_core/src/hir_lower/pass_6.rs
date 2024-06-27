use super::hir_build::{HirData, HirEmit, SymbolKind};
use crate::ast::BasicType;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::package::manifest::PackageKind;
use crate::session::{ModuleOrDirectory, Session};

pub fn check_entry_point<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
) {
    let root_package = session.package(Session::ROOT_ID);
    let root_manifest = root_package.manifest();
    if root_manifest.package.kind != PackageKind::Bin {
        return;
    }

    let main_id = hir.intern_name().intern("main");
    let module_or_directory = root_package.src.find(session, main_id);
    let origin_id = match module_or_directory {
        ModuleOrDirectory::Module(module_id) => module_id,
        _ => {
            emit.error(ErrorComp::message(
                "could not find `main` module, expected `src/main.rock` to exist",
            ));
            return;
        }
    };

    let defined = hir.symbol_get_defined(origin_id, main_id);
    let proc_id = if let Some(SymbolKind::Proc(proc_id)) = defined {
        proc_id
    } else {
        emit.error(ErrorComp::message(
            "could not find entry point in `src/main.rock`\ndefine it like this: `proc main() -> s32 { return 0; }`",
        ));
        return;
    };

    check_main_procedure(hir, emit, proc_id);
}

pub fn check_main_procedure<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc_id: hir::ProcID,
) {
    let item = hir.registry().proc_item(proc_id);
    let data = hir.registry().proc_data(proc_id);
    let external = item.block.is_none();
    let name_src = SourceRange::new(data.origin_id, data.name.range);

    if !data.params.is_empty() {
        emit.error(ErrorComp::new(
            "main procedure cannot have any parameters",
            name_src,
            None,
        ));
    }

    //@allow `never`?
    if !matches!(
        data.return_ty,
        hir::Type::Error | hir::Type::Basic(BasicType::S32)
    ) {
        let ty_range = if let Some(ty) = item.return_ty {
            ty.range
        } else {
            data.name.range
        };
        emit.error(ErrorComp::new(
            "main procedure must return `s32`",
            SourceRange::new(data.origin_id, ty_range),
            None,
        ));
    }

    //@convert those into a bitfield
    if external {
        emit.error(ErrorComp::new(
            "main procedure cannot be external, define the entry block",
            name_src,
            None,
        ));
    }

    if data.is_test {
        emit.error(ErrorComp::new(
            "main procedure cannot be a test, remove #[test] attribute",
            name_src,
            None,
        ));
    }
}

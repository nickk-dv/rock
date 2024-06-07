use super::hir_build::{HirData, HirEmit, SymbolKind};
use crate::ast::BasicType;
use crate::error::ErrorComp;
use crate::hir;
use crate::session::PackageID;

pub fn check_entry_point<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    executable: bool,
) {
    if !executable {
        return;
    }

    if let Some(main_id) = hir.intern().get_id("main") {
        if let Some(module_id) = hir.get_package_module_id(PackageID::new(0), main_id) {
            let defined = hir.symbol_get_defined(module_id, main_id);
            match defined {
                Some(SymbolKind::Proc(proc_id)) => check_main_procedure(hir, emit, proc_id),
                _ => {
                    emit.error(ErrorComp::message(
                        "could not find entry point in `src/main.rock`\ndefine it like this: `proc main() -> s32 { return 0; }`",
                    ));
                    return;
                }
            }
        }
    }

    emit.error(ErrorComp::message(
        "could not find `main` module, expected `src/main.rock` to exist",
    ));
}

pub fn check_main_procedure<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc_id: hir::ProcID,
) {
    let item = hir.registry().proc_item(proc_id);
    let data = hir.registry().proc_data(proc_id);
    let name_src = hir.src(data.origin_id, data.name.range);

    if !data.params.is_empty() {
        emit.error(ErrorComp::new(
            "main procedure cannot have any parameters",
            name_src,
            None,
        ));
    }

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
            hir.src(data.origin_id, ty_range),
            None,
        ));
    }

    if item.block.is_none() {
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

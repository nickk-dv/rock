mod hir_build;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_4;
mod pass_5;
mod proc_scope;

use crate::ast;
use crate::error::{DiagnosticCollection, WarningComp};
use crate::hir;
use crate::session::Session;
use hir_build::{HirData, HirEmit};

//@use ast::Type range in diagnostics, update any work arounds of missing range 20.05.24
pub fn check<'hir, 'ast, 'intern: 'hir>(
    ast: ast::Ast<'ast, 'intern>,
    session: &Session,
) -> Result<(hir::Hir<'hir>, Vec<WarningComp>), DiagnosticCollection> {
    let mut hir = HirData::new(ast, session);
    let mut emit = HirEmit::new();
    pass_1::populate_scopes(&mut hir, &mut emit);
    pass_2::resolve_imports(&mut hir, &mut emit);
    pass_3::process_items(&mut hir, &mut emit);
    pass_4::resolve_const_dependencies(&mut hir, &mut emit);
    pass_5::typecheck_procedures(&mut hir, &mut emit);
    emit.emit(hir)
}

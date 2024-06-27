mod hir_build;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_4;
mod pass_5;
mod pass_6;
mod proc_scope;

use crate::ast;
use crate::error::ResultComp;
use crate::hir;
use crate::session::Session;
use hir_build::{HirData, HirEmit};

pub fn check<'hir, 'ast, 'intern: 'hir>(
    ast: ast::Ast<'ast, 'intern>,
    session: &Session,
) -> ResultComp<hir::Hir<'hir>> {
    let mut hir = HirData::new(ast);
    let mut emit = HirEmit::new();
    pass_1::populate_scopes(&mut hir, &mut emit, session);
    pass_2::resolve_imports(&mut hir, &mut emit, session);
    pass_3::process_items(&mut hir, &mut emit);
    pass_4::resolve_const_dependencies(&mut hir, &mut emit);
    pass_5::typecheck_procedures(&mut hir, &mut emit);
    pass_6::check_entry_point(&mut hir, &mut emit, session);
    emit.emit(hir)
}

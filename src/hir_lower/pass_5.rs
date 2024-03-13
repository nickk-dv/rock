use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity};
use crate::hir;
use crate::hir::hir_builder as hb;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    //
}

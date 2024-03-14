use super::hir_builder as hb;
use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity};
use crate::hir;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    //
}

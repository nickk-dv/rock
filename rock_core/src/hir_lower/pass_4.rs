use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

//@const exprs are not part of typical typechecking pipeline:
// const and global item types are not checked againts their values,
// currently just assuming that those types match the literals of the expresssion itself.
//@would need to come up with a way to create a good integration for constants,
// which isnt possible yet due to apis and code structures constantly changing.
// leaving this unfinished on purpose.
pub fn const_expr_resolve<'hir, 'ast>(
    hir: &HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    expr: ast::ConstExpr<'ast>,
) -> hir::ConstExpr<'hir> {
    let hir_expr = match expr.0.kind {
        ast::ExprKind::Unit => hir::Expr::Unit, //@ units are no-ops, should they be stripped fully before LLVM ?
        ast::ExprKind::LitNull => hir::Expr::LitNull,
        ast::ExprKind::LitBool { val } => hir::Expr::LitBool { val },
        ast::ExprKind::LitInt { val } => hir::Expr::LitInt {
            val,
            ty: ast::BasicType::U64, //@always u64
        },
        ast::ExprKind::LitFloat { val } => hir::Expr::LitFloat {
            val,
            ty: ast::BasicType::F64, //@always f64
        },
        ast::ExprKind::LitChar { val } => hir::Expr::LitChar { val },
        ast::ExprKind::LitString { id, c_string } => hir::Expr::LitString { id, c_string },
        _ => {
            emit.error(ErrorComp::error(
                "constant expressions only support single literals so far",
                hir.src(origin_id, expr.0.range),
                None,
            ));
            hir::Expr::Error
        }
    };
    hir::ConstExpr(emit.arena.alloc(hir_expr))
}

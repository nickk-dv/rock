use super::hir_builder as hb;
use crate::ast::ast;
use crate::hir;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let decl = hb.proc_ast(id);
    if let Some(block) = decl.block {
        let ty = typecheck_expr(hb, block);
        //println!(
        //    "procedure `{}` block has type: {}",
        //    hb.name_str(decl.name.id).to_string(),
        //    type_format(hb, ty)
        //);
    }
}

fn type_format(hb: &mut hb::HirBuilder, ty: hir::Type) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
            ast::BasicType::Unit => "()".into(),
            ast::BasicType::Bool => "bool".into(),
            ast::BasicType::S8 => "s8".into(),
            ast::BasicType::S16 => "s16".into(),
            ast::BasicType::S32 => "s32".into(),
            ast::BasicType::S64 => "s64".into(),
            ast::BasicType::Ssize => "ssize".into(),
            ast::BasicType::U8 => "u8".into(),
            ast::BasicType::U16 => "u16".into(),
            ast::BasicType::U32 => "u32".into(),
            ast::BasicType::U64 => "u64".into(),
            ast::BasicType::Usize => "usize".into(),
            ast::BasicType::F32 => "f32".into(),
            ast::BasicType::F64 => "f64".into(),
            ast::BasicType::Char => "char".into(),
            ast::BasicType::Rawptr => "rawptr".into(),
        },
        hir::Type::Enum(id) => hb.name_str(hb.enum_data(id).name.id).into(),
        hir::Type::Union(id) => hb.name_str(hb.union_data(id).name.id).into(),
        hir::Type::Struct(id) => hb.name_str(hb.struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hb, *ref_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hb, slice.ty))
        }
        hir::Type::ArrayStatic(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
        hir::Type::ArrayStaticDecl(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
    }
}

//@better idea would be to return type repr that is not allocated via arena
// and will be fast to construct and compare
fn typecheck_expr<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    expr: &ast::Expr<'ast>,
) -> hir::Type<'hir> {
    match expr.kind {
        ast::ExprKind::Unit => hir::Type::Basic(ast::BasicType::Unit),
        ast::ExprKind::LitNull => hir::Type::Basic(ast::BasicType::Rawptr),
        ast::ExprKind::LitBool { val } => hir::Type::Basic(ast::BasicType::Bool),
        ast::ExprKind::LitInt { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::S32)),
        ast::ExprKind::LitFloat { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::F64)),
        ast::ExprKind::LitChar { val } => hir::Type::Basic(ast::BasicType::Char),
        ast::ExprKind::LitString { id } => {
            let slice = hb.arena().alloc(hir::ArraySlice {
                mutt: ast::Mut::Immutable,
                ty: hir::Type::Basic(ast::BasicType::U8),
            });
            hir::Type::ArraySlice(slice)
        }
        ast::ExprKind::If { if_ } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Block { stmts } => {
            //for stmt in stmts {
            //    match stmt.kind {
            //        ast::StmtKind::Break => todo!(),
            //        ast::StmtKind::Continue => todo!(),
            //        ast::StmtKind::Return(_) => todo!(),
            //        ast::StmtKind::Defer(_) => todo!(),
            //        ast::StmtKind::ForLoop(_) => todo!(),
            //        ast::StmtKind::VarDecl(_) => todo!(),
            //        ast::StmtKind::VarAssign(_) => todo!(),
            //        ast::StmtKind::ExprSemi(_) => todo!(),
            //        ast::StmtKind::ExprTail(_) => todo!(),
            //    }
            //}
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Match { match_ } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Field { target, name } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Index { target, index } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Cast { target, ty } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Sizeof { ty } => hir::Type::Basic(ast::BasicType::Usize),
        ast::ExprKind::Item { path } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::ProcCall { proc_call } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::StructInit { struct_init } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::ArrayInit { input } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::ArrayRepeat { expr, size } => {
            //@todo
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::UnaryExpr { op, rhs } => {
            let rhs = typecheck_expr(hb, rhs);
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::BinaryExpr { op, lhs, rhs } => {
            let lhs = typecheck_expr(hb, lhs);
            let rhs = typecheck_expr(hb, rhs);
            hir::Type::Basic(ast::BasicType::Unit)
        }
    }
}

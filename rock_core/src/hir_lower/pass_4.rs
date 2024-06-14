use super::hir_build::{HirData, HirEmit};
use super::pass_5::{self, TypeExpectation};
use super::{pass_3, proc_scope};
use crate::ast;
use crate::error::{ErrorComp, Info, StringOrStr};
use crate::intern::InternID;
use crate::text::TextRange;
use crate::{hir, id_impl};

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    EnumVariant(hir::EnumID, hir::EnumVariantID),
    StructSize(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    ArrayLen(hir::ConstEvalID),
}

pub fn resolve_const_dependencies<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().enum_ids() {
        let data = hir.registry().enum_data(id);

        for (idx, variant) in data.variants.iter().enumerate() {
            let (eval, origin_id) = *hir.registry().const_eval(variant.value);
            let variant_id = hir::EnumVariantID::new(idx);

            match eval {
                hir::ConstEval::Unresolved(expr) => {
                    let (mut tree, root_id) =
                        Tree::new_rooted(ConstDependency::EnumVariant(id, variant_id));

                    if let Err(from_id) = add_expr_const_dependencies(
                        hir, emit, &mut tree, root_id, origin_id, expr.0,
                    ) {
                        const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                    } else {
                        resolve_const_dependency_tree(hir, emit, &tree);
                    }
                }
                hir::ConstEval::ResolvedError => {}
                hir::ConstEval::ResolvedValue(_) => {}
            }
        }
    }

    for id in hir.registry().struct_ids() {
        let data = hir.registry().struct_data(id);

        if matches!(data.size_eval, hir::SizeEval::Unresolved) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::StructSize(id));
            let mut is_ok = true;

            for field in data.fields {
                if let Err(from_id) =
                    add_type_size_const_dependencies(hir, emit, &mut tree, root_id, field.ty)
                {
                    const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                    is_ok = false;
                    break;
                }
            }
            if is_ok {
                resolve_const_dependency_tree(hir, emit, &tree);
            }
        }
    }

    for id in hir.registry().const_ids() {
        let data = hir.registry().const_data(id);
        let (eval, origin_id) = *hir.registry().const_eval(data.value);

        match eval {
            hir::ConstEval::Unresolved(expr) => {
                let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Const(id));

                if let Err(from_id) =
                    add_type_usage_const_dependencies(hir, emit, &mut tree, root_id, data.ty)
                {
                    const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                } else if let Err(from_id) =
                    add_expr_const_dependencies(hir, emit, &mut tree, root_id, origin_id, expr.0)
                {
                    const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(hir, emit, &tree);
                }
            }
            hir::ConstEval::ResolvedError => {}
            hir::ConstEval::ResolvedValue(_) => {}
        }
    }

    for id in hir.registry().global_ids() {
        let data = hir.registry().global_data(id);
        let (eval, origin_id) = *hir.registry().const_eval(data.value);

        match eval {
            hir::ConstEval::Unresolved(expr) => {
                let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Global(id));

                if let Err(from_id) =
                    add_type_usage_const_dependencies(hir, emit, &mut tree, root_id, data.ty)
                {
                    const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                } else if let Err(from_id) =
                    add_expr_const_dependencies(hir, emit, &mut tree, root_id, origin_id, expr.0)
                {
                    const_dependencies_mark_error_up_to_root(hir, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(hir, emit, &tree);
                }
            }
            hir::ConstEval::ResolvedError => {}
            hir::ConstEval::ResolvedValue(_) => {}
        }
    }

    //@assuming that remaining constevals are array len 15.05.24
    // that didnt cycle with anything, thus can be resolved in `immediate mode`
    // this is only true when previos const dependencies and const evals were handled correctly
    //@how will unresolved expressions due to something else erroring earlier behave?
    // for example struct field with some unresolved array len
    for eval_id in hir.registry().const_eval_ids() {
        let (eval, _) = hir.registry().const_eval(eval_id);

        if matches!(eval, hir::ConstEval::Unresolved(_)) {
            resolve_and_update_const_eval(hir, emit, eval_id, TypeExpectation::USIZE);
        }
    }
}

struct Tree<T: PartialEq + Copy + Clone> {
    nodes: Vec<TreeNode<T>>,
}

id_impl!(TreeNodeID);
struct TreeNode<T: PartialEq + Copy + Clone> {
    value: T,
    parent: Option<TreeNodeID>,
}

impl<T: PartialEq + Copy + Clone> Tree<T> {
    #[must_use]
    fn new_rooted(root: T) -> (Tree<T>, TreeNodeID) {
        let root_id = TreeNodeID(0);
        let tree = Tree {
            nodes: vec![TreeNode {
                value: root,
                parent: None,
            }],
        };
        (tree, root_id)
    }

    #[must_use]
    fn add_child(&mut self, parent_id: TreeNodeID, value: T) -> TreeNodeID {
        let id = TreeNodeID::new(self.nodes.len());
        self.nodes.push(TreeNode {
            value,
            parent: Some(parent_id),
        });
        id
    }

    #[must_use]
    fn find_cycle(&self, id: TreeNodeID) -> Option<TreeNodeID> {
        let mut node = self.get_node(id);
        let value = node.value;

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            if node.value == value {
                return Some(parent_id);
            }
        }
        None
    }

    #[must_use]
    fn get_values_up_to_node(&self, from_id: TreeNodeID, up_to: TreeNodeID) -> Vec<T> {
        let mut node = self.get_node(from_id);
        let mut values = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            values.push(node.value);
            if parent_id == up_to {
                return values;
            }
        }
        values
    }

    #[must_use]
    fn get_values_up_to_root(&self, from_id: TreeNodeID) -> Vec<T> {
        let mut node = self.get_node(from_id);
        let mut values = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            values.push(node.value);
        }
        values
    }

    #[must_use]
    fn get_node(&self, id: TreeNodeID) -> &TreeNode<T> {
        &self.nodes[id.index()]
    }
}

fn check_const_dependency_cycle(
    hir: &HirData,
    emit: &mut HirEmit,
    tree: &Tree<ConstDependency>,
    parent_id: TreeNodeID,
    node_id: TreeNodeID,
) -> Result<(), TreeNodeID> {
    let cycle_id = match tree.find_cycle(node_id) {
        Some(cycle_id) => cycle_id,
        None => return Ok(()),
    };

    let src = match tree.get_node(cycle_id).value {
        ConstDependency::EnumVariant(id, variant_id) => {
            let data = hir.registry().enum_data(id);
            let variant = data.variant(variant_id);
            hir.src(data.origin_id, variant.name.range)
        }
        ConstDependency::StructSize(id) => {
            let data = hir.registry().struct_data(id);
            hir.src(data.origin_id, data.name.range)
        }
        ConstDependency::Const(id) => {
            let data = hir.registry().const_data(id);
            hir.src(data.origin_id, data.name.range)
        }
        ConstDependency::Global(id) => {
            let data = hir.registry().global_data(id);
            hir.src(data.origin_id, data.name.range)
        }
        ConstDependency::ArrayLen(eval_id) => {
            let (eval, origin_id) = *hir.registry().const_eval(eval_id);
            if let hir::ConstEval::Unresolved(expr) = eval {
                hir.src(origin_id, expr.0.range)
            } else {
                //@access to range information is behind consteval the state
                // always store SourceRange instead? 06.06.24
                panic!("array len consteval range not available");
            }
        }
    };

    let cycle_deps = tree.get_values_up_to_node(node_id, cycle_id);
    let mut ctx_msg: StringOrStr = "".into();
    let mut info_vec = Vec::with_capacity(cycle_deps.len());
    let mut info_src = src;

    for (idx, const_dep) in cycle_deps.iter().cloned().rev().skip(1).enumerate() {
        let first = idx == 0;
        let last = idx + 2 == cycle_deps.len();

        let prefix = if first { "" } else { "whitch " };
        let postfix = if last {
            ", completing the cycle..."
        } else {
            ""
        };

        let (msg, src) = match const_dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = hir.registry().enum_data(id);
                let variant = data.variant(variant_id);
                let msg = format!(
                    "{prefix}depends on `{}.{}` enum variant{postfix}",
                    hir.name_str(data.name.id),
                    hir.name_str(variant.name.id)
                );
                let src = hir.src(data.origin_id, variant.name.range);
                (msg, src)
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry().struct_data(id);
                let msg = format!(
                    "{prefix}depends on size of `{}`{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = hir.src(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` const value{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = hir.src(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` global value{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = hir.src(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, origin_id) = *hir.registry().const_eval(eval_id);
                if let hir::ConstEval::Unresolved(expr) = eval {
                    let msg = format!("{prefix}depends on array length{postfix}");
                    let src = hir.src(origin_id, expr.0.range);
                    (msg, src)
                } else {
                    //@access to range information is behind consteval the state
                    // always store SourceRange instead? 06.06.24
                    panic!("array len consteval range not available");
                }
            }
        };

        if first {
            ctx_msg = msg.into();
        } else {
            info_vec.push(Info::new_value(msg, info_src));
        }

        info_src = src;
    }

    emit.error(ErrorComp::new_detailed_info_vec(
        "constant dependency cycle found:",
        ctx_msg,
        src,
        info_vec,
    ));
    Err(parent_id)
}

fn const_dependencies_mark_error_up_to_root(
    hir: &mut HirData,
    tree: &Tree<ConstDependency>,
    from_id: TreeNodeID,
) {
    let const_deps = tree.get_values_up_to_root(from_id);
    for dep in const_deps {
        match dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = hir.registry().enum_data(id);
                let eval_id = data.variant(variant_id).value;
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry_mut().struct_data_mut(id);
                data.size_eval = hir::SizeEval::ResolvedError;
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let eval_id = data.value;
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let eval_id = data.value;
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
        }
    }
}

fn add_enum_variant_const_dependency<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    variant_id: hir::EnumVariantID,
) -> Result<(), TreeNodeID> {
    let data = hir.registry().enum_data(enum_id);
    let eval_id = data.variant(variant_id).value;
    let (eval, origin_id) = *hir.registry().const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id =
                tree.add_child(parent_id, ConstDependency::EnumVariant(enum_id, variant_id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;

            add_expr_const_dependencies(hir, emit, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::ResolvedValue(_) => Ok(()),
    }
}

fn add_struct_size_const_dependency<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
) -> Result<(), TreeNodeID> {
    let data = hir.registry().struct_data(struct_id);

    match data.size_eval {
        hir::SizeEval::Unresolved => {
            let node_id = tree.add_child(parent_id, ConstDependency::StructSize(struct_id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;

            for field in data.fields {
                add_type_size_const_dependencies(hir, emit, tree, node_id, field.ty)?;
            }
            Ok(())
        }
        hir::SizeEval::ResolvedError => Err(parent_id),
        hir::SizeEval::Resolved(_) => Ok(()),
    }
}

fn add_const_var_const_dependency<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    const_id: hir::ConstID,
) -> Result<(), TreeNodeID> {
    let data = hir.registry().const_data(const_id);
    let const_ty = data.ty;
    let eval_id = data.value;
    let (eval, origin_id) = *hir.registry().const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id = tree.add_child(parent_id, ConstDependency::Const(const_id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;

            // @will order of eval be correct? 13.06.24
            add_type_usage_const_dependencies(hir, emit, tree, parent_id, const_ty)?;
            add_expr_const_dependencies(hir, emit, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::ResolvedValue(_) => Ok(()),
    }
}

fn add_array_len_const_dependency<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    eval_id: hir::ConstEvalID,
) -> Result<(), TreeNodeID> {
    let (eval, origin_id) = *hir.registry().const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id = tree.add_child(parent_id, ConstDependency::ArrayLen(eval_id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;

            add_expr_const_dependencies(hir, emit, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::ResolvedValue(_) => Ok(()),
    }
}

fn add_type_size_const_dependencies<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    ty: hir::Type,
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(_) => {}
        hir::Type::Struct(id) => {
            add_struct_size_const_dependency(hir, emit, tree, parent_id, id)?;
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::Procedure(_) => {}
        hir::Type::ArraySlice(_) => {}
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_const_dependency(hir, emit, tree, parent_id, eval_id)?;
            }
            add_type_size_const_dependencies(hir, emit, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn add_type_usage_const_dependencies<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    ty: hir::Type,
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(_) => {}
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            for field in data.fields {
                add_type_usage_const_dependencies(hir, emit, tree, parent_id, field.ty)?
            }
        }
        hir::Type::Reference(ref_ty, _) => {
            add_type_usage_const_dependencies(hir, emit, tree, parent_id, *ref_ty)?
        }
        hir::Type::Procedure(proc_ty) => {
            for param in proc_ty.params {
                add_type_usage_const_dependencies(hir, emit, tree, parent_id, *param)?
            }
            add_type_usage_const_dependencies(hir, emit, tree, parent_id, proc_ty.return_ty)?
        }
        hir::Type::ArraySlice(slice) => {
            add_type_usage_const_dependencies(hir, emit, tree, parent_id, slice.elem_ty)?;
        }
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_const_dependency(hir, emit, tree, parent_id, eval_id)?;
            }
            add_type_usage_const_dependencies(hir, emit, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn add_expr_const_dependencies<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    origin_id: hir::ModuleID,
    expr: &'ast ast::Expr<'ast>,
) -> Result<(), TreeNodeID> {
    match expr.kind {
        ast::ExprKind::LitNull => Ok(()),
        ast::ExprKind::LitBool { .. } => Ok(()),
        ast::ExprKind::LitInt { .. } => Ok(()),
        ast::ExprKind::LitFloat { .. } => Ok(()),
        ast::ExprKind::LitChar { .. } => Ok(()),
        ast::ExprKind::LitString { .. } => Ok(()),
        ast::ExprKind::If { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "if");
            Err(parent_id)
        }
        ast::ExprKind::Block { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "block");
            Err(parent_id)
        }
        ast::ExprKind::Match { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "match");
            Err(parent_id)
        }
        ast::ExprKind::Field { target, .. } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Index { target, index } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, target)?;
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, index)?;
            Ok(())
        }
        ast::ExprKind::Slice { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "slice");
            Err(parent_id)
        }
        ast::ExprKind::Call { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "procedure call");
            Err(parent_id)
        }
        ast::ExprKind::Cast { target, .. } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Sizeof { ty } => {
            let ty = pass_3::type_resolve_delayed(hir, emit, origin_id, *ty);
            add_type_size_const_dependencies(hir, emit, tree, parent_id, ty)?;
            Ok(())
        }
        ast::ExprKind::Variant { .. } => {
            //@no type inference on this `ast name resolve` pass thus cannot infer variant type 14.06.24
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "variant selector");
            Err(parent_id)
        }
        ast::ExprKind::Item { path } => {
            let (value_id, _) = pass_5::path_resolve_value(hir, emit, None, origin_id, path);
            match value_id {
                pass_5::ValueID::None => Err(parent_id),
                pass_5::ValueID::Proc(proc_id) => {
                    //@borrowing hacks, just get data once here
                    // change the result Err type with delayed mutation of HirData only at top lvl?
                    for param in hir.registry().proc_data(proc_id).params {
                        add_type_usage_const_dependencies(hir, emit, tree, parent_id, param.ty)?;
                    }
                    let data = hir.registry().proc_data(proc_id);
                    add_type_usage_const_dependencies(hir, emit, tree, parent_id, data.return_ty)?;
                    Ok(())
                }
                pass_5::ValueID::Enum(enum_id, variant_id) => {
                    add_enum_variant_const_dependency(
                        hir, emit, tree, parent_id, enum_id, variant_id,
                    )?;
                    Ok(())
                }
                pass_5::ValueID::Const(const_id) => {
                    add_const_var_const_dependency(hir, emit, tree, parent_id, const_id)?;
                    Ok(())
                }
                pass_5::ValueID::Global(_) => {
                    error_cannot_refer_to_in_constants(hir, emit, origin_id, expr.range, "globals");
                    Err(parent_id)
                }
                pass_5::ValueID::Local(_) => {
                    error_cannot_refer_to_in_constants(hir, emit, origin_id, expr.range, "locals");
                    Err(parent_id)
                }
                pass_5::ValueID::Param(_) => {
                    error_cannot_refer_to_in_constants(
                        hir,
                        emit,
                        origin_id,
                        expr.range,
                        "parameters",
                    );
                    Err(parent_id)
                }
            }
        }
        ast::ExprKind::StructInit { struct_init } => {
            if let Some(struct_id) =
                pass_5::path_resolve_struct(hir, emit, None, origin_id, struct_init.path)
            {
                let ty = hir::Type::Struct(struct_id);
                add_type_usage_const_dependencies(hir, emit, tree, parent_id, ty)?;
                for init in struct_init.input {
                    add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, init.expr)?;
                }
                Ok(())
            } else {
                Err(parent_id)
            }
        }
        ast::ExprKind::ArrayInit { input } => {
            for &expr in input {
                add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayRepeat { expr, len } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, expr)?;
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, len.0)?;
            Ok(())
        }
        ast::ExprKind::Address { .. } => {
            error_cannot_use_in_constants(hir, emit, origin_id, expr.range, "address");
            Err(parent_id)
        }
        ast::ExprKind::Unary { rhs, .. } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
        ast::ExprKind::Binary { bin, .. } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, bin.lhs)?;
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, bin.rhs)?;
            Ok(())
        }
    }
}

fn error_cannot_use_in_constants(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(ErrorComp::new(
        format!("cannot use `{name}` expression in constants"),
        hir.src(origin_id, range),
        None,
    ));
}

fn error_cannot_refer_to_in_constants(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(ErrorComp::new(
        format!("cannot refer to `{name}` in constants"),
        hir.src(origin_id, range),
        None,
    ));
}

fn resolve_const_dependency_tree<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &Tree<ConstDependency>,
) {
    // reverse iteration allows to resolve dependencies in correct order
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = hir.registry().enum_data(id);
                let variant = data.variant(variant_id);
                let expect = TypeExpectation::new(hir::Type::Basic(data.basic), None); //@add range for basic type on enum
                resolve_and_update_const_eval(hir, emit, variant.value, expect);
            }
            ConstDependency::StructSize(id) => {
                let size_eval = resolve_struct_size(hir, emit, id);
                hir.registry_mut().struct_data_mut(id).size_eval = size_eval;
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let item = hir.registry().const_item(id);
                let expect =
                    TypeExpectation::new(data.ty, Some(hir.src(data.origin_id, item.ty.range)));
                resolve_and_update_const_eval(hir, emit, data.value, expect);
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let item = hir.registry().global_item(id);
                let expect =
                    TypeExpectation::new(data.ty, Some(hir.src(data.origin_id, item.ty.range)));
                resolve_and_update_const_eval(hir, emit, data.value, expect);
            }
            ConstDependency::ArrayLen(eval_id) => {
                resolve_and_update_const_eval(hir, emit, eval_id, TypeExpectation::USIZE);
            }
        }
    }
}

fn resolve_and_update_const_eval<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    eval_id: hir::ConstEvalID,
    expect: TypeExpectation<'hir>,
) {
    let (eval, origin_id) = *hir.registry().const_eval(eval_id);

    let value = match eval {
        hir::ConstEval::Unresolved(expr) => resolve_const_expr(hir, emit, origin_id, expect, expr),
        _ => panic!("calling `resolve_const_expr` on already resolved expr"),
    };

    let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
    *eval = hir::ConstEval::ResolvedValue(emit.const_intern.intern(value));
}

#[must_use]
pub fn resolve_const_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    expect: TypeExpectation<'hir>,
    expr: ast::ConstExpr,
) -> hir::ConstValue<'hir> {
    let dummy_data = hir::ProcData {
        origin_id,
        vis: ast::Vis::Private,
        name: ast::Name {
            id: InternID::dummy(),
            range: TextRange::empty_at(0.into()),
        },
        params: &[],
        is_variadic: false,
        return_ty: hir::Type::VOID,
        block: None,
        locals: &[],
        is_test: false,
        is_main: false,
    };
    let mut proc = proc_scope::ProcScope::new(&dummy_data, TypeExpectation::NOTHING);

    let error_count = emit.error_count();
    let hir_expr = pass_5::typecheck_expr(hir, emit, &mut proc, expect, expr.0);
    if emit.did_error(error_count) {
        hir::ConstValue::Error
    } else {
        fold_const_expr(hir, emit, origin_id, hir_expr.expr)
    }
}

fn resolve_struct_size(
    hir: &HirData,
    emit: &mut HirEmit,
    struct_id: hir::StructID,
) -> hir::SizeEval {
    let data = hir.registry().struct_data(struct_id);
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for field in data.fields {
        let (field_size, field_align) = match pass_5::type_size(
            hir,
            emit,
            field.ty,
            hir.src(data.origin_id, field.name.range), //@review source range for this type_size error 10.05.24
        ) {
            Some(size) => (size.size(), size.align()),
            None => return hir::SizeEval::ResolvedError,
        };
        size = aligned_size(size, field_align);
        size = if let Some(new_size) = size.checked_add(field_size) {
            new_size
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "struct size overflow: `{}` + `{}` (when computing: total_size + field_size)",
                    size, field_size
                ),
                hir.src(data.origin_id, field.name.range), //@review source range for size overflow error 10.05.24
                None,
            ));
            return hir::SizeEval::ResolvedError;
        };
        align = align.max(field_align);
    }

    size = aligned_size(size, align);
    hir::SizeEval::Resolved(hir::Size::new(size, align))
}

//@remove asserts later on when compiler is stable? 02.05.24
fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

//@check int, float value range constraints 14.05.24
// same for typecheck_int_lit etc, regular expressions checking
// should later be merged with this constant resolution / folding flow

//@ban globals being mentioned in constant expressions (immutable / mut doesnt matter)
// also ajust const dependencies since globals wont be a dependency anymore they are not allowed
//@more refined message for each incompatible expression type

//@assume invalid expressions to be already checked for?
// if `force` return ConstValue::Error
// else panic!("unexpected `name` in constant");
pub fn fold_const_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    expr: &'hir hir::Expr<'hir>,
) -> hir::ConstValue<'hir> {
    let result = match *expr {
        hir::Expr::Error => Ok(hir::ConstValue::Error),
        hir::Expr::Const { value } => Ok(value),
        hir::Expr::If { .. } => Err("if"),
        hir::Expr::Block { .. } => Err("block"),
        hir::Expr::Match { .. } => Err("match"),
        hir::Expr::StructField {
            target,
            field_id,
            deref,
            ..
        } => Ok(fold_struct_field(
            hir, emit, origin_id, target, field_id, deref,
        )),
        hir::Expr::SliceField {
            target,
            first_ptr,
            deref,
        } => Ok(fold_slice_field(
            hir, emit, origin_id, target, first_ptr, deref,
        )),
        hir::Expr::Index { target, access } => Ok(fold_index(hir, emit, origin_id, target, access)),
        hir::Expr::Slice { .. } => Err("slice"),
        hir::Expr::Cast { target, into, kind } => Err("cast"), //@todo cast 10.06.24
        hir::Expr::LocalVar { .. } => Err("local var"),
        hir::Expr::ParamVar { .. } => Err("param var"),
        hir::Expr::ConstVar { const_id } => Ok(fold_const_var(hir, emit, const_id)),
        hir::Expr::GlobalVar { .. } => Err("global vall"), //@custom message
        hir::Expr::CallDirect { .. } => Err("call direct"),
        hir::Expr::CallIndirect { .. } => Err("call indirect"),
        hir::Expr::StructInit { struct_id, input } => {
            Ok(fold_struct_init(hir, emit, origin_id, struct_id, input))
        }
        hir::Expr::ArrayInit { array_init } => {
            Ok(fold_array_init(hir, emit, origin_id, array_init))
        }
        hir::Expr::ArrayRepeat { array_repeat } => {
            Ok(fold_array_repeat(hir, emit, origin_id, array_repeat))
        }
        hir::Expr::Address { .. } => Err("address"),
        hir::Expr::Unary { op, rhs } => Ok(fold_unary_expr(hir, emit, origin_id, op, rhs)),
        hir::Expr::Binary {
            op,
            lhs,
            rhs,
            lhs_signed_int,
        } => Err("binary"), //@todo binary 10.06.24
    };

    match result {
        Ok(value) => value,
        Err(expr_name) => {
            //@range not available
            //emit.error(ErrorComp::new(
            //    format!("cannot use `{expr_name}` expression in constants"),
            //    hir.src(origin_id, expr.0.range),
            //    None,
            //));
            emit.error(ErrorComp::message(format!(
                "cannot use `{expr_name}` expression in constants"
            )));
            hir::ConstValue::Error
        }
    }
}

fn fold_struct_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    target: &'hir hir::Expr<'hir>,
    field_id: hir::StructFieldID,
    deref: bool,
) -> hir::ConstValue<'hir> {
    if deref {
        //@expr range required
        emit.error(ErrorComp::message(
            "cannot perform implicit dereference in constant expression",
        ));
        return hir::ConstValue::Error;
    }

    let target = fold_const_expr(hir, emit, origin_id, target);
    match target {
        hir::ConstValue::Struct { struct_ } => {
            let value_id = struct_.fields[field_id.index()];
            emit.const_intern.get(value_id)
        }
        _ => hir::ConstValue::Error,
    }
}

fn fold_slice_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    target: &'hir hir::Expr<'hir>,
    first_ptr: bool,
    deref: bool,
) -> hir::ConstValue<'hir> {
    if deref {
        //@expr range required
        emit.error(ErrorComp::message(
            "cannot perform implicit dereference in constant expression",
        ));
        return hir::ConstValue::Error;
    }

    let target = fold_const_expr(hir, emit, origin_id, target);
    match target {
        hir::ConstValue::String { id, c_string } => {
            if !first_ptr && !c_string {
                let string = hir.intern_string().get_str(id);
                let len = string.len();
                hir::ConstValue::Int {
                    val: len as u64,
                    neg: false,
                    ty: ast::BasicType::Usize,
                }
            } else {
                hir::ConstValue::Error
            }
        }
        _ => hir::ConstValue::Error,
    }
}

fn fold_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    target: &'hir hir::Expr<'hir>,
    access: &'hir hir::IndexAccess<'hir>,
) -> hir::ConstValue<'hir> {
    let target_value = fold_const_expr(hir, emit, origin_id, target);
    let index_value = fold_const_expr(hir, emit, origin_id, access.index);

    let index = match index_value {
        hir::ConstValue::Int { val, neg, ty } => {
            if !neg {
                Some(val)
            } else {
                None
            }
        }
        _ => None,
    };
    if let Some(index) = index {
        //@bounds check, same with normal array
        match target_value {
            hir::ConstValue::Array { array } => {
                if index >= array.len {
                    //@no source range available
                    hir::ConstValue::Error
                } else {
                    emit.const_intern.get(array.values[index as usize])
                }
            }
            hir::ConstValue::ArrayRepeat { len, value } => {
                if index >= len {
                    //@no source range available
                    hir::ConstValue::Error
                } else {
                    emit.const_intern.get(value)
                }
            }
            _ => hir::ConstValue::Error,
        }
    } else {
        hir::ConstValue::Error
    }
}

fn fold_const_var<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    const_id: hir::ConstID,
) -> hir::ConstValue<'hir> {
    let data = hir.registry().const_data(const_id);
    let (eval, _) = hir.registry().const_eval(data.value);
    match *eval {
        hir::ConstEval::ResolvedValue(value_id) => emit.const_intern.get(value_id),
        _ => panic!("unresolved constant"),
    }
}

fn fold_struct_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    struct_id: hir::StructID,
    input: &'hir [hir::StructFieldInit<'hir>],
) -> hir::ConstValue<'hir> {
    let mut value_ids = Vec::new();
    let error_id = emit.const_intern.intern(hir::ConstValue::Error);
    value_ids.resize(input.len(), error_id);

    for init in input {
        let value = fold_const_expr(hir, emit, origin_id, init.expr);
        value_ids[init.field_id.index()] = emit.const_intern.intern(value);
    }

    let fields = emit.const_intern.arena().alloc_slice(&value_ids);
    let const_struct = hir::ConstStruct { struct_id, fields };
    let struct_ = emit.const_intern.arena().alloc(const_struct);
    hir::ConstValue::Struct { struct_ }
}

fn fold_array_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    array_init: &'hir hir::ArrayInit<'hir>,
) -> hir::ConstValue<'hir> {
    let mut value_ids = Vec::with_capacity(array_init.input.len());

    for &init in array_init.input {
        let value = fold_const_expr(hir, emit, origin_id, init);
        value_ids.push(emit.const_intern.intern(value));
    }

    let values = emit.const_intern.arena().alloc_slice(value_ids.as_slice());
    let const_array = hir::ConstArray {
        len: values.len() as u64,
        values,
    };
    let array = emit.const_intern.arena().alloc(const_array);
    hir::ConstValue::Array { array }
}

fn fold_array_repeat<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    array_repeat: &'hir hir::ArrayRepeat<'hir>,
) -> hir::ConstValue<'hir> {
    let value = fold_const_expr(hir, emit, origin_id, array_repeat.expr);

    hir::ConstValue::ArrayRepeat {
        value: emit.const_intern.intern(value),
        len: array_repeat.len,
    }
}

fn fold_unary_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    op: ast::UnOp,
    rhs: &'hir hir::Expr<'hir>,
) -> hir::ConstValue<'hir> {
    let rhs_value = fold_const_expr(hir, emit, origin_id, rhs);
    match op {
        ast::UnOp::Neg => match rhs_value {
            hir::ConstValue::Int { val, neg, ty } => hir::ConstValue::Int { val, neg: !neg, ty },
            hir::ConstValue::Float { val, ty } => hir::ConstValue::Float { val: -val, ty },
            _ => hir::ConstValue::Error,
        },
        ast::UnOp::BitNot => hir::ConstValue::Error,
        ast::UnOp::LogicNot => match rhs_value {
            hir::ConstValue::Bool { val } => hir::ConstValue::Bool { val: !val },
            _ => hir::ConstValue::Error,
        },
        ast::UnOp::Deref => hir::ConstValue::Error, //@disallow with error?
    }
}

impl<'hir> hir::ConstValue<'hir> {
    fn test() {
        //const shift_test: i32 = i32::MAX + i32::MAX;

        let lhs: i128 = 10;
        let rhs: i128 = -10;

        let add = lhs.checked_add(rhs);
        let sub = lhs.checked_sub(rhs);
        let mul = lhs.checked_mul(rhs);
        let div = lhs.checked_div(rhs); //@check / 0 separately
        let rem = lhs.checked_rem(rhs); //@check / 0 separately

        let shl = lhs.checked_shl(rhs as u32); //@check for positive
        let shr = lhs.checked_shr(rhs as u32); //@check for positive
        let is_eq = lhs == rhs;
        let not_eq = lhs != rhs;
        let less = lhs < rhs;
        let less_eq = lhs <= rhs;
        let greater = lhs > rhs;
        let greater_eq = lhs >= rhs;
    }

    fn add(self, other: hir::ConstValue<'hir>) -> hir::ConstValue<'hir> {
        match (self, other) {
            (hir::ConstValue::IntS(lhs), hir::ConstValue::IntS(rhs)) => {
                Self::from_i64_opt(lhs.checked_add(rhs))
            }
            (hir::ConstValue::IntS(lhs), hir::ConstValue::IntU(rhs)) => {
                Self::from_i64_opt(lhs.checked_add_unsigned(rhs))
            }
            (hir::ConstValue::IntU(lhs), hir::ConstValue::IntU(rhs)) => {
                Self::from_u64_opt(lhs.checked_add(rhs))
            }
            (hir::ConstValue::IntU(lhs), hir::ConstValue::IntS(rhs)) => {
                Self::from_u64_opt(lhs.checked_add_signed(rhs))
            }
            _ => hir::ConstValue::Error,
        }
    }

    fn sub(self, other: hir::ConstValue<'hir>) -> hir::ConstValue<'hir> {
        match (self, other) {
            (hir::ConstValue::IntS(lhs), hir::ConstValue::IntS(rhs)) => {
                Self::from_i64_opt(lhs.checked_sub(rhs))
            }
            (hir::ConstValue::IntS(lhs), hir::ConstValue::IntU(rhs)) => {
                Self::from_i64_opt(lhs.checked_sub_unsigned(rhs))
            }
            (hir::ConstValue::IntU(lhs), hir::ConstValue::IntU(rhs)) => {
                Self::from_u64_opt(lhs.checked_sub(rhs))
            }
            (hir::ConstValue::IntU(lhs), hir::ConstValue::IntS(rhs)) => {
                if let Some(rhs) = Self::i64_to_unsigned(rhs) {
                    Self::from_u64_opt(lhs.checked_sub(rhs))
                } else {
                    eprintln!("error in const sub u - s int operation");
                    hir::ConstValue::Error
                }
            }
            _ => hir::ConstValue::Error,
        }
    }

    fn i64_to_unsigned(val: i64) -> Option<u64> {
        if val >= 0 {
            Some(val as u64)
        } else {
            None
        }
    }

    fn from_i64_opt(val: Option<i64>) -> hir::ConstValue<'hir> {
        if let Some(val) = val {
            hir::ConstValue::IntS(val)
        } else {
            eprintln!("error in const signed int operation");
            hir::ConstValue::Error
        }
    }

    fn from_u64_opt(val: Option<u64>) -> hir::ConstValue<'hir> {
        if let Some(val) = val {
            hir::ConstValue::IntU(val)
        } else {
            eprintln!("error in const unsigned int operation");
            hir::ConstValue::Error
        }
    }
}

use super::hir_build::{HirData, HirEmit};
use super::pass_3;
use super::pass_5::{self, Expectation};
use super::proc_scope;
use crate::ast;
use crate::bitset::BitSet;
use crate::error::{ErrorComp, Info, SourceRange, StringOrStr};
use crate::hir;
use crate::id_impl;
use crate::intern::InternID;
use crate::session::ModuleID;
use crate::text::TextRange;

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    EnumVariant(hir::EnumID, hir::VariantID),
    StructSize(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    ArrayLen(hir::ConstEvalID),
}

pub fn resolve_const_dependencies<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().enum_ids() {
        let data = hir.registry().enum_data(id);

        for (idx, variant) in data.variants.iter().enumerate() {
            let variant_id = hir::VariantID::new(idx);

            match variant.kind {
                hir::VariantKind::Default(_) => {}
                hir::VariantKind::Constant(eval_id) => {
                    let (eval, origin_id) = *hir.registry().const_eval(eval_id);
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
                //@resolve automatically at the end of the process?
                // array lens etc
                hir::VariantKind::HasValues(_) => {}
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
            let expect = Expectation::HasType(hir::Type::USIZE, None);
            resolve_and_update_const_eval(hir, emit, eval_id, expect);
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
            SourceRange::new(data.origin_id, variant.name.range)
        }
        ConstDependency::StructSize(id) => {
            let data = hir.registry().struct_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::Const(id) => {
            let data = hir.registry().const_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::Global(id) => {
            let data = hir.registry().global_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::ArrayLen(eval_id) => {
            let (eval, origin_id) = *hir.registry().const_eval(eval_id);
            if let hir::ConstEval::Unresolved(expr) = eval {
                SourceRange::new(origin_id, expr.0.range)
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
                let src = SourceRange::new(data.origin_id, variant.name.range);
                (msg, src)
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry().struct_data(id);
                let msg = format!(
                    "{prefix}depends on size of `{}`{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` const value{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` global value{postfix}",
                    hir.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, origin_id) = *hir.registry().const_eval(eval_id);
                if let hir::ConstEval::Unresolved(expr) = eval {
                    let msg = format!("{prefix}depends on array length{postfix}");
                    let src = SourceRange::new(origin_id, expr.0.range);
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
                let variant = data.variant(variant_id);

                match variant.kind {
                    //@unreachable for default?
                    hir::VariantKind::Default(_) => todo!("mark as error for VariantKind::Default"),
                    hir::VariantKind::Constant(eval_id) => {
                        let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                        *eval = hir::ConstEval::ResolvedError;
                    }
                    hir::VariantKind::HasValues(_) => {
                        todo!("mark as error for VariantKind::HasValues")
                    }
                }
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

fn add_variant_const_dependency<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) -> Result<(), TreeNodeID> {
    let data = hir.registry().enum_data(enum_id);
    let eval_id = match data.variant(variant_id).kind {
        hir::VariantKind::Default(_) => todo!("enum variant dep VariantKind::Default"),
        hir::VariantKind::Constant(eval_id) => eval_id,
        hir::VariantKind::HasValues(_) => todo!("enum variant dep VariantKind::HasValues"),
    };
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
            for param_ty in proc_ty.param_types {
                add_type_usage_const_dependencies(hir, emit, tree, parent_id, *param_ty)?
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
    origin_id: ModuleID,
    expr: &'ast ast::Expr<'ast>,
) -> Result<(), TreeNodeID> {
    match expr.kind {
        ast::ExprKind::Lit(_) => Ok(()),
        ast::ExprKind::If { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "if");
            Err(parent_id)
        }
        ast::ExprKind::Block { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "block");
            Err(parent_id)
        }
        ast::ExprKind::Match { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "match");
            Err(parent_id)
        }
        ast::ExprKind::Match2 { .. } => todo!("match2 `add_expr_const_dependencies`"),
        ast::ExprKind::Field { target, .. } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        //@index or slicing
        ast::ExprKind::Index {
            target,
            mutt,
            index,
        } => {
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, target)?;
            add_expr_const_dependencies(hir, emit, tree, parent_id, origin_id, index)?;
            Ok(())
        }
        ast::ExprKind::Call { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "procedure call");
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
        //@input not used
        ast::ExprKind::Item { path, input } => {
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
                    add_variant_const_dependency(hir, emit, tree, parent_id, enum_id, variant_id)?;
                    Ok(())
                }
                pass_5::ValueID::Const(const_id) => {
                    add_const_var_const_dependency(hir, emit, tree, parent_id, const_id)?;
                    Ok(())
                }
                pass_5::ValueID::Global(_) => {
                    error_cannot_refer_to_in_constants(emit, origin_id, expr.range, "globals");
                    Err(parent_id)
                }
                pass_5::ValueID::Local(_) => {
                    error_cannot_refer_to_in_constants(emit, origin_id, expr.range, "locals");
                    Err(parent_id)
                }
                pass_5::ValueID::Param(_) => {
                    error_cannot_refer_to_in_constants(emit, origin_id, expr.range, "parameters");
                    Err(parent_id)
                }
            }
        }
        ast::ExprKind::Variant { .. } => {
            //@no type inference on this `ast name resolve` pass thus cannot infer variant type 14.06.24
            error_cannot_use_in_constants(emit, origin_id, expr.range, "variant selector");
            Err(parent_id)
        }
        ast::ExprKind::StructInit { struct_init } => match struct_init.path {
            //@cannot infer struct / enum variant type in constants
            Some(path) => {
                if let Some(struct_id) =
                    pass_5::path_resolve_struct(hir, emit, None, origin_id, path)
                {
                    let ty = hir::Type::Struct(struct_id);
                    add_type_usage_const_dependencies(hir, emit, tree, parent_id, ty)?;
                    for init in struct_init.input {
                        add_expr_const_dependencies(
                            hir, emit, tree, parent_id, origin_id, init.expr,
                        )?;
                    }
                    Ok(())
                } else {
                    Err(parent_id)
                }
            }
            None => {
                pass_5::error_cannot_infer_struct_type(
                    emit,
                    SourceRange::new(origin_id, expr.range),
                );
                Err(parent_id)
            }
        },
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
        ast::ExprKind::Deref { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "deref");
            Err(parent_id)
        }
        ast::ExprKind::Address { .. } => {
            error_cannot_use_in_constants(emit, origin_id, expr.range, "address");
            Err(parent_id)
        }
        ast::ExprKind::Range { range } => todo!("range feature"),
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
    emit: &mut HirEmit,
    origin_id: ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(ErrorComp::new(
        format!("cannot use `{name}` expression in constants"),
        SourceRange::new(origin_id, range),
        None,
    ));
}

fn error_cannot_refer_to_in_constants(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(ErrorComp::new(
        format!("cannot refer to `{name}` in constants"),
        SourceRange::new(origin_id, range),
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
                let item = hir.registry().enum_item(id);
                let variant = data.variant(variant_id);

                let expect_src = if let Some((_, range)) = item.basic {
                    SourceRange::new(data.origin_id, range)
                } else {
                    SourceRange::new(data.origin_id, data.name.range)
                };
                let expect = Expectation::HasType(
                    hir::Type::Basic(data.int_ty.into_basic()),
                    Some(expect_src),
                );

                match variant.kind {
                    hir::VariantKind::Default(_) => todo!("resolve tree VariantKind::Default"),
                    hir::VariantKind::Constant(eval_id) => {
                        resolve_and_update_const_eval(hir, emit, eval_id, expect);
                    }
                    hir::VariantKind::HasValues(_) => todo!("resolve tree VariantKind::HasValues"),
                }
            }
            ConstDependency::StructSize(id) => {
                let size_eval = resolve_struct_size(hir, emit, id);
                hir.registry_mut().struct_data_mut(id).size_eval = size_eval;
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let item = hir.registry().const_item(id);

                let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                let expect = Expectation::HasType(data.ty, Some(expect_src));
                resolve_and_update_const_eval(hir, emit, data.value, expect);
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let item = hir.registry().global_item(id);

                let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                let expect = Expectation::HasType(data.ty, Some(expect_src));
                resolve_and_update_const_eval(hir, emit, data.value, expect);
            }
            ConstDependency::ArrayLen(eval_id) => {
                let expect = Expectation::HasType(hir::Type::USIZE, None);
                resolve_and_update_const_eval(hir, emit, eval_id, expect);
            }
        }
    }
}

fn resolve_and_update_const_eval<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    eval_id: hir::ConstEvalID,
    expect: Expectation<'hir>,
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
    origin_id: ModuleID,
    expect: Expectation<'hir>,
    expr: ast::ConstExpr,
) -> hir::ConstValue<'hir> {
    let dummy_data = hir::ProcData {
        origin_id,
        attr_set: BitSet::EMPTY,
        vis: ast::Vis::Private,
        name: ast::Name {
            id: InternID::dummy(),
            range: TextRange::empty_at(0.into()),
        },
        params: &[],
        return_ty: hir::Type::VOID,
        block: None,
        locals: &[],
    };

    let mut proc = proc_scope::ProcScope::new(&dummy_data, Expectation::None);
    let error_count = emit.error_count();
    let hir_expr = pass_5::typecheck_expr(hir, emit, &mut proc, expect, expr.0);

    //@instead result option or result?
    if emit.did_error(error_count) {
        hir::ConstValue::Error
    } else if let Ok(value) = fold_const_expr(hir, emit, origin_id, hir_expr.expr) {
        value
    } else {
        hir::ConstValue::Error
    }
}

fn resolve_enum_size(hir: &HirData, emit: &mut HirEmit, enum_id: hir::EnumID) -> hir::SizeEval {
    let data = hir.registry().enum_data(enum_id);
    if data.variants.is_empty() {
        return hir::SizeEval::Resolved(hir::Size::new(0, 1));
    }
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    //@tag size + max variant size
    // with proper alignment

    //@temp
    hir::SizeEval::Unresolved
}

fn resolve_variant_size(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    variant: &hir::Variant,
) -> Option<hir::Size> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    match variant.kind {
        hir::VariantKind::Default(_) => {}
        hir::VariantKind::Constant(_) => {}
        hir::VariantKind::HasValues(types) => {
            for ty in types {
                let (ty_size, ty_align) = match pass_5::type_size(
                    hir,
                    emit,
                    *ty,
                    SourceRange::new(origin_id, variant.name.range),
                ) {
                    Some(size) => (size.size(), size.align()),
                    None => return None,
                };

                size = aligned_size(size, ty_align);
                size = if let Some(new_size) = size.checked_add(ty_size) {
                    new_size
                } else {
                    emit.error(ErrorComp::new(
                        format!(
                            "variant size overflow: `{}` + `{}` (when computing: total_size + value_size)",
                            size, ty_size
                        ),
                        SourceRange::new(origin_id, variant.name.range), //@review source range for size overflow error 10.05.24
                        None,
                    ));
                    return None;
                };
                align = align.max(ty_align);
            }
        }
    }

    Some(hir::Size::new(size, align))
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
            SourceRange::new(data.origin_id, field.name.range), //@review source range for this type_size error 10.05.24
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
                SourceRange::new(data.origin_id, field.name.range), //@review source range for size overflow error 10.05.24
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

fn fold_const_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    expr: &hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { value } => fold_const(emit, value),
        hir::Expr::If { .. } => unreachable!(),
        hir::Expr::Block { .. } => unreachable!(),
        hir::Expr::Match { .. } => unreachable!(),
        hir::Expr::Match2 { .. } => unreachable!(),
        hir::Expr::StructField {
            target,
            field_id,
            deref,
            ..
        } => fold_struct_field(hir, emit, origin_id, target, field_id, deref),
        hir::Expr::SliceField {
            target,
            field,
            deref,
        } => fold_slice_field(hir, emit, origin_id, target, field, deref),
        hir::Expr::Index { target, access } => fold_index(hir, emit, origin_id, target, access),
        hir::Expr::Slice { .. } => unreachable!(),
        hir::Expr::Cast { .. } => unimplemented!("fold cast"),
        hir::Expr::LocalVar { .. } => unreachable!(),
        hir::Expr::ParamVar { .. } => unreachable!(),
        hir::Expr::ConstVar { const_id } => fold_const_var(hir, emit, const_id),
        hir::Expr::GlobalVar { .. } => unreachable!(),
        hir::Expr::Variant { .. } => unimplemented!("fold enum variant"),
        hir::Expr::CallDirect { .. } => unreachable!(),
        hir::Expr::CallIndirect { .. } => unreachable!(),
        hir::Expr::StructInit { struct_id, input } => {
            fold_struct_init(hir, emit, origin_id, struct_id, input)
        }
        hir::Expr::ArrayInit { array_init } => fold_array_init(hir, emit, origin_id, array_init),
        hir::Expr::ArrayRepeat { array_repeat } => {
            fold_array_repeat(hir, emit, origin_id, array_repeat)
        }
        hir::Expr::Deref { .. } => unreachable!(),
        hir::Expr::Address { .. } => unreachable!(),
        hir::Expr::Unary { op, rhs } => fold_unary_expr(hir, emit, origin_id, op, rhs),
        hir::Expr::Binary { op, lhs, rhs } => fold_binary(hir, emit, origin_id, op, lhs, rhs),
    }
}

fn fold_const<'hir>(
    emit: &mut HirEmit,
    value: hir::ConstValue<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match value {
        hir::ConstValue::Error => unreachable!(),
        hir::ConstValue::Int { val, int_ty, .. } => int_range_check(emit, val.into(), int_ty),
        hir::ConstValue::Float { val, float_ty } => float_range_check(emit, val, float_ty),
        hir::ConstValue::Null
        | hir::ConstValue::Bool { .. }
        | hir::ConstValue::Char { .. }
        | hir::ConstValue::String { .. }
        | hir::ConstValue::Procedure { .. }
        | hir::ConstValue::Variant { .. }
        | hir::ConstValue::Struct { .. }
        | hir::ConstValue::Array { .. }
        | hir::ConstValue::ArrayRepeat { .. } => Ok(value),
    }
}

fn fold_struct_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    target: &hir::Expr<'hir>,
    field_id: hir::FieldID,
    deref: bool,
) -> Result<hir::ConstValue<'hir>, ()> {
    if deref {
        unreachable!()
    }
    let target = fold_const_expr(hir, emit, origin_id, target)?;

    match target {
        hir::ConstValue::Struct { struct_ } => {
            let value_id = struct_.fields[field_id.index()];
            Ok(emit.const_intern.get(value_id))
        }
        _ => unreachable!(),
    }
}

fn fold_slice_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    target: &hir::Expr<'hir>,
    field: hir::SliceField,
    deref: bool,
) -> Result<hir::ConstValue<'hir>, ()> {
    if deref {
        unreachable!();
    }
    let target = fold_const_expr(hir, emit, origin_id, target)?;

    match target {
        hir::ConstValue::String { id, c_string } => match field {
            hir::SliceField::Ptr => unreachable!(),
            hir::SliceField::Len => {
                if !c_string {
                    let string = hir.intern_string().get_str(id);
                    let len = string.len();
                    Ok(hir::ConstValue::Int {
                        val: len as u64,
                        neg: false,
                        int_ty: hir::BasicInt::Usize,
                    })
                } else {
                    unreachable!()
                }
            }
        },
        _ => unreachable!(),
    }
}

//@check out of bounds static array access even in non constant targets (during typecheck)
fn fold_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    target: &hir::Expr<'hir>,
    access: &hir::IndexAccess<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    if access.deref {
        unreachable!();
    }
    let target = fold_const_expr(hir, emit, origin_id, target)?;
    let index = fold_const_expr(hir, emit, origin_id, access.index)?;

    let index = match index {
        hir::ConstValue::Int { val, neg, int_ty } => {
            assert!(!neg);
            assert!(!int_ty.is_signed());
            val
        }
        _ => unreachable!(),
    };

    let array_len = match target {
        hir::ConstValue::Array { array } => array.len,
        hir::ConstValue::ArrayRepeat { len, .. } => len,
        _ => unreachable!(),
    };

    if index >= array_len {
        //@update error message
        emit.error(ErrorComp::message(format!(
            "index out of bounds\nvalue `{index}` >= `{array_len}` (array len)"
        )));
        Err(())
    } else {
        let value_id = match target {
            hir::ConstValue::Array { array } => array.values[index as usize],
            hir::ConstValue::ArrayRepeat { value, .. } => value,
            _ => unreachable!(),
        };
        Ok(emit.const_intern.get(value_id))
    }
}

fn fold_const_var<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    const_id: hir::ConstID,
) -> Result<hir::ConstValue<'hir>, ()> {
    let data = hir.registry().const_data(const_id);
    let (eval, _) = hir.registry().const_eval(data.value);

    match *eval {
        hir::ConstEval::Unresolved(..) => unreachable!("fold_const_var unresolved"),
        hir::ConstEval::ResolvedError => Err(()),
        hir::ConstEval::ResolvedValue(value_id) => Ok(emit.const_intern.get(value_id)),
    }
}

fn fold_struct_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    struct_id: hir::StructID,
    input: &[hir::FieldInit<'hir>],
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut value_ids = Vec::new();
    let error_id = emit.const_intern.intern(hir::ConstValue::Error);
    value_ids.resize(input.len(), error_id);

    for init in input {
        let value = fold_const_expr(hir, emit, origin_id, init.expr)?;
        value_ids[init.field_id.index()] = emit.const_intern.intern(value);
    }

    let fields = emit.const_intern.arena().alloc_slice(&value_ids);
    let const_struct = hir::ConstStruct { struct_id, fields };
    let struct_ = emit.const_intern.arena().alloc(const_struct);
    Ok(hir::ConstValue::Struct { struct_ })
}

fn fold_array_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    array_init: &hir::ArrayInit<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut value_ids = Vec::with_capacity(array_init.input.len());

    for &init in array_init.input {
        let value = fold_const_expr(hir, emit, origin_id, init)?;
        value_ids.push(emit.const_intern.intern(value));
    }

    let values = emit.const_intern.arena().alloc_slice(value_ids.as_slice());
    let const_array = hir::ConstArray {
        len: values.len() as u64,
        values,
    };
    let array = emit.const_intern.arena().alloc(const_array);
    Ok(hir::ConstValue::Array { array })
}

fn fold_array_repeat<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    array_repeat: &hir::ArrayRepeat<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let value = fold_const_expr(hir, emit, origin_id, array_repeat.expr)?;

    Ok(hir::ConstValue::ArrayRepeat {
        value: emit.const_intern.intern(value),
        len: array_repeat.len,
    })
}

fn fold_unary_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    op: hir::UnOp,
    rhs: &'hir hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let rhs = fold_const_expr(hir, emit, origin_id, rhs)?;

    match op {
        hir::UnOp::Neg_Int => {
            let int_ty = rhs.into_int_ty();
            let val = -rhs.into_int();
            int_range_check(emit, val, int_ty)
        }
        hir::UnOp::Neg_Float => {
            let float_ty = rhs.into_float_ty();
            let val = rhs.into_float();
            float_range_check(emit, val, float_ty)
        }
        hir::UnOp::BitNot => unimplemented!(),
        hir::UnOp::LogicNot => {
            let val = !rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
    }
}

fn fold_binary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    op: hir::BinOp,
    lhs: &hir::Expr<'hir>,
    rhs: &hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let lhs = fold_const_expr(hir, emit, origin_id, lhs)?;
    let rhs = fold_const_expr(hir, emit, origin_id, rhs)?;

    match op {
        hir::BinOp::Add_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() + rhs.into_int();
            int_range_check(emit, val, int_ty)
        }
        hir::BinOp::Add_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() + rhs.into_float();
            float_range_check(emit, val, float_ty)
        }
        hir::BinOp::Sub_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() - rhs.into_int();
            int_range_check(emit, val, int_ty)
        }
        hir::BinOp::Sub_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() - rhs.into_float();
            float_range_check(emit, val, float_ty)
        }
        hir::BinOp::Mul_Int => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_mul(rhs) {
                int_range_check(emit, val, int_ty)
            } else {
                error_binary_int_overflow(emit, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Mul_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() * rhs.into_float();
            float_range_check(emit, val, float_ty)
        }
        hir::BinOp::Div_IntS | hir::BinOp::Div_IntU => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if rhs == 0 {
                error_binary_int_div_zero(emit, op, lhs, rhs);
                Err(())
            } else if let Some(val) = lhs.checked_div(rhs) {
                int_range_check(emit, val, int_ty)
            } else {
                error_binary_int_overflow(emit, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Div_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() / rhs.into_float();
            float_range_check(emit, val, float_ty)
        }
        hir::BinOp::Rem_IntS | hir::BinOp::Rem_IntU => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if rhs == 0 {
                error_binary_int_div_zero(emit, op, lhs, rhs);
                Err(())
            } else if let Some(val) = lhs.checked_rem(rhs) {
                int_range_check(emit, val, int_ty)
            } else {
                error_binary_int_overflow(emit, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::BitAnd => unimplemented!(),
        hir::BinOp::BitOr => unimplemented!(),
        hir::BinOp::BitXor => unimplemented!(),
        hir::BinOp::BitShl => unimplemented!(),
        hir::BinOp::BitShr_IntS => unimplemented!(),
        hir::BinOp::BitShr_IntU => unimplemented!(),
        hir::BinOp::IsEq_Int => {
            let val = lhs.into_int() == rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::IsEq_Float => {
            let val = lhs.into_float() == rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::NotEq_Int => {
            let val = lhs.into_int() != rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::NotEq_Float => {
            let val = lhs.into_float() != rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Less_IntS | hir::BinOp::Less_IntU => {
            let val = lhs.into_int() < rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Less_Float => {
            let val = lhs.into_float() < rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LessEq_IntS | hir::BinOp::LessEq_IntU => {
            let val = lhs.into_int() <= rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LessEq_Float => {
            let val = lhs.into_float() <= rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Greater_IntS | hir::BinOp::Greater_IntU => {
            let val = lhs.into_int() > rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Greater_Float => {
            let val = lhs.into_float() > rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::GreaterEq_IntS | hir::BinOp::GreaterEq_IntU => {
            let val = lhs.into_int() >= rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::GreaterEq_Float => {
            let val = lhs.into_float() >= rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LogicAnd => {
            let val = lhs.into_bool() && rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LogicOr => {
            let val = lhs.into_bool() || rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
    }
}

fn error_binary_int_div_zero(emit: &mut HirEmit, op: hir::BinOp, lhs: i128, rhs: i128) {
    let op_str = op.as_str();
    let format = format!("integer division by zero\nwhen computing: `{lhs}` {op_str} `{rhs}`");
    emit.error(ErrorComp::message(format));
}

fn error_binary_int_overflow(emit: &mut HirEmit, op: hir::BinOp, lhs: i128, rhs: i128) {
    let op_str = op.as_str();
    let format = format!("integer constant overflow\nwhen computing: `{lhs}` {op_str} `{rhs}`");
    emit.error(ErrorComp::message(format));
}

fn int_range_check<'hir>(
    emit: &mut HirEmit,
    val: i128,
    int_ty: hir::BasicInt,
) -> Result<hir::ConstValue<'hir>, ()> {
    let (min, max) = match int_ty {
        hir::BasicInt::S8 => (i8::MIN as i128, i8::MAX as i128),
        hir::BasicInt::S16 => (i16::MIN as i128, i16::MAX as i128),
        hir::BasicInt::S32 => (i32::MIN as i128, i32::MAX as i128),
        hir::BasicInt::S64 => (i64::MIN as i128, i64::MAX as i128),
        hir::BasicInt::Ssize => todo!(), //@requires target pointer_width
        hir::BasicInt::U8 => (u8::MIN as i128, u8::MAX as i128),
        hir::BasicInt::U16 => (u16::MIN as i128, u16::MAX as i128),
        hir::BasicInt::U32 => (u32::MIN as i128, u32::MAX as i128),
        hir::BasicInt::U64 => (u64::MIN as i128, u64::MAX as i128),
        hir::BasicInt::Usize => todo!(), //@requires target pointer_width
    };

    if val < min || val > max {
        //@no source info available
        emit.error(ErrorComp::message(format!(
            "integer constant out of range for `{}`\nvalue `{val}` is outside `{min}..={max}` range",
            int_ty.into_basic().as_str()
        )));
        Err(())
    } else {
        if val > 0 {
            let val: u64 = val.try_into().unwrap();
            let neg = false;
            Ok(hir::ConstValue::Int { val, neg, int_ty })
        } else {
            let val: u64 = (-val).try_into().unwrap();
            let neg = true;
            Ok(hir::ConstValue::Int { val, neg, int_ty })
        }
    }
}

fn float_range_check<'hir>(
    emit: &mut HirEmit,
    val: f64,
    float_ty: hir::BasicFloat,
) -> Result<hir::ConstValue<'hir>, ()> {
    let (min, max) = match float_ty {
        hir::BasicFloat::F32 => (f32::MIN as f64, f32::MAX as f64),
        hir::BasicFloat::F64 => (f64::MIN as f64, f64::MAX as f64),
    };

    if val.is_nan() {
        //@no source info available
        emit.error(ErrorComp::message(format!("float constant is NaN")));
        Err(())
    } else if val.is_infinite() {
        //@no source info available
        emit.error(ErrorComp::message(format!("float constant is Infinite")));
        Err(())
    } else if val < min || val > max {
        //@no error source info available
        emit.error(ErrorComp::message(format!(
            "float constant out of range for `{}`\nvalue `{val}` is outside `{min}..={max}` range",
            float_ty.into_basic().as_str()
        )));
        Err(())
    } else {
        Ok(hir::ConstValue::Float { val, float_ty })
    }
}

impl<'hir> hir::ConstValue<'hir> {
    fn into_bool(&self) -> bool {
        match *self {
            hir::ConstValue::Bool { val } => val,
            _ => unreachable!(),
        }
    }
    fn into_int(&self) -> i128 {
        match *self {
            hir::ConstValue::Int { val, neg, .. } => {
                if neg {
                    -(val as i128)
                } else {
                    val as i128
                }
            }
            _ => unreachable!(),
        }
    }
    fn into_int_ty(&self) -> hir::BasicInt {
        match *self {
            hir::ConstValue::Int { int_ty, .. } => int_ty,
            _ => unreachable!(),
        }
    }
    fn into_float(&self) -> f64 {
        match *self {
            hir::ConstValue::Float { val, .. } => val,
            _ => unreachable!(),
        }
    }
    fn into_float_ty(&self) -> hir::BasicFloat {
        match *self {
            hir::ConstValue::Float { float_ty, .. } => float_ty,
            _ => unreachable!(),
        }
    }
}

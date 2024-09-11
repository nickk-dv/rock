use super::fold;
use super::layout;
use crate::ast;
use crate::error::{ErrorComp, ErrorSink, Info, SourceRange, StringOrStr};
use crate::hir;
use crate::hir_lower::context::{HirCtx, HirEmit};
use crate::hir_lower::{pass_3, pass_5, pass_5::Expectation};
use crate::session::ModuleID;
use crate::support::{IndexID, ID};
use crate::text::TextRange;

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency<'hir> {
    EnumVariant(hir::EnumID<'hir>, hir::VariantID<'hir>),
    EnumLayout(hir::EnumID<'hir>),
    StructLayout(hir::StructID<'hir>),
    Const(hir::ConstID<'hir>),
    Global(hir::GlobalID<'hir>),
    ArrayLen(hir::ConstEvalID),
}

pub fn resolve_const_dependencies<'hir>(ctx: &mut HirCtx) {
    for enum_id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(enum_id);

        for (idx, variant) in data.variants.iter().enumerate() {
            let variant_id = hir::VariantID::new_raw(idx);

            let unresolved = match variant.kind {
                hir::VariantKind::Default(eval_id) => {
                    let eval = *ctx.registry.variant_eval(eval_id);
                    eval.is_unresolved()
                }
                hir::VariantKind::Constant(eval_id) => {
                    let (eval, _) = *ctx.registry.const_eval(eval_id);
                    eval.is_unresolved()
                }
            };

            if unresolved {
                let dependency = ConstDependency::EnumVariant(enum_id, variant_id);
                let (mut tree, root_id) = Tree::new_rooted(dependency);

                if let Err(from_id) =
                    add_variant_tag_const_dependency(ctx, &mut tree, root_id, enum_id, variant_id)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(ctx, &tree);
                }
            }
        }
    }

    for id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(id);

        if data.layout.is_unresolved() {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::EnumLayout(id));
            let mut is_ok = true;

            for variant in data.variants {
                for ty in variant.fields {
                    if let Err(from_id) =
                        add_type_size_const_dependencies(ctx, &mut tree, root_id, *ty)
                    {
                        const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                        is_ok = false;
                        break;
                    }
                }
            }
            if is_ok {
                resolve_const_dependency_tree(ctx, &tree);
            }
        }
    }

    for id in ctx.registry.struct_ids() {
        let data = ctx.registry.struct_data(id);

        if data.layout.is_unresolved() {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::StructLayout(id));
            let mut is_ok = true;

            for field in data.fields {
                if let Err(from_id) =
                    add_type_size_const_dependencies(ctx, &mut tree, root_id, field.ty)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                    is_ok = false;
                    break;
                }
            }
            if is_ok {
                resolve_const_dependency_tree(ctx, &tree);
            }
        }
    }

    for id in ctx.registry.const_ids() {
        let data = ctx.registry.const_data(id);
        let (eval, origin_id) = *ctx.registry.const_eval(data.value);

        match eval {
            hir::ConstEval::Unresolved(expr) => {
                let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Const(id));

                if let Err(from_id) =
                    add_type_usage_const_dependencies(ctx, &mut tree, root_id, data.ty)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else if let Err(from_id) =
                    add_expr_const_dependencies(ctx, &mut tree, root_id, origin_id, expr.0)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(ctx, &tree);
                }
            }
            hir::ConstEval::ResolvedError => {}
            hir::ConstEval::Resolved(_) => {}
        }
    }

    for id in ctx.registry.global_ids() {
        let data = ctx.registry.global_data(id);
        let (eval, origin_id) = *ctx.registry.const_eval(data.value);

        match eval {
            hir::ConstEval::Unresolved(expr) => {
                let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Global(id));

                if let Err(from_id) =
                    add_type_usage_const_dependencies(ctx, &mut tree, root_id, data.ty)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else if let Err(from_id) =
                    add_expr_const_dependencies(ctx, &mut tree, root_id, origin_id, expr.0)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(ctx, &tree);
                }
            }
            hir::ConstEval::ResolvedError => {}
            hir::ConstEval::Resolved(_) => {}
        }
    }

    //@assuming that remaining constevals are array len 15.05.24
    // that didnt cycle with anything, thus can be resolved in `immediate mode`
    // this is only true when previos const dependencies and const evals were handled correctly
    //@how will unresolved expressions due to something else erroring earlier behave?
    // for example struct field with some unresolved array len
    for eval_id in ctx.registry.const_eval_ids() {
        let (eval, _) = ctx.registry.const_eval(eval_id);

        if matches!(eval, hir::ConstEval::Unresolved(_)) {
            let expect = Expectation::HasType(hir::Type::USIZE, None);
            resolve_and_update_const_eval(ctx, eval_id, expect);
        }
    }
}

struct Tree<T: PartialEq + Copy + Clone> {
    nodes: Vec<TreeNode<T>>,
}

type TreeNodeID<T> = ID<TreeNode<T>>;
struct TreeNode<T: PartialEq + Copy + Clone> {
    value: T,
    parent: Option<TreeNodeID<T>>,
}

impl<T: PartialEq + Copy + Clone> Tree<T> {
    #[must_use]
    fn new_rooted(root: T) -> (Tree<T>, TreeNodeID<T>) {
        let root_id = TreeNodeID::new_raw(0);
        let tree = Tree {
            nodes: vec![TreeNode {
                value: root,
                parent: None,
            }],
        };
        (tree, root_id)
    }

    #[must_use]
    fn add_child(&mut self, parent_id: TreeNodeID<T>, value: T) -> TreeNodeID<T> {
        let id = TreeNodeID::new(&self.nodes);
        self.nodes.push(TreeNode {
            value,
            parent: Some(parent_id),
        });
        id
    }

    #[must_use]
    fn find_cycle(&self, id: TreeNodeID<T>) -> Option<TreeNodeID<T>> {
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
    fn get_values_up_to_node(&self, from_id: TreeNodeID<T>, up_to: TreeNodeID<T>) -> Vec<T> {
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
    fn get_values_up_to_root(&self, from_id: TreeNodeID<T>) -> Vec<T> {
        let mut node = self.get_node(from_id);
        let mut values = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            values.push(node.value);
        }
        values
    }

    #[must_use]
    fn get_node(&self, id: TreeNodeID<T>) -> &TreeNode<T> {
        self.nodes.id_get(id)
    }
}

//@change Err type to enum
// bubble it to the top
// `already error`, `cycle` variants
// tree itself should return a result on each `add` operation
fn check_const_dependency_cycle<'hir>(
    ctx: &mut HirCtx,
    tree: &Tree<ConstDependency>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    node_id: TreeNodeID<ConstDependency>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let cycle_id = match tree.find_cycle(node_id) {
        Some(cycle_id) => cycle_id,
        None => return Ok(()),
    };

    let src = match tree.get_node(cycle_id).value {
        ConstDependency::EnumVariant(id, variant_id) => {
            let data = ctx.registry.enum_data(id);
            let variant = data.variant(variant_id);
            SourceRange::new(data.origin_id, variant.name.range)
        }
        ConstDependency::EnumLayout(id) => {
            let data = ctx.registry.enum_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::StructLayout(id) => {
            let data = ctx.registry.struct_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::Const(id) => {
            let data = ctx.registry.const_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::Global(id) => {
            let data = ctx.registry.global_data(id);
            SourceRange::new(data.origin_id, data.name.range)
        }
        ConstDependency::ArrayLen(eval_id) => {
            let (eval, origin_id) = *ctx.registry.const_eval(eval_id);
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
                let data = ctx.registry.enum_data(id);
                let variant = data.variant(variant_id);
                let msg = format!(
                    "{prefix}depends on `{}.{}` enum variant{postfix}",
                    ctx.name_str(data.name.id),
                    ctx.name_str(variant.name.id)
                );
                let src = SourceRange::new(data.origin_id, variant.name.range);
                (msg, src)
            }
            ConstDependency::EnumLayout(id) => {
                let data = ctx.registry.enum_data(id);
                let msg = format!(
                    "{prefix}depends on size of `{}`{postfix}",
                    ctx.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::StructLayout(id) => {
                let data = ctx.registry.struct_data(id);
                let msg = format!(
                    "{prefix}depends on size of `{}`{postfix}",
                    ctx.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Const(id) => {
                let data = ctx.registry.const_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` const value{postfix}",
                    ctx.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` global value{postfix}",
                    ctx.name_str(data.name.id)
                );
                let src = SourceRange::new(data.origin_id, data.name.range);
                (msg, src)
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, origin_id) = *ctx.registry.const_eval(eval_id);
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

    ctx.emit.error(ErrorComp::new_detailed_info_vec(
        "constant dependency cycle found:",
        ctx_msg,
        src,
        info_vec,
    ));
    Err(parent_id)
}

fn const_dependencies_mark_error_up_to_root<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &Tree<ConstDependency<'hir>>,
    from_id: TreeNodeID<ConstDependency<'hir>>,
) {
    let const_deps = tree.get_values_up_to_root(from_id);
    for dep in const_deps {
        match dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = ctx.registry.enum_data(id);
                let variant = data.variant(variant_id);

                match variant.kind {
                    hir::VariantKind::Default(_) => unreachable!(),
                    hir::VariantKind::Constant(eval_id) => {
                        let (eval, _) = ctx.registry.const_eval_mut(eval_id);
                        *eval = hir::ConstEval::ResolvedError;
                    }
                }
            }
            ConstDependency::EnumLayout(id) => {
                let data = ctx.registry.enum_data_mut(id);
                data.layout = hir::Eval::ResolvedError;
            }
            ConstDependency::StructLayout(id) => {
                let data = ctx.registry.struct_data_mut(id);
                data.layout = hir::Eval::ResolvedError;
            }
            ConstDependency::Const(id) => {
                let data = ctx.registry.const_data(id);
                let eval_id = data.value;
                let (eval, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let eval_id = data.value;
                let (eval, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
        }
    }
}

fn add_variant_tag_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    enum_id: hir::EnumID<'hir>,
    variant_id: hir::VariantID<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let data = ctx.registry.enum_data(enum_id);
    match data.variant(variant_id).kind {
        hir::VariantKind::Default(eval_id) => {
            let eval = *ctx.registry.variant_eval(eval_id);
            match eval {
                hir::Eval::Unresolved(_) => {
                    if variant_id.raw_index() > 0 {
                        let prev_id = variant_id.dec();
                        let prev = data.variant(prev_id);

                        let unresolved = match prev.kind {
                            hir::VariantKind::Default(eval_id) => {
                                let eval = *ctx.registry.variant_eval(eval_id);
                                eval.is_unresolved()
                            }
                            hir::VariantKind::Constant(eval_id) => {
                                let (eval, _) = *ctx.registry.const_eval(eval_id);
                                eval.is_unresolved()
                            }
                        };

                        if unresolved {
                            let node_id = tree.add_child(
                                parent_id,
                                ConstDependency::EnumVariant(enum_id, prev_id),
                            );
                            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

                            add_variant_tag_const_dependency(ctx, tree, node_id, enum_id, prev_id)?;
                        }
                    }
                    Ok(())
                }
                hir::Eval::Resolved(_) => Ok(()),
                hir::Eval::ResolvedError => Err(parent_id),
            }
        }
        hir::VariantKind::Constant(eval_id) => {
            let (eval, origin_id) = *ctx.registry.const_eval(eval_id);
            match eval {
                hir::ConstEval::Unresolved(expr) => {
                    add_expr_const_dependencies(ctx, tree, parent_id, origin_id, expr.0)?;
                    Ok(())
                }
                hir::ConstEval::Resolved(_) => Ok(()),
                hir::ConstEval::ResolvedError => Err(parent_id),
            }
        }
    }
}

//@verify correctness
// adds tag dependency and type usage of each inner field value
fn add_variant_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    enum_id: hir::EnumID<'hir>,
    variant_id: hir::VariantID<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    add_variant_tag_const_dependency(ctx, tree, parent_id, enum_id, variant_id)?;

    let data = ctx.registry.enum_data(enum_id);
    let variant = data.variant(variant_id);
    for ty in variant.fields {
        add_type_usage_const_dependencies(ctx, tree, parent_id, *ty)?;
    }
    Ok(())
}

fn add_const_var_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    const_id: hir::ConstID<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let data = ctx.registry.const_data(const_id);
    let const_ty = data.ty;
    let eval_id = data.value;
    let (eval, origin_id) = *ctx.registry.const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id = tree.add_child(parent_id, ConstDependency::Const(const_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            // @will order of eval be correct? 13.06.24
            add_type_usage_const_dependencies(ctx, tree, parent_id, const_ty)?;
            add_expr_const_dependencies(ctx, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::Resolved(_) => Ok(()),
    }
}

fn add_array_len_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    eval_id: hir::ConstEvalID,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let (eval, origin_id) = *ctx.registry.const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id = tree.add_child(parent_id, ConstDependency::ArrayLen(eval_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            add_expr_const_dependencies(ctx, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::Resolved(_) => Ok(()),
    }
}

fn add_type_size_const_dependencies<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    ty: hir::Type<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(id) => {
            add_enum_size_const_dependency(ctx, tree, parent_id, id)?;
        }
        hir::Type::Struct(id) => {
            add_struct_size_const_dependency(ctx, tree, parent_id, id)?;
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::Procedure(_) => {}
        hir::Type::ArraySlice(_) => {}
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_const_dependency(ctx, tree, parent_id, eval_id)?;
            }
            add_type_size_const_dependencies(ctx, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn add_enum_size_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    enum_id: hir::EnumID<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let data = ctx.registry.enum_data(enum_id);

    match data.layout {
        hir::Eval::Unresolved(()) => {
            let node_id = tree.add_child(parent_id, ConstDependency::EnumLayout(enum_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            //@forced re-borrow due to `check_const_dependency_cycle` taking &mut ctx
            let data = ctx.registry.enum_data(enum_id);
            for variant in data.variants {
                for ty in variant.fields {
                    add_type_size_const_dependencies(ctx, tree, node_id, *ty)?;
                }
            }
            Ok(())
        }
        hir::Eval::ResolvedError => Err(parent_id),
        hir::Eval::Resolved(_) => Ok(()),
    }
}

fn add_struct_size_const_dependency<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    struct_id: hir::StructID<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    let data = ctx.registry.struct_data(struct_id);

    match data.layout {
        hir::Eval::Unresolved(()) => {
            let node_id = tree.add_child(parent_id, ConstDependency::StructLayout(struct_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            //@forced re-borrow due to `check_const_dependency_cycle` taking &mut ctx
            let data = ctx.registry.struct_data(struct_id);
            for field in data.fields {
                add_type_size_const_dependencies(ctx, tree, node_id, field.ty)?;
            }
            Ok(())
        }
        hir::Eval::ResolvedError => Err(parent_id),
        hir::Eval::Resolved(_) => Ok(()),
    }
}

fn add_type_usage_const_dependencies<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    ty: hir::Type<'hir>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(id) => {
            let data = ctx.registry.enum_data(id);
            for variant in data.variants {
                for ty in variant.fields {
                    add_type_usage_const_dependencies(ctx, tree, parent_id, *ty)?;
                }
            }
        }
        hir::Type::Struct(id) => {
            let data = ctx.registry.struct_data(id);
            for field in data.fields {
                add_type_usage_const_dependencies(ctx, tree, parent_id, field.ty)?
            }
        }
        hir::Type::Reference(ref_ty, _) => {
            add_type_usage_const_dependencies(ctx, tree, parent_id, *ref_ty)?
        }
        hir::Type::Procedure(proc_ty) => {
            for param_ty in proc_ty.param_types {
                add_type_usage_const_dependencies(ctx, tree, parent_id, *param_ty)?
            }
            add_type_usage_const_dependencies(ctx, tree, parent_id, proc_ty.return_ty)?
        }
        hir::Type::ArraySlice(slice) => {
            add_type_usage_const_dependencies(ctx, tree, parent_id, slice.elem_ty)?;
        }
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_const_dependency(ctx, tree, parent_id, eval_id)?;
            }
            add_type_usage_const_dependencies(ctx, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn add_expr_const_dependencies<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast>,
    tree: &mut Tree<ConstDependency<'hir>>,
    parent_id: TreeNodeID<ConstDependency<'hir>>,
    origin_id: ModuleID,
    expr: &ast::Expr<'ast>,
) -> Result<(), TreeNodeID<ConstDependency<'hir>>> {
    match expr.kind {
        ast::ExprKind::Lit { .. } => Ok(()),
        ast::ExprKind::If { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "if");
            Err(parent_id)
        }
        ast::ExprKind::Block { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "block");
            Err(parent_id)
        }
        ast::ExprKind::Match { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "match");
            Err(parent_id)
        }
        ast::ExprKind::Field { target, .. } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        //@index or slicing
        ast::ExprKind::Index {
            target,
            mutt,
            index,
        } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, target)?;
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, index)?;
            Ok(())
        }
        ast::ExprKind::Call { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "procedure call");
            Err(parent_id)
        }
        ast::ExprKind::Cast { target, .. } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Sizeof { ty } => {
            let ty = pass_3::type_resolve_delayed(ctx, origin_id, *ty);
            add_type_size_const_dependencies(ctx, tree, parent_id, ty)?;
            Ok(())
        }
        //@input not used
        ast::ExprKind::Item { path, input } => {
            let (value_id, _) = pass_5::path_resolve_value(ctx, origin_id, path);
            match value_id {
                pass_5::ValueID::None => Err(parent_id),
                pass_5::ValueID::Proc(proc_id) => {
                    //@borrowing hacks, just get data once here
                    // change the result Err type with delayed mutation of HirData only at top lvl?
                    for param in ctx.registry.proc_data(proc_id).params {
                        add_type_usage_const_dependencies(ctx, tree, parent_id, param.ty)?;
                    }
                    let data = ctx.registry.proc_data(proc_id);
                    add_type_usage_const_dependencies(ctx, tree, parent_id, data.return_ty)?;
                    Ok(())
                }
                pass_5::ValueID::Enum(enum_id, variant_id) => {
                    add_variant_const_dependency(ctx, tree, parent_id, enum_id, variant_id)?;
                    Ok(())
                }
                pass_5::ValueID::Const(const_id) => {
                    add_const_var_const_dependency(ctx, tree, parent_id, const_id)?;
                    Ok(())
                }
                pass_5::ValueID::Global(_) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "globals",
                    );
                    Err(parent_id)
                }
                pass_5::ValueID::Local(_) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "locals",
                    );
                    Err(parent_id)
                }
                pass_5::ValueID::Param(_) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "parameters",
                    );
                    Err(parent_id)
                }
            }
        }
        ast::ExprKind::Variant { .. } => {
            //@no type inference on this `ast name resolve` pass thus cannot infer variant type 14.06.24
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "variant selector");
            Err(parent_id)
        }
        ast::ExprKind::StructInit { struct_init } => match struct_init.path {
            //@cannot infer struct / enum variant type in constants
            Some(path) => {
                if let Some(struct_id) = pass_5::path_resolve_struct(ctx, origin_id, path) {
                    let ty = hir::Type::Struct(struct_id);
                    add_type_usage_const_dependencies(ctx, tree, parent_id, ty)?;
                    for init in struct_init.input {
                        add_expr_const_dependencies(ctx, tree, parent_id, origin_id, init.expr)?;
                    }
                    Ok(())
                } else {
                    Err(parent_id)
                }
            }
            None => {
                pass_5::error_cannot_infer_struct_type(
                    &mut ctx.emit,
                    SourceRange::new(origin_id, expr.range),
                );
                Err(parent_id)
            }
        },
        ast::ExprKind::ArrayInit { input } => {
            for &expr in input {
                add_expr_const_dependencies(ctx, tree, parent_id, origin_id, expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayRepeat { expr, len } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, expr)?;
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, len.0)?;
            Ok(())
        }
        ast::ExprKind::Deref { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "deref");
            Err(parent_id)
        }
        ast::ExprKind::Address { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "address");
            Err(parent_id)
        }
        ast::ExprKind::Range { range } => todo!("range feature"),
        ast::ExprKind::Unary { rhs, .. } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
        ast::ExprKind::Binary { bin, .. } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, bin.lhs)?;
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, bin.rhs)?;
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
    ctx: &mut HirCtx<'hir, '_>,
    tree: &Tree<ConstDependency<'hir>>,
) {
    // reverse iteration allows to resolve dependencies in correct order
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::EnumVariant(enum_id, variant_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let variant = data.variant(variant_id);

                // variant is set to `ResolvedError` if tag_ty is not known (safe to unwrap)
                let tag_ty = data.tag_ty.unwrap();
                let expect = Expectation::HasType(hir::Type::Basic(tag_ty.into_basic()), None);

                match variant.kind {
                    hir::VariantKind::Default(eval_id) => {
                        if variant_id.raw_index() == 0 {
                            let zero = hir::ConstValue::Int {
                                val: 0,
                                neg: false,
                                int_ty: tag_ty,
                            };

                            let eval = ctx.registry.variant_eval_mut(eval_id);
                            *eval = hir::Eval::Resolved(zero);
                        } else {
                            let prev = data.variant(variant_id.dec());

                            let prev_value = match prev.kind {
                                hir::VariantKind::Default(eval_id) => {
                                    let eval = ctx.registry.variant_eval(eval_id);
                                    eval.get_resolved()
                                }
                                hir::VariantKind::Constant(eval_id) => {
                                    let (eval, _) = ctx.registry.const_eval(eval_id);
                                    eval.get_resolved().map(|id| ctx.const_intern.get(id))
                                }
                            };

                            //@have some int_inc function in fold:: instead of `pub` const value api for `into_int` / int_range_check()
                            let value_res = match prev_value {
                                Ok(prev) => {
                                    let prev_tag = prev.into_int();
                                    let prev_inc = prev_tag + 1;
                                    let variant_src =
                                        SourceRange::new(data.origin_id, variant.name.range);
                                    fold::int_range_check(ctx, variant_src, prev_inc, tag_ty)
                                }
                                Err(()) => Err(()),
                            };

                            let eval = ctx.registry.variant_eval_mut(eval_id);
                            *eval = hir::Eval::from_res(value_res);
                        }
                    }
                    hir::VariantKind::Constant(eval_id) => {
                        resolve_and_update_const_eval(ctx, eval_id, expect);
                    }
                }
            }
            ConstDependency::EnumLayout(id) => {
                let layout_res = layout::resolve_enum_layout(ctx, id);
                let layout = hir::Eval::from_res(layout_res);
                ctx.registry.enum_data_mut(id).layout = layout;
            }
            ConstDependency::StructLayout(id) => {
                let layout_res = layout::resolve_struct_layout(ctx, id);
                let layout = hir::Eval::from_res(layout_res);
                ctx.registry.struct_data_mut(id).layout = layout;
            }
            ConstDependency::Const(id) => {
                let data = ctx.registry.const_data(id);
                let item = ctx.registry.const_item(id);

                let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                let expect = Expectation::HasType(data.ty, Some(expect_src));
                resolve_and_update_const_eval(ctx, data.value, expect);
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let item = ctx.registry.global_item(id);

                let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                let expect = Expectation::HasType(data.ty, Some(expect_src));
                resolve_and_update_const_eval(ctx, data.value, expect);
            }
            ConstDependency::ArrayLen(eval_id) => {
                let expect = Expectation::HasType(hir::Type::USIZE, None);
                resolve_and_update_const_eval(ctx, eval_id, expect);
            }
        }
    }
}

//@change how this is handled, still check double resolve
// with assert to catch potential bugs in implementation
fn resolve_and_update_const_eval<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    eval_id: hir::ConstEvalID,
    expect: Expectation<'hir>,
) {
    let (eval, origin_id) = *ctx.registry.const_eval(eval_id);
    ctx.proc.reset_origin(origin_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let value_res = resolve_const_expr(ctx, origin_id, expect, expr);
            let value_res = value_res.map(|v| ctx.const_intern.intern(v));
            let (eval, _) = ctx.registry.const_eval_mut(eval_id);
            *eval = hir::Eval::from_res(value_res);
        }
        _ => panic!("calling `resolve_const_expr` on already resolved expr"),
    };
}

#[must_use]
pub fn resolve_const_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_>,
    origin_id: ModuleID,
    expect: Expectation<'hir>,
    expr: ast::ConstExpr,
) -> Result<hir::ConstValue<'hir>, ()> {
    let error_count = ctx.emit.error_count();
    let expr_res = pass_5::typecheck_expr(ctx, expect, expr.0);

    if !ctx.emit.did_error(error_count) {
        let src = SourceRange::new(origin_id, expr_res.expr.range);
        fold::fold_const_expr(ctx, src, expr_res.expr)
    } else {
        Err(())
    }
}

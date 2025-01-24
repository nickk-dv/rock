use super::super::check_path::{self, ValueID};
use super::super::context::HirCtx;
use super::super::{pass_3, pass_5, pass_5::Expectation};
use super::fold;
use super::layout;
use crate::ast;
use crate::error::{Error, ErrorSink, ErrorWarningBuffer, Info, SourceRange, StringOrStr};
use crate::errors as err;
use crate::hir;
use crate::session::ModuleID;
use crate::text::TextRange;

//@set correct poly scopes everywhere paths are resolved
pub fn resolve_const_dependencies(ctx: &mut HirCtx) {
    let mut tree = Tree { nodes: Vec::with_capacity(128) };

    for enum_id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(enum_id);
        let error_count = ctx.emit.error_count();

        for (idx, variant) in data.variants.iter().enumerate() {
            let variant_id = hir::VariantID::new(idx);

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
                let root_id =
                    tree.root_and_reset(ConstDependency::EnumVariant(enum_id, variant_id));

                if let Err(from_id) =
                    add_variant_tag_const_dependency(ctx, &mut tree, root_id, enum_id, variant_id)
                {
                    const_dependencies_mark_error_up_to_root(ctx, &tree, from_id);
                } else {
                    resolve_const_dependency_tree(ctx, &tree);
                }
            }
        }

        if ctx.emit.did_error(error_count) {
            continue;
        }
        ctx.enum_tag_set.clear();
        let data = ctx.registry.enum_data(enum_id);

        for (idx, variant) in data.variants.iter().enumerate() {
            let variant_id = hir::VariantID::new(idx);

            let tag_value = match variant.kind {
                hir::VariantKind::Default(eval_id) => {
                    if let Ok(value) = ctx.registry.variant_eval(eval_id).resolved() {
                        value.into_int()
                    } else {
                        continue;
                    }
                }
                hir::VariantKind::Constant(eval_id) => {
                    if let Ok(value) = ctx.registry.const_eval(eval_id).0.resolved() {
                        value.into_int()
                    } else {
                        continue;
                    }
                }
            };

            if let Some(existing_id) = ctx.enum_tag_set.get(&tag_value).copied() {
                let existing_variant = data.variant(existing_id);

                let variant_src = SourceRange::new(data.origin_id, variant.name.range);
                let existing = SourceRange::new(data.origin_id, existing_variant.name.range);
                let variant_name = ctx.name(variant.name.id);
                let existing_name = ctx.name(existing_variant.name.id);
                err::item_enum_duplicate_tag_value(
                    &mut ctx.emit,
                    variant_src,
                    existing,
                    variant_name,
                    existing_name,
                    tag_value,
                );
            } else {
                ctx.enum_tag_set.insert(tag_value, variant_id);
            }
        }
    }

    for id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(id);

        if data.layout.is_unresolved() {
            let root_id = tree.root_and_reset(ConstDependency::EnumLayout(id));
            let mut is_ok = true;

            for variant in data.variants {
                for field in variant.fields {
                    if let Err(from_id) =
                        add_type_size_const_dependencies(ctx, &mut tree, root_id, field.ty)
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
            let root_id = tree.root_and_reset(ConstDependency::StructLayout(id));
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
                let root_id = tree.root_and_reset(ConstDependency::Const(id));

                if let Err(from_id) = {
                    if let Some(const_ty) = data.ty {
                        add_type_usage_const_dependencies(ctx, &mut tree, root_id, const_ty)
                    } else {
                        Ok(())
                    }
                } {
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
        let eval_id = match data.init {
            hir::GlobalInit::Init(eval_id) => eval_id,
            hir::GlobalInit::Zeroed => continue,
        };
        let (eval, origin_id) = *ctx.registry.const_eval(eval_id);

        match eval {
            hir::ConstEval::Unresolved(expr) => {
                let root_id = tree.root_and_reset(ConstDependency::Global(id));

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
        if eval.is_unresolved() {
            resolve_and_update_const_eval(ctx, eval_id, Expectation::USIZE);
        }
    }
}

//@change Err type to enum
// bubble it to the top
// `already error`, `cycle` variants
// tree itself should return a result on each `add` operation
fn check_const_dependency_cycle(
    ctx: &mut HirCtx,
    tree: &Tree,
    parent_id: TreeNodeID,
    node_id: TreeNodeID,
) -> Result<(), TreeNodeID> {
    let cycle_id = match tree.find_cycle(node_id) {
        Some(cycle_id) => cycle_id,
        None => return Ok(()),
    };

    let src = match tree.node(cycle_id).value {
        ConstDependency::EnumVariant(id, variant_id) => {
            let data = ctx.registry.enum_data(id);
            let variant = data.variant(variant_id);
            SourceRange::new(data.origin_id, variant.name.range)
        }
        ConstDependency::EnumLayout(id) => ctx.registry.enum_data(id).src(),
        ConstDependency::StructLayout(id) => ctx.registry.struct_data(id).src(),
        ConstDependency::Const(id) => ctx.registry.const_data(id).src(),
        ConstDependency::Global(id) => ctx.registry.global_data(id).src(),
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

    let cycle_deps = tree.values_up_to_node(node_id, cycle_id);
    let mut ctx_msg: StringOrStr = "".into();
    let mut info_vec = Vec::with_capacity(cycle_deps.len());
    let mut info_src = src;

    for (idx, const_dep) in cycle_deps.iter().cloned().rev().skip(1).enumerate() {
        let first = idx == 0;
        let last = idx + 2 == cycle_deps.len();

        let prefix = if first { "" } else { "which " };
        let postfix = if last { ", completing the cycle..." } else { "" };

        let (msg, src) = match const_dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = ctx.registry.enum_data(id);
                let variant = data.variant(variant_id);
                let msg = format!(
                    "{prefix}depends on `{}.{}` enum variant{postfix}",
                    ctx.name(data.name.id),
                    ctx.name(variant.name.id)
                );
                let src = SourceRange::new(data.origin_id, variant.name.range);
                (msg, src)
            }
            ConstDependency::EnumLayout(id) => {
                let data = ctx.registry.enum_data(id);
                let msg =
                    format!("{prefix}depends on size of `{}`{postfix}", ctx.name(data.name.id));
                (msg, data.src())
            }
            ConstDependency::StructLayout(id) => {
                let data = ctx.registry.struct_data(id);
                let msg =
                    format!("{prefix}depends on size of `{}`{postfix}", ctx.name(data.name.id));
                (msg, data.src())
            }
            ConstDependency::Const(id) => {
                let data = ctx.registry.const_data(id);
                let msg =
                    format!("{prefix}depends on `{}` const value{postfix}", ctx.name(data.name.id));
                (msg, data.src())
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let msg = format!(
                    "{prefix}depends on `{}` global value{postfix}",
                    ctx.name(data.name.id)
                );
                (msg, data.src())
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
            info_vec.push(Info::new_val(msg, info_src));
        }

        info_src = src;
    }

    ctx.emit.error(Error::new_info_vec("constant dependency cycle found:", ctx_msg, src, info_vec));
    Err(parent_id)
}

fn const_dependencies_mark_error_up_to_root(ctx: &mut HirCtx, tree: &Tree, from_id: TreeNodeID) {
    let const_deps = tree.values_up_to_root(from_id);
    for dep in const_deps {
        match dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = ctx.registry.enum_data(id);
                let variant = data.variant(variant_id);

                match variant.kind {
                    hir::VariantKind::Default(eval_id) => {
                        let eval = ctx.registry.variant_eval_mut(eval_id);
                        *eval = hir::VariantEval::ResolvedError;
                    }
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
                let eval_id = match data.init {
                    hir::GlobalInit::Init(eval_id) => eval_id,
                    hir::GlobalInit::Zeroed => unreachable!(),
                };
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

fn add_variant_tag_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.enum_data(enum_id);
    match data.variant(variant_id).kind {
        hir::VariantKind::Default(eval_id) => {
            let eval = *ctx.registry.variant_eval(eval_id);
            match eval {
                hir::Eval::Unresolved(_) => {
                    if variant_id.raw() > 0 {
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
fn add_variant_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) -> Result<(), TreeNodeID> {
    add_variant_tag_const_dependency(ctx, tree, parent_id, enum_id, variant_id)?;

    let data = ctx.registry.enum_data(enum_id);
    let variant = data.variant(variant_id);
    for field in variant.fields {
        add_type_usage_const_dependencies(ctx, tree, parent_id, field.ty)?;
    }
    Ok(())
}

fn add_const_var_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    const_id: hir::ConstID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.const_data(const_id);
    let const_ty = data.ty;
    let eval_id = data.value;
    let (eval, origin_id) = *ctx.registry.const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            let node_id = tree.add_child(parent_id, ConstDependency::Const(const_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            // @will order of eval be correct? 13.06.24
            if let Some(const_ty) = const_ty {
                add_type_usage_const_dependencies(ctx, tree, parent_id, const_ty)?;
            }
            add_expr_const_dependencies(ctx, tree, node_id, origin_id, expr.0)?;
            Ok(())
        }
        hir::ConstEval::ResolvedError => Err(parent_id),
        hir::ConstEval::Resolved(_) => Ok(()),
    }
}

fn add_array_len_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    eval_id: hir::ConstEvalID,
) -> Result<(), TreeNodeID> {
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
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    ty: hir::Type<'hir>,
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => return Err(parent_id),
        hir::Type::Any => {}
        hir::Type::Char => {}
        hir::Type::Void => {}
        hir::Type::Never => {}
        hir::Type::Rawptr => {}
        hir::Type::Int(_) => {}
        hir::Type::Float(_) => {}
        hir::Type::Bool(_) => {}
        hir::Type::String(_) => {}
        hir::Type::InferDef(_, _) => {
            eprintln!("unhandled infer_def in (add_type_size_const_dependencies)");
            return Err(parent_id);
        }
        hir::Type::Enum(id, poly_types) => {
            if !poly_types.is_empty() {
                eprintln!("unhandled poly_types in enum (add_type_size_const_dependencies)");
                return Err(parent_id);
            }
            add_enum_size_const_dependency(ctx, tree, parent_id, id)?;
        }
        hir::Type::Struct(id, poly_types) => {
            if !poly_types.is_empty() {
                eprintln!("unhandled poly_types in struct (add_type_size_const_dependencies)");
                return Err(parent_id);
            }
            add_struct_size_const_dependency(ctx, tree, parent_id, id)?;
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::MultiReference(_, _) => {}
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

fn add_enum_size_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.enum_data(enum_id);

    match data.layout {
        hir::Eval::Unresolved(()) => {
            let node_id = tree.add_child(parent_id, ConstDependency::EnumLayout(enum_id));
            check_const_dependency_cycle(ctx, tree, parent_id, node_id)?;

            //@forced re-borrow due to `check_const_dependency_cycle` taking &mut ctx
            let data = ctx.registry.enum_data(enum_id);
            for variant in data.variants {
                for field in variant.fields {
                    add_type_size_const_dependencies(ctx, tree, node_id, field.ty)?;
                }
            }
            Ok(())
        }
        hir::Eval::ResolvedError => Err(parent_id),
        hir::Eval::Resolved(_) => Ok(()),
    }
}

fn add_struct_size_const_dependency(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
) -> Result<(), TreeNodeID> {
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
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    ty: hir::Type<'hir>,
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => return Err(parent_id),
        hir::Type::Any => {}
        hir::Type::Char => {}
        hir::Type::Void => {}
        hir::Type::Never => {}
        hir::Type::Rawptr => {}
        hir::Type::Int(_) => {}
        hir::Type::Float(_) => {}
        hir::Type::Bool(_) => {}
        hir::Type::String(_) => {}
        hir::Type::InferDef(_, _) => {
            eprintln!("unhandled infer_def in (add_type_usage_const_dependencies)");
            return Err(parent_id);
        }
        hir::Type::Enum(id, poly_types) => {
            if !poly_types.is_empty() {
                eprintln!("unhandled poly_types in enum (add_type_usage_const_dependencies)");
                return Err(parent_id);
            }
            let data = ctx.registry.enum_data(id);
            for variant in data.variants {
                for field in variant.fields {
                    add_type_usage_const_dependencies(ctx, tree, parent_id, field.ty)?;
                }
            }
        }
        hir::Type::Struct(id, poly_types) => {
            if !poly_types.is_empty() {
                eprintln!("unhandled poly_types in struct (add_type_usage_const_dependencies)");
                return Err(parent_id);
            }
            let data = ctx.registry.struct_data(id);
            for field in data.fields {
                add_type_usage_const_dependencies(ctx, tree, parent_id, field.ty)?
            }
        }
        hir::Type::Reference(_, ref_ty) => {
            add_type_usage_const_dependencies(ctx, tree, parent_id, *ref_ty)?
        }
        hir::Type::MultiReference(_, ref_ty) => {
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

fn add_expr_const_dependencies<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    origin_id: ModuleID,
    expr: &ast::Expr<'ast>,
) -> Result<(), TreeNodeID> {
    //@check_path uses set origin to report errors
    ctx.scope.set_origin(origin_id);

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
        ast::ExprKind::Index { target, index } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, target)?;
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, index)?;
            Ok(())
        }
        ast::ExprKind::Slice { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "slice");
            Err(parent_id)
        }
        ast::ExprKind::Call { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "call");
            Err(parent_id)
        }
        ast::ExprKind::Cast { target, .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "cast");
            Err(parent_id)
            //@add_expr_const_dependencies(ctx, tree, parent_id, origin_id, target)?;
            //Ok(())
        }
        ast::ExprKind::Sizeof { ty } => {
            let ty = pass_3::type_resolve(ctx, *ty, true);
            add_type_size_const_dependencies(ctx, tree, parent_id, ty)?;
            Ok(())
        }
        ast::ExprKind::Directive { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "directive");
            Err(parent_id)
        }
        ast::ExprKind::Item { path, args_list } => {
            match check_path::path_resolve_value(ctx, path) {
                ValueID::None => Err(parent_id),
                //@fold panics on direct | indirect calls
                ValueID::Proc(proc_id) => {
                    //@borrowing hacks, just get data once here
                    // change the result Err type with delayed mutation of HirData only at top lvl?
                    for param in ctx.registry.proc_data(proc_id).params {
                        add_type_usage_const_dependencies(ctx, tree, parent_id, param.ty)?;
                    }
                    let data = ctx.registry.proc_data(proc_id);
                    add_type_usage_const_dependencies(ctx, tree, parent_id, data.return_ty)?;

                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Enum(enum_id, variant_id) => {
                    add_variant_const_dependency(ctx, tree, parent_id, enum_id, variant_id)?;
                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Const(const_id, _) => {
                    add_const_var_const_dependency(ctx, tree, parent_id, const_id)?;
                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Global(_, _) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "globals",
                    );
                    Err(parent_id)
                }
                ValueID::Param(_, _) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "parameters",
                    );
                    Err(parent_id)
                }
                ValueID::Variable(_, _) => {
                    error_cannot_refer_to_in_constants(
                        &mut ctx.emit,
                        origin_id,
                        expr.range,
                        "variables",
                    );
                    Err(parent_id)
                }
            }
        }
        ast::ExprKind::Variant { args_list, .. } => {
            if let Some(arg_list) = args_list {
                for arg in arg_list.exprs {
                    add_expr_const_dependencies(ctx, tree, parent_id, origin_id, arg)?;
                }
            }
            Ok(())
        }
        ast::ExprKind::StructInit { struct_init } => {
            //@make sure dependency order is correct for typecheck to work
            // both with & without known struct type in the struct_init
            if let Some(path) = struct_init.path {
                if let Some(struct_id) = check_path::path_resolve_struct(ctx, path) {
                    //@poly_types not handled
                    let struct_ty = hir::Type::Struct(struct_id, &[]);
                    add_type_usage_const_dependencies(ctx, tree, parent_id, struct_ty)?;
                } else {
                    return Err(parent_id);
                }
            }
            for init in struct_init.input {
                add_expr_const_dependencies(ctx, tree, parent_id, origin_id, init.expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayInit { input } => {
            for &expr in input {
                add_expr_const_dependencies(ctx, tree, parent_id, origin_id, expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayRepeat { value, len } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, value)?;
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
        ast::ExprKind::Unary { rhs, .. } => {
            add_expr_const_dependencies(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            error_cannot_use_in_constants(
                &mut ctx.emit,
                origin_id,
                expr.range,
                "binary expr (temp)",
            );
            Err(parent_id)
            //add_expr_const_dependencies(ctx, tree, parent_id, origin_id, lhs)?;
            //add_expr_const_dependencies(ctx, tree, parent_id, origin_id, rhs)?;
            //Ok(())
        }
    }
}

fn error_cannot_use_in_constants(
    emit: &mut ErrorWarningBuffer,
    origin_id: ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(Error::new(
        format!("cannot use `{name}` expression in constants"),
        SourceRange::new(origin_id, range),
        None,
    ));
}

fn error_cannot_refer_to_in_constants(
    emit: &mut ErrorWarningBuffer,
    origin_id: ModuleID,
    range: TextRange,
    name: &str,
) {
    emit.error(Error::new(
        format!("cannot refer to `{name}` in constants"),
        SourceRange::new(origin_id, range),
        None,
    ));
}

fn resolve_const_dependency_tree(ctx: &mut HirCtx, tree: &Tree) {
    // reverse iteration allows to resolve dependencies in correct order
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::EnumVariant(enum_id, variant_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let variant = data.variant(variant_id);

                //@variant is set to `ResolvedError` if tag_ty is not known (safe to unwrap)
                let tag_ty = data.tag_ty.resolved_unwrap();
                let expect = Expectation::HasType(hir::Type::Int(tag_ty), None);

                match variant.kind {
                    hir::VariantKind::Default(eval_id) => {
                        if variant_id.raw() == 0 {
                            let zero = hir::ConstValue::Int { val: 0, neg: false, int_ty: tag_ty };

                            let eval = ctx.registry.variant_eval_mut(eval_id);
                            *eval = hir::Eval::Resolved(zero);
                        } else {
                            let prev = data.variant(variant_id.dec());

                            let prev_value = match prev.kind {
                                hir::VariantKind::Default(eval_id) => {
                                    let eval = ctx.registry.variant_eval(eval_id);
                                    eval.resolved()
                                }
                                hir::VariantKind::Constant(eval_id) => {
                                    let (eval, _) = ctx.registry.const_eval(eval_id);
                                    eval.resolved()
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

                let expect = if let Some(ty) = item.ty {
                    let expect_src = SourceRange::new(data.origin_id, ty.range);
                    Expectation::HasType(data.ty.unwrap(), Some(expect_src)) //unwrap since item and data are both typed
                } else {
                    Expectation::None
                };
                if let Some(ty) = resolve_and_update_const_eval(ctx, data.value, expect) {
                    let data = ctx.registry.const_data_mut(id);
                    data.ty = Some(ty);
                }
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let item = ctx.registry.global_item(id);

                let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                let expect = Expectation::HasType(data.ty, Some(expect_src));
                let eval_id = match data.init {
                    hir::GlobalInit::Init(eval_id) => eval_id,
                    hir::GlobalInit::Zeroed => unreachable!(),
                };
                resolve_and_update_const_eval(ctx, eval_id, expect);
            }
            ConstDependency::ArrayLen(eval_id) => {
                resolve_and_update_const_eval(ctx, eval_id, Expectation::USIZE);
            }
        }
    }
}

fn resolve_and_update_const_eval<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    eval_id: hir::ConstEvalID,
    expect: Expectation<'hir>,
) -> Option<hir::Type<'hir>> {
    let (eval, origin_id) = *ctx.registry.const_eval(eval_id);

    match eval {
        hir::ConstEval::Unresolved(expr) => {
            //@reset any possible blocks / locals
            // currently blocks or stmts are not supported in constants
            // so this is in theory not required
            ctx.scope.set_origin(origin_id);
            ctx.scope.local.reset();

            let (value_res, value_ty) = resolve_const_expr(ctx, expect, expr);
            let (eval, _) = ctx.registry.const_eval_mut(eval_id);
            *eval = hir::Eval::from_res(value_res);
            Some(value_ty)
        }
        // ignore resolution calls on already resolved
        hir::ConstEval::Resolved(_) => None,
        hir::ConstEval::ResolvedError => None,
    }
}

/// typecheck and fold contant expression
/// in currently active scope origin_id
pub fn resolve_const_expr<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: ast::ConstExpr<'ast>,
) -> (Result<hir::ConstValue<'hir>, ()>, hir::Type<'hir>) {
    let error_count = ctx.emit.error_count();
    let expr_res = match expect {
        Expectation::HasType(_, _) => pass_5::typecheck_expr(ctx, expect, expr.0),
        _ => pass_5::typecheck_expr_untyped(ctx, expect, expr.0),
    };

    if !ctx.emit.did_error(error_count) {
        match expr_res.expr {
            hir::Expr::Error => (Err(()), expr_res.ty),
            hir::Expr::Const { value } => (Ok(*value), expr_res.ty),
            _ => {
                error_cannot_use_in_constants(
                    &mut ctx.emit,
                    ctx.scope.origin(),
                    expr.0.range,
                    "non constant",
                ); //@temp message for this case
                (Err(()), expr_res.ty)
            }
        }
    } else {
        (Err(()), expr_res.ty)
    }
}

//==================== DEPENDENCY TREE ====================

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    EnumVariant(hir::EnumID, hir::VariantID),
    EnumLayout(hir::EnumID),
    StructLayout(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    ArrayLen(hir::ConstEvalID),
}

crate::define_id!(TreeNodeID);
struct Tree {
    nodes: Vec<TreeNode>,
}
struct TreeNode {
    value: ConstDependency,
    parent: Option<TreeNodeID>,
}

//@remove the vector allocations
// iterate directly where needed
impl Tree {
    #[must_use]
    #[inline(always)]
    fn node(&self, id: TreeNodeID) -> &TreeNode {
        &self.nodes[id.index()]
    }
    #[must_use]
    #[inline(always)]
    fn root_and_reset(&mut self, value: ConstDependency) -> TreeNodeID {
        self.nodes.clear();
        let id = TreeNodeID::new(self.nodes.len());
        self.nodes.push(TreeNode { value, parent: None });
        id
    }
    #[must_use]
    #[inline(always)]
    fn add_child(&mut self, parent_id: TreeNodeID, value: ConstDependency) -> TreeNodeID {
        let id = TreeNodeID::new(self.nodes.len());
        self.nodes.push(TreeNode { value, parent: Some(parent_id) });
        id
    }
    #[must_use]
    fn find_cycle(&self, id: TreeNodeID) -> Option<TreeNodeID> {
        let mut node = self.node(id);
        let value = node.value;

        while let Some(parent_id) = node.parent {
            node = self.node(parent_id);
            if node.value == value {
                return Some(parent_id);
            }
        }
        None
    }
    #[must_use]
    fn values_up_to_node(&self, from: TreeNodeID, up_to: TreeNodeID) -> Vec<ConstDependency> {
        let mut node = self.node(from);
        let mut values = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.node(parent_id);
            values.push(node.value);
            if parent_id == up_to {
                return values;
            }
        }
        values
    }
    #[must_use]
    fn values_up_to_root(&self, from: TreeNodeID) -> Vec<ConstDependency> {
        let mut node = self.node(from);
        let mut values = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.node(parent_id);
            values.push(node.value);
        }
        values
    }
}

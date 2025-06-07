use super::check_path::{self, ValueID};
use super::context::HirCtx;
use super::layout;
use super::{pass_3, pass_5, pass_5::Expectation};
use crate::ast;
use crate::error::{ErrorSink, Info, SourceRange};
use crate::errors as err;
use crate::hir;
use crate::session::ModuleID;

pub fn resolve_const_dependencies(ctx: &mut HirCtx) {
    let mut tree = Tree { nodes: Vec::with_capacity(128) };

    for enum_id in ctx.registry.enum_ids() {
        let data = ctx.registry.enum_data(enum_id);
        let error_count = ctx.emit.error_count();

        for variant_id in (0..data.variants.len()).map(hir::VariantID::new).rev() {
            let parent_id = tree.root_and_reset();
            match add_variant_tag_deps(ctx, &mut tree, parent_id, enum_id, variant_id) {
                Ok(()) => resolve_dependency_tree(ctx, &tree),
                Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
            }
        }
        if !ctx.emit.did_error(error_count) {
            check_duplicate_variant_tags(ctx, enum_id)
        }
    }
    for enum_id in ctx.registry.enum_ids() {
        let parent_id = tree.root_and_reset();
        match add_enum_size_deps(ctx, &mut tree, parent_id, enum_id, &[]) {
            Ok(()) => resolve_dependency_tree(ctx, &tree),
            Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
        }
    }
    for struct_id in ctx.registry.struct_ids() {
        let parent_id = tree.root_and_reset();
        match add_struct_size_deps(ctx, &mut tree, parent_id, struct_id, &[]) {
            Ok(()) => resolve_dependency_tree(ctx, &tree),
            Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
        }
    }
    for const_id in ctx.registry.const_ids() {
        let parent_id = tree.root_and_reset();
        match add_const_var_deps(ctx, &mut tree, parent_id, const_id) {
            Ok(()) => resolve_dependency_tree(ctx, &tree),
            Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
        }
    }
    for global_id in ctx.registry.global_ids() {
        let parent_id = tree.root_and_reset();
        match add_global_var_deps(ctx, &mut tree, parent_id, global_id) {
            Ok(()) => resolve_dependency_tree(ctx, &tree),
            Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
        }
    }
    // remaining constevals must be array len expressions
    for eval_id in ctx.registry.const_eval_ids() {
        let (eval, _, _) = ctx.registry.const_eval(eval_id);
        if eval.is_unresolved() {
            resolve_and_update_const_eval(ctx, eval_id, Expectation::USIZE);
        }
    }
}

fn check_duplicate_variant_tags(ctx: &mut HirCtx, enum_id: hir::EnumID) {
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

fn mark_error_up_to_root(ctx: &mut HirCtx, tree: &Tree, from_id: TreeNodeID) {
    for dep in tree.values_up_to_node(from_id, TreeNodeID(0)) {
        match dep {
            ConstDependency::Root => {}
            ConstDependency::VariantTag(id, variant_id) => {
                let data = ctx.registry.enum_data(id);
                match data.variant(variant_id).kind {
                    hir::VariantKind::Default(eval_id) => {
                        *ctx.registry.variant_eval_mut(eval_id) = hir::Eval::ResolvedError;
                    }
                    hir::VariantKind::Constant(eval_id) => {
                        ctx.registry.const_eval_mut(eval_id).0 = hir::Eval::ResolvedError;
                    }
                }
            }
            ConstDependency::EnumLayout(id) => {
                ctx.registry.enum_data_mut(id).layout = hir::Eval::ResolvedError;
            }
            ConstDependency::StructLayout(id) => {
                ctx.registry.struct_data_mut(id).layout = hir::Eval::ResolvedError;
            }
            ConstDependency::Const(id) => {
                let eval_id = ctx.registry.const_data(id).value;
                ctx.registry.const_eval_mut(eval_id).0 = hir::ConstEval::ResolvedError;
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                if let hir::GlobalInit::Init(eval_id) = data.init {
                    ctx.registry.const_eval_mut(eval_id).0 = hir::ConstEval::ResolvedError;
                }
            }
            ConstDependency::ArrayLen(eval_id) => {
                ctx.registry.const_eval_mut(eval_id).0 = hir::ConstEval::ResolvedError;
            }
            ConstDependency::EnumTypeUsage(id) => {
                ctx.registry.enum_data_mut(id).type_usage = hir::Eval::ResolvedError;
            }
            ConstDependency::StructTypeUsage(id) => {
                ctx.registry.struct_data_mut(id).type_usage = hir::Eval::ResolvedError;
            }
        }
    }
}

fn dependency_cycle_error(
    ctx: &mut HirCtx,
    tree: &Tree,
    node_id: TreeNodeID,
    cycle_id: TreeNodeID,
) {
    let mut ctx_msg = String::with_capacity(32);
    let src = dependency_decs(ctx, tree.node(cycle_id).value, &mut ctx_msg);

    let cycle_deps = tree.values_up_to_node(node_id, cycle_id);
    let mut info_vec = Vec::with_capacity(cycle_deps.len());
    let mut info_src = src;

    for (idx, dep) in cycle_deps.iter().copied().rev().skip(1).enumerate() {
        let first = idx == 0;
        let last = idx + 2 == cycle_deps.len();

        if first {
            ctx_msg.push_str(" depends on ");
            info_src = dependency_decs(ctx, dep, &mut ctx_msg);
            if last {
                ctx_msg.push_str(", completing the cycle...");
            }
        } else {
            let mut msg = String::with_capacity(32);
            msg.push_str("which depends on ");
            let dep_src = dependency_decs(ctx, dep, &mut msg);
            if last {
                msg.push_str(", completing the cycle...");
            }
            info_vec.push(Info::new_val(msg, info_src));
            info_src = dep_src;
        }
    }

    err::const_dependency_cycle(&mut ctx.emit, ctx_msg, src, info_vec);
}

fn dependency_decs(ctx: &HirCtx, dep: ConstDependency, desc: &mut String) -> SourceRange {
    use std::fmt::Write;
    match dep {
        ConstDependency::Root => unreachable!(),
        ConstDependency::VariantTag(id, variant_id) => {
            let data = ctx.registry.enum_data(id);
            let variant = data.variant(variant_id);
            let _ = write!(
                desc,
                "variant tag of `{}.{}`",
                ctx.name(data.name.id),
                ctx.name(variant.name.id)
            );
            SourceRange::new(data.origin_id, variant.name.range)
        }
        ConstDependency::EnumLayout(id) => {
            let data = ctx.registry.enum_data(id);
            let _ = write!(desc, "layout of `{}` enum", ctx.name(data.name.id));
            data.src()
        }
        ConstDependency::StructLayout(id) => {
            let data = ctx.registry.struct_data(id);
            let _ = write!(desc, "layout of `{}` struct", ctx.name(data.name.id));
            data.src()
        }
        ConstDependency::Const(id) => {
            let data = ctx.registry.const_data(id);
            let _ = write!(desc, "value of `{}` constant", ctx.name(data.name.id));
            data.src()
        }
        ConstDependency::Global(id) => {
            let data = ctx.registry.global_data(id);
            let _ = write!(desc, "value of `{}` global", ctx.name(data.name.id));
            data.src()
        }
        ConstDependency::ArrayLen(eval_id) => {
            let (eval, origin_id, _) = *ctx.registry.const_eval(eval_id);
            let _ = write!(desc, "array length value");
            SourceRange::new(origin_id, eval.unresolved_unwrap().0.range)
        }
        ConstDependency::EnumTypeUsage(id) => {
            let data = ctx.registry.enum_data(id);
            let _ = write!(desc, "usage of `{}` enum type", ctx.name(data.name.id));
            data.src()
        }
        ConstDependency::StructTypeUsage(id) => {
            let data = ctx.registry.struct_data(id);
            let _ = write!(desc, "usage of `{}` struct type", ctx.name(data.name.id));
            data.src()
        }
    }
}

macro_rules! unresolved_or_return {
    ($eval:expr, $err:expr) => {
        match $eval {
            hir::Eval::Unresolved(value) => value,
            hir::Eval::Resolved(_) => return Ok(()),
            hir::Eval::ResolvedError => return Err($err),
        }
    };
}

macro_rules! resolved_error_return {
    ($eval:expr, $err:expr) => {
        match $eval {
            hir::Eval::Unresolved(_) => {}
            hir::Eval::Resolved(_) => {}
            hir::Eval::ResolvedError => return Err($err),
        }
    };
}

fn add_dep(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    dep: ConstDependency,
) -> Result<TreeNodeID, TreeNodeID> {
    let node_id = tree.add_child(parent_id, dep);
    if let Some(cycle_id) = tree.find_cycle(node_id) {
        dependency_cycle_error(ctx, tree, node_id, cycle_id);
        Err(parent_id)
    } else {
        Ok(node_id)
    }
}

fn add_dep_allow_cycle(
    tree: &mut Tree,
    parent_id: TreeNodeID,
    dep: ConstDependency,
) -> (TreeNodeID, bool) {
    let node_id = tree.add_child(parent_id, dep);
    (node_id, tree.find_cycle(node_id).is_some())
}

fn add_variant_tag_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    mut parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    mut variant_id: hir::VariantID,
) -> Result<(), TreeNodeID> {
    loop {
        let data = ctx.registry.enum_data(enum_id);
        let variant = data.variant(variant_id);
        let dep = ConstDependency::VariantTag(enum_id, variant_id);

        match variant.kind {
            hir::VariantKind::Default(eval_id) => {
                let eval = *ctx.registry.variant_eval(eval_id);
                unresolved_or_return!(eval, parent_id);
                parent_id = add_dep(ctx, tree, parent_id, dep)?;
            }
            hir::VariantKind::Constant(eval_id) => {
                let (eval, origin_id, scope) = *ctx.registry.const_eval(eval_id);
                let expr = unresolved_or_return!(eval, parent_id);
                let parent_id = add_dep(ctx, tree, parent_id, dep)?;
                return add_expr_deps(ctx, tree, parent_id, origin_id, expr.0);
            }
        };

        if variant_id.index() == 0 {
            return Ok(());
        }
        variant_id = variant_id.dec();
    }
}

fn add_enum_size_deps<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    poly_set: &'hir [hir::Type<'hir>],
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.enum_data(enum_id);
    if data.poly_params.is_none() {
        unresolved_or_return!(data.layout, parent_id);
    } else {
        resolved_error_return!(data.layout, parent_id);
    }

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::EnumLayout(enum_id))?;
    for variant in ctx.registry.enum_data(enum_id).variants {
        for field in variant.fields {
            add_type_size_deps(ctx, tree, parent_id, field.ty, poly_set)?;
        }
    }
    Ok(())
}

fn add_struct_size_deps<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
    poly_set: &'hir [hir::Type<'hir>],
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.struct_data(struct_id);
    if data.poly_params.is_none() {
        unresolved_or_return!(data.layout, parent_id);
    } else {
        resolved_error_return!(data.layout, parent_id);
    }

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::StructLayout(struct_id))?;
    for field in ctx.registry.struct_data(struct_id).fields {
        add_type_size_deps(ctx, tree, parent_id, field.ty, poly_set)?;
    }
    Ok(())
}

fn add_const_var_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    const_id: hir::ConstID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.const_data(const_id);
    let const_ty = data.ty;
    let (eval, origin_id, scope) = *ctx.registry.const_eval(data.value);
    let expr = unresolved_or_return!(eval, parent_id);

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::Const(const_id))?;
    if let Some(const_ty) = const_ty {
        add_type_usage_deps(ctx, tree, parent_id, const_ty)?;
    }
    add_expr_deps(ctx, tree, parent_id, origin_id, expr.0)
}

fn add_global_var_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    global_id: hir::GlobalID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.global_data(global_id);
    let global_ty = data.ty;
    let value = match data.init {
        hir::GlobalInit::Init(eval_id) => eval_id,
        hir::GlobalInit::Zeroed => return Ok(()),
    };
    let (eval, origin_id, scope) = *ctx.registry.const_eval(value);
    let expr = unresolved_or_return!(eval, parent_id);

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::Global(global_id))?;
    add_type_usage_deps(ctx, tree, parent_id, global_ty)?;
    add_expr_deps(ctx, tree, parent_id, origin_id, expr.0)
}

fn add_array_len_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    eval_id: hir::ConstEvalID,
) -> Result<(), TreeNodeID> {
    let (eval, origin_id, scope) = *ctx.registry.const_eval(eval_id);
    let expr = unresolved_or_return!(eval, parent_id);

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::ArrayLen(eval_id))?;
    add_expr_deps(ctx, tree, parent_id, origin_id, expr.0)
}

fn add_variant_usage_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.enum_data(enum_id);
    let variant = data.variant(variant_id);

    add_variant_tag_deps(ctx, tree, parent_id, enum_id, variant_id)?;
    for field in variant.fields {
        add_type_usage_deps(ctx, tree, parent_id, field.ty)?;
    }
    Ok(())
}

fn add_type_size_deps<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    ty: hir::Type<'hir>,
    poly_set: &'hir [hir::Type<'hir>],
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => return Err(parent_id),
        hir::Type::Unknown | hir::Type::Char | hir::Type::Void => {}
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar => {}
        hir::Type::Int(_) | hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => {}
        hir::Type::PolyProc(_, _) => unreachable!(),
        hir::Type::PolyEnum(_, poly_idx) => {
            if !poly_set.is_empty() {
                add_type_size_deps(ctx, tree, parent_id, poly_set[poly_idx], &[])?
            }
        }
        hir::Type::PolyStruct(_, poly_idx) => {
            if !poly_set.is_empty() {
                add_type_size_deps(ctx, tree, parent_id, poly_set[poly_idx], &[])?
            }
        }
        hir::Type::Enum(id, poly_types) => {
            add_enum_size_deps(ctx, tree, parent_id, id, poly_types)?;
        }
        hir::Type::Struct(id, poly_types) => {
            add_struct_size_deps(ctx, tree, parent_id, id, poly_types)?;
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::MultiReference(_, _) => {}
        hir::Type::Procedure(_) => {}
        hir::Type::ArraySlice(_) => {}
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_deps(ctx, tree, parent_id, eval_id)?;
            }
            add_type_size_deps(ctx, tree, parent_id, array.elem_ty, &[])?;
        }
    }
    Ok(())
}

fn add_type_usage_deps<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    ty: hir::Type<'hir>,
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => return Err(parent_id),
        hir::Type::Unknown | hir::Type::Char | hir::Type::Void => {}
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar => {}
        hir::Type::Int(_) | hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => {}
        hir::Type::PolyProc(_, _) | hir::Type::PolyEnum(_, _) | hir::Type::PolyStruct(_, _) => {}
        hir::Type::Enum(id, poly_types) => {
            for ty in poly_types.iter().copied() {
                add_type_usage_deps(ctx, tree, parent_id, ty)?
            }
            let data = ctx.registry.enum_data(id);
            unresolved_or_return!(data.type_usage, parent_id);

            let dep = ConstDependency::EnumTypeUsage(id);
            let (parent_id, cycle) = add_dep_allow_cycle(tree, parent_id, dep);
            if cycle {
                return Ok(());
            }
            for variant in data.variants {
                for field in variant.fields {
                    add_type_usage_deps(ctx, tree, parent_id, field.ty)?;
                }
            }
        }
        hir::Type::Struct(id, poly_types) => {
            for ty in poly_types.iter().copied() {
                add_type_usage_deps(ctx, tree, parent_id, ty)?
            }
            let data = ctx.registry.struct_data(id);
            unresolved_or_return!(data.type_usage, parent_id);

            let dep = ConstDependency::StructTypeUsage(id);
            let (parent_id, cycle) = add_dep_allow_cycle(tree, parent_id, dep);
            if cycle {
                return Ok(());
            }
            for field in data.fields {
                add_type_usage_deps(ctx, tree, parent_id, field.ty)?
            }
        }
        hir::Type::Reference(_, ref_ty) => add_type_usage_deps(ctx, tree, parent_id, *ref_ty)?,
        hir::Type::MultiReference(_, ref_ty) => add_type_usage_deps(ctx, tree, parent_id, *ref_ty)?,
        hir::Type::Procedure(proc_ty) => {
            for param in proc_ty.params {
                add_type_usage_deps(ctx, tree, parent_id, param.ty)?
            }
            add_type_usage_deps(ctx, tree, parent_id, proc_ty.return_ty)?
        }
        hir::Type::ArraySlice(slice) => add_type_usage_deps(ctx, tree, parent_id, slice.elem_ty)?,
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_deps(ctx, tree, parent_id, eval_id)?;
            }
            add_type_usage_deps(ctx, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn add_expr_deps<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    origin_id: ModuleID,
    expr: &ast::Expr<'ast>,
) -> Result<(), TreeNodeID> {
    ctx.scope.origin = origin_id;

    let res = match expr.kind {
        ast::ExprKind::Lit { .. } => Ok(()),
        ast::ExprKind::If { .. } => Err("if"),
        ast::ExprKind::Block { .. } => Err("block"),
        ast::ExprKind::Match { .. } => Err("match"),
        ast::ExprKind::Field { target, .. } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Index { target, index } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            add_expr_deps(ctx, tree, parent_id, origin_id, index)?;
            Ok(())
        }
        ast::ExprKind::Slice { .. } => Err("slice"),
        ast::ExprKind::Call { .. } => Err("procedure call"),
        ast::ExprKind::Cast { target, into } => {
            let into = pass_3::type_resolve(ctx, *into, true);
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            add_type_usage_deps(ctx, tree, parent_id, into)?;
            Ok(())
        }
        ast::ExprKind::Item { path, args_list } => {
            match check_path::path_resolve_value(ctx, path, true) {
                ValueID::None => return Err(parent_id),
                ValueID::Proc(proc_id, poly_types) => {
                    let data = ctx.registry.proc_data(proc_id);
                    if data.flag_set.contains(hir::ProcFlag::Intrinsic) {
                        let name = ctx.name(data.name.id);
                        if let "size_of" | "align_of" = name {
                            if let Some(Some(ty)) = poly_types.map(|p| p.get(0).copied()) {
                                add_type_size_deps(ctx, tree, parent_id, ty, &[])?;
                            }
                        }
                    }

                    for param in ctx.registry.proc_data(proc_id).params {
                        add_type_usage_deps(ctx, tree, parent_id, param.ty)?;
                    }
                    let data = ctx.registry.proc_data(proc_id);
                    add_type_usage_deps(ctx, tree, parent_id, data.return_ty)?;

                    if let Some(poly_types) = poly_types {
                        for ty in poly_types.iter().copied() {
                            add_type_usage_deps(ctx, tree, parent_id, ty)?;
                        }
                    }
                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Enum(enum_id, variant_id, poly_types) => {
                    add_variant_usage_deps(ctx, tree, parent_id, enum_id, variant_id)?;

                    if let Some(poly_types) = poly_types {
                        for ty in poly_types.iter().copied() {
                            add_type_usage_deps(ctx, tree, parent_id, ty)?;
                        }
                    }
                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Const(const_id, _) => {
                    add_const_var_deps(ctx, tree, parent_id, const_id)?;
                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Global(_, _) => Err("global"),
                ValueID::Param(_, _) => Err("parameter"),
                ValueID::Variable(_, _) => Err("variable"),
            }
        }
        ast::ExprKind::Variant { args_list, .. } => {
            if let Some(arg_list) = args_list {
                for arg in arg_list.exprs {
                    add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                }
            }
            Ok(())
        }
        ast::ExprKind::StructInit { struct_init } => {
            if let Some(path) = struct_init.path {
                if let Some((struct_id, poly_types)) =
                    check_path::path_resolve_struct(ctx, path, true)
                {
                    let struct_ty = hir::Type::Struct(struct_id, poly_types.unwrap_or(&[]));
                    add_type_usage_deps(ctx, tree, parent_id, struct_ty)?;
                } else {
                    return Err(parent_id);
                }
            }
            for init in struct_init.input {
                add_expr_deps(ctx, tree, parent_id, origin_id, init.expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayInit { input } => {
            for &expr in input {
                add_expr_deps(ctx, tree, parent_id, origin_id, expr)?;
            }
            Ok(())
        }
        ast::ExprKind::ArrayRepeat { value, len } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, value)?;
            add_expr_deps(ctx, tree, parent_id, origin_id, len.0)?;
            Ok(())
        }
        ast::ExprKind::Deref { .. } => Err("dereference"),
        ast::ExprKind::Address { .. } => Err("address"),
        ast::ExprKind::Unary { rhs, .. } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, lhs)?;
            add_expr_deps(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
    };

    if let Err(expr_kind) = res {
        let src = SourceRange::new(origin_id, expr.range);
        err::const_cannot_use_expr(&mut ctx.emit, src, expr_kind);
        Err(parent_id)
    } else {
        Ok(())
    }
}

fn resolve_dependency_tree(ctx: &mut HirCtx, tree: &Tree) {
    //reverse iteration allows to resolve dependencies in the correct order
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::Root => {}
            ConstDependency::VariantTag(enum_id, variant_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let variant = data.variant(variant_id);
                let tag_ty = data.tag_ty.resolved_unwrap(); //pass_3 sets variants with missing tag_ty to error
                let expect = Expectation::HasType(hir::Type::Int(tag_ty), None);

                match variant.kind {
                    hir::VariantKind::Default(eval_id) => {
                        if variant_id.index() == 0 {
                            let zero = hir::ConstValue::Int { val: 0, neg: false, int_ty: tag_ty };
                            *ctx.registry.variant_eval_mut(eval_id) = hir::Eval::Resolved(zero);
                            continue;
                        }

                        let prev = data.variant(variant_id.dec());
                        let prev_tag = match prev.kind {
                            hir::VariantKind::Default(eval_id) => {
                                ctx.registry.variant_eval(eval_id).resolved()
                            }
                            hir::VariantKind::Constant(eval_id) => {
                                ctx.registry.const_eval(eval_id).0.resolved()
                            }
                        };
                        let value_inc = match prev_tag {
                            Ok(prev) => {
                                let src = SourceRange::new(data.origin_id, variant.name.range);
                                pass_5::int_range_check(ctx, src, prev.into_int() + 1, tag_ty)
                            }
                            Err(()) => Err(()),
                        };
                        *ctx.registry.variant_eval_mut(eval_id) = hir::Eval::from_res(value_inc);
                    }
                    hir::VariantKind::Constant(eval_id) => {
                        resolve_and_update_const_eval(ctx, eval_id, expect);
                    }
                }
            }
            ConstDependency::EnumLayout(id) => {
                if ctx.registry.enum_data(id).poly_params.is_some() {
                    //placeholder for polymorphic type, signals no cycles
                    let layout = hir::Eval::from_res(Ok(hir::Layout::equal(0)));
                    ctx.registry.enum_data_mut(id).layout = layout;
                } else {
                    let layout_res = layout::resolve_enum_layout(ctx, id, &[]);
                    let layout = hir::Eval::from_res(layout_res);
                    ctx.registry.enum_data_mut(id).layout = layout;
                }
            }
            ConstDependency::StructLayout(id) => {
                if ctx.registry.struct_data(id).poly_params.is_some() {
                    //placeholder for polymorphic type, signals no cycles
                    let layout = hir::Eval::from_res(Ok(hir::Layout::equal(0)));
                    ctx.registry.struct_data_mut(id).layout = layout;
                } else {
                    let layout_res = layout::resolve_struct_layout(ctx, id, &[]);
                    let layout = hir::Eval::from_res(layout_res.map(|l| l.total));
                    ctx.registry.struct_data_mut(id).layout = layout;
                }
            }
            ConstDependency::Const(id) => {
                let data = ctx.registry.const_data(id);
                let item = ctx.registry.const_item(id);

                let expect = if let Some(ty) = item.ty {
                    let expect_src = SourceRange::new(data.origin_id, ty.range);
                    Expectation::HasType(data.ty.unwrap(), Some(expect_src)) //unwrap: const has explicit type
                } else {
                    Expectation::None
                };
                if let Some(ty) = resolve_and_update_const_eval(ctx, data.value, expect) {
                    ctx.registry.const_data_mut(id).ty = Some(ty); //expr type takes priority over defined type
                }
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let item = ctx.registry.global_item(id);

                if let hir::GlobalInit::Init(eval_id) = data.init {
                    let expect_src = SourceRange::new(data.origin_id, item.ty.range);
                    let expect = Expectation::HasType(data.ty, Some(expect_src));
                    resolve_and_update_const_eval(ctx, eval_id, expect);
                }
            }
            ConstDependency::ArrayLen(eval_id) => {
                resolve_and_update_const_eval(ctx, eval_id, Expectation::USIZE);
            }
            ConstDependency::EnumTypeUsage(id) => {
                ctx.registry.enum_data_mut(id).type_usage = hir::Eval::Resolved(());
            }
            ConstDependency::StructTypeUsage(id) => {
                ctx.registry.struct_data_mut(id).type_usage = hir::Eval::Resolved(());
            }
        }
    }
}

fn resolve_and_update_const_eval<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    eval_id: hir::ConstEvalID,
    expect: Expectation<'hir>,
) -> Option<hir::Type<'hir>> {
    let (eval, origin_id, scope) = *ctx.registry.const_eval(eval_id);
    let expr = match eval {
        hir::Eval::Unresolved(expr) => expr,
        hir::Eval::Resolved(_) | hir::Eval::ResolvedError => return None,
    };

    let prev_poly = ctx.scope.poly;
    ctx.scope.poly = scope;
    ctx.scope.origin = origin_id;

    let (value_res, value_ty) = resolve_const_expr(ctx, expect, expr);
    let (eval, _, _) = ctx.registry.const_eval_mut(eval_id);
    *eval = hir::Eval::from_res(value_res);

    ctx.scope.poly = prev_poly;
    Some(value_ty)
}

pub fn resolve_const_expr<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    expect: Expectation<'hir>,
    expr: ast::ConstExpr<'ast>,
) -> (Result<hir::ConstValue<'hir>, ()>, hir::Type<'hir>) {
    ctx.in_const = true;
    let error_count = ctx.emit.error_count();
    let expr_res = match expect {
        Expectation::None => pass_5::typecheck_expr_untyped(ctx, expect, expr.0),
        Expectation::HasType(_, _) => pass_5::typecheck_expr(ctx, expect, expr.0),
    };
    ctx.in_const = false;

    if ctx.emit.did_error(error_count) {
        return (Err(()), expr_res.ty);
    }
    match expr_res.expr {
        hir::Expr::Error => return (Err(()), expr_res.ty),
        hir::Expr::Const(value, _) => return (Ok(*value), expr_res.ty),
        hir::Expr::CallDirectPoly { proc_id, input } => {
            let data = ctx.registry.proc_data(*proc_id);
            if data.flag_set.contains(hir::ProcFlag::Intrinsic) {
                let name = ctx.name(data.name.id);
                if let "size_of" | "align_of" = name {
                    let ty = pass_5::type_format(ctx, input.1[0]);
                    let src = ctx.src(expr.0.range);
                    err::tycheck_const_poly_dep(&mut ctx.emit, src, ty.as_str(), name);
                    return (Err(()), expr_res.ty);
                }
            }
        }
        _ => {}
    }
    let src = ctx.src(expr.0.range);
    err::const_cannot_use_expr(&mut ctx.emit, src, "non-constant");
    (Err(()), expr_res.ty)
}

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    Root,
    VariantTag(hir::EnumID, hir::VariantID),
    EnumLayout(hir::EnumID),
    StructLayout(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    ArrayLen(hir::ConstEvalID),
    EnumTypeUsage(hir::EnumID),
    StructTypeUsage(hir::StructID),
}

crate::define_id!(TreeNodeID);
struct Tree {
    nodes: Vec<TreeNode>,
}
struct TreeNode {
    value: ConstDependency,
    parent: Option<TreeNodeID>,
}

impl Tree {
    #[inline(always)]
    fn node(&self, id: TreeNodeID) -> &TreeNode {
        &self.nodes[id.index()]
    }
    #[inline(always)]
    fn root_and_reset(&mut self) -> TreeNodeID {
        self.nodes.clear();
        let id = TreeNodeID::new(self.nodes.len());
        self.nodes.push(TreeNode { value: ConstDependency::Root, parent: None });
        id
    }
    #[inline(always)]
    fn add_child(&mut self, parent_id: TreeNodeID, value: ConstDependency) -> TreeNodeID {
        let id = TreeNodeID::new(self.nodes.len());
        self.nodes.push(TreeNode { value, parent: Some(parent_id) });
        id
    }
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
    fn values_up_to_node(&self, from: TreeNodeID, up_to: TreeNodeID) -> Vec<ConstDependency> {
        let mut node = self.node(from);
        let mut values = vec![node.value];
        if from == up_to {
            return values;
        }
        while let Some(parent_id) = node.parent {
            node = self.node(parent_id);
            values.push(node.value);
            if parent_id == up_to {
                return values;
            }
        }
        values
    }
}

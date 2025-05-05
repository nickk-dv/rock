use super::fold;
use super::layout;
use crate::ast;
use crate::error::{Error, ErrorSink, ErrorWarningBuffer, Info, SourceRange, StringOrStr};
use crate::errors as err;
use crate::hir;
use crate::hir_lower::check_path::{self, ValueID};
use crate::hir_lower::context::HirCtx;
use crate::hir_lower::{pass_3, pass_5, pass_5::Expectation};
use crate::session::ModuleID;
use crate::text::TextRange;

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
        match add_enum_size_deps(ctx, &mut tree, parent_id, enum_id) {
            Ok(()) => resolve_dependency_tree(ctx, &tree),
            Err(from_id) => mark_error_up_to_root(ctx, &tree, from_id),
        }
    }
    for struct_id in ctx.registry.struct_ids() {
        let parent_id = tree.root_and_reset();
        match add_struct_size_deps(ctx, &mut tree, parent_id, struct_id) {
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
    //@assuming that remaining constevals are array len 15.05.24
    // that didnt cycle with anything, thus can be resolved in `immediate mode`
    // this is only true when previos const dependencies and const evals were handled correctly
    //@how will unresolved expressions due to something else erroring earlier behave?
    // for example struct field with some unresolved array len
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
                let variant = data.variant(variant_id);

                match variant.kind {
                    hir::VariantKind::Default(eval_id) => {
                        let eval = ctx.registry.variant_eval_mut(eval_id);
                        *eval = hir::VariantEval::ResolvedError;
                    }
                    hir::VariantKind::Constant(eval_id) => {
                        let (eval, _, _) = ctx.registry.const_eval_mut(eval_id);
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
                let (eval, _, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::Global(id) => {
                let data = ctx.registry.global_data(id);
                let eval_id = match data.init {
                    hir::GlobalInit::Init(eval_id) => eval_id,
                    hir::GlobalInit::Zeroed => unreachable!(),
                };
                let (eval, _, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, _, _) = ctx.registry.const_eval_mut(eval_id);
                *eval = hir::ConstEval::ResolvedError;
            }
        }
    }
}

fn const_dependency_cycle(
    ctx: &mut HirCtx,
    tree: &Tree,
    node_id: TreeNodeID,
    cycle_id: TreeNodeID,
) {
    let src = match tree.node(cycle_id).value {
        ConstDependency::Root => unreachable!(),
        ConstDependency::VariantTag(id, variant_id) => {
            let data = ctx.registry.enum_data(id);
            let variant = data.variant(variant_id);
            SourceRange::new(data.origin_id, variant.name.range)
        }
        ConstDependency::EnumLayout(id) => ctx.registry.enum_data(id).src(),
        ConstDependency::StructLayout(id) => ctx.registry.struct_data(id).src(),
        ConstDependency::Const(id) => ctx.registry.const_data(id).src(),
        ConstDependency::Global(id) => ctx.registry.global_data(id).src(),
        ConstDependency::ArrayLen(eval_id) => {
            let (eval, origin_id, _) = *ctx.registry.const_eval(eval_id);
            SourceRange::new(origin_id, eval.unresolved_unwrap().0.range)
        }
    };

    let cycle_deps = tree.values_up_to_node(node_id, cycle_id);
    let mut ctx_msg: StringOrStr = "".into();
    let mut info_vec = Vec::with_capacity(cycle_deps.len());
    let mut info_src = src;

    for (idx, const_dep) in cycle_deps.iter().copied().rev().skip(1).enumerate() {
        let first = idx == 0;
        let last = idx + 2 == cycle_deps.len();

        let prefix = if first { "" } else { "which " };
        let postfix = if last { ", completing the cycle..." } else { "" };

        let (msg, src) = match const_dep {
            ConstDependency::Root => unreachable!(),
            ConstDependency::VariantTag(id, variant_id) => {
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
                let (eval, origin_id, _) = *ctx.registry.const_eval(eval_id);
                let src = SourceRange::new(origin_id, eval.unresolved_unwrap().0.range);
                let msg = format!("{prefix}depends on array length{postfix}");
                (msg, src)
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

fn add_dep(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    dep: ConstDependency,
) -> Result<TreeNodeID, TreeNodeID> {
    let node_id = tree.add_child(parent_id, dep);
    if let Some(cycle_id) = tree.find_cycle(node_id) {
        const_dependency_cycle(ctx, tree, node_id, cycle_id);
        Err(parent_id)
    } else {
        Ok(node_id)
    }
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

fn add_enum_size_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    enum_id: hir::EnumID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.enum_data(enum_id);
    unresolved_or_return!(data.layout, parent_id);

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::EnumLayout(enum_id))?;
    for variant in ctx.registry.enum_data(enum_id).variants {
        for field in variant.fields {
            add_type_size_deps(ctx, tree, parent_id, field.ty)?;
        }
    }
    Ok(())
}

fn add_struct_size_deps(
    ctx: &mut HirCtx,
    tree: &mut Tree,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
) -> Result<(), TreeNodeID> {
    let data = ctx.registry.struct_data(struct_id);
    unresolved_or_return!(data.layout, parent_id);

    let parent_id = add_dep(ctx, tree, parent_id, ConstDependency::StructLayout(struct_id))?;
    for field in ctx.registry.struct_data(struct_id).fields {
        add_type_size_deps(ctx, tree, parent_id, field.ty)?;
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

//@should type be ignored if init is Zeroed?
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
) -> Result<(), TreeNodeID> {
    match ty {
        hir::Type::Error => return Err(parent_id),
        hir::Type::Unknown | hir::Type::Char | hir::Type::Void => {}
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar => {}
        hir::Type::Int(_) | hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => {}
        hir::Type::PolyProc(_, _) => {}   //@unrechable?
        hir::Type::PolyEnum(_, _) => {}   //@pass poly_types and add deps?
        hir::Type::PolyStruct(_, _) => {} //@pass poly_types and add deps?
        hir::Type::Enum(id, poly_types) => {
            add_enum_size_deps(ctx, tree, parent_id, id)?;
            //@overconstraint: assuming they always affect the size
            for ty in poly_types {
                add_type_size_deps(ctx, tree, parent_id, *ty)?;
            }
        }
        hir::Type::Struct(id, poly_types) => {
            add_struct_size_deps(ctx, tree, parent_id, id)?;
            //@overconstraint: assuming they always affect the size
            for ty in poly_types {
                add_type_size_deps(ctx, tree, parent_id, *ty)?;
            }
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::MultiReference(_, _) => {}
        hir::Type::Procedure(_) => {}
        hir::Type::ArraySlice(_) => {}
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                add_array_len_deps(ctx, tree, parent_id, eval_id)?;
            }
            add_type_size_deps(ctx, tree, parent_id, array.elem_ty)?;
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
        hir::Type::PolyProc(_, _) | hir::Type::PolyEnum(_, _) | hir::Type::PolyStruct(_, _) => {
            eprintln!("unhandled poly param in (add_type_usage_const_dependencies)");
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
                    add_type_usage_deps(ctx, tree, parent_id, field.ty)?;
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
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Index { target, index } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            add_expr_deps(ctx, tree, parent_id, origin_id, index)?;
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
            add_expr_deps(ctx, tree, parent_id, origin_id, target)?;
            Ok(())
        }
        ast::ExprKind::Builtin { builtin } => match builtin {
            ast::Builtin::Error(name) => {
                let name = ctx.name(name.id);
                let src = SourceRange::new(origin_id, expr.range);
                err::tycheck_builtin_unknown(&mut ctx.emit, src, name);
                Err(parent_id)
            }
            ast::Builtin::SizeOf(ty) => {
                let ty = pass_3::type_resolve(ctx, *ty, true); //@in definition?
                if pass_5::type_has_poly_param_layout_dep(ty) {
                    let ty = pass_5::type_format(ctx, ty);
                    let src = SourceRange::new(origin_id, expr.range);
                    err::tycheck_const_poly_dep(&mut ctx.emit, src, ty.as_str(), "size_of");
                    Err(parent_id)
                } else {
                    add_type_size_deps(ctx, tree, parent_id, ty)
                }
            }
            ast::Builtin::AlignOf(ty) => {
                let ty = pass_3::type_resolve(ctx, *ty, true); //@in definition?
                if pass_5::type_has_poly_param_layout_dep(ty) {
                    let ty = pass_5::type_format(ctx, ty);
                    let src = SourceRange::new(origin_id, expr.range);
                    err::tycheck_const_poly_dep(&mut ctx.emit, src, ty.as_str(), "align_of");
                    Err(parent_id)
                } else {
                    add_type_size_deps(ctx, tree, parent_id, ty)
                }
            }
            ast::Builtin::Transmute(_, _) => {
                error_cannot_use_in_constants(
                    &mut ctx.emit,
                    origin_id,
                    expr.range,
                    "builtin @transmute",
                );
                Err(parent_id)
            }
        },
        ast::ExprKind::Item { path, args_list } => {
            match check_path::path_resolve_value(ctx, path, true) {
                ValueID::None => Err(parent_id),
                ValueID::Proc(proc_id, poly_types) => {
                    //@borrowing hacks, just get data once here
                    // change the result Err type with delayed mutation of HirData only at top lvl?
                    for param in ctx.registry.proc_data(proc_id).params {
                        add_type_usage_deps(ctx, tree, parent_id, param.ty)?;
                    }
                    let data = ctx.registry.proc_data(proc_id);
                    add_type_usage_deps(ctx, tree, parent_id, data.return_ty)?;

                    if let Some(arg_list) = args_list {
                        for arg in arg_list.exprs {
                            add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                        }
                    }
                    Ok(())
                }
                ValueID::Enum(enum_id, variant_id, poly_types) => {
                    add_variant_usage_deps(ctx, tree, parent_id, enum_id, variant_id)?;
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
                    add_expr_deps(ctx, tree, parent_id, origin_id, arg)?;
                }
            }
            Ok(())
        }
        ast::ExprKind::StructInit { struct_init } => {
            //@make sure dependency order is correct for typecheck to work
            // both with & without known struct type in the struct_init
            if let Some(path) = struct_init.path {
                if let Some((struct_id, poly_types)) =
                    check_path::path_resolve_struct(ctx, path, true)
                {
                    //@temp hack using empty poly_types if missing, is it correct?
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
        ast::ExprKind::Deref { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "deref");
            Err(parent_id)
        }
        ast::ExprKind::Address { .. } => {
            error_cannot_use_in_constants(&mut ctx.emit, origin_id, expr.range, "address");
            Err(parent_id)
        }
        ast::ExprKind::Unary { rhs, .. } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
        ast::ExprKind::Binary { lhs, rhs, .. } => {
            add_expr_deps(ctx, tree, parent_id, origin_id, lhs)?;
            add_expr_deps(ctx, tree, parent_id, origin_id, rhs)?;
            Ok(())
        }
    }
}

pub fn error_cannot_use_in_constants(
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

fn resolve_dependency_tree(ctx: &mut HirCtx, tree: &Tree) {
    // reverse iteration allows to resolve dependencies in correct order
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::Root => {}
            ConstDependency::VariantTag(enum_id, variant_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let variant = data.variant(variant_id);

                let tag_ty = data.tag_ty.resolved_unwrap(); // if tag_ty not resolved, variant set to error in pass3
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
                                    let (eval, _, _) = ctx.registry.const_eval(eval_id);
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
                if ctx.registry.enum_data(id).poly_params.is_some() {
                    //@hack, placeholder layout for polymorphic type
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
                    //@hack, placeholder layout for polymorphic type
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
                    Expectation::HasType(data.ty.unwrap(), Some(expect_src)) //unwrap, item and data are both typed
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
    let (eval, origin_id, scope) = *ctx.registry.const_eval(eval_id);
    let expr = match eval {
        hir::ConstEval::Unresolved(expr) => expr,
        hir::ConstEval::Resolved(_) => return None,
        hir::ConstEval::ResolvedError => return None,
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
        Expectation::HasType(_, _) => pass_5::typecheck_expr(ctx, expect, expr.0),
        _ => pass_5::typecheck_expr_untyped(ctx, expect, expr.0),
    };
    ctx.in_const = false;

    if !ctx.emit.did_error(error_count) {
        match expr_res.expr {
            hir::Expr::Error => (Err(()), expr_res.ty),
            hir::Expr::Const { value } => (Ok(*value), expr_res.ty),
            _ => {
                error_cannot_use_in_constants(
                    &mut ctx.emit,
                    ctx.scope.origin,
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

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    Root,
    VariantTag(hir::EnumID, hir::VariantID),
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

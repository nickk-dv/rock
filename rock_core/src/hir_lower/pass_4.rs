use super::hir_build::{HirData, HirEmit};
use super::pass_5::{self, TypeExpectation};
use crate::ast::{self, BasicType};
use crate::error::ErrorComp;
use crate::hir;

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    EnumVariant(hir::EnumID, hir::EnumVariantID),
    UnionSize(hir::UnionID),
    StructSize(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    ArrayLen(hir::ConstEvalID),
}

pub fn resolve_const_dependencies<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().enum_ids() {
        let data = hir.registry().enum_data(id);

        for (idx, variant) in data.variants.iter().enumerate() {
            let (eval, _) = hir.registry().const_eval(variant.value);
            let variant_id = hir::EnumVariantID::new(idx);

            if matches!(eval, hir::ConstEval::Unresolved(_)) {
                let (mut tree, root_id) =
                    Tree::new_rooted(ConstDependency::EnumVariant(id, variant_id));
                // check dependencies
                resolve_const_dependency_tree(hir, emit, &tree);
            }
        }
    }

    for id in hir.registry().union_ids() {
        let eval = hir.registry().union_data(id).size_eval;

        if matches!(eval, hir::SizeEval::Unresolved) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::UnionSize(id));
            if check_union_size_const_dependency(hir, emit, &mut tree, root_id, id).is_ok() {
                resolve_const_dependency_tree(hir, emit, &tree);
            }
        }
    }

    for id in hir.registry().struct_ids() {
        let eval = hir.registry().struct_data(id).size_eval;

        if matches!(eval, hir::SizeEval::Unresolved) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::StructSize(id));
            if check_struct_size_const_dependency(hir, emit, &mut tree, root_id, id).is_ok() {
                resolve_const_dependency_tree(hir, emit, &tree);
            }
        }
    }

    for id in hir.registry().const_ids() {
        let data = hir.registry().const_data(id);
        let (eval, _) = hir.registry().const_eval(data.value);

        if matches!(eval, hir::ConstEval::Unresolved(_)) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Const(id));
            // check dependencies
            resolve_const_dependency_tree(hir, emit, &tree);
        }
    }

    for id in hir.registry().global_ids() {
        let data = hir.registry().global_data(id);
        let (eval, _) = hir.registry().const_eval(data.value);

        if matches!(eval, hir::ConstEval::Unresolved(_)) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::Global(id));
            // check dependencies
            resolve_const_dependency_tree(hir, emit, &tree);
        }
    }

    //@assuming that remaining constevals are array len 15.05.24
    // that didnt cycle with anything, thus can be resolved in `immediate mode`
    // this is only true when previos const dependencies and const evals were handled correctly
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

#[derive(Copy, Clone, PartialEq, Debug)]
struct TreeNodeID(u32);

struct TreeNode<T: PartialEq + Copy + Clone> {
    value: T,
    parent: Option<TreeNodeID>,
}

impl TreeNodeID {
    const fn new(index: usize) -> TreeNodeID {
        TreeNodeID(index as u32)
    }
    const fn index(self) -> usize {
        self.0 as usize
    }
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
        assert_ne!(from_id, up_to);
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

//@improve messaging by using Info vectors @13.05.24
// will require to allow errors to have multiple Info contexts
// also use: "depends on:" "which depends on:" "... , completing the cycle"
//@opt reduce vector allocation for cycles? (rarely happens) @13.05.24
fn check_const_dependency_cycle(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &Tree<ConstDependency>,
    parent_id: TreeNodeID,
    node_id: TreeNodeID,
) -> Result<(), ()> {
    let cycle_id = match tree.find_cycle(node_id) {
        Some(cycle_id) => cycle_id,
        None => return Ok(()),
    };

    let cycle_deps = tree.get_values_up_to_node(node_id, cycle_id);
    let mut message = String::from("constant dependency cycle found: \n");

    for const_dep in cycle_deps.iter().cloned().rev() {
        match const_dep {
            ConstDependency::EnumVariant(id, variant_id) => {
                let data = hir.registry().enum_data(id);
                let variant = data.variant(variant_id);
                message.push_str(&format!(
                    "`{}.{}` -> ",
                    hir.name_str(data.name.id),
                    hir.name_str(variant.name.id)
                ));
            }
            ConstDependency::UnionSize(id) => {
                let data = hir.registry().union_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry().struct_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
            ConstDependency::ArrayLen(eval_id) => {
                //@should be info instead with expression source 15.05.24
                message.push_str("`array len <expr>` -> ");
            }
        }
    }
    message.push_str("completing the cycle");

    let src = match tree.get_node(cycle_id).value {
        ConstDependency::EnumVariant(id, variant_id) => {
            let data = hir.registry().enum_data(id);
            let variant = data.variant(variant_id);
            hir.src(data.origin_id, variant.name.range)
        }
        ConstDependency::UnionSize(id) => {
            let data = hir.registry().union_data(id);
            hir.src(data.origin_id, data.name.range)
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
                // access to range information is behind consteval the state
                // just always store SourceRange instead? 15.05.24
                panic!("array len consteval range not available");
            }
        }
    };

    // marking after message was finished to prevent panic! in ConstDependency::ArrayLen 15.05.24
    const_dependencies_mark_error_up_to_root(hir, tree, parent_id);

    emit.error(ErrorComp::new(message, src, None));
    Err(())
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
                *eval = hir::ConstEval::Error;
            }
            ConstDependency::UnionSize(id) => {
                let data = hir.registry_mut().union_data_mut(id);
                data.size_eval = hir::SizeEval::ResolvedError;
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry_mut().struct_data_mut(id);
                data.size_eval = hir::SizeEval::ResolvedError;
            }
            ConstDependency::Const(id) => {
                let data = hir.registry().const_data(id);
                let eval_id = data.value;
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::Error;
            }
            ConstDependency::Global(id) => {
                let data = hir.registry().global_data(id);
                let eval_id = data.value;
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::Error;
            }
            ConstDependency::ArrayLen(eval_id) => {
                let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
                *eval = hir::ConstEval::Error;
            }
        }
    }
}

// currently only cycles cause making as Error up to root and returning @01.05.24
// if dep being added to a tree is already an Error
// it might be worth doing marking and still looking for cycles?
// knowing that something is an Error can save processing time on constants up the tree
// which would eventually be resolved to same Error
// since they depend on constant which is already known to be Error

//@make a function to add const depepencies and check cycles for any ConstDependency ? @12.05.24
// instead of doing per type duplication?
fn check_union_size_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    union_id: hir::UnionID,
) -> Result<(), ()> {
    let data = hir.registry().union_data(union_id);
    match data.size_eval {
        hir::SizeEval::ResolvedError => {
            const_dependencies_mark_error_up_to_root(hir, tree, parent_id);
            Err(()) //@potentially return Ok without marking? more coverage if top level will resolve even with some Errors in that tree?
        }
        hir::SizeEval::Resolved(_) => Ok(()),
        hir::SizeEval::Unresolved => {
            for member in data.members {
                check_type_size_const_dependency(hir, emit, tree, parent_id, member.ty)?;
            }
            Ok(())
        }
    }
}

fn check_struct_size_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
) -> Result<(), ()> {
    let data = hir.registry().struct_data(struct_id);
    match data.size_eval {
        hir::SizeEval::ResolvedError => {
            const_dependencies_mark_error_up_to_root(hir, tree, parent_id);
            Err(()) //@potentially return Ok without marking? more coverage if top level will resolve even with some Errors in that tree?
        }
        hir::SizeEval::Resolved(_) => Ok(()),
        hir::SizeEval::Unresolved => {
            for field in data.fields {
                check_type_size_const_dependency(hir, emit, tree, parent_id, field.ty)?;
            }
            Ok(())
        }
    }
}

fn check_type_size_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    ty: hir::Type,
) -> Result<(), ()> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(_) => {}
        hir::Type::Union(id) => {
            let node_id: TreeNodeID = tree.add_child(parent_id, ConstDependency::UnionSize(id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;
            check_union_size_const_dependency(hir, emit, tree, node_id, id)?;
        }
        hir::Type::Struct(id) => {
            let node_id = tree.add_child(parent_id, ConstDependency::StructSize(id));
            check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;
            check_struct_size_const_dependency(hir, emit, tree, node_id, id)?;
        }
        hir::Type::Reference(_, _) => {}
        hir::Type::Procedure(_) => {}
        hir::Type::ArraySlice(_) => {}
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                let node_id = tree.add_child(parent_id, ConstDependency::ArrayLen(eval_id));
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;
            }
            check_type_size_const_dependency(hir, emit, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
}

fn check_type_usage_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    ty: hir::Type,
) -> Result<(), ()> {
    match ty {
        hir::Type::Error => {}
        hir::Type::Basic(_) => {}
        hir::Type::Enum(_) => {}
        hir::Type::Union(id) => {
            let data = hir.registry().union_data(id);
            for member in data.members {
                check_type_usage_const_dependency(hir, emit, tree, parent_id, member.ty)?
            }
        }
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            for field in data.fields {
                check_type_usage_const_dependency(hir, emit, tree, parent_id, field.ty)?
            }
        }
        hir::Type::Reference(ref_ty, _) => {
            check_type_usage_const_dependency(hir, emit, tree, parent_id, *ref_ty)?
        }
        hir::Type::Procedure(proc_ty) => {
            for param in proc_ty.params {
                check_type_usage_const_dependency(hir, emit, tree, parent_id, *param)?
            }
            check_type_usage_const_dependency(hir, emit, tree, parent_id, proc_ty.return_ty)?
        }
        hir::Type::ArraySlice(slice) => {
            check_type_usage_const_dependency(hir, emit, tree, parent_id, slice.elem_ty)?;
        }
        hir::Type::ArrayStatic(array) => {
            if let hir::ArrayStaticLen::ConstEval(eval_id) = array.len {
                let node_id = tree.add_child(parent_id, ConstDependency::ArrayLen(eval_id));
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id)?;
            }
            check_type_usage_const_dependency(hir, emit, tree, parent_id, array.elem_ty)?;
        }
    }
    Ok(())
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
            ConstDependency::UnionSize(id) => {
                let size_eval = resolve_union_size(hir, emit, id);
                hir.registry_mut().union_data_mut(id).size_eval = size_eval;
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
    let (_, value_id) = match eval {
        hir::ConstEval::Unresolved(expr) => resolve_const_expr(hir, emit, origin_id, expect, expr),
        _ => panic!("calling `resolve_const_expr` on already resolved expr"),
    };
    let (eval, _) = hir.registry_mut().const_eval_mut(eval_id);
    *eval = hir::ConstEval::ResolvedValue(value_id);
}

//@remove asserts later on when compiler is stable? 02.05.24
fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

fn resolve_union_size(hir: &HirData, emit: &mut HirEmit, union_id: hir::UnionID) -> hir::SizeEval {
    let data = hir.registry().union_data(union_id);
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for member in data.members {
        let (member_size, member_align) = match pass_5::type_size(
            hir,
            emit,
            member.ty,
            hir.src(data.origin_id, member.name.range), //@review source range for this type_size error 10.05.24
        ) {
            Some(size) => (size.size(), size.align()),
            None => return hir::SizeEval::ResolvedError,
        };
        size = size.max(member_size);
        align = align.max(member_align);
    }

    hir::SizeEval::Resolved(hir::Size::new(size, align))
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

//@check int, float value range constraints 14.05.24
// same for typecheck_int_lit etc, regular expressions checking
// should later be merged with this constant resolution / folding flow
//@return type and integrate this with normal expr typecheck?
#[must_use]
pub fn resolve_const_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    expect: TypeExpectation<'hir>,
    expr: ast::ConstExpr,
) -> (hir::ConstValue<'hir>, hir::ConstValueID) {
    let result: Result<(hir::ConstValue, hir::Type), &'static str> = match expr.0.kind {
        ast::ExprKind::LitNull => {
            //@coersion of rawptr `null` should be explicit via cast expression 14.05.24
            // to make intention clear, same for conversion back to untyped rawptr
            Ok((hir::ConstValue::Null, hir::Type::Basic(BasicType::Rawptr)))
        }
        ast::ExprKind::LitBool { val } => Ok((
            hir::ConstValue::Bool { val },
            hir::Type::Basic(BasicType::Bool),
        )),
        ast::ExprKind::LitInt { val } => {
            let int_ty = pass_5::coerce_int_type(expect.ty);
            let value = hir::ConstValue::Int {
                val,
                neg: false,
                ty: Some(int_ty),
            };
            Ok((value, hir::Type::Basic(int_ty)))
        }
        ast::ExprKind::LitFloat { val } => {
            let float_ty = pass_5::coerce_float_type(expect.ty);
            let value = hir::ConstValue::Float {
                val,
                ty: Some(float_ty),
            };
            Ok((value, hir::Type::Basic(float_ty)))
        }
        ast::ExprKind::LitChar { val } => Ok((
            hir::ConstValue::Char { val },
            hir::Type::Basic(BasicType::Char),
        )),
        ast::ExprKind::LitString { id, c_string } => {
            let string_ty = pass_5::alloc_string_lit_type(emit, c_string);
            Ok((hir::ConstValue::String { id, c_string }, string_ty))
        }
        ast::ExprKind::If { .. } => Err("if"),
        ast::ExprKind::Block { .. } => Err("block"),
        ast::ExprKind::Match { .. } => Err("match"),
        ast::ExprKind::Field { .. } => Err("field"),
        ast::ExprKind::Index { .. } => Err("index"),
        ast::ExprKind::Slice { .. } => Err("slice"),
        ast::ExprKind::Call { .. } => Err("procedure call"),
        ast::ExprKind::Cast { .. } => Err("cast"),
        ast::ExprKind::Sizeof { .. } => Err("sizeof"),
        ast::ExprKind::Item { .. } => Err("item"),
        ast::ExprKind::StructInit { .. } => Err("structure initializer"),
        ast::ExprKind::ArrayInit { .. } => Err("array initializer"),
        ast::ExprKind::ArrayRepeat { .. } => Err("array repeat"),
        ast::ExprKind::Address { .. } => Err("address"),
        ast::ExprKind::Unary { .. } => Err("unary"),
        ast::ExprKind::Binary { .. } => Err("binary"),
    };

    match result {
        Ok((value, value_ty)) => {
            pass_5::check_type_expectation(hir, emit, origin_id, expr.0.range, expect, value_ty);
            (value, emit.const_intern.intern(value))
        }
        Err(expr_name) => {
            emit.error(ErrorComp::new(
                format!("cannot use `{expr_name}` expression in constants"),
                hir.src(origin_id, expr.0.range),
                None,
            ));
            let value = hir::ConstValue::Error;
            (value, emit.const_intern.intern(value))
        }
    }
}

pub fn fold_const_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    expr: &'hir hir::Expr<'hir>,
) -> (hir::ConstValue<'hir>, hir::ConstValueID) {
    let result = match *expr {
        hir::Expr::Error => Ok(hir::ConstValue::Error),
        hir::Expr::Const { value } => Ok(value),
        hir::Expr::If { .. } => Err("if"),
        hir::Expr::Block { .. } => Err("block"),
        hir::Expr::Match { .. } => Err("match"),
        hir::Expr::UnionMember {
            target,
            union_id,
            member_id,
            deref,
        } => todo!(),
        hir::Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => todo!(),
        hir::Expr::SliceField {
            target,
            first_ptr,
            deref,
        } => todo!(),
        hir::Expr::Index { target, access } => {
            let (target_value, _) = fold_const_expr(hir, emit, origin_id, target);
            let (index_value, _) = fold_const_expr(hir, emit, origin_id, access.index);

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
                            Ok(hir::ConstValue::Error)
                        } else {
                            Ok(emit.const_intern.get(array.values[index as usize]))
                        }
                    }
                    hir::ConstValue::ArrayRepeat { len, value } => {
                        if index >= len {
                            //@no source range available
                            Ok(hir::ConstValue::Error)
                        } else {
                            Ok(emit.const_intern.get(value))
                        }
                    }
                    _ => Ok(hir::ConstValue::Error),
                }
            } else {
                Ok(hir::ConstValue::Error)
            }
        }
        hir::Expr::Slice { target, access } => todo!(),
        hir::Expr::Cast { target, into, kind } => todo!(),
        hir::Expr::LocalVar { local_id } => todo!(),
        hir::Expr::ParamVar { param_id } => todo!(),
        hir::Expr::ConstVar { const_id } => todo!(),
        hir::Expr::GlobalVar { global_id } => todo!(),
        hir::Expr::Procedure { proc_id } => todo!(),
        hir::Expr::CallDirect { proc_id, input } => todo!(),
        hir::Expr::CallIndirect { target, indirect } => todo!(),
        hir::Expr::UnionInit { union_id, input } => todo!(),
        hir::Expr::StructInit { struct_id, input } => todo!(),
        hir::Expr::ArrayInit { array_init } => {
            let mut value_ids = Vec::with_capacity(array_init.input.len());
            for &input in array_init.input {
                let (_, value_id) = fold_const_expr(hir, emit, origin_id, input);
                value_ids.push(value_id);
            }

            let values = emit.const_intern.arena().alloc_slice(value_ids.as_slice());
            let array = emit.const_intern.arena().alloc(hir::ConstArray {
                len: values.len() as u64,
                values,
            });
            Ok(hir::ConstValue::Array { array })
        }
        hir::Expr::ArrayRepeat { array_repeat } => {
            if let Some(len) = array_repeat.len {
                let (_, value_id) = fold_const_expr(hir, emit, origin_id, array_repeat.expr);
                Ok(hir::ConstValue::ArrayRepeat {
                    len,
                    value: value_id,
                })
            } else {
                Ok(hir::ConstValue::Error)
            }
        }
        hir::Expr::Address { .. } => Err("address"),
        hir::Expr::Unary { op, rhs } => todo!(),
        hir::Expr::Binary {
            op,
            lhs,
            rhs,
            lhs_signed_int,
        } => todo!(),
    };

    match result {
        Ok(value) => (value, emit.const_intern.intern(value)),
        Err(expr_name) => {
            //@range not available
            //emit.error(ErrorComp::new(
            //    format!("cannot use `{expr_name}` expression in constants"),
            //    hir.src(origin_id, expr.0.range),
            //    None,
            //));
            let value = hir::ConstValue::Error;
            (value, emit.const_intern.intern(value))
        }
    }
}

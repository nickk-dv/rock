use super::hir_build::{HirData, HirEmit, SymbolKind};
use super::proc_scope::{ProcScope, VariableID};
use crate::ast::{self, BasicType};
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::{TextOffset, TextRange};

// @enum cycle example from rust @02.05.24
// variants without initializers depend on previous variants
// in case first one its known to be 0 if not specified

/* C (EnumTest::D) -> D (EnumTest::C + 1) ->
#[repr(isize)]
enum EnumTest {
    A,
    B = 2,
    C = EnumTest::D as isize,
    D,
    V,
}
*/

#[derive(Copy, Clone, PartialEq)]
enum ConstDependency {
    EnumVariant(hir::EnumID, hir::EnumVariantID),
    UnionSize(hir::UnionID),
    StructSize(hir::StructID),
}

struct Tree<T: PartialEq + Copy + Clone> {
    nodes: Vec<TreeNode<T>>,
}

//@remove debug when stable (using it for asserts) @30.04.24
#[derive(Copy, Clone, PartialEq, Debug)]
struct TreeNodeID(u32);

struct TreeNode<T: PartialEq + Copy + Clone> {
    value: T,
    parent: Option<TreeNodeID>,
    first_child: Option<TreeNodeID>,
    last_child: Option<TreeNodeID>,
}

impl<T: PartialEq + Copy + Clone> Tree<T> {
    fn new_rooted(root: T) -> (Tree<T>, TreeNodeID) {
        let root_id = TreeNodeID(0);
        let tree = Tree {
            nodes: vec![TreeNode {
                value: root,
                parent: None,
                first_child: None,
                last_child: None,
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
            first_child: None,
            last_child: None,
        });

        let parent = self.get_node_mut(parent_id);
        if parent.first_child.is_none() {
            parent.first_child = Some(id);
        }
        parent.last_child = Some(id);
        id
    }

    #[must_use]
    fn find_cycle(&self, id: TreeNodeID, value: T) -> Option<TreeNodeID> {
        let mut node = self.get_node(id);

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            if node.value == value {
                return Some(parent_id);
            }
        }
        None
    }

    fn get_parents_up_to_node(&self, from_id: TreeNodeID, up_to: TreeNodeID) -> Vec<T> {
        assert_ne!(from_id, up_to);
        let mut node = self.get_node(from_id);
        let mut parents = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            parents.push(node.value);
            if parent_id == up_to {
                return parents;
            }
        }

        parents
    }

    fn get_parents_up_to_root(&self, from_id: TreeNodeID) -> Vec<T> {
        let mut node = self.get_node(from_id);
        let mut parents = vec![node.value];

        while let Some(parent_id) = node.parent {
            node = self.get_node(parent_id);
            parents.push(node.value);
        }

        parents
    }

    fn get_node(&self, id: TreeNodeID) -> &TreeNode<T> {
        &self.nodes[id.index()]
    }
    fn get_node_mut(&mut self, id: TreeNodeID) -> &mut TreeNode<T> {
        &mut self.nodes[id.index()]
    }
}

impl TreeNodeID {
    const fn new(index: usize) -> TreeNodeID {
        TreeNodeID(index as u32)
    }
    const fn index(self) -> usize {
        self.0 as usize
    }
}

fn resolve_const_dependencies(hir: &mut HirData, emit: &mut HirEmit) {
    for id in hir.registry().enum_ids() {
        let data = hir.registry().enum_data(id);

        for variant in data.variants {
            //@check & resolve
        }
    }

    for id in hir.registry().union_ids() {
        let data = hir.registry().union_data(id);

        if matches!(data.size_eval, hir::SizeEval::Unresolved) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::UnionSize(id));
            if check_union_size_const_dependency(hir, emit, &mut tree, root_id, id).is_ok() {
                resolve_const_dependency_tree(hir, &tree);
            }
        }
    }

    for id in hir.registry().struct_ids() {
        let data = hir.registry().struct_data(id);

        if matches!(data.size_eval, hir::SizeEval::Unresolved) {
            let (mut tree, root_id) = Tree::new_rooted(ConstDependency::StructSize(id));
            if check_struct_size_const_dependency(hir, emit, &mut tree, root_id, id).is_ok() {
                resolve_const_dependency_tree(hir, &tree);
            }
        }
    }

    for id in hir.registry().const_ids() {
        let data = hir.registry().const_data(id);
        //@check & resolve
    }

    for id in hir.registry().global_ids() {
        let data = hir.registry().global_data(id);
        //@check & resolve
    }
}

// is rev order stable when dealing with recursive tree inputs? seems like it @01.05.24
fn resolve_const_dependency_tree(hir: &mut HirData, tree: &Tree<ConstDependency>) {
    for node in tree.nodes.iter().rev() {
        match node.value {
            ConstDependency::EnumVariant(_, _) => {
                todo!("EnumVariant const resolve not implemented")
            }
            ConstDependency::UnionSize(id) => {
                let size_eval = resolve_union_size(hir, id);
                hir.registry_mut().union_data_mut(id).size_eval = size_eval;
            }
            ConstDependency::StructSize(id) => {
                let size_eval = resolve_struct_size(hir, id);
                hir.registry_mut().struct_data_mut(id).size_eval = size_eval;
            }
        }
    }
}

//@remove asserts later on when compiler is stable? 02.05.24
fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

fn resolve_union_size(hir: &HirData, union_id: hir::UnionID) -> hir::SizeEval {
    let data = hir.registry().union_data(union_id);
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for member in data.members {
        let (member_size, member_align) = match type_size(hir, member.ty) {
            Some(size) => (size.size(), size.align()),
            None => return hir::SizeEval::Error,
        };
        size = size.max(member_size);
        align = align.max(member_align);
    }

    hir::SizeEval::Resolved(hir::Size::new(size, align))
}

fn resolve_struct_size(hir: &HirData, struct_id: hir::StructID) -> hir::SizeEval {
    let data = hir.registry().struct_data(struct_id);
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for field in data.fields {
        let (field_size, field_align) = match type_size(hir, field.ty) {
            Some(size) => (size.size(), size.align()),
            None => return hir::SizeEval::Error,
        };
        size = aligned_size(size, field_align);
        size += field_size;
        align = align.max(field_align);
    }

    size = aligned_size(size, align);
    hir::SizeEval::Resolved(hir::Size::new(size, align))
}

fn check_const_dependency_cycle(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &Tree<ConstDependency>,
    parent_id: TreeNodeID,
    node_id: TreeNodeID,
    dep: ConstDependency,
    src: SourceRange,
) -> Result<(), ()> {
    let cycle_id = match tree.find_cycle(node_id, dep) {
        Some(cycle_id) => cycle_id,
        None => return Ok(()),
    };

    //@better alternative would be to have iterators for this purpose @30.04.24
    // without any need to get vectors of links twice (they partially overlap)
    let cycle_deps = tree.get_parents_up_to_node(node_id, cycle_id);
    let parents_deps = tree.get_parents_up_to_root(parent_id);

    // create error message up to cycle
    let mut message = String::from("constant dependency cycle found: \n");
    for const_dep in cycle_deps.iter().cloned().rev() {
        match const_dep {
            ConstDependency::EnumVariant(_, _) => {
                panic!("EnumVariant const dependency is not supported")
            }
            ConstDependency::UnionSize(id) => {
                let data = hir.registry().union_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry().struct_data(id);
                message.push_str(&format!("`{}` -> ", hir.name_str(data.name.id)));
            }
        }
    }

    // mark as error up to root
    for const_dep in parents_deps {
        match const_dep {
            ConstDependency::EnumVariant(_, _) => {
                panic!("EnumVariant const dependency is not supported")
            }
            ConstDependency::UnionSize(id) => {
                let data = hir.registry_mut().union_data_mut(id);
                data.size_eval = hir::SizeEval::Error;
            }
            ConstDependency::StructSize(id) => {
                let data = hir.registry_mut().struct_data_mut(id);
                data.size_eval = hir::SizeEval::Error;
            }
        }
    }

    emit.error(ErrorComp::error(message, src, None));
    Err(())
}

// currently only cycles cause making as Error up to root and returning @01.05.24
// if dep being added to a tree is already an Error
// it might be worth doing marking and still looking for cycles?
// knowing that something is an Error can save processing time on constants up the tree
// which would eventually be resolved to same Error
// since they depend on constant which is already known to be Error
fn check_union_size_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    union_id: hir::UnionID,
) -> Result<(), ()> {
    for member in hir.registry().union_data(union_id).members {
        match member.ty {
            hir::Type::Union(id) => {
                let dep = ConstDependency::UnionSize(id);
                let node_id = tree.add_child(parent_id, dep);
                let src = hir.src(
                    hir.registry().union_data(union_id).origin_id,
                    member.name.range,
                );
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id, dep, src)?;
                check_union_size_const_dependency(hir, emit, tree, node_id, id)?;
            }
            hir::Type::Struct(id) => {
                let dep = ConstDependency::StructSize(id);
                let node_id = tree.add_child(parent_id, dep);
                let src = hir.src(
                    hir.registry().union_data(union_id).origin_id,
                    member.name.range,
                );
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id, dep, src)?;
                check_struct_size_const_dependency(hir, emit, tree, node_id, id)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_struct_size_const_dependency(
    hir: &mut HirData,
    emit: &mut HirEmit,
    tree: &mut Tree<ConstDependency>,
    parent_id: TreeNodeID,
    struct_id: hir::StructID,
) -> Result<(), ()> {
    for field in hir.registry().struct_data(struct_id).fields {
        match field.ty {
            hir::Type::Union(id) => {
                let dep = ConstDependency::UnionSize(id);
                let node_id = tree.add_child(parent_id, dep);
                let src = hir.src(
                    hir.registry().struct_data(struct_id).origin_id,
                    field.name.range,
                );
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id, dep, src)?;
                check_union_size_const_dependency(hir, emit, tree, node_id, id)?;
            }
            hir::Type::Struct(id) => {
                let dep = ConstDependency::StructSize(id);
                let node_id = tree.add_child(parent_id, dep);
                let src = hir.src(
                    hir.registry().struct_data(struct_id).origin_id,
                    field.name.range,
                );
                check_const_dependency_cycle(hir, emit, tree, parent_id, node_id, dep, src)?;
                check_struct_size_const_dependency(hir, emit, tree, node_id, id)?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn check_attribute(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: hir::ModuleID,
    attr: Option<ast::Attribute>,
    expected: ast::AttributeKind,
) -> bool {
    let attr = if let Some(attr) = attr {
        attr
    } else {
        return false;
    };

    if attr.kind == expected {
        return true;
    }

    match attr.kind {
        ast::AttributeKind::Unknown => {
            emit.error(ErrorComp::error(
                format!(
                    "unknown attribute, only #[{}] is allowed here",
                    expected.as_str()
                ),
                hir.src(origin_id, attr.range),
                None,
            ));
        }
        _ => {
            emit.error(ErrorComp::error(
                format!(
                    "unexpected #[{}] attribute, only #[{}] is allowed here",
                    attr.kind.as_str(),
                    expected.as_str()
                ),
                hir.src(origin_id, attr.range),
                None,
            ));
        }
    }
    false
}

pub fn run<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    resolve_const_dependencies(hir, emit);

    for id in hir.registry().proc_ids() {
        typecheck_proc(hir, emit, id)
    }
}

fn typecheck_proc<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.registry().proc_item(id);
    let data = hir.registry().proc_data(id);

    let c_call = check_attribute(
        hir,
        emit,
        data.origin_id,
        item.attr_tail,
        ast::AttributeKind::C_Call,
    );

    if c_call {
        if item.block.is_some() {
            emit.error(ErrorComp::error(
                "procedures with #[c_call] attribute cannot have a body",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    } else {
        if item.block.is_none() {
            emit.error(ErrorComp::error(
                "expected tail attribute #[c_call], since procedure has no body",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if data.is_variadic {
            emit.error(ErrorComp::error(
                "procedures without #[c_call] attribute cannot be variadic, remove `..` from parameter list",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    if data.is_test {
        if c_call {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute cannot be external #[c_call]",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        if !data.params.is_empty() {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute cannot have any input parameters",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
        // not allowing `never` in test procedures,
        // panic or exit in test code doesnt make much sence, or does it? @27.04.24
        if !matches!(data.return_ty, hir::Type::Basic(BasicType::Void)) {
            emit.error(ErrorComp::error(
                "procedures with #[test] attribute can only return `void`",
                hir.src(data.origin_id, data.name.range),
                None,
            ))
        }
    }

    // main entry point cannot be external or a test (is_main detection isnt done yet) @27.04.24

    if let Some(block) = item.block {
        let mut proc = ProcScope::new(data);

        let block_res = typecheck_block(hir, emit, &mut proc, data.return_ty, block, false, None);

        let locals = proc.finish();
        let locals = emit.arena.alloc_slice(&locals);

        let data = hir.registry_mut().proc_data_mut(id);
        data.block = Some(block_res.expr);
        data.locals = locals;
    }
}

fn type_matches<'hir>(ty: hir::Type<'hir>, ty2: hir::Type<'hir>) -> bool {
    match (ty, ty2) {
        (hir::Type::Error, ..) => true,
        (.., hir::Type::Error) => true,
        (hir::Type::Basic(basic), hir::Type::Basic(basic2)) => basic == basic2,
        (hir::Type::Enum(id), hir::Type::Enum(id2)) => id == id2,
        (hir::Type::Union(id), hir::Type::Union(id2)) => id == id2,
        (hir::Type::Struct(id), hir::Type::Struct(id2)) => id == id2,
        (hir::Type::Reference(ref_ty, mutt), hir::Type::Reference(ref_ty2, mutt2)) => {
            mutt == mutt2 && type_matches(*ref_ty, *ref_ty2)
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(slice2)) => {
            slice.mutt == slice2.mutt && type_matches(slice.ty, slice2.ty)
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(array2)) => {
            //@size const_expr is ignored
            type_matches(array.ty, array2.ty)
        }
        _ => false,
    }
}

fn type_format<'hir>(hir: &HirData<'hir, '_, '_>, ty: hir::Type<'hir>) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
            BasicType::S8 => "s8".into(),
            BasicType::S16 => "s16".into(),
            BasicType::S32 => "s32".into(),
            BasicType::S64 => "s64".into(),
            BasicType::Ssize => "ssize".into(),
            BasicType::U8 => "u8".into(),
            BasicType::U16 => "u16".into(),
            BasicType::U32 => "u32".into(),
            BasicType::U64 => "u64".into(),
            BasicType::Usize => "usize".into(),
            BasicType::F16 => "f16".into(),
            BasicType::F32 => "f32".into(),
            BasicType::F64 => "f64".into(),
            BasicType::Bool => "bool".into(),
            BasicType::Char => "char".into(),
            BasicType::Rawptr => "rawptr".into(),
            BasicType::Void => "void".into(),
            BasicType::Never => "never".into(),
        },
        hir::Type::Enum(id) => hir.name_str(hir.registry().enum_data(id).name.id).into(),
        hir::Type::Union(id) => hir.name_str(hir.registry().union_data(id).name.id).into(),
        hir::Type::Struct(id) => hir.name_str(hir.registry().struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hir, *ref_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hir, slice.ty))
        }
        hir::Type::ArrayStatic(array) => format!("[<SIZE>]{}", type_format(hir, array.ty)),
    }
}

struct TypeResult<'hir> {
    ty: hir::Type<'hir>,
    expr: &'hir hir::Expr<'hir>,
    ignore: bool,
}

impl<'hir> TypeResult<'hir> {
    fn new(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: false,
        }
    }

    fn new_ignore_typecheck(ty: hir::Type<'hir>, expr: &'hir hir::Expr<'hir>) -> TypeResult<'hir> {
        TypeResult {
            ty,
            expr,
            ignore: true,
        }
    }
}

//@need type_repr instead of allocating hir types
// and maybe type::unknown, or type::infer type to facilitate better inference
// to better represent partially typed arrays, etc
#[must_use]
fn typecheck_expr<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    expr: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let expr_res = match expr.kind {
        ast::ExprKind::LitNull => typecheck_lit_null(emit),
        ast::ExprKind::LitBool { val } => typecheck_lit_bool(emit, val),
        ast::ExprKind::LitInt { val } => typecheck_lit_int(emit, expect, val),
        ast::ExprKind::LitFloat { val } => typecheck_lit_float(emit, expect, val),
        ast::ExprKind::LitChar { val } => typecheck_lit_char(emit, val),
        ast::ExprKind::LitString { id, c_string } => typecheck_lit_string(emit, id, c_string),
        ast::ExprKind::If { if_ } => typecheck_if(hir, emit, proc, expect, if_),
        ast::ExprKind::Block { block } => {
            typecheck_block(hir, emit, proc, expect, block, false, None)
        }
        ast::ExprKind::Match { match_ } => typecheck_match(hir, emit, proc, expect, match_),
        ast::ExprKind::Field { target, name } => typecheck_field(hir, emit, proc, target, name),
        ast::ExprKind::Index { target, index } => typecheck_index(hir, emit, proc, target, index),
        ast::ExprKind::Slice {
            target,
            mutt,
            slice_range,
        } => todo!("slice range slicing isnt typechecked yet"),
        ast::ExprKind::Cast { target, into } => {
            typecheck_cast(hir, emit, proc, target, into, expr.range)
        }
        ast::ExprKind::Sizeof { ty } => typecheck_sizeof(hir, emit, proc, ty),
        ast::ExprKind::Item { path } => typecheck_item(hir, emit, proc, path),
        ast::ExprKind::ProcCall { proc_call } => {
            typecheck_proc_call(hir, emit, proc, proc_call, expr.range)
        }
        ast::ExprKind::StructInit { struct_init } => {
            typecheck_struct_init(hir, emit, proc, struct_init)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_array_init(hir, emit, proc, expect, input),
        ast::ExprKind::ArrayRepeat { expr, size } => {
            typecheck_array_repeat(hir, emit, proc, expect, expr, size)
        }
        ast::ExprKind::Address { mutt, rhs } => typecheck_address(hir, emit, proc, mutt, rhs),
        ast::ExprKind::Unary { op, rhs } => typecheck_unary(hir, emit, proc, op, rhs),
        ast::ExprKind::Binary { op, lhs, rhs } => typecheck_binary(hir, emit, proc, op, lhs, rhs),
    };

    if !expr_res.ignore && !type_matches(expect, expr_res.ty) {
        emit.error(ErrorComp::error(
            format!(
                "type mismatch: expected `{}`, found `{}`",
                type_format(hir, expect),
                type_format(hir, expr_res.ty)
            ),
            hir.src(proc.origin(), expr.range),
            None,
        ));
    }

    expr_res
}

fn typecheck_lit_null<'hir>(emit: &mut HirEmit<'hir>) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Rawptr),
        emit.arena.alloc(hir::Expr::LitNull),
    )
}

fn typecheck_lit_bool<'hir>(emit: &mut HirEmit<'hir>, val: bool) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Bool),
        emit.arena.alloc(hir::Expr::LitBool { val }),
    )
}

fn typecheck_lit_int<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: hir::Type<'hir>,
    val: u64,
) -> TypeResult<'hir> {
    const DEFAULT_INT_TYPE: BasicType = BasicType::S32;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            BasicType::S8
            | BasicType::S16
            | BasicType::S32
            | BasicType::S64
            | BasicType::Ssize
            | BasicType::U8
            | BasicType::U16
            | BasicType::U32
            | BasicType::U64
            | BasicType::Usize => basic,
            _ => DEFAULT_INT_TYPE,
        },
        _ => DEFAULT_INT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::LitInt { val, ty: lit_type }),
    )
}

fn typecheck_lit_float<'hir>(
    emit: &mut HirEmit<'hir>,
    expect: hir::Type<'hir>,
    val: f64,
) -> TypeResult<'hir> {
    const DEFAULT_FLOAT_TYPE: BasicType = BasicType::F64;

    let lit_type = match expect {
        hir::Type::Basic(basic) => match basic {
            BasicType::F16 | BasicType::F32 | BasicType::F64 => basic,
            _ => DEFAULT_FLOAT_TYPE,
        },
        _ => DEFAULT_FLOAT_TYPE,
    };

    TypeResult::new(
        hir::Type::Basic(lit_type),
        emit.arena.alloc(hir::Expr::LitFloat { val, ty: lit_type }),
    )
}

fn typecheck_lit_char<'hir>(emit: &mut HirEmit<'hir>, val: char) -> TypeResult<'hir> {
    TypeResult::new(
        hir::Type::Basic(BasicType::Char),
        emit.arena.alloc(hir::Expr::LitChar { val }),
    )
}

fn typecheck_lit_string<'hir>(
    emit: &mut HirEmit<'hir>,
    id: InternID,
    c_string: bool,
) -> TypeResult<'hir> {
    let string_ty = if c_string {
        let byte = emit.arena.alloc(hir::Type::Basic(BasicType::U8));
        hir::Type::Reference(byte, ast::Mut::Immutable)
    } else {
        let slice = emit.arena.alloc(hir::ArraySlice {
            mutt: ast::Mut::Immutable,
            ty: hir::Type::Basic(BasicType::U8),
        });
        hir::Type::ArraySlice(slice)
    };

    let string_expr = hir::Expr::LitString { id, c_string };

    TypeResult::new(string_ty, emit.arena.alloc(string_expr))
}

fn typecheck_if<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    if_: &ast::If<'_>,
) -> TypeResult<'hir> {
    //@remove this when theres TypeRepr with shorthand creation functions eg TypeRepr::bool()
    const BOOL_TYPE: hir::Type = hir::Type::Basic(BasicType::Bool);

    let expect = if if_.else_block.is_some() {
        expect
    } else {
        hir::Type::Error
    };

    let entry_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, if_.entry.cond);
    let entry_block = typecheck_block(hir, emit, proc, expect, if_.entry.block, false, None);
    let entry = hir::Branch {
        cond: entry_cond.expr,
        block: entry_block.expr,
    };

    //@approx esmitation based on first entry block type
    // this is the same typecheck error reporting problem as described below
    let if_type = if if_.else_block.is_some() {
        entry_block.ty
    } else {
        hir::Type::Basic(BasicType::Void)
    };

    let mut branches = Vec::<hir::Branch>::new();
    for &branch in if_.branches {
        let branch_cond = typecheck_expr(hir, emit, proc, BOOL_TYPE, branch.cond);
        let branch_block = typecheck_block(hir, emit, proc, expect, branch.block, false, None);
        branches.push(hir::Branch {
            cond: branch_cond.expr,
            block: branch_block.expr,
        });
    }

    let mut fallback = None;
    if let Some(else_block) = if_.else_block {
        let fallback_block = typecheck_block(hir, emit, proc, expect, else_block, false, None);
        fallback = Some(fallback_block.expr);
    }

    let branches = emit.arena.alloc_slice(&branches);
    let if_ = emit.arena.alloc(hir::If {
        entry,
        branches,
        fallback,
    });
    let if_expr = emit.arena.alloc(hir::Expr::If { if_ });

    //@no idea which type to return for the if expression
    // too many different permutations, current expectation model doesnt provide enough information
    // for example caller cannot know if type error occured on that call
    // this problem applies to block, if, match, array expressions. @02.04.24
    TypeResult::new(if_type, if_expr)
}

fn typecheck_match<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    match_: &ast::Match<'_>,
) -> TypeResult<'hir> {
    let on_res = typecheck_expr(hir, emit, proc, hir::Type::Error, match_.on_expr);

    if !verify_can_match(on_res.ty) {
        emit.error(ErrorComp::error(
            format!(
                "cannot match on value of type `{}`",
                type_format(hir, on_res.ty)
            ),
            hir.src(proc.origin(), match_.on_expr.range),
            None,
        ));
    }

    //@approx esmitation based on first match block value
    let mut match_type = hir::Type::Error;
    let mut match_arms = Vec::<hir::MatchArm>::with_capacity(match_.arms.len());

    for (idx, arm) in match_.arms.iter().enumerate() {
        //@pat must also be constant value, which isnt checked
        // since it will always compile to a switch primitive
        let pat = if let Some(pat) = arm.pat {
            let pat_res = typecheck_expr(hir, emit, proc, on_res.ty, pat);
            Some(pat_res.expr)
        } else {
            None
        };

        let block_res = typecheck_expr(hir, emit, proc, expect, arm.expr);
        if idx == 0 {
            match_type = block_res.ty;
        }

        match_arms.push(hir::MatchArm {
            pat,
            expr: block_res.expr,
        })
    }

    let match_arms = emit.arena.alloc_slice(&match_arms);
    let match_ = emit.arena.alloc(hir::Match {
        on_expr: on_res.expr,
        arms: match_arms,
    });
    let match_expr = emit.arena.alloc(hir::Expr::Match { match_ });
    TypeResult::new(match_type, match_expr)
}

fn verify_can_match(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        hir::Type::Enum(_) => true,
        _ => false,
    }
}

fn typecheck_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr,
    name: ast::Name,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_res.ty, name);

    match kind {
        FieldKind::Error => TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error)),
        FieldKind::Member(union_id, member_id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::UnionMember {
                target: target_res.expr,
                union_id,
                member_id,
                deref,
            }),
        ),
        FieldKind::Field(struct_id, field_id) => TypeResult::new(
            field_ty,
            emit.arena.alloc(hir::Expr::StructField {
                target: target_res.expr,
                struct_id,
                field_id,
                deref,
            }),
        ),
    }
}

enum FieldKind {
    Error,
    Member(hir::UnionID, hir::UnionMemberID),
    Field(hir::StructID, hir::StructFieldID),
}

fn check_type_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldKind, bool) {
    let field = match ty {
        hir::Type::Reference(ref_ty, mutt) => {
            (type_get_field(hir, emit, proc, *ref_ty, name), true)
        }
        _ => (type_get_field(hir, emit, proc, ty, name), false),
    };
    (field.0 .0, field.0 .1, field.1)
}

fn type_get_field<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    name: ast::Name,
) -> (hir::Type<'hir>, FieldKind) {
    match ty {
        hir::Type::Error => (hir::Type::Error, FieldKind::Error),
        hir::Type::Union(id) => {
            let data = hir.registry().union_data(id);
            if let Some((member_id, member)) = data.find_member(name.id) {
                (member.ty, FieldKind::Member(id, member_id))
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "no field `{}` exists on union type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ),
                    hir.src(proc.origin(), name.range),
                    None,
                ));
                (hir::Type::Error, FieldKind::Error)
            }
        }
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            if let Some((field_id, field)) = data.find_field(name.id) {
                (field.ty, FieldKind::Field(id, field_id))
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "no field `{}` exists on struct type `{}`",
                        hir.name_str(name.id),
                        hir.name_str(data.name.id),
                    ),
                    hir.src(proc.origin(), name.range),
                    None,
                ));
                (hir::Type::Error, FieldKind::Error)
            }
        }
        _ => {
            let ty_format = type_format(hir, ty);
            emit.error(ErrorComp::error(
                format!(
                    "no field `{}` exists on value of type `{}`",
                    hir.name_str(name.id),
                    ty_format,
                ),
                hir.src(proc.origin(), name.range),
                None,
            ));
            (hir::Type::Error, FieldKind::Error)
        }
    }
}

fn typecheck_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    index: &ast::Expr<'_>,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let index_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), index);
    let access = check_type_index(hir, emit, proc, target_res.ty, index_res.expr, index.range);

    let access = emit.arena.alloc(access);
    let index_expr = emit.arena.alloc(hir::Expr::Index {
        target: target_res.expr,
        access,
    });
    TypeResult::new(access.elem_ty, index_expr)
}

//@codegen adjustments: @13.04.24
// auto dereference made explicit in hir
// array vs slice indexing made explicit in hir
fn check_type_index<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    index: &'hir hir::Expr<'hir>,
    index_range: TextRange,
) -> hir::IndexAccess<'hir> {
    match ty {
        hir::Type::Reference(ref_ty, mutt) => {
            type_get_elem(hir, emit, proc, *ref_ty, true, index, index_range)
        }
        _ => type_get_elem(hir, emit, proc, ty, false, index, index_range),
    }
}

fn type_get_elem<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: hir::Type<'hir>,
    deref: bool,
    index: &'hir hir::Expr<'hir>,
    index_range: TextRange,
) -> hir::IndexAccess<'hir> {
    match ty {
        hir::Type::Error => hir::IndexAccess {
            deref,
            elem_ty: hir::Type::Error,
            kind: hir::IndexKind::Slice { elem_size: 0 },
            index,
        },
        hir::Type::ArraySlice(slice) => hir::IndexAccess {
            deref,
            elem_ty: slice.ty,
            kind: hir::IndexKind::Slice { elem_size: 0 }, //@todo size
            index,
        },
        hir::Type::ArrayStatic(array) => hir::IndexAccess {
            deref,
            elem_ty: array.ty,
            kind: hir::IndexKind::Array { array },
            index,
        },
        _ => {
            emit.error(ErrorComp::error(
                format!("cannot index value of type {}", type_format(hir, ty)),
                hir.src(proc.origin(), index_range),
                None,
            ));
            hir::IndexAccess {
                deref,
                elem_ty: hir::Type::Error,
                kind: hir::IndexKind::Slice { elem_size: 0 },
                index,
            }
        }
    }
}

fn type_size(hir: &HirData, ty: hir::Type) -> Option<hir::Size> {
    match ty {
        hir::Type::Error => return None,
        hir::Type::Basic(basic) => Some(basic_type_size(basic)),
        hir::Type::Enum(id) => Some(basic_type_size(hir.registry().enum_data(id).basic)),
        hir::Type::Union(id) => hir.registry().union_data(id).size_eval.get_size(),
        hir::Type::Struct(id) => hir.registry().struct_data(id).size_eval.get_size(),
        hir::Type::Reference(_, _) => Some(hir::Size::new_equal(8)), //@assume 64bit target
        hir::Type::ArraySlice(_) => Some(hir::Size::new(16, 8)),     //@assume 64bit target
        hir::Type::ArrayStatic(array) => {
            todo!("array static sizing (size is const_expr which isnt correct currently)")
        }
    }
}

fn basic_type_size(basic: BasicType) -> hir::Size {
    match basic {
        BasicType::S8 => hir::Size::new_equal(1),
        BasicType::S16 => hir::Size::new_equal(2),
        BasicType::S32 => hir::Size::new_equal(4),
        BasicType::S64 => hir::Size::new_equal(8),
        BasicType::Ssize => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::U8 => hir::Size::new_equal(1),
        BasicType::U16 => hir::Size::new_equal(2),
        BasicType::U32 => hir::Size::new_equal(4),
        BasicType::U64 => hir::Size::new_equal(8),
        BasicType::Usize => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::F16 => hir::Size::new_equal(2),
        BasicType::F32 => hir::Size::new_equal(4),
        BasicType::F64 => hir::Size::new_equal(8),
        BasicType::Bool => hir::Size::new_equal(1),
        BasicType::Char => hir::Size::new_equal(4),
        BasicType::Rawptr => hir::Size::new_equal(8), //@assume 64bit target
        BasicType::Void => hir::Size::new(0, 1),
        BasicType::Never => hir::Size::new(0, 1),
    }
}

enum BasicTypeKind {
    SignedInt,
    UnsignedInt,
    Float,
    Bool,
    Char,
    Rawptr,
    Void,
    Never,
}

impl BasicTypeKind {
    fn from_basic(basic: BasicType) -> BasicTypeKind {
        match basic {
            BasicType::S8 | BasicType::S16 | BasicType::S32 | BasicType::S64 | BasicType::Ssize => {
                BasicTypeKind::SignedInt
            }
            BasicType::U8 | BasicType::U16 | BasicType::U32 | BasicType::U64 | BasicType::Usize => {
                BasicTypeKind::UnsignedInt
            }
            BasicType::F16 | BasicType::F32 | BasicType::F64 => BasicTypeKind::Float,
            BasicType::Bool => BasicTypeKind::Bool,
            BasicType::Char => BasicTypeKind::Char,
            BasicType::Rawptr => BasicTypeKind::Rawptr,
            BasicType::Void => BasicTypeKind::Void,
            BasicType::Never => BasicTypeKind::Never,
        }
    }
}

fn typecheck_cast<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    target: &ast::Expr<'_>,
    into: &ast::Type<'_>,
    range: TextRange,
) -> TypeResult<'hir> {
    let target_res = typecheck_expr(hir, emit, proc, hir::Type::Error, target);
    let into = super::pass_3::type_resolve(hir, emit, proc.origin(), *into);

    // early return prevents false positives on cast warning & cast error
    if matches!(target_res.ty, hir::Type::Error) || matches!(into, hir::Type::Error) {
        //@this could be skipped by returning reference to same error expression
        // to save memory in error cases and reduce noise in the code itself.

        let cast_expr = hir::Expr::Cast {
            target: target_res.expr,
            into: emit.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult::new(into, emit.arena.alloc(cast_expr));
    }

    // invariant: both types are not Error
    // ensured by early return above
    if type_matches(target_res.ty, into) {
        emit.error(ErrorComp::warning(
            format!(
                "redundant cast from `{}` into `{}`",
                type_format(hir, target_res.ty),
                type_format(hir, into)
            ),
            hir.src(proc.origin(), range),
            None,
        ));

        let cast_expr = hir::Expr::Cast {
            target: target_res.expr,
            into: emit.arena.alloc(into),
            kind: hir::CastKind::NoOp,
        };
        return TypeResult::new(into, emit.arena.alloc(cast_expr));
    }

    // invariant: from_size != into_size
    // ensured by cast redundancy warning above
    let cast_kind = match (target_res.ty, into) {
        (hir::Type::Basic(from), hir::Type::Basic(into)) => {
            let from_kind = BasicTypeKind::from_basic(from);
            let into_kind = BasicTypeKind::from_basic(into);
            let from_size = basic_type_size(from).size();
            let into_size = basic_type_size(into).size();

            match from_kind {
                BasicTypeKind::SignedInt => match into_kind {
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => {
                        if from_size < into_size {
                            hir::CastKind::Sint_Sign_Extend
                        } else {
                            hir::CastKind::Integer_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::Sint_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::UnsignedInt => match into_kind {
                    BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt => {
                        if from_size < into_size {
                            hir::CastKind::Uint_Zero_Extend
                        } else {
                            hir::CastKind::Integer_Trunc
                        }
                    }
                    BasicTypeKind::Float => hir::CastKind::Uint_to_Float,
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::Float => match into_kind {
                    BasicTypeKind::SignedInt => hir::CastKind::Float_to_Sint,
                    BasicTypeKind::UnsignedInt => hir::CastKind::Float_to_Uint,
                    BasicTypeKind::Float => {
                        if from_size < into_size {
                            hir::CastKind::Float_Extend
                        } else {
                            hir::CastKind::Float_Trunc
                        }
                    }
                    _ => hir::CastKind::Error,
                },
                BasicTypeKind::Bool => hir::CastKind::Error,
                BasicTypeKind::Char => hir::CastKind::Error,
                BasicTypeKind::Rawptr => hir::CastKind::Error,
                BasicTypeKind::Void => hir::CastKind::Error,
                BasicTypeKind::Never => hir::CastKind::Error,
            }
        }
        _ => hir::CastKind::Error,
    };

    if let hir::CastKind::Error = cast_kind {
        //@cast could be primitive but still invalid
        // wording might be improved
        // or have 2 error types for this
        emit.error(ErrorComp::error(
            format!(
                "non primitive cast from `{}` into `{}`",
                type_format(hir, target_res.ty),
                type_format(hir, into)
            ),
            hir.src(proc.origin(), range),
            None,
        ));
    }

    let cast_expr = hir::Expr::Cast {
        target: target_res.expr,
        into: emit.arena.alloc(into),
        kind: cast_kind,
    };
    TypeResult::new(into, emit.arena.alloc(cast_expr))
}

//@type-sizing not done:
// is complicated due to constant dependency graphs,
// recursive types also not detected yet.
fn typecheck_sizeof<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    ty: ast::Type,
) -> TypeResult<'hir> {
    let ty = super::pass_3::type_resolve(hir, emit, proc.origin(), ty);
    let size = type_size(hir, ty);

    //@usize semantics not finalized yet
    // assigning usize type to constant int, since it represents size
    let sizeof_expr = match size {
        Some(size) => emit.arena.alloc(hir::Expr::LitInt {
            val: size.size(),
            ty: BasicType::Usize,
        }),
        None => emit.arena.alloc(hir::Expr::Error),
    };

    TypeResult::new(hir::Type::Basic(BasicType::Usize), sizeof_expr)
}

fn typecheck_item<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    path: &ast::Path,
) -> TypeResult<'hir> {
    let (value_id, field_names) = path_resolve_value(hir, emit, Some(proc), proc.origin(), path);

    let item_res = match value_id {
        ValueID::None => {
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
        ValueID::Enum(enum_id, variant_id) => {
            let enum_variant = hir::Expr::EnumVariant {
                enum_id,
                variant_id,
            };
            return TypeResult::new(hir::Type::Enum(enum_id), emit.arena.alloc(enum_variant));
        }
        ValueID::Const(id) => TypeResult::new(
            hir.registry().const_data(id).ty,
            emit.arena.alloc(hir::Expr::ConstVar { const_id: id }),
        ),
        ValueID::Global(id) => TypeResult::new(
            hir.registry().global_data(id).ty,
            emit.arena.alloc(hir::Expr::GlobalVar { global_id: id }),
        ),
        ValueID::Local(id) => TypeResult::new(
            proc.get_local(id).ty, //@type of local var might not be known
            emit.arena.alloc(hir::Expr::LocalVar { local_id: id }),
        ),
        ValueID::Param(id) => TypeResult::new(
            proc.get_param(id).ty,
            emit.arena.alloc(hir::Expr::ParamVar { param_id: id }),
        ),
    };

    let mut target = item_res.expr;
    let mut target_ty = item_res.ty;

    for &name in field_names {
        let (field_ty, kind, deref) = check_type_field(hir, emit, proc, target_ty, name);

        match kind {
            FieldKind::Error => return TypeResult::new(hir::Type::Error, target),
            FieldKind::Member(union_id, member_id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::UnionMember {
                    target,
                    union_id,
                    member_id,
                    deref,
                });
            }
            FieldKind::Field(struct_id, field_id) => {
                target_ty = field_ty;
                target = emit.arena.alloc(hir::Expr::StructField {
                    target,
                    struct_id,
                    field_id,
                    deref,
                });
            }
        }
    }

    TypeResult::new(target_ty, target)
}

fn typecheck_proc_call<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    proc_call: &ast::ProcCall<'_>,
    expr_range: TextRange,
) -> TypeResult<'hir> {
    let proc_id = match path_resolve_proc(hir, emit, Some(proc), proc.origin(), proc_call.path) {
        Some(id) => id,
        None => {
            for &expr in proc_call.input {
                let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
            }
            return TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error));
        }
    };
    let data = hir.registry().proc_data(proc_id);
    let input_count = proc_call.input.len();
    let expected_count = data.params.len();

    if (data.is_variadic && (input_count < expected_count))
        || (!data.is_variadic && (input_count != expected_count))
    {
        let at_least = if data.is_variadic { " at least" } else { "" };
        //@plular form for argument`s` only needed if != 1
        emit.error(ErrorComp::error(
            format!(
                "expected{at_least} {} input arguments, found {}",
                expected_count, input_count
            ),
            hir.src(proc.origin(), expr_range),
            ErrorComp::info(
                "calling this procedure",
                hir.src(data.origin_id, data.name.range),
            ),
        ));
    }

    let mut input = Vec::with_capacity(proc_call.input.len());
    for (idx, &expr) in proc_call.input.iter().enumerate() {
        let expect = match data.params.get(idx) {
            Some(param) => param.ty,
            None => hir::Type::Error,
        };
        let input_res = typecheck_expr(hir, emit, proc, expect, expr);
        input.push(input_res.expr);
    }

    let input = emit.arena.alloc_slice(&input);
    let proc_call = hir::Expr::ProcCall { proc_id, input };
    TypeResult::new(data.return_ty, emit.arena.alloc(proc_call))
}

fn typecheck_struct_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    struct_init: &ast::StructInit<'_>,
) -> TypeResult<'hir> {
    let structure_id =
        path_resolve_structure(hir, emit, Some(proc), proc.origin(), struct_init.path);
    let structure_name = *struct_init.path.names.last().expect("non empty path");

    match structure_id {
        StructureID::None => {
            for input in struct_init.input {
                let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
            }
            TypeResult::new(hir::Type::Error, emit.arena.alloc(hir::Expr::Error))
        }
        StructureID::Union(union_id) => {
            let data = hir.registry().union_data(union_id);

            if let Some((first, other)) = struct_init.input.split_first() {
                let type_res = if let Some((member_id, member)) = data.find_member(first.name.id) {
                    let input_res = typecheck_expr(hir, emit, proc, member.ty, first.expr);

                    let member_init = hir::UnionMemberInit {
                        member_id,
                        expr: input_res.expr,
                    };
                    let union_init = hir::Expr::UnionInit {
                        union_id,
                        input: member_init,
                    };
                    TypeResult::new(hir::Type::Union(union_id), emit.arena.alloc(union_init))
                } else {
                    emit.error(ErrorComp::error(
                        format!("field `{}` is not found", hir.name_str(first.name.id),),
                        hir.src(proc.origin(), first.name.range),
                        ErrorComp::info(
                            "union defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, first.expr);

                    TypeResult::new(
                        hir::Type::Union(union_id),
                        emit.arena.alloc(hir::Expr::Error),
                    )
                };

                for input in other {
                    emit.error(ErrorComp::error(
                        "union initializer must have exactly one field",
                        hir.src(proc.origin(), input.name.range),
                        None,
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }

                type_res
            } else {
                emit.error(ErrorComp::error(
                    "union initializer must have exactly one field",
                    hir.src(proc.origin(), structure_name.range),
                    None,
                ));

                TypeResult::new(
                    hir::Type::Union(union_id),
                    emit.arena.alloc(hir::Expr::Error),
                )
            }
        }
        StructureID::Struct(struct_id) => {
            let data = hir.registry().struct_data(struct_id);
            let field_count = data.fields.len();

            enum FieldStatus {
                None,
                Init(TextRange),
            }

            //@potentially a lot of allocations (simple solution), same memory could be re-used
            let mut field_inits = Vec::<hir::StructFieldInit>::with_capacity(field_count);
            let mut field_status = Vec::<FieldStatus>::new();
            field_status.resize_with(field_count, || FieldStatus::None);
            let mut init_count: usize = 0;

            for input in struct_init.input {
                if let Some((field_id, field)) = data.find_field(input.name.id) {
                    let input_res = typecheck_expr(hir, emit, proc, field.ty, input.expr);

                    if let FieldStatus::Init(range) = field_status[field_id.index()] {
                        emit.error(ErrorComp::error(
                            format!(
                                "field `{}` was already initialized",
                                hir.name_str(input.name.id),
                            ),
                            hir.src(proc.origin(), input.name.range),
                            ErrorComp::info("initialized here", hir.src(data.origin_id, range)),
                        ));
                    } else {
                        let field_init = hir::StructFieldInit {
                            field_id,
                            expr: input_res.expr,
                        };
                        field_inits.push(field_init);
                        field_status[field_id.index()] = FieldStatus::Init(input.name.range);
                        init_count += 1;
                    }
                } else {
                    emit.error(ErrorComp::error(
                        format!("field `{}` is not found", hir.name_str(input.name.id),),
                        hir.src(proc.origin(), input.name.range),
                        ErrorComp::info(
                            "struct defined here",
                            hir.src(data.origin_id, data.name.range),
                        ),
                    ));
                    let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, input.expr);
                }
            }

            if init_count < field_count {
                let mut message = "missing field initializers: ".to_string();

                for (idx, status) in field_status.iter().enumerate() {
                    if let FieldStatus::None = status {
                        let field = data.field(hir::StructFieldID::new(idx));
                        message.push('`');
                        message.push_str(hir.name_str(field.name.id));
                        if idx + 1 != field_count {
                            message.push_str("`, ");
                        } else {
                            message.push('`');
                        }
                    }
                }

                emit.error(ErrorComp::error(
                    message,
                    hir.src(proc.origin(), structure_name.range),
                    ErrorComp::info(
                        "struct defined here",
                        hir.src(data.origin_id, data.name.range),
                    ),
                ));
            }

            let input = emit.arena.alloc_slice(&field_inits);
            let struct_init = hir::Expr::StructInit { struct_id, input };
            TypeResult::new(hir::Type::Struct(struct_id), emit.arena.alloc(struct_init))
        }
    }
}

fn typecheck_array_init<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    input: &[&ast::Expr<'_>],
) -> TypeResult<'hir> {
    //@unknown type in empty initializers gets ignored
    // same would be a problem for empty slices: `&[]`
    // & will be used as slicing syntax most likely
    // need to properly seaprate Error type and Unknown types
    // and handle them properly (relates to variables and overall inference flow) @06.04.24
    let mut elem_ty = hir::Type::Error;

    let expect = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let mut hir_input = Vec::with_capacity(input.len());
    for (idx, &expr) in input.iter().enumerate() {
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        hir_input.push(expr_res.expr);
        if idx == 0 {
            elem_ty = expr_res.ty;
        }
    }
    let hir_input = emit.arena.alloc_slice(&hir_input);

    //@types used during typechecking shount be a hir::Type
    // allocating const expr to store array size is temporary
    let size = emit.arena.alloc(hir::Expr::LitInt {
        val: input.len() as u64,
        ty: BasicType::Ssize,
    });
    let array_type = emit.arena.alloc(hir::ArrayStatic {
        size: hir::ConstExpr(size),
        ty: elem_ty,
    });

    let array_init = emit.arena.alloc(hir::ArrayInit {
        elem_ty,
        input: hir_input,
    });
    let array_expr = hir::Expr::ArrayInit { array_init };
    TypeResult::new(
        hir::Type::ArrayStatic(array_type),
        emit.arena.alloc(array_expr),
    )
}

fn typecheck_array_repeat<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    expr: &ast::Expr,
    size: ast::ConstExpr,
) -> TypeResult<'hir> {
    let expect = match expect {
        hir::Type::ArrayStatic(array) => array.ty,
        _ => hir::Type::Error,
    };

    let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
    let size_res = super::pass_4::const_expr_resolve(hir, emit, proc.origin(), size);

    let array_type = emit.arena.alloc(hir::ArrayStatic {
        size: size_res,
        ty: expr_res.ty,
    });

    let array_repeat = emit.arena.alloc(hir::ArrayRepeat {
        elem_ty: expr_res.ty,
        expr: expr_res.expr,
        size: size_res,
    });
    let array_expr = hir::Expr::ArrayRepeat { array_repeat };
    TypeResult::new(
        hir::Type::ArrayStatic(array_type),
        emit.arena.alloc(array_expr),
    )
}

fn typecheck_address<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    mutt: ast::Mut,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);
    //@not generating anything if type is invalid
    if let hir::Type::Error = rhs_res.ty {
        return TypeResult::new(hir::Type::Error, rhs_res.expr);
    }
    //@consider muttability and proper addressability semantics
    let ref_ty = hir::Type::Reference(emit.arena.alloc(rhs_res.ty), mutt);
    let address_expr = hir::Expr::Address { rhs: rhs_res.expr };
    TypeResult::new(ref_ty, emit.arena.alloc(address_expr))
}

fn typecheck_unary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::UnOp,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);
    //@not generating anything if type is invalid
    if let hir::Type::Error = rhs_res.ty {
        return TypeResult::new(hir::Type::Error, rhs_res.expr);
    }

    let unary_ty = match op {
        ast::UnOp::Neg => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::S8
                | BasicType::S16
                | BasicType::S32
                | BasicType::S64
                | BasicType::Ssize
                | BasicType::F32
                | BasicType::F64 => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::BitNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::U8
                | BasicType::U16
                | BasicType::U32
                | BasicType::U64
                | BasicType::Usize => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::LogicNot => match rhs_res.ty {
            hir::Type::Basic(basic) => match basic {
                BasicType::Bool => hir::Type::Basic(basic),
                _ => hir::Type::Error,
            },
            _ => hir::Type::Error,
        },
        ast::UnOp::Deref => match rhs_res.ty {
            hir::Type::Reference(ref_ty, ..) => *ref_ty,
            _ => hir::Type::Error,
        },
    };

    if let hir::Type::Error = unary_ty {
        //@unary op &str is same as token.to_str()
        // but those are separate types, this could be de-duplicated
        let op_str = match op {
            ast::UnOp::Neg => "-",
            ast::UnOp::BitNot => "~",
            ast::UnOp::LogicNot => "!",
            ast::UnOp::Deref => "*",
        };
        emit.error(ErrorComp::error(
            format!(
                "unary operator `{op_str}` cannot be applied to `{}`",
                type_format(hir, rhs_res.ty)
            ),
            hir.src(proc.origin(), rhs.range),
            None,
        ));
    }

    let unary_expr = hir::Expr::Unary {
        op,
        rhs: rhs_res.expr,
    };
    TypeResult::new(unary_ty, emit.arena.alloc(unary_expr))
}

// @26.04.24
// operator incompatability messages are bad
// no as_str() for bin_op is available, only for tokens they come from
// add range for un_op bin_op and assign_op in the ast?
// will allow for more precise and clear messages, while using more mem

fn typecheck_binary<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    op: ast::BinOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
) -> TypeResult<'hir> {
    //@remove this when theres TypeRepr with shorthand creation functions eg TypeRepr::bool()
    const BOOL_TYPE: hir::Type = hir::Type::Basic(BasicType::Bool);

    let (binary_ty, lhs_expr, rhs_expr) = match op {
        ast::BinOp::Add | ast::BinOp::Sub | ast::BinOp::Mul | ast::BinOp::Div => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if check_type_allow_math_ops(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use math operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Rem => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if check_type_allow_remainder(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use remainder operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::BitAnd | ast::BinOp::BitOr | ast::BinOp::BitXor => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            let binary_ty = if check_type_allow_and_or_xor(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use bit-wise operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::BitShl | ast::BinOp::BitShr => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            // this expectation should aim to be integer of same size @26.04.24
            // but passing lhs_res.ty would result in hard error, eg: i32 >> u32
            // passing hir::Type::Error will turn literals into default i32 values which isnt always expected
            let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, rhs);

            // how to threat arch dependant sizes? @26.04.24
            // during build we know which size we want
            // during check no such info is available
            // same question about folding the sizeof()
            // size only known when target arch is known

            match (lhs_res.ty, rhs_res.ty) {
                (hir::Type::Basic(lhs_basic), hir::Type::Basic(rhs_basic)) => {
                    if matches!(
                        BasicTypeKind::from_basic(lhs_basic),
                        BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
                    ) && matches!(
                        BasicTypeKind::from_basic(rhs_basic),
                        BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
                    ) {
                        let lhs_size = basic_type_size(lhs_basic).size();
                        let rhs_size = basic_type_size(rhs_basic).size();
                        if lhs_size != rhs_size {
                            emit.error(ErrorComp::error(
                                format!(
                                    "cannot use bit-shift operator on integers of different sizes\n`{}` has bitwidth of {}, `{}` has bitwidth of {} ",
                                    type_format(hir, lhs_res.ty),
                                    lhs_size * 8,
                                    type_format(hir, rhs_res.ty),
                                    rhs_size * 8,
                                ),
                                hir.src(proc.origin(), lhs.range),
                                None,
                            ));
                        }
                    }
                }
                _ => {}
            }

            let binary_ty = if check_type_allow_shl_shr(lhs_res.ty) {
                lhs_res.ty
            } else {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot use bit-shift operator on value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
                hir::Type::Error
            };

            (binary_ty, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::IsEq | ast::BinOp::NotEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_eq(lhs_res.ty) {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot compare equality for value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
            }

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Less | ast::BinOp::LessEq | ast::BinOp::Greater | ast::BinOp::GreaterEq => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, lhs_res.ty, rhs);

            if !check_type_allow_compare_ord(lhs_res.ty) {
                emit.error(ErrorComp::error(
                    format!(
                        "cannot compare order for value of type `{}`",
                        type_format(hir, lhs_res.ty)
                    ),
                    hir.src(proc.origin(), lhs.range),
                    None,
                ));
            }

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::LogicAnd | ast::BinOp::LogicOr => {
            let lhs_res = typecheck_expr(hir, emit, proc, BOOL_TYPE, lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, BOOL_TYPE, rhs);

            (BOOL_TYPE, lhs_res.expr, rhs_res.expr)
        }
        ast::BinOp::Range | ast::BinOp::RangeInc => {
            let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), lhs);
            let rhs_res = typecheck_expr(hir, emit, proc, hir::Type::Basic(BasicType::Usize), rhs);

            panic!("pass_5: ranges dont procude a valid typechecked expression")
        }
    };

    let lhs_signed_int = match binary_ty {
        hir::Type::Basic(basic) => {
            matches!(BasicTypeKind::from_basic(basic), BasicTypeKind::SignedInt)
        }
        _ => false,
    };
    let binary_expr = hir::Expr::Binary {
        op,
        lhs: lhs_expr,
        rhs: rhs_expr,
        lhs_signed_int,
    };
    TypeResult::new(binary_ty, emit.arena.alloc(binary_expr))
}

fn check_type_allow_math_ops(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float
        ),
        _ => false,
    }
}

fn check_type_allow_remainder(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_and_or_xor(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_shl_shr(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt
        ),
        _ => false,
    }
}

fn check_type_allow_compare_eq(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt
                | BasicTypeKind::UnsignedInt
                | BasicTypeKind::Float
                | BasicTypeKind::Bool
                | BasicTypeKind::Char
                | BasicTypeKind::Rawptr
        ),
        _ => false,
    }
}

fn check_type_allow_compare_ord(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => matches!(
            BasicTypeKind::from_basic(basic),
            BasicTypeKind::SignedInt | BasicTypeKind::UnsignedInt | BasicTypeKind::Float
        ),
        _ => false,
    }
}

fn typecheck_block<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    expect: hir::Type<'hir>,
    block: ast::Block<'_>,
    enter_loop: bool,
    enter_defer: Option<TextRange>,
) -> TypeResult<'hir> {
    proc.push_block(enter_loop, enter_defer);

    let mut block_ty = None;
    let mut hir_stmts = Vec::with_capacity(block.stmts.len());

    for stmt in block.stmts {
        let hir_stmt = match stmt.kind {
            ast::StmtKind::Break => Some(typecheck_break(hir, emit, proc, stmt.range)),
            ast::StmtKind::Continue => Some(typecheck_continue(hir, emit, proc, stmt.range)),
            ast::StmtKind::Return(expr) => {
                Some(typecheck_return(hir, emit, proc, stmt.range, expr))
            }
            ast::StmtKind::Defer(block) => {
                Some(typecheck_defer(hir, emit, proc, stmt.range.start(), *block))
            }
            ast::StmtKind::ForLoop(for_) => {
                // @alloca in loops should be done outside of a loop @13.04.24
                // to not cause memory issues and preserve the stack space?
                // research the topic more
                match for_.kind {
                    ast::ForKind::Loop => {
                        let block_res = typecheck_block(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Void),
                            for_.block,
                            true,
                            None,
                        );

                        let for_ = hir::For {
                            kind: hir::ForKind::Loop,
                            block: block_res.expr,
                        };
                        Some(hir::Stmt::ForLoop(emit.arena.alloc(for_)))
                    }
                    ast::ForKind::While { cond } => {
                        let cond_res = typecheck_expr(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Bool),
                            cond,
                        );
                        let block_res = typecheck_block(
                            hir,
                            emit,
                            proc,
                            hir::Type::Basic(BasicType::Void),
                            for_.block,
                            true,
                            None,
                        );

                        let for_ = hir::For {
                            kind: hir::ForKind::While {
                                cond: cond_res.expr,
                            },
                            block: block_res.expr,
                        };
                        Some(hir::Stmt::ForLoop(emit.arena.alloc(for_)))
                    }
                    ast::ForKind::ForLoop {
                        local,
                        cond,
                        assign,
                    } => todo!("for_loop c-like is not supported"),
                }
            }
            ast::StmtKind::Local(local) => typecheck_local(hir, emit, proc, local),
            ast::StmtKind::Assign(assign) => Some(typecheck_assign(hir, emit, proc, assign)),
            ast::StmtKind::ExprSemi(expr) => {
                let expect = if matches!(expr.kind, ast::ExprKind::Block { .. }) {
                    hir::Type::Basic(BasicType::Void)
                } else {
                    hir::Type::Error
                };
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                Some(hir::Stmt::ExprSemi(expr_res.expr))
            }
            ast::StmtKind::ExprTail(expr) => {
                let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
                if block_ty.is_none() {
                    block_ty = Some(expr_res.ty);
                }
                Some(hir::Stmt::ExprTail(expr_res.expr))
            }
        };
        if let Some(hir_stmt) = hir_stmt {
            hir_stmts.push(hir_stmt);
        }
    }

    proc.pop_block();

    let stmts = emit.arena.alloc_slice(&hir_stmts);
    let block_expr = emit.arena.alloc(hir::Expr::Block { stmts });

    if let Some(block_ty) = block_ty {
        TypeResult::new_ignore_typecheck(block_ty, block_expr)
    } else {
        TypeResult::new(hir::Type::Basic(BasicType::Void), block_expr)
    }
}

fn typecheck_break<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.is_inside_loop() {
        emit.error(ErrorComp::error(
            "cannot use `break` outside of a loop",
            hir.src(proc.origin(), range),
            None,
        ));
    }

    hir::Stmt::Break
}

fn typecheck_continue<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
) -> hir::Stmt<'hir> {
    if !proc.is_inside_loop() {
        emit.error(ErrorComp::error(
            "cannot use `continue` outside of a loop",
            hir.src(proc.origin(), range),
            None,
        ));
    }

    hir::Stmt::Continue
}

fn typecheck_return<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    range: TextRange,
    expr: Option<&ast::Expr>,
) -> hir::Stmt<'hir> {
    let expect = proc.data().return_ty;

    if let Some(prev_defer) = proc.is_inside_defer() {
        emit.error(ErrorComp::error(
            "cannot use `return` inside `defer`",
            hir.src(proc.origin(), range),
            ErrorComp::info("in this defer", hir.src(proc.origin(), prev_defer)),
        ));
    }

    if let Some(expr) = expr {
        let expr_res = typecheck_expr(hir, emit, proc, expect, expr);
        hir::Stmt::Return(Some(expr_res.expr))
    } else {
        //@this is modified duplicated typecheck error, special case for empty return @07.04.24
        let found = hir::Type::Basic(BasicType::Void);
        if !type_matches(expect, found) {
            let expect_format = type_format(hir, expect);
            let found_format = type_format(hir, found);
            emit.error(ErrorComp::error(
                format!(
                    "type mismatch: expected `{}`, found `{}`",
                    expect_format, found_format,
                ),
                hir.src(proc.origin(), range),
                ErrorComp::info(
                    format!("procedure returns `{expect_format}`"),
                    hir.src(proc.origin(), proc.data().name.range),
                ),
            ));
        }
        hir::Stmt::Return(None)
    }
}

//@allow break and continue from loops that originated within defer itself
// this can probably be done via resetting the in_loop when entering defer block
fn typecheck_defer<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    start: TextOffset,
    block: ast::Block<'_>,
) -> hir::Stmt<'hir> {
    let defer_range = TextRange::new(start, start + 5.into());

    if let Some(prev_defer) = proc.is_inside_defer() {
        emit.error(ErrorComp::error(
            "`defer` statements cannot be nested",
            hir.src(proc.origin(), defer_range),
            ErrorComp::info("already in this defer", hir.src(proc.origin(), prev_defer)),
        ));
    }

    let block_res = typecheck_block(
        hir,
        emit,
        proc,
        hir::Type::Basic(BasicType::Void),
        block,
        false,
        Some(defer_range),
    );
    hir::Stmt::Defer(block_res.expr)
}

fn typecheck_local<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    local: &ast::Local,
) -> Option<hir::Stmt<'hir>> {
    if let Some(existing) = hir.scope_name_defined(proc.origin(), local.name.id) {
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);

        if let Some(ty) = local.ty {
            super::pass_3::type_resolve(hir, emit, proc.origin(), ty);
        };
        if let Some(expr) = local.value {
            let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
        }
        return None;
    }

    //@theres no `nice` way to find both existing name from global (hir) scope
    // and proc_scope, those are so far disconnected,
    // some unified model of symbols might be better in the future
    // this also applies to SymbolKind which is separate from VariableID (leads to some issues in path resolve) @1.04.24
    if let Some(existing_var) = proc.find_variable(local.name.id) {
        let existing = match existing_var {
            VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
            VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
        };
        super::pass_1::name_already_defined_error(hir, emit, proc.origin(), local.name, existing);

        if let Some(ty) = local.ty {
            super::pass_3::type_resolve(hir, emit, proc.origin(), ty);
        };
        if let Some(expr) = local.value {
            let _ = typecheck_expr(hir, emit, proc, hir::Type::Error, expr);
        }
        return None;
    }

    //@local type can be both not specified and not inferred by the expression
    // this is currently being represented as hir::Type::Error
    // instead of having some UnknownType.
    // also proc_scope stores hir::Local which are immutable
    // so type cannot be filled in after the fact.
    //@this could be adressed by requiring type or expression on local
    // this could be a valid design choice:
    // bindings like: `let x; let y;` dont make too much sence, and can be confusing; @1.04.24

    let mut local_ty = match local.ty {
        Some(ty) => super::pass_3::type_resolve(hir, emit, proc.origin(), ty),
        None => hir::Type::Error,
    };

    let local_value = match local.value {
        Some(expr) => {
            let expr_res = typecheck_expr(hir, emit, proc, local_ty, expr);
            if local.ty.is_none() {
                local_ty = expr_res.ty;
            }
            Some(expr_res.expr)
        }
        None => None,
    };

    let local = emit.arena.alloc(hir::Local {
        mutt: local.mutt,
        name: local.name,
        ty: local_ty,
        value: local_value,
    });
    let local_id = proc.push_local(local);
    Some(hir::Stmt::Local(local_id))
}

//@not checking lhs variable mutability
//@not emitting any specific errors like when assigning to constants
//@not checking bin assignment operators (need a good way to do it same in binary expr typecheck)
// clean this up in general
fn typecheck_assign<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    assign: &ast::Assign,
) -> hir::Stmt<'hir> {
    let lhs_res = typecheck_expr(hir, emit, proc, hir::Type::Error, assign.lhs);

    let expect = if verify_is_expr_assignable(lhs_res.expr) {
        lhs_res.ty
    } else {
        emit.error(ErrorComp::error(
            "cannot assign to this expression",
            hir.src(proc.origin(), assign.lhs.range),
            None,
        ));
        hir::Type::Error
    };

    let rhs_res = typecheck_expr(hir, emit, proc, expect, assign.rhs);

    let lhs_signed_int = match lhs_res.ty {
        hir::Type::Basic(basic) => {
            matches!(BasicTypeKind::from_basic(basic), BasicTypeKind::SignedInt)
        }
        _ => false,
    };
    let assign = hir::Assign {
        op: assign.op,
        lhs: lhs_res.expr,
        rhs: rhs_res.expr,
        lhs_ty: lhs_res.ty,
        lhs_signed_int,
    };
    hir::Stmt::Assign(emit.arena.alloc(assign))
}

fn verify_is_expr_assignable(expr: &hir::Expr) -> bool {
    match *expr {
        hir::Expr::Error => true,
        hir::Expr::UnionMember { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::StructField { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::Index { target, .. } => verify_is_expr_assignable(target),
        hir::Expr::LocalVar { .. } => true,
        hir::Expr::ParamVar { .. } => true,
        hir::Expr::GlobalVar { .. } => true, //@add mut to globals
        hir::Expr::Unary { op, .. } => matches!(op, ast::UnOp::Deref), //@support deref assignment properly
        _ => false,
    }
}

// these calls are only done for items so far @26.04.24
// locals or input params etc not checked yet (need to find a reasonable strategy)
// proc return type is allowed to be never or void unlike other instances where types are used
pub fn require_value_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    ty: hir::Type,
    source: SourceRange,
) {
    if !type_is_value_type(ty) {
        emit.error(ErrorComp::error(
            format!("expected value type, found `{}`", type_format(hir, ty)),
            source,
            None,
        ))
    }
}

pub fn type_is_value_type(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error => true,
        hir::Type::Basic(basic) => !matches!(basic, BasicType::Void | BasicType::Never),
        hir::Type::Enum(_) => true,
        hir::Type::Union(_) => true,
        hir::Type::Struct(_) => true,
        hir::Type::Reference(ref_ty, _) => type_is_value_type(*ref_ty),
        hir::Type::ArraySlice(slice) => type_is_value_type(slice.ty),
        hir::Type::ArrayStatic(array) => type_is_value_type(array.ty),
    }
}

/*

module     -> <first?>
proc       -> [no follow]
enum       -> <follow?> by single enum variant name
union      -> [no follow]
struct     -> [no follow]
const      -> <follow?> by <chained> field access
global     -> <follow?> by <chained> field access
param_var  -> <follow?> by <chained> field access
local_var  -> <follow?> by <chained> field access

*/

enum ResolvedPath {
    None,
    Variable(VariableID),
    Symbol(SymbolKind, SourceRange),
}

fn path_resolve<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> (ResolvedPath, usize) {
    let name = path.names.first().cloned().expect("non empty path");

    if let Some(proc) = proc {
        if let Some(var_id) = proc.find_variable(name.id) {
            return (ResolvedPath::Variable(var_id), 0);
        }
    }

    let (module_id, name) = match hir.symbol_from_scope(emit, origin_id, origin_id, name) {
        Some((kind, source)) => {
            let next_name = path.names.get(1).cloned();
            match (kind, next_name) {
                (SymbolKind::Module(module_id), Some(name)) => (module_id, name),
                _ => return (ResolvedPath::Symbol(kind, source), 0),
            }
        }
        None => return (ResolvedPath::None, 0),
    };

    match hir.symbol_from_scope(emit, origin_id, module_id, name) {
        Some((kind, source)) => (ResolvedPath::Symbol(kind, source), 1),
        None => (ResolvedPath::None, 1),
    }
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
pub fn path_resolve_type<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let ty = match resolved {
        ResolvedPath::None => return hir::Type::Error,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::error(
                format!("expected type, found local `{}`", hir.name_str(name.id)),
                hir.src(origin_id, name.range),
                ErrorComp::info("defined here", source),
            ));
            return hir::Type::Error;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => hir::Type::Enum(id),
            SymbolKind::Union(id) => hir::Type::Union(id),
            SymbolKind::Struct(id) => hir::Type::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected type, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return hir::Type::Error;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::error(
                "unexpected path segment",
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    ty
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
fn path_resolve_proc<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> Option<hir::ProcID> {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let proc_id = match resolved {
        ResolvedPath::None => return None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::error(
                format!(
                    "expected procedure, found local `{}`",
                    hir.name_str(name.id)
                ),
                hir.src(origin_id, name.range),
                ErrorComp::info("defined here", source),
            ));
            return None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Proc(id) => Some(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected procedure, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::error(
                "unexpected path segment",
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    proc_id
}

enum StructureID {
    None,
    Union(hir::UnionID),
    Struct(hir::StructID),
}

//@duplication issue with other path resolve procs
// mainly due to bad scope / symbol design
fn path_resolve_structure<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &ast::Path,
) -> StructureID {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let structure_id = match resolved {
        ResolvedPath::None => return StructureID::None,
        ResolvedPath::Variable(variable) => {
            let name = path.names[name_idx];

            let proc = proc.expect("proc context");
            let source = match variable {
                VariableID::Local(id) => hir.src(proc.origin(), proc.get_local(id).name.range),
                VariableID::Param(id) => hir.src(proc.origin(), proc.get_param(id).name.range),
            };
            //@calling this `local` for both params and locals, validate wording consistency
            // by maybe extracting all error formats to separate module @07.04.24
            emit.error(ErrorComp::error(
                format!(
                    "expected struct or union, found local `{}`",
                    hir.name_str(name.id)
                ),
                hir.src(origin_id, name.range),
                ErrorComp::info("defined here", source),
            ));
            return StructureID::None;
        }
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Union(id) => StructureID::Union(id),
            SymbolKind::Struct(id) => StructureID::Struct(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected struct or union, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return StructureID::None;
            }
        },
    };

    if let Some(remaining) = path.names.get(name_idx + 1..) {
        if let (Some(first), Some(last)) = (remaining.first(), remaining.last()) {
            let range = TextRange::new(first.range.start(), last.range.end());
            emit.error(ErrorComp::error(
                "unexpected path segment",
                hir.src(origin_id, range),
                None,
            ));
        }
    }

    structure_id
}

enum ValueID {
    None,
    Enum(hir::EnumID, hir::EnumVariantID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

fn path_resolve_value<'hir, 'ast>(
    hir: &HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    proc: Option<&ProcScope<'hir, '_>>,
    origin_id: hir::ModuleID,
    path: &'ast ast::Path<'ast>,
) -> (ValueID, &'ast [ast::Name]) {
    let (resolved, name_idx) = path_resolve(hir, emit, proc, origin_id, path);

    let value_id = match resolved {
        ResolvedPath::None => return (ValueID::None, &[]),
        ResolvedPath::Variable(var) => match var {
            VariableID::Local(id) => ValueID::Local(id),
            VariableID::Param(id) => ValueID::Param(id),
        },
        ResolvedPath::Symbol(kind, source) => match kind {
            SymbolKind::Enum(id) => {
                if let Some(variant_name) = path.names.get(name_idx + 1) {
                    let enum_data = hir.registry().enum_data(id);
                    if let Some((variant_id, ..)) = enum_data.find_variant(variant_name.id) {
                        if let Some(remaining) = path.names.get(name_idx + 2..) {
                            if let (Some(first), Some(last)) = (remaining.first(), remaining.last())
                            {
                                let range = TextRange::new(first.range.start(), last.range.end());
                                emit.error(ErrorComp::error(
                                    "unexpected path segment",
                                    hir.src(origin_id, range),
                                    None,
                                ));
                            }
                        }
                        return (ValueID::Enum(id, variant_id), &[]);
                    } else {
                        emit.error(ErrorComp::error(
                            format!(
                                "enum variant `{}` is not found",
                                hir.name_str(variant_name.id)
                            ),
                            hir.src(origin_id, variant_name.range),
                            ErrorComp::info("enum defined here", source),
                        ));
                        return (ValueID::None, &[]);
                    }
                } else {
                    let name = path.names[name_idx];
                    emit.error(ErrorComp::error(
                        format!(
                            "expected value, found {} `{}`",
                            HirData::symbol_kind_name(kind),
                            hir.name_str(name.id)
                        ),
                        hir.src(origin_id, name.range),
                        ErrorComp::info("defined here", source),
                    ));
                    return (ValueID::None, &[]);
                }
            }
            SymbolKind::Const(id) => ValueID::Const(id),
            SymbolKind::Global(id) => ValueID::Global(id),
            _ => {
                let name = path.names[name_idx];
                emit.error(ErrorComp::error(
                    format!(
                        "expected value, found {} `{}`",
                        HirData::symbol_kind_name(kind),
                        hir.name_str(name.id)
                    ),
                    hir.src(origin_id, name.range),
                    ErrorComp::info("defined here", source),
                ));
                return (ValueID::None, &[]);
            }
        },
    };

    let field_names = &path.names[name_idx + 1..];
    (value_id, field_names)
}

use super::syntax_kind::SyntaxKind;
use super::syntax_tree::{Node, NodeID, NodeOrToken, SyntaxTree};
use std::marker::PhantomData;

impl<'syn> SyntaxTree<'syn> {
    pub fn source_file(&'syn self) -> SourceFile<'syn> {
        let root = self.node(NodeID::new(0));
        SourceFile::cast(root).unwrap()
    }
}

impl<'syn> Node<'syn> {
    fn find_first<T: AstNode<'syn>>(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<T> {
        AstNodeIterator::new(tree, self).next()
    }
}

pub trait AstNode<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized;
}

pub struct AstNodeIterator<'syn, T: AstNode<'syn>> {
    tree: &'syn SyntaxTree<'syn>,
    iter: std::slice::Iter<'syn, NodeOrToken>,
    phantom: PhantomData<T>,
}

impl<'syn, T: AstNode<'syn>> AstNodeIterator<'syn, T> {
    fn new(tree: &'syn SyntaxTree<'syn>, node: &'syn Node<'syn>) -> AstNodeIterator<'syn, T> {
        AstNodeIterator {
            tree,
            iter: node.content.iter(),
            phantom: PhantomData::default(),
        }
    }
}

impl<'syn, T: AstNode<'syn>> Iterator for AstNodeIterator<'syn, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(NodeOrToken::Node(node_id)) => {
                    let node = self.tree.node(*node_id);
                    if let Some(ast_node) = T::cast(node) {
                        return Some(ast_node);
                    }
                }
                Some(NodeOrToken::Token(_)) => {}
                None => return None,
            }
        }
    }
}

macro_rules! ast_node_impl {
    ($name:ident, $kind_pat:pat) => {
        pub struct $name<'syn>(&'syn Node<'syn>);

        impl<'syn> AstNode<'syn> for $name<'syn> {
            fn cast(node: &'syn Node) -> Option<Self>
            where
                Self: Sized,
            {
                if matches!(node.kind, $kind_pat) {
                    Some($name(node))
                } else {
                    None
                }
            }
        }
    };
}

ast_node_impl!(SourceFile, SyntaxKind::SOURCE_FILE);

pub enum Item<'syn> {
    Proc(ProcItem<'syn>),
    Enum(EnumItem<'syn>),
    Struct(StructItem<'syn>),
    Const(ConstItem<'syn>),
    Global(GlobalItem<'syn>),
    Import(ImportItem<'syn>),
}

ast_node_impl!(ProcItem, SyntaxKind::PROC_ITEM);
ast_node_impl!(ParamList, SyntaxKind::PARAM_LIST);
ast_node_impl!(Param, SyntaxKind::PARAM);
ast_node_impl!(EnumItem, SyntaxKind::ENUM_ITEM);
ast_node_impl!(VariantList, SyntaxKind::VARIANT_LIST);
ast_node_impl!(Variant, SyntaxKind::VARIANT);
ast_node_impl!(StructItem, SyntaxKind::STRUCT_ITEM);
ast_node_impl!(FieldList, SyntaxKind::FIELD_LIST);
ast_node_impl!(Field, SyntaxKind::FIELD);
ast_node_impl!(ConstItem, SyntaxKind::CONST_ITEM);
ast_node_impl!(GlobalItem, SyntaxKind::GLOBAL_ITEM);
ast_node_impl!(ImportItem, SyntaxKind::IMPORT_ITEM);
ast_node_impl!(ImportSymbolList, SyntaxKind::IMPORT_SYMBOL_LIST);
ast_node_impl!(ImportSymbol, SyntaxKind::IMPORT_SYMBOL);

ast_node_impl!(Name, SyntaxKind::NAME);
ast_node_impl!(Path, SyntaxKind::PATH);

impl<'syn> SourceFile<'syn> {
    pub fn items(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Item<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> AstNode<'syn> for Item<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized,
    {
        match node.kind {
            SyntaxKind::PROC_ITEM => Some(Item::Proc(ProcItem(node))),
            SyntaxKind::ENUM_ITEM => Some(Item::Enum(EnumItem(node))),
            SyntaxKind::STRUCT_ITEM => Some(Item::Struct(StructItem(node))),
            SyntaxKind::CONST_ITEM => Some(Item::Const(ConstItem(node))),
            SyntaxKind::GLOBAL_ITEM => Some(Item::Global(GlobalItem(node))),
            SyntaxKind::IMPORT_ITEM => Some(Item::Import(ImportItem(node))),
            _ => None,
        }
    }
}

impl<'syn> ProcItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    pub fn param_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ParamList<'syn>> {
        self.0.find_first(tree)
    }
    //@return type
    //@optional block
}

impl<'syn> ParamList<'syn> {
    pub fn params(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Param<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> Param<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@type
}

impl<'syn> EnumItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@optional basic type
    pub fn variant_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<VariantList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> VariantList<'syn> {
    pub fn variants(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Variant<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> Variant<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@optional value
}

impl<'syn> StructItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    pub fn field_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<FieldList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> FieldList<'syn> {
    pub fn fields(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Field<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> Field<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@type
}

impl<'syn> ConstItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@type
    //@value
}

impl<'syn> GlobalItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@mut
    //@type
    //@value
}

impl<'syn> ImportItem<'syn> {
    //@import `/` separated path (add separate node)
    //@alias (need separate node for `as` part)
    pub fn import_symbol_list(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> Option<ImportSymbolList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ImportSymbolList<'syn> {
    pub fn import_symbols(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> AstNodeIterator<'syn, ImportSymbol<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> ImportSymbol<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@alias (need separate node for `as` part)
}

impl<'syn> Name<'syn> {
    //@identifier token
}

impl<'syn> Path<'syn> {
    pub fn names(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Name<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

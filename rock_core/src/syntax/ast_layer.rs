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

ast_node_impl!(TypeBasic, SyntaxKind::TYPE_BASIC);
ast_node_impl!(TypeCustom, SyntaxKind::TYPE_CUSTOM);
ast_node_impl!(TypeReference, SyntaxKind::TYPE_REFERENCE);
ast_node_impl!(TypeProcedure, SyntaxKind::TYPE_PROCEDURE);
ast_node_impl!(ParamTypeList, SyntaxKind::PARAM_TYPE_LIST);
ast_node_impl!(TypeArraySlice, SyntaxKind::TYPE_ARRAY_SLICE);
ast_node_impl!(TypeArrayStatic, SyntaxKind::TYPE_ARRAY_STATIC);

ast_node_impl!(StmtBreak, SyntaxKind::STMT_BREAK);
ast_node_impl!(StmtContinue, SyntaxKind::STMT_CONTINUE);
ast_node_impl!(StmtReturn, SyntaxKind::STMT_RETURN);
ast_node_impl!(StmtDefer, SyntaxKind::STMT_DEFER);
ast_node_impl!(StmtLoop, SyntaxKind::STMT_LOOP);
ast_node_impl!(StmtLocal, SyntaxKind::STMT_LOCAL);
ast_node_impl!(StmtAssign, SyntaxKind::STMT_ASSIGN);
ast_node_impl!(StmtExprSemi, SyntaxKind::STMT_EXPR_SEMI);
ast_node_impl!(StmtExprTail, SyntaxKind::STMT_EXPR_TAIL);

ast_node_impl!(ExprParen, SyntaxKind::EXPR_PAREN);
ast_node_impl!(ExprLitNull, SyntaxKind::EXPR_LIT_NULL);
ast_node_impl!(ExprLitBool, SyntaxKind::EXPR_LIT_BOOL);
ast_node_impl!(ExprLitInt, SyntaxKind::EXPR_LIT_INT);
ast_node_impl!(ExprLitFloat, SyntaxKind::EXPR_LIT_FLOAT);
ast_node_impl!(ExprLitChar, SyntaxKind::EXPR_LIT_CHAR);
ast_node_impl!(ExprLitString, SyntaxKind::EXPR_LIT_STRING);
ast_node_impl!(ExprIf, SyntaxKind::EXPR_IF);
ast_node_impl!(ExprBlock, SyntaxKind::EXPR_BLOCK);
ast_node_impl!(ExprMatch, SyntaxKind::EXPR_MATCH);
ast_node_impl!(MatchArmList, SyntaxKind::MATCH_ARM_LIST);
ast_node_impl!(MatchArm, SyntaxKind::MATCH_ARM);
ast_node_impl!(MatchArmFallback, SyntaxKind::MATCH_ARM_FALLBACK);
ast_node_impl!(ExprField, SyntaxKind::EXPR_FIELD);
ast_node_impl!(ExprIndex, SyntaxKind::EXPR_INDEX);
ast_node_impl!(ExprCall, SyntaxKind::EXPR_CALL);
ast_node_impl!(CallArgumentList, SyntaxKind::CALL_ARGUMENT_LIST);
ast_node_impl!(ExprCast, SyntaxKind::EXPR_CAST);
ast_node_impl!(ExprSizeof, SyntaxKind::EXPR_SIZEOF);
ast_node_impl!(ExprItem, SyntaxKind::EXPR_ITEM);
ast_node_impl!(ExprVariant, SyntaxKind::EXPR_VARIANT);
ast_node_impl!(ExprStructInit, SyntaxKind::EXPR_STRUCT_INIT);
ast_node_impl!(StructFieldInitList, SyntaxKind::STRUCT_FIELD_INIT_LIST);
ast_node_impl!(StructFieldInit, SyntaxKind::STRUCT_FIELD_INIT);
ast_node_impl!(ExprArrayInit, SyntaxKind::EXPR_ARRAY_INIT);
ast_node_impl!(ExprArrayRepeat, SyntaxKind::EXPR_ARRAY_REPEAT);
ast_node_impl!(ExprDeref, SyntaxKind::EXPR_DEREF);
ast_node_impl!(ExprAddress, SyntaxKind::EXPR_ADDRESS);
ast_node_impl!(ExprUnary, SyntaxKind::EXPR_UNARY);
ast_node_impl!(ExprBinary, SyntaxKind::EXPR_BINARY);

pub enum Item<'syn> {
    Proc(ProcItem<'syn>),
    Enum(EnumItem<'syn>),
    Struct(StructItem<'syn>),
    Const(ConstItem<'syn>),
    Global(GlobalItem<'syn>),
    Import(ImportItem<'syn>),
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

pub enum Type<'syn> {
    Basic(TypeBasic<'syn>),
    Custom(TypeCustom<'syn>),
    Reference(TypeReference<'syn>),
    Procedure(TypeProcedure<'syn>),
    ArraySlice(TypeArraySlice<'syn>),
    ArrayStatic(TypeArrayStatic<'syn>),
}

impl<'syn> AstNode<'syn> for Type<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized,
    {
        match node.kind {
            SyntaxKind::TYPE_BASIC => Some(Type::Basic(TypeBasic(node))),
            SyntaxKind::TYPE_CUSTOM => Some(Type::Custom(TypeCustom(node))),
            SyntaxKind::TYPE_REFERENCE => Some(Type::Reference(TypeReference(node))),
            SyntaxKind::TYPE_PROCEDURE => Some(Type::Procedure(TypeProcedure(node))),
            SyntaxKind::TYPE_ARRAY_SLICE => Some(Type::ArraySlice(TypeArraySlice(node))),
            SyntaxKind::TYPE_ARRAY_STATIC => Some(Type::ArrayStatic(TypeArrayStatic(node))),
            _ => None,
        }
    }
}

pub enum Stmt<'syn> {
    Break(StmtBreak<'syn>),
    Continue(StmtContinue<'syn>),
    Return(StmtReturn<'syn>),
    Defer(StmtDefer<'syn>),
    Loop(StmtLoop<'syn>),
    Local(StmtLocal<'syn>),
    Assign(StmtAssign<'syn>),
    ExprSemi(StmtExprSemi<'syn>),
    ExprTail(StmtExprTail<'syn>),
}

impl<'syn> AstNode<'syn> for Stmt<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized,
    {
        match node.kind {
            SyntaxKind::STMT_BREAK => Some(Stmt::Break(StmtBreak(node))),
            SyntaxKind::STMT_CONTINUE => Some(Stmt::Continue(StmtContinue(node))),
            SyntaxKind::STMT_RETURN => Some(Stmt::Return(StmtReturn(node))),
            SyntaxKind::STMT_DEFER => Some(Stmt::Defer(StmtDefer(node))),
            SyntaxKind::STMT_LOOP => Some(Stmt::Loop(StmtLoop(node))),
            SyntaxKind::STMT_LOCAL => Some(Stmt::Local(StmtLocal(node))),
            SyntaxKind::STMT_ASSIGN => Some(Stmt::Assign(StmtAssign(node))),
            SyntaxKind::STMT_EXPR_SEMI => Some(Stmt::ExprSemi(StmtExprSemi(node))),
            SyntaxKind::STMT_EXPR_TAIL => Some(Stmt::ExprTail(StmtExprTail(node))),
            _ => None,
        }
    }
}

pub enum Expr<'syn> {
    Paren(ExprParen<'syn>),
    LitNull(ExprLitNull<'syn>),
    LitBool(ExprLitBool<'syn>),
    LitInt(ExprLitInt<'syn>),
    LitFloat(ExprLitFloat<'syn>),
    LitChar(ExprLitChar<'syn>),
    LitString(ExprLitString<'syn>),
    If(ExprIf<'syn>),
    Block(ExprBlock<'syn>),
    Match(ExprMatch<'syn>),
    Field(ExprField<'syn>),
    Index(ExprIndex<'syn>),
    Call(ExprCall<'syn>),
    Cast(ExprCast<'syn>),
    Sizeof(ExprSizeof<'syn>),
    Item(ExprItem<'syn>),
    Variant(ExprVariant<'syn>),
    StructInit(ExprStructInit<'syn>),
    ArrayInit(ExprArrayInit<'syn>),
    ArrayRepeat(ExprArrayRepeat<'syn>),
    Deref(ExprDeref<'syn>),
    Address(ExprAddress<'syn>),
    Unary(ExprUnary<'syn>),
    Binary(ExprBinary<'syn>),
}

impl<'syn> AstNode<'syn> for Expr<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized,
    {
        match node.kind {
            SyntaxKind::EXPR_PAREN => Some(Expr::Paren(ExprParen(node))),
            SyntaxKind::EXPR_LIT_NULL => Some(Expr::LitNull(ExprLitNull(node))),
            SyntaxKind::EXPR_LIT_BOOL => Some(Expr::LitBool(ExprLitBool(node))),
            SyntaxKind::EXPR_LIT_INT => Some(Expr::LitInt(ExprLitInt(node))),
            SyntaxKind::EXPR_LIT_FLOAT => Some(Expr::LitFloat(ExprLitFloat(node))),
            SyntaxKind::EXPR_LIT_CHAR => Some(Expr::LitChar(ExprLitChar(node))),
            SyntaxKind::EXPR_LIT_STRING => Some(Expr::LitString(ExprLitString(node))),
            SyntaxKind::EXPR_IF => Some(Expr::If(ExprIf(node))),
            SyntaxKind::EXPR_BLOCK => Some(Expr::Block(ExprBlock(node))),
            SyntaxKind::EXPR_MATCH => Some(Expr::Match(ExprMatch(node))),
            SyntaxKind::EXPR_FIELD => Some(Expr::Field(ExprField(node))),
            SyntaxKind::EXPR_INDEX => Some(Expr::Index(ExprIndex(node))),
            SyntaxKind::EXPR_CALL => Some(Expr::Call(ExprCall(node))),
            SyntaxKind::EXPR_CAST => Some(Expr::Cast(ExprCast(node))),
            SyntaxKind::EXPR_SIZEOF => Some(Expr::Sizeof(ExprSizeof(node))),
            SyntaxKind::EXPR_ITEM => Some(Expr::Item(ExprItem(node))),
            SyntaxKind::EXPR_VARIANT => Some(Expr::Variant(ExprVariant(node))),
            SyntaxKind::EXPR_STRUCT_INIT => Some(Expr::StructInit(ExprStructInit(node))),
            SyntaxKind::EXPR_ARRAY_INIT => Some(Expr::ArrayInit(ExprArrayInit(node))),
            SyntaxKind::EXPR_ARRAY_REPEAT => Some(Expr::ArrayRepeat(ExprArrayRepeat(node))),
            SyntaxKind::EXPR_DEREF => Some(Expr::Deref(ExprDeref(node))),
            SyntaxKind::EXPR_ADDRESS => Some(Expr::Address(ExprAddress(node))),
            SyntaxKind::EXPR_UNARY => Some(Expr::Unary(ExprUnary(node))),
            SyntaxKind::EXPR_BINARY => Some(Expr::Binary(ExprBinary(node))),
            _ => None,
        }
    }
}

impl<'syn> SourceFile<'syn> {
    pub fn items(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Item<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> ProcItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    pub fn param_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ParamList<'syn>> {
        self.0.find_first(tree)
    }
    //@is variadic
    pub fn return_ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
    pub fn block(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ExprBlock<'syn>> {
        self.0.find_first(tree)
    }
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
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
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
    pub fn value(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
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
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ConstItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
    pub fn value(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> GlobalItem<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
    //@mut
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
    pub fn value(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
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

impl<'syn> TypeBasic<'syn> {
    //@basic type token?
}

impl<'syn> TypeCustom<'syn> {
    pub fn path(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Path<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> TypeReference<'syn> {
    //@mut
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> TypeProcedure<'syn> {
    pub fn param_type_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ParamTypeList<'syn>> {
        self.0.find_first(tree)
    }
    //@is variadic
    pub fn return_ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ParamTypeList<'syn> {
    pub fn param_types(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Type<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> TypeArraySlice<'syn> {
    //@mut
    pub fn elem_ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> TypeArrayStatic<'syn> {
    pub fn len(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    pub fn elem_ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> StmtBreak<'syn> {}

impl<'syn> StmtContinue<'syn> {}

impl<'syn> StmtReturn<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> StmtDefer<'syn> {
    pub fn block(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ExprBlock<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> StmtLoop<'syn> {
    //@todo
}

impl<'syn> StmtLocal<'syn> {
    //@todo
}

impl<'syn> StmtAssign<'syn> {
    //@todo
}

impl<'syn> StmtExprSemi<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> StmtExprTail<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprParen<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprLitNull<'syn> {}

impl<'syn> ExprLitBool<'syn> {
    //@true of false token
}

impl<'syn> ExprLitInt<'syn> {
    //@int token
}

impl<'syn> ExprLitFloat<'syn> {
    //@float token
}

impl<'syn> ExprLitChar<'syn> {
    //@char token
}

impl<'syn> ExprLitString<'syn> {
    //@string lit token
}

impl<'syn> ExprIf<'syn> {
    //@branches
    //@fallback, add new fallback node?
}

impl<'syn> ExprBlock<'syn> {
    pub fn stmts(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Stmt<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> ExprMatch<'syn> {
    pub fn on_expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    pub fn match_arm_list(&self, tree: &'syn SyntaxTree<'syn>) -> Option<MatchArmList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> MatchArmList<'syn> {
    pub fn match_arms(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> AstNodeIterator<'syn, MatchArm<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
    pub fn fallback(&self, tree: &'syn SyntaxTree<'syn>) -> Option<MatchArmFallback<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> MatchArm<'syn> {
    //@how to separate pat and expr?
    // find_first `before` some token pattern?
    // what if -> is missing?
}

impl<'syn> MatchArmFallback<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprField<'syn> {
    pub fn target(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprIndex<'syn> {
    pub fn target(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    //@index or slice range
}

impl<'syn> ExprCall<'syn> {
    pub fn target(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    pub fn call_argument_list(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> Option<CallArgumentList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> CallArgumentList<'syn> {
    pub fn input(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Expr<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> ExprCast<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprSizeof<'syn> {
    pub fn ty(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Type<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprItem<'syn> {
    pub fn path(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Path<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprVariant<'syn> {
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Name<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprStructInit<'syn> {
    pub fn path(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Path<'syn>> {
        self.0.find_first(tree)
    }
    pub fn name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<StructFieldInitList<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> StructFieldInitList<'syn> {
    pub fn field_inits(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> AstNodeIterator<'syn, StructFieldInit<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> StructFieldInit<'syn> {
    //@differentiate name, and name: expr
    //@name
    //@expr init
}

impl<'syn> ExprArrayInit<'syn> {
    pub fn input(&self, tree: &'syn SyntaxTree<'syn>) -> AstNodeIterator<'syn, Expr<'syn>> {
        AstNodeIterator::new(tree, self.0)
    }
}

impl<'syn> ExprArrayRepeat<'syn> {
    //@expr
    //@repeat count
}

impl<'syn> ExprDeref<'syn> {
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprAddress<'syn> {
    //@mut
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprUnary<'syn> {
    //@unary op
    pub fn expr(&self, tree: &'syn SyntaxTree<'syn>) -> Option<Expr<'syn>> {
        self.0.find_first(tree)
    }
}

impl<'syn> ExprBinary<'syn> {
    //@binary op
    //@lhs
    //@rhs
}

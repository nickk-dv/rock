use super::syntax_kind::SyntaxKind;
use super::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use crate::ast;
use crate::text::TextRange;
use crate::token::{Token, T};
use std::marker::PhantomData;

impl<'syn> SyntaxTree<'syn> {
    pub fn source_file(&'syn self) -> SourceFile<'syn> {
        let root = self.root();
        SourceFile::cast(root).unwrap()
    }
}

impl<'syn> Node<'syn> {
    fn find_first<T: AstNode<'syn>>(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<T> {
        AstNodeIterator::new(tree, self).next()
    }

    fn find_by_token<T, F>(&self, tree: &'syn SyntaxTree<'syn>, predicate: F) -> Option<T>
    where
        F: Fn(Token) -> Option<T>,
    {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(value) = predicate(tree.tokens().token(token_id)) {
                    return Some(value);
                }
            }
        }
        None
    }

    fn find_by_token_with_range<T, F>(
        &self,
        tree: &'syn SyntaxTree<'syn>,
        predicate: F,
    ) -> Option<(T, TextRange)>
    where
        F: Fn(Token) -> Option<T>,
    {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(value) = predicate(tree.tokens().token(token_id)) {
                    return Some((value, tree.tokens().token_range(token_id)));
                }
            }
        }
        None
    }

    fn find_token(&self, tree: &'syn SyntaxTree<'syn>, token: Token) -> bool {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if token == tree.tokens().token(token_id) {
                    return true;
                }
            }
        }
        false
    }

    fn find_token_rev(&self, tree: &'syn SyntaxTree<'syn>, token: Token) -> bool {
        for node_or_token in self.content.iter().rev().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if token == tree.tokens().token(token_id) {
                    return true;
                }
            }
        }
        false
    }

    //@can get stuck if no tokens are present, which is maybe possible in incomplete tree?
    fn find_range(&self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        let start;
        let end;

        let mut content_curr = self.content;

        'outher: loop {
            for not in content_curr.iter().copied() {
                match not {
                    NodeOrToken::Node(node_id) => {
                        content_curr = tree.node(node_id).content;
                        continue 'outher;
                    }
                    NodeOrToken::Token(token_id) => {
                        start = tree.tokens().token_range(token_id).start();
                        break 'outher;
                    }
                    NodeOrToken::Trivia(_) => {}
                }
            }
            unreachable!("node start not found");
        }

        content_curr = self.content;

        'outher: loop {
            for node_or_token in content_curr.iter().rev().copied() {
                match node_or_token {
                    NodeOrToken::Node(node_id) => {
                        content_curr = tree.node(node_id).content;
                        continue 'outher;
                    }
                    NodeOrToken::Token(token_id) => {
                        end = tree.tokens().token_range(token_id).end();
                        break 'outher;
                    }
                    NodeOrToken::Trivia(_) => {}
                }
            }
            unreachable!("node end not found");
        }

        TextRange::new(start, end)
    }
}

pub trait AstNode<'syn> {
    fn cast(node: &'syn Node) -> Option<Self>
    where
        Self: Sized;
    fn node(&self) -> &Node<'syn>;
}

pub struct AstNodeIterator<'syn, T: AstNode<'syn>> {
    tree: &'syn SyntaxTree<'syn>,
    iter: std::slice::Iter<'syn, NodeOrToken<'syn>>,
    phantom: PhantomData<T>,
}

impl<'syn, T: AstNode<'syn>> AstNodeIterator<'syn, T> {
    fn new(tree: &'syn SyntaxTree<'syn>, node: &'syn Node<'syn>) -> AstNodeIterator<'syn, T> {
        AstNodeIterator {
            tree,
            iter: node.content.iter(),
            phantom: PhantomData,
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
                Some(NodeOrToken::Trivia(_)) => {}
                None => return None,
            }
        }
    }
}

macro_rules! ast_node_impl {
    ($name:ident, $kind_pat:pat) => {
        #[derive(Copy, Clone)]
        pub struct $name<'syn>(pub &'syn Node<'syn>);

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

            fn node(&self) -> &Node<'syn> {
                self.0
            }
        }

        impl<'syn> $name<'syn> {
            pub fn range(&self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
                self.0.find_range(tree)
            }
        }
    };
}

macro_rules! find_first {
    ($fn_name:ident, $find_ty:ident) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree<'syn>) -> Option<$find_ty<'syn>> {
            self.0.find_first(tree)
        }
    };
}

macro_rules! find_token {
    ($fn_name:ident, $find_token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree<'syn>) -> bool {
            self.0.find_token(tree, $find_token)
        }
    };
}

macro_rules! find_token_rev {
    ($fn_name:ident, $find_token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree<'syn>) -> bool {
            self.0.find_token_rev(tree, $find_token)
        }
    };
}

macro_rules! node_iter {
    ($fn_name:ident, $node_ty:ident) => {
        pub fn $fn_name(
            &self,
            tree: &'syn SyntaxTree<'syn>,
        ) -> AstNodeIterator<'syn, $node_ty<'syn>> {
            AstNodeIterator::new(tree, self.0)
        }
    };
}

ast_node_impl!(SourceFile, SyntaxKind::SOURCE_FILE);

ast_node_impl!(AttrList, SyntaxKind::ATTR_LIST);
ast_node_impl!(Attr, SyntaxKind::ATTR);
ast_node_impl!(AttrParamList, SyntaxKind::ATTR_PARAM_LIST);
ast_node_impl!(AttrParam, SyntaxKind::ATTR_PARAM);
ast_node_impl!(Visibility, SyntaxKind::VISIBILITY);

ast_node_impl!(ProcItem, SyntaxKind::PROC_ITEM);
ast_node_impl!(ParamList, SyntaxKind::PARAM_LIST);
ast_node_impl!(Param, SyntaxKind::PARAM);
ast_node_impl!(EnumItem, SyntaxKind::ENUM_ITEM);
ast_node_impl!(VariantList, SyntaxKind::VARIANT_LIST);
ast_node_impl!(Variant, SyntaxKind::VARIANT);
ast_node_impl!(VariantFieldList, SyntaxKind::VARIANT_FIELD_LIST);
ast_node_impl!(StructItem, SyntaxKind::STRUCT_ITEM);
ast_node_impl!(FieldList, SyntaxKind::FIELD_LIST);
ast_node_impl!(Field, SyntaxKind::FIELD);
ast_node_impl!(ConstItem, SyntaxKind::CONST_ITEM);
ast_node_impl!(GlobalItem, SyntaxKind::GLOBAL_ITEM);
ast_node_impl!(ImportItem, SyntaxKind::IMPORT_ITEM);
ast_node_impl!(ImportPath, SyntaxKind::IMPORT_PATH);
ast_node_impl!(ImportSymbolList, SyntaxKind::IMPORT_SYMBOL_LIST);
ast_node_impl!(ImportSymbol, SyntaxKind::IMPORT_SYMBOL);
ast_node_impl!(ImportSymbolRename, SyntaxKind::IMPORT_SYMBOL_RENAME);

ast_node_impl!(Name, SyntaxKind::NAME);
ast_node_impl!(Path, SyntaxKind::PATH);
ast_node_impl!(Bind, SyntaxKind::BIND);
ast_node_impl!(BindList, SyntaxKind::BIND_LIST);
ast_node_impl!(ArgsList, SyntaxKind::ARGS_LIST);

ast_node_impl!(TypeBasic, SyntaxKind::TYPE_BASIC);
ast_node_impl!(TypeCustom, SyntaxKind::TYPE_CUSTOM);
ast_node_impl!(TypeReference, SyntaxKind::TYPE_REFERENCE);
ast_node_impl!(TypeProcedure, SyntaxKind::TYPE_PROCEDURE);
ast_node_impl!(ParamTypeList, SyntaxKind::PARAM_TYPE_LIST);
ast_node_impl!(TypeArraySlice, SyntaxKind::TYPE_ARRAY_SLICE);
ast_node_impl!(TypeArrayStatic, SyntaxKind::TYPE_ARRAY_STATIC);

ast_node_impl!(Block, SyntaxKind::BLOCK);
ast_node_impl!(StmtBreak, SyntaxKind::STMT_BREAK);
ast_node_impl!(StmtContinue, SyntaxKind::STMT_CONTINUE);
ast_node_impl!(StmtReturn, SyntaxKind::STMT_RETURN);
ast_node_impl!(StmtDefer, SyntaxKind::STMT_DEFER);
ast_node_impl!(StmtLoop, SyntaxKind::STMT_LOOP);
ast_node_impl!(LoopWhileHeader, SyntaxKind::LOOP_WHILE_HEADER);
ast_node_impl!(LoopCLikeHeader, SyntaxKind::LOOP_CLIKE_HEADER);
ast_node_impl!(StmtLocal, SyntaxKind::STMT_LOCAL);
ast_node_impl!(StmtAssign, SyntaxKind::STMT_ASSIGN);
ast_node_impl!(StmtExprSemi, SyntaxKind::STMT_EXPR_SEMI);
ast_node_impl!(StmtExprTail, SyntaxKind::STMT_EXPR_TAIL);

ast_node_impl!(ExprParen, SyntaxKind::EXPR_PAREN);
ast_node_impl!(ExprIf, SyntaxKind::EXPR_IF);
ast_node_impl!(BranchEntry, SyntaxKind::BRANCH_ENTRY);
ast_node_impl!(BranchElseIf, SyntaxKind::BRANCH_ELSE_IF);
ast_node_impl!(ExprMatch, SyntaxKind::EXPR_MATCH);
ast_node_impl!(MatchArmList, SyntaxKind::MATCH_ARM_LIST);
ast_node_impl!(MatchArm, SyntaxKind::MATCH_ARM);
ast_node_impl!(ExprField, SyntaxKind::EXPR_FIELD);
ast_node_impl!(ExprIndex, SyntaxKind::EXPR_INDEX);
ast_node_impl!(ExprSlice, SyntaxKind::EXPR_SLICE);
ast_node_impl!(ExprCall, SyntaxKind::EXPR_CALL);
ast_node_impl!(ExprCast, SyntaxKind::EXPR_CAST);
ast_node_impl!(ExprSizeof, SyntaxKind::EXPR_SIZEOF);
ast_node_impl!(ExprItem, SyntaxKind::EXPR_ITEM);
ast_node_impl!(ExprVariant, SyntaxKind::EXPR_VARIANT);
ast_node_impl!(ExprStructInit, SyntaxKind::EXPR_STRUCT_INIT);
ast_node_impl!(FieldInitList, SyntaxKind::FIELD_INIT_LIST);
ast_node_impl!(FieldInit, SyntaxKind::FIELD_INIT);
ast_node_impl!(ExprArrayInit, SyntaxKind::EXPR_ARRAY_INIT);
ast_node_impl!(ExprArrayRepeat, SyntaxKind::EXPR_ARRAY_REPEAT);
ast_node_impl!(ExprDeref, SyntaxKind::EXPR_DEREF);
ast_node_impl!(ExprAddress, SyntaxKind::EXPR_ADDRESS);
ast_node_impl!(ExprUnary, SyntaxKind::EXPR_UNARY);
ast_node_impl!(ExprBinary, SyntaxKind::EXPR_BINARY);

ast_node_impl!(PatWild, SyntaxKind::PAT_WILD);
ast_node_impl!(PatLit, SyntaxKind::PAT_LIT);
ast_node_impl!(PatItem, SyntaxKind::PAT_ITEM);
ast_node_impl!(PatVariant, SyntaxKind::PAT_VARIANT);
ast_node_impl!(PatOr, SyntaxKind::PAT_OR);

ast_node_impl!(LitNull, SyntaxKind::LIT_NULL);
ast_node_impl!(LitBool, SyntaxKind::LIT_BOOL);
ast_node_impl!(LitInt, SyntaxKind::LIT_INT);
ast_node_impl!(LitFloat, SyntaxKind::LIT_FLOAT);
ast_node_impl!(LitChar, SyntaxKind::LIT_CHAR);
ast_node_impl!(LitString, SyntaxKind::LIT_STRING);

ast_node_impl!(RangeFull, SyntaxKind::RANGE_FULL);
ast_node_impl!(RangeToExclusive, SyntaxKind::RANGE_TO_EXCLUSIVE);
ast_node_impl!(RangeToInclusive, SyntaxKind::RANGE_TO_INCLUSIVE);
ast_node_impl!(RangeFrom, SyntaxKind::RANGE_FROM);
ast_node_impl!(RangeExclusive, SyntaxKind::RANGE_EXCLUSIVE);
ast_node_impl!(RangeInclusive, SyntaxKind::RANGE_INCLUSIVE);

#[derive(Copy, Clone)]
pub enum Item<'syn> {
    Proc(ProcItem<'syn>),
    Enum(EnumItem<'syn>),
    Struct(StructItem<'syn>),
    Const(ConstItem<'syn>),
    Global(GlobalItem<'syn>),
    Import(ImportItem<'syn>),
}

impl<'syn> AstNode<'syn> for Item<'syn> {
    fn cast(node: &'syn Node) -> Option<Item<'syn>> {
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
    fn node(&self) -> &Node<'syn> {
        match self {
            Item::Proc(item) => item.0,
            Item::Enum(item) => item.0,
            Item::Struct(item) => item.0,
            Item::Const(item) => item.0,
            Item::Global(item) => item.0,
            Item::Import(item) => item.0,
        }
    }
}

impl<'syn> Item<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Item::Proc(item) => item.range(tree),
            Item::Enum(item) => item.range(tree),
            Item::Struct(item) => item.range(tree),
            Item::Const(item) => item.range(tree),
            Item::Global(item) => item.range(tree),
            Item::Import(item) => item.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Type<'syn> {
    Basic(TypeBasic<'syn>),
    Custom(TypeCustom<'syn>),
    Reference(TypeReference<'syn>),
    Procedure(TypeProcedure<'syn>),
    ArraySlice(TypeArraySlice<'syn>),
    ArrayStatic(TypeArrayStatic<'syn>),
}

impl<'syn> AstNode<'syn> for Type<'syn> {
    fn cast(node: &'syn Node) -> Option<Type<'syn>> {
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
    fn node(&self) -> &Node<'syn> {
        match self {
            Type::Basic(ty) => ty.0,
            Type::Custom(ty) => ty.0,
            Type::Reference(ty) => ty.0,
            Type::Procedure(ty) => ty.0,
            Type::ArraySlice(ty) => ty.0,
            Type::ArrayStatic(ty) => ty.0,
        }
    }
}

impl<'syn> Type<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Type::Basic(ty) => ty.range(tree),
            Type::Custom(ty) => ty.range(tree),
            Type::Reference(ty) => ty.range(tree),
            Type::Procedure(ty) => ty.range(tree),
            Type::ArraySlice(ty) => ty.range(tree),
            Type::ArrayStatic(ty) => ty.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
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
    fn cast(node: &'syn Node) -> Option<Stmt<'syn>> {
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
    fn node(&self) -> &Node<'syn> {
        match self {
            Stmt::Break(stmt) => stmt.0,
            Stmt::Continue(stmt) => stmt.0,
            Stmt::Return(stmt) => stmt.0,
            Stmt::Defer(stmt) => stmt.0,
            Stmt::Loop(stmt) => stmt.0,
            Stmt::Local(stmt) => stmt.0,
            Stmt::Assign(stmt) => stmt.0,
            Stmt::ExprSemi(stmt) => stmt.0,
            Stmt::ExprTail(stmt) => stmt.0,
        }
    }
}

impl<'syn> Stmt<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Stmt::Break(stmt) => stmt.range(tree),
            Stmt::Continue(stmt) => stmt.range(tree),
            Stmt::Return(stmt) => stmt.range(tree),
            Stmt::Defer(stmt) => stmt.range(tree),
            Stmt::Loop(stmt) => stmt.range(tree),
            Stmt::Local(stmt) => stmt.range(tree),
            Stmt::Assign(stmt) => stmt.range(tree),
            Stmt::ExprSemi(stmt) => stmt.range(tree),
            Stmt::ExprTail(stmt) => stmt.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Expr<'syn> {
    Paren(ExprParen<'syn>),
    Lit(Lit<'syn>),
    If(ExprIf<'syn>),
    Block(Block<'syn>),
    Match(ExprMatch<'syn>),
    Field(ExprField<'syn>),
    Index(ExprIndex<'syn>),
    Slice(ExprSlice<'syn>),
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
    Range(Range<'syn>),
    Unary(ExprUnary<'syn>),
    Binary(ExprBinary<'syn>),
}

impl<'syn> AstNode<'syn> for Expr<'syn> {
    fn cast(node: &'syn Node) -> Option<Expr<'syn>> {
        match node.kind {
            SyntaxKind::EXPR_PAREN => Some(Expr::Paren(ExprParen(node))),
            SyntaxKind::EXPR_IF => Some(Expr::If(ExprIf(node))),
            SyntaxKind::EXPR_MATCH => Some(Expr::Match(ExprMatch(node))),
            SyntaxKind::EXPR_FIELD => Some(Expr::Field(ExprField(node))),
            SyntaxKind::EXPR_INDEX => Some(Expr::Index(ExprIndex(node))),
            SyntaxKind::EXPR_SLICE => Some(Expr::Slice(ExprSlice(node))),
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
            _ => {
                if let Some(lit) = Lit::cast(node) {
                    Some(Expr::Lit(lit))
                } else if let Some(range) = Range::cast(node) {
                    Some(Expr::Range(range))
                } else if let Some(block) = Block::cast(node) {
                    Some(Expr::Block(block))
                } else {
                    None
                }
            }
        }
    }
    fn node(&self) -> &Node<'syn> {
        match self {
            Expr::Paren(expr) => expr.0,
            Expr::Lit(lit) => lit.node(),
            Expr::If(expr) => expr.0,
            Expr::Block(block) => block.0,
            Expr::Match(expr) => expr.0,
            Expr::Field(expr) => expr.0,
            Expr::Index(expr) => expr.0,
            Expr::Slice(expr) => expr.0,
            Expr::Call(expr) => expr.0,
            Expr::Cast(expr) => expr.0,
            Expr::Sizeof(expr) => expr.0,
            Expr::Item(expr) => expr.0,
            Expr::Variant(expr) => expr.0,
            Expr::StructInit(expr) => expr.0,
            Expr::ArrayInit(expr) => expr.0,
            Expr::ArrayRepeat(expr) => expr.0,
            Expr::Deref(expr) => expr.0,
            Expr::Address(expr) => expr.0,
            Expr::Range(range) => range.node(),
            Expr::Unary(expr) => expr.0,
            Expr::Binary(expr) => expr.0,
        }
    }
}

impl<'syn> Expr<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Expr::Paren(expr) => expr.range(tree),
            Expr::Lit(expr) => expr.range(tree),
            Expr::If(expr) => expr.range(tree),
            Expr::Block(expr) => expr.range(tree),
            Expr::Match(expr) => expr.range(tree),
            Expr::Field(expr) => expr.range(tree),
            Expr::Index(expr) => expr.range(tree),
            Expr::Slice(expr) => expr.range(tree),
            Expr::Call(expr) => expr.range(tree),
            Expr::Cast(expr) => expr.range(tree),
            Expr::Sizeof(expr) => expr.range(tree),
            Expr::Item(expr) => expr.range(tree),
            Expr::Variant(expr) => expr.range(tree),
            Expr::StructInit(expr) => expr.range(tree),
            Expr::ArrayInit(expr) => expr.range(tree),
            Expr::ArrayRepeat(expr) => expr.range(tree),
            Expr::Deref(expr) => expr.range(tree),
            Expr::Address(expr) => expr.range(tree),
            Expr::Range(expr) => expr.range(tree),
            Expr::Unary(expr) => expr.range(tree),
            Expr::Binary(expr) => expr.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Pat<'syn> {
    Wild(PatWild<'syn>),
    Lit(PatLit<'syn>),
    Item(PatItem<'syn>),
    Variant(PatVariant<'syn>),
    Or(PatOr<'syn>),
}

impl<'syn> AstNode<'syn> for Pat<'syn> {
    fn cast(node: &'syn Node) -> Option<Pat<'syn>> {
        match node.kind {
            SyntaxKind::PAT_WILD => Some(Pat::Wild(PatWild(node))),
            SyntaxKind::PAT_LIT => Some(Pat::Lit(PatLit(node))),
            SyntaxKind::PAT_ITEM => Some(Pat::Item(PatItem(node))),
            SyntaxKind::PAT_VARIANT => Some(Pat::Variant(PatVariant(node))),
            SyntaxKind::PAT_OR => Some(Pat::Or(PatOr(node))),
            _ => None,
        }
    }
    fn node(&self) -> &Node<'syn> {
        match self {
            Pat::Wild(pat) => pat.0,
            Pat::Lit(pat) => pat.0,
            Pat::Item(pat) => pat.0,
            Pat::Variant(pat) => pat.0,
            Pat::Or(pat) => pat.0,
        }
    }
}

impl<'syn> Pat<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Pat::Wild(pat) => pat.range(tree),
            Pat::Lit(pat) => pat.range(tree),
            Pat::Item(pat) => pat.range(tree),
            Pat::Variant(pat) => pat.range(tree),
            Pat::Or(pat) => pat.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Lit<'syn> {
    Null(LitNull<'syn>),
    Bool(LitBool<'syn>),
    Int(LitInt<'syn>),
    Float(LitFloat<'syn>),
    Char(LitChar<'syn>),
    String(LitString<'syn>),
}

impl<'syn> AstNode<'syn> for Lit<'syn> {
    fn cast(node: &'syn Node) -> Option<Lit<'syn>> {
        match node.kind {
            SyntaxKind::LIT_NULL => Some(Lit::Null(LitNull(node))),
            SyntaxKind::LIT_BOOL => Some(Lit::Bool(LitBool(node))),
            SyntaxKind::LIT_INT => Some(Lit::Int(LitInt(node))),
            SyntaxKind::LIT_FLOAT => Some(Lit::Float(LitFloat(node))),
            SyntaxKind::LIT_CHAR => Some(Lit::Char(LitChar(node))),
            SyntaxKind::LIT_STRING => Some(Lit::String(LitString(node))),
            _ => None,
        }
    }
    fn node(&self) -> &Node<'syn> {
        match self {
            Lit::Null(lit) => lit.0,
            Lit::Bool(lit) => lit.0,
            Lit::Int(lit) => lit.0,
            Lit::Float(lit) => lit.0,
            Lit::Char(lit) => lit.0,
            Lit::String(lit) => lit.0,
        }
    }
}

impl<'syn> Lit<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Lit::Null(lit) => lit.range(tree),
            Lit::Bool(lit) => lit.range(tree),
            Lit::Int(lit) => lit.range(tree),
            Lit::Float(lit) => lit.range(tree),
            Lit::Char(lit) => lit.range(tree),
            Lit::String(lit) => lit.range(tree),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Range<'syn> {
    Full(RangeFull<'syn>),
    ToExclusive(RangeToExclusive<'syn>),
    ToInclusive(RangeToInclusive<'syn>),
    From(RangeFrom<'syn>),
    Exclusive(RangeExclusive<'syn>),
    Inclusive(RangeInclusive<'syn>),
}

impl<'syn> AstNode<'syn> for Range<'syn> {
    fn cast(node: &'syn Node) -> Option<Range<'syn>> {
        match node.kind {
            SyntaxKind::RANGE_FULL => Some(Range::Full(RangeFull(node))),
            SyntaxKind::RANGE_TO_EXCLUSIVE => Some(Range::ToExclusive(RangeToExclusive(node))),
            SyntaxKind::RANGE_TO_INCLUSIVE => Some(Range::ToInclusive(RangeToInclusive(node))),
            SyntaxKind::RANGE_FROM => Some(Range::From(RangeFrom(node))),
            SyntaxKind::RANGE_EXCLUSIVE => Some(Range::Exclusive(RangeExclusive(node))),
            SyntaxKind::RANGE_INCLUSIVE => Some(Range::Inclusive(RangeInclusive(node))),
            _ => None,
        }
    }
    fn node(&self) -> &Node<'syn> {
        match self {
            Range::Full(range) => range.0,
            Range::ToExclusive(range) => range.0,
            Range::ToInclusive(range) => range.0,
            Range::From(range) => range.0,
            Range::Exclusive(range) => range.0,
            Range::Inclusive(range) => range.0,
        }
    }
}

impl<'syn> Range<'syn> {
    pub fn range(self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
        match self {
            Range::Full(range) => range.range(tree),
            Range::ToExclusive(range) => range.range(tree),
            Range::ToInclusive(range) => range.range(tree),
            Range::From(range) => range.range(tree),
            Range::Exclusive(range) => range.range(tree),
            Range::Inclusive(range) => range.range(tree),
        }
    }
}

impl<'syn> SourceFile<'syn> {
    node_iter!(items, Item);
}

impl<'syn> AttrList<'syn> {
    node_iter!(attrs, Attr);
}

impl<'syn> Attr<'syn> {
    find_first!(name, Name);
    find_first!(param_list, AttrParamList);
}

impl<'syn> AttrParamList<'syn> {
    node_iter!(params, AttrParam);
}

impl<'syn> AttrParam<'syn> {
    find_first!(name, Name);
    find_first!(value, LitString);
}

impl<'syn> Visibility<'syn> {
    find_token!(is_pub, T![pub]);
}

impl<'syn> ProcItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility);
    find_first!(name, Name);
    find_first!(param_list, ParamList);
    find_first!(return_ty, Type);
    find_first!(block, Block);
}

impl<'syn> ParamList<'syn> {
    node_iter!(params, Param);
    find_token_rev!(is_variadic, T![..]);
}

impl<'syn> Param<'syn> {
    find_token!(is_mut, T![mut]);
    find_first!(name, Name);
    find_first!(ty, Type);
}

impl<'syn> EnumItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility);
    find_first!(name, Name);
    find_first!(variant_list, VariantList);
}

impl<'syn> VariantList<'syn> {
    node_iter!(variants, Variant);
}

impl<'syn> Variant<'syn> {
    find_first!(name, Name);
    find_first!(value, Expr);
    find_first!(field_list, VariantFieldList);
}

impl<'syn> VariantFieldList<'syn> {
    node_iter!(fields, Type);
}

impl<'syn> StructItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility);
    find_first!(name, Name);
    find_first!(field_list, FieldList);
}

impl<'syn> FieldList<'syn> {
    node_iter!(fields, Field);
}

impl<'syn> Field<'syn> {
    find_first!(visibility, Visibility);
    find_first!(name, Name);
    find_first!(ty, Type);
}

impl<'syn> ConstItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility);
    find_first!(name, Name);
    find_first!(ty, Type);
    find_first!(value, Expr);
}

impl<'syn> GlobalItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility);
    find_token!(is_mut, T![mut]);
    find_first!(name, Name);
    find_first!(ty, Type);
    find_first!(value, Expr);
}

impl<'syn> ImportItem<'syn> {
    find_first!(attr_list, AttrList);
    find_first!(visibility, Visibility); //@exists but ignored
    find_first!(package, Name);
    find_first!(import_path, ImportPath);
    find_first!(rename, ImportSymbolRename);
    find_first!(import_symbol_list, ImportSymbolList);
}

impl<'syn> ImportPath<'syn> {
    node_iter!(names, Name);
}

impl<'syn> ImportSymbolList<'syn> {
    node_iter!(import_symbols, ImportSymbol);
}

impl<'syn> ImportSymbol<'syn> {
    find_first!(name, Name);
    find_first!(rename, ImportSymbolRename);
}

impl<'syn> ImportSymbolRename<'syn> {
    find_first!(alias, Name);

    pub fn discard(&self, tree: &'syn SyntaxTree<'syn>) -> ((), TextRange) {
        self.0.find_by_token_with_range(tree, |_| Some(())).unwrap()
    }
}

impl<'syn> Name<'syn> {}

impl<'syn> Path<'syn> {
    node_iter!(names, Name);
}

//@find `_` token range
impl<'syn> Bind<'syn> {
    find_token!(is_mut, T![mut]);
    find_first!(name, Name);
}

impl<'syn> BindList<'syn> {
    node_iter!(binds, Bind);
}

impl<'syn> ArgsList<'syn> {
    node_iter!(exprs, Expr);
}

impl<'syn> TypeBasic<'syn> {
    pub fn basic(&self, tree: &'syn SyntaxTree<'syn>) -> ast::BasicType {
        self.0.find_by_token(tree, Token::as_basic_type).unwrap()
    }
    pub fn basic_with_range(&self, tree: &'syn SyntaxTree<'syn>) -> (ast::BasicType, TextRange) {
        self.0
            .find_by_token_with_range(tree, Token::as_basic_type)
            .unwrap()
    }
}

impl<'syn> TypeCustom<'syn> {
    find_first!(path, Path);
}

impl<'syn> TypeReference<'syn> {
    find_token!(is_mut, T![mut]);
    find_first!(ref_ty, Type);
}

impl<'syn> TypeProcedure<'syn> {
    find_first!(type_list, ParamTypeList);
    find_first!(return_ty, Type);
}

impl<'syn> ParamTypeList<'syn> {
    node_iter!(types, Type);
    find_token_rev!(is_variadic, T![..]);
}

impl<'syn> TypeArraySlice<'syn> {
    find_token!(is_mut, T![mut]);
    find_first!(elem_ty, Type);
}

impl<'syn> TypeArrayStatic<'syn> {
    find_first!(len, Expr);
    find_first!(elem_ty, Type);
}

impl<'syn> Block<'syn> {
    node_iter!(stmts, Stmt);
}

impl<'syn> StmtBreak<'syn> {}

impl<'syn> StmtContinue<'syn> {}

impl<'syn> StmtReturn<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> StmtDefer<'syn> {
    find_first!(block, Block);
}

impl<'syn> StmtLoop<'syn> {
    find_first!(while_header, LoopWhileHeader);
    find_first!(clike_header, LoopCLikeHeader);
    find_first!(block, Block);
}

impl<'syn> LoopWhileHeader<'syn> {
    find_first!(cond, Expr);
}

impl<'syn> LoopCLikeHeader<'syn> {
    find_first!(local, StmtLocal);
    find_first!(cond, Expr);
    find_first!(assign, StmtAssign);
}

impl<'syn> StmtLocal<'syn> {
    find_first!(bind, Bind);
    find_first!(ty, Type);
    find_first!(init, Expr);
}

impl<'syn> StmtAssign<'syn> {
    pub fn assign_op(&self, tree: &'syn SyntaxTree<'syn>) -> Option<ast::AssignOp> {
        self.0.find_by_token(tree, Token::as_assign_op)
    }
    pub fn assign_op_with_range(
        &self,
        tree: &'syn SyntaxTree<'syn>,
    ) -> Option<(ast::AssignOp, TextRange)> {
        self.0.find_by_token_with_range(tree, Token::as_assign_op)
    }
    //@ambiguity in incomplete tree
    node_iter!(lhs_rhs_iter, Expr);
}

impl<'syn> StmtExprSemi<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> StmtExprTail<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> ExprParen<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> ExprIf<'syn> {
    find_first!(entry_branch, BranchEntry);
    node_iter!(else_if_branches, BranchElseIf);
    find_first!(else_block, Block);
}

impl<'syn> BranchEntry<'syn> {
    find_first!(cond, Expr);
    find_first!(block, Block);
}

impl<'syn> BranchElseIf<'syn> {
    find_first!(cond, Expr);
    find_first!(block, Block);
}

impl<'syn> ExprMatch<'syn> {
    find_first!(on_expr, Expr);
    find_first!(match_arm_list, MatchArmList);
}

impl<'syn> MatchArmList<'syn> {
    node_iter!(match_arms, MatchArm);
}

impl<'syn> MatchArm<'syn> {
    find_first!(pat, Pat);
    find_first!(expr, Expr);
}

impl<'syn> ExprField<'syn> {
    find_first!(target, Expr);
    find_first!(name, Name);
}

impl<'syn> ExprIndex<'syn> {
    node_iter!(target_index_iter, Expr);
}

impl<'syn> ExprSlice<'syn> {
    find_token!(is_mut, T![mut]);
    node_iter!(target_range_iter, Expr);
}

impl<'syn> ExprCall<'syn> {
    find_first!(target, Expr);
    find_first!(args_list, ArgsList);
}

impl<'syn> ExprCast<'syn> {
    find_first!(target, Expr);
    find_first!(into_ty, Type);
}

impl<'syn> ExprSizeof<'syn> {
    find_first!(ty, Type);
}

impl<'syn> ExprItem<'syn> {
    find_first!(path, Path);
    find_first!(args_list, ArgsList);
}

impl<'syn> ExprVariant<'syn> {
    find_first!(name, Name);
    find_first!(args_list, ArgsList);
}

impl<'syn> ExprStructInit<'syn> {
    find_first!(path, Path);
    find_first!(field_init_list, FieldInitList);
}

impl<'syn> FieldInitList<'syn> {
    node_iter!(field_inits, FieldInit);
}

impl<'syn> FieldInit<'syn> {
    find_first!(name, Name);
    find_first!(expr, Expr);
}

impl<'syn> ExprArrayInit<'syn> {
    node_iter!(inputs, Expr);
}

impl<'syn> ExprArrayRepeat<'syn> {
    //@ambiguity in incomplete tree
    node_iter!(expr_len_iter, Expr);
}

impl<'syn> ExprDeref<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> ExprAddress<'syn> {
    find_token!(is_mut, T![mut]);
    find_first!(expr, Expr);
}

impl<'syn> ExprUnary<'syn> {
    pub fn un_op(&self, tree: &'syn SyntaxTree<'syn>) -> ast::UnOp {
        self.0.find_by_token(tree, Token::as_un_op).unwrap()
    }
    pub fn un_op_with_range(&self, tree: &'syn SyntaxTree<'syn>) -> (ast::UnOp, TextRange) {
        self.0
            .find_by_token_with_range(tree, Token::as_un_op)
            .unwrap()
    }
    find_first!(rhs, Expr);
}

//@support no range versions of `find_by_token`?
// fmt doesnt need ranges and some other uses
impl<'syn> ExprBinary<'syn> {
    pub fn bin_op(&self, tree: &'syn SyntaxTree<'syn>) -> ast::BinOp {
        self.0.find_by_token(tree, Token::as_bin_op).unwrap()
    }
    pub fn bin_op_with_range(&self, tree: &'syn SyntaxTree<'syn>) -> (ast::BinOp, TextRange) {
        self.0
            .find_by_token_with_range(tree, Token::as_bin_op)
            .unwrap()
    }
    //@ambiguity in incomplete tree
    node_iter!(lhs_rhs_iter, Expr);
}

impl<'syn> LitNull<'syn> {}

impl<'syn> LitBool<'syn> {
    pub fn value(&self, tree: &'syn SyntaxTree<'syn>) -> bool {
        self.0.find_by_token(tree, Token::as_bool).unwrap()
    }
}

//==================== PAT ====================

impl<'syn> PatWild<'syn> {}

impl<'syn> PatLit<'syn> {
    find_first!(lit, Lit);
}

impl<'syn> PatItem<'syn> {
    find_first!(path, Path);
    find_first!(bind_list, BindList);
}

impl<'syn> PatVariant<'syn> {
    find_first!(name, Name);
    find_first!(bind_list, BindList);
}

impl<'syn> PatOr<'syn> {
    node_iter!(patterns, Pat);
}

//==================== LIT ====================

impl<'syn> LitInt<'syn> {}

impl<'syn> LitFloat<'syn> {}

impl<'syn> LitChar<'syn> {}

impl<'syn> LitString<'syn> {}

//==================== RANGE ====================

impl<'syn> RangeFull<'syn> {}

impl<'syn> RangeToExclusive<'syn> {
    find_first!(end, Expr);
}

impl<'syn> RangeToInclusive<'syn> {
    find_first!(end, Expr);
}

impl<'syn> RangeFrom<'syn> {
    find_first!(start, Expr);
}

impl<'syn> RangeExclusive<'syn> {
    node_iter!(start_end_iter, Expr);
}

impl<'syn> RangeInclusive<'syn> {
    node_iter!(start_end_iter, Expr);
}

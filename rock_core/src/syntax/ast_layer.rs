use super::token::{Token, TokenID, T};
use super::tree::{Node, NodeContentIter, NodeOrToken, SyntaxKind, SyntaxTree};
use crate::ast;
use crate::text::TextRange;
use std::marker::PhantomData;

pub trait AstNode<'syn>: Copy + Clone + Sized {
    fn cast(node: &'syn Node) -> Option<Self>;
    fn range(self) -> TextRange;
}

#[derive(Clone)]
pub struct AstNodeIterator<'syn, T: AstNode<'syn>> {
    iter: NodeContentIter<'syn>,
    phantom: PhantomData<T>,
}

impl<'syn, T: AstNode<'syn>> AstNodeIterator<'syn, T> {
    fn new(tree: &'syn SyntaxTree, node: &'syn Node) -> AstNodeIterator<'syn, T> {
        AstNodeIterator { iter: tree.content(node), phantom: PhantomData }
    }
}

impl<'syn, T: AstNode<'syn>> Iterator for AstNodeIterator<'syn, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(NodeOrToken::Node(node_id)) => {
                    let node = self.iter.tree.node(node_id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                Some(NodeOrToken::Token(_)) => {}
                Some(NodeOrToken::Trivia(_)) => {}
                None => return None,
            }
        }
    }
}

impl SyntaxTree {
    pub fn source_file(&self) -> SourceFile {
        let root = self.root();
        SourceFile::cast(root).unwrap()
    }
}

impl Node {
    fn node_find<'syn, T: AstNode<'syn>>(&self, tree: &'syn SyntaxTree) -> Option<T> {
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(_) => {}
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn node_after_token<'syn, T: AstNode<'syn>>(
        &self,
        tree: &'syn SyntaxTree,
        after: Token,
    ) -> Option<T> {
        let mut found = false;
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    if !found {
                        continue;
                    }
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(id) => {
                    if found {
                        continue;
                    }
                    let token = tree.tokens.token(id);
                    found = token == after;
                }
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn node_after_token_predicate<'syn, T: AstNode<'syn>, F, P>(
        &self,
        tree: &'syn SyntaxTree,
        predicate: F,
    ) -> Option<T>
    where
        F: Fn(Token) -> Option<P>,
    {
        let mut found = false;
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    if !found {
                        continue;
                    }
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(id) => {
                    if found {
                        continue;
                    }
                    let token = tree.tokens.token(id);
                    found = predicate(token).is_some();
                }
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn node_before_token<'syn, T: AstNode<'syn>>(
        &self,
        tree: &'syn SyntaxTree,
        before: Token,
    ) -> Option<T> {
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(id) => {
                    let token = tree.tokens.token(id);
                    if token == before {
                        return None;
                    }
                }
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn node_before_token_predicate<'syn, T: AstNode<'syn>, F, P>(
        &self,
        tree: &'syn SyntaxTree,
        predicate: F,
    ) -> Option<T>
    where
        F: Fn(Token) -> Option<P>,
    {
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(id) => {
                    let token = tree.tokens.token(id);
                    if predicate(token).is_some() {
                        return None;
                    }
                }
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn node_between_tokens<'syn, T: AstNode<'syn>>(
        &self,
        tree: &'syn SyntaxTree,
        after: Token,
        before: Token,
    ) -> Option<T> {
        let mut found_start = false;
        for not in tree.content(self) {
            match not {
                NodeOrToken::Node(id) => {
                    if !found_start {
                        continue;
                    }
                    let node = tree.node(id);
                    if let Some(cst_node) = T::cast(node) {
                        return Some(cst_node);
                    }
                }
                NodeOrToken::Token(id) => {
                    let token = tree.tokens.token(id);
                    if !found_start && token == after {
                        found_start = true;
                    }
                    if token == before {
                        return None;
                    }
                }
                NodeOrToken::Trivia(_) => {}
            }
        }
        None
    }

    fn token_find(&self, tree: &SyntaxTree, find: Token) -> Option<TextRange> {
        for not in tree.content(self) {
            if let NodeOrToken::Token(id) = not {
                let (token, range) = tree.tokens.token_and_range(id);
                if token == find {
                    return Some(range);
                }
            }
        }
        None
    }

    fn token_find_rev(&self, tree: &SyntaxTree, find: Token) -> Option<TextRange> {
        for not in tree.content(self).rev() {
            if let NodeOrToken::Token(id) = not {
                let (token, range) = tree.tokens.token_and_range(id);
                if token == find {
                    return Some(range);
                }
            }
        }
        None
    }

    fn token_find_predicate<T, F>(&self, tree: &SyntaxTree, predicate: F) -> Option<(T, TextRange)>
    where
        F: Fn(Token) -> Option<T>,
    {
        for not in tree.content(self) {
            if let NodeOrToken::Token(id) = not {
                let (token, range) = tree.tokens.token_and_range(id);
                if let Some(value) = predicate(token) {
                    return Some((value, range));
                }
            }
        }
        None
    }

    fn token_find_id(&self, tree: &SyntaxTree, find: Token) -> Option<TokenID> {
        for not in tree.content(self) {
            if let NodeOrToken::Token(id) = not {
                let token = tree.tokens.token(id);
                if token == find {
                    return Some(id);
                }
            }
        }
        None
    }
}

//==================== AST NODE MACROS ====================

macro_rules! ast_node_impl {
    ($name:ident, $kind_pat:pat) => {
        #[derive(Copy, Clone)]
        pub struct $name<'syn>(pub &'syn Node);

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
            #[inline]
            fn range(self) -> TextRange {
                self.0.range
            }
        }
    };
}

macro_rules! node_iter {
    ($fn_name:ident, $node_ty:ident) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> AstNodeIterator<'syn, $node_ty<'syn>> {
            AstNodeIterator::new(tree, self.0)
        }
    };
}

macro_rules! node_find {
    ($fn_name:ident, $find_ty:ident) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_find(tree)
        }
    };
}

macro_rules! node_after_token {
    ($fn_name:ident, $find_ty:ident, $token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_after_token(tree, $token)
        }
    };
}

macro_rules! node_after_token_predicate {
    ($fn_name:ident, $find_ty:ident, $predicate:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_after_token_predicate(tree, $predicate)
        }
    };
}

macro_rules! node_before_token {
    ($fn_name:ident, $find_ty:ident, $token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_before_token(tree, $token)
        }
    };
}

macro_rules! node_before_token_predicate {
    ($fn_name:ident, $find_ty:ident, $predicate:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_before_token_predicate(tree, $predicate)
        }
    };
}

macro_rules! node_between_tokens {
    ($fn_name:ident, $find_ty:ident, $after:expr, $before:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<$find_ty<'syn>> {
            self.0.node_between_tokens(tree, $after, $before)
        }
    };
}

macro_rules! token_find {
    ($fn_name:ident, $find_token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<TextRange> {
            self.0.token_find(tree, $find_token)
        }
    };
}

macro_rules! token_find_rev {
    ($fn_name:ident, $find_token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<TextRange> {
            self.0.token_find_rev(tree, $find_token)
        }
    };
}

macro_rules! token_find_predicate {
    ($fn_name:ident, $predicate:expr, $pred_ty:ty) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<($pred_ty, TextRange)> {
            self.0.token_find_predicate(tree, $predicate)
        }
    };
}

macro_rules! token_find_id {
    ($fn_name:ident, $find_token:expr) => {
        pub fn $fn_name(&self, tree: &'syn SyntaxTree) -> Option<TokenID> {
            self.0.token_find_id(tree, $find_token)
        }
    };
}

//==================== AST NODE IMPL ====================

ast_node_impl!(SourceFile, SyntaxKind::SOURCE_FILE);

ast_node_impl!(ProcItem, SyntaxKind::PROC_ITEM);
ast_node_impl!(ParamList, SyntaxKind::PARAM_LIST);
ast_node_impl!(Param, SyntaxKind::PARAM);
ast_node_impl!(EnumItem, SyntaxKind::ENUM_ITEM);
ast_node_impl!(VariantList, SyntaxKind::VARIANT_LIST);
ast_node_impl!(Variant, SyntaxKind::VARIANT);
ast_node_impl!(VariantFieldList, SyntaxKind::VARIANT_FIELD_LIST);
ast_node_impl!(VariantField, SyntaxKind::VARIANT_FIELD);
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

ast_node_impl!(DirectiveList, SyntaxKind::DIRECTIVE_LIST);
ast_node_impl!(DirectiveSimple, SyntaxKind::DIRECTIVE_SIMPLE);
ast_node_impl!(DirectiveWithParams, SyntaxKind::DIRECTIVE_WITH_PARAMS);
ast_node_impl!(DirectiveParamList, SyntaxKind::DIRECTIVE_PARAM_LIST);
ast_node_impl!(DirectiveParam, SyntaxKind::DIRECTIVE_PARAM);

ast_node_impl!(TypeBasic, SyntaxKind::TYPE_BASIC);
ast_node_impl!(TypeCustom, SyntaxKind::TYPE_CUSTOM);
ast_node_impl!(TypeReference, SyntaxKind::TYPE_REFERENCE);
ast_node_impl!(TypeMultiReference, SyntaxKind::TYPE_MULTI_REFERENCE);
ast_node_impl!(TypeProcedure, SyntaxKind::TYPE_PROCEDURE);
ast_node_impl!(ProcTypeParamList, SyntaxKind::PROC_TYPE_PARAM_LIST);
ast_node_impl!(ProcTypeParam, SyntaxKind::PROC_TYPE_PARAM);
ast_node_impl!(TypeArraySlice, SyntaxKind::TYPE_ARRAY_SLICE);
ast_node_impl!(TypeArrayStatic, SyntaxKind::TYPE_ARRAY_STATIC);

ast_node_impl!(Block, SyntaxKind::BLOCK);
ast_node_impl!(StmtBreak, SyntaxKind::STMT_BREAK);
ast_node_impl!(StmtContinue, SyntaxKind::STMT_CONTINUE);
ast_node_impl!(StmtReturn, SyntaxKind::STMT_RETURN);
ast_node_impl!(StmtDefer, SyntaxKind::STMT_DEFER);
ast_node_impl!(StmtFor, SyntaxKind::STMT_FOR);
ast_node_impl!(ForBind, SyntaxKind::FOR_BIND);
ast_node_impl!(ForHeaderCond, SyntaxKind::FOR_HEADER_COND);
ast_node_impl!(ForHeaderElem, SyntaxKind::FOR_HEADER_ELEM);
ast_node_impl!(ForHeaderRange, SyntaxKind::FOR_HEADER_RANGE);
ast_node_impl!(ForHeaderPat, SyntaxKind::FOR_HEADER_PAT);
ast_node_impl!(StmtLocal, SyntaxKind::STMT_LOCAL);
ast_node_impl!(StmtAssign, SyntaxKind::STMT_ASSIGN);
ast_node_impl!(StmtExprSemi, SyntaxKind::STMT_EXPR_SEMI);
ast_node_impl!(StmtExprTail, SyntaxKind::STMT_EXPR_TAIL);
ast_node_impl!(StmtWithDirective, SyntaxKind::STMT_WITH_DIRECTIVE);

ast_node_impl!(ExprParen, SyntaxKind::EXPR_PAREN);
ast_node_impl!(ExprIf, SyntaxKind::EXPR_IF);
ast_node_impl!(BranchCond, SyntaxKind::BRANCH_COND);
ast_node_impl!(BranchPat, SyntaxKind::BRANCH_PAT);
ast_node_impl!(ExprMatch, SyntaxKind::EXPR_MATCH);
ast_node_impl!(MatchArmList, SyntaxKind::MATCH_ARM_LIST);
ast_node_impl!(MatchArm, SyntaxKind::MATCH_ARM);
ast_node_impl!(ExprField, SyntaxKind::EXPR_FIELD);
ast_node_impl!(ExprIndex, SyntaxKind::EXPR_INDEX);
ast_node_impl!(ExprSlice, SyntaxKind::EXPR_SLICE);
ast_node_impl!(ExprCall, SyntaxKind::EXPR_CALL);
ast_node_impl!(ExprCast, SyntaxKind::EXPR_CAST);
ast_node_impl!(ExprItem, SyntaxKind::EXPR_ITEM);
ast_node_impl!(ExprVariant, SyntaxKind::EXPR_VARIANT);
ast_node_impl!(ExprStructInit, SyntaxKind::EXPR_STRUCT_INIT);
ast_node_impl!(FieldInitList, SyntaxKind::FIELD_INIT_LIST);
ast_node_impl!(FieldInit, SyntaxKind::FIELD_INIT);
ast_node_impl!(ExprArrayInit, SyntaxKind::EXPR_ARRAY_INIT);
ast_node_impl!(ArrayInit, SyntaxKind::ARRAY_INIT);
ast_node_impl!(ExprArrayRepeat, SyntaxKind::EXPR_ARRAY_REPEAT);
ast_node_impl!(ExprTry, SyntaxKind::EXPR_TRY);
ast_node_impl!(ExprDeref, SyntaxKind::EXPR_DEREF);
ast_node_impl!(ExprAddress, SyntaxKind::EXPR_ADDRESS);
ast_node_impl!(ExprUnary, SyntaxKind::EXPR_UNARY);
ast_node_impl!(ExprBinary, SyntaxKind::EXPR_BINARY);

ast_node_impl!(PatWild, SyntaxKind::PAT_WILD);
ast_node_impl!(PatLit, SyntaxKind::PAT_LIT);
ast_node_impl!(PatItem, SyntaxKind::PAT_ITEM);
ast_node_impl!(PatVariant, SyntaxKind::PAT_VARIANT);
ast_node_impl!(PatOr, SyntaxKind::PAT_OR);

ast_node_impl!(LitVoid, SyntaxKind::LIT_VOID);
ast_node_impl!(LitNull, SyntaxKind::LIT_NULL);
ast_node_impl!(LitBool, SyntaxKind::LIT_BOOL);
ast_node_impl!(LitInt, SyntaxKind::LIT_INT);
ast_node_impl!(LitFloat, SyntaxKind::LIT_FLOAT);
ast_node_impl!(LitChar, SyntaxKind::LIT_CHAR);
ast_node_impl!(LitString, SyntaxKind::LIT_STRING);

ast_node_impl!(Name, SyntaxKind::NAME);
ast_node_impl!(Bind, SyntaxKind::BIND);
ast_node_impl!(BindList, SyntaxKind::BIND_LIST);
ast_node_impl!(ArgsList, SyntaxKind::ARGS_LIST);
ast_node_impl!(Path, SyntaxKind::PATH);
ast_node_impl!(PathSegment, SyntaxKind::PATH_SEGMENT);
ast_node_impl!(PolymorphArgs, SyntaxKind::POLYMORPH_ARGS);
ast_node_impl!(PolymorphParams, SyntaxKind::POLYMORPH_PARAMS);

#[derive(Copy, Clone)]
pub enum Item<'syn> {
    Proc(ProcItem<'syn>),
    Enum(EnumItem<'syn>),
    Struct(StructItem<'syn>),
    Const(ConstItem<'syn>),
    Global(GlobalItem<'syn>),
    Import(ImportItem<'syn>),
    Directive(DirectiveList<'syn>),
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
            SyntaxKind::DIRECTIVE_LIST => Some(Item::Directive(DirectiveList(node))),
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Item::Proc(item) => item.0.range,
            Item::Enum(item) => item.0.range,
            Item::Struct(item) => item.0.range,
            Item::Const(item) => item.0.range,
            Item::Global(item) => item.0.range,
            Item::Import(item) => item.0.range,
            Item::Directive(item) => item.0.range,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Directive<'syn> {
    Simple(DirectiveSimple<'syn>),
    WithParams(DirectiveWithParams<'syn>),
}

impl<'syn> AstNode<'syn> for Directive<'syn> {
    fn cast(node: &'syn Node) -> Option<Directive<'syn>> {
        match node.kind {
            SyntaxKind::DIRECTIVE_SIMPLE => Some(Directive::Simple(DirectiveSimple(node))),
            SyntaxKind::DIRECTIVE_WITH_PARAMS => {
                Some(Directive::WithParams(DirectiveWithParams(node)))
            }
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Directive::Simple(dir) => dir.0.range,
            Directive::WithParams(dir) => dir.0.range,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Type<'syn> {
    Basic(TypeBasic<'syn>),
    Custom(TypeCustom<'syn>),
    Reference(TypeReference<'syn>),
    MultiReference(TypeMultiReference<'syn>),
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
            SyntaxKind::TYPE_MULTI_REFERENCE => {
                Some(Type::MultiReference(TypeMultiReference(node)))
            }
            SyntaxKind::TYPE_PROCEDURE => Some(Type::Procedure(TypeProcedure(node))),
            SyntaxKind::TYPE_ARRAY_SLICE => Some(Type::ArraySlice(TypeArraySlice(node))),
            SyntaxKind::TYPE_ARRAY_STATIC => Some(Type::ArrayStatic(TypeArrayStatic(node))),
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Type::Basic(ty) => ty.0.range,
            Type::Custom(ty) => ty.0.range,
            Type::Reference(ty) => ty.0.range,
            Type::MultiReference(ty) => ty.0.range,
            Type::Procedure(ty) => ty.0.range,
            Type::ArraySlice(ty) => ty.0.range,
            Type::ArrayStatic(ty) => ty.0.range,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Stmt<'syn> {
    Break(StmtBreak<'syn>),
    Continue(StmtContinue<'syn>),
    Return(StmtReturn<'syn>),
    Defer(StmtDefer<'syn>),
    For(StmtFor<'syn>),
    Local(StmtLocal<'syn>),
    Assign(StmtAssign<'syn>),
    ExprSemi(StmtExprSemi<'syn>),
    ExprTail(StmtExprTail<'syn>),
    WithDirective(StmtWithDirective<'syn>),
}

impl<'syn> AstNode<'syn> for Stmt<'syn> {
    fn cast(node: &'syn Node) -> Option<Stmt<'syn>> {
        match node.kind {
            SyntaxKind::STMT_BREAK => Some(Stmt::Break(StmtBreak(node))),
            SyntaxKind::STMT_CONTINUE => Some(Stmt::Continue(StmtContinue(node))),
            SyntaxKind::STMT_RETURN => Some(Stmt::Return(StmtReturn(node))),
            SyntaxKind::STMT_DEFER => Some(Stmt::Defer(StmtDefer(node))),
            SyntaxKind::STMT_FOR => Some(Stmt::For(StmtFor(node))),
            SyntaxKind::STMT_LOCAL => Some(Stmt::Local(StmtLocal(node))),
            SyntaxKind::STMT_ASSIGN => Some(Stmt::Assign(StmtAssign(node))),
            SyntaxKind::STMT_EXPR_SEMI => Some(Stmt::ExprSemi(StmtExprSemi(node))),
            SyntaxKind::STMT_EXPR_TAIL => Some(Stmt::ExprTail(StmtExprTail(node))),
            SyntaxKind::STMT_WITH_DIRECTIVE => Some(Stmt::WithDirective(StmtWithDirective(node))),
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Stmt::Break(stmt) => stmt.0.range,
            Stmt::Continue(stmt) => stmt.0.range,
            Stmt::Return(stmt) => stmt.0.range,
            Stmt::Defer(stmt) => stmt.0.range,
            Stmt::For(stmt) => stmt.0.range,
            Stmt::Local(stmt) => stmt.0.range,
            Stmt::Assign(stmt) => stmt.0.range,
            Stmt::ExprSemi(stmt) => stmt.0.range,
            Stmt::ExprTail(stmt) => stmt.0.range,
            Stmt::WithDirective(stmt) => stmt.0.range,
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
    Item(ExprItem<'syn>),
    Variant(ExprVariant<'syn>),
    StructInit(ExprStructInit<'syn>),
    ArrayInit(ExprArrayInit<'syn>),
    ArrayRepeat(ExprArrayRepeat<'syn>),
    Try(ExprTry<'syn>),
    Deref(ExprDeref<'syn>),
    Address(ExprAddress<'syn>),
    Unary(ExprUnary<'syn>),
    Binary(ExprBinary<'syn>),
}

impl<'syn> AstNode<'syn> for Expr<'syn> {
    fn cast(node: &'syn Node) -> Option<Expr<'syn>> {
        match node.kind {
            SyntaxKind::EXPR_PAREN => Some(Expr::Paren(ExprParen(node))),
            SyntaxKind::EXPR_IF => Some(Expr::If(ExprIf(node))),
            SyntaxKind::BLOCK => Some(Expr::Block(Block(node))),
            SyntaxKind::EXPR_MATCH => Some(Expr::Match(ExprMatch(node))),
            SyntaxKind::EXPR_FIELD => Some(Expr::Field(ExprField(node))),
            SyntaxKind::EXPR_INDEX => Some(Expr::Index(ExprIndex(node))),
            SyntaxKind::EXPR_SLICE => Some(Expr::Slice(ExprSlice(node))),
            SyntaxKind::EXPR_CALL => Some(Expr::Call(ExprCall(node))),
            SyntaxKind::EXPR_CAST => Some(Expr::Cast(ExprCast(node))),
            SyntaxKind::EXPR_ITEM => Some(Expr::Item(ExprItem(node))),
            SyntaxKind::EXPR_VARIANT => Some(Expr::Variant(ExprVariant(node))),
            SyntaxKind::EXPR_STRUCT_INIT => Some(Expr::StructInit(ExprStructInit(node))),
            SyntaxKind::EXPR_ARRAY_INIT => Some(Expr::ArrayInit(ExprArrayInit(node))),
            SyntaxKind::EXPR_ARRAY_REPEAT => Some(Expr::ArrayRepeat(ExprArrayRepeat(node))),
            SyntaxKind::EXPR_TRY => Some(Expr::Try(ExprTry(node))),
            SyntaxKind::EXPR_DEREF => Some(Expr::Deref(ExprDeref(node))),
            SyntaxKind::EXPR_ADDRESS => Some(Expr::Address(ExprAddress(node))),
            SyntaxKind::EXPR_UNARY => Some(Expr::Unary(ExprUnary(node))),
            SyntaxKind::EXPR_BINARY => Some(Expr::Binary(ExprBinary(node))),
            _ => Lit::cast(node).map(Expr::Lit),
        }
    }
    fn range(self) -> TextRange {
        match self {
            Expr::Paren(expr) => expr.0.range,
            Expr::Lit(lit) => lit.range(),
            Expr::If(expr) => expr.0.range,
            Expr::Block(expr) => expr.0.range,
            Expr::Match(expr) => expr.0.range,
            Expr::Field(expr) => expr.0.range,
            Expr::Index(expr) => expr.0.range,
            Expr::Slice(expr) => expr.0.range,
            Expr::Call(expr) => expr.0.range,
            Expr::Cast(expr) => expr.0.range,
            Expr::Item(expr) => expr.0.range,
            Expr::Variant(expr) => expr.0.range,
            Expr::StructInit(expr) => expr.0.range,
            Expr::ArrayInit(expr) => expr.0.range,
            Expr::ArrayRepeat(expr) => expr.0.range,
            Expr::Try(expr) => expr.0.range,
            Expr::Deref(expr) => expr.0.range,
            Expr::Address(expr) => expr.0.range,
            Expr::Unary(expr) => expr.0.range,
            Expr::Binary(expr) => expr.0.range,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Branch<'syn> {
    Cond(BranchCond<'syn>),
    Pat(BranchPat<'syn>),
}

impl<'syn> AstNode<'syn> for Branch<'syn> {
    fn cast(node: &'syn Node) -> Option<Branch<'syn>> {
        match node.kind {
            SyntaxKind::BRANCH_COND => Some(Branch::Cond(BranchCond(node))),
            SyntaxKind::BRANCH_PAT => Some(Branch::Pat(BranchPat(node))),
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Branch::Cond(branch) => branch.0.range,
            Branch::Pat(branch) => branch.0.range,
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
    fn range(self) -> TextRange {
        match self {
            Pat::Wild(pat) => pat.0.range,
            Pat::Lit(pat) => pat.0.range,
            Pat::Item(pat) => pat.0.range,
            Pat::Variant(pat) => pat.0.range,
            Pat::Or(pat) => pat.0.range,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Lit<'syn> {
    Void(LitVoid<'syn>),
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
            SyntaxKind::LIT_VOID => Some(Lit::Void(LitVoid(node))),
            SyntaxKind::LIT_NULL => Some(Lit::Null(LitNull(node))),
            SyntaxKind::LIT_BOOL => Some(Lit::Bool(LitBool(node))),
            SyntaxKind::LIT_INT => Some(Lit::Int(LitInt(node))),
            SyntaxKind::LIT_FLOAT => Some(Lit::Float(LitFloat(node))),
            SyntaxKind::LIT_CHAR => Some(Lit::Char(LitChar(node))),
            SyntaxKind::LIT_STRING => Some(Lit::String(LitString(node))),
            _ => None,
        }
    }
    fn range(self) -> TextRange {
        match self {
            Lit::Void(lit) => lit.0.range,
            Lit::Null(lit) => lit.0.range,
            Lit::Bool(lit) => lit.0.range,
            Lit::Int(lit) => lit.0.range,
            Lit::Float(lit) => lit.0.range,
            Lit::Char(lit) => lit.0.range,
            Lit::String(lit) => lit.0.range,
        }
    }
}

//==================== ITEMS ====================

impl<'syn> SourceFile<'syn> {
    node_iter!(items, Item);
}

impl<'syn> ProcItem<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(poly_params, PolymorphParams);
    node_find!(param_list, ParamList);
    node_find!(return_ty, Type);
    node_find!(block, Block);
}
impl<'syn> ParamList<'syn> {
    node_iter!(params, Param);
}
impl<'syn> Param<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(name, Name);
    node_find!(ty, Type);
    node_find!(directive, Directive);
}

impl<'syn> EnumItem<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(poly_params, PolymorphParams);
    token_find_predicate!(tag_ty, Token::as_basic_type, ast::BasicType);
    node_find!(variant_list, VariantList);
}
impl<'syn> VariantList<'syn> {
    node_iter!(variants, Variant);
}
impl<'syn> Variant<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(value, Expr);
    node_find!(field_list, VariantFieldList);
}
impl<'syn> VariantFieldList<'syn> {
    node_iter!(fields, VariantField);
}
impl<'syn> VariantField<'syn> {
    node_find!(name, Name);
    node_find!(ty, Type);
}

impl<'syn> StructItem<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(poly_params, PolymorphParams);
    node_find!(field_list, FieldList);
}
impl<'syn> FieldList<'syn> {
    node_iter!(fields, Field);
}
impl<'syn> Field<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(ty, Type);
}

impl<'syn> ConstItem<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(name, Name);
    node_find!(ty, Type);
    node_find!(value, Expr);
}

impl<'syn> GlobalItem<'syn> {
    node_find!(dir_list, DirectiveList);
    token_find!(t_mut, T![mut]);
    node_find!(name, Name);
    node_find!(ty, Type);
    node_find!(value, Expr);
    token_find!(t_zeroed, T![zeroed]);
}

impl<'syn> ImportItem<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(package, Name);
    node_find!(import_path, ImportPath);
    node_find!(rename, ImportSymbolRename);
    node_find!(import_symbol_list, ImportSymbolList);
}
impl<'syn> ImportPath<'syn> {
    node_iter!(names, Name);
}
impl<'syn> ImportSymbolList<'syn> {
    node_iter!(import_symbols, ImportSymbol);
}
impl<'syn> ImportSymbol<'syn> {
    node_find!(name, Name);
    node_find!(rename, ImportSymbolRename);
}
impl<'syn> ImportSymbolRename<'syn> {
    node_find!(alias, Name);
    token_find!(t_discard, T![_]);
}

//==================== DIRECTIVE ====================

impl<'syn> DirectiveList<'syn> {
    node_iter!(directives, Directive);
}
impl<'syn> DirectiveSimple<'syn> {
    node_find!(name, Name);
}
impl<'syn> DirectiveWithParams<'syn> {
    node_find!(name, Name);
    node_find!(param_list, DirectiveParamList);
}
impl<'syn> DirectiveParamList<'syn> {
    node_iter!(params, DirectiveParam);
}
impl<'syn> DirectiveParam<'syn> {
    node_find!(name, Name);
    node_find!(value, LitString);
}

//==================== TYPE ====================

impl<'syn> TypeBasic<'syn> {
    token_find_predicate!(basic, Token::as_basic_type, ast::BasicType);
}

impl<'syn> TypeCustom<'syn> {
    node_find!(path, Path);
}

impl<'syn> TypeReference<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(ref_ty, Type);
}

impl<'syn> TypeMultiReference<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(ref_ty, Type);
}

impl<'syn> TypeProcedure<'syn> {
    node_find!(directive, Directive);
    node_find!(param_list, ProcTypeParamList);
    node_find!(return_ty, Type);
}

impl<'syn> ProcTypeParamList<'syn> {
    node_iter!(params, ProcTypeParam);
}

impl<'syn> ProcTypeParam<'syn> {
    node_find!(name, Name);
    node_find!(ty, Type);
    node_find!(directive, Directive);
}

impl<'syn> TypeArraySlice<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(elem_ty, Type);
}

impl<'syn> TypeArrayStatic<'syn> {
    node_find!(len, Expr);
    node_find!(elem_ty, Type);
}

//==================== STMT ====================

impl<'syn> Block<'syn> {
    node_iter!(stmts, Stmt);
}

impl StmtBreak<'_> {}

impl StmtContinue<'_> {}

impl<'syn> StmtReturn<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> StmtDefer<'syn> {
    node_find!(block, Block);
    node_find!(stmt, Stmt);
}

impl<'syn> StmtFor<'syn> {
    node_find!(header_cond, ForHeaderCond);
    node_find!(header_elem, ForHeaderElem);
    node_find!(header_range, ForHeaderRange);
    node_find!(header_pat, ForHeaderPat);
    node_find!(block, Block);
}

impl<'syn> ForBind<'syn> {
    node_find!(name, Name);
    token_find!(t_discard, T![_]);
}

impl<'syn> ForHeaderCond<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> ForHeaderElem<'syn> {
    token_find!(t_ampersand, T![&]);
    token_find!(t_mut, T![mut]);
    node_before_token!(value, ForBind, T![,]);
    node_after_token!(index, ForBind, T![,]);
    token_find!(t_rev, T![<<]);
    node_find!(expr, Expr);
}

impl<'syn> ForHeaderRange<'syn> {
    token_find!(t_ampersand, T![&]);
    token_find!(t_mut, T![mut]);
    node_before_token!(value, ForBind, T![,]);
    node_after_token!(index, ForBind, T![,]);
    token_find!(t_rev, T![<<]);
    token_find!(t_exclusive, T!["..<"]);
    token_find!(t_inclusive, T!["..="]);
    node_before_token!(start_exclusive, Expr, T!["..<"]);
    node_before_token!(start_inclusive, Expr, T!["..="]);
    node_after_token!(end_exclusive, Expr, T!["..<"]);
    node_after_token!(end_inclusive, Expr, T!["..="]);
}

impl<'syn> ForHeaderPat<'syn> {
    node_find!(pat, Pat);
    node_find!(expr, Expr);
}

impl<'syn> StmtLocal<'syn> {
    node_find!(bind, Bind);
    node_find!(ty, Type);
    node_find!(init, Expr);
    token_find!(t_zeroed, T![zeroed]);
    token_find!(t_undefined, T![undefined]);
}

impl<'syn> StmtAssign<'syn> {
    token_find_predicate!(assign_op, Token::as_assign_op, ast::AssignOp);
    node_before_token_predicate!(lhs, Expr, Token::as_assign_op);
    node_after_token_predicate!(rhs, Expr, Token::as_assign_op);
}

impl<'syn> StmtExprSemi<'syn> {
    node_find!(expr, Expr);
    token_find_rev!(t_semi, T![;]);
}

impl<'syn> StmtExprTail<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> StmtWithDirective<'syn> {
    node_find!(dir_list, DirectiveList);
    node_find!(stmt, Stmt);
}

//==================== EXPR ====================

impl<'syn> ExprParen<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> ExprIf<'syn> {
    node_iter!(branches, Branch);
    node_find!(else_block, Block);
}

impl<'syn> BranchCond<'syn> {
    node_find!(cond, Expr);
    node_find!(block, Block);
}

impl<'syn> BranchPat<'syn> {
    node_find!(pat, Pat);
    node_find!(expr, Expr);
    node_find!(block, Block);
}

impl<'syn> ExprMatch<'syn> {
    node_find!(on_expr, Expr);
    node_find!(match_arm_list, MatchArmList);
}

impl<'syn> MatchArmList<'syn> {
    node_iter!(match_arms, MatchArm);
}

impl<'syn> MatchArm<'syn> {
    node_find!(pat, Pat);
    node_find!(stmt, Stmt);
}

impl<'syn> ExprField<'syn> {
    node_find!(target, Expr);
    node_find!(name, Name);
}

impl<'syn> ExprIndex<'syn> {
    node_before_token!(target, Expr, T!['[']);
    node_after_token!(index, Expr, T!['[']);
}

impl<'syn> ExprSlice<'syn> {
    node_before_token!(target, Expr, T!['[']);
    token_find!(t_full, T![..]);
    token_find!(t_exclusive, T!["..<"]);
    token_find!(t_inclusive, T!["..="]);
    node_between_tokens!(start_full, Expr, T!['['], T![..]);
    node_between_tokens!(start_exclusive, Expr, T!['['], T!["..<"]);
    node_between_tokens!(start_inclusive, Expr, T!['['], T!["..="]);
    node_after_token!(end_exclusive, Expr, T!["..<"]);
    node_after_token!(end_inclusive, Expr, T!["..="]);
}

impl<'syn> ExprCall<'syn> {
    node_find!(target, Expr);
    node_find!(args_list, ArgsList);
}

impl<'syn> ExprCast<'syn> {
    node_find!(target, Expr);
    node_find!(into_ty, Type);
}

impl<'syn> ExprItem<'syn> {
    node_find!(path, Path);
    node_find!(args_list, ArgsList);
}

impl<'syn> ExprVariant<'syn> {
    node_find!(name, Name);
    node_find!(args_list, ArgsList);
}

impl<'syn> ExprStructInit<'syn> {
    node_find!(path, Path);
    node_find!(field_init_list, FieldInitList);
}

impl<'syn> FieldInitList<'syn> {
    node_iter!(field_inits, FieldInit);
}

impl<'syn> FieldInit<'syn> {
    node_find!(name, Name);
    node_find!(expr, Expr);
}

impl<'syn> ExprArrayInit<'syn> {
    node_iter!(input, ArrayInit);
}

impl<'syn> ArrayInit<'syn> {
    node_find!(expr, Expr);
    token_find!(t_equals, T![=]);
    node_before_token!(variant, Expr, T![=]);
    node_after_token!(variant_expr, Expr, T![=]);
}

impl<'syn> ExprArrayRepeat<'syn> {
    node_before_token!(value, ArrayInit, T![;]);
    node_after_token!(len, Expr, T![;]);
}

impl<'syn> ExprTry<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> ExprDeref<'syn> {
    node_find!(expr, Expr);
}

impl<'syn> ExprAddress<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(expr, Expr);
}

impl<'syn> ExprUnary<'syn> {
    token_find_predicate!(un_op, Token::as_un_op, ast::UnOp);
    node_find!(rhs, Expr);
}

impl<'syn> ExprBinary<'syn> {
    token_find_predicate!(bin_op, Token::as_bin_op, ast::BinOp);
    node_before_token_predicate!(lhs, Expr, Token::as_bin_op);
    node_after_token_predicate!(rhs, Expr, Token::as_bin_op);
}

//==================== PAT ====================

impl PatWild<'_> {}

impl<'syn> PatLit<'syn> {
    token_find_predicate!(un_op, Token::as_un_op, ast::UnOp);
    node_find!(lit, Lit);
}

impl<'syn> PatItem<'syn> {
    node_find!(path, Path);
    node_find!(bind_list, BindList);
}

impl<'syn> PatVariant<'syn> {
    node_find!(name, Name);
    node_find!(bind_list, BindList);
}

impl<'syn> PatOr<'syn> {
    node_iter!(pats, Pat);
}

impl LitVoid<'_> {}

impl LitNull<'_> {}

impl<'syn> LitBool<'syn> {
    token_find_predicate!(value, Token::as_bool, bool);
}

impl<'syn> LitInt<'syn> {
    token_find_id!(t_int_lit_id, T![int_lit]);
}

impl<'syn> LitFloat<'syn> {
    token_find_id!(t_float_lit_id, T![float_lit]);
}

impl<'syn> LitChar<'syn> {
    token_find_id!(t_char_lit_id, T![char_lit]);
}

impl<'syn> LitString<'syn> {
    token_find_id!(t_string_lit_id, T![string_lit]);
}

//==================== COMMON ====================

impl<'syn> Name<'syn> {
    token_find!(ident, T![ident]);
}

impl<'syn> Bind<'syn> {
    token_find!(t_mut, T![mut]);
    node_find!(name, Name);
    token_find!(t_discard, T![_]);
}

impl<'syn> BindList<'syn> {
    node_iter!(binds, Bind);
}

impl<'syn> ArgsList<'syn> {
    node_iter!(exprs, Expr);
}

impl<'syn> Path<'syn> {
    node_iter!(segments, PathSegment);
}

impl<'syn> PathSegment<'syn> {
    node_find!(name, Name);
    node_find!(poly_args, PolymorphArgs);
}

impl<'syn> PolymorphArgs<'syn> {
    node_iter!(types, Type);
}

impl<'syn> PolymorphParams<'syn> {
    node_iter!(names, Name);
}

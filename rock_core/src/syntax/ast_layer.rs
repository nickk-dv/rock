use super::syntax_kind::SyntaxKind;
use super::syntax_tree::{Node, NodeID, NodeOrToken, SyntaxTree};
use crate::ast;
use crate::text::TextRange;
use crate::token::{Token, T};
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

    fn find_basic_ty(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<ast::BasicType> {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(basic) = tree.token(token_id).as_basic_type() {
                    return Some(basic);
                }
            }
        }
        None
    }

    fn find_un_op(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<ast::UnOp> {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(op) = tree.token(token_id).as_un_op() {
                    return Some(op);
                }
            }
        }
        None
    }

    fn find_bin_op(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<ast::BinOp> {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(op) = tree.token(token_id).as_bin_op() {
                    return Some(op);
                }
            }
        }
        None
    }

    fn find_assign_op(&'syn self, tree: &'syn SyntaxTree<'syn>) -> Option<ast::AssignOp> {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if let Some(op) = tree.token(token_id).as_assign_op() {
                    return Some(op);
                }
            }
        }
        None
    }

    fn find_token(&'syn self, tree: &'syn SyntaxTree<'syn>, token: Token) -> bool {
        for node_or_token in self.content.iter().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if token == tree.token(token_id) {
                    return true;
                }
            }
        }
        false
    }

    fn find_token_rev(&'syn self, tree: &'syn SyntaxTree<'syn>, token: Token) -> bool {
        for node_or_token in self.content.iter().rev().copied() {
            if let NodeOrToken::Token(token_id) = node_or_token {
                if token == tree.token(token_id) {
                    return true;
                }
            }
        }
        false
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

macro_rules! token_range {
    () => {
        pub fn token_range(&self, tree: &'syn SyntaxTree<'syn>) -> TextRange {
            match self.0.content[0] {
                NodeOrToken::Node(_) => unreachable!(),
                NodeOrToken::Token(token_id) => tree.token_range(token_id),
            }
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

ast_node_impl!(Attribute, SyntaxKind::ATTRIBUTE);
ast_node_impl!(Visibility, SyntaxKind::VISIBILITY);
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
ast_node_impl!(ImportPath, SyntaxKind::IMPORT_PATH);
ast_node_impl!(ImportSymbolList, SyntaxKind::IMPORT_SYMBOL_LIST);
ast_node_impl!(ImportSymbol, SyntaxKind::IMPORT_SYMBOL);
ast_node_impl!(NameAlias, SyntaxKind::NAME_ALIAS);

ast_node_impl!(Name, SyntaxKind::NAME);
ast_node_impl!(Path, SyntaxKind::PATH);

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
ast_node_impl!(ShortBlock, SyntaxKind::SHORT_BLOCK);
ast_node_impl!(StmtLoop, SyntaxKind::STMT_LOOP);
ast_node_impl!(LoopWhileHeader, SyntaxKind::LOOP_WHILE_HEADER);
ast_node_impl!(LoopCLikeHeader, SyntaxKind::LOOP_CLIKE_HEADER);
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
ast_node_impl!(EntryBranch, SyntaxKind::ENTRY_BRANCH);
ast_node_impl!(ElseIfBranch, SyntaxKind::ELSE_IF_BRANCH);
ast_node_impl!(FallbackBranch, SyntaxKind::FALLBACK_BRANCH);
ast_node_impl!(ExprBlock, SyntaxKind::EXPR_BLOCK);
ast_node_impl!(ExprMatch, SyntaxKind::EXPR_MATCH);
ast_node_impl!(MatchArmList, SyntaxKind::MATCH_ARM_LIST);
ast_node_impl!(MatchArm, SyntaxKind::MATCH_ARM);
ast_node_impl!(MatchFallback, SyntaxKind::MATCH_FALLBACK);
ast_node_impl!(ExprField, SyntaxKind::EXPR_FIELD);
ast_node_impl!(ExprIndex, SyntaxKind::EXPR_INDEX);
ast_node_impl!(ExprCall, SyntaxKind::EXPR_CALL);
ast_node_impl!(CallArgumentList, SyntaxKind::CALL_ARGUMENT_LIST);
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
    node_iter!(items, Item);
}

impl<'syn> Attribute<'syn> {
    find_first!(name, Name);
}

impl<'syn> Visibility<'syn> {
    find_token!(is_pub, T![pub]);
}

impl<'syn> ProcItem<'syn> {
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
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
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
    find_first!(name, Name);
    find_first!(type_basic, TypeBasic);
    find_first!(variant_list, VariantList);
}

impl<'syn> VariantList<'syn> {
    node_iter!(variants, Variant);
}

impl<'syn> Variant<'syn> {
    find_first!(name, Name);
    find_first!(value, Expr);
}

impl<'syn> StructItem<'syn> {
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
    find_first!(name, Name);
    find_first!(field_list, FieldList);
}

impl<'syn> FieldList<'syn> {
    node_iter!(fields, Field);
}

impl<'syn> Field<'syn> {
    find_first!(name, Name);
    find_first!(ty, Type);
}

impl<'syn> ConstItem<'syn> {
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
    find_first!(name, Name);
    find_first!(ty, Type);
    find_first!(value, Expr);
}

impl<'syn> GlobalItem<'syn> {
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
    find_token!(is_mut, T![mut]);
    find_first!(name, Name);
    find_first!(ty, Type);
    find_first!(value, Expr);
}

impl<'syn> ImportItem<'syn> {
    find_first!(attribute, Attribute);
    find_first!(visiblity, Visibility);
    find_first!(import_path, ImportPath);
    find_first!(name_alias, NameAlias);
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
    find_first!(name_alias, NameAlias);
}

impl<'syn> NameAlias<'syn> {
    find_first!(name, Name);
}

impl<'syn> Name<'syn> {
    token_range!();
}

impl<'syn> Path<'syn> {
    node_iter!(names, Name);
}

impl<'syn> TypeBasic<'syn> {
    pub fn basic(&self, tree: &'syn SyntaxTree<'syn>) -> ast::BasicType {
        self.0.find_basic_ty(tree).unwrap()
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
    find_first!(param_type_list, ParamTypeList);
    find_first!(return_ty, Type);
}

impl<'syn> ParamTypeList<'syn> {
    node_iter!(param_types, Type);
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
    find_first!(short_block, ShortBlock);
    find_first!(block, Block);
}

impl<'syn> ShortBlock<'syn> {
    find_first!(stmt, Stmt);
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
    find_token!(is_mut, T![mut]);
    find_first!(name, Name);
    find_first!(ty, Type);
    find_first!(expr, Expr);
}

impl<'syn> StmtAssign<'syn> {
    pub fn assign_op(&self, tree: &'syn SyntaxTree<'syn>) -> ast::AssignOp {
        self.0.find_assign_op(tree).unwrap()
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

impl<'syn> ExprLitNull<'syn> {}

impl<'syn> ExprLitBool<'syn> {
    token_range!();
}

impl<'syn> ExprLitInt<'syn> {
    token_range!();
}

impl<'syn> ExprLitFloat<'syn> {
    token_range!();
}

impl<'syn> ExprLitChar<'syn> {
    token_range!();
}

impl<'syn> ExprLitString<'syn> {
    token_range!();
}

impl<'syn> ExprIf<'syn> {
    find_first!(entry_branch, EntryBranch);
    node_iter!(else_if_branches, ElseIfBranch);
    find_first!(fallback, FallbackBranch);
}

impl<'syn> EntryBranch<'syn> {
    find_first!(cond, Expr);
    find_first!(block, Block);
}

impl<'syn> ElseIfBranch<'syn> {
    find_first!(cond, Expr);
    find_first!(block, Block);
}

impl<'syn> FallbackBranch<'syn> {
    find_first!(block, Block);
}

impl<'syn> ExprBlock<'syn> {
    node_iter!(stmts, Stmt);
}

impl<'syn> ExprMatch<'syn> {
    find_first!(on_expr, Expr);
    find_first!(match_arm_list, MatchArmList);
}

impl<'syn> MatchArmList<'syn> {
    node_iter!(match_arms, MatchArm);
    find_first!(fallback, MatchFallback);
}

impl<'syn> MatchArm<'syn> {
    //@ambiguity in incomplete tree
    node_iter!(pat_and_expr, Expr);
}

impl<'syn> MatchFallback<'syn> {
    find_first!(expr, Expr);
}

impl<'syn> ExprField<'syn> {
    find_first!(target, Expr);
    find_first!(name, Name);
}

impl<'syn> ExprIndex<'syn> {
    find_first!(target, Expr);
    find_token!(is_mut, T![mut]);
    //@index or slice range
}

impl<'syn> ExprCall<'syn> {
    find_first!(target, Expr);
    find_first!(call_argument_list, CallArgumentList);
}

impl<'syn> CallArgumentList<'syn> {
    node_iter!(inputs, Expr);
}

impl<'syn> ExprCast<'syn> {
    find_first!(expr, Expr);
    find_first!(into_ty, Type);
}

impl<'syn> ExprSizeof<'syn> {
    find_first!(ty, Type);
}

impl<'syn> ExprItem<'syn> {
    find_first!(path, Path);
}

impl<'syn> ExprVariant<'syn> {
    find_first!(name, Name);
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
        self.0.find_un_op(tree).unwrap()
    }
    find_first!(expr, Expr);
}

impl<'syn> ExprBinary<'syn> {
    pub fn bin_op(&self, tree: &'syn SyntaxTree<'syn>) -> ast::BinOp {
        self.0.find_bin_op(tree).unwrap()
    }
    //@ambiguity in incomplete tree
    node_iter!(lhs_rhs_iter, Expr);
}

use super::ast2::*;
use super::intern::INTERN_DUMMY_ID;
use super::parser::FileID;
use super::span::Span;
use super::token2::*;
use crate::err::error::{ParseContext, ParseError};
use crate::mem::arena_id::*;
use crate::mem::list_id::*;

struct Parser<'ast> {
    cursor: usize,
    tokens: TokenList,
    arena: &'ast mut Arena,
}

impl<'ast> Parser<'ast> {
    fn peek(&self) -> Token {
        self.tokens.token(self.cursor)
    }

    fn peek_next(&self, offset: usize) -> Token {
        self.tokens.token(self.cursor + offset)
    }

    fn eat(&mut self) {
        self.cursor += 1;
    }

    fn try_eat(&mut self, token: Token) -> bool {
        if self.peek() == token {
            self.eat();
            return true;
        }
        return false;
    }

    fn expect(&mut self, token: Token, ctx: ParseContext) -> Result<(), ParseError> {
        if self.peek() == token {
            self.eat();
            Ok(())
        } else {
            Err(ParseError::ExpectToken2(ctx, token))
        }
    }

    fn module(&mut self) -> Module {
        //@templ file id
        let module = Module {
            file_id: FileID(0),
            decls: List::new(),
        };
        //@decls
        module
    }

    fn vis(&mut self) -> Vis {
        if self.try_eat(Token::KwPub) {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    fn mutt(&mut self) -> Mut {
        if self.try_eat(Token::KwMut) {
            Mut::Mutable
        } else {
            Mut::Immutable
        }
    }

    fn ident(&mut self, ctx: ParseContext) -> Result<Ident, ParseError> {
        let index = TokenIndex::new(self.cursor);
        if self.try_eat(Token::Ident) {
            Ok(Ident {
                id: INTERN_DUMMY_ID,
                index,
            })
        } else {
            Err(ParseError::Ident(ctx))
        }
    }

    fn path(&mut self) -> Result<Option<Id<Path>>, ParseError> {
        let kind_index = TokenIndex::new(self.cursor);
        let kind = match self.peek() {
            Token::KwSuper => PathKind::Super,
            Token::KwPackage => PathKind::Package,
            _ => PathKind::None,
        };
        if kind != PathKind::None {
            self.eat();
            self.expect(Token::ColonColon, ParseContext::ModulePath)?;
        }
        let mut segments = List::new();
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.ident(ParseContext::ModulePath)?;
            self.eat();
            segments.add(&mut self.arena, name);
        }
        if segments.is_empty() {
            Ok(None)
        } else {
            let path = self.arena.alloc(Path {
                kind,
                kind_index,
                segments,
            });
            Ok(Some(path))
        }
    }

    //fn type_(&mut self) -> Result<P<Type>, ParseError> {}
}

fn test() {
    let mut a = Arena::new(1024);
    let expr_id = a.alloc(Expr {
        span: Span::new(1, 2),
        kind: ExprKind::Discard,
    });
    let expr = a.get_mut(expr_id);
    match expr.kind {
        ExprKind::Unit => todo!(),
        ExprKind::Discard => todo!(),
        ExprKind::LitNull => todo!(),
        ExprKind::LitBool { val } => todo!(),
        ExprKind::LitUint { val, ty } => todo!(),
        ExprKind::LitFloat { val, ty } => todo!(),
        ExprKind::LitChar { val } => todo!(),
        ExprKind::LitString { id } => todo!(),
        ExprKind::If { if_, else_ } => todo!(),
        ExprKind::Block { block } => todo!(),
        ExprKind::Match { expr, arms } => todo!(),
        ExprKind::Field { target, name } => todo!(),
        ExprKind::Index { target, index } => todo!(),
        ExprKind::Cast { target, ty } => todo!(),
        ExprKind::Sizeof { ty } => todo!(),
        ExprKind::Item { item } => todo!(),
        ExprKind::ProcCall { item, input } => todo!(),
        ExprKind::StructInit { item, input } => todo!(),
        ExprKind::ArrayInit { input } => todo!(),
        ExprKind::ArrayRepeat { expr, size } => todo!(),
        ExprKind::UnaryExpr { op, rhs } => todo!(),
        ExprKind::BinaryExpr { op, lhs, rhs } => todo!(),
    }
}

struct Evaluator<'a> {
    arena: &'a mut Arena,
}

impl<'a> Evaluator<'a> {
    fn eval_expr(&self, expr: &Expr) -> () {
        match expr.kind {
            ExprKind::Unit => todo!(),
            ExprKind::Discard => todo!(),
            ExprKind::LitNull => todo!(),
            ExprKind::LitBool { val } => todo!(),
            ExprKind::LitUint { val, ty } => todo!(),
            ExprKind::LitFloat { val, ty } => todo!(),
            ExprKind::LitChar { val } => todo!(),
            ExprKind::LitString { id } => todo!(),
            ExprKind::If { if_, else_ } => todo!(),
            ExprKind::Block { block } => todo!(),
            ExprKind::Match { expr, arms } => todo!(),
            ExprKind::Field { target, name } => todo!(),
            ExprKind::Index { target, index } => todo!(),
            ExprKind::Cast { target, ty } => todo!(),
            ExprKind::Sizeof { ty } => todo!(),
            ExprKind::Item { item } => todo!(),
            ExprKind::ProcCall { item, input } => todo!(),
            ExprKind::StructInit { item, input } => todo!(),
            ExprKind::ArrayInit { input } => todo!(),
            ExprKind::ArrayRepeat { expr, size } => todo!(),
            ExprKind::UnaryExpr { op, rhs } => todo!(),
            ExprKind::BinaryExpr { op, lhs, rhs } => {
                let v_lhs = self.eval_expr(self.arena.get(rhs));
                let v_rhs = self.eval_expr(self.arena.get(rhs));
            }
        }
    }
}

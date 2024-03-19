use super::ast::*;
use super::intern::InternPool;
use super::token::{Token, T};
use super::token_list::TokenList;
use crate::arena::Arena;
use crate::error::{ErrorComp, SourceRange};
use crate::text::TextOffset;
use crate::text::TextRange;
use crate::vfs;

pub struct Parser<'a, 'ast> {
    cursor: usize,
    tokens: TokenList,
    arena: &'a mut Arena<'ast>,
    intern_pool: &'a mut InternPool,
    source: &'a str,
    char_id: u32,
    string_id: u32,
    decls: NodeBuffer<Decl<'ast>>,
    use_symbols: NodeBuffer<UseSymbol>,
    proc_params: NodeBuffer<ProcParam<'ast>>,
    enum_variants: NodeBuffer<EnumVariant<'ast>>,
    union_members: NodeBuffer<UnionMember<'ast>>,
    struct_fields: NodeBuffer<StructField<'ast>>,
    names: NodeBuffer<Ident>,
    stmts: NodeBuffer<Stmt<'ast>>,
    match_arms: NodeBuffer<MatchArm<'ast>>,
    exprs: NodeBuffer<&'ast Expr<'ast>>,
    field_inits: NodeBuffer<FieldInit<'ast>>,
    if_arms: NodeBuffer<IfArm<'ast>>,
}

struct NodeOffset(usize);
struct NodeBuffer<T: Copy> {
    buffer: Vec<T>,
}

impl<T: Copy> NodeBuffer<T> {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn start(&self) -> NodeOffset {
        NodeOffset(self.buffer.len())
    }

    fn add(&mut self, value: T) {
        self.buffer.push(value);
    }

    fn take<'arena>(&mut self, start: NodeOffset, arena: &mut Arena<'arena>) -> &'arena [T] {
        let slice = arena.alloc_slice(&self.buffer[start.0..]);
        self.buffer.truncate(start.0);
        slice
    }
}

impl<'a, 'ast> Parser<'a, 'ast> {
    pub fn new(
        tokens: TokenList,
        arena: &'a mut Arena<'ast>,
        intern_pool: &'a mut InternPool,
        source: &'a str,
    ) -> Self {
        Self {
            cursor: 0,
            tokens,
            arena,
            intern_pool,
            source,
            char_id: 0,
            string_id: 0,
            decls: NodeBuffer::new(),
            use_symbols: NodeBuffer::new(),
            proc_params: NodeBuffer::new(),
            enum_variants: NodeBuffer::new(),
            union_members: NodeBuffer::new(),
            struct_fields: NodeBuffer::new(),
            names: NodeBuffer::new(),
            stmts: NodeBuffer::new(),
            match_arms: NodeBuffer::new(),
            exprs: NodeBuffer::new(),
            field_inits: NodeBuffer::new(),
            if_arms: NodeBuffer::new(),
        }
    }

    pub fn at(&self, t: Token) -> bool {
        self.peek() == t
    }

    pub fn at_next(&self, t: Token) -> bool {
        self.peek_next() == t
    }

    fn peek(&self) -> Token {
        self.tokens.get_token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.tokens.get_token(self.cursor + 1)
    }

    fn eat(&mut self, t: Token) -> bool {
        if self.at(t) {
            self.bump();
            return true;
        }
        false
    }

    fn bump(&mut self) {
        self.cursor += 1;
    }

    fn expect(&mut self, t: Token) -> Result<(), String> {
        if self.eat(t) {
            return Ok(());
        }
        Err(format!("expected `{}`", t.as_str()))
    }

    fn peek_range(&self) -> TextRange {
        self.tokens.get_range(self.cursor)
    }

    fn peek_range_start(&self) -> TextOffset {
        self.tokens.get_range(self.cursor).start()
    }

    fn peek_range_end(&self) -> TextOffset {
        self.tokens.get_range(self.cursor - 1).end()
    }
}

macro_rules! comma_separated_list {
    ($p:expr, $parse_function:ident, $node_buffer:ident, $delim_open:expr, $delim_close:expr) => {{
        $p.expect($delim_open)?;
        let start = $p.$node_buffer.start();
        while !$p.at($delim_close) && !$p.at(T![eof]) {
            let item = $parse_function($p)?;
            $p.$node_buffer.add(item);
            if !$p.eat(T![,]) {
                break;
            }
        }
        $p.expect($delim_close)?;
        $p.$node_buffer.take(start, $p.arena)
    }};
}

macro_rules! semi_separated_block {
    ($p:expr, $parse_function:ident, $node_buffer:ident) => {{
        $p.expect(T!['{'])?;
        let start = $p.$node_buffer.start();
        while !$p.at(T!['}']) && !$p.at(T![eof]) {
            let item = $parse_function($p)?;
            $p.$node_buffer.add(item);
            $p.expect(T![;])?;
        }
        $p.expect(T!['}'])?;
        $p.$node_buffer.take(start, $p.arena)
    }};
}

pub fn module<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    file_id: vfs::FileID,
) -> Result<Module<'ast>, ErrorComp> {
    let start = p.decls.start();
    while !p.at(T![eof]) {
        match decl(p) {
            Ok(decl) => p.decls.add(decl),
            Err(error) => {
                if p.at(T![eof]) {
                    p.cursor -= 1;
                }
                let range = p.peek_range();
                return Err(ErrorComp::error(error)
                    .context_msg("unexpected token", SourceRange::new(range, file_id)));
            }
        }
    }
    let decls = p.decls.take(start, p.arena);
    Ok(Module { file_id, decls })
}

fn decl<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<Decl<'ast>, String> {
    let vis = vis(p); //@not allowing vis with `use` is not enforced right now
    match p.peek() {
        T![mod] => Ok(Decl::Mod(mod_decl(p, vis)?)),
        T![use] => Ok(Decl::Use(use_decl(p)?)),
        T![proc] => Ok(Decl::Proc(proc_decl(p, vis)?)),
        T![enum] => Ok(Decl::Enum(enum_decl(p, vis)?)),
        T![union] => Ok(Decl::Union(union_decl(p, vis)?)),
        T![struct] => Ok(Decl::Struct(struct_decl(p, vis)?)),
        T![const] => Ok(Decl::Const(const_decl(p, vis)?)),
        T![global] => Ok(Decl::Global(global_decl(p, vis)?)),
        _ => Err("expected declaration".into()),
    }
}

fn mod_decl<'a, 'ast>(p: &mut Parser<'a, 'ast>, vis: Vis) -> Result<&'ast ModDecl, String> {
    p.bump();
    let name = name(p)?;
    p.expect(T![;])?;

    Ok(p.arena.alloc(ModDecl { vis, name }))
}

fn use_decl<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast UseDecl<'ast>, String> {
    p.bump();
    let path = path(p)?;
    p.expect(T![.])?;
    let symbols = comma_separated_list!(p, use_symbol, use_symbols, T!['{'], T!['}']);
    Ok(p.arena.alloc(UseDecl { path, symbols }))
}

fn use_symbol(p: &mut Parser) -> Result<UseSymbol, String> {
    Ok(UseSymbol {
        name: name(p)?,
        alias: if p.eat(T![as]) { Some(name(p)?) } else { None },
    })
}

fn proc_decl<'a, 'ast>(p: &mut Parser<'a, 'ast>, vis: Vis) -> Result<&'ast ProcDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;

    p.expect(T!['('])?;
    let start = p.proc_params.start();
    let mut is_variadic = false;
    while !p.at(T![')']) && !p.at(T![eof]) {
        let param = proc_param(p)?;
        p.proc_params.add(param);
        if !p.eat(T![,]) {
            break;
        }
        if p.eat(T![..]) {
            is_variadic = true;
            break;
        }
    }
    p.expect(T![')'])?;
    let params = p.proc_params.take(start, p.arena);

    let return_ty = if p.eat(T![->]) { Some(ty(p)?) } else { None };
    let directive_tail = directive(p)?;
    let block = if directive_tail.is_none() {
        Some(block(p)?)
    } else {
        None
    };

    Ok(p.arena.alloc(ProcDecl {
        vis,
        name,
        params,
        is_variadic,
        return_ty,
        directive_tail,
        block,
    }))
}

fn proc_param<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<ProcParam<'ast>, String> {
    let mutt = mutt(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(ProcParam { mutt, name, ty })
}

fn enum_decl<'a, 'ast>(p: &mut Parser<'a, 'ast>, vis: Vis) -> Result<&'ast EnumDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let variants = semi_separated_block!(p, enum_variant, enum_variants);
    Ok(p.arena.alloc(EnumDecl {
        vis,
        name,
        variants,
    }))
}

fn enum_variant<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<EnumVariant<'ast>, String> {
    let name = name(p)?;
    let value = if p.eat(T![=]) {
        Some(ConstExpr(expr(p)?))
    } else {
        None
    };
    Ok(EnumVariant { name, value })
}

fn union_decl<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    vis: Vis,
) -> Result<&'ast UnionDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let members = semi_separated_block!(p, union_member, union_members);
    Ok(p.arena.alloc(UnionDecl { vis, name, members }))
}

fn union_member<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<UnionMember<'ast>, String> {
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(UnionMember { name, ty })
}

fn struct_decl<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    vis: Vis,
) -> Result<&'ast StructDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let fields = semi_separated_block!(p, struct_field, struct_fields);
    Ok(p.arena.alloc(StructDecl { vis, name, fields }))
}

fn struct_field<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<StructField<'ast>, String> {
    let vis = vis(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(StructField { vis, name, ty })
}

fn const_decl<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    vis: Vis,
) -> Result<&'ast ConstDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    p.expect(T![=])?;
    let value = ConstExpr(expr(p)?);
    p.expect(T![;])?;

    Ok(p.arena.alloc(ConstDecl {
        vis,
        name,
        ty,
        value,
    }))
}

fn global_decl<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    vis: Vis,
) -> Result<&'ast GlobalDecl<'ast>, String> {
    p.bump();
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    p.expect(T![=])?;
    let value = ConstExpr(expr(p)?);
    p.expect(T![;])?;

    Ok(p.arena.alloc(GlobalDecl {
        vis,
        name,
        ty,
        value,
    }))
}

fn vis(p: &mut Parser) -> Vis {
    if p.eat(T![pub]) {
        Vis::Public
    } else {
        Vis::Private
    }
}

fn mutt(p: &mut Parser) -> Mut {
    if p.eat(T![mut]) {
        Mut::Mutable
    } else {
        Mut::Immutable
    }
}

fn name(p: &mut Parser) -> Result<Ident, String> {
    let range = p.peek_range();
    p.expect(T![ident])?;
    let string = &p.source[range.as_usize()];
    let id = p.intern_pool.intern(string);
    Ok(Ident { range, id })
}

fn directive(p: &mut Parser) -> Result<Option<Directive>, String> {
    if p.eat(T![#]) {
        p.expect(T!['['])?;
        let name = name(p)?;
        p.expect(T![']'])?;
        Ok(Some(Directive { name }))
    } else {
        Ok(None)
    }
}

fn path<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast Path<'ast>, String> {
    let start = p.names.start();
    let range_start = p.peek_range_start();

    let kind = match p.peek() {
        T![super] => {
            p.bump();
            PathKind::Super
        }
        T![package] => {
            p.bump();
            PathKind::Package
        }
        _ => {
            let name = name(p)?;
            p.names.add(name);
            PathKind::None
        }
    };

    while p.at(T![.]) {
        if p.at_next(T!['{']) {
            break;
        }
        p.bump();
        let name = name(p)?;
        p.names.add(name);
    }
    let names = p.names.take(start, p.arena);

    Ok(p.arena.alloc(Path {
        kind,
        names,
        range_start,
    }))
}

fn ty<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<Type<'ast>, String> {
    if let Some(basic) = p.peek().as_basic_type() {
        p.bump();
        return Ok(Type::Basic(basic));
    }
    match p.peek() {
        T!['('] => {
            p.bump();
            p.expect(T![')'])?;
            Ok(Type::Basic(BasicType::Unit))
        }
        T![ident] | T![super] | T![package] => Ok(Type::Custom(path(p)?)),
        T![*] => {
            p.bump();
            let mutt = mutt(p);
            let ty = ty(p)?;
            let ty_ref = p.arena.alloc(ty);
            Ok(Type::Reference(ty_ref, mutt))
        }
        T!['['] => match p.peek_next() {
            T![mut] | T![']'] => {
                p.bump();
                let mutt = mutt(p);
                p.expect(T![']'])?;
                let ty = ty(p)?;
                Ok(Type::ArraySlice(p.arena.alloc(ArraySlice { mutt, ty })))
            }
            _ => {
                p.bump();
                let size = ConstExpr(expr(p)?);
                p.expect(T![']'])?;
                let ty = ty(p)?;
                Ok(Type::ArrayStatic(p.arena.alloc(ArrayStatic { size, ty })))
            }
        },
        _ => Err("expected type".into()),
    }
}

fn stmt<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<Stmt<'ast>, String> {
    let range_start = p.peek_range_start();
    let kind = match p.peek() {
        T![break] => {
            p.bump();
            p.expect(T![;])?;
            StmtKind::Break
        }
        T![continue] => {
            p.bump();
            p.expect(T![;])?;
            StmtKind::Continue
        }
        T![return] => {
            p.bump();
            if p.eat(T![;]) {
                StmtKind::Return(None)
            } else {
                let expr = expr(p)?;
                p.expect(T![;])?;
                StmtKind::Return(Some(expr))
            }
        }
        T![defer] => {
            p.bump();
            StmtKind::Defer(block(p)?)
        }
        T![for] => {
            p.bump();
            StmtKind::ForLoop(for_loop(p)?)
        }
        T![let] | T![var] => StmtKind::VarDecl(var_decl(p)?),
        _ => {
            let lhs = expr(p)?;
            if let Some(op) = p.peek().as_assign_op() {
                p.bump();
                let rhs = expr(p)?;
                p.expect(T![;])?;
                StmtKind::VarAssign(p.arena.alloc(VarAssign { op, lhs, rhs }))
            } else if p.eat(T![;]) {
                StmtKind::ExprSemi(lhs)
            } else {
                StmtKind::ExprTail(lhs)
            }
        }
    };

    Ok(Stmt {
        kind,
        range: TextRange::new(range_start, p.peek_range_end()),
    })
}

fn for_loop<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast For<'ast>, String> {
    let kind = match p.peek() {
        T!['{'] => ForKind::Loop,
        T![let] | T![var] => {
            let var_decl = var_decl(p)?;
            let cond = expr(p)?;
            p.expect(T![;])?;

            let lhs = expr(p)?;
            let op = match p.peek().as_assign_op() {
                Some(op) => {
                    p.bump();
                    op
                }
                _ => return Err("expected assignment operator".into()),
            };
            let rhs = expr(p)?;
            let var_assign = p.arena.alloc(VarAssign { op, lhs, rhs });

            ForKind::ForLoop {
                var_decl,
                cond,
                var_assign,
            }
        }
        _ => ForKind::While { cond: expr(p)? },
    };
    let block = block(p)?;
    Ok(p.arena.alloc(For { kind, block }))
}

fn var_decl<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast VarDecl<'ast>, String> {
    let mutt = match p.peek() {
        T![let] => Mut::Immutable,
        T![var] => Mut::Mutable,
        _ => return Err("expected `let` or `var`".into()),
    };
    p.bump();

    let name = name(p)?;
    let ty = if p.eat(T![:]) { Some(ty(p)?) } else { None };
    let expr = if p.eat(T![=]) { Some(expr(p)?) } else { None };
    p.expect(T![;])?;

    Ok(p.arena.alloc(VarDecl {
        mutt,
        name,
        ty,
        expr,
    }))
}

fn expr<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast Expr<'ast>, String> {
    sub_expr(p, 0)
}

fn sub_expr<'a, 'ast>(p: &mut Parser<'a, 'ast>, min_prec: u32) -> Result<&'ast Expr<'ast>, String> {
    let mut expr_lhs = primary_expr(p)?;
    loop {
        let prec: u32;
        let binary_op: BinOp;
        if let Some(op) = p.peek().as_bin_op() {
            binary_op = op;
            prec = op.prec();
            if prec < min_prec {
                break;
            }
            p.bump();
        } else {
            break;
        }
        let op = binary_op;
        let lhs = expr_lhs;
        let rhs = sub_expr(p, prec + 1)?;
        expr_lhs = p.arena.alloc(Expr {
            kind: ExprKind::BinaryExpr { op, lhs, rhs },
            range: TextRange::new(lhs.range.start(), rhs.range.end()),
        });
    }
    Ok(expr_lhs)
}

fn primary_expr<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast Expr<'ast>, String> {
    let range_start = p.peek_range_start();

    if p.eat(T!['(']) {
        if p.eat(T![')']) {
            let expr = p.arena.alloc(Expr {
                kind: ExprKind::Unit,
                range: TextRange::new(range_start, p.peek_range_end()),
            });
            return tail_expr(p, expr);
        }
        let expr = sub_expr(p, 0)?;
        p.expect(T![')'])?;
        return tail_expr(p, expr);
    }

    if let Some(un_op) = p.peek().as_un_op() {
        p.bump();
        let kind = ExprKind::UnaryExpr {
            op: un_op,
            rhs: primary_expr(p)?,
        };
        return Ok(p.arena.alloc(Expr {
            kind,
            range: TextRange::new(range_start, p.peek_range_end()),
        }));
    }

    let kind = match p.peek() {
        T![null] => {
            p.bump();
            ExprKind::LitNull
        }
        T![true] => {
            p.bump();
            ExprKind::LitBool { val: true }
        }
        T![false] => {
            p.bump();
            ExprKind::LitBool { val: false }
        }
        T![int_lit] => {
            let range = p.peek_range();
            p.bump();
            let string = &p.source[range.as_usize()];
            let v = match string.parse::<u64>() {
                Ok(value) => value,
                Err(error) => {
                    panic!("parse int error: {}", error.to_string());
                }
            };
            if let Some(basic) = p.peek().as_basic_type() {
                match basic {
                    BasicType::S8
                    | BasicType::S16
                    | BasicType::S32
                    | BasicType::S64
                    | BasicType::Ssize
                    | BasicType::U8
                    | BasicType::U16
                    | BasicType::U32
                    | BasicType::U64
                    | BasicType::Usize => {
                        p.bump();
                        ExprKind::LitInt {
                            val: v,
                            ty: Some(basic),
                        }
                    }
                    BasicType::F32 | BasicType::F64 => {
                        p.bump();
                        //@some values cant be represented
                        ExprKind::LitFloat {
                            val: v as f64,
                            ty: Some(basic),
                        }
                    }
                    _ => return Err("expected integer of float type".into()),
                }
            } else {
                ExprKind::LitInt { val: v, ty: None }
            }
        }
        T![float_lit] => {
            let range = p.peek_range();
            p.bump();
            let string = &p.source[range.as_usize()];
            let v = match string.parse::<f64>() {
                Ok(value) => value,
                Err(error) => {
                    panic!("parse float error: {}", error.to_string());
                }
            };
            if let Some(basic) = p.peek().as_basic_type() {
                match basic {
                    BasicType::F32 | BasicType::F64 => {
                        p.bump();
                        ExprKind::LitFloat {
                            val: v,
                            ty: Some(basic),
                        }
                    }
                    _ => return Err("expected `f32` or `f64`".into()),
                }
            } else {
                ExprKind::LitFloat { val: v, ty: None }
            }
        }
        T![char_lit] => {
            p.bump();
            let v = p.tokens.get_char(p.char_id as usize);
            p.char_id += 1;
            ExprKind::LitChar { val: v }
        }
        T![string_lit] => {
            p.bump();
            let string = p.tokens.get_string(p.string_id as usize);
            p.string_id += 1;
            ExprKind::LitString {
                id: p.intern_pool.intern(string),
            }
        }
        T![if] => ExprKind::If { if_: if_match(p)? },
        T!['{'] => ExprKind::Block {
            stmts: block_stmts(p)?,
        },
        T![match] => {
            let start = p.match_arms.start();
            p.bump();
            let on_expr = expr(p)?;
            p.expect(T!['{'])?;
            while !p.eat(T!['}']) {
                let arm = match_arm(p)?;
                p.match_arms.add(arm);
            }

            let arms = p.match_arms.take(start, p.arena);
            let match_ = p.arena.alloc(Match { on_expr, arms });
            ExprKind::Match { match_ }
        }
        T![sizeof] => {
            p.bump();
            p.expect(T!['('])?;
            let ty = ty(p)?;
            p.expect(T![')'])?;
            ExprKind::Sizeof { ty }
        }
        T![ident] | T![super] | T![package] => {
            let path = path(p)?;

            match (p.peek(), p.peek_next()) {
                (T!['('], ..) => {
                    let input = comma_separated_list!(p, expr, exprs, T!['('], T![')']);
                    let proc_call = p.arena.alloc(ProcCall { path, input });
                    ExprKind::ProcCall { proc_call }
                }
                (T![.], T!['{']) => {
                    p.bump();
                    p.expect(T!['{'])?;
                    let start = p.field_inits.start();
                    if !p.eat(T!['}']) {
                        loop {
                            let name = name(p)?;
                            let expr = match p.peek() {
                                T![:] => {
                                    p.bump(); // ':'
                                    Some(expr(p)?)
                                }
                                T![,] | T!['}'] => None,
                                _ => return Err("expected `:`, `}` or `,`".into()),
                            };
                            p.field_inits.add(FieldInit { name, expr });
                            if !p.eat(T![,]) {
                                break;
                            }
                        }
                        p.expect(T!['}'])?;
                    }
                    let input = p.field_inits.take(start, p.arena);
                    let struct_init = p.arena.alloc(StructInit { path, input });
                    ExprKind::StructInit { struct_init }
                }
                _ => ExprKind::Item { path },
            }
        }
        T!['['] => {
            p.bump();
            if p.eat(T![']']) {
                ExprKind::ArrayInit { input: &[] }
            } else {
                let first_expr = expr(p)?;
                if p.eat(T![;]) {
                    let size = ConstExpr(expr(p)?);
                    p.expect(T![']'])?;
                    ExprKind::ArrayRepeat {
                        expr: first_expr,
                        size,
                    }
                } else {
                    let start = p.exprs.start();
                    p.exprs.add(first_expr);
                    if !p.eat(T![']']) {
                        p.expect(T![,])?;
                        loop {
                            let expr = expr(p)?;
                            p.exprs.add(expr);
                            if !p.eat(T![,]) {
                                break;
                            }
                        }
                        p.expect(T![']'])?;
                    }
                    ExprKind::ArrayInit {
                        input: p.exprs.take(start, p.arena),
                    }
                }
            }
        }
        _ => return Err("expected expression".into()),
    };
    let expr = p.arena.alloc(Expr {
        kind,
        range: TextRange::new(range_start, p.peek_range_end()),
    });
    tail_expr(p, expr)
}

fn tail_expr<'a, 'ast>(
    p: &mut Parser<'a, 'ast>,
    target: &'ast Expr<'ast>,
) -> Result<&'ast Expr<'ast>, String> {
    let mut target = target;
    let mut last_cast = false;
    let range_start = target.range.start();
    loop {
        match p.peek() {
            T![.] => {
                if last_cast {
                    return Ok(target);
                }
                p.bump();
                let name = name(p)?;
                target = p.arena.alloc(Expr {
                    kind: ExprKind::Field { target, name },
                    range: TextRange::new(range_start, p.peek_range_end()),
                });
            }
            T!['['] => {
                if last_cast {
                    return Ok(target);
                }
                p.bump();
                let index = expr(p)?;
                p.expect(T![']'])?;
                target = p.arena.alloc(Expr {
                    kind: ExprKind::Index { target, index },
                    range: TextRange::new(range_start, p.peek_range_end()),
                });
            }
            T![as] => {
                p.bump();
                let ty = ty(p)?;
                let ty_ref = p.arena.alloc(ty);
                target = p.arena.alloc(Expr {
                    kind: ExprKind::Cast { target, ty: ty_ref },
                    range: TextRange::new(range_start, p.peek_range_end()),
                });
                last_cast = true;
            }
            _ => return Ok(target),
        }
    }
}

fn if_<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast If<'ast>, String> {
    p.bump();
    let if_ = If {
        cond: expr(p)?,
        block: block(p)?,
        else_: else_branch(p)?,
    };
    Ok(p.arena.alloc(if_))
}

fn else_branch<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<Option<Else<'ast>>, String> {
    if p.eat(T![else]) {
        match p.peek() {
            T![if] => Ok(Some(Else::If { else_if: if_(p)? })),
            T!['{'] => Ok(Some(Else::Block { block: block(p)? })),
            _ => return Err("expected `if` or `{`".into()),
        }
    } else {
        Ok(None)
    }
}

fn if_match<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast [IfArm<'ast>], String> {
    p.bump();
    let arms = semi_separated_block!(p, if_arm, if_arms);
    Ok(arms)
}

fn if_arm<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<IfArm<'ast>, String> {
    let cond = if p.eat(T![_]) { None } else { Some(expr(p)?) };
    p.expect(T![->])?;
    let expr = expr(p)?;
    Ok(IfArm { cond, expr })
}

fn block<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast Expr<'ast>, String> {
    let range_start = p.peek_range_start();
    let stmts = block_stmts(p)?;
    Ok(p.arena.alloc(Expr {
        kind: ExprKind::Block { stmts },
        range: TextRange::new(range_start, p.peek_range_end()),
    }))
}

fn block_stmts<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<&'ast [Stmt<'ast>], String> {
    let start = p.stmts.start();
    p.expect(T!['{'])?;
    while !p.eat(T!['}']) {
        let stmt = stmt(p)?;
        p.stmts.add(stmt);
    }
    Ok(p.stmts.take(start, p.arena))
}

fn match_arm<'a, 'ast>(p: &mut Parser<'a, 'ast>) -> Result<MatchArm<'ast>, String> {
    let pat = expr(p)?;
    p.expect(T![=>])?;
    let expr = expr(p)?;
    Ok(MatchArm { pat, expr })
}

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicAnd | BinOp::LogicOr => 1,
            BinOp::CmpLt
            | BinOp::CmpLtEq
            | BinOp::CmpGt
            | BinOp::CmpGtEq
            | BinOp::CmpIsEq
            | BinOp::CmpNotEq => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::BitShl | BinOp::BitShr => 6,
        }
    }
}

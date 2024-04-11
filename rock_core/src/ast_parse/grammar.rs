use super::Parser;
use crate::ast::*;
use crate::error::{ErrorComp, SourceRange};
use crate::intern::InternID;
use crate::session::FileID;
use crate::text::TextRange;
use crate::token::{Token, T};

macro_rules! comma_separated_list {
    ($p:expr, $parse_function:ident, $node_buffer:ident, $delim_open:expr, $delim_close:expr) => {{
        $p.expect($delim_open)?;
        let start = $p.state.$node_buffer.start();
        while !$p.at($delim_close) && !$p.at(T![eof]) {
            let item = $parse_function($p)?;
            $p.state.$node_buffer.add(item);
            if !$p.eat(T![,]) {
                break;
            }
        }
        $p.expect($delim_close)?;
        $p.state.$node_buffer.take(start, &mut $p.state.arena)
    }};
}

macro_rules! semi_separated_block {
    ($p:expr, $parse_function:ident, $node_buffer:ident) => {{
        $p.expect(T!['{'])?;
        let start = $p.state.$node_buffer.start();
        while !$p.at(T!['}']) && !$p.at(T![eof]) {
            let item = $parse_function($p)?;
            $p.state.$node_buffer.add(item);
            $p.expect(T![;])?;
        }
        $p.expect(T!['}'])?;
        $p.state.$node_buffer.take(start, &mut $p.state.arena)
    }};
}

pub fn module<'ast>(
    mut p: Parser<'ast, '_, '_, '_>,
    file_id: FileID,
    name_id: InternID,
) -> Result<Module<'ast>, ErrorComp> {
    let start = p.state.items.start();
    while !p.at(T![eof]) {
        match item(&mut p) {
            Ok(item) => p.state.items.add(item),
            Err(error) => {
                if p.at(T![eof]) {
                    p.cursor -= 1;
                }
                let range = p.peek_range();
                return Err(ErrorComp::error_detailed(
                    error,
                    "unexpected token",
                    SourceRange::new(range, file_id),
                    None,
                ));
            }
        }
    }
    let items = p.state.items.take(start, &mut p.state.arena);
    Ok(Module {
        file_id,
        name_id,
        items,
    })
}

fn item<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Item<'ast>, String> {
    let vis = vis(p); //@not allowing vis with `import` is not enforced right now
    match p.peek() {
        T![proc] => Ok(Item::Proc(proc_item(p, vis)?)),
        T![enum] => Ok(Item::Enum(enum_item(p, vis)?)),
        T![union] => Ok(Item::Union(union_item(p, vis)?)),
        T![struct] => Ok(Item::Struct(struct_item(p, vis)?)),
        T![const] => Ok(Item::Const(const_item(p, vis)?)),
        T![global] => Ok(Item::Global(global_item(p, vis)?)),
        T![import] => Ok(Item::Import(import_item(p)?)),
        _ => Err("expected item".into()),
    }
}

fn proc_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast ProcItem<'ast>, String> {
    p.bump();
    let name = name(p)?;

    p.expect(T!['('])?;
    let start = p.state.proc_params.start();
    let mut is_variadic = false;
    while !p.at(T![')']) && !p.at(T![eof]) {
        let param = proc_param(p)?;
        p.state.proc_params.add(param);
        if !p.eat(T![,]) {
            break;
        }
        if p.eat(T![..]) {
            is_variadic = true;
            break;
        }
    }
    p.expect(T![')'])?;
    let params = p.state.proc_params.take(start, &mut p.state.arena);

    let return_ty = if p.eat(T![->]) { Some(ty(p)?) } else { None };
    let directive_tail = directive(p)?;
    let block = if directive_tail.is_none() {
        Some(block(p)?)
    } else {
        None
    };

    Ok(p.state.arena.alloc(ProcItem {
        vis,
        name,
        params,
        is_variadic,
        return_ty,
        directive_tail,
        block,
    }))
}

fn proc_param<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<ProcParam<'ast>, String> {
    let mutt = mutt(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(ProcParam { mutt, name, ty })
}

fn enum_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast EnumItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let basic = p.peek().as_basic_type();
    if basic.is_some() {
        p.bump();
    }
    let variants = semi_separated_block!(p, enum_variant, enum_variants);
    Ok(p.state.arena.alloc(EnumItem {
        vis,
        name,
        basic,
        variants,
    }))
}

fn enum_variant<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<EnumVariant<'ast>, String> {
    let name = name(p)?;
    let value = if p.eat(T![=]) {
        Some(ConstExpr(expr(p)?))
    } else {
        None
    };
    Ok(EnumVariant { name, value })
}

fn union_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast UnionItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let members = semi_separated_block!(p, union_member, union_members);
    Ok(p.state.arena.alloc(UnionItem { vis, name, members }))
}

fn union_member<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<UnionMember<'ast>, String> {
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(UnionMember { name, ty })
}

fn struct_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast StructItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let fields = semi_separated_block!(p, struct_field, struct_fields);
    Ok(p.state.arena.alloc(StructItem { vis, name, fields }))
}

fn struct_field<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<StructField<'ast>, String> {
    let vis = vis(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    Ok(StructField { vis, name, ty })
}

fn const_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast ConstItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    p.expect(T![=])?;
    let value = ConstExpr(expr(p)?);
    p.expect(T![;])?;

    Ok(p.state.arena.alloc(ConstItem {
        vis,
        name,
        ty,
        value,
    }))
}

fn global_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    vis: Vis,
) -> Result<&'ast GlobalItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    p.expect(T![=])?;
    let value = ConstExpr(expr(p)?);
    p.expect(T![;])?;

    Ok(p.state.arena.alloc(GlobalItem {
        vis,
        name,
        ty,
        value,
    }))
}

fn import_item<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast ImportItem<'ast>, String> {
    p.bump();
    let module = name(p)?;
    let alias = if p.eat(T![as]) { Some(name(p)?) } else { None };

    let symbols = if p.eat(T![.]) {
        let symbols = comma_separated_list!(p, import_symbol, import_symbols, T!['{'], T!['}']);
        p.eat(T![;]);
        symbols
    } else {
        p.expect(T![;])?;
        &[]
    };

    Ok(p.state.arena.alloc(ImportItem {
        module,
        alias,
        symbols,
    }))
}

fn import_symbol(p: &mut Parser) -> Result<ImportSymbol, String> {
    Ok(ImportSymbol {
        name: name(p)?,
        alias: if p.eat(T![as]) { Some(name(p)?) } else { None },
    })
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

fn name(p: &mut Parser) -> Result<Name, String> {
    let range = p.peek_range();
    p.expect(T![ident])?;
    let string = &p.source[range.as_usize()];
    let id = p.state.intern.intern(string);
    Ok(Name { range, id })
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

fn path<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Path<'ast>, String> {
    let start = p.state.names.start();

    let first = name(p)?;
    p.state.names.add(first);

    while p.at(T![.]) {
        if p.at_next(T!['{']) {
            break;
        }
        p.bump();
        let name = name(p)?;
        p.state.names.add(name);
    }
    let names = p.state.names.take(start, &mut p.state.arena);

    Ok(p.state.arena.alloc(Path { names }))
}

fn ty<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Type<'ast>, String> {
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
        T![ident] => Ok(Type::Custom(path(p)?)),
        T![*] => {
            p.bump();
            let mutt = mutt(p);
            let ty = ty(p)?;
            let ty_ref = p.state.arena.alloc(ty);
            Ok(Type::Reference(ty_ref, mutt))
        }
        T!['['] => match p.peek_next() {
            T![mut] | T![']'] => {
                p.bump();
                let mutt = mutt(p);
                p.expect(T![']'])?;
                let ty = ty(p)?;
                Ok(Type::ArraySlice(
                    p.state.arena.alloc(ArraySlice { mutt, ty }),
                ))
            }
            _ => {
                p.bump();
                let size = ConstExpr(expr(p)?);
                p.expect(T![']'])?;
                let ty = ty(p)?;
                Ok(Type::ArrayStatic(
                    p.state.arena.alloc(ArrayStatic { size, ty }),
                ))
            }
        },
        _ => Err("expected type".into()),
    }
}

fn stmt<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Stmt<'ast>, String> {
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
            let block = block(p)?;
            StmtKind::Defer(p.state.arena.alloc(block))
        }
        T![for] => {
            p.bump();
            StmtKind::ForLoop(for_loop(p)?)
        }
        T![let] | T![mut] => StmtKind::Local(local(p)?),
        T![->] => {
            p.bump();
            let expr = expr(p)?;
            p.expect(T![;])?;
            StmtKind::ExprTail(expr)
        }
        _ => {
            let lhs = expr(p)?;
            if let Some(op) = p.peek().as_assign_op() {
                p.bump();
                let rhs = expr(p)?;
                p.expect(T![;])?;
                StmtKind::Assign(p.state.arena.alloc(Assign { op, lhs, rhs }))
            } else {
                p.expect(T![;])?;
                StmtKind::ExprSemi(lhs)
            }
        }
    };

    Ok(Stmt {
        kind,
        range: TextRange::new(range_start, p.peek_range_end()),
    })
}

fn for_loop<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast For<'ast>, String> {
    let kind = match p.peek() {
        T!['{'] => ForKind::Loop,
        T![let] | T![mut] => {
            let local = local(p)?;
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
            let assign = p.state.arena.alloc(Assign { op, lhs, rhs });

            ForKind::ForLoop {
                local,
                cond,
                assign,
            }
        }
        _ => ForKind::While { cond: expr(p)? },
    };
    let block = block(p)?;
    Ok(p.state.arena.alloc(For { kind, block }))
}

fn local<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Local<'ast>, String> {
    let mutt = match p.peek() {
        T![mut] => Mut::Mutable,
        T![let] => Mut::Immutable,
        _ => return Err("expected `let` or `var`".into()),
    };
    p.bump();

    let name = name(p)?;
    let ty = if p.eat(T![:]) { Some(ty(p)?) } else { None };
    let value = if p.eat(T![=]) { Some(expr(p)?) } else { None };
    p.expect(T![;])?;

    Ok(p.state.arena.alloc(Local {
        mutt,
        name,
        ty,
        value,
    }))
}

fn expr<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Expr<'ast>, String> {
    sub_expr(p, 0)
}

fn sub_expr<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    min_prec: u32,
) -> Result<&'ast Expr<'ast>, String> {
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
        expr_lhs = p.state.arena.alloc(Expr {
            kind: ExprKind::Binary { op, lhs, rhs },
            range: TextRange::new(lhs.range.start(), rhs.range.end()),
        });
    }
    Ok(expr_lhs)
}

fn primary_expr<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Expr<'ast>, String> {
    let range_start = p.peek_range_start();

    if p.eat(T!['(']) {
        if p.eat(T![')']) {
            let expr = p.state.arena.alloc(Expr {
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
        let kind = ExprKind::Unary {
            op: un_op,
            rhs: primary_expr(p)?,
        };
        return Ok(p.state.arena.alloc(Expr {
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

            let val = match string.parse::<u64>() {
                Ok(value) => value,
                Err(error) => {
                    panic!("parse int error: {}", error); //@handle gracefully, without a panic
                }
            };
            ExprKind::LitInt { val }
        }
        T![float_lit] => {
            let range = p.peek_range();
            p.bump();
            let string = &p.source[range.as_usize()];

            let val = match string.parse::<f64>() {
                Ok(value) => value,
                Err(error) => {
                    panic!("parse float error: {}", error); //@handle gracefully, without a panic
                }
            };
            ExprKind::LitFloat { val }
        }
        T![char_lit] => {
            p.bump();
            let v = p.tokens.get_char(p.char_id as usize);
            p.char_id += 1;
            ExprKind::LitChar { val: v }
        }
        T![string_lit] => {
            p.bump();
            let (string, c_string) = p.tokens.get_string(p.string_id as usize);
            p.string_id += 1;
            ExprKind::LitString {
                id: p.state.intern.intern(string),
                c_string,
            }
        }
        T![if] => ExprKind::If { if_: if_(p)? },
        T!['{'] => ExprKind::Block { block: block(p)? },
        T![match] => {
            let start = p.state.match_arms.start();
            p.bump();
            let on_expr = expr(p)?;
            p.expect(T!['{'])?;
            while !p.eat(T!['}']) {
                let arm = match_arm(p)?;
                p.state.match_arms.add(arm);
            }

            let arms = p.state.match_arms.take(start, &mut p.state.arena);
            let match_ = p.state.arena.alloc(Match { on_expr, arms });
            ExprKind::Match { match_ }
        }
        T![sizeof] => {
            p.bump();
            p.expect(T!['('])?;
            let ty = ty(p)?;
            p.expect(T![')'])?;
            ExprKind::Sizeof { ty }
        }
        T![ident] => {
            let path = path(p)?;

            match (p.peek(), p.peek_next()) {
                (T!['('], ..) => {
                    let input = comma_separated_list!(p, expr, exprs, T!['('], T![')']);
                    let proc_call = p.state.arena.alloc(ProcCall { path, input });
                    ExprKind::ProcCall { proc_call }
                }
                (T![.], T!['{']) => {
                    p.bump();
                    p.expect(T!['{'])?;
                    let start = p.state.field_inits.start();
                    if !p.eat(T!['}']) {
                        loop {
                            let range_start = p.peek_range_start();
                            let name = name(p)?;
                            let expr = match p.peek() {
                                T![:] => {
                                    p.bump(); // ':'
                                    expr(p)?
                                }
                                T![,] | T!['}'] => {
                                    let names = p.state.arena.alloc_slice(&[name]);
                                    let path = p.state.arena.alloc(Path { names });
                                    p.state.arena.alloc(Expr {
                                        kind: ExprKind::Item { path },
                                        range: TextRange::new(range_start, p.peek_range_end()),
                                    })
                                }
                                _ => return Err("expected `:`, `}` or `,`".into()),
                            };
                            p.state.field_inits.add(FieldInit { name, expr });
                            if !p.eat(T![,]) {
                                break;
                            }
                        }
                        p.expect(T!['}'])?;
                    }
                    let input = p.state.field_inits.take(start, &mut p.state.arena);
                    let struct_init = p.state.arena.alloc(StructInit { path, input });
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
                    let start = p.state.exprs.start();
                    p.state.exprs.add(first_expr);
                    if !p.eat(T![']']) {
                        p.expect(T![,])?;
                        loop {
                            let expr = expr(p)?;
                            p.state.exprs.add(expr);
                            if !p.eat(T![,]) {
                                break;
                            }
                        }
                        p.expect(T![']'])?;
                    }
                    ExprKind::ArrayInit {
                        input: p.state.exprs.take(start, &mut p.state.arena),
                    }
                }
            }
        }
        _ => return Err("expected expression".into()),
    };
    let expr = p.state.arena.alloc(Expr {
        kind,
        range: TextRange::new(range_start, p.peek_range_end()),
    });
    tail_expr(p, expr)
}

fn tail_expr<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
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
                target = p.state.arena.alloc(Expr {
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
                target = p.state.arena.alloc(Expr {
                    kind: ExprKind::Index { target, index },
                    range: TextRange::new(range_start, p.peek_range_end()),
                });
            }
            T![as] => {
                p.bump();
                let ty = ty(p)?;
                let ty_ref = p.state.arena.alloc(ty);
                target = p.state.arena.alloc(Expr {
                    kind: ExprKind::Cast {
                        target,
                        into: ty_ref,
                    },
                    range: TextRange::new(range_start, p.peek_range_end()),
                });
                last_cast = true;
            }
            _ => return Ok(target),
        }
    }
}

fn if_<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast If<'ast>, String> {
    p.bump();
    let entry = Branch {
        cond: expr(p)?,
        block: block(p)?,
    };
    let mut else_block = None;

    let start = p.state.branches.start();
    while p.eat(T![else]) {
        if p.eat(T![if]) {
            let branch = Branch {
                cond: expr(p)?,
                block: block(p)?,
            };
            p.state.branches.add(branch);
        } else {
            else_block = Some(block(p)?);
            break;
        }
    }
    let branches = p.state.branches.take(start, &mut p.state.arena);

    Ok(p.state.arena.alloc(If {
        entry,
        branches,
        else_block,
    }))
}

fn block<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Block<'ast>, String> {
    let start = p.state.stmts.start();

    p.expect(T!['{'])?;
    while !p.eat(T!['}']) {
        let stmt = stmt(p)?;
        p.state.stmts.add(stmt);
    }

    let stmts = p.state.stmts.take(start, &mut p.state.arena);
    Ok(Block { stmts })
}

fn match_arm<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<MatchArm<'ast>, String> {
    let pat = if p.eat(T![_]) { None } else { Some(expr(p)?) };
    p.expect(T![->])?;
    let expr = expr(p)?;
    Ok(MatchArm { pat, expr })
}

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::Range | BinOp::RangeInc => 1,
            BinOp::LogicOr => 2,
            BinOp::LogicAnd => 3,
            BinOp::CmpIsEq
            | BinOp::CmpNotEq
            | BinOp::CmpLt
            | BinOp::CmpLtEq
            | BinOp::CmpGt
            | BinOp::CmpGtEq => 4,
            BinOp::Add | BinOp::Sub | BinOp::BitOr => 5,
            BinOp::Mul
            | BinOp::Div
            | BinOp::Rem
            | BinOp::BitAnd
            | BinOp::BitXor
            | BinOp::BitShl
            | BinOp::BitShr => 6,
        }
    }
}

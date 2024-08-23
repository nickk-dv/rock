use super::parser::Parser;
use crate::ast::*;
use crate::error::{ErrorComp, SourceRange};
use crate::session::ModuleID;
use crate::text::TextRange;
use crate::token::{Token, T};

macro_rules! comma_separated_list {
    ($p:expr, $parse_function:ident, $node_buffer:ident, $delim_open:expr, $delim_close:expr) => {{
        $p.expect($delim_open)?;
        let offset = $p.state.$node_buffer.start();
        while !$p.at($delim_close) && !$p.at(T![eof]) {
            let item = $parse_function($p)?;
            $p.state.$node_buffer.add(item);
            if !$p.eat(T![,]) {
                break;
            }
        }
        $p.expect($delim_close)?;
        $p.state.$node_buffer.take(offset, &mut $p.state.arena)
    }};
}

pub fn module<'ast>(
    mut p: Parser<'ast, '_, '_, '_>,
    module_id: ModuleID,
) -> Result<Module<'ast>, ErrorComp> {
    let offset = p.state.items.start();
    while !p.at(T![eof]) {
        match item(&mut p) {
            Ok(item) => p.state.items.add(item),
            Err(error) => {
                if p.at(T![eof]) {
                    p.cursor -= 1;
                }
                let range = p.peek_range();
                return Err(ErrorComp::new_detailed(
                    error,
                    "unexpected token",
                    SourceRange::new(module_id, range),
                    None,
                ));
            }
        }
    }
    let items = p.state.items.take(offset, &mut p.state.arena);

    Ok(Module { items })
}

fn item<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Item<'ast>, String> {
    let attrs = attribute_list(p)?;
    let vis = vis(p); //@not allowing vis with `import` is not enforced right now

    match p.peek() {
        T![proc] => Ok(Item::Proc(proc_item(p, attrs, vis)?)),
        T![enum] => Ok(Item::Enum(enum_item(p, attrs, vis)?)),
        T![struct] => Ok(Item::Struct(struct_item(p, attrs, vis)?)),
        T![const] => Ok(Item::Const(const_item(p, attrs, vis)?)),
        T![global] => Ok(Item::Global(global_item(p, attrs, vis)?)),
        T![import] => Ok(Item::Import(import_item(p, attrs, vis)?)),
        _ => Err("expected item".into()),
    }
}

fn proc_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
    vis: Vis,
) -> Result<&'ast ProcItem<'ast>, String> {
    p.bump();
    let name = name(p)?;

    let offset = p.state.params.start();
    let mut is_variadic = false;
    p.expect(T!['('])?;
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.eat(T![..]) {
            is_variadic = true;
            break;
        }
        let param = param(p)?;
        p.state.params.add(param);
        if !p.eat(T![,]) {
            break;
        }
    }
    p.expect(T![')'])?;
    let params = p.state.params.take(offset, &mut p.state.arena);
    let return_ty = if p.eat(T![->]) { Some(ty(p)?) } else { None };

    let block = if p.at(T!['{']) {
        Some(block(p)?)
    } else if p.eat(T![;]) {
        None
    } else {
        return Err("expected `{` or `;`".into());
    };

    Ok(p.state.arena.alloc(ProcItem {
        attrs,
        vis,
        name,
        params,
        is_variadic,
        return_ty,
        block,
    }))
}

fn param<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Param<'ast>, String> {
    let mutt = mutt(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;

    Ok(Param { mutt, name, ty })
}

fn enum_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
    vis: Vis,
) -> Result<&'ast EnumItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let variants = comma_separated_list!(p, variant, variants, T!['{'], T!['}']);

    Ok(p.state.arena.alloc(EnumItem {
        attrs,
        vis,
        name,
        variants,
    }))
}

fn variant<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Variant<'ast>, String> {
    let name = name(p)?;

    let kind = if p.eat(T![=]) {
        let value = ConstExpr(expr(p)?);
        VariantKind::Constant(value)
    } else if p.at(T!['(']) {
        let types = comma_separated_list!(p, ty, types, T!['('], T![')']);
        VariantKind::HasValues(types)
    } else {
        VariantKind::Default
    };

    Ok(Variant { name, kind })
}

fn struct_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
    vis: Vis,
) -> Result<&'ast StructItem<'ast>, String> {
    p.bump();
    let name = name(p)?;
    let fields = comma_separated_list!(p, field, fields, T!['{'], T!['}']);

    Ok(p.state.arena.alloc(StructItem {
        attrs,
        vis,
        name,
        fields,
    }))
}

fn field<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Field<'ast>, String> {
    let vis = vis(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;

    Ok(Field { vis, name, ty })
}

fn const_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
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
        attrs,
        vis,
        name,
        ty,
        value,
    }))
}

fn global_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
    vis: Vis,
) -> Result<&'ast GlobalItem<'ast>, String> {
    p.bump();
    let mutt = mutt(p);
    let name = name(p)?;
    p.expect(T![:])?;
    let ty = ty(p)?;
    p.expect(T![=])?;
    let value = ConstExpr(expr(p)?);
    p.expect(T![;])?;

    Ok(p.state.arena.alloc(GlobalItem {
        attrs,
        vis,
        mutt,
        name,
        ty,
        value,
    }))
}

fn import_item<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    attrs: &'ast [Attribute],
    vis: Vis,
) -> Result<&'ast ImportItem<'ast>, String> {
    p.bump();

    let package = if p.at(T![ident]) && p.at_next(T![:]) {
        let name = name(p)?;
        p.bump();
        Some(name)
    } else {
        None
    };

    let offset = p.state.names.start();
    let first = name(p)?;
    p.state.names.add(first);
    while p.eat(T![/]) {
        let name = name(p)?;
        p.state.names.add(name);
    }
    let import_path = p.state.names.take(offset, &mut p.state.arena);
    let rename = symbol_rename(p)?;

    let symbols = if p.eat(T![.]) {
        let symbols = comma_separated_list!(p, import_symbol, import_symbols, T!['{'], T!['}']);
        p.eat(T![;]);
        symbols
    } else {
        p.expect(T![;])?;
        &[]
    };

    Ok(p.state.arena.alloc(ImportItem {
        attrs,
        package,
        import_path,
        rename,
        symbols,
    }))
}

fn import_symbol(p: &mut Parser) -> Result<ImportSymbol, String> {
    let name = name(p)?;
    let rename = symbol_rename(p)?;

    Ok(ImportSymbol { name, rename })
}

fn symbol_rename(p: &mut Parser) -> Result<SymbolRename, String> {
    if p.eat(T![as]) {
        let range = p.peek_range();
        if p.eat(T![_]) {
            Ok(SymbolRename::Discard(range))
        } else {
            let alias = name(p)?;
            Ok(SymbolRename::Alias(alias))
        }
    } else {
        Ok(SymbolRename::None)
    }
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
    let id = p.state.intern_name.intern(string);

    Ok(Name { range, id })
}

fn attribute_list<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
) -> Result<&'ast [Attribute<'ast>], String> {
    let offset = p.state.attrs.start();

    while p.at(T![#]) {
        let start = p.start_range();
        p.expect(T![#])?;
        p.expect(T!['['])?;

        let range = p.peek_range();
        p.expect(T![ident])?;
        let string = &p.source[range.as_usize()];
        let kind = AttributeKind::from_str(string);

        let params = if p.at(T!['(']) {
            comma_separated_list!(p, attribute_param, attr_params, T!['('], T![')'])
        } else {
            &[]
        };
        p.expect(T![']'])?;

        let attr = Attribute {
            kind,
            range: p.make_range(start),
            params,
        };
        p.state.attrs.add(attr);
    }

    Ok(p.state.attrs.take(offset, &mut p.state.arena))
}

fn attribute_param<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<AttributeParam, String> {
    let key = name(p)?;

    let val = if p.eat(T![=]) {
        if p.at(T![string_lit]) {
            p.bump();
            Some(p.get_string_lit().0)
        } else {
            return Err("expected string literal".into());
        }
    } else {
        None
    };

    Ok(AttributeParam { key, val })
}

fn path<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Path<'ast>, String> {
    let offset = p.state.names.start();
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
    let names = p.state.names.take(offset, &mut p.state.arena);

    Ok(p.state.arena.alloc(Path { names }))
}

fn ty<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Type<'ast>, String> {
    let start = p.start_range();

    if let Some(basic) = p.peek().as_basic_type() {
        p.bump();
        return Ok(Type {
            kind: TypeKind::Basic(basic),
            range: p.make_range(start),
        });
    }

    let kind = match p.peek() {
        T![ident] => TypeKind::Custom(path(p)?),
        T![&] => {
            p.bump();
            let mutt = mutt(p);
            let ty = ty(p)?;
            let ty_ref = p.state.arena.alloc(ty);
            TypeKind::Reference(ty_ref, mutt)
        }
        T![proc] => {
            p.bump();
            p.expect(T!['('])?;
            let offset = p.state.types.start();
            let mut is_variadic = false;
            while !p.at(T![')']) && !p.at(T![eof]) {
                let ty = ty(p)?;
                p.state.types.add(ty);
                if !p.eat(T![,]) {
                    break;
                }
                if p.eat(T![..]) {
                    is_variadic = true;
                    break;
                }
            }
            p.expect(T![')'])?;
            let param_types = p.state.types.take(offset, &mut p.state.arena);
            let return_ty = if p.eat(T![->]) { Some(ty(p)?) } else { None };

            TypeKind::Procedure(p.state.arena.alloc(ProcType {
                param_types,
                is_variadic,
                return_ty,
            }))
        }
        T!['['] => {
            p.bump();
            match p.peek() {
                T![mut] | T![']'] => {
                    let mutt = mutt(p);
                    p.expect(T![']'])?;
                    let elem_ty = ty(p)?;
                    TypeKind::ArraySlice(p.state.arena.alloc(ArraySlice { mutt, elem_ty }))
                }
                _ => {
                    let len = ConstExpr(expr(p)?);
                    p.expect(T![']'])?;
                    let elem_ty = ty(p)?;
                    TypeKind::ArrayStatic(p.state.arena.alloc(ArrayStatic { len, elem_ty }))
                }
            }
        }
        _ => {
            return Err("expected type".into());
        }
    };

    Ok(Type {
        kind,
        range: p.make_range(start),
    })
}

fn stmt<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Stmt<'ast>, String> {
    let start = p.start_range();

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
            let defer_block = if p.at(T!['{']) {
                block(p)?
            } else {
                let start = p.start_range();
                let offset = p.state.stmts.start();

                let stmt = stmt(p)?;
                p.state.stmts.add(stmt);
                let stmts = p.state.stmts.take(offset, &mut p.state.arena);

                Block {
                    stmts,
                    range: p.make_range(start),
                }
            };
            StmtKind::Defer(p.state.arena.alloc(defer_block))
        }
        T![for] => StmtKind::Loop(loop_(p)?),
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
                let op_range = p.peek_range();
                p.bump();
                let rhs = expr(p)?;
                p.expect(T![;])?;

                StmtKind::Assign(p.state.arena.alloc(Assign {
                    op,
                    op_range,
                    lhs,
                    rhs,
                }))
            } else {
                if !p.at_prev(T!['}']) {
                    p.expect(T![;])?;
                } else {
                    p.eat(T![;]);
                }
                StmtKind::ExprSemi(lhs)
            }
        }
    };

    Ok(Stmt {
        kind,
        range: p.make_range(start),
    })
}

fn loop_<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Loop<'ast>, String> {
    p.bump();
    let kind = match p.peek() {
        T!['{'] => LoopKind::Loop,
        T![let] | T![mut] => {
            let local = local(p)?;
            let cond = expr(p)?;
            p.expect(T![;])?;

            let lhs = expr(p)?;
            let op = match p.peek().as_assign_op() {
                Some(op) => op,
                _ => return Err("expected assignment operator".into()),
            };
            let op_range = p.peek_range();
            p.bump();

            let rhs = expr(p)?;
            let assign = p.state.arena.alloc(Assign {
                op,
                op_range,
                lhs,
                rhs,
            });

            LoopKind::ForLoop {
                local,
                cond,
                assign,
            }
        }
        _ => LoopKind::While { cond: expr(p)? },
    };

    let block = block(p)?;
    Ok(p.state.arena.alloc(Loop { kind, block }))
}

fn local<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Local<'ast>, String> {
    let mutt = match p.peek() {
        T![mut] => Mut::Mutable,
        T![let] => Mut::Immutable,
        _ => return Err("expected `let` or `mut`".into()),
    };
    p.bump();

    let name = name(p)?;
    let kind = if p.eat(T![:]) {
        let ty = ty(p)?;
        if p.eat(T![=]) {
            let value = expr(p)?;
            LocalKind::Init(Some(ty), value)
        } else {
            LocalKind::Decl(ty)
        }
    } else {
        p.expect(T![=])?;
        let value = expr(p)?;
        LocalKind::Init(None, value)
    };
    p.expect(T![;])?;

    Ok(p.state.arena.alloc(Local { mutt, name, kind }))
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
        let op: BinOp;
        let op_range: TextRange;

        if let Some(bin_op) = p.peek().as_bin_op() {
            op = bin_op;
            prec = bin_op.prec();
            if prec < min_prec {
                break;
            }
            op_range = p.peek_range();
            p.bump();
        } else {
            break;
        }

        let lhs = expr_lhs;
        let rhs = sub_expr(p, prec + 1)?;
        let bin = p.state.arena.alloc(BinExpr { lhs, rhs });

        expr_lhs = p.state.arena.alloc(Expr {
            kind: ExprKind::Binary { op, op_range, bin },
            range: TextRange::new(lhs.range.start(), rhs.range.end()),
        });
    }

    Ok(expr_lhs)
}

fn primary_expr<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Expr<'ast>, String> {
    let start = p.start_range();

    if p.eat(T!['(']) {
        let expr = sub_expr(p, 0)?;
        p.expect(T![')'])?;
        return tail_expr(p, expr);
    }

    if let Some(un_op) = p.peek().as_un_op() {
        let op_range = p.peek_range();
        p.bump();

        let kind = ExprKind::Unary {
            op: un_op,
            op_range,
            rhs: primary_expr(p)?,
        };
        return Ok(p.state.arena.alloc(Expr {
            kind,
            range: p.make_range(start),
        }));
    } else if p.eat(T![&]) {
        let kind = ExprKind::Address {
            mutt: mutt(p),
            rhs: primary_expr(p)?,
        };
        return Ok(p.state.arena.alloc(Expr {
            kind,
            range: p.make_range(start),
        }));
    } else if p.eat(T![*]) {
        let kind = ExprKind::Deref {
            rhs: primary_expr(p)?,
        };
        return Ok(p.state.arena.alloc(Expr {
            kind,
            range: p.make_range(start),
        }));
    }

    let kind = match p.peek() {
        T![null]
        | T![true]
        | T![false]
        | T![int_lit]
        | T![float_lit]
        | T![char_lit]
        | T![string_lit] => {
            let lit = lit(p)?;
            ExprKind::Lit(lit)
        }
        T![if] => ExprKind::If { if_: if_(p)? },
        T!['{'] => {
            let block = block(p)?;
            let block_ref = p.state.arena.alloc(block);
            ExprKind::Block { block: block_ref }
        }
        T![match] => ExprKind::Match { match_: match_(p)? },
        T![match2] => ExprKind::Match2 {
            match_2: match_2(p)?,
        },
        T![sizeof] => {
            p.bump();
            p.expect(T!['('])?;
            let ty = ty(p)?;
            p.expect(T![')'])?;
            let ty_ref = p.state.arena.alloc(ty);
            ExprKind::Sizeof { ty: ty_ref }
        }
        T![.] => {
            p.bump();

            if p.at(T!['{']) {
                let input = field_init_list(p)?;

                let struct_init = p.state.arena.alloc(StructInit { path: None, input });
                ExprKind::StructInit { struct_init }
            } else {
                let name = name(p)?;
                let input = if p.at(T!['(']) {
                    let input = input(p)?;
                    let input = p.state.arena.alloc(input);
                    Some(input)
                } else {
                    None
                };
                ExprKind::Variant { name, input }
            }
        }
        T![ident] => {
            let path = path(p)?;

            match p.peek() {
                T![.] => {
                    p.bump();
                    let input = field_init_list(p)?;

                    let struct_init = p.state.arena.alloc(StructInit {
                        path: Some(path),
                        input,
                    });
                    ExprKind::StructInit { struct_init }
                }
                _ => {
                    let input = if p.at(T!['(']) {
                        let input = input(p)?;
                        let input = p.state.arena.alloc(input);
                        Some(input)
                    } else {
                        None
                    };
                    ExprKind::Item { path, input }
                }
            }
        }
        T!['['] => {
            p.bump();
            if p.eat(T![']']) {
                ExprKind::ArrayInit { input: &[] }
            } else {
                let first_expr = expr(p)?;
                if p.eat(T![;]) {
                    let len = ConstExpr(expr(p)?);
                    p.expect(T![']'])?;
                    ExprKind::ArrayRepeat {
                        expr: first_expr,
                        len,
                    }
                } else {
                    let offset = p.state.exprs.start();
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
                        input: p.state.exprs.take(offset, &mut p.state.arena),
                    }
                }
            }
        }
        T![..] => {
            p.bump();
            let range = Range::Full;
            let range = p.state.arena.alloc(range);
            ExprKind::Range { range }
        }
        T!["..<"] => {
            p.bump();
            let end = expr(p)?;
            let range = Range::RangeTo(end);
            let range = p.state.arena.alloc(range);
            ExprKind::Range { range }
        }
        T!["..="] => {
            p.bump();
            let end = expr(p)?;
            let range = Range::RangeToInclusive(end);
            let range = p.state.arena.alloc(range);
            ExprKind::Range { range }
        }
        _ => return Err("expected expression".into()),
    };

    let expr = p.state.arena.alloc(Expr {
        kind,
        range: p.make_range(start),
    });
    tail_expr(p, expr)
}

fn tail_expr<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
    target: &'ast Expr<'ast>,
) -> Result<&'ast Expr<'ast>, String> {
    let start = target.range.start();
    let mut target = target;

    loop {
        match p.peek() {
            T![.] => {
                p.bump();
                let name = name(p)?;

                let expr = Expr {
                    kind: ExprKind::Field { target, name },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            T!['['] => {
                p.bump();
                let mutt = mutt(p);
                let index = expr(p)?;
                p.expect(T![']'])?;

                let expr = Expr {
                    kind: ExprKind::Index {
                        target,
                        mutt,
                        index,
                    },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            T!['('] => {
                let input = input(p)?;
                let input = p.state.arena.alloc(input);

                let expr = Expr {
                    kind: ExprKind::Call { target, input },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            T![as] => {
                p.bump();
                let ty = ty(p)?;
                let ty_ref = p.state.arena.alloc(ty);

                let expr = Expr {
                    kind: ExprKind::Cast {
                        target,
                        into: ty_ref,
                    },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
                return Ok(target);
            }
            T![..] => {
                p.bump();
                let range = Range::RangeFrom(target);
                let range = p.state.arena.alloc(range);

                let expr = Expr {
                    kind: ExprKind::Range { range },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            T!["..<"] => {
                p.bump();
                let end = expr(p)?;
                let range = Range::Range(target, end);
                let range = p.state.arena.alloc(range);

                let expr = Expr {
                    kind: ExprKind::Range { range },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            T!["..="] => {
                p.bump();
                let end = expr(p)?;
                let range = Range::RangeInclusive(target, end);
                let range = p.state.arena.alloc(range);

                let expr = Expr {
                    kind: ExprKind::Range { range },
                    range: p.make_range(start),
                };
                target = p.state.arena.alloc(expr);
            }
            _ => return Ok(target),
        }
    }
}

fn lit<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Lit, String> {
    match p.peek() {
        T![null] => {
            p.bump();
            Ok(Lit::Null)
        }
        T![true] => {
            p.bump();
            Ok(Lit::Bool(true))
        }
        T![false] => {
            p.bump();
            Ok(Lit::Bool(false))
        }
        T![int_lit] => {
            p.bump();
            let val = p.get_int_lit();
            Ok(Lit::Int(val))
        }
        T![float_lit] => {
            let range = p.peek_range();
            p.bump();
            let string = &p.source[range.as_usize()];

            let val = match string.parse::<f64>() {
                Ok(value) => value,
                Err(error) => {
                    p.state.errors.push(ErrorComp::new(
                        format!("parse float error: {}", error),
                        SourceRange::new(p.module_id, range),
                        None,
                    ));
                    0.0
                }
            };
            Ok(Lit::Float(val))
        }
        T![char_lit] => {
            p.bump();
            let val = p.get_char_lit();
            Ok(Lit::Char(val))
        }
        T![string_lit] => {
            p.bump();
            let (id, c_string) = p.get_string_lit();
            Ok(Lit::String { id, c_string })
        }
        _ => unreachable!(),
    }
}

fn if_<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast If<'ast>, String> {
    p.bump();
    let entry = Branch {
        cond: expr(p)?,
        block: block(p)?,
    };
    let mut else_block = None;

    let offset = p.state.branches.start();
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
    let branches = p.state.branches.take(offset, &mut p.state.arena);

    Ok(p.state.arena.alloc(If {
        entry,
        branches,
        else_block,
    }))
}

fn block<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Block<'ast>, String> {
    let start = p.start_range();
    let offset = p.state.stmts.start();

    p.expect(T!['{'])?;
    while !p.at(T!['}']) && !p.at(T![eof]) {
        let stmt = stmt(p)?;
        p.state.stmts.add(stmt);
    }
    p.expect(T!['}'])?;

    let stmts = p.state.stmts.take(offset, &mut p.state.arena);
    Ok(Block {
        stmts,
        range: p.make_range(start),
    })
}

fn match_<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Match<'ast>, String> {
    p.bump();
    let offset = p.state.match_arms.start();
    let on_expr = expr(p)?;
    let mut fallback = None;
    let mut fallback_range = TextRange::zero();

    p.expect(T!['{'])?;
    while !p.at(T!['}']) && !p.at(T![eof]) {
        fallback_range = p.peek_range();
        if p.eat(T![_]) {
            p.expect(T![->])?;
            let expr = expr(p)?;
            fallback = Some(expr);
        } else {
            let pat = ConstExpr(expr(p)?);
            p.expect(T![->])?;
            let expr = expr(p)?;
            let arm = MatchArm { pat, expr };
            p.state.match_arms.add(arm);
        }

        p.expect(T![,])?;
        if fallback.is_some() {
            break;
        }
    }
    p.expect(T!['}'])?;

    let arms = p.state.match_arms.take(offset, &mut p.state.arena);
    let match_ = p.state.arena.alloc(Match {
        on_expr,
        arms,
        fallback,
        fallback_range,
    });
    Ok(match_)
}

fn match_2<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<&'ast Match2<'ast>, String> {
    p.bump();
    let on_expr = expr(p)?;
    let arms = comma_separated_list!(p, match_arm_2, match_arms_2, T!['{'], T!['}']);

    let match_ = Match2 { on_expr, arms };
    let match_ = p.state.arena.alloc(match_);
    Ok(match_)
}

fn match_arm_2<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<MatchArm2<'ast>, String> {
    let pat = pat(p)?;
    p.expect(T![->])?;
    let expr = expr(p)?;

    let arm = MatchArm2 { pat, expr };
    Ok(arm)
}

fn pat<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Pat<'ast>, String> {
    let start = p.start_range();
    let mut pat_first = primary_pat(p)?;

    if p.at(T![|]) {
        let offset = p.state.patterns.start();
        p.state.patterns.add(pat_first);
        while p.at(T![|]) {
            p.bump();
            let pat = primary_pat(p)?;
            p.state.patterns.add(pat);
        }
        let patterns = p.state.patterns.take(offset, &mut p.state.arena);

        pat_first = Pat {
            kind: PatKind::Or { patterns },
            range: p.make_range(start),
        };
    }

    Ok(pat_first)
}

fn primary_pat<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Pat<'ast>, String> {
    let start = p.start_range();

    let kind = match p.peek() {
        T![_] => {
            p.bump();
            PatKind::Wild
        }
        T![null]
        | T![true]
        | T![false]
        | T![int_lit]
        | T![float_lit]
        | T![char_lit]
        | T![string_lit] => {
            let lit = lit(p)?;
            PatKind::Lit(lit)
        }
        T![ident] => {
            let path = path(p)?;
            let binds = binds(p)?;
            PatKind::Item { path, binds }
        }
        T![.] => {
            p.bump();
            let name = name(p)?;
            let binds = binds(p)?;
            PatKind::Variant { name, binds }
        }
        _ => return Err("expected pattern".into()),
    };

    let pat = Pat {
        kind,
        range: p.make_range(start),
    };
    Ok(pat)
}

fn binds<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Option<&'ast [Name]>, String> {
    if p.at(T!['(']) {
        let names = comma_separated_list!(p, name, names, T!['('], T![')']);
        Ok(Some(names))
    } else {
        Ok(None)
    }
}

fn input<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<Input<'ast>, String> {
    let start = p.start_range();
    let exprs = comma_separated_list!(p, expr, exprs, T!['('], T![')']);

    Ok(Input {
        exprs,
        range: p.make_range(start),
    })
}

fn field_init_list<'ast>(
    p: &mut Parser<'ast, '_, '_, '_>,
) -> Result<&'ast [FieldInit<'ast>], String> {
    p.expect(T!['{'])?;

    let offset = p.state.field_inits.start();
    while !p.at(T!['}']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            let field_init = field_init(p)?;
            p.state.field_inits.add(field_init);
        } else {
            return Err("expected field initializer".into());
        }
        if !p.at(T!['}']) {
            p.expect(T![,])?;
        }
    }
    p.expect(T!['}'])?;

    Ok(p.state.field_inits.take(offset, &mut p.state.arena))
}

fn field_init<'ast>(p: &mut Parser<'ast, '_, '_, '_>) -> Result<FieldInit<'ast>, String> {
    let start = p.start_range();
    let name = name(p)?;

    let expr = if p.at(T![:]) {
        p.bump();
        expr(p)?
    } else {
        let names = p.state.arena.alloc_slice(&[name]);
        let path = p.state.arena.alloc(Path { names });
        p.state.arena.alloc(Expr {
            kind: ExprKind::Item { path, input: None },
            range: p.make_range(start),
        })
    };

    Ok(FieldInit { name, expr })
}

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicOr => 1,
            BinOp::LogicAnd => 2,
            BinOp::IsEq
            | BinOp::NotEq
            | BinOp::Less
            | BinOp::LessEq
            | BinOp::Greater
            | BinOp::GreaterEq => 3,
            BinOp::Add | BinOp::Sub => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitShl | BinOp::BitShr => 6,
        }
    }
}

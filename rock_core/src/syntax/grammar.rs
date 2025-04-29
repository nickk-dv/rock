use super::parser::{Marker, MarkerClosed, Parser};
use super::syntax_kind::SyntaxKind;
use super::token::{Token, TokenSet, T};

pub fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(T![eof]) {
        item(p);
    }
    m.complete(p, SyntaxKind::SOURCE_FILE);
}

fn item(p: &mut Parser) {
    let mut mc = None;

    if p.at(T![#]) {
        let (m, scope) = directive_list_item(p);
        if scope {
            return;
        }
        mc = Some(m);
    }

    let m = if let Some(mc) = mc { p.start_before(mc) } else { p.start() };

    match p.peek() {
        T![proc] => proc_item(p, m),
        T![enum] => enum_item(p, m),
        T![struct] => struct_item(p, m),
        T![ident] => const_item(p, m),
        T![let] => global_item(p, m),
        T![import] => import_item(p, m),
        _ => {
            p.error("expected item");
            p.bump_sync(FIRST_ITEM);
            m.complete(p, SyntaxKind::ERROR);
        }
    }
}

fn proc_item(p: &mut Parser, m: Marker) {
    p.bump(T![proc]);
    name(p);
    if p.at(T!['(']) {
        param_list(p);
    } else {
        p.error_recover("expected parameter list", RECOVER_PARAM_LIST);
    }
    if p.at(T!['(']) {
        polymorph_params(p);
    }
    ty(p);
    if p.at(T!['{']) {
        block(p);
    } else if !p.eat(T![;]) {
        p.error_recover("expected block or `;`", FIRST_ITEM);
    }
    m.complete(p, SyntaxKind::PROC_ITEM);
}

fn param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_PARAM) {
            param(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected parameter", RECOVER_PARAM_LIST);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::PARAM_LIST);
}

fn param(p: &mut Parser) {
    let m = p.start();
    p.eat(T![mut]);
    name(p);
    p.expect(T![:]);

    if p.at(T![#]) {
        directive(p);
    } else {
        ty(p);
    }
    m.complete(p, SyntaxKind::PARAM);
}

fn enum_item(p: &mut Parser, m: Marker) {
    p.bump(T![enum]);
    name(p);
    if p.at(T!['(']) {
        polymorph_params(p);
    }
    if p.peek().as_basic_type().is_some() {
        p.bump(p.peek());
    }
    if p.at(T!['{']) {
        variant_list(p);
    } else {
        p.error_recover("expected variant list", RECOVER_VARIANT_LIST);
    }
    m.complete(p, SyntaxKind::ENUM_ITEM);
}

fn variant_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        if p.at_set(FIRST_VARIANT) {
            variant(p);
            if !p.at(T!['}']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected variant", RECOVER_VARIANT_LIST);
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::VARIANT_LIST);
}

fn variant(p: &mut Parser) {
    let m = if p.at(T![#]) {
        let mc = directive_list(p);
        p.start_before(mc)
    } else {
        p.start()
    };

    name(p);
    if p.eat(T![=]) {
        expr(p);
    } else if p.at(T!['(']) {
        variant_field_list(p);
    }
    m.complete(p, SyntaxKind::VARIANT);
}

fn variant_field_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_TYPE) {
            ty(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected field type", RECOVER_VARIANT_FIELD_LIST);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::VARIANT_FIELD_LIST);
}

fn struct_item(p: &mut Parser, m: Marker) {
    p.bump(T![struct]);
    name(p);
    if p.at(T!['(']) {
        polymorph_params(p);
    }
    if p.at(T!['{']) {
        field_list(p);
    } else {
        p.error_recover("expected field list", RECOVER_FIELD_LIST);
    }
    m.complete(p, SyntaxKind::STRUCT_ITEM);
}

fn field_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        if p.at_set(FIRST_FIELD) {
            field(p);
            if !p.at(T!['}']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected field", RECOVER_FIELD_LIST);
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::FIELD_LIST);
}

fn field(p: &mut Parser) {
    let m = if p.at(T![#]) {
        let mc = directive_list(p);
        p.start_before(mc)
    } else {
        p.start()
    };

    name(p);
    p.expect(T![:]);
    ty(p);
    m.complete(p, SyntaxKind::FIELD);
}

fn const_item(p: &mut Parser, m: Marker) {
    name_bump(p);
    if p.eat(T![:]) {
        ty(p);
    }
    p.expect(T![=]);
    expr(p);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::CONST_ITEM);
}

fn global_item(p: &mut Parser, m: Marker) {
    p.bump(T![let]);
    p.eat(T![mut]);
    name(p);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    if !p.eat(T![zeroed]) {
        expr(p);
    }
    p.expect(T![;]);
    m.complete(p, SyntaxKind::GLOBAL_ITEM);
}

fn import_item(p: &mut Parser, m: Marker) {
    p.bump(T![import]);
    if p.at(T![ident]) && p.at_next(T![:]) {
        name(p);
        p.bump(T![:]);
    }
    if p.at(T![ident]) {
        import_path(p);
    } else {
        p.error_recover("expected import path", RECOVER_IMPORT_PATH);
    }
    if p.at(T![as]) {
        import_symbol_rename(p);
    }
    if p.eat(T![.]) {
        if p.at(T!['{']) {
            import_symbol_list(p);
        } else {
            p.error_recover("expected import symbol list", RECOVER_IMPORT_SYMBOL_LIST);
        }
        p.eat(T![;]);
    } else {
        p.expect(T![;]);
    }
    m.complete(p, SyntaxKind::IMPORT_ITEM);
}

fn import_path(p: &mut Parser) {
    let m = p.start();
    name(p);
    while p.eat(T![/]) {
        name(p);
    }
    m.complete(p, SyntaxKind::IMPORT_PATH);
}

fn import_symbol_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            import_symbol(p);
            if !p.at(T!['}']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected import symbol", RECOVER_IMPORT_SYMBOL_LIST);
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::IMPORT_SYMBOL_LIST);
}

fn import_symbol(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.at(T![as]) {
        import_symbol_rename(p);
    }
    m.complete(p, SyntaxKind::IMPORT_SYMBOL);
}

fn import_symbol_rename(p: &mut Parser) {
    let m = p.start();
    p.bump(T![as]);
    if !p.eat(T![_]) {
        name(p);
    }
    m.complete(p, SyntaxKind::IMPORT_SYMBOL_RENAME);
}

//==================== DIRECTIVE ====================

fn directive_list(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    while p.at(T![#]) {
        directive(p);
    }
    m.complete(p, SyntaxKind::DIRECTIVE_LIST)
}

fn directive_list_item(p: &mut Parser) -> (MarkerClosed, bool) {
    let m = p.start();
    let mut scope = false;
    while p.at(T![#]) {
        scope = directive(p).1;
        if scope {
            break;
        }
    }
    (m.complete(p, SyntaxKind::DIRECTIVE_LIST), scope)
}

fn directive(p: &mut Parser) -> (MarkerClosed, bool) {
    let m = p.start();
    p.bump(T![#]);

    let mut scope = false;
    let kind = if p.at(T![ident]) {
        match p.current_token_string() {
            "scope_public" | "scope_package" | "scope_private" => {
                scope = true;
                SyntaxKind::DIRECTIVE_SIMPLE
            }
            "config" | "config_any" | "config_not" => SyntaxKind::DIRECTIVE_WITH_PARAMS,
            _ => SyntaxKind::DIRECTIVE_SIMPLE,
        }
    } else {
        SyntaxKind::DIRECTIVE_SIMPLE
    };

    name(p);
    if kind == SyntaxKind::DIRECTIVE_WITH_PARAMS {
        if p.at(T!['(']) {
            directive_param_list(p);
        } else {
            p.error_recover("expected directive parameter list", RECOVER_DIRECTIVE_PARAM_LIST);
        }
    }
    (m.complete(p, kind), scope)
}

fn directive_param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            directive_param(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected directive parameter", RECOVER_DIRECTIVE_PARAM);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::DIRECTIVE_PARAM_LIST);
}

fn directive_param(p: &mut Parser) {
    let m = p.start();
    name_bump(p);
    p.expect(T![=]);
    if p.at(T![string_lit]) {
        let m = p.start();
        p.bump(T![string_lit]);
        m.complete(p, SyntaxKind::LIT_STRING);
    } else {
        p.expect(T![string_lit]);
    }
    m.complete(p, SyntaxKind::DIRECTIVE_PARAM);
}

//==================== TYPE ====================

fn ty(p: &mut Parser) {
    if p.peek().as_basic_type().is_some() {
        let m = p.start();
        p.bump(p.peek());
        m.complete(p, SyntaxKind::TYPE_BASIC);
        return;
    }

    match p.peek() {
        T![ident] => {
            let m = p.start();
            path_type(p);
            m.complete(p, SyntaxKind::TYPE_CUSTOM);
        }
        T![&] => {
            let m = p.start();
            p.bump(T![&]);
            p.eat(T![mut]);
            ty(p);
            m.complete(p, SyntaxKind::TYPE_REFERENCE);
        }
        T![proc] => type_proc(p),
        T!['['] => type_at_bracket(p),
        _ => p.error("expected type"),
    }
}

fn type_proc(p: &mut Parser) {
    let m = p.start();
    p.bump(T![proc]);
    if p.at(T![#]) {
        directive(p);
    }
    if p.at(T!['(']) {
        proc_type_param_list(p);
    } else {
        p.error_recover("expected parameter list", RECOVER_PROC_TYPE_PARAM_LIST);
    }
    ty(p);
    m.complete(p, SyntaxKind::TYPE_PROCEDURE);
}

fn proc_type_param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_PROC_TYPE_PARAM) {
            proc_type_param(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected parameter", RECOVER_PROC_TYPE_PARAM_LIST);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::PROC_TYPE_PARAM_LIST);
}

fn proc_type_param(p: &mut Parser) {
    let m = p.start();
    if p.at_next(T![:]) {
        name(p);
        p.expect(T![:]);
    }
    if p.at(T![#]) {
        directive(p);
    } else {
        ty(p);
    }
    m.complete(p, SyntaxKind::PROC_TYPE_PARAM);
}

fn type_at_bracket(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['[']);

    if p.at(T![']']) || p.at(T![mut]) {
        p.eat(T![mut]);
        p.expect(T![']']);
        ty(p);
        m.complete(p, SyntaxKind::TYPE_ARRAY_SLICE);
    } else if p.eat(T![&]) {
        p.eat(T![mut]);
        p.expect(T![']']);
        ty(p);
        m.complete(p, SyntaxKind::TYPE_MULTI_REFERENCE);
    } else {
        expr(p);
        p.expect(T![']']);
        ty(p);
        m.complete(p, SyntaxKind::TYPE_ARRAY_STATIC);
    }
}

//==================== STMT ====================

fn block(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        stmt(p);
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::BLOCK)
}

fn block_expect(p: &mut Parser) {
    if p.at(T!['{']) {
        block(p);
    } else {
        p.error("expected block");
    }
}

fn stmt(p: &mut Parser) {
    match p.peek() {
        T![break] => stmt_break(p),
        T![continue] => stmt_continue(p),
        T![return] => stmt_return(p),
        T![defer] => stmt_defer(p),
        T![for] => stmt_for(p),
        T![let] => stmt_local(p),
        T![#] => stmt_with_directive(p),
        _ => stmt_assign_or_expr_semi(p),
    }
}

fn stmt_break(p: &mut Parser) {
    let m = p.start();
    p.bump(T![break]);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::STMT_BREAK);
}

fn stmt_continue(p: &mut Parser) {
    let m = p.start();
    p.bump(T![continue]);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::STMT_CONTINUE);
}

fn stmt_return(p: &mut Parser) {
    let m = p.start();
    p.bump(T![return]);
    if !p.at(T![;]) {
        expr(p);
    }
    p.expect(T![;]);
    m.complete(p, SyntaxKind::STMT_RETURN);
}

fn stmt_defer(p: &mut Parser) {
    let m = p.start();
    p.bump(T![defer]);
    if p.at(T!['{']) {
        block(p);
        p.eat(T![;]);
    } else {
        stmt(p);
    }
    m.complete(p, SyntaxKind::STMT_DEFER);
}

fn stmt_for(p: &mut Parser) {
    let m = p.start();
    p.bump(T![for]);

    match p.peek() {
        T!['{'] => {}
        T![&] => {
            let mh = p.start();
            p.bump(T![&]);
            p.eat(T![mut]);
            for_bind(p);
            if p.eat(T![,]) {
                for_bind(p);
            }
            p.expect(T![in]);
            p.eat(T![<<]);
            expr(p);
            if p.eat(T!["..<"]) || p.eat(T!["..="]) {
                expr(p);
                mh.complete(p, SyntaxKind::FOR_HEADER_RANGE);
            } else {
                mh.complete(p, SyntaxKind::FOR_HEADER_ELEM);
            }
        }
        T![ident] | T![_] => {
            if p.at_next(T![,]) || p.at_next(T![in]) {
                let mh = p.start();
                for_bind(p);
                if p.eat(T![,]) {
                    for_bind(p);
                }
                p.expect(T![in]);
                p.eat(T![<<]);
                expr(p);
                if p.eat(T!["..<"]) || p.eat(T!["..="]) {
                    expr(p);
                    mh.complete(p, SyntaxKind::FOR_HEADER_RANGE);
                } else {
                    mh.complete(p, SyntaxKind::FOR_HEADER_ELEM);
                }
            } else {
                let mh = p.start();
                expr(p);
                mh.complete(p, SyntaxKind::FOR_HEADER_COND);
            }
        }
        T![let] => {
            let mh = p.start();
            p.bump(T![let]);
            pat(p);
            p.expect(T![=]);
            expr(p);
            mh.complete(p, SyntaxKind::FOR_HEADER_PAT);
        }
        _ => {
            let mh = p.start();
            expr(p);
            mh.complete(p, SyntaxKind::FOR_HEADER_COND);
        }
    }

    block_expect(p);
    m.complete(p, SyntaxKind::STMT_FOR);
}

fn for_bind(p: &mut Parser) {
    let m = p.start();
    if p.at(T![ident]) {
        name(p);
    } else if !p.eat(T![_]) {
        p.error("expected name or `_`");
    }
    m.complete(p, SyntaxKind::FOR_BIND);
}

fn stmt_local(p: &mut Parser) {
    let m = p.start();
    p.bump(T![let]);
    bind(p);
    if p.eat(T![:]) {
        ty(p);
    }
    p.expect(T![=]);
    if !p.eat(T![zeroed]) && !p.eat(T![undefined]) {
        expr(p);
    }
    p.expect(T![;]);
    m.complete(p, SyntaxKind::STMT_LOCAL);
}

fn stmt_with_directive(p: &mut Parser) {
    let mc = directive_list(p);
    let m = p.start_before(mc);
    stmt(p);
    m.complete(p, SyntaxKind::STMT_WITH_DIRECTIVE);
}

fn stmt_assign_or_expr_semi(p: &mut Parser) {
    let m = p.start();
    expr(p);

    if p.peek().as_assign_op().is_some() {
        p.bump(p.peek());
        expr(p);
        p.expect(T![;]);
        m.complete(p, SyntaxKind::STMT_ASSIGN);
    } else if p.at(T!['}']) {
        m.complete(p, SyntaxKind::STMT_EXPR_TAIL);
    } else if p.at_prev(T!['}']) {
        p.eat(T![;]);
        m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
    } else {
        p.expect(T![;]);
        m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
    }
}

//==================== EXPR ====================

fn expr(p: &mut Parser) {
    sub_expr(p, 0);
}

fn sub_expr(p: &mut Parser, min_prec: u32) {
    let mut mc_curr = primary_expr(p);

    while p.at(T![as]) {
        let m = p.start_before(mc_curr);
        p.bump(T![as]);
        ty(p);
        mc_curr = m.complete(p, SyntaxKind::EXPR_CAST)
    }

    while let Some(bin_op) = p.peek().as_bin_op() {
        let prec = bin_op.prec();
        if prec < min_prec {
            return;
        }
        let m = p.start_before(mc_curr);
        p.bump(p.peek());
        sub_expr(p, prec + 1);
        mc_curr = m.complete(p, SyntaxKind::EXPR_BINARY);
    }
}

fn primary_expr(p: &mut Parser) -> MarkerClosed {
    if p.at(T!['(']) {
        let m = p.start();
        p.bump(T!['(']);
        sub_expr(p, 0);
        p.expect(T![')']);
        let mc = m.complete(p, SyntaxKind::EXPR_PAREN);
        return tail_expr(p, mc);
    }

    if p.peek().as_un_op().is_some() {
        let m = p.start();
        p.bump(p.peek());
        primary_expr(p);
        return m.complete(p, SyntaxKind::EXPR_UNARY);
    }

    let mc = match p.peek() {
        T![void]
        | T![null]
        | T![true]
        | T![false]
        | T![int_lit]
        | T![float_lit]
        | T![char_lit]
        | T![string_lit] => lit(p),
        T![if] => expr_if(p),
        T!['{'] => block(p),
        T![match] => expr_match(p),
        T![@] => expr_builtin(p),
        T![#] => directive(p).0,
        T![ident] => expr_item_or_struct_init(p),
        T![.] => expr_variant_or_struct_init(p),
        T!['['] => expr_array_init_or_repeat(p),
        T![*] => expr_deref(p),
        T![&] => expr_address(p),
        _ => p.error_recover("expected expression", TokenSet::empty()),
    };

    tail_expr(p, mc)
}

fn tail_expr(p: &mut Parser, mut mc: MarkerClosed) -> MarkerClosed {
    loop {
        match p.peek() {
            T![.] => {
                let m = p.start_before(mc);
                p.bump(T![.]);
                name(p);
                mc = m.complete(p, SyntaxKind::EXPR_FIELD);
            }
            T!['['] => {
                let m = p.start_before(mc);
                p.bump(T!['[']);

                if p.eat(T![..]) {
                    p.expect(T![']']);
                    mc = m.complete(p, SyntaxKind::EXPR_SLICE);
                } else if p.eat(T!["..<"]) || p.eat(T!["..="]) {
                    expr(p);
                    p.expect(T![']']);
                    mc = m.complete(p, SyntaxKind::EXPR_SLICE);
                } else {
                    expr(p);
                    if p.eat(T![..]) {
                        p.expect(T![']']);
                        mc = m.complete(p, SyntaxKind::EXPR_SLICE);
                    } else if p.eat(T!["..<"]) || p.eat(T!["..="]) {
                        expr(p);
                        p.expect(T![']']);
                        mc = m.complete(p, SyntaxKind::EXPR_SLICE);
                    } else {
                        p.expect(T![']']);
                        mc = m.complete(p, SyntaxKind::EXPR_INDEX);
                    }
                }
            }
            T!['('] => {
                let m = p.start_before(mc);
                args_list(p);
                mc = m.complete(p, SyntaxKind::EXPR_CALL);
            }
            _ => return mc,
        }
    }
}

fn expr_if(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let mb = p.start();
    p.bump(T![if]);
    expr(p);
    block_expect(p);
    mb.complete(p, SyntaxKind::IF_BRANCH);

    while p.eat(T![else]) {
        if p.at(T![if]) {
            let mb = p.start();
            p.bump(T![if]);
            expr(p);
            block_expect(p);
            mb.complete(p, SyntaxKind::IF_BRANCH);
        } else {
            block_expect(p);
            break;
        }
    }

    m.complete(p, SyntaxKind::EXPR_IF)
}

fn expr_match(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![match]);
    expr(p);
    if p.at(T!['{']) {
        match_arm_list(p);
    } else {
        p.error("expected match arm list");
    }
    m.complete(p, SyntaxKind::EXPR_MATCH)
}

fn match_arm_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        match_arm(p);
        if !p.at(T!['}']) {
            p.expect(T![,]);
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::MATCH_ARM_LIST);
}

fn match_arm(p: &mut Parser) {
    let m = p.start();
    pat(p);
    p.expect(T![->]);
    expr(p);
    m.complete(p, SyntaxKind::MATCH_ARM);
}

fn expr_builtin(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![@]);

    let kind = if p.at(T![ident]) {
        match p.current_token_string() {
            "size_of" | "align_of" => SyntaxKind::BUILTIN_WITH_TYPE,
            "transmute" => SyntaxKind::BUILTIN_TRANSMUTE,
            _ => SyntaxKind::BUILTIN_ERROR,
        }
    } else {
        SyntaxKind::BUILTIN_ERROR
    };
    name(p);

    p.expect(T!['(']);
    match kind {
        SyntaxKind::BUILTIN_ERROR => {}
        SyntaxKind::BUILTIN_WITH_TYPE => {
            ty(p);
        }
        SyntaxKind::BUILTIN_TRANSMUTE => {
            expr(p);
            p.expect(T![,]);
            ty(p);
        }
        _ => {}
    }
    p.expect(T![')']);
    m.complete(p, kind)
}

fn expr_item_or_struct_init(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let mp = p.start();
    path_segment_expr(p);
    while p.eat(T![.]) {
        if p.at(T!['{']) {
            mp.complete(p, SyntaxKind::PATH);
            field_init_list(p);
            return m.complete(p, SyntaxKind::EXPR_STRUCT_INIT);
        }
        path_segment_expr(p);
    }
    mp.complete(p, SyntaxKind::PATH);

    if p.at(T!['(']) {
        args_list(p);
    }
    m.complete(p, SyntaxKind::EXPR_ITEM)
}

fn expr_variant_or_struct_init(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![.]);

    if p.at(T!['{']) {
        field_init_list(p);
        m.complete(p, SyntaxKind::EXPR_STRUCT_INIT)
    } else {
        name(p);
        if p.at(T!['(']) {
            args_list(p);
        }
        m.complete(p, SyntaxKind::EXPR_VARIANT)
    }
}

fn field_init_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            field_init(p);
            if !p.at(T!['}']) {
                p.expect(T![,]);
            }
        } else {
            p.error("expected field initializer");
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::FIELD_INIT_LIST);
}

fn field_init(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.eat(T![:]) {
        expr(p);
    }
    m.complete(p, SyntaxKind::FIELD_INIT);
}

fn expr_array_init_or_repeat(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T!['[']);
    while !p.at(T![']']) && !p.at(T![eof]) {
        expr(p);
        if p.eat(T![;]) {
            expr(p);
            p.expect(T![']']);
            return m.complete(p, SyntaxKind::EXPR_ARRAY_REPEAT);
        }
        if !p.at(T![']']) {
            p.expect(T![,]);
        }
    }
    p.expect(T![']']);
    m.complete(p, SyntaxKind::EXPR_ARRAY_INIT)
}

fn expr_deref(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![*]);
    primary_expr(p);
    m.complete(p, SyntaxKind::EXPR_DEREF)
}

fn expr_address(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![&]);
    p.eat(T![mut]);
    primary_expr(p);
    m.complete(p, SyntaxKind::EXPR_ADDRESS)
}

//==================== PAT ====================

fn pat(p: &mut Parser) {
    let mc = primary_pat(p);
    if p.at(T![|]) {
        let m = p.start_before(mc);
        while p.eat(T![|]) {
            primary_pat(p);
        }
        m.complete(p, SyntaxKind::PAT_OR);
    }
}

fn primary_pat(p: &mut Parser) -> MarkerClosed {
    match p.peek() {
        T![_] => {
            let m = p.start();
            p.bump(T![_]);
            m.complete(p, SyntaxKind::PAT_WILD)
        }
        T![void]
        | T![null]
        | T![true]
        | T![false]
        | T![int_lit]
        | T![float_lit]
        | T![char_lit]
        | T![string_lit] => {
            let m = p.start();
            lit(p);
            m.complete(p, SyntaxKind::PAT_LIT)
        }
        T![-] => {
            let m = p.start();
            p.bump(T![-]);
            if p.at(T![int_lit]) {
                let ml = p.start();
                p.bump(T![int_lit]);
                ml.complete(p, SyntaxKind::LIT_INT);
            } else {
                p.error("expected integer literal");
            }
            m.complete(p, SyntaxKind::PAT_LIT)
        }
        T![ident] => {
            let m = p.start();
            path_expr(p);
            if p.at(T!['(']) {
                bind_list(p);
            }
            m.complete(p, SyntaxKind::PAT_ITEM)
        }
        T![.] => {
            let m = p.start();
            p.bump(T![.]);
            name(p);
            if p.at(T!['(']) {
                bind_list(p);
            }
            m.complete(p, SyntaxKind::PAT_VARIANT)
        }
        _ => p.error_recover("expected pattern", TokenSet::empty()),
    }
}

fn lit(p: &mut Parser) -> MarkerClosed {
    match p.peek() {
        T![void] => {
            let m = p.start();
            p.bump(T![void]);
            m.complete(p, SyntaxKind::LIT_VOID)
        }
        T![null] => {
            let m = p.start();
            p.bump(T![null]);
            m.complete(p, SyntaxKind::LIT_NULL)
        }
        T![true] | T![false] => {
            let m = p.start();
            p.bump(p.peek());
            m.complete(p, SyntaxKind::LIT_BOOL)
        }
        T![int_lit] => {
            let m = p.start();
            p.bump(T![int_lit]);
            m.complete(p, SyntaxKind::LIT_INT)
        }
        T![float_lit] => {
            let m = p.start();
            p.bump(T![float_lit]);
            m.complete(p, SyntaxKind::LIT_FLOAT)
        }
        T![char_lit] => {
            let m = p.start();
            p.bump(T![char_lit]);
            m.complete(p, SyntaxKind::LIT_CHAR)
        }
        T![string_lit] => {
            let m = p.start();
            p.bump(T![string_lit]);
            m.complete(p, SyntaxKind::LIT_STRING)
        }
        _ => unreachable!(),
    }
}

//==================== COMMON ====================

fn name(p: &mut Parser) {
    let m = p.start();
    if p.eat(T![ident]) {
        m.complete(p, SyntaxKind::NAME);
    } else {
        p.expect(T![ident]);
        m.complete(p, SyntaxKind::ERROR);
    }
}

fn name_bump(p: &mut Parser) {
    let m = p.start();
    p.bump(T![ident]);
    m.complete(p, SyntaxKind::NAME);
}

fn bind(p: &mut Parser) {
    let m = p.start();
    if p.eat(T![mut]) || p.at(T![ident]) {
        name(p);
    } else if !p.eat(T![_]) {
        p.error("expected name or `_`");
    }
    m.complete(p, SyntaxKind::BIND);
}

fn bind_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_BIND) {
            bind(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error("expected binding");
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::BIND_LIST);
}

fn args_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        expr(p);
        if !p.at(T![')']) {
            p.expect(T![,]);
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::ARGS_LIST);
}

fn path_type(p: &mut Parser) {
    let m = p.start();
    path_segment_type(p);
    while p.eat(T![.]) {
        path_segment_type(p);
    }
    m.complete(p, SyntaxKind::PATH);
}

fn path_expr(p: &mut Parser) {
    let m = p.start();
    path_segment_expr(p);
    while p.eat(T![.]) {
        path_segment_expr(p);
    }
    m.complete(p, SyntaxKind::PATH);
}

fn path_segment_type(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.at(T!['(']) {
        polymorph_args(p);
    }
    m.complete(p, SyntaxKind::PATH_SEGMENT);
}

fn path_segment_expr(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.eat(T![:]) {
        polymorph_args(p);
    }
    m.complete(p, SyntaxKind::PATH_SEGMENT);
}

fn polymorph_args(p: &mut Parser) {
    let m = p.start();
    p.expect(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_TYPE) {
            ty(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected type argument", TokenSet::empty());
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::POLYMORPH_ARGS);
}

fn polymorph_params(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            name(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected type parameter", TokenSet::empty());
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::POLYMORPH_PARAMS);
}

//==================== FIRST SETS ====================

#[rustfmt::skip]
const FIRST_ITEM: TokenSet = TokenSet::new(&[T![#], T![proc], T![enum], T![struct], T![ident], T![let], T![import]]);
const FIRST_PARAM: TokenSet = TokenSet::new(&[T![mut], T![ident]]);
const FIRST_VARIANT: TokenSet = TokenSet::new(&[T![#], T![ident]]);
const FIRST_FIELD: TokenSet = TokenSet::new(&[T![#], T![ident]]);
const FIRST_BIND: TokenSet = TokenSet::new(&[T![mut], T![ident], T![_]]);
#[rustfmt::skip]
const FIRST_TYPE: TokenSet = TokenSet::new(&[
    T![char], T![void], T![never], T![rawptr],
    T![s8], T![s16], T![s32], T![s64], T![ssize],
    T![u8], T![u16], T![u32], T![u64], T![usize],
    T![f32], T![f64], T![bool], T![bool16], T![bool32], T![bool64],
    T![string], T![cstring], T![ident], T![&], T![proc], T!['['],
]);
const FIRST_PROC_TYPE_PARAM: TokenSet = FIRST_TYPE.combine(TokenSet::new(&[T![ident], T![#]]));

//==================== RECOVER SETS ====================

const RECOVER_DIRECTIVE_PARAM_LIST: TokenSet = FIRST_ITEM;
const RECOVER_DIRECTIVE_PARAM: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![')']]));
const RECOVER_PARAM_LIST: TokenSet =
    FIRST_ITEM.combine(FIRST_TYPE).combine(TokenSet::new(&[T!['('], T!['{'], T![;]]));
const RECOVER_VARIANT_LIST: TokenSet = FIRST_ITEM;
const RECOVER_VARIANT_FIELD_LIST: TokenSet =
    FIRST_ITEM.combine(TokenSet::new(&[T![,], T![')'], T!['}'], T![ident]]));
const RECOVER_FIELD_LIST: TokenSet = FIRST_ITEM;
const RECOVER_IMPORT_PATH: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![as], T![.], T![;]]));
const RECOVER_IMPORT_SYMBOL_LIST: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![;]]));
const RECOVER_PROC_TYPE_PARAM_LIST: TokenSet =
    FIRST_ITEM.combine(FIRST_TYPE).combine(TokenSet::new(&[T![')']]));

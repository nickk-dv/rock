use super::parser::{Marker, MarkerClosed, Parser};
use super::syntax_kind::SyntaxKind;
use super::token_set::TokenSet;
use crate::token::{Token, T};

pub fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(T![eof]) {
        if p.at_set(FIRST_ITEM) {
            item(p);
        } else {
            //@is this sync useful?
            p.error("expected item");
            p.sync_to(FIRST_ITEM);
        }
    }
    m.complete(p, SyntaxKind::SOURCE_FILE);
}

fn item(p: &mut Parser) {
    let mut mc = None;

    if p.at(T![#]) {
        let m = p.start();
        while p.at(T![#]) {
            attribute(p);
        }
        mc = Some(m.complete(p, SyntaxKind::ATTRIBUTE_LIST));
    }

    //@not used in import, ignored without errors
    if p.at(T![pub]) {
        let mc_vis = visibility(p);
        if mc.is_none() {
            mc = Some(mc_vis);
        }
    }

    let m = if let Some(mc) = mc {
        p.start_before(mc)
    } else {
        p.start()
    };

    match p.peek() {
        T![proc] => proc_item(p, m),
        T![enum] => enum_item(p, m),
        T![struct] => struct_item(p, m),
        T![const] => const_item(p, m),
        T![global] => global_item(p, m),
        T![import] => import_item(p, m),
        _ => {
            p.error("expected item");
            p.sync_to(FIRST_ITEM);
            m.complete(p, SyntaxKind::ERROR);
        }
    }
}

fn attribute(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![#]);
    if p.eat(T!['[']) {
        name(p);
        p.expect(T![']']);
    } else {
        p.expect(T!['[']);
    }
    m.complete(p, SyntaxKind::ATTRIBUTE)
}

fn visibility(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![pub]);
    m.complete(p, SyntaxKind::VISIBILITY)
}

const FIRST_ITEM: TokenSet = TokenSet::new(&[
    T![#],
    T![pub],
    T![proc],
    T![enum],
    T![struct],
    T![const],
    T![global],
    T![import],
]);

const FIRST_PARAM: TokenSet = TokenSet::new(&[T![mut], T![ident]]);

const RECOVER_PARAM_LIST: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![->], T!['{'], T![;]]));
const RECOVER_VARIANT_LIST: TokenSet = FIRST_ITEM;
const RECOVER_FIELD_LIST: TokenSet = FIRST_ITEM;
const RECOVER_IMPORT_PATH: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![as], T![.], T![;]]));
const RECOVER_IMPORT_SYMBOL_LIST: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![;]]));

fn proc_item(p: &mut Parser, m: Marker) {
    p.bump(T![proc]);
    name(p);
    if p.at(T!['(']) {
        param_list(p);
    } else {
        p.error_recover("expected parameter list", RECOVER_PARAM_LIST);
    }
    if p.eat(T![->]) {
        ty(p);
    }
    if p.at(T!['{']) {
        block(p, SyntaxKind::BLOCK);
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
        } else if p.eat(T![..]) {
            break;
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
    ty(p);
    m.complete(p, SyntaxKind::PARAM);
}

fn enum_item(p: &mut Parser, m: Marker) {
    p.bump(T![enum]);
    name(p);
    if p.peek().as_basic_type().is_some() {
        let m = p.start();
        p.bump(p.peek());
        m.complete(p, SyntaxKind::TYPE_BASIC);
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
        if p.at(T![ident]) {
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
    let m = p.start();
    name(p);
    if p.eat(T![=]) {
        expr(p);
    }
    m.complete(p, SyntaxKind::VARIANT);
}

fn struct_item(p: &mut Parser, m: Marker) {
    p.bump(T![struct]);
    name(p);
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
        if p.at(T![ident]) || p.at(T![pub]) {
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
    let m = p.start();
    if p.at(T![pub]) {
        visibility(p);
    }
    name(p);
    p.expect(T![:]);
    ty(p);
    m.complete(p, SyntaxKind::FIELD);
}

fn const_item(p: &mut Parser, m: Marker) {
    p.bump(T![const]);
    name(p);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    expr(p);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::CONST_ITEM);
}

fn global_item(p: &mut Parser, m: Marker) {
    p.bump(T![global]);
    p.eat(T![mut]);
    name(p);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    expr(p);
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
        name_alias(p);
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
        name_alias(p);
    }
    m.complete(p, SyntaxKind::IMPORT_SYMBOL);
}

fn name_alias(p: &mut Parser) {
    let m = p.start();
    p.bump(T![as]);
    name(p);
    m.complete(p, SyntaxKind::NAME_ALIAS);
}

fn name(p: &mut Parser) {
    let m = p.start();
    p.expect(T![ident]);
    m.complete(p, SyntaxKind::NAME);
}

fn path_type(p: &mut Parser) {
    let m = p.start();
    name(p);
    while p.eat(T![.]) {
        name(p);
    }
    m.complete(p, SyntaxKind::PATH);
}

fn path_expr(p: &mut Parser) -> bool {
    let m = p.start();
    name(p);
    while p.eat(T![.]) {
        if p.at(T!['{']) {
            m.complete(p, SyntaxKind::PATH);
            return true;
        }
        name(p);
    }
    m.complete(p, SyntaxKind::PATH);
    false
}

const FIRST_TYPE_SET: TokenSet = TokenSet::new(&[
    T![s8],
    T![s16],
    T![s32],
    T![s64],
    T![ssize],
    T![u8],
    T![u16],
    T![u32],
    T![u64],
    T![usize],
    T![f32],
    T![f64],
    T![bool],
    T![char],
    T![rawptr],
    T![void],
    T![never],
    T![ident],
    T![&],
    T![proc],
    T!['['],
]);

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
        T!['['] => type_slice_or_array(p),
        _ => p.error("expected type"),
    }
}

fn type_proc(p: &mut Parser) {
    let m = p.start();
    p.bump(T![proc]);
    if p.at(T!['(']) {
        param_type_list(p);
    } else {
        p.error_recover("expected parameter type list", RECOVER_PARAM_TYPE_LIST);
    }
    if p.eat(T![->]) {
        ty(p);
    }
    m.complete(p, SyntaxKind::TYPE_PROCEDURE);
}

const RECOVER_PARAM_TYPE_LIST: TokenSet = TokenSet::new(&[T![->]]);

fn param_type_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_TYPE_SET) {
            ty(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else if p.eat(T![..]) {
            break;
        } else {
            p.error_recover("expected parameter type", RECOVER_PARAM_TYPE_LIST);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::PARAM_TYPE_LIST);
}

fn type_slice_or_array(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['[']);
    match p.peek() {
        T![mut] | T![']'] => {
            p.eat(T![mut]);
            p.expect(T![']']);
            ty(p);
            m.complete(p, SyntaxKind::TYPE_ARRAY_SLICE);
        }
        _ => {
            expr(p);
            p.expect(T![']']);
            ty(p);
            m.complete(p, SyntaxKind::TYPE_ARRAY_STATIC);
        }
    }
}

fn stmt(p: &mut Parser) {
    match p.peek() {
        T![break] => {
            let m = p.start();
            p.bump(T![break]);
            p.expect(T![;]);
            m.complete(p, SyntaxKind::STMT_BREAK);
        }
        T![continue] => {
            let m = p.start();
            p.bump(T![continue]);
            p.expect(T![;]);
            m.complete(p, SyntaxKind::STMT_CONTINUE);
        }
        T![return] => {
            let m = p.start();
            p.bump(T![return]);
            if !p.at(T![;]) {
                expr(p);
            }
            p.expect(T![;]);
            m.complete(p, SyntaxKind::STMT_RETURN);
        }
        T![defer] => {
            let m = p.start();
            p.bump(T![defer]);
            if p.at(T!['{']) {
                block(p, SyntaxKind::BLOCK);
            } else {
                let m = p.start();
                stmt(p);
                m.complete(p, SyntaxKind::SHORT_BLOCK);
            }
            m.complete(p, SyntaxKind::STMT_DEFER);
        }
        T![for] => loop_(p),
        T![let] | T![mut] => local(p),
        T![->] => {
            let m = p.start();
            p.bump(T![->]);
            expr(p);
            p.expect(T![;]);
            m.complete(p, SyntaxKind::STMT_EXPR_TAIL);
        }
        _ => {
            let m = p.start();
            expr(p);
            if p.peek().as_assign_op().is_some() {
                p.bump(p.peek());
                expr(p);
                p.expect(T![;]);
                m.complete(p, SyntaxKind::STMT_ASSIGN);
            } else {
                if !p.at_prev(T!['}']) {
                    p.expect(T![;]);
                } else {
                    p.eat(T![;]);
                }
                m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
            }
        }
    }
}

fn loop_(p: &mut Parser) {
    let m = p.start();
    p.bump(T![for]);

    match p.peek() {
        T!['{'] => {}
        T![let] | T![mut] => {
            let mh = p.start();
            local(p);
            expr(p);
            p.expect(T![;]);

            let m = p.start();
            expr(p);
            if p.peek().as_assign_op().is_some() {
                p.bump(p.peek());
                expr(p);
            } else {
                p.error("expected assignment operator");
            }
            m.complete(p, SyntaxKind::STMT_ASSIGN);
            mh.complete(p, SyntaxKind::LOOP_CLIKE_HEADER);
        }
        _ => {
            let mh = p.start();
            expr(p);
            mh.complete(p, SyntaxKind::LOOP_WHILE_HEADER);
        }
    }

    block_expect(p);
    m.complete(p, SyntaxKind::STMT_LOOP);
}

fn local(p: &mut Parser) {
    let m = p.start();
    match p.peek() {
        T![let] => p.bump(T![let]),
        T![mut] => p.bump(T![mut]),
        _ => unreachable!(),
    }
    name(p);
    if p.eat(T![:]) {
        ty(p);
        if p.eat(T![=]) {
            expr(p);
        }
    } else {
        p.expect(T![=]);
        expr(p);
    }
    p.expect(T![;]);
    m.complete(p, SyntaxKind::STMT_LOCAL);
}

fn expr(p: &mut Parser) {
    sub_expr(p, 0);
}

fn sub_expr(p: &mut Parser, min_prec: u32) {
    let mut mc_curr = primary_expr(p);

    loop {
        let prec: u32;
        if let Some(bin_op) = p.peek().as_bin_op() {
            prec = bin_op.prec();
            if prec < min_prec {
                break;
            }
            let m = p.start_before(mc_curr);
            p.bump(p.peek());
            sub_expr(p, prec + 1);
            mc_curr = m.complete(p, SyntaxKind::EXPR_BINARY);
        } else {
            break;
        }
    }
}

fn primary_expr(p: &mut Parser) -> MarkerClosed {
    if p.at(T!['(']) {
        let m = p.start();
        p.bump(T!['(']);
        expr(p);
        p.expect(T![')']);
        let mc = m.complete(p, SyntaxKind::EXPR_PAREN);
        let mc = tail_expr(p, mc);
        return mc;
    }

    if p.peek().as_un_op().is_some() {
        let m = p.start();
        p.bump(p.peek());
        primary_expr(p);
        let mc = m.complete(p, SyntaxKind::EXPR_UNARY);
        return mc;
    }

    let mc = match p.peek() {
        T![null] => {
            let m = p.start();
            p.bump(T![null]);
            m.complete(p, SyntaxKind::EXPR_LIT_NULL)
        }
        T![true] | T![false] => {
            let m = p.start();
            p.bump(p.peek());
            m.complete(p, SyntaxKind::EXPR_LIT_BOOL)
        }
        T![int_lit] => {
            let m = p.start();
            p.bump(T![int_lit]);
            m.complete(p, SyntaxKind::EXPR_LIT_INT)
        }
        T![float_lit] => {
            let m = p.start();
            p.bump(T![float_lit]);
            m.complete(p, SyntaxKind::EXPR_LIT_FLOAT)
        }
        T![char_lit] => {
            let m = p.start();
            p.bump(T![char_lit]);
            m.complete(p, SyntaxKind::EXPR_LIT_CHAR)
        }
        T![string_lit] => {
            let m = p.start();
            p.bump(T![string_lit]);
            m.complete(p, SyntaxKind::EXPR_LIT_STRING)
        }
        T![if] => if_(p),
        T!['{'] => block(p, SyntaxKind::EXPR_BLOCK),
        T![match] => match_(p),
        T![sizeof] => {
            let m = p.start();
            p.bump(T![sizeof]);
            p.expect(T!['(']);
            ty(p);
            p.expect(T![')']);
            m.complete(p, SyntaxKind::EXPR_SIZEOF)
        }
        T![ident] => {
            let m = p.start();
            let field_list = path_expr(p);
            if field_list && p.at(T!['{']) {
                field_init_list(p);
                m.complete(p, SyntaxKind::EXPR_STRUCT_INIT)
            } else {
                m.complete(p, SyntaxKind::EXPR_ITEM)
            }
        }
        T![.] => {
            let m = p.start();
            p.bump(T![.]);
            if p.at(T!['{']) {
                field_init_list(p);
                m.complete(p, SyntaxKind::EXPR_STRUCT_INIT)
            } else {
                name(p);
                m.complete(p, SyntaxKind::EXPR_VARIANT)
            }
        }
        T!['['] => array_expr(p),
        T![*] => {
            let m = p.start();
            p.bump(T![*]);
            expr(p);
            m.complete(p, SyntaxKind::EXPR_DEREF)
        }
        T![&] => {
            let m = p.start();
            p.bump(T![&]);
            p.eat(T![mut]);
            expr(p);
            m.complete(p, SyntaxKind::EXPR_ADDRESS)
        }
        _ => {
            //@return instead? this is double error node
            // or pass marker to be closed with error during error_bump
            let m = p.start();
            p.error_bump("expected expression");
            m.complete(p, SyntaxKind::ERROR)
        }
    };

    tail_expr(p, mc)
}

fn tail_expr(p: &mut Parser, mc: MarkerClosed) -> MarkerClosed {
    let mut last_cast = false;
    let mut mc_curr = mc;

    loop {
        match p.peek() {
            T![.] => {
                if last_cast {
                    break;
                }
                let m = p.start_before(mc_curr);
                p.bump(T![.]);
                name(p);
                mc_curr = m.complete(p, SyntaxKind::EXPR_FIELD);
            }
            T!['['] => {
                if last_cast {
                    break;
                }
                let m = p.start_before(mc_curr);
                p.bump(T!['[']);
                p.eat(T![mut]);
                if !p.at(T![..]) {
                    expr(p);
                }
                if p.eat(T![..]) {
                    if !p.at(T![']']) {
                        expr(p);
                    }
                }
                p.expect(T![']']);
                mc_curr = m.complete(p, SyntaxKind::EXPR_INDEX);
            }
            T!['('] => {
                if last_cast {
                    break;
                }
                let m = p.start_before(mc_curr);
                call_argument_list(p);
                mc_curr = m.complete(p, SyntaxKind::EXPR_CALL);
            }
            T![as] => {
                last_cast = true;
                let m = p.start_before(mc_curr);
                p.bump(T![as]);
                ty(p);
                mc_curr = m.complete(p, SyntaxKind::EXPR_CAST);
            }
            _ => break,
        }
    }

    mc_curr
}

fn if_(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let eb = p.start();
    p.bump(T![if]);
    expr(p);
    block_expect(p);
    eb.complete(p, SyntaxKind::ENTRY_BRANCH);

    //@else not included in ELSE_IF_BRANCH?
    while p.eat(T![else]) {
        if p.at(T![if]) {
            let mb = p.start();
            p.bump(T![if]);
            expr(p);
            block_expect(p);
            mb.complete(p, SyntaxKind::ELSE_IF_BRANCH);
        } else {
            block_expect(p);
            break;
        }
    }

    m.complete(p, SyntaxKind::EXPR_IF)
}

fn block(p: &mut Parser, kind: SyntaxKind) -> MarkerClosed {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        stmt(p);
    }
    p.expect(T!['}']);
    m.complete(p, kind)
}

fn block_expect(p: &mut Parser) {
    if p.at(T!['{']) {
        block(p, SyntaxKind::BLOCK);
    } else {
        p.error("expected block");
    }
}

fn match_(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![match]);
    expr(p);
    if p.at(T!['{']) {
        match_arm_list(p);
    } else {
        p.error_bump("expected match arm list");
    }
    m.complete(p, SyntaxKind::EXPR_MATCH)
}

fn match_arm_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        let fallback = match_arm(p);
        if !p.at(T!['}']) {
            p.expect(T![,]);
        }
        if fallback {
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::MATCH_ARM_LIST);
}

fn match_arm(p: &mut Parser) -> bool {
    let m = p.start();
    if p.eat(T![_]) {
        p.expect(T![->]);
        expr(p);
        m.complete(p, SyntaxKind::MATCH_FALLBACK);
        true
    } else {
        expr(p);
        p.expect(T![->]);
        expr(p);
        m.complete(p, SyntaxKind::MATCH_ARM);
        false
    }
}

fn call_argument_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        expr(p);
        if !p.at(T![')']) {
            p.expect(T![,]);
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::CALL_ARGUMENT_LIST);
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
            //@bump or not?
            p.error_bump("expected field initializer");
            break;
        }
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::FIELD_INIT_LIST);
}

fn field_init(p: &mut Parser) {
    let m = p.start();
    if p.at_next(T![:]) {
        name(p);
        p.bump(T![:]);
    }
    expr(p);
    m.complete(p, SyntaxKind::FIELD_INIT);
}

fn array_expr(p: &mut Parser) -> MarkerClosed {
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

use super::parser::{Marker, MarkerClosed, Parser};
use super::syntax_kind::SyntaxKind;
use crate::token::{Token, TokenSet, T};

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
        mc = Some(attr_list(p));
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

fn attr_list(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    while p.at(T![#]) {
        attr(p);
    }
    m.complete(p, SyntaxKind::ATTR_LIST)
}

fn attr(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![#]);
    p.expect(T!['[']);
    name(p);
    if p.at(T!['(']) {
        attr_param_list(p);
    }
    p.expect(T![']']);
    m.complete(p, SyntaxKind::ATTR)
}

fn attr_param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            attr_param(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected attribute parameter", RECOVER_ATTR_PARAM);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::ATTR_PARAM_LIST);
}

//@basic ty cannot be represented,
// use name intern IDs instead for value & name?
fn attr_param(p: &mut Parser) {
    let m = p.start();
    name(p);
    if p.eat(T![=]) {
        let m = p.start();
        p.expect(T![string_lit]);
        m.complete(p, SyntaxKind::LIT_STRING);
    }
    m.complete(p, SyntaxKind::ATTR_PARAM);
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
const RECOVER_ATTR_PARAM: TokenSet = FIRST_ITEM.combine(TokenSet::new(&[T![')'], T![']']]));

const RECOVER_PARAM_LIST: TokenSet = TokenSet::new(&[T!['('], T!['{'], T![;]])
    .combine(FIRST_ITEM)
    .combine(FIRST_TYPE);
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
    if p.at(T!['(']) {
        generic_params(p);
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
    if p.at(T!['(']) {
        generic_params(p);
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

const FIRST_VARIANT: TokenSet = TokenSet::new(&[T![#], T![ident]]);

fn variant(p: &mut Parser) {
    let m = if p.at(T![#]) {
        let mc = attr_list(p);
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

const RECOVER_VARIANT_FIELD_LIST: TokenSet =
    FIRST_ITEM.combine(TokenSet::new(&[T![,], T![')'], T!['}'], T![ident]]));

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
        generic_params(p);
    }
    if p.at(T!['{']) {
        field_list(p);
    } else {
        p.error_recover("expected field list", RECOVER_FIELD_LIST);
    }
    m.complete(p, SyntaxKind::STRUCT_ITEM);
}

const FIRST_FIELD: TokenSet = TokenSet::new(&[T![#], T![pub], T![ident]]);

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
        let mc = attr_list(p);
        p.start_before(mc)
    } else {
        p.start()
    };

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

//==================== GENERIC ====================

fn generic_params(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            name(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected generic parameter name", TokenSet::empty());
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::GENERIC_PARAMS);
}

fn generic_types(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_TYPE) {
            ty(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected generic type", TokenSet::empty());
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::GENERIC_TYPES);
}

//==================== TYPE ====================

const FIRST_TYPE: TokenSet = TokenSet::new(&[
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
            path(p);
            if p.at(T!['(']) {
                generic_types(p);
                m.complete(p, SyntaxKind::TYPE_GENERIC);
            } else {
                m.complete(p, SyntaxKind::TYPE_CUSTOM);
            }
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
    if p.at(T!['(']) {
        param_type_list(p);
    } else {
        p.error_recover("expected parameter type list", RECOVER_PARAM_TYPE_LIST);
    }
    ty(p);
    m.complete(p, SyntaxKind::TYPE_PROCEDURE);
}

const RECOVER_PARAM_TYPE_LIST: TokenSet = FIRST_ITEM.combine(FIRST_TYPE);

fn param_type_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at_set(FIRST_TYPE) {
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
        T![for] => stmt_loop(p),
        T![let] => stmt_local(p),
        T![#] => stmt_attr_stmt(p),
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

fn stmt_loop(p: &mut Parser) {
    let m = p.start();
    p.bump(T![for]);

    match p.peek() {
        T!['{'] => {}
        T![let] => {
            let mh = p.start();
            stmt_local(p);
            expr(p);
            p.expect(T![;]);

            let ma = p.start();
            expr(p);
            if p.peek().as_assign_op().is_some() {
                p.bump(p.peek());
                expr(p);
            } else {
                p.error("expected assignment operator");
            }
            ma.complete(p, SyntaxKind::STMT_ASSIGN);
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

fn stmt_attr_stmt(p: &mut Parser) {
    let mc = attr_list(p);
    let m = p.start_before(mc);
    stmt(p);
    m.complete(p, SyntaxKind::STMT_ATTR_STMT);
}

fn stmt_assign_or_expr_semi(p: &mut Parser) {
    let m = p.start();
    expr(p);

    if p.peek().as_assign_op().is_some() {
        p.bump(p.peek());
        expr(p);
        p.expect(T![;]);
        m.complete(p, SyntaxKind::STMT_ASSIGN);
    } else {
        if p.at(T!['}']) {
            m.complete(p, SyntaxKind::STMT_EXPR_TAIL);
        } else if p.at_prev(T!['}']) {
            p.eat(T![;]);
            m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
        } else {
            p.expect(T![;]);
            m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
        }
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
        T![sizeof] => expr_sizeof(p),
        T![ident] => expr_item_or_struct_init(p),
        T![.] => expr_variant_or_struct_init(p),
        T!['['] => expr_array_init_or_repeat(p),
        T![*] => expr_deref(p),
        T![&] => expr_address(p),
        T![..] => {
            let m = p.start();
            p.bump(T![..]);
            m.complete(p, SyntaxKind::RANGE_FULL)
        }
        T!["..<"] => {
            let m = p.start();
            p.bump(T!["..<"]);
            expr(p);
            m.complete(p, SyntaxKind::RANGE_TO_EXCLUSIVE)
        }
        T!["..="] => {
            let m = p.start();
            p.bump(T!["..="]);
            expr(p);
            m.complete(p, SyntaxKind::RANGE_TO_INCLUSIVE)
        }
        _ => {
            //@double error node is created, we still need marker_closed for Error
            let m = p.start();
            p.error_bump("expected expression");
            m.complete(p, SyntaxKind::ERROR)
        }
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

                if p.eat(T![:]) {
                    p.eat(T![mut]);
                    expr(p);
                    p.expect(T![']']);
                    mc = m.complete(p, SyntaxKind::EXPR_SLICE);
                } else {
                    expr(p);
                    p.expect(T![']']);
                    mc = m.complete(p, SyntaxKind::EXPR_INDEX);
                }
            }
            T!['('] => {
                let m = p.start_before(mc);
                args_list(p);
                mc = m.complete(p, SyntaxKind::EXPR_CALL);
            }
            T![..] => {
                let m = p.start_before(mc);
                p.bump(T![..]);
                mc = m.complete(p, SyntaxKind::RANGE_FROM);
            }
            T!["..<"] => {
                let m = p.start_before(mc);
                p.bump(T!["..<"]);
                primary_expr(p);
                mc = m.complete(p, SyntaxKind::RANGE_EXCLUSIVE);
            }
            T!["..="] => {
                let m = p.start_before(mc);
                p.bump(T!["..="]);
                primary_expr(p);
                mc = m.complete(p, SyntaxKind::RANGE_INCLUSIVE);
            }
            _ => return mc,
        }
    }
}

fn expr_if(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let me = p.start();
    p.bump(T![if]);
    expr(p);
    block_expect(p);
    me.complete(p, SyntaxKind::IF_BRANCH);

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
        p.error_bump("expected match arm list");
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

fn expr_sizeof(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T![sizeof]);
    p.expect(T!['(']);
    ty(p);
    p.expect(T![')']);
    m.complete(p, SyntaxKind::EXPR_SIZEOF)
}

fn expr_item_or_struct_init(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let mp = p.start();
    name(p);
    while p.eat(T![.]) {
        if p.at(T!['{']) {
            mp.complete(p, SyntaxKind::PATH);
            field_init_list(p);
            return m.complete(p, SyntaxKind::EXPR_STRUCT_INIT);
        }
        name(p);
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
        T![ident] => {
            let m = p.start();
            path(p);
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
        _ => {
            //@double error node is created, we still need marker_closed for Error
            //@add recovery!
            let m = p.start();
            p.error_bump("expected pattern");
            m.complete(p, SyntaxKind::ERROR)
        }
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
    p.expect(T![ident]);
    m.complete(p, SyntaxKind::NAME);
}

fn path(p: &mut Parser) {
    let m = p.start();
    name(p);
    while p.eat(T![.]) {
        name(p);
    }
    m.complete(p, SyntaxKind::PATH);
}

const FIRST_BIND: TokenSet = TokenSet::new(&[T![mut], T![ident], T![_]]);

fn bind(p: &mut Parser) {
    let m = p.start();
    if p.eat(T![mut]) {
        name(p);
    } else if p.at(T![ident]) {
        name(p);
    } else if !p.eat(T![_]) {
        p.error("expected `identifier` or `_`");
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

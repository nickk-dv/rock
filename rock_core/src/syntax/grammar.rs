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
        let m = p.start();
        while p.at(T![#]) {
            attr(p);
        }
        mc = Some(m.complete(p, SyntaxKind::ATTR_LIST));
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
    p.expect(T![->]);
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
        if p.at_set(FIRST_TYPE_SET) {
            ty(p);
            if !p.at(T![')']) {
                p.expect(T![,]);
            }
        } else {
            p.error_recover("expected type", RECOVER_VARIANT_FIELD_LIST);
            break;
        }
    }
    p.expect(T![')']);
    m.complete(p, SyntaxKind::VARIANT_FIELD_LIST);
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

//@remove state
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
        bind(p);
        if !p.at(T![')']) {
            p.expect(T![,]);
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
    p.expect(T![->]);
    ty(p);
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
                block(p);
            } else {
                block_short(p);
            }
            m.complete(p, SyntaxKind::STMT_DEFER);
        }
        T![for] => loop_(p),
        T![let] => local(p),
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
        T![let] => {
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
    p.bump(T![let]);
    bind(p);
    if p.eat(T![:]) {
        ty(p);
    }
    p.expect(T![=]);
    expr(p);
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
        T![null]
        | T![true]
        | T![false]
        | T![int_lit]
        | T![float_lit]
        | T![char_lit]
        | T![string_lit] => lit(p),
        T![if] => if_(p),
        T!['{'] => block(p),
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
                if p.at(T!['(']) {
                    args_list(p);
                }
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
                if p.at(T!['(']) {
                    args_list(p);
                }
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
                p.eat(T![mut]);
                expr(p);
                p.expect(T![']']);
                mc = m.complete(p, SyntaxKind::EXPR_INDEX);
            }
            T!['('] => {
                let m = p.start_before(mc);
                args_list(p);
                mc = m.complete(p, SyntaxKind::EXPR_CALL);
            }
            T![as] => {
                let m = p.start_before(mc);
                p.bump(T![as]);
                ty(p);
                return m.complete(p, SyntaxKind::EXPR_CAST);
            }
            T![..] => {
                let m = p.start_before(mc);
                p.bump(T![..]);
                mc = m.complete(p, SyntaxKind::RANGE_FROM);
            }
            T!["..<"] => {
                let m = p.start_before(mc);
                p.bump(T!["..<"]);
                expr(p);
                mc = m.complete(p, SyntaxKind::RANGE_EXCLUSIVE);
            }
            T!["..="] => {
                let m = p.start_before(mc);
                p.bump(T!["..="]);
                expr(p);
                mc = m.complete(p, SyntaxKind::RANGE_INCLUSIVE);
            }
            _ => return mc,
        }
    }
}

fn lit(p: &mut Parser) -> MarkerClosed {
    match p.peek() {
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

fn if_(p: &mut Parser) -> MarkerClosed {
    let m = p.start();

    let me = p.start();
    p.bump(T![if]);
    expr(p);
    block_expect(p);
    me.complete(p, SyntaxKind::BRANCH_ENTRY);

    while p.at(T![else]) {
        if p.at_next(T![if]) {
            let mb = p.start();
            p.bump(T![else]);
            p.bump(T![if]);
            expr(p);
            block_expect(p);
            mb.complete(p, SyntaxKind::BRANCH_ELSE_IF);
        } else {
            p.bump(T![else]);
            block_expect(p);
            break;
        }
    }

    m.complete(p, SyntaxKind::EXPR_IF)
}

fn block(p: &mut Parser) -> MarkerClosed {
    let m = p.start();
    p.bump(T!['{']);
    while !p.at(T!['}']) && !p.at(T![eof]) {
        stmt(p);
    }
    p.expect(T!['}']);
    m.complete(p, SyntaxKind::BLOCK)
}

fn block_short(p: &mut Parser) {
    let m = p.start();
    stmt(p);
    m.complete(p, SyntaxKind::BLOCK);
}

fn block_expect(p: &mut Parser) {
    if p.at(T!['{']) {
        block(p);
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

fn pat(p: &mut Parser) {
    let mc = primary_pat(p);

    if p.at(T![|]) {
        let m = p.start_before(mc);
        while p.at(T![|]) {
            p.bump(T![|]);
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
        T![null]
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
            //@rename `path_type`? or rework how path works
            // special cast for struct init is in path_expr
            let m = p.start();
            path_type(p);
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

use super::parser::{Event, MarkerClosed, Parser};
use super::syntax_tree::{Node, NodeID, NodeOrToken, SyntaxKind, SyntaxTree, TokenID};
use super::token_set::TokenSet;
use crate::arena::Arena;
use crate::error::{ErrorComp, SourceRange};
use crate::lexer;
use crate::session::FileID;
use crate::temp_buffer::TempBuffer;
use crate::token::{Token, T};

pub fn parse(source: &str, file_id: FileID) -> (SyntaxTree, Vec<ErrorComp>) {
    let tokens = if let Ok(tokens) = lexer::lex(source, file_id, false) {
        tokens
    } else {
        //@temp work-around
        panic!("lexer failed");
    };
    let mut parser = Parser::new(tokens);
    source_file(&mut parser);

    let mut arena = Arena::new();
    let mut nodes = Vec::new();
    let (tokens, mut events) = parser.finish();

    let mut stack = Vec::new();
    let mut parent_stack = Vec::with_capacity(8);
    let mut content_buf = TempBuffer::new();
    let mut token_idx = 0;
    let mut errors = Vec::new();

    pretty_print_events(&events);

    for event_idx in 0..events.len() {
        //@clone to get around borrowing,
        //change the error handling to copy/clone events
        match events[event_idx].clone() {
            Event::StartNode {
                kind,
                forward_parent,
            } => {
                let mut parent_next = forward_parent;
                parent_stack.clear();

                while let Some(parent_idx) = parent_next {
                    let start_event = events[parent_idx as usize].clone();

                    match start_event {
                        Event::StartNode {
                            kind,
                            forward_parent,
                        } => {
                            parent_stack.push(kind);
                            parent_next = forward_parent;
                            events[parent_idx as usize] = Event::Ignore;
                        }
                        _ => unreachable!(),
                    }
                }

                while let Some(kind) = parent_stack.pop() {
                    let node_id = NodeID::new(nodes.len());
                    content_buf.add(NodeOrToken::Node(node_id));
                    let offset = content_buf.start();
                    stack.push((offset, node_id));

                    let node = Node { kind, content: &[] };
                    nodes.push(node);
                }

                let node_id = NodeID::new(nodes.len());
                content_buf.add(NodeOrToken::Node(node_id));
                let offset = content_buf.start();
                stack.push((offset, node_id));

                let node = Node { kind, content: &[] };
                nodes.push(node);
            }
            Event::EndNode => {
                let (offset, node_id) = stack.pop().unwrap();
                nodes[node_id.index()].content = content_buf.take(offset, &mut arena);
            }
            Event::Token { token } => {
                let token_id = TokenID::new(token_idx);
                token_idx += 1;
                content_buf.add(NodeOrToken::Token(token_id));
            }
            Event::Error { message } => {
                let token_id = TokenID::new(token_idx);
                let range = tokens.get_range(token_id.index());
                let src = SourceRange::new(range, file_id);
                errors.push(ErrorComp::new(message, src, None));
            }
            Event::Ignore => {}
        }
    }

    (SyntaxTree::new(arena, nodes, tokens), errors)
}

#[test]
fn parse_test() {
    let source = r#"

    # import
    some garbage
    true false } {
    
    import name.
    proc something(x: , y ) -> {
        let val = 3 + 2 * 5;
        let point = math.point.{x: 10, y: 20,};
        let point2: math. = .{x: 10, y: 20};
        let x: s32 = null;
        let g = (23);
        if true {
            let c = 10;
        } else if false {}
        else {}
    }
    proc something3 ( -> proc (math.Vec3, s32, ..) -> u64;
    enum TileKind {
        L
        B,,
    }
    
    "#;

    fn print_depth(depth: u32) {
        for _ in 0..depth {
            print!("  ");
        }
    }

    fn print_node(tree: &SyntaxTree, node: &Node, depth: u32) {
        print_depth(depth);
        println!("[{:?}]", node.kind);

        for node_or_token in node.content {
            match *node_or_token {
                NodeOrToken::Node(node_id) => {
                    print_node(tree, tree.node(node_id), depth + 1);
                }
                NodeOrToken::Token(token_id) => {
                    let token = tree.token(token_id);
                    let range = tree.token_range(token_id);
                    print_depth(depth + 1);
                    println!("`{}`@{:?}", token.as_str(), range);
                }
            }
        }
    }

    let (tree, errors) = parse(source, FileID::dummy());
    let root = tree.node(NodeID::new(0));
    print_node(&tree, root, 0);
}

fn pretty_print_events(events: &[Event]) {
    println!("EVENTS:");
    let mut depth = 0;
    fn print_depth(depth: i32) {
        for _ in 0..depth {
            print!("  ");
        }
    }
    for e in events {
        match e {
            Event::EndNode => {}
            _ => print_depth(depth),
        }
        match e {
            Event::StartNode {
                kind,
                forward_parent,
            } => {
                if let Some(idx) = *forward_parent {
                    println!("[START] {:?} [FORWARD_PARENT] {}", kind, idx);
                } else {
                    println!("[START] {:?}", kind);
                }
                depth += 1;
            }
            Event::EndNode => {
                depth -= 1;
            }
            Event::Token { token } => println!("TOKEN `{}`", token.as_str()),
            Event::Error { message } => {
                println!("[ERROR] {}", message)
            }
            Event::Ignore => println!("[IGNORE]"),
        }
    }
}

fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(T![eof]) {
        item(p);
    }
    m.complete(p, SyntaxKind::SOURCE_FILE);
}

//@change attribute, pub, item parsing rules
fn item(p: &mut Parser) {
    match p.peek() {
        T![#] => attribute(p),
        T![pub] => visibility(p),
        T![proc] => proc_item(p),
        T![enum] => enum_item(p),
        T![struct] => struct_item(p),
        T![const] => const_item(p),
        T![global] => global_item(p),
        T![import] => import_item(p),
        _ => {
            p.error("expected item");
            p.sync_to(RECOVER_ITEM);
        }
    }
}

fn attribute(p: &mut Parser) {
    let m = p.start();
    p.bump(T![#]);
    if p.eat(T!['[']) {
        p.expect(T![ident]);
        p.expect(T![']']);
    } else {
        p.expect(T!['[']);
        p.sync_to(RECOVER_ITEM);
    }
    m.complete(p, SyntaxKind::ATTRIBUTE);
}

fn visibility(p: &mut Parser) {
    let m = p.start();
    p.bump(T![pub]);
    m.complete(p, SyntaxKind::VISIBILITY);
}

const RECOVER_ITEM: TokenSet = TokenSet::new(&[
    T![#],
    T![pub],
    T![proc],
    T![enum],
    T![struct],
    T![const],
    T![global],
    T![import],
]);

const RECOVER_PARAM_LIST: TokenSet = RECOVER_ITEM.combine(TokenSet::new(&[T![->], T!['{'], T![;]]));
const RECOVER_VARIANT_LIST: TokenSet = RECOVER_ITEM;
const RECOVER_FIELD_LIST: TokenSet = RECOVER_ITEM;
const RECOVER_IMPORT_SYMBOL_LIST: TokenSet = RECOVER_ITEM.combine(TokenSet::new(&[T![;]]));

fn proc_item(p: &mut Parser) {
    let m = p.start();
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
        block(p);
    } else if !p.eat(T![;]) {
        p.error_recover("expected block or `;`", RECOVER_ITEM);
    }
    m.complete(p, SyntaxKind::PROC_ITEM);
}

fn param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
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
    p.bump(T![ident]);
    p.expect(T![:]);
    ty(p);
    m.complete(p, SyntaxKind::PARAM);
}

fn enum_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![enum]);
    name(p);
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
    p.bump(T![ident]);
    if p.eat(T![=]) {
        expr(p);
    }
    m.complete(p, SyntaxKind::VARIANT);
}

fn struct_item(p: &mut Parser) {
    let m = p.start();
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
        if p.at(T![ident]) {
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
    p.bump(T![ident]);
    p.expect(T![:]);
    ty(p);
    m.complete(p, SyntaxKind::FIELD);
}

fn const_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![const]);
    name(p);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    expr(p);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::CONST_ITEM);
}

fn global_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![global]);
    name(p);
    p.eat(T![mut]);
    p.expect(T![:]);
    ty(p);
    p.expect(T![=]);
    expr(p);
    p.expect(T![;]);
    m.complete(p, SyntaxKind::GLOBAL_ITEM);
}

fn import_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![import]);
    name(p);
    if p.eat(T![/]) {
        name(p);
    }
    if p.eat(T![as]) {
        name(p);
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
    if p.eat(T![as]) {
        name(p);
    }
    m.complete(p, SyntaxKind::IMPORT_SYMBOL);
}

fn name(p: &mut Parser) {
    p.expect(T![ident]);
}

fn name_ref(p: &mut Parser) {
    p.expect(T![ident]);
}

fn path_type(p: &mut Parser) {
    let m = p.start();
    p.bump(T![ident]);
    while p.eat(T![.]) {
        name(p);
    }
    m.complete(p, SyntaxKind::PATH);
}

fn path_expr(p: &mut Parser) {
    let m = p.start();
    p.bump(T![ident]);
    while p.eat(T![.]) {
        if p.at(T!['{']) {
            break;
        }
        name(p);
    }
    m.complete(p, SyntaxKind::PATH);
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
    T![f16],
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
            p.eat(T![mut]);
            ty(p);
            m.complete(p, SyntaxKind::TYPE_REFERENCE);
        }
        T![proc] => type_proc(p),
        T!['['] => type_slice_or_array(p),
        _ => {
            //@no bump no recovery?
            p.error("expected type");
        }
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
                block(p);
            } else {
                block_short(p);
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
                }
                m.complete(p, SyntaxKind::STMT_EXPR_SEMI);
            }
        }
    }
}

fn loop_(p: &mut Parser) {
    let m = p.start();
    match p.peek() {
        T!['{'] => {}
        T![let] | T![mut] => {
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
        }
        _ => expr(p),
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
            path_expr(p);
            if p.at(T!['{']) {
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
                //@todo
                //@index or slice SyntaxKind
                mc_curr = m.complete(p, SyntaxKind::EXPR_INDEX);
            }
            T!['('] => {
                if last_cast {
                    break;
                }
                let m = p.start_before(mc_curr);
                p.bump(T!['(']);
                //@todo
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
    p.bump(T![if]);
    expr(p);
    block_expect(p);
    while p.eat(T![else]) {
        if p.eat(T![if]) {
            expr(p);
            block_expect(p);
        } else {
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
    m.complete(p, SyntaxKind::EXPR_BLOCK)
}

fn block_expect(p: &mut Parser) {
    if p.at(T!['{']) {
        block(p);
    } else {
        p.error("expected block");
    }
}

fn block_short(p: &mut Parser) {
    let m = p.start();
    stmt(p);
    m.complete(p, SyntaxKind::EXPR_BLOCK);
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
        m.complete(p, SyntaxKind::MATCH_ARM_FALLBACK);
        true
    } else {
        expr(p);
        p.expect(T![->]);
        expr(p);
        m.complete(p, SyntaxKind::MATCH_ARM);
        false
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
    m.complete(p, SyntaxKind::STRUCT_FIELD_INIT_LIST);
}

fn field_init(p: &mut Parser) {
    let m = p.start();
    if p.at_next(T![:]) {
        p.bump(T![ident]);
        p.bump(T![:]);
    }
    expr(p);
    m.complete(p, SyntaxKind::STRUCT_FIELD_INIT);
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

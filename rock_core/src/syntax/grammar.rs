use super::parser::{Event, Parser};
use super::syntax_tree::{SyntaxKind, SyntaxTree};
use super::token_set::TokenSet;
use crate::error::ErrorComp;
use crate::lexer;
use crate::session::FileID;
use crate::token::{Token, T};

pub fn parse(source: &str, file_id: FileID) -> Result<SyntaxTree, Vec<ErrorComp>> {
    let tokens = lexer::lex(source, file_id, false)?;
    let mut parser = Parser::new(tokens);
    source_file(&mut parser);
    pretty_print_events(&parser.finish());
    Err(vec![]) //@todo create tree
}

#[test]
fn parse_test() {
    let source = r#"
    
    proc something some some
    proc something2 ->
    proc
    proc something3 ( -> ;
    enum TileKind
    
    "#;
    let _ = parse(source, FileID::dummy());
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
            Event::StartNode { kind } => {
                println!("{:?}", kind);
                depth += 1;
            }
            Event::EndNode => {
                depth -= 1;
            }
            Event::Token { token } => println!("{}", token.as_str()),
            Event::Error { message } => {
                println!("error event: {}", message)
            }
        }
    }
}

fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(T![eof]) {
        match p.peek() {
            T![proc] => proc_item(p),
            T![enum] => enum_item(p),
            T![struct] => struct_item(p),
            T![const] => const_item(p),
            T![global] => const_item(p),
            T![import] => import_item(p),
            _ => p.error_bump("expected item"),
        }
    }
    m.complete(p, SyntaxKind::SOURCE_FILE);
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
        //@return type
    }
    if p.at(T!['{']) {
        //@block
    } else if !p.eat(T![;]) {
        p.error_recover("expected `{` or `;`", RECOVER_ITEM);
    }
    m.complete(p, SyntaxKind::PROC_ITEM);
}

fn param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(T!['(']);
    while !p.at(T![')']) && !p.at(T![eof]) {
        if p.at(T![ident]) {
            param(p);
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
    //@type
    if !p.at(T![')']) {
        p.expect(T![,]);
    }
    m.complete(p, SyntaxKind::PARAM);
}

fn enum_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![enum]);
    name(p);
    //@basic type
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
    //@ (= expr)?
    if !p.at(T!['}']) {
        p.expect(T![,]);
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
    //@type
    if !p.at(T!['}']) {
        p.expect(T![,]);
    }
    m.complete(p, SyntaxKind::FIELD);
}

fn const_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![const]);
    name(p);
    p.expect(T![:]);
    //@type
    p.expect(T![=]);
    //@expr
    p.expect(T![;]);
    m.complete(p, SyntaxKind::CONST_ITEM);
}

fn global_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![global]);
    name(p);
    p.eat(T![mut]);
    p.expect(T![:]);
    //@type
    p.expect(T![=]);
    //@expr
    p.expect(T![;]);
    m.complete(p, SyntaxKind::GLOBAL_ITEM);
}

fn import_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![import]);
    m.complete(p, SyntaxKind::IMPORT_ITEM);
}

fn import_symbol_list(p: &mut Parser) {}

fn import_symbol(p: &mut Parser) {}

fn visibility(p: &mut Parser) {}

fn name(p: &mut Parser) {
    p.expect(T![ident]);
}

fn name_ref(p: &mut Parser) {
    p.expect(T![ident]);
}

fn attribute(p: &mut Parser) {}

fn path(p: &mut Parser) {}

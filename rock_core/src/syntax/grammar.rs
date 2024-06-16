use super::parser::Parser;
use super::syntax_tree::{SyntaxKind, SyntaxTree};
use crate::error::ErrorComp;
use crate::lexer;
use crate::session::FileID;
use crate::token::{Token, T};

pub fn parse(source: &str, file_id: FileID) -> Result<SyntaxTree, Vec<ErrorComp>> {
    let tokens = lexer::lex(source, file_id, false)?;
    let mut parser = Parser::new(tokens);
    source_file(&mut parser);
    Err(vec![]) //@todo create tree
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

fn proc_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![proc]);
    m.complete(p, SyntaxKind::PROC_ITEM);
}

fn param_list(p: &mut Parser) {}

fn param(p: &mut Parser) {}

fn enum_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![enum]);
    m.complete(p, SyntaxKind::ENUM_ITEM);
}

fn variant_list(p: &mut Parser) {}

fn variant(p: &mut Parser) {}

fn struct_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![struct]);
    m.complete(p, SyntaxKind::STRUCT_ITEM);
}

fn field_list(p: &mut Parser) {}

fn field(p: &mut Parser) {}

fn const_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![const]);
    m.complete(p, SyntaxKind::CONST_ITEM);
}

fn global_item(p: &mut Parser) {
    let m = p.start();
    p.bump(T![global]);
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

fn name(p: &mut Parser) {}

fn name_ref(p: &mut Parser) {}

fn attribute(p: &mut Parser) {}

fn path(p: &mut Parser) {}

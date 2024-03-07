use super::parser_api::Parser;
use super::syntax_tree::SyntaxNodeKind;
use super::token_set::TokenSet;
use crate::ast::token::Token;

const DECL_RECOVERY_SET: TokenSet = TokenSet::new(&[Token::KwMod, Token::KwUse, Token::KwProc]);

pub fn source_file(p: &mut Parser) {
    let m = p.start();
    while !p.at(Token::Eof) {
        println!("decl loop");
        decl(p);
    }
    m.complete(p, SyntaxNodeKind::SOURCE_FILE);
}

fn decl(p: &mut Parser) {
    match p.peek() {
        Token::KwMod => mod_decl(p),
        Token::KwUse => use_decl(p),
        Token::KwProc => proc_decl(p),
        _ => p.error_bump("expected declaration"),
    }
}

// recovery / follow sets can be used to formalize
// this preffed behavior
// maybe eat tokens that are not in recovery / follow sets
// instead of emitting missing `token` after 'current'
fn mod_decl(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::KwMod);
    // this only triggers a single error
    // requesting the name to be present after current token
    if !p.expect(Token::Ident) {
        // `;` is follow set of name, eat it even when name was missing
        p.eat(Token::Semicolon);
        m.complete(p, SyntaxNodeKind::MOD_DECL);
        return;
    }
    // the semi will only be requested when name was present
    p.expect(Token::Semicolon);
    m.complete(p, SyntaxNodeKind::MOD_DECL);
}

fn use_decl(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::KwUse);
    m.complete(p, SyntaxNodeKind::USE_DECL);
}

fn proc_decl(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::KwProc);
    p.expect(Token::Ident);
    // required but not reported as missing yet
    if p.at(Token::OpenParen) {
        proc_param_list(p);
    }
    // required but not reported as missing yet
    if p.eat(Token::ArrowThin) {
        // consider first set of type? same as with other parts
        ty(p);
    }
    if p.at(Token::OpenBlock) {
        block(p);
    }
    m.complete(p, SyntaxNodeKind::PROC_DECL);
}

fn proc_param_list(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::OpenParen);
    while !p.at(Token::CloseParen) && !p.at(Token::Eof) {
        if p.at(Token::Ident) {
            proc_param(p);
        } else {
            break;
        }
    }
    p.expect(Token::CloseParen);
    m.complete(p, SyntaxNodeKind::PROC_PARAM_LIST);
}

fn proc_param(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::Ident);
    p.expect(Token::Colon);
    // consider first set of type? same as with other parts
    ty(p);
    if !p.at(Token::CloseParen) {
        p.expect(Token::Comma);
    }
    m.complete(p, SyntaxNodeKind::PROC_PARAM);
}

fn ty(p: &mut Parser) {
    let m = p.start();
    p.expect(Token::Ident); // name
    m.complete(p, SyntaxNodeKind::TYPE);
}

fn block(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::OpenBlock);
    while !p.at(Token::CloseBlock) && !p.at(Token::Eof) {
        stmt(p);
    }
    p.expect(Token::CloseBlock);
    m.complete(p, SyntaxNodeKind::EXPR_BLOCK);
}

fn stmt(p: &mut Parser) {
    match p.peek() {
        Token::KwReturn => stmt_return(p),
        _ => stmt_expr(p),
    }
}

fn stmt_return(p: &mut Parser) {
    let m = p.start();
    p.bump(Token::KwReturn);
    expr(p);
    p.expect(Token::Semicolon);
    m.complete(p, SyntaxNodeKind::STMT_RETURN);
}

fn stmt_expr(p: &mut Parser) {
    let m = p.start();
    expr(p);
    p.expect(Token::Semicolon);
    m.complete(p, SyntaxNodeKind::STMT_EXPR);
}

fn expr(p: &mut Parser) {
    match p.peek() {
        Token::IntLit => {
            let m = p.start();
            p.bump(Token::IntLit);
            m.complete(p, SyntaxNodeKind::EXPR_LIT);
        }
        _ => p.error_bump("expected integer expression"),
    }
}

fn name(p: &mut Parser, recovery: TokenSet) {
    if p.at(Token::Ident) {
        let m = p.start();
        p.bump(Token::Ident);
        m.complete(p, SyntaxNodeKind::NAME);
    } else {
        p.error_recover("expected identifier", recovery);
    }
}

fn name_ref(p: &mut Parser) {
    let m = p.start();
    if p.at(Token::Ident) {
        let m = p.start();
        p.bump(Token::Ident);
        m.complete(p, SyntaxNodeKind::NAME_REF);
    } else {
        p.error_bump("expected identifier");
    }
}

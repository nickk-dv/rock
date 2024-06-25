mod grammar;
mod lexer;

use crate::error::ErrorComp;
use crate::session::ModuleID;
use crate::token::token_list::TokenList;

pub fn lex(source: &str, module_id: ModuleID, lex_whitespace: bool) -> (TokenList, Vec<ErrorComp>) {
    let mut lex = lexer::Lexer::new(source, module_id, lex_whitespace);
    grammar::source_file(&mut lex);
    lex.finish()
}

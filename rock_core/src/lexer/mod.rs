mod grammar;
mod lexer;

use crate::error::ErrorComp;
use crate::session::ModuleID;
use crate::token::TokenList;

pub fn lex(source: &str, module_id: ModuleID, with_trivia: bool) -> (TokenList, Vec<ErrorComp>) {
    let mut lex = lexer::Lexer::new(source, module_id, with_trivia);
    grammar::source_file(&mut lex);
    lex.finish()
}

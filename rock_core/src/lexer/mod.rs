mod grammar;
mod lexer;

use crate::error::ErrorComp;
use crate::session::FileID;
use crate::token::token_list::TokenList;

pub fn lex(
    source: &str,
    file_id: FileID,
    lex_whitespace: bool,
) -> Result<TokenList, Vec<ErrorComp>> {
    let mut lex = lexer::Lexer::new(source, file_id, lex_whitespace);
    grammar::source_file(&mut lex);
    lex.finish()
}

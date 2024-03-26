mod parser;

use crate::arena::Arena;
use crate::ast;
use crate::error::ErrorComp;
use crate::lexer;
use crate::session::Session;

pub fn parse<'ast>(session: &mut Session) -> Result<ast::Ast<'ast>, Vec<ErrorComp>> {
    let mut errors = Vec::new();
    let mut ast = ast::Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };

    for file_id in session.file_ids() {
        //@fix source.clone()
        //@due to borrowing of intern_pool & source text at the same time
        //somehow create the intern pool while we parse?
        //problem is inserting it back to the Session
        //maybe it doesnt belong in a Session? and instead should be passed from Ast -> Hir?
        let source = session.file(file_id).source.clone();
        let lexer = lexer::Lexer::new(&source, false);
        let tokens = lexer.lex();
        let mut parser = parser::Parser::new(tokens, &mut ast.arena, session.intern_mut(), &source);

        match parser::module(&mut parser, file_id) {
            Ok(module) => {
                ast.modules.push(module);
            }
            Err(error) => {
                errors.push(error);
            }
        }
    }

    if errors.is_empty() {
        Ok(ast)
    } else {
        Err(errors)
    }
}

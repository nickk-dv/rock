mod grammar;
mod parser;

use crate::ast::*;
use crate::error::ResultComp;
use crate::intern::{InternName, InternPool};
use crate::lexer;
use crate::session::Session;
use crate::support::Timer;

pub fn parse<'ast>(
    session: &Session,
    intern_name: InternPool<'ast, InternName>,
) -> ResultComp<Ast<'ast>> {
    let t_total = Timer::new();
    let mut state = parser::ParseState::new(intern_name);

    for module_id in session.module_ids() {
        let module = session.module(module_id);

        let (tokens, errors) = lexer::lex(&module.source, module_id, false);
        if !errors.is_empty() {
            state.errors.extend(errors);
            continue;
        }
        let parser = parser::Parser::new(tokens, module_id, &module.source, &mut state);

        match grammar::module(parser, module_id) {
            Ok(module) => state.modules.push(module),
            Err(error) => state.errors.push(error),
        }
    }

    t_total.stop("ast parse (old) total");
    state.result()
}

mod grammar;
mod parser;

use crate::ast::*;
use crate::error::ResultComp;
use crate::lexer;
use crate::session::{FileID, Session};
use crate::timer::Timer;

pub fn parse(session: &Session) -> ResultComp<Ast> {
    let t_total = Timer::new();
    let mut state = parser::ParseState::new();
    let mut file_idx: usize = 0;

    for package_id in session.package_ids() {
        let package = session.package(package_id);
        let package_name_id = state.intern_name.intern(&package.manifest().package.name);
        let mut modules = Vec::<Module>::new();

        for idx in file_idx..file_idx + package.file_count() {
            let file_id = FileID::new(idx);
            let file = session.file(file_id);
            let filename = file
                .path
                .file_stem()
                .expect("filename")
                .to_str()
                .expect("utf-8");
            let module_name_id = state.intern_name.intern(filename);

            let (tokens, errors) = lexer::lex(&file.source, file_id, false);
            if !errors.is_empty() {
                state.errors.extend(errors);
                continue;
            }
            let parser = parser::Parser::new(tokens, file_id, &file.source, &mut state);

            match grammar::module(parser, file_id, module_name_id) {
                Ok(module) => modules.push(module),
                Err(error) => state.errors.push(error),
            }
        }

        file_idx += package.file_count();
        state.packages.push(Package {
            name_id: package_name_id,
            modules,
        })
    }

    t_total.stop("ast parse (old) total");
    state.result()
}

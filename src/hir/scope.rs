use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::error::*;
use crate::mem::P;

pub struct Scope {
    pub id: ScopeID,
    pub module: P<Module>,
    pub parent: Option<ScopeID>,
    pub declared: SymbolTable,
    pub imported: SymbolTable,
    pub wildcards: Vec<Wildcard>,
    pub errors: Vec<Error>,
}

#[derive(Copy, Clone)]
pub struct Wildcard {
    pub from_id: ScopeID,
    pub import_span: Span,
}

impl Scope {
    pub fn new(module: P<Module>) -> Self {
        Self {
            id: 0, //@todo
            module,
            parent: None, //@todo
            declared: SymbolTable::new(),
            imported: SymbolTable::new(),
            wildcards: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn md(&self) -> P<Module> {
        self.module.copy()
    }

    pub fn err(&mut self, error: CheckError, span: Span) {
        self.errors
            .push(Error::check(error, self.md(), span).into());
    }

    pub fn error(&mut self, error: Error) {
        self.errors.push(error);
    }
}

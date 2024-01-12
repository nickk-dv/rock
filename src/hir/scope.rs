use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::error::*;
use crate::mem::{InternID, P};
use std::collections::HashMap;

pub struct Scope {
    pub id: ScopeID,
    pub module: P<Module>,
    pub parent: Option<ScopeID>,
    pub errors: Vec<Error>,
    pub declared: SymbolTable,
    pub glob_imports: Vec<GlobImport>,
    pub symbol_imports: HashMap<InternID, SymbolImport>,
}

pub struct GlobImport {
    pub from_id: ScopeID,
    pub import_span: Span,
}

pub struct SymbolImport {
    pub from_id: ScopeID,
    pub name: Ident,
}

impl Scope {
    pub fn new(id: ScopeID, module: P<Module>, parent: Option<ScopeID>) -> Self {
        Self {
            id,
            module,
            parent,
            errors: Vec::new(),
            declared: SymbolTable::new(),
            glob_imports: Vec::new(),
            symbol_imports: HashMap::new(),
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

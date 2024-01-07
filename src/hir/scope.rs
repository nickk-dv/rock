use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::error::{CheckError, Error};
use crate::mem::P;

pub struct Scope {
    pub module: P<Module>,
    pub declared: SymbolTable,
    pub imported: SymbolTable,
    pub wildcards: Vec<Wildcard>,
    pub errors: Vec<Error>,
}

#[derive(Copy, Clone)]
pub struct Wildcard {
    pub from_id: ModuleID,
    pub import_span: Span,
}

impl Scope {
    pub fn new(module: P<Module>) -> Self {
        Self {
            module,
            declared: SymbolTable::new(),
            imported: SymbolTable::new(),
            wildcards: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn id(&self) -> ModuleID {
        self.module.id
    }

    pub fn err(&mut self, error: CheckError, span: Span) {}
    pub fn err_info(&mut self, span: Span, marker: &'static str) {}
    pub fn err_info_external(&mut self, span: Span, source: ModuleID, marker: &'static str) {}
}

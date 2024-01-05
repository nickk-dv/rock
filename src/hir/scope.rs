use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::check_err::*;
use crate::mem::P;

pub struct Scope {
    pub module: P<Module>,
    pub errors: Vec<Error>,
    pub declared: SymbolTable,
    pub imported: SymbolTable,
    pub wildcards: Vec<WildcardImport>,
}

#[derive(Copy, Clone)]
pub struct WildcardImport {
    pub from_id: ModuleID,
    pub import_span: Span,
}

impl Scope {
    pub fn new(module: P<Module>) -> Self {
        Self {
            module,
            errors: Vec::new(),
            declared: SymbolTable::new(),
            imported: SymbolTable::new(),
            wildcards: Vec::new(),
        }
    }

    pub fn id(&self) -> ModuleID {
        self.module.id
    }

    pub fn err(&mut self, error: CheckError, span: Span) {
        self.errors.push(Error::new(error, self.id(), span));
    }

    pub fn err_info(&mut self, span: Span, marker: &'static str) {
        let info = ErrorInfo {
            source: self.id(),
            span,
            marker,
        };
        unsafe {
            self.errors.last_mut().unwrap_unchecked().info.push(info);
        }
    }

    pub fn err_info_external(&mut self, span: Span, source: ModuleID, marker: &'static str) {
        let info = ErrorInfo {
            source,
            span,
            marker,
        };
        unsafe {
            self.errors.last_mut().unwrap_unchecked().info.push(info);
        }
    }
}

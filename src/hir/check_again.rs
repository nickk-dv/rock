use crate::ast::{ast::*, span::Span};
use crate::err::check_err::CheckError;
use crate::mem::*;
use std::collections::HashMap;

//@note external proc uniqueness check will be delayed to after hir creation

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    let mut context = Context::new(ast);
    context.pass_0_create_scopes();
    context.pass_1_add_declared_symbols();
    context.pass_2_check_main_proc();
    context.pass_3_check_decl_namesets();
    context.pass_4_import_symbols();
    context.report_errors()
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    errors: Vec<Error>,
    scopes: Vec<Scope>,
}

struct Scope {
    module: P<Module>,
    errors: Vec<Error>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
}

struct Error {
    error: CheckError,
    no_context: bool,
    source: SourceID,
    span: Span,
}

impl<'ast> Context<'ast> {
    fn new(ast: &'ast mut Ast) -> Self {
        Self {
            ast,
            errors: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn err(&mut self, error: CheckError) {
        self.errors.push(Error::new_no_context(error));
    }

    fn report_errors(&self) -> Result<(), ()> {
        for err in self.errors.iter() {
            crate::err::report::err(self.ast, err.error, err.no_context, err.source, err.span);
        }
        for scope in self.scopes.iter() {
            for err in scope.errors.iter() {
                crate::err::report::err(self.ast, err.error, err.no_context, err.source, err.span);
            }
        }
        if crate::err::report::did_error() {
            println!("");
            Err(())
        } else {
            Ok(())
        }
    }

    fn get_scope(&mut self, scope_id: SourceID) -> &mut Scope {
        unsafe { self.scopes.get_unchecked_mut(scope_id as usize) }
    }

    fn pass_0_create_scopes(&mut self) {
        self.create_scopes(self.ast.package.root);
    }

    fn create_scopes(&mut self, module: P<Module>) {
        self.scopes.push(Scope::new(module));
        for submodule in module.submodules.iter() {
            self.create_scopes(submodule);
        }
    }

    fn pass_1_add_declared_symbols(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            let scope = self.get_scope(scope_id);
            for decl in scope.module.decls.iter() {
                if let Some(name) = decl.get_name() {
                    scope.try_add_declared_symbol(name, decl);
                }
            }
        }
    }

    //@ lib / exe package type is not considered, main is always required
    // root has id = 0 currently, id scheme might change with
    // dependencies in ast or module tree repr. changes
    fn pass_2_check_main_proc(&mut self) {
        if let Some(main_id) = self.ast.intern_pool.get_id_if_exists("main".as_bytes()) {
            let root_scope = self.get_scope(0);
            if let Some(main_proc) = root_scope.find_declared_proc(main_id) {
                root_scope.check_main_proc(main_proc);
            } else {
                self.err(CheckError::MainProcMissing);
            }
        } else {
            self.err(CheckError::MainProcMissing);
        }
    }

    fn pass_3_check_decl_namesets(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            let scope = self.get_scope(scope_id);
            for decl in scope.module.decls.iter() {
                scope.check_decl_nameset(decl);
            }
        }
    }

    fn pass_4_import_symbols(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            self.scope_import_symbols(scope_id);
        }
    }

    fn scope_import_symbols(&mut self, scope_id: SourceID) {
        let scope = self.get_scope(scope_id);
        let mut imports = Vec::new();

        for decl in scope.module.decls.iter() {
            if let Decl::Import(import) = decl {
                if import.module_access.names.is_empty()
                    && import.module_access.modifier == ModuleAccessModifier::None
                {
                    scope.err(CheckError::ImportModuleAccessMissing, import.span);
                } else {
                    imports.push(import);
                }
            }
        }

        for import in imports.iter() {
            if import.module_access.modifier == ModuleAccessModifier::Super
                && scope.module.parent.is_none()
            {
                scope.err(CheckError::SuperUsedFromRootModule, import.span);
                continue;
            }
        }
    }
}

impl Scope {
    fn new(module: P<Module>) -> Self {
        Self {
            module,
            errors: Vec::new(),
            declared_symbols: HashMap::new(),
            imported_symbols: HashMap::new(),
        }
    }

    fn err(&mut self, error: CheckError, span: Span) {
        self.errors
            .push(Error::new(error, self.module.source, span));
    }

    fn find_declared_proc(&self, id: InternID) -> Option<P<ProcDecl>> {
        if let Some(symbol) = self.declared_symbols.get(&id) {
            if let Decl::Proc(proc_decl) = symbol {
                return Some(*proc_decl);
            }
        }
        None
    }

    fn try_add_declared_symbol(&mut self, name: Ident, decl: Decl) {
        if let Some(existing) = self.declared_symbols.get(&name.id) {
            self.err(CheckError::SymbolRedefinition, name.span);
        } else {
            self.declared_symbols.insert(name.id, decl);
        }
    }

    fn check_main_proc(&mut self, main_proc: P<ProcDecl>) {
        if main_proc.is_variadic {
            self.err(CheckError::MainProcVariadic, main_proc.name.span);
        }
        if main_proc.block.is_none() {
            self.err(CheckError::MainProcExternal, main_proc.name.span);
        }
        if !main_proc.params.is_empty() {
            self.err(CheckError::MainProcHasParams, main_proc.name.span);
        }
        let mut ret_type_valid = false;
        if let Some(tt) = main_proc.return_type {
            if tt.pointer_level == 0 {
                if let TypeKind::Basic(BasicType::S32) = tt.kind {
                    ret_type_valid = true;
                }
            }
        }
        if !ret_type_valid {
            self.err(CheckError::MainProcWrongRetType, main_proc.name.span);
        }
    }

    //@duplicates need to be removed from the lists
    // to not affect later stages, list removal is not yet implemented
    fn check_decl_nameset(&mut self, decl: Decl) {
        match decl {
            Decl::Proc(proc_decl) => {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for param in proc_decl.params.iter() {
                    if let Some(existing) = name_set.get(&param.name.id) {
                        self.err(CheckError::ProcParamRedefinition, param.name.span);
                    } else {
                        name_set.insert(param.name.id, param.name);
                    }
                }
            }
            Decl::Enum(enum_decl) => {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for variant in enum_decl.variants.iter() {
                    if let Some(existing) = name_set.get(&variant.name.id) {
                        self.err(CheckError::EnumVariantRedefinition, variant.name.span);
                    } else {
                        name_set.insert(variant.name.id, variant.name);
                    }
                }
            }
            Decl::Struct(struct_decl) => {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for field in struct_decl.fields.iter() {
                    if let Some(existing) = name_set.get(&field.name.id) {
                        self.err(CheckError::StructFieldRedefinition, field.name.span);
                    } else {
                        name_set.insert(field.name.id, field.name);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Error {
    pub fn new(error: CheckError, source: SourceID, span: Span) -> Self {
        Self {
            error,
            no_context: false,
            source,
            span,
        }
    }

    pub fn new_no_context(error: CheckError) -> Self {
        Self {
            error,
            no_context: true,
            source: 0,
            span: Span::new(1, 1),
        }
    }
}

impl Decl {
    fn get_name(&self) -> Option<Ident> {
        match self {
            Decl::Mod(mod_decl) => Some(mod_decl.name),
            Decl::Proc(proc_decl) => Some(proc_decl.name),
            Decl::Enum(enum_decl) => Some(enum_decl.name),
            Decl::Struct(struct_decl) => Some(struct_decl.name),
            Decl::Global(global_decl) => Some(global_decl.name),
            _ => None,
        }
    }
}

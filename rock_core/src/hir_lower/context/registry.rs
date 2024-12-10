use crate::ast;
use crate::hir;
use crate::session::{ModuleID, Session};

pub struct Registry<'hir, 'ast> {
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    ast_imports: Vec<&'ast ast::ImportItem<'ast>>,
    pub(super) hir_procs: Vec<hir::ProcData<'hir>>,
    pub(super) hir_enums: Vec<hir::EnumData<'hir>>,
    pub(super) hir_structs: Vec<hir::StructData<'hir>>,
    pub(super) hir_consts: Vec<hir::ConstData<'hir>>,
    pub(super) hir_globals: Vec<hir::GlobalData<'hir>>,
    pub(super) hir_imports: Vec<hir::ImportData>,
    pub(super) const_evals: Vec<(hir::ConstEval<'hir, 'ast>, ModuleID)>,
    pub(super) variant_evals: Vec<hir::VariantEval<'hir>>,
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    pub(super) fn new(session: &Session) -> Registry<'hir, 'ast> {
        let mut proc_count = 0;
        let mut enum_count = 0;
        let mut struct_count = 0;
        let mut const_count = 0;
        let mut global_count = 0;
        let mut import_count = 0;
        let mut const_eval_count = 0;
        let mut variant_eval_count = 0;

        for module_id in session.module.ids() {
            let module = session.module.get(module_id);
            let ast = module.ast_expect();

            for item in ast.items {
                match item {
                    ast::Item::Proc(_) => proc_count += 1,
                    ast::Item::Enum(enum_item) => {
                        enum_count += 1;
                        for variant in enum_item.variants {
                            match variant.kind {
                                ast::VariantKind::Default => variant_eval_count += 1,
                                ast::VariantKind::Constant(_) => const_eval_count += 1,
                                ast::VariantKind::HasFields(_) => variant_eval_count += 1,
                            }
                        }
                    }
                    ast::Item::Struct(_) => struct_count += 1,
                    ast::Item::Const(_) => const_count += 1,
                    ast::Item::Global(_) => global_count += 1,
                    ast::Item::Import(_) => import_count += 1,
                    ast::Item::Directive(_) => {}
                }
            }
        }

        Registry {
            ast_procs: Vec::with_capacity(proc_count),
            ast_enums: Vec::with_capacity(enum_count),
            ast_structs: Vec::with_capacity(struct_count),
            ast_consts: Vec::with_capacity(const_count),
            ast_globals: Vec::with_capacity(global_count),
            ast_imports: Vec::with_capacity(import_count),
            hir_procs: Vec::with_capacity(proc_count),
            hir_enums: Vec::with_capacity(enum_count),
            hir_structs: Vec::with_capacity(struct_count),
            hir_consts: Vec::with_capacity(const_count),
            hir_globals: Vec::with_capacity(global_count),
            hir_imports: Vec::with_capacity(import_count),
            const_evals: Vec::with_capacity(const_eval_count),
            variant_evals: Vec::with_capacity(variant_eval_count),
        }
    }

    pub fn add_proc(
        &mut self,
        item: &'ast ast::ProcItem<'ast>,
        data: hir::ProcData<'hir>,
    ) -> hir::ProcID {
        let id = hir::ProcID::new(self.hir_procs.len());
        self.ast_procs.push(item);
        self.hir_procs.push(data);
        id
    }

    pub fn add_enum(
        &mut self,
        item: &'ast ast::EnumItem<'ast>,
        data: hir::EnumData<'hir>,
    ) -> hir::EnumID {
        let id = hir::EnumID::new(self.hir_enums.len());
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }

    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) -> hir::StructID {
        let id = hir::StructID::new(self.hir_structs.len());
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }

    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) -> hir::ConstID {
        let id = hir::ConstID::new(self.hir_consts.len());
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }

    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> hir::GlobalID {
        let id = hir::GlobalID::new(self.hir_globals.len());
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
    }

    pub fn add_import(
        &mut self,
        item: &'ast ast::ImportItem<'ast>,
        data: hir::ImportData,
    ) -> hir::ImportID {
        let id = hir::ImportID::new(self.hir_imports.len());
        self.ast_imports.push(item);
        self.hir_imports.push(data);
        id
    }

    pub fn add_const_eval(
        &mut self,
        const_expr: ast::ConstExpr<'ast>,
        origin_id: ModuleID,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new(self.const_evals.len());
        let eval = hir::ConstEval::Unresolved(const_expr);
        self.const_evals.push((eval, origin_id));
        id
    }

    pub fn add_variant_eval(&mut self) -> hir::VariantEvalID {
        let id = hir::VariantEvalID::new(self.variant_evals.len());
        let eval = hir::VariantEval::Unresolved(());
        self.variant_evals.push(eval);
        id
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.hir_procs.len()).map(hir::ProcID::new)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.hir_enums.len()).map(hir::EnumID::new)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.hir_structs.len()).map(hir::StructID::new)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.hir_consts.len()).map(hir::ConstID::new)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.hir_globals.len()).map(hir::GlobalID::new)
    }
    pub fn import_ids(&self) -> impl Iterator<Item = hir::ImportID> {
        (0..self.hir_imports.len()).map(hir::ImportID::new)
    }
    pub fn const_eval_ids(&self) -> impl Iterator<Item = hir::ConstEvalID> {
        (0..self.const_evals.len()).map(hir::ConstEvalID::new)
    }

    pub fn proc_item(&self, id: hir::ProcID) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.index()]
    }
    pub fn enum_item(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.index()]
    }
    pub fn struct_item(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.index()]
    }
    pub fn const_item(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.index()]
    }
    pub fn global_item(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.index()]
    }
    pub fn import_item(&self, id: hir::ImportID) -> &'ast ast::ImportItem<'ast> {
        self.ast_imports[id.index()]
    }

    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        &self.hir_procs[id.index()]
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        &self.hir_enums[id.index()]
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        &self.hir_structs[id.index()]
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        &self.hir_consts[id.index()]
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        &self.hir_globals[id.index()]
    }
    pub fn import_data(&self, id: hir::ImportID) -> &hir::ImportData {
        &self.hir_imports[id.index()]
    }
    pub fn const_eval(&self, id: hir::ConstEvalID) -> &(hir::ConstEval<'hir, 'ast>, ModuleID) {
        &self.const_evals[id.index()]
    }
    pub fn variant_eval(&self, id: hir::VariantEvalID) -> &hir::VariantEval<'hir> {
        &self.variant_evals[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        &mut self.hir_procs[id.index()]
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        &mut self.hir_enums[id.index()]
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        &mut self.hir_structs[id.index()]
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        &mut self.hir_consts[id.index()]
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        &mut self.hir_globals[id.index()]
    }
    pub fn const_eval_mut(
        &mut self,
        id: hir::ConstEvalID,
    ) -> &mut (hir::ConstEval<'hir, 'ast>, ModuleID) {
        &mut self.const_evals[id.index()]
    }
    pub fn variant_eval_mut(&mut self, id: hir::VariantEvalID) -> &mut hir::VariantEval<'hir> {
        &mut self.variant_evals[id.index()]
    }
}

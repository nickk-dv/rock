use crate::ast;
use crate::hir;
use crate::session::{ModuleID, Session};
use crate::support::IndexID;

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
    ) -> hir::ProcID<'hir> {
        let id = hir::ProcID::new(&self.hir_procs);
        self.ast_procs.push(item);
        self.hir_procs.push(data);
        id
    }

    pub fn add_enum(
        &mut self,
        item: &'ast ast::EnumItem<'ast>,
        data: hir::EnumData<'hir>,
    ) -> hir::EnumID<'hir> {
        let id = hir::EnumID::new(&self.hir_enums);
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }

    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) -> hir::StructID<'hir> {
        let id = hir::StructID::new(&self.hir_structs);
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }

    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) -> hir::ConstID<'hir> {
        let id = hir::ConstID::new(&self.hir_consts);
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }

    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> hir::GlobalID<'hir> {
        let id = hir::GlobalID::new(&self.hir_globals);
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
    }

    pub fn add_import(
        &mut self,
        item: &'ast ast::ImportItem<'ast>,
        data: hir::ImportData,
    ) -> hir::ImportID {
        let id = hir::ImportID::new(&self.hir_imports);
        self.ast_imports.push(item);
        self.hir_imports.push(data);
        id
    }

    pub fn add_const_eval(
        &mut self,
        const_expr: ast::ConstExpr<'ast>,
        origin_id: ModuleID,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new_raw(self.const_evals.len());
        let eval = hir::ConstEval::Unresolved(const_expr);
        self.const_evals.push((eval, origin_id));
        id
    }

    pub fn add_variant_eval(&mut self) -> hir::VariantEvalID<'hir> {
        let id = hir::VariantEvalID::new(&self.variant_evals);
        let eval = hir::VariantEval::Unresolved(());
        self.variant_evals.push(eval);
        id
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID<'hir>> {
        (0..self.hir_procs.len()).map(hir::ProcID::new_raw)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID<'hir>> {
        (0..self.hir_enums.len()).map(hir::EnumID::new_raw)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID<'hir>> {
        (0..self.hir_structs.len()).map(hir::StructID::new_raw)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID<'hir>> {
        (0..self.hir_consts.len()).map(hir::ConstID::new_raw)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID<'hir>> {
        (0..self.hir_globals.len()).map(hir::GlobalID::new_raw)
    }
    pub fn import_ids(&self) -> impl Iterator<Item = hir::ImportID> {
        (0..self.hir_imports.len()).map(hir::ImportID::new_raw)
    }
    pub fn const_eval_ids(&self) -> impl Iterator<Item = hir::ConstEvalID> {
        (0..self.const_evals.len()).map(hir::ConstEvalID::new_raw)
    }

    pub fn proc_item(&self, id: hir::ProcID<'hir>) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.raw_index()]
    }
    pub fn enum_item(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.raw_index()]
    }
    pub fn struct_item(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.raw_index()]
    }
    pub fn const_item(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.raw_index()]
    }
    pub fn global_item(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.raw_index()]
    }
    pub fn import_item(&self, id: hir::ImportID) -> &'ast ast::ImportItem<'ast> {
        self.ast_imports[id.raw_index()]
    }

    pub fn proc_data(&self, id: hir::ProcID<'hir>) -> &hir::ProcData<'hir> {
        self.hir_procs.id_get(id)
    }
    pub fn enum_data(&self, id: hir::EnumID<'hir>) -> &hir::EnumData<'hir> {
        self.hir_enums.id_get(id)
    }
    pub fn struct_data(&self, id: hir::StructID<'hir>) -> &hir::StructData<'hir> {
        self.hir_structs.id_get(id)
    }
    pub fn const_data(&self, id: hir::ConstID<'hir>) -> &hir::ConstData<'hir> {
        self.hir_consts.id_get(id)
    }
    pub fn global_data(&self, id: hir::GlobalID<'hir>) -> &hir::GlobalData<'hir> {
        self.hir_globals.id_get(id)
    }
    pub fn import_data(&self, id: hir::ImportID) -> &hir::ImportData {
        self.hir_imports.id_get(id)
    }
    pub fn const_eval(&self, id: hir::ConstEvalID) -> &(hir::ConstEval<'hir, 'ast>, ModuleID) {
        &self.const_evals[id.raw_index()]
    }
    pub fn variant_eval(&self, id: hir::VariantEvalID<'hir>) -> &hir::VariantEval<'hir> {
        self.variant_evals.id_get(id)
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID<'hir>) -> &mut hir::ProcData<'hir> {
        self.hir_procs.id_get_mut(id)
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID<'hir>) -> &mut hir::EnumData<'hir> {
        self.hir_enums.id_get_mut(id)
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID<'hir>) -> &mut hir::StructData<'hir> {
        self.hir_structs.id_get_mut(id)
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID<'hir>) -> &mut hir::ConstData<'hir> {
        self.hir_consts.id_get_mut(id)
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID<'hir>) -> &mut hir::GlobalData<'hir> {
        self.hir_globals.id_get_mut(id)
    }
    pub fn const_eval_mut(
        &mut self,
        id: hir::ConstEvalID,
    ) -> &mut (hir::ConstEval<'hir, 'ast>, ModuleID) {
        &mut self.const_evals[id.raw_index()]
    }
    pub fn variant_eval_mut(
        &mut self,
        id: hir::VariantEvalID<'hir>,
    ) -> &mut hir::VariantEval<'hir> {
        self.variant_evals.id_get_mut(id)
    }
}

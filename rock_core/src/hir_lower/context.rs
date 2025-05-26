use super::check_match::PatCov;
use super::scope::{PolyScope, Scope};
use crate::ast;
use crate::error::{DiagnosticData, ErrorSink, ErrorWarningBuffer, SourceRange, WarningSink};
use crate::hir;
use crate::intern::NameID;
use crate::session::{ModuleID, Session};
use crate::support::{Arena, TempBuffer, TempOffset};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirCtx<'hir, 's, 'sref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub in_const: bool,
    pub scope: Scope<'hir>,
    pub registry: Registry<'hir, 's>,
    pub enum_tag_set: HashMap<i128, hir::VariantID>,
    pub session: &'sref mut Session<'s>,
    pub pat: PatCov,
    pub core: hir::CoreItems,
    pub cache: Cache<'hir>,
    pub entry_point: Option<hir::ProcID>,
    pub enum_layout: HashMap<hir::EnumKey<'hir>, hir::Layout>,
    pub struct_layout: HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>>,
    pub variant_layout: HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>>,
}

pub struct Cache<'hir> {
    pub proc_params: Vec<hir::Param<'hir>>,
    pub enum_variants: Vec<hir::Variant<'hir>>,
    pub struct_fields: Vec<hir::Field<'hir>>,
    pub poly_param_names: Vec<ast::Name>,
    pub u8s: TempBuffer<u8>,
    pub u64s: TempBuffer<u64>,
    pub types: TempBuffer<hir::Type<'hir>>,
    pub stmts: TempBuffer<hir::Stmt<'hir>>,
    pub exprs: TempBuffer<&'hir hir::Expr<'hir>>,
    pub branches: TempBuffer<hir::Branch<'hir>>,
    pub match_arms: TempBuffer<hir::MatchArm<'hir>>,
    pub patterns: TempBuffer<hir::Pat<'hir>>,
    pub var_ids: TempBuffer<hir::VariableID>,
    pub variadics: TempBuffer<hir::Variadic<'hir>>,
    pub field_inits: TempBuffer<hir::FieldInit<'hir>>,
    pub const_values: TempBuffer<hir::ConstValue<'hir>>,
    pub proc_ty_params: TempBuffer<hir::ProcTypeParam<'hir>>,
}

impl<'hir, 's, 'sref> HirCtx<'hir, 's, 'sref> {
    pub fn new(session: &'sref mut Session<'s>) -> HirCtx<'hir, 's, 'sref> {
        let core = hir::CoreItems {
            start: hir::ProcID::dummy(),
            index_out_of_bounds: hir::ProcID::dummy(),
            slice_range: hir::ProcID::dummy(),
            string_equals: hir::ProcID::dummy(),
            cstring_equals: hir::ProcID::dummy(),
            range_bound: None,
            type_info: hir::EnumID::dummy(),
            int_ty: hir::EnumID::dummy(),
            float_ty: hir::EnumID::dummy(),
            bool_ty: hir::EnumID::dummy(),
            string_ty: hir::EnumID::dummy(),
            any: None,
            source_loc: None,
        };

        let cache = Cache {
            proc_params: Vec::with_capacity(32),
            enum_variants: Vec::with_capacity(256),
            struct_fields: Vec::with_capacity(32),
            poly_param_names: Vec::with_capacity(32),
            u8s: TempBuffer::new(32),
            u64s: TempBuffer::new(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(64),
            exprs: TempBuffer::new(64),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            patterns: TempBuffer::new(32),
            var_ids: TempBuffer::new(32),
            variadics: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
            const_values: TempBuffer::new(64),
            proc_ty_params: TempBuffer::new(32),
        };

        HirCtx {
            arena: Arena::new(),
            emit: ErrorWarningBuffer::default(),
            in_const: false,
            scope: Scope::new(session),
            registry: Registry::new(session),
            enum_tag_set: HashMap::with_capacity(128),
            session,
            pat: PatCov::new(),
            core,
            cache,
            entry_point: None,
            enum_layout: HashMap::with_capacity(128),
            struct_layout: HashMap::with_capacity(128),
            variant_layout: HashMap::with_capacity(128),
        }
    }

    #[inline]
    pub fn src(&self, range: TextRange) -> SourceRange {
        SourceRange::new(self.scope.origin, range)
    }
    #[inline]
    pub fn name(&self, name_id: NameID) -> &'s str {
        self.session.intern_name.get(name_id)
    }

    pub fn array_len(&self, len: hir::ArrayStaticLen) -> Result<u64, ()> {
        match len {
            hir::ArrayStaticLen::Immediate(len) => Ok(len),
            hir::ArrayStaticLen::ConstEval(eval_id) => {
                let (eval, _, _) = self.registry.const_eval(eval_id);
                let value = eval.resolved()?;
                match value {
                    hir::ConstValue::Int { val, .. } => Ok(val),
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn finish(self) -> Result<hir::Hir<'hir>, ()> {
        //moving errors into per module storage
        let (errors, warnings) = self.emit.collect();
        let did_error = !errors.is_empty();

        for e in errors {
            let origin = match e.diagnostic().data() {
                DiagnosticData::Message => {
                    self.session.errors.error(e);
                    continue;
                }
                DiagnosticData::Context { main, .. } => main.src().module_id(),
                DiagnosticData::ContextVec { main, .. } => main.src().module_id(),
            };
            self.session.module.get_mut(origin).errors.error(e);
        }
        for w in warnings {
            let origin = match w.diagnostic().data() {
                DiagnosticData::Message => unreachable!(),
                DiagnosticData::Context { main, .. } => main.src().module_id(),
                DiagnosticData::ContextVec { main, .. } => main.src().module_id(),
            };
            self.session.module.get_mut(origin).errors.warning(w);
        }

        if did_error {
            return Err(());
        }
        let mut const_eval_values = Vec::with_capacity(self.registry.const_evals.len());
        for (eval, _, _) in self.registry.const_evals.iter() {
            const_eval_values.push(eval.resolved_unwrap());
        }
        let mut variant_eval_values = Vec::with_capacity(self.registry.const_evals.len());
        for eval in self.registry.variant_evals.iter() {
            variant_eval_values.push(eval.resolved_unwrap());
        }

        Ok(hir::Hir {
            arena: self.arena,
            procs: self.registry.hir_procs,
            enums: self.registry.hir_enums,
            structs: self.registry.hir_structs,
            globals: self.registry.hir_globals,
            const_eval_values,
            variant_eval_values,
            entry_point: self.entry_point,
            enum_layout: self.enum_layout,
            struct_layout: self.struct_layout,
            variant_layout: self.variant_layout,
            core: self.core,
        })
    }
}

pub struct Registry<'hir, 'ast> {
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    ast_imports: Vec<&'ast ast::ImportItem<'ast>>,
    hir_procs: Vec<hir::ProcData<'hir>>,
    hir_enums: Vec<hir::EnumData<'hir>>,
    hir_structs: Vec<hir::StructData<'hir>>,
    hir_consts: Vec<hir::ConstData<'hir>>,
    hir_globals: Vec<hir::GlobalData<'hir>>,
    hir_imports: Vec<hir::ImportData>,
    const_evals: Vec<(hir::ConstEval<'hir, 'ast>, ModuleID, PolyScope)>,
    variant_evals: Vec<hir::VariantEval<'hir>>,
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    fn new(session: &Session) -> Registry<'hir, 'ast> {
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

        const_eval_count += const_count;
        const_eval_count += global_count;

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
            const_evals: Vec::with_capacity(const_eval_count.next_power_of_two()),
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
        scope: PolyScope,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new(self.const_evals.len());
        self.const_evals.push((hir::ConstEval::Unresolved(const_expr), origin_id, scope));
        id
    }
    pub fn add_variant_eval(&mut self) -> hir::VariantEvalID {
        let id = hir::VariantEvalID::new(self.variant_evals.len());
        self.variant_evals.push(hir::VariantEval::Unresolved(()));
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
    pub fn const_eval(
        &self,
        id: hir::ConstEvalID,
    ) -> &(hir::ConstEval<'hir, 'ast>, ModuleID, PolyScope) {
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
    ) -> &mut (hir::ConstEval<'hir, 'ast>, ModuleID, PolyScope) {
        &mut self.const_evals[id.index()]
    }
    pub fn variant_eval_mut(&mut self, id: hir::VariantEvalID) -> &mut hir::VariantEval<'hir> {
        &mut self.variant_evals[id.index()]
    }
}

impl<'hir> super::types::SubstituteContext<'hir> for HirCtx<'hir, '_, '_> {
    fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.arena
    }
    fn types(&mut self) -> &mut TempBuffer<hir::Type<'hir>> {
        &mut self.cache.types
    }
    fn proc_ty_params(&mut self) -> &mut TempBuffer<hir::ProcTypeParam<'hir>> {
        &mut self.cache.proc_ty_params
    }
    fn take_types(&mut self, offset: TempOffset<hir::Type<'hir>>) -> &'hir [hir::Type<'hir>] {
        self.cache.types.take(offset, &mut self.arena)
    }
    fn take_proc_ty_params(
        &mut self,
        offset: TempOffset<hir::ProcTypeParam<'hir>>,
    ) -> &'hir [hir::ProcTypeParam<'hir>] {
        self.cache.proc_ty_params.take(offset, &mut self.arena)
    }
}

impl<'hir> super::layout::LayoutContext<'hir> for HirCtx<'hir, '_, '_> {
    fn u8s(&mut self) -> &mut TempBuffer<u8> {
        &mut self.cache.u8s
    }
    fn u64s(&mut self) -> &mut TempBuffer<u64> {
        &mut self.cache.u64s
    }
    fn take_u8s(&mut self, offset: TempOffset<u8>) -> &'hir [u8] {
        self.cache.u8s.take(offset, &mut self.arena)
    }
    fn take_u64s(&mut self, offset: TempOffset<u64>) -> &'hir [u64] {
        self.cache.u64s.take(offset, &mut self.arena)
    }

    fn error(&mut self) -> &mut impl ErrorSink {
        &mut self.emit
    }
    fn ptr_size(&self) -> u64 {
        self.session.config.target_ptr_width.ptr_size()
    }
    fn array_len(&self, len: hir::ArrayStaticLen) -> Result<u64, ()> {
        self.array_len(len)
    }
    fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        self.registry.enum_data(id)
    }
    fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        self.registry.struct_data(id)
    }

    fn enum_layout(&self) -> &HashMap<hir::EnumKey<'hir>, hir::Layout> {
        &self.enum_layout
    }
    fn struct_layout(&self) -> &HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>> {
        &self.struct_layout
    }
    fn variant_layout(&self) -> &HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>> {
        &self.variant_layout
    }

    fn enum_layout_mut(&mut self) -> &mut HashMap<hir::EnumKey<'hir>, hir::Layout> {
        &mut self.enum_layout
    }
    fn struct_layout_mut(&mut self) -> &mut HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>> {
        &mut self.struct_layout
    }
    fn variant_layout_mut(
        &mut self,
    ) -> &mut HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>> {
        &mut self.variant_layout
    }
}

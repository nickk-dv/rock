pub mod registry;
pub mod scope;

use crate::ast;
use crate::error::{ErrorWarningBuffer, SourceRange, WarningBuffer};
use crate::hir;
use crate::intern::NameID;
use crate::session::Session;
use crate::support::{Arena, TempBuffer};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirCtx<'hir, 's, 'sref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub in_const: bool,
    pub scope: scope::Scope<'hir>,
    pub registry: registry::Registry<'hir, 's>,
    pub enum_tag_set: HashMap<i128, hir::VariantID>,
    pub session: &'sref Session<'s>,
    pub cache: Cache<'hir>,
}

pub struct Cache<'hir> {
    pub proc_params: Vec<hir::Param<'hir>>,
    pub enum_variants: Vec<hir::Variant<'hir>>,
    pub struct_fields: Vec<hir::Field<'hir>>,
    pub poly_param_names: Vec<ast::Name>,
    pub types: TempBuffer<hir::Type<'hir>>,
    pub stmts: TempBuffer<hir::Stmt<'hir>>,
    pub exprs: TempBuffer<&'hir hir::Expr<'hir>>,
    pub branches: TempBuffer<hir::Branch<'hir>>,
    pub match_arms: TempBuffer<hir::MatchArm<'hir>>,
    pub patterns: TempBuffer<hir::Pat<'hir>>,
    pub var_ids: TempBuffer<hir::VariableID>,
    pub field_inits: TempBuffer<hir::FieldInit<'hir>>,
    pub const_values: TempBuffer<hir::ConstValue<'hir>>,
}

impl<'hir, 's, 'sref> HirCtx<'hir, 's, 'sref> {
    pub fn new(session: &'sref Session<'s>) -> HirCtx<'hir, 's, 'sref> {
        let cache = Cache {
            proc_params: Vec::with_capacity(32),
            enum_variants: Vec::with_capacity(256),
            struct_fields: Vec::with_capacity(32),
            poly_param_names: Vec::with_capacity(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(64),
            exprs: TempBuffer::new(64),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            patterns: TempBuffer::new(32),
            var_ids: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
            const_values: TempBuffer::new(64),
        };
        HirCtx {
            arena: Arena::new(),
            emit: ErrorWarningBuffer::default(),
            in_const: false,
            scope: scope::Scope::new(session),
            registry: registry::Registry::new(session),
            enum_tag_set: HashMap::with_capacity(128),
            session,
            cache,
        }
    }

    #[inline]
    pub fn src(&self, range: TextRange) -> SourceRange {
        SourceRange::new(self.scope.origin(), range)
    }
    #[inline]
    pub fn name(&self, name_id: NameID) -> &'s str {
        self.session.intern_name.get(name_id)
    }

    pub fn poly_param_name(&self, poly_def: hir::PolymorphDefID, poly_param_idx: u32) -> ast::Name {
        let poly_params = match poly_def {
            hir::PolymorphDefID::Proc(id) => self.registry.proc_data(id).poly_params.unwrap(),
            hir::PolymorphDefID::Enum(id) => self.registry.enum_data(id).poly_params.unwrap(),
            hir::PolymorphDefID::Struct(id) => self.registry.struct_data(id).poly_params.unwrap(),
        };
        poly_params[poly_param_idx as usize]
    }

    pub fn finish(self) -> Result<(hir::Hir<'hir>, WarningBuffer), ErrorWarningBuffer> {
        //@dont unwrap, make all required core dependencies checked
        let core = hir::CoreItems {
            string_equals: super::pass_5::core_find_proc(&self, "slice", "string_equals").unwrap(),
            cstring_equals: super::pass_5::core_find_proc(&self, "slice", "cstring_equals")
                .unwrap(),
        };

        let ((), warnings) = self.emit.result(())?;

        let mut const_eval_values = Vec::with_capacity(self.registry.const_evals.len());
        for (eval, _) in self.registry.const_evals.iter() {
            const_eval_values.push(eval.resolved_unwrap());
        }
        let mut variant_eval_values = Vec::with_capacity(self.registry.const_evals.len());
        for eval in self.registry.variant_evals.iter() {
            variant_eval_values.push(eval.resolved_unwrap());
        }

        let hir = hir::Hir {
            arena: self.arena,
            procs: self.registry.hir_procs,
            enums: self.registry.hir_enums,
            structs: self.registry.hir_structs,
            globals: self.registry.hir_globals,
            const_eval_values,
            variant_eval_values,
            core,
        };
        Ok((hir, warnings))
    }
}

//@move?
impl hir::ArrayStaticLen {
    pub fn get_resolved(self, ctx: &HirCtx) -> Result<u64, ()> {
        match self {
            hir::ArrayStaticLen::Immediate(len) => Ok(len),
            hir::ArrayStaticLen::ConstEval(eval_id) => {
                let (eval, _) = *ctx.registry.const_eval(eval_id);
                let value = eval.resolved()?;
                match value {
                    hir::ConstValue::Int { val, .. } => Ok(val),
                    _ => unreachable!(),
                }
            }
        }
    }
}

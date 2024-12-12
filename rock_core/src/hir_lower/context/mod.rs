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

pub struct HirCtx<'hir, 's, 's_ref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub scope: scope::Scope<'hir>,
    pub registry: registry::Registry<'hir, 's>,
    pub enum_tag_set: HashMap<i128, hir::VariantID>,
    pub session: &'s_ref Session<'s>,
    pub cache: Cache<'hir>,
}

pub struct Cache<'hir> {
    pub proc_params: Vec<hir::Param<'hir>>,
    pub enum_variants: Vec<hir::Variant<'hir>>,
    pub struct_fields: Vec<hir::Field<'hir>>,
    pub types: TempBuffer<hir::Type<'hir>>,
    pub stmts: TempBuffer<hir::Stmt<'hir>>,
    pub exprs: TempBuffer<&'hir hir::Expr<'hir>>,
    pub branches: TempBuffer<hir::Branch<'hir>>,
    pub match_arms: TempBuffer<hir::MatchArm<'hir>>,
    pub patterns: TempBuffer<hir::Pat<'hir>>,
    pub bind_ids: TempBuffer<hir::LocalBindID>,
    pub field_inits: TempBuffer<hir::FieldInit<'hir>>,
}

impl<'hir, 's, 's_ref> HirCtx<'hir, 's, 's_ref> {
    pub fn new(session: &'s_ref Session<'s>) -> HirCtx<'hir, 's, 's_ref> {
        let cache = Cache {
            proc_params: Vec::with_capacity(32),
            enum_variants: Vec::with_capacity(256),
            struct_fields: Vec::with_capacity(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(64),
            exprs: TempBuffer::new(64),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            patterns: TempBuffer::new(32),
            bind_ids: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
        };
        HirCtx {
            arena: Arena::new(),
            emit: ErrorWarningBuffer::default(),
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
        poly_params.names[poly_param_idx as usize]
    }

    pub fn finish(self) -> Result<(hir::Hir<'hir>, WarningBuffer), ErrorWarningBuffer> {
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
            consts: self.registry.hir_consts,
            globals: self.registry.hir_globals,
            const_eval_values,
            variant_eval_values,
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

pub mod registry;
pub mod scope;

use crate::ast;
use crate::error::{ErrorWarningBuffer, SourceRange, WarningBuffer};
use crate::hir;
use crate::intern::NameID;
use crate::session::Session;
use crate::support::Arena;
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirCtx<'hir, 's, 's_ref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub scope: scope::Scope<'hir>,
    pub registry: registry::Registry<'hir, 's>,
    pub const_intern: hir::ConstInternPool<'hir>,
    pub enum_tag_set: HashMap<i128, hir::VariantID>,
    pub session: &'s_ref Session<'s>,
}

impl<'hir, 's, 's_ref> HirCtx<'hir, 's, 's_ref> {
    pub fn new(session: &'s_ref Session<'s>) -> HirCtx<'hir, 's, 's_ref> {
        HirCtx {
            arena: Arena::new(),
            emit: ErrorWarningBuffer::default(),
            scope: scope::Scope::new(session),
            registry: registry::Registry::new(session),
            const_intern: hir::ConstInternPool::new(),
            enum_tag_set: HashMap::with_capacity(128),
            session,
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

    pub fn generic_param_name(
        &self,
        gen_item_id: hir::GenericItemID,
        gen_param_idx: u32,
    ) -> ast::Name {
        let gen_params = match gen_item_id {
            hir::GenericItemID::Proc(id) => self.registry.proc_data(id).gen_params.unwrap(),
            hir::GenericItemID::Enum(id) => self.registry.enum_data(id).gen_params.unwrap(),
            hir::GenericItemID::Struct(id) => self.registry.struct_data(id).gen_params.unwrap(),
        };
        gen_params.names[gen_param_idx as usize]
    }

    pub fn finish(self) -> Result<(hir::Hir<'hir>, WarningBuffer), ErrorWarningBuffer> {
        let ((), warnings) = self.emit.result(())?;

        let mut const_values = Vec::with_capacity(self.registry.const_evals.len());
        for (eval, _) in self.registry.const_evals.iter() {
            const_values.push(eval.resolved_unwrap());
        }

        let mut variant_tag_values = Vec::with_capacity(self.registry.const_evals.len());
        for eval in self.registry.variant_evals.iter() {
            variant_tag_values.push(eval.resolved_unwrap());
        }

        let hir = hir::Hir {
            arena: self.arena,
            const_intern: self.const_intern,
            procs: self.registry.hir_procs,
            enums: self.registry.hir_enums,
            structs: self.registry.hir_structs,
            consts: self.registry.hir_consts,
            globals: self.registry.hir_globals,
            const_values,
            variant_tag_values,
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
                let value_id = eval.resolved()?;

                match ctx.const_intern.get(value_id) {
                    hir::ConstValue::Int { val, .. } => Ok(val),
                    _ => unreachable!(),
                }
            }
        }
    }
}

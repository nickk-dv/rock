pub mod registry;
pub mod scope;

use crate::error::{ErrorWarningBuffer, SourceRange, WarningBuffer};
use crate::hir;
use crate::intern::InternName;
use crate::session::Session;
use crate::support::{Arena, ID};
use crate::text::TextRange;

pub struct HirCtx<'hir, 's, 's_ref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub scope: scope::Scope<'hir>,
    pub registry: registry::Registry<'hir, 's>,
    pub const_intern: hir::ConstInternPool<'hir>,
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
            session,
        }
    }

    #[inline]
    pub fn src(&self, range: TextRange) -> SourceRange {
        SourceRange::new(self.scope.origin(), range)
    }
    #[inline]
    pub fn name(&self, name_id: ID<InternName>) -> &'s str {
        self.session.intern_name.get(name_id)
    }

    pub fn finish(self) -> Result<(hir::Hir<'hir>, WarningBuffer), ErrorWarningBuffer> {
        let ((), warnings) = self.emit.result(())?;

        let mut const_values = Vec::with_capacity(self.registry.const_evals.len());
        for (eval, _) in self.registry.const_evals.iter() {
            const_values.push(eval.get_resolved().expect("resolved constant"));
        }

        let mut variant_tag_values = Vec::with_capacity(self.registry.const_evals.len());
        for eval in self.registry.variant_evals.iter() {
            variant_tag_values.push(eval.get_resolved().expect("resolved variant tag"));
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

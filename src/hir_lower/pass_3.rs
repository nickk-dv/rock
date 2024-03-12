use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity, SourceRange};
use crate::hir;
use crate::hir::hir_builder as hb;

pub fn run(hb: &mut hb::HirBuilder) {}

/*
let scope = hb.get_scope(scope_id);
let mut unique_params = Vec::<hir::ProcParam>::new();

for param in decl.params.iter() {
    if let Some(existing) = unique_params.iter().find_map(|it| {
        if it.name.id == param.name.id {
            Some(it)
        } else {
            None
        }
    }) {
        p.errors.push(
            ErrorComp::new(
                format!(
                    "parameter `{}` is defined multiple times",
                    hb.ctx.intern().get_str(param.name.id)
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(param.name.range),
            )
            .context(
                "existing parameter".into(),
                ErrorSeverity::InfoHint,
                Some(scope.source(existing.name.range)),
            ),
        );
    } else {
        unique_params.push(hir::ProcParam {
            mutt: param.mutt,
            name: param.name,
            ty: hir::Type::Error,
        });
    }
}
let params = hb.arena().alloc_slice(&unique_params);
*/

/*
let scope = hb.get_scope(scope_id);
let mut unique_variants = Vec::<hir::EnumVariant>::new();

for variant in decl.variants.iter() {
    if let Some(existing) = unique_variants.iter().find_map(|it| {
        if it.name.id == variant.name.id {
            Some(it)
        } else {
            None
        }
    }) {
        p.errors.push(
            ErrorComp::new(
                format!(
                    "variant `{}` is defined multiple times",
                    hb.ctx.intern().get_str(variant.name.id)
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(variant.name.range),
            )
            .context(
                "existing variant".into(),
                ErrorSeverity::InfoHint,
                Some(scope.source(existing.name.range)),
            ),
        );
    } else {
        unique_variants.push(hir::EnumVariant {
            name: variant.name,
            value: None, //@optional constant expr value not handled
        });
    }
}
let variants = hb.arena().alloc_slice(&unique_variants);
*/

/*
let scope = hb.get_scope(scope_id);
let mut unique_members = Vec::<hir::UnionMember>::new();

for member in decl.members.iter() {
    if let Some(existing) = unique_members.iter().find_map(|it| {
        if it.name.id == member.name.id {
            Some(it)
        } else {
            None
        }
    }) {
        p.errors.push(
            ErrorComp::new(
                format!(
                    "member `{}` is defined multiple times",
                    hb.ctx.intern().get_str(member.name.id)
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(member.name.range),
            )
            .context(
                "existing member".into(),
                ErrorSeverity::InfoHint,
                Some(scope.source(existing.name.range)),
            ),
        );
    } else {
        unique_members.push(hir::UnionMember {
            name: member.name,
            ty: hir::Type::Error, // @not handled
        });
    }
}
let members = hb.arena().alloc_slice(&unique_members);
*/

/*
let scope = hb.get_scope(scope_id);
let mut unique_fields = Vec::<hir::StructField>::new();

for field in decl.fields.iter() {
    if let Some(existing) = unique_fields.iter().find_map(|it| {
        if it.name.id == field.name.id {
            Some(it)
        } else {
            None
        }
    }) {
        p.errors.push(
            ErrorComp::new(
                format!(
                    "field `{}` is defined multiple times",
                    hb.ctx.intern().get_str(field.name.id)
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(field.name.range),
            )
            .context(
                "existing field".into(),
                ErrorSeverity::InfoHint,
                Some(scope.source(existing.name.range)),
            ),
        );
    } else {
        unique_fields.push(hir::StructField {
            vis: field.vis,
            name: field.name,
            ty: hir::Type::Error, // @not handled
        });
    }
}
let fields = hb.arena().alloc_slice(&unique_fields);
*/

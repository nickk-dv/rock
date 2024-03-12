use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity, SourceRange};
use crate::hir;
use crate::hir::hir_builder as hb;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.enum_ids() {
        let decl = hb.enum_ast(id);
        let from_id = hb.enum_data(id).from_id;

        let mut unique_variants = Vec::<hir::EnumVariant>::new();
        for variant in decl.variants.iter() {
            if let Some(existing) = unique_variants.iter().find_map(|it| {
                if it.name.id == variant.name.id {
                    Some(it)
                } else {
                    None
                }
            }) {
                let scope = hb.get_scope(from_id);
                hb.error(
                    ErrorComp::new(
                        format!(
                            "variant `{}` is defined multiple times",
                            hb.name_str(variant.name.id)
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
                let value = if let Some(const_expr) = variant.value {
                    let const_id = hb.add_const_expr(from_id, const_expr);
                    match const_expr.0.kind {
                        ast::ExprKind::LitInt { val, ty } => {
                            let resolved = hb.arena().alloc_ref_new(hir::Expr {
                                kind: hir::ExprKind::LitInt {
                                    val,
                                    ty: ast::BasicType::U64,
                                },
                                range: const_expr.0.range,
                            });
                            hb.resolve_const_expr(const_id, resolved);
                        }
                        _ => {
                            let source = hb.get_scope(from_id).source(const_expr.0.range);
                            hb.error(ErrorComp::new(
                                "only integers can be constant expressions for now".into(),
                                ErrorSeverity::Error,
                                source,
                            ));
                        }
                    }
                    Some(const_id)
                } else {
                    None
                };
                unique_variants.push(hir::EnumVariant {
                    name: variant.name,
                    value,
                });
            }
        }

        let variants = hb.arena().alloc_slice(&unique_variants);
        let data_mut = hb.enum_data_mut(id);
        data_mut.variants = variants;
    }
}

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

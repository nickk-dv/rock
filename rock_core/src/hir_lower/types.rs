use crate::hir;
use crate::support::{Arena, TempBuffer, TempOffset};

pub trait SubstituteContext<'hir> {
    fn arena(&mut self) -> &mut Arena<'hir>;
    fn types(&mut self) -> &mut TempBuffer<hir::Type<'hir>>;
    fn proc_ty_params(&mut self) -> &mut TempBuffer<hir::ProcTypeParam<'hir>>;
    fn take_types(&mut self, offset: TempOffset<hir::Type<'hir>>) -> &'hir [hir::Type<'hir>];
    fn take_proc_ty_params(
        &mut self,
        offset: TempOffset<hir::ProcTypeParam<'hir>>,
    ) -> &'hir [hir::ProcTypeParam<'hir>];
}

pub fn expect_concrete(types: &[hir::Type]) {
    assert!(types.iter().all(|ty| !has_poly_param(*ty)));
}

pub fn substitute<'hir>(
    ctx: &mut impl SubstituteContext<'hir>,
    ty: hir::Type<'hir>,
    poly_set: &[hir::Type<'hir>],
    proc_set: Option<&[hir::Type<'hir>]>,
) -> hir::Type<'hir> {
    match ty {
        hir::Type::Error | hir::Type::Unknown | hir::Type::Char | hir::Type::Void => ty,
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar | hir::Type::Int(_) => ty,
        hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => ty,
        hir::Type::PolyProc(_, idx) => match proc_set {
            Some(proc_set) => proc_set[idx],
            None => poly_set[idx],
        },
        hir::Type::PolyEnum(_, idx) => poly_set[idx],
        hir::Type::PolyStruct(_, idx) => poly_set[idx],
        hir::Type::Enum(enum_id, poly_types) => {
            hir::Type::Enum(enum_id, substitute_types(ctx, poly_types, poly_set, proc_set))
        }
        hir::Type::Struct(struct_id, poly_types) => {
            hir::Type::Struct(struct_id, substitute_types(ctx, poly_types, poly_set, proc_set))
        }
        hir::Type::Reference(mutt, ref_ty) => {
            let ref_ty = substitute(ctx, *ref_ty, poly_set, proc_set);
            hir::Type::Reference(mutt, ctx.arena().alloc(ref_ty))
        }
        hir::Type::MultiReference(mutt, ref_ty) => {
            let ref_ty = substitute(ctx, *ref_ty, poly_set, proc_set);
            hir::Type::MultiReference(mutt, ctx.arena().alloc(ref_ty))
        }
        hir::Type::Procedure(proc_ty) => {
            let offset = ctx.proc_ty_params().start();
            for param in proc_ty.params {
                let param = hir::ProcTypeParam {
                    ty: substitute(ctx, param.ty, poly_set, proc_set),
                    kind: param.kind,
                };
                ctx.proc_ty_params().push(param);
            }
            let proc_ty = hir::ProcType {
                flag_set: proc_ty.flag_set,
                params: ctx.take_proc_ty_params(offset),
                return_ty: substitute(ctx, proc_ty.return_ty, poly_set, proc_set),
            };
            hir::Type::Procedure(ctx.arena().alloc(proc_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let elem_ty = substitute(ctx, slice.elem_ty, poly_set, proc_set);
            let slice = hir::ArraySlice { mutt: slice.mutt, elem_ty };
            hir::Type::ArraySlice(ctx.arena().alloc(slice))
        }
        hir::Type::ArrayStatic(array) => {
            let elem_ty = substitute(ctx, array.elem_ty, poly_set, proc_set);
            let array = hir::ArrayStatic { elem_ty, len: array.len };
            hir::Type::ArrayStatic(ctx.arena().alloc(array))
        }
    }
}

pub fn substitute_types<'hir>(
    ctx: &mut impl SubstituteContext<'hir>,
    types: &'hir [hir::Type<'hir>],
    poly_set: &[hir::Type<'hir>],
    proc_set: Option<&[hir::Type<'hir>]>,
) -> &'hir [hir::Type<'hir>] {
    let offset = ctx.types().start();
    let mut any_poly = false;

    for ty in types.iter().copied() {
        if !has_poly_param(ty) {
            ctx.types().push(ty);
            continue;
        }
        any_poly = true;
        let concrete = substitute(ctx, ty, poly_set, proc_set);
        ctx.types().push(concrete);
    }

    if any_poly {
        ctx.take_types(offset)
    } else {
        ctx.types().pop_view(offset);
        types
    }
}

pub fn apply_inference<'hir>(
    infer: &mut [hir::Type<'hir>],
    ty: hir::Type<'hir>,
    def_ty: hir::Type,
) {
    let poly_idx = match def_ty {
        hir::Type::PolyProc(_, idx) => Some(idx),
        hir::Type::PolyEnum(_, idx) => Some(idx),
        hir::Type::PolyStruct(_, idx) => Some(idx),
        _ => None,
    };
    if let Some(poly_idx) = poly_idx {
        if infer[poly_idx].is_unknown() {
            infer[poly_idx] = ty;
        }
        return;
    }
    match (ty, def_ty) {
        (hir::Type::Enum(id, types), hir::Type::Enum(def_id, def_types)) => {
            if id == def_id {
                (0..types.len()).for_each(|idx| apply_inference(infer, types[idx], def_types[idx]));
            }
        }
        (hir::Type::Struct(id, types), hir::Type::Struct(def_id, def_types)) => {
            if id == def_id {
                (0..types.len()).for_each(|idx| apply_inference(infer, types[idx], def_types[idx]));
            }
        }
        (hir::Type::Reference(_, ref_ty), hir::Type::Reference(_, def_ref_ty)) => {
            apply_inference(infer, *ref_ty, *def_ref_ty)
        }
        (hir::Type::MultiReference(_, ref_ty), hir::Type::MultiReference(_, def_ref_ty)) => {
            apply_inference(infer, *ref_ty, *def_ref_ty)
        }
        (hir::Type::Procedure(proc_ty), hir::Type::Procedure(def_proc_ty)) => {
            for (idx, def_param) in def_proc_ty.params.iter().enumerate() {
                if let Some(param) = proc_ty.params.get(idx) {
                    apply_inference(infer, param.ty, def_param.ty)
                }
            }
            apply_inference(infer, proc_ty.return_ty, def_proc_ty.return_ty)
        }
        (hir::Type::ArraySlice(slice), hir::Type::ArraySlice(def_slice)) => {
            apply_inference(infer, slice.elem_ty, def_slice.elem_ty)
        }
        (hir::Type::ArrayStatic(array), hir::Type::ArrayStatic(def_array)) => {
            apply_inference(infer, array.elem_ty, def_array.elem_ty)
        }
        _ => {}
    }
}

pub fn has_poly_param(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error | hir::Type::Unknown | hir::Type::Char | hir::Type::Void => false,
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar | hir::Type::Int(_) => false,
        hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => false,
        hir::Type::PolyProc(_, _) | hir::Type::PolyEnum(_, _) | hir::Type::PolyStruct(_, _) => true,
        hir::Type::Enum(_, poly_types) => poly_types.iter().copied().any(|ty| has_poly_param(ty)),
        hir::Type::Struct(_, poly_types) => poly_types.iter().copied().any(|ty| has_poly_param(ty)),
        hir::Type::Reference(_, ref_ty) => has_poly_param(*ref_ty),
        hir::Type::MultiReference(_, ref_ty) => has_poly_param(*ref_ty),
        hir::Type::Procedure(proc_ty) => {
            has_poly_param(proc_ty.return_ty) || proc_ty.params.iter().any(|p| has_poly_param(p.ty))
        }
        hir::Type::ArraySlice(slice) => has_poly_param(slice.elem_ty),
        hir::Type::ArrayStatic(array) => has_poly_param(array.elem_ty),
    }
}

//making an assumption that polymorphic type parameter,
//that is part of the enum or struct type affects its layout.
//this avoids tracking extra param data or walking the type fields.
pub fn has_poly_layout_dep(ty: hir::Type) -> bool {
    match ty {
        hir::Type::Error | hir::Type::Unknown | hir::Type::Char | hir::Type::Void => false,
        hir::Type::Never | hir::Type::Rawptr | hir::Type::UntypedChar | hir::Type::Int(_) => false,
        hir::Type::Float(_) | hir::Type::Bool(_) | hir::Type::String(_) => false,
        hir::Type::PolyProc(_, _) | hir::Type::PolyEnum(_, _) | hir::Type::PolyStruct(_, _) => true,
        hir::Type::Enum(_, poly_types) => {
            poly_types.iter().copied().any(|ty| has_poly_layout_dep(ty))
        }
        hir::Type::Struct(_, poly_types) => {
            poly_types.iter().copied().any(|ty| has_poly_layout_dep(ty))
        }
        hir::Type::Reference(_, _)
        | hir::Type::MultiReference(_, _)
        | hir::Type::Procedure(_)
        | hir::Type::ArraySlice(_) => false,
        hir::Type::ArrayStatic(array) => has_poly_layout_dep(array.elem_ty),
    }
}

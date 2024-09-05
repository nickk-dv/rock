use crate::ast::BasicType;
use crate::error::SourceRange;
use crate::hir;
use crate::hir_lower::context::HirCtx;
use crate::hir_lower::errors as err;

pub fn type_layout(ctx: &mut HirCtx, ty: hir::Type, src: SourceRange) -> Result<hir::Layout, ()> {
    match ty {
        hir::Type::Error => Err(()),
        hir::Type::Basic(basic) => Ok(basic_layout(ctx, basic)),
        hir::Type::Enum(id) => {
            let data = ctx.registry.enum_data(id);
            data.layout.get_resolved()
        }
        hir::Type::Struct(id) => {
            let data = ctx.registry.struct_data(id);
            data.layout.get_resolved()
        }
        hir::Type::Reference(_, _) => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            Ok(hir::Layout::new_equal(ptr_size))
        }
        hir::Type::Procedure(_) => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            Ok(hir::Layout::new_equal(ptr_size))
        }
        hir::Type::ArraySlice(_) => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            Ok(hir::Layout::new(ptr_size * 2, ptr_size))
        }
        hir::Type::ArrayStatic(array) => {
            let len = array.len.get_resolved(ctx)?;
            let elem_layout = type_layout(ctx, array.elem_ty, src)?;
            let elem_size = elem_layout.size();

            if let Some(total) = elem_size.checked_mul(len) {
                Ok(hir::Layout::new(total, elem_layout.align()))
            } else {
                err::const_array_size_overflow(&mut ctx.emit, src, elem_size, len);
                Err(())
            }
        }
    }
}

pub fn basic_layout(ctx: &HirCtx, basic: BasicType) -> hir::Layout {
    match basic {
        BasicType::S8 => hir::Layout::new_equal(1),
        BasicType::S16 => hir::Layout::new_equal(2),
        BasicType::S32 => hir::Layout::new_equal(4),
        BasicType::S64 => hir::Layout::new_equal(8),
        BasicType::Ssize => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::U8 => hir::Layout::new_equal(1),
        BasicType::U16 => hir::Layout::new_equal(2),
        BasicType::U32 => hir::Layout::new_equal(4),
        BasicType::U64 => hir::Layout::new_equal(8),
        BasicType::Usize => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::F32 => hir::Layout::new_equal(4),
        BasicType::F64 => hir::Layout::new_equal(8),
        BasicType::Bool => hir::Layout::new_equal(1),
        BasicType::Char => hir::Layout::new_equal(4),
        BasicType::Rawptr => {
            let ptr_size = ctx.target.arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::Void => hir::Layout::new(0, 1),
        BasicType::Never => hir::Layout::new(0, 1),
    }
}

pub fn resolve_enum_layout(ctx: &mut HirCtx, enum_id: hir::EnumID) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    let data = ctx.registry.enum_data(enum_id);
    for variant in data.variants {
        let variant_layout = resolve_variant_layout(ctx, enum_id, variant)?;

        size = size.max(variant_layout.size());
        align = align.max(variant_layout.align());
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

pub fn resolve_struct_layout(
    ctx: &mut HirCtx,
    struct_id: hir::StructID,
) -> Result<hir::Layout, ()> {
    let data = ctx.registry.struct_data(struct_id);
    let src = SourceRange::new(data.origin_id, data.name.range);

    let mut types = data.fields.iter().map(|f| f.ty);
    resolve_aggregate_layout(ctx, src, "struct", &mut types)
}

fn resolve_variant_layout(
    ctx: &mut HirCtx,
    enum_id: hir::EnumID,
    variant: &hir::Variant,
) -> Result<hir::Layout, ()> {
    let data = ctx.registry.enum_data(enum_id);
    let src = SourceRange::new(data.origin_id, variant.name.range);

    let tag_ty = data.tag_ty?;
    let tag_ty = [hir::Type::Basic(tag_ty.into_basic())];
    let mut types = tag_ty.iter().copied().chain(variant.fields.iter().copied());
    resolve_aggregate_layout(ctx, src, "variant", &mut types)
}

fn resolve_aggregate_layout<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    item_kind: &'static str,
    types: &mut dyn Iterator<Item = hir::Type<'hir>>,
) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for ty in types {
        let elem_layout = type_layout(ctx, ty, src)?;
        let elem_size = elem_layout.size();

        size = aligned_size(size, elem_layout.align());
        align = align.max(elem_layout.align());

        if let Some(total) = size.checked_add(elem_size) {
            size = total;
        } else {
            err::const_item_size_overflow(&mut ctx.emit, src, item_kind, size, elem_size);
            return Err(());
        }
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

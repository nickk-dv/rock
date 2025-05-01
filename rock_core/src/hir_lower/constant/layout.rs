use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::hir_lower::context::HirCtx;

pub fn type_layout(ctx: &mut HirCtx, ty: hir::Type, src: SourceRange) -> Result<hir::Layout, ()> {
    match ty {
        hir::Type::Error => Err(()),
        hir::Type::Unknown => unreachable!(),
        hir::Type::Char => Ok(hir::Layout::equal(4)),
        hir::Type::Void => Ok(hir::Layout::new(0, 1)),
        hir::Type::Never => Ok(hir::Layout::new(0, 1)),
        hir::Type::UntypedChar => unreachable!(),
        hir::Type::Rawptr => Ok(hir::Layout::equal(ctx.session.config.target_ptr_width.ptr_size())),
        hir::Type::Int(int_ty) => Ok(int_layout(ctx, int_ty)),
        hir::Type::Float(float_ty) => Ok(float_layout(float_ty)),
        hir::Type::Bool(bool_ty) => Ok(bool_layout(bool_ty)),
        hir::Type::String(string_ty) => Ok(string_layout(ctx, string_ty)),
        hir::Type::PolyProc(_, _) | hir::Type::PolyEnum(_, _) | hir::Type::PolyStruct(_, _) => {
            eprintln!("unhandled poly param in (type_layout)");
            return Err(());
        }
        hir::Type::Enum(id, poly_types) => {
            if !poly_types.is_empty() {
                err::internal_not_implemented(&mut ctx.emit, src, "polymorphic enum layout");
                return Err(());
            }
            ctx.registry.enum_data(id).layout.resolved()
        }
        hir::Type::Struct(id, poly_types) => {
            if !poly_types.is_empty() {
                err::internal_not_implemented(&mut ctx.emit, src, "polymorphic struct layout");
                return Err(());
            }
            ctx.registry.struct_data(id).layout.resolved()
        }
        hir::Type::Reference(_, _) | hir::Type::MultiReference(_, _) | hir::Type::Procedure(_) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::equal(ptr_size))
        }
        hir::Type::ArraySlice(_) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::new(2 * ptr_size, ptr_size))
        }
        hir::Type::ArrayStatic(array) => {
            let len = array.len.get_resolved(ctx)?;
            let elem_layout = type_layout(ctx, array.elem_ty, src)?;
            let elem_size = elem_layout.size;

            if let Some(total) = elem_size.checked_mul(len) {
                Ok(hir::Layout::new(total, elem_layout.align))
            } else {
                err::const_array_size_overflow(&mut ctx.emit, src, elem_size, len);
                Err(())
            }
        }
    }
}

#[inline]
pub fn int_layout(ctx: &HirCtx, int_ty: hir::IntType) -> hir::Layout {
    match int_ty {
        hir::IntType::S8 | hir::IntType::U8 => hir::Layout::equal(1),
        hir::IntType::S16 | hir::IntType::U16 => hir::Layout::equal(2),
        hir::IntType::S32 | hir::IntType::U32 => hir::Layout::equal(4),
        hir::IntType::S64 | hir::IntType::U64 => hir::Layout::equal(8),
        hir::IntType::Ssize | hir::IntType::Usize => {
            hir::Layout::equal(ctx.session.config.target_ptr_width.ptr_size())
        }
        hir::IntType::Untyped => unreachable!(),
    }
}

#[inline]
pub fn float_layout(float_ty: hir::FloatType) -> hir::Layout {
    match float_ty {
        hir::FloatType::F32 => hir::Layout::equal(4),
        hir::FloatType::F64 => hir::Layout::equal(8),
        hir::FloatType::Untyped => unreachable!(),
    }
}

#[inline]
pub fn bool_layout(bool_ty: hir::BoolType) -> hir::Layout {
    match bool_ty {
        hir::BoolType::Bool => hir::Layout::equal(1),
        hir::BoolType::Bool16 => hir::Layout::equal(2),
        hir::BoolType::Bool32 => hir::Layout::equal(4),
        hir::BoolType::Bool64 => hir::Layout::equal(8),
        hir::BoolType::Untyped => unreachable!(),
    }
}

#[inline]
pub fn string_layout(ctx: &HirCtx, string_ty: hir::StringType) -> hir::Layout {
    let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
    match string_ty {
        hir::StringType::String => hir::Layout::new(2 * ptr_size, ptr_size),
        hir::StringType::CString => hir::Layout::equal(ptr_size),
        hir::StringType::Untyped => unreachable!(),
    }
}

pub fn resolve_struct_layout(
    ctx: &mut HirCtx,
    struct_id: hir::StructID,
) -> Result<hir::Layout, ()> {
    let data = ctx.registry.struct_data(struct_id);
    let src = SourceRange::new(data.origin_id, data.name.range);
    let types = data.fields.iter().map(|f| f.ty);
    resolve_aggregate_layout(ctx, src, "struct", types)
}

fn resolve_variant_layout(
    ctx: &mut HirCtx,
    enum_id: hir::EnumID,
    variant: &hir::Variant,
) -> Result<hir::Layout, ()> {
    let data = ctx.registry.enum_data(enum_id);
    let src = SourceRange::new(data.origin_id, variant.name.range);
    let tag_ty = [hir::Type::Int(data.tag_ty.resolved()?)];
    let types = tag_ty.iter().copied().chain(variant.fields.iter().map(|f| &f.ty).copied());
    resolve_aggregate_layout(ctx, src, "variant", types)
}

fn resolve_aggregate_layout<'hir, I: Iterator<Item = hir::Type<'hir>>>(
    ctx: &mut HirCtx,
    src: SourceRange,
    item_kind: &'static str,
    types: I,
) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for ty in types {
        let elem_layout = type_layout(ctx, ty, src)?;
        size = aligned_size(size, elem_layout.align);
        align = align.max(elem_layout.align);

        if let Some(total) = size.checked_add(elem_layout.size) {
            size = total;
        } else {
            err::const_item_size_overflow(&mut ctx.emit, src, item_kind, size, elem_layout.size);
            return Err(());
        }
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

pub fn resolve_enum_layout(ctx: &mut HirCtx, enum_id: hir::EnumID) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    let data = ctx.registry.enum_data(enum_id);
    for variant in data.variants {
        let variant_layout = resolve_variant_layout(ctx, enum_id, variant)?;
        size = size.max(variant_layout.size);
        align = align.max(variant_layout.align);
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

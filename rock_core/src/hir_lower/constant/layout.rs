use crate::ast::BasicType;
use crate::error::SourceRange;
use crate::hir;
use crate::hir_lower::errors as err;
use crate::hir_lower::hir_build::{HirData, HirEmit};

pub fn type_layout(
    hir: &HirData,
    emit: &mut HirEmit,
    ty: hir::Type,
    src: SourceRange,
) -> Result<hir::Layout, ()> {
    match ty {
        hir::Type::Error => Err(()),
        hir::Type::Basic(basic) => Ok(basic_layout(hir, basic)),
        hir::Type::Enum(id) => {
            let data = hir.registry().enum_data(id);
            data.layout.get_resolved()
        }
        hir::Type::Struct(id) => {
            let data = hir.registry().struct_data(id);
            data.layout.get_resolved()
        }
        hir::Type::Reference(_, _) => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            Ok(hir::Layout::new_equal(ptr_size))
        }
        hir::Type::Procedure(_) => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            Ok(hir::Layout::new_equal(ptr_size))
        }
        hir::Type::ArraySlice(_) => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            Ok(hir::Layout::new(ptr_size * 2, ptr_size))
        }
        hir::Type::ArrayStatic(array) => {
            let len = array.len.get_resolved(hir, emit)?;
            let elem_layout = type_layout(hir, emit, array.elem_ty, src)?;
            let elem_size = elem_layout.size();

            if let Some(val) = elem_size.checked_mul(len) {
                Ok(hir::Layout::new(val, elem_layout.align()))
            } else {
                err::const_array_size_overflow(emit, src, elem_size, len);
                Err(())
            }
        }
    }
}

pub fn basic_layout(hir: &HirData, basic: BasicType) -> hir::Layout {
    match basic {
        BasicType::S8 => hir::Layout::new_equal(1),
        BasicType::S16 => hir::Layout::new_equal(2),
        BasicType::S32 => hir::Layout::new_equal(4),
        BasicType::S64 => hir::Layout::new_equal(8),
        BasicType::Ssize => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::U8 => hir::Layout::new_equal(1),
        BasicType::U16 => hir::Layout::new_equal(2),
        BasicType::U32 => hir::Layout::new_equal(4),
        BasicType::U64 => hir::Layout::new_equal(8),
        BasicType::Usize => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::F32 => hir::Layout::new_equal(4),
        BasicType::F64 => hir::Layout::new_equal(8),
        BasicType::Bool => hir::Layout::new_equal(1),
        BasicType::Char => hir::Layout::new_equal(4),
        BasicType::Rawptr => {
            let ptr_size = hir.target().arch().ptr_width().ptr_size();
            hir::Layout::new_equal(ptr_size)
        }
        BasicType::Void => hir::Layout::new(0, 1),
        BasicType::Never => hir::Layout::new(0, 1),
    }
}

pub fn resolve_struct_layout(
    hir: &HirData,
    emit: &mut HirEmit,
    struct_id: hir::StructID,
) -> Result<hir::Layout, ()> {
    let data = hir.registry().struct_data(struct_id);
    let src = SourceRange::new(data.origin_id, data.name.range);
    let mut types = data.fields.iter().map(|f| f.ty);
    resolve_aggregate_layout(hir, emit, src, "struct", &mut types)
}

fn resolve_variant_layout(
    hir: &HirData,
    emit: &mut HirEmit,
    data: &hir::EnumData,
    variant: &hir::Variant,
) -> Result<hir::Layout, ()> {
    let src = SourceRange::new(data.origin_id, variant.name.range);
    let tag_ty = data.tag_ty?;
    let tag_ty = [hir::Type::Basic(tag_ty.into_basic())];
    let mut types = tag_ty.iter().copied().chain(variant.fields.iter().copied());
    resolve_aggregate_layout(hir, emit, src, "variant", &mut types)
}

pub fn resolve_enum_layout(
    hir: &HirData,
    emit: &mut HirEmit,
    enum_id: hir::EnumID,
) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    let data = hir.registry().enum_data(enum_id);
    for variant in data.variants {
        let variant_layout = resolve_variant_layout(hir, emit, data, variant)?;

        size = size.max(variant_layout.size());
        align = align.max(variant_layout.align());
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

fn resolve_aggregate_layout<'hir>(
    hir: &HirData,
    emit: &mut HirEmit,
    src: SourceRange,
    item_kind: &'static str,
    types: &mut dyn Iterator<Item = hir::Type<'hir>>,
) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for ty in types {
        let elem_layout = type_layout(hir, emit, ty, src)?;
        let elem_size = elem_layout.size();

        size = aligned_size(size, elem_layout.align());
        align = align.max(elem_layout.align());

        if let Some(val) = size.checked_add(elem_size) {
            size = val;
        } else {
            err::const_item_size_overflow(emit, src, item_kind, size, elem_size);
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

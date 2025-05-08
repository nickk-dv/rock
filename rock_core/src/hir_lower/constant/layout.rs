use crate::error::ErrorSink;
use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::support::Arena;
use std::collections::HashMap;

pub trait LayoutContext<'hir> {
    fn arena(&mut self) -> &mut Arena<'hir>;
    fn error(&mut self) -> &mut impl ErrorSink;
    fn ptr_size(&self) -> u64;
    fn array_len(&self, len: hir::ArrayStaticLen) -> Result<u64, ()>;
    fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir>;
    fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir>;

    fn enum_layout(&self) -> &HashMap<hir::EnumKey<'hir>, hir::Layout>;
    fn struct_layout(&self) -> &HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>>;
    fn variant_layout(&self) -> &HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>>;

    fn enum_layout_mut(&mut self) -> &mut HashMap<hir::EnumKey<'hir>, hir::Layout>;
    fn struct_layout_mut(&mut self) -> &mut HashMap<hir::StructKey<'hir>, hir::StructLayout<'hir>>;
    fn variant_layout_mut(
        &mut self,
    ) -> &mut HashMap<hir::VariantKey<'hir>, hir::StructLayout<'hir>>;
}

pub fn type_layout<'hir>(
    ctx: &mut impl LayoutContext<'hir>,
    ty: hir::Type<'hir>,
    poly_types: &[hir::Type<'hir>],
    src: SourceRange,
) -> Result<hir::Layout, ()> {
    match ty {
        hir::Type::Error => Err(()),
        hir::Type::Unknown => unreachable!(),
        hir::Type::Char => Ok(hir::Layout::equal(4)),
        hir::Type::Void => Ok(hir::Layout::new(0, 1)),
        hir::Type::Never => Ok(hir::Layout::new(0, 1)),
        hir::Type::UntypedChar => unreachable!(),
        hir::Type::Rawptr => Ok(hir::Layout::equal(ctx.ptr_size())),
        hir::Type::Int(int_ty) => Ok(int_layout(ctx, int_ty)),
        hir::Type::Float(float_ty) => Ok(float_layout(float_ty)),
        hir::Type::Bool(bool_ty) => Ok(bool_layout(bool_ty)),
        hir::Type::String(string_ty) => Ok(string_layout(ctx, string_ty)),
        hir::Type::PolyProc(_, poly_idx) => type_layout(ctx, poly_types[poly_idx], &[], src),
        hir::Type::PolyEnum(_, poly_idx) => type_layout(ctx, poly_types[poly_idx], &[], src),
        hir::Type::PolyStruct(_, poly_idx) => type_layout(ctx, poly_types[poly_idx], &[], src),
        hir::Type::Enum(id, poly_types) => {
            if poly_types.is_empty() {
                ctx.enum_data(id).layout.resolved()
            } else {
                let _ = ctx.enum_data(id).layout.resolved()?; //@hack prevent inifinite recursion
                if let Some(layout) = ctx.enum_layout().get(&(id, poly_types)) {
                    Ok(*layout)
                } else {
                    let layout_res = resolve_enum_layout(ctx, id, poly_types);
                    if let Ok(layout) = layout_res {
                        ctx.enum_layout_mut().insert((id, poly_types), layout);
                    }
                    layout_res
                }
            }
        }
        hir::Type::Struct(id, poly_types) => {
            if poly_types.is_empty() {
                ctx.struct_data(id).layout.resolved()
            } else {
                let _ = ctx.struct_data(id).layout.resolved()?; //@hack prevent inifinite recursion
                if let Some(layout) = ctx.struct_layout().get(&(id, poly_types)) {
                    Ok(layout.total)
                } else {
                    let layout_res = resolve_struct_layout(ctx, id, poly_types);
                    if let Ok(layout) = layout_res {
                        ctx.struct_layout_mut().insert((id, poly_types), layout);
                    }
                    layout_res.map(|l| l.total)
                }
            }
        }
        hir::Type::Reference(_, _) | hir::Type::MultiReference(_, _) | hir::Type::Procedure(_) => {
            Ok(hir::Layout::equal(ctx.ptr_size()))
        }
        hir::Type::ArraySlice(_) => {
            let ptr_size = ctx.ptr_size();
            Ok(hir::Layout::new(2 * ptr_size, ptr_size))
        }
        hir::Type::ArrayStatic(array) => {
            let len = ctx.array_len(array.len)?;
            let elem_layout = type_layout(ctx, array.elem_ty, &[], src)?;
            let elem_size = elem_layout.size;

            if let Some(total) = elem_size.checked_mul(len) {
                Ok(hir::Layout::new(total, elem_layout.align))
            } else {
                err::const_array_size_overflow(ctx.error(), src, elem_size, len);
                Err(())
            }
        }
    }
}

#[inline]
pub fn int_layout<'hir>(ctx: &impl LayoutContext<'hir>, int_ty: hir::IntType) -> hir::Layout {
    match int_ty {
        hir::IntType::S8 | hir::IntType::U8 => hir::Layout::equal(1),
        hir::IntType::S16 | hir::IntType::U16 => hir::Layout::equal(2),
        hir::IntType::S32 | hir::IntType::U32 => hir::Layout::equal(4),
        hir::IntType::S64 | hir::IntType::U64 => hir::Layout::equal(8),
        hir::IntType::Ssize | hir::IntType::Usize => hir::Layout::equal(ctx.ptr_size()),
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
pub fn string_layout<'hir>(
    ctx: &impl LayoutContext<'hir>,
    string_ty: hir::StringType,
) -> hir::Layout {
    let ptr_size = ctx.ptr_size();
    match string_ty {
        hir::StringType::String => hir::Layout::new(2 * ptr_size, ptr_size),
        hir::StringType::CString => hir::Layout::equal(ptr_size),
        hir::StringType::Untyped => unreachable!(),
    }
}

pub fn resolve_struct_layout<'hir>(
    ctx: &mut impl LayoutContext<'hir>,
    struct_id: hir::StructID,
    poly_types: &[hir::Type<'hir>],
) -> Result<hir::StructLayout<'hir>, ()> {
    let data = ctx.struct_data(struct_id);
    let src = SourceRange::new(data.origin_id, data.name.range);

    let types = data.fields.iter().map(|f| f.ty);
    resolve_aggregate_layout(ctx, src, "struct", types, poly_types)
}

fn resolve_variant_layout<'hir>(
    ctx: &mut impl LayoutContext<'hir>,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    poly_types: &[hir::Type<'hir>],
) -> Result<hir::StructLayout<'hir>, ()> {
    let data = ctx.enum_data(enum_id);
    let variant = data.variant(variant_id);
    let src = SourceRange::new(data.origin_id, variant.name.range);

    let tag_ty = [hir::Type::Int(data.tag_ty.resolved()?)];
    let types = tag_ty.into_iter().chain(variant.fields.iter().map(|f| f.ty));
    resolve_aggregate_layout(ctx, src, "variant", types, poly_types)
}

//@use temp buffers, annoying due to possible `?` early return
fn resolve_aggregate_layout<'hir>(
    ctx: &mut impl LayoutContext<'hir>,
    src: SourceRange,
    item_kind: &'static str,
    types: impl Iterator<Item = hir::Type<'hir>> + Clone,
    poly_types: &[hir::Type<'hir>],
) -> Result<hir::StructLayout<'hir>, ()> {
    let field_count = types.clone().count();
    let mut field_pad = Vec::with_capacity(field_count);
    let mut field_offset = Vec::with_capacity(field_count);

    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for (field_idx, ty) in types.enumerate() {
        let layout = type_layout(ctx, ty, poly_types, src)?;
        let aligned = aligned_size(size, layout.align);
        field_offset.push(aligned);

        align = align.max(layout.align);
        if field_idx != 0 {
            field_pad.push((aligned - size) as u8);
        }

        if let Some(total) = aligned.checked_add(layout.size) {
            size = total;
        } else {
            err::const_item_size_overflow(ctx.error(), src, item_kind, aligned, layout.size);
            return Err(());
        }
    }

    let aligned = aligned_size(size, align);
    field_pad.push((aligned - size) as u8);
    size = aligned;
    let total = hir::Layout::new(size, align);

    let field_pad = ctx.arena().alloc_slice(&field_pad);
    let field_offset = ctx.arena().alloc_slice(&field_offset);
    let layout = hir::StructLayout { total, field_pad, field_offset };
    Ok(layout)
}

pub fn resolve_enum_layout<'hir>(
    ctx: &mut impl LayoutContext<'hir>,
    enum_id: hir::EnumID,
    poly_types: &'hir [hir::Type<'hir>],
) -> Result<hir::Layout, ()> {
    let data = ctx.enum_data(enum_id);
    if !data.flag_set.contains(hir::EnumFlag::WithFields) {
        let tag_ty = data.tag_ty.resolved()?;
        return Ok(int_layout(ctx, tag_ty));
    }

    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for variant_id in (0..data.variants.len()).map(hir::VariantID::new) {
        let layout = resolve_variant_layout(ctx, enum_id, variant_id, poly_types)?;
        size = size.max(layout.total.size);
        align = align.max(layout.total.align);

        let key = (enum_id, variant_id, poly_types);
        ctx.variant_layout_mut().insert(key, layout);
    }

    size = aligned_size(size, align);
    Ok(hir::Layout::new(size, align))
}

fn aligned_size(size: u64, align: u64) -> u64 {
    assert!(align != 0);
    assert!(align.is_power_of_two());
    size.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)
}

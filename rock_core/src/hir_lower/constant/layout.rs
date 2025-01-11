use crate::ast::BasicType;
use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::hir_lower::context::HirCtx;

pub fn type_layout(ctx: &mut HirCtx, ty: hir::Type, src: SourceRange) -> Result<hir::Layout, ()> {
    match ty {
        hir::Type::Error => Err(()),
        hir::Type::Any => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::new(ptr_size * 2, ptr_size))
        }
        hir::Type::Char => Ok(hir::Layout::equal(4)),
        hir::Type::Void => Ok(hir::Layout::new(0, 1)),
        hir::Type::Never => Ok(hir::Layout::new(0, 1)),
        hir::Type::Rawptr => Ok(hir::Layout::equal(ctx.session.config.target_ptr_width.ptr_size())),
        hir::Type::Int(int_ty) => match int_ty {
            hir::IntType::S8 | hir::IntType::U8 => Ok(hir::Layout::equal(1)),
            hir::IntType::S16 | hir::IntType::U16 => Ok(hir::Layout::equal(2)),
            hir::IntType::S32 | hir::IntType::U32 => Ok(hir::Layout::equal(4)),
            hir::IntType::S64 | hir::IntType::U64 => Ok(hir::Layout::equal(8)),
            hir::IntType::Ssize | hir::IntType::Usize => {
                Ok(hir::Layout::equal(ctx.session.config.target_ptr_width.ptr_size()))
            }
            hir::IntType::Untyped => unreachable!(),
        },
        hir::Type::Float(float_ty) => match float_ty {
            hir::FloatType::F32 => Ok(hir::Layout::equal(4)),
            hir::FloatType::F64 => Ok(hir::Layout::equal(8)),
            hir::FloatType::Untyped => unreachable!(),
        },
        hir::Type::Bool(bool_ty) => match bool_ty {
            hir::BoolType::Bool => Ok(hir::Layout::equal(1)),
            hir::BoolType::Bool32 => Ok(hir::Layout::equal(4)),
            hir::BoolType::Untyped => unreachable!(),
        },
        hir::Type::String(string_ty) => match string_ty {
            hir::StringType::String => {
                let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
                Ok(hir::Layout::new(ptr_size * 2, ptr_size))
            }
            hir::StringType::CString => {
                Ok(hir::Layout::equal(ctx.session.config.target_ptr_width.ptr_size()))
            }
            hir::StringType::Untyped => unreachable!(),
        },
        hir::Type::InferDef(_, _) => {
            err::internal_not_implemented(&mut ctx.emit, src, "polymorphic param layout");
            Err(())
        }
        hir::Type::Enum(id, poly_types) => {
            if !poly_types.is_empty() {
                err::internal_not_implemented(&mut ctx.emit, src, "polymorphic enum layout");
                return Err(());
            }
            let data = ctx.registry.enum_data(id);
            data.layout.resolved()
        }
        hir::Type::Struct(id, poly_types) => {
            if !poly_types.is_empty() {
                err::internal_not_implemented(&mut ctx.emit, src, "polymorphic struct layout");
                return Err(());
            }
            let data = ctx.registry.struct_data(id);
            data.layout.resolved()
        }
        hir::Type::Reference(_, _) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::equal(ptr_size))
        }
        hir::Type::MultiReference(_, _) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::equal(ptr_size))
        }
        hir::Type::Procedure(_) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::equal(ptr_size))
        }
        hir::Type::ArraySlice(_) => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            Ok(hir::Layout::new(ptr_size * 2, ptr_size))
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

pub fn basic_layout(ctx: &HirCtx, basic: BasicType) -> hir::Layout {
    match basic {
        BasicType::S8 => hir::Layout::equal(1),
        BasicType::S16 => hir::Layout::equal(2),
        BasicType::S32 => hir::Layout::equal(4),
        BasicType::S64 => hir::Layout::equal(8),
        BasicType::Ssize => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::equal(ptr_size)
        }
        BasicType::U8 => hir::Layout::equal(1),
        BasicType::U16 => hir::Layout::equal(2),
        BasicType::U32 => hir::Layout::equal(4),
        BasicType::U64 => hir::Layout::equal(8),
        BasicType::Usize => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::equal(ptr_size)
        }
        BasicType::F32 => hir::Layout::equal(4),
        BasicType::F64 => hir::Layout::equal(8),
        BasicType::Bool => hir::Layout::equal(1),
        BasicType::Bool32 => hir::Layout::equal(4),
        BasicType::Char => hir::Layout::equal(4),
        BasicType::Rawptr => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::equal(ptr_size)
        }
        BasicType::Any => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::new(ptr_size * 2, ptr_size)
        }
        BasicType::Void => hir::Layout::new(0, 1),
        BasicType::Never => hir::Layout::new(0, 1),
        BasicType::String => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::new(ptr_size * 2, ptr_size)
        }
        BasicType::CString => {
            let ptr_size = ctx.session.config.target_ptr_width.ptr_size();
            hir::Layout::equal(ptr_size)
        }
    }
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

    let tag_ty = data.tag_ty.resolved()?;
    let tag_ty = [hir::Type::Int(tag_ty)];
    let mut types = tag_ty.iter().copied().chain(variant.fields.iter().map(|f| &f.ty).copied());
    resolve_aggregate_layout(ctx, src, "variant", &mut types)
}

//@avoid dyn trait if possible
fn resolve_aggregate_layout(
    ctx: &mut HirCtx,
    src: SourceRange,
    item_kind: &'static str,
    types: &mut dyn Iterator<Item = hir::Type>,
) -> Result<hir::Layout, ()> {
    let mut size: u64 = 0;
    let mut align: u64 = 1;

    for ty in types {
        let elem_layout = type_layout(ctx, ty, src)?;
        let elem_size = elem_layout.size;

        size = aligned_size(size, elem_layout.align);
        align = align.max(elem_layout.align);

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

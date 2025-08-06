use super::context;
use super::context::{Codegen, Expect};
use super::emit_mod;
use super::emit_stmt;
use super::llvm;
use rock_core::error::SourceRange;
use rock_core::hir::{self, CmpPred};
use rock_core::hir_lower::layout;
use rock_core::intern::LitID;
use rock_core::support::TempOffset;
use rock_core::text::TextRange;

pub fn codegen_expr_discard<'c>(cg: &mut Codegen<'c, '_, '_>, expr: &hir::Expr<'c>) {
    codegen_expr(cg, expr, Expect::Value(None));
}

pub fn codegen_expr_value<'c>(cg: &mut Codegen<'c, '_, '_>, expr: &hir::Expr<'c>) -> llvm::Value {
    let value_id = cg.proc.add_tail_value();
    if let Some(value) = codegen_expr(cg, expr, Expect::Value(Some(value_id))) {
        value
    } else if let Some(tail) = cg.proc.tail_value(value_id) {
        cg.build.load(tail.value_ty, tail.value_ptr, "tail_val")
    } else {
        unreachable!();
    }
}

pub fn codegen_expr_value_opt<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
) -> Option<llvm::Value> {
    let value_id = cg.proc.add_tail_value();
    if let Some(value) = codegen_expr(cg, expr, Expect::Value(Some(value_id))) {
        Some(value)
    } else if let Some(tail) = cg.proc.tail_value(value_id) {
        Some(cg.build.load(tail.value_ty, tail.value_ptr, "tail_val"))
    } else {
        None
    }
}

pub fn codegen_expr_tail<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, expr: &hir::Expr<'c>) {
    if let Some(value) = codegen_expr(cg, expr, expect) {
        match expect {
            Expect::Value(Some(value_id)) => {
                if let Some(tail) = cg.proc.tail_value(value_id) {
                    cg.build.store(value, tail.value_ptr);
                } else {
                    let value_ty = llvm::typeof_value(value);
                    let value_ptr = cg.entry_alloca(value_ty, "tail_ptr");
                    cg.proc.set_tail_value(value_id, value_ptr, value_ty);
                    cg.build.store(value, value_ptr);
                }
            }
            Expect::Value(None) => {}
            Expect::Pointer(_) => unreachable!(),
            Expect::Store(ptr_val) => {
                let _ = cg.build.store(value, ptr_val);
            }
        }
    }
}

pub fn codegen_expr_pointer<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
) -> llvm::ValuePtr {
    codegen_expr(cg, expr, Expect::Pointer(false)).unwrap().into_ptr()
}

pub fn codegen_expr_pointer_to_expr_value<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
) -> llvm::ValuePtr {
    codegen_expr(cg, expr, Expect::Pointer(true)).unwrap().into_ptr()
}

pub fn codegen_expr_store<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
    ptr_val: llvm::ValuePtr,
) {
    if let Some(value) = codegen_expr(cg, expr, Expect::Store(ptr_val)) {
        cg.build.store(value, ptr_val);
    }
}

pub fn maybe_alloc_copy(cg: &mut Codegen, expect: Expect, value: llvm::Value) -> llvm::Value {
    match expect {
        Expect::Pointer(_) => {
            let temp_ptr = cg.entry_alloca(llvm::typeof_value(value), "copy.temp");
            cg.build.store(value, temp_ptr);
            temp_ptr.as_val()
        }
        Expect::Value(_) => value,
        Expect::Store(_) => value,
    }
}

fn codegen_expr<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
    expect: Expect,
) -> Option<llvm::Value> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const(value, _) => {
            let value = codegen_const(cg, value);
            Some(maybe_alloc_copy(cg, expect, value))
        }
        hir::Expr::If { if_ } => {
            codegen_if(cg, expect, if_);
            None
        }
        hir::Expr::Block { block } => {
            emit_stmt::codegen_block(cg, expect, block);
            None
        }
        hir::Expr::Match { match_ } => {
            codegen_match(cg, expect, match_);
            None
        }
        hir::Expr::StructField { target, access } => {
            Some(codegen_struct_field(cg, expect, target, access))
        }
        hir::Expr::SliceField { target, access } => {
            Some(codegen_slice_field(cg, expect, target, access))
        }
        hir::Expr::Index { target, access } => Some(codegen_index(cg, expect, target, access)),
        hir::Expr::Slice { access, .. } => codegen_expr(cg, &access.op_call, expect),
        hir::Expr::Cast { target, into, kind } => {
            let value = codegen_cast(cg, target, into, kind);
            Some(maybe_alloc_copy(cg, expect, value))
        }
        hir::Expr::ParamVar { param_id } => Some(codegen_param_var(cg, expect, param_id)),
        hir::Expr::Variable { var_id } => Some(codegen_variable(cg, expect, var_id)),
        hir::Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, expect, global_id)),
        hir::Expr::Variant { enum_id, variant_id, input } => {
            codegen_variant(cg, expect, enum_id, variant_id, input.0, input.1)
        }
        hir::Expr::CallDirect { proc_id, input } => {
            codegen_call_direct(cg, expect, proc_id, input, &[])
        }
        hir::Expr::CallDirectPoly { proc_id, input } => {
            let poly_types = context::substitute_types(cg, input.1, cg.proc.poly_types);
            codegen_call_direct(cg, expect, proc_id, input.0, poly_types)
        }
        hir::Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, expect, target, indirect)
        }
        hir::Expr::VariadicArg { arg } => Some(codegen_variadic_arg(cg, arg)),
        hir::Expr::StructInit { struct_id, input } => {
            codegen_struct_init(cg, expect, struct_id, input, &[])
        }
        hir::Expr::StructInitPoly { struct_id, input } => {
            codegen_struct_init(cg, expect, struct_id, input.0, input.1)
        }
        hir::Expr::ArrayInit { array } => codegen_array_init(cg, expect, array),
        hir::Expr::ArrayRepeat { array } => codegen_array_repeat(cg, expect, array),
        hir::Expr::Deref { rhs, ref_ty, .. } => Some(codegen_deref(cg, expect, rhs, *ref_ty)),
        hir::Expr::Address { rhs } => Some(codegen_address(cg, expect, rhs)),
        hir::Expr::Unary { op, rhs } => {
            let value = codegen_unary(cg, op, rhs);
            Some(maybe_alloc_copy(cg, expect, value))
        }
        hir::Expr::Binary { op, lhs, rhs } => {
            let value = codegen_binary(cg, op, lhs, rhs);
            Some(maybe_alloc_copy(cg, expect, value))
        }
        hir::Expr::ArrayBinary { op, array } => codegen_array_binary(cg, expect, op, array),
    }
}

pub fn const_has_variant_with_ptrs(value: hir::ConstValue, in_variant: bool) -> bool {
    match value {
        hir::ConstValue::Void
        | hir::ConstValue::Null
        | hir::ConstValue::Bool { .. }
        | hir::ConstValue::Int { .. }
        | hir::ConstValue::Float { .. }
        | hir::ConstValue::Char { .. } => false,
        hir::ConstValue::String { .. } | hir::ConstValue::Procedure { .. } => in_variant,
        hir::ConstValue::Variant { .. } => false,
        hir::ConstValue::VariantPoly { variant, .. } => {
            variant.values.iter().copied().any(|v| const_has_variant_with_ptrs(v, true))
        }
        hir::ConstValue::Struct { struct_, .. } => {
            struct_.values.iter().copied().any(|v| const_has_variant_with_ptrs(v, in_variant))
        }
        hir::ConstValue::Array { array } => {
            array.values.iter().copied().any(|v| const_has_variant_with_ptrs(v, in_variant))
        }
        hir::ConstValue::ArrayRepeat { array } => {
            const_has_variant_with_ptrs(array.value, in_variant)
        }
        hir::ConstValue::ArrayEmpty { .. } => false,
        hir::ConstValue::GlobalIndex { .. } => in_variant,
        hir::ConstValue::GlobalSlice { .. } => in_variant,
    }
}

#[derive(Copy, Clone)]
struct ConstWriter {
    use_undef: bool,
    start: TempOffset<llvm::Value>,
    current: TempOffset<llvm::Value>,
}

impl ConstWriter {
    fn new(cg: &mut Codegen, use_undef: bool) -> ConstWriter {
        let start = cg.cache.values.start();
        ConstWriter { use_undef, start, current: start }
    }
    fn write_ptr_or_undef(&mut self, cg: &mut Codegen, ptr_or_undef: llvm::Value) {
        self.collect_bytes(cg);
        cg.cache.values.push(ptr_or_undef);
        self.current = cg.cache.values.start();
    }
    fn collect_bytes(&mut self, cg: &mut Codegen) {
        let values = cg.cache.values.view(self.current);
        if !values.is_empty() {
            let array = llvm::const_array(cg.int_type(hir::IntType::U8), values);
            cg.cache.values.pop_view(self.current);
            cg.cache.values.push(array);
        }
    }
}

pub fn const_writer<'c>(cg: &mut Codegen<'c, '_, '_>, value: hir::ConstValue<'c>) -> llvm::Value {
    let mut writer = ConstWriter::new(cg, true);
    write_const(cg, &mut writer, value);
    writer.collect_bytes(cg);

    let values = cg.cache.values.view(writer.start);
    let packed_struct = llvm::const_struct_inline(&cg.context, values, true);
    cg.cache.values.pop_view(writer.start);
    packed_struct
}

fn const_write_array<'c>(cg: &mut Codegen<'c, '_, '_>, value: hir::ConstValue<'c>) -> llvm::Value {
    let mut writer = ConstWriter::new(cg, false);
    write_const(cg, &mut writer, value);

    let bytes = cg.cache.values.view(writer.start);
    let array = llvm::const_array(cg.int_type(hir::IntType::U8), bytes);
    cg.cache.values.pop_view(writer.start);
    array
}

fn write_const<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    writer: &mut ConstWriter,
    value: hir::ConstValue<'c>,
) {
    match value {
        hir::ConstValue::Void => {}
        hir::ConstValue::Null => {
            let ptr_size = cg.session.config.target_ptr_width.ptr_size();
            (0..ptr_size).for_each(|_| cg.cache.values.push(cg.cache.zero_i8));
        }
        hir::ConstValue::Bool { val, bool_ty } => {
            write_const_int(cg, val as u64, false, bool_ty.int_equivalent())
        }
        hir::ConstValue::Int { val, neg, int_ty } => write_const_int(cg, val, neg, int_ty),
        hir::ConstValue::Float { val, float_ty } => {
            let bytes = match float_ty {
                hir::FloatType::F32 => &(val as f32).to_bits().to_le_bytes()[..],
                hir::FloatType::F64 => &val.to_bits().to_le_bytes()[..],
                hir::FloatType::Untyped => unreachable!(),
            };
            let byte_ty = cg.int_type(hir::IntType::U8);
            for b in bytes.iter().copied() {
                let byte_val = llvm::const_int(byte_ty, b as u64, false);
                cg.cache.values.push(byte_val);
            }
        }
        hir::ConstValue::Char { val, .. } => {
            write_const_int(cg, val as u64, false, hir::IntType::U32)
        }
        hir::ConstValue::String { val, string_ty } => {
            let string_ptr = emit_mod::codegen_string_lit(cg, val);
            writer.write_ptr_or_undef(cg, string_ptr.as_val());

            if string_ty == hir::StringType::String {
                let len = cg.session.intern_lit.get(val).len();
                write_const_int(cg, len as u64, false, hir::IntType::Usize);
            }
        }
        hir::ConstValue::Procedure { proc_id, poly_types } => {
            let poly_types = poly_types
                .map(|p| context::substitute_types(cg, p, cg.proc.poly_types))
                .unwrap_or(&[]);
            let proc_ptr = emit_mod::codegen_function(cg, proc_id, poly_types).0.as_val();
            writer.write_ptr_or_undef(cg, proc_ptr);
        }
        hir::ConstValue::Variant { enum_id, variant_id } => {
            write_enum_tag(cg, writer, enum_id, variant_id);
        }
        hir::ConstValue::VariantPoly { enum_id, variant } => {
            let enum_layout = cg.enum_layout((enum_id, variant.poly_types));
            let layout = cg.variant_layout((enum_id, variant.variant_id, variant.poly_types));

            write_enum_tag(cg, writer, enum_id, variant.variant_id);
            let pad = if variant.values.is_empty() {
                enum_layout.size - layout.total.size + layout.field_pad[0] as u64
            } else {
                layout.field_pad[0] as u64
            };
            write_padding(cg, writer, pad);

            for (idx, field) in variant.values.iter().copied().enumerate() {
                write_const(cg, writer, field);
                let pad = if idx + 1 == variant.values.len() {
                    enum_layout.size - layout.total.size + layout.field_pad[idx + 1] as u64
                } else {
                    layout.field_pad[idx + 1] as u64
                };
                write_padding(cg, writer, pad);
            }
        }
        hir::ConstValue::Struct { struct_id, struct_ } => {
            let layout = cg.struct_layout((struct_id, struct_.poly_types));
            for (idx, field) in struct_.values.iter().copied().enumerate() {
                write_const(cg, writer, field);
                write_padding(cg, writer, layout.field_pad[idx] as u64);
            }
        }
        hir::ConstValue::Array { array } => {
            array.values.iter().copied().for_each(|value| write_const(cg, writer, value));
        }
        hir::ConstValue::ArrayRepeat { array } => {
            (0..array.len).for_each(|_| write_const(cg, writer, array.value));
        }
        hir::ConstValue::ArrayEmpty { .. } => {}
        hir::ConstValue::GlobalIndex { global_id, index } => {
            let global = cg.globals[global_id.index()];
            let ptr_ty = if global_id == cg.info.types_id {
                let type_info = cg.ty(hir::Type::Enum(cg.hir.core.type_info, &[]));
                llvm::array_type(type_info, cg.info.types.len() as u64)
            } else {
                global.value_type()
            };
            let ptr = cg.build.gep_inbounds(
                ptr_ty,
                global.as_ptr(),
                &[cg.const_usize(0), cg.const_usize(index)],
                "global.idx",
            );
            writer.write_ptr_or_undef(cg, ptr.as_val());
        }
        hir::ConstValue::GlobalSlice { global_id, index, len } => {
            let global = cg.globals[global_id.index()];
            let ptr_ty = if global_id == cg.info.types_id {
                let type_info = cg.ty(hir::Type::Enum(cg.hir.core.type_info, &[]));
                llvm::array_type(type_info, cg.info.types.len() as u64)
            } else {
                global.value_type()
            };
            let ptr = cg.build.gep_inbounds(
                ptr_ty,
                global.as_ptr(),
                &[cg.const_usize(0), cg.const_usize(index as u64)],
                "global.idx",
            );
            writer.write_ptr_or_undef(cg, ptr.as_val());
            write_const_int(cg, len as u64, false, hir::IntType::Usize);
        }
    }
}

fn write_padding(cg: &mut Codegen, writer: &mut ConstWriter, pad: u64) {
    if pad == 0 {
        return;
    }
    if writer.use_undef {
        let pad = llvm::undef(llvm::array_type(cg.int_type(hir::IntType::U8), pad));
        writer.write_ptr_or_undef(cg, pad);
    } else {
        (0..pad).for_each(|_| cg.cache.values.push(cg.cache.zero_i8));
    }
}

fn write_enum_tag(
    cg: &mut Codegen,
    writer: &mut ConstWriter,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) {
    let data = cg.hir.enum_data(enum_id);
    let tag = match data.variant(variant_id).kind {
        hir::VariantKind::Default(id) => cg.hir.variant_eval_values[id.index()],
        hir::VariantKind::Constant(id) => cg.hir.const_eval_values[id.index()],
    };
    write_const(cg, writer, tag);
}

fn write_const_int(cg: &mut Codegen, val: u64, neg: bool, int_ty: hir::IntType) {
    let ptr_size = cg.session.config.target_ptr_width.ptr_size();
    let sign = if neg { -1 } else { 1 };
    let bytes = match int_ty {
        hir::IntType::S8 => &(val as i8 * sign as i8).to_le_bytes()[..],
        hir::IntType::S16 => &(val as i16 * sign as i16).to_le_bytes()[..],
        hir::IntType::S32 => &(val as i32 * sign).to_le_bytes()[..],
        hir::IntType::S64 => &(val as i64 * sign as i64).to_le_bytes()[..],
        hir::IntType::U8 => &(val as u8).to_le_bytes()[..],
        hir::IntType::U16 => &(val as u16).to_le_bytes()[..],
        hir::IntType::U32 => &(val as u32).to_le_bytes()[..],
        hir::IntType::U64 => &val.to_le_bytes()[..],
        hir::IntType::Ssize => {
            if ptr_size == 8 {
                &(val as i64 * sign as i64).to_le_bytes()[..]
            } else {
                &(val as i32 * sign).to_le_bytes()[..]
            }
        }
        hir::IntType::Usize => {
            if ptr_size == 8 {
                &val.to_le_bytes()[..]
            } else {
                &(val as u32).to_le_bytes()[..]
            }
        }
        hir::IntType::Untyped => unreachable!(),
    };
    let byte_ty = cg.int_type(hir::IntType::U8);
    for b in bytes.iter().copied() {
        let byte_val = llvm::const_int(byte_ty, b as u64, false);
        cg.cache.values.push(byte_val);
    }
}

pub fn codegen_const<'c>(cg: &mut Codegen<'c, '_, '_>, value: hir::ConstValue<'c>) -> llvm::Value {
    match value {
        hir::ConstValue::Void => llvm::const_struct_named(cg.void_val_type(), &[]),
        hir::ConstValue::Null => llvm::const_zeroed(cg.ptr_type()),
        hir::ConstValue::Bool { val, bool_ty } => codegen_const_bool(cg, val, bool_ty),
        hir::ConstValue::Int { val, neg, int_ty } => {
            let ext_val = if neg { (!val).wrapping_add(1) } else { val };
            llvm::const_int(cg.int_type(int_ty), ext_val, int_ty.is_signed())
        }
        hir::ConstValue::Float { val, float_ty } => llvm::const_float(cg.float_type(float_ty), val),
        hir::ConstValue::Char { val, .. } => llvm::const_int(cg.char_type(), val as u64, false),
        hir::ConstValue::String { val, string_ty } => codegen_const_string(cg, val, string_ty),
        hir::ConstValue::Procedure { proc_id, poly_types } => {
            let poly_types = poly_types
                .map(|p| context::substitute_types(cg, p, cg.proc.poly_types))
                .unwrap_or(&[]);
            emit_mod::codegen_function(cg, proc_id, poly_types).0.as_val()
        }
        hir::ConstValue::Variant { enum_id, variant_id } => {
            codegen_const_variant(cg, enum_id, variant_id)
        }
        hir::ConstValue::VariantPoly { enum_id, variant } => {
            let array = const_write_array(cg, value);
            let enum_ty = cg.ty(hir::Type::Enum(enum_id, variant.poly_types)).as_st();
            llvm::const_struct_named(enum_ty, &[array])
        }
        hir::ConstValue::Struct { struct_id, struct_ } => {
            codegen_const_struct(cg, struct_id, struct_)
        }
        hir::ConstValue::Array { array } => codegen_const_array(cg, array),
        hir::ConstValue::ArrayRepeat { array } => codegen_const_array_repeat(cg, array),
        hir::ConstValue::ArrayEmpty { elem_ty } => llvm::const_array(cg.ty(*elem_ty), &[]),
        hir::ConstValue::GlobalIndex { global_id, index } => {
            let global = cg.globals[global_id.index()];
            let ptr_ty = if global_id == cg.info.types_id {
                let type_info = cg.ty(hir::Type::Enum(cg.hir.core.type_info, &[]));
                llvm::array_type(type_info, cg.info.types.len() as u64)
            } else {
                global.value_type()
            };
            cg.build
                .gep_inbounds(
                    ptr_ty,
                    global.as_ptr(),
                    &[cg.const_usize(0), cg.const_usize(index)],
                    "global.idx",
                )
                .as_val()
        }
        hir::ConstValue::GlobalSlice { global_id, index, len } => {
            let global = cg.globals[global_id.index()];
            let ptr_ty = if global_id == cg.info.types_id {
                let type_info = cg.ty(hir::Type::Enum(cg.hir.core.type_info, &[]));
                llvm::array_type(type_info, cg.info.types.len() as u64)
            } else {
                global.value_type()
            };
            let slice_ptr = cg
                .build
                .gep_inbounds(
                    ptr_ty,
                    global.as_ptr(),
                    &[cg.const_usize(0), cg.const_usize(index as u64)],
                    "global.idx",
                )
                .as_val();
            llvm::const_struct_named(cg.slice_type(), &[slice_ptr, cg.const_usize(len as u64)])
        }
    }
}

fn codegen_const_bool(cg: &Codegen, val: bool, bool_ty: hir::BoolType) -> llvm::Value {
    llvm::const_int(cg.bool_type(bool_ty), val as u64, false)
}

fn codegen_const_string(cg: &mut Codegen, val: LitID, string_ty: hir::StringType) -> llvm::Value {
    let string_ptr = emit_mod::codegen_string_lit(cg, val);
    match string_ty {
        hir::StringType::String => {
            let string = cg.session.intern_lit.get(val);
            let slice_len = cg.const_usize(string.len() as u64);
            llvm::const_struct_named(cg.slice_type(), &[string_ptr.as_val(), slice_len])
        }
        hir::StringType::CString => string_ptr.as_val(),
        hir::StringType::Untyped => unreachable!(),
    }
}

fn codegen_const_variant(
    cg: &mut Codegen,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
) -> llvm::Value {
    let data = cg.hir.enum_data(enum_id);
    let tag = match data.variant(variant_id).kind {
        hir::VariantKind::Default(id) => cg.hir.variant_eval_values[id.index()],
        hir::VariantKind::Constant(id) => cg.hir.const_eval_values[id.index()],
    };
    codegen_const(cg, tag)
}

fn codegen_const_struct<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    struct_id: hir::StructID,
    struct_: &hir::ConstStruct<'c>,
) -> llvm::Value {
    let offset = cg.cache.values.start();
    for value in struct_.values {
        let value = codegen_const(cg, *value);
        cg.cache.values.push(value);
    }

    let struct_ty = cg.ty(hir::Type::Struct(struct_id, struct_.poly_types)).as_st();
    let values = cg.cache.values.view(offset);
    let struct_ = llvm::const_struct_named(struct_ty, values);
    cg.cache.values.pop_view(offset);
    struct_
}

fn codegen_const_array<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    array: &hir::ConstArray<'c>,
) -> llvm::Value {
    let offset = cg.cache.values.start();
    for value in array.values {
        let value = codegen_const(cg, *value);
        cg.cache.values.push(value);
    }
    let values = cg.cache.values.view(offset);

    let elem_ty = llvm::typeof_value(values[0]);
    let array = llvm::const_array(elem_ty, values);
    cg.cache.values.pop_view(offset);
    array
}

fn codegen_const_array_repeat<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    array: &hir::ConstArrayRepeat<'c>,
) -> llvm::Value {
    let offset = cg.cache.values.start();
    let value = codegen_const(cg, array.value);
    for _ in 0..array.len {
        cg.cache.values.push(value);
    }
    let values = cg.cache.values.view(offset);

    let elem_ty = llvm::typeof_value(values[0]);
    let array = llvm::const_array(elem_ty, values);
    cg.cache.values.pop_view(offset);
    array
}

fn codegen_if<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, if_: &hir::If<'c>) {
    let exit_bb = cg.append_bb("if_exit");
    let mut branch_bb = cg.build.insert_bb();

    for (idx, branch) in if_.branches.iter().enumerate() {
        cg.build.position_at_end(branch_bb);
        let cond = codegen_expr_value(cg, branch.cond);
        let cond = convert_bool_to_i1(cg, cond);

        let body_bb = cg.append_bb("if_body");
        let last_branch = idx + 1 == if_.branches.len() && if_.else_block.is_none();
        branch_bb = if !last_branch { cg.append_bb("if_branch") } else { exit_bb };

        cg.build.cond_br(cond, body_bb, branch_bb);
        cg.build.position_at_end(body_bb);
        emit_stmt::codegen_block(cg, expect, branch.block);
        cg.build_br_no_term(exit_bb);
    }

    if let Some(block) = if_.else_block {
        cg.build.position_at_end(branch_bb);
        emit_stmt::codegen_block(cg, expect, block);
        cg.build_br_no_term(exit_bb);
    }
    cg.build.position_at_end(exit_bb);
}

fn codegen_match<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, match_: &hir::Match<'c>) {
    let mut enum_poly: &'c [hir::Type<'c>] = &[];
    let (on_value, enum_ptr, bind_by_pointer) = match match_.kind {
        hir::MatchKind::Int { .. }
        | hir::MatchKind::Bool { .. }
        | hir::MatchKind::Char
        | hir::MatchKind::String => (codegen_expr_value(cg, match_.on_expr), None, false),
        hir::MatchKind::Enum { enum_id, ref_mut, poly_types } => {
            let data = cg.hir.enum_data(enum_id);
            let tag_ty = cg.int_type(data.tag_ty.resolved_unwrap());
            let with_fields = data.flag_set.contains(hir::EnumFlag::WithFields);

            if let Some(poly_types) = poly_types {
                enum_poly = context::substitute_types(cg, poly_types, cg.proc.poly_types);
            }

            if with_fields {
                let mut enum_ptr = codegen_expr_pointer_to_expr_value(cg, match_.on_expr);
                if ref_mut.is_some() {
                    enum_ptr = cg.build.load(cg.ptr_type(), enum_ptr, "deref").into_ptr();
                }
                let on_value = cg.build.load(tag_ty, enum_ptr, "enum_tag");
                (on_value, Some(enum_ptr), ref_mut.is_some())
            } else {
                let mut tag_value = codegen_expr_value(cg, match_.on_expr);
                if ref_mut.is_some() {
                    tag_value = cg.build.load(tag_ty, tag_value.into_ptr(), "deref");
                }
                (tag_value, None, false)
            }
        }
    };

    let exit_bb = cg.append_bb("match_exit");
    let insert_bb = cg.build.insert_bb();
    let mut wild_bb = None;
    let offset = cg.cache.cases.start();

    for arm in match_.arms {
        let arm_bb = cg.append_bb("match_arm");
        cg.build.position_at_end(arm_bb);

        match arm.pat {
            hir::Pat::Error => unreachable!(),
            hir::Pat::Wild => wild_bb = Some(arm_bb),
            hir::Pat::Lit(value) => {
                let value = codegen_const(cg, value);
                cg.cache.cases.push((value, arm_bb));
            }
            hir::Pat::Variant(enum_id, variant_id, bind_ids) => {
                let enum_data = cg.hir.enum_data(enum_id);
                let variant = enum_data.variant(variant_id);
                let variant_tag = match variant.kind {
                    hir::VariantKind::Default(id) => cg.hir.variant_eval_values[id.index()],
                    hir::VariantKind::Constant(id) => cg.hir.const_eval_values[id.index()],
                };
                let variant_tag = codegen_const(cg, variant_tag);
                cg.cache.cases.push((variant_tag, arm_bb));

                if !bind_ids.is_empty() {
                    let enum_ty = cg.ty(hir::Type::Enum(enum_id, enum_poly)).as_st();
                    let layout = cg.variant_layout((enum_id, variant_id, enum_poly));

                    for (field_idx, var_id) in bind_ids.iter().copied().enumerate() {
                        if var_id == hir::VariableID::dummy() {
                            continue; //discarded binding
                        }
                        let proc_data = cg.hir.proc_data(cg.proc.proc_id);
                        let bind_var = proc_data.variable(var_id);

                        let offset = cg.const_usize(layout.field_offset[field_idx + 1]);
                        let array_ptr =
                            cg.build.gep_struct(enum_ty, enum_ptr.unwrap(), 0, "enum_bytes_ptr");
                        let field_ptr = cg.build.gep_inbounds(
                            enum_ty.field_ty(0),
                            array_ptr,
                            &[cg.const_usize(0), offset],
                            "variant_field_ptr",
                        );
                        let var_ptr = cg.proc.variable_ptrs[var_id.index()];

                        if bind_by_pointer {
                            cg.build.store(field_ptr.as_val(), var_ptr);
                        } else {
                            let field_ty = cg.ty(bind_var.ty);
                            let field_val = cg.build.load(field_ty, field_ptr, "variant_field");
                            cg.build.store(field_val, var_ptr);
                        }
                    }
                }
            }
            hir::Pat::Or(pats) => {
                for pat in pats.iter().copied() {
                    match pat {
                        hir::Pat::Error | hir::Pat::Or(_) => unreachable!(),
                        hir::Pat::Wild => wild_bb = Some(arm_bb),
                        hir::Pat::Lit(value) => {
                            let value = codegen_const(cg, value);
                            cg.cache.cases.push((value, arm_bb));
                        }
                        hir::Pat::Variant(enum_id, variant_id, _) => {
                            let enum_data = cg.hir.enum_data(enum_id);
                            let variant = enum_data.variant(variant_id);
                            let variant_tag = match variant.kind {
                                hir::VariantKind::Default(id) => {
                                    cg.hir.variant_eval_values[id.index()]
                                }
                                hir::VariantKind::Constant(id) => {
                                    cg.hir.const_eval_values[id.index()]
                                }
                            };
                            let variant_tag = codegen_const(cg, variant_tag);
                            cg.cache.cases.push((variant_tag, arm_bb));
                        }
                    }
                }
            }
        };

        emit_stmt::codegen_block(cg, expect, arm.block);
        cg.build_br_no_term(exit_bb);
    }

    cg.build.position_at_end(insert_bb);
    let else_bb = wild_bb.unwrap_or(exit_bb);

    let cases = cg.cache.cases.view(offset);
    let switch = cg.build.switch(on_value, else_bb, cases.len() as u32);
    for (case_val, dest_bb) in cases.iter().copied() {
        cg.build.add_case(switch, case_val, dest_bb);
    }
    cg.cache.cases.pop_view(offset);

    cg.build.position_at_end(exit_bb);
}

fn codegen_struct_field<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::StructFieldAccess<'c>,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build.load(cg.ptr_type(), target_ptr, "deref_ptr").into_ptr()
    } else {
        target_ptr
    };

    let struct_ty = cg.ty(hir::Type::Struct(access.struct_id, access.poly_types)).as_st();
    let field_ptr = cg.build.gep_struct(struct_ty, target_ptr, access.field_id.raw(), "field_ptr");

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let field_ty = cg.ty(access.field_ty);
            cg.build.load(field_ty, field_ptr, "field_val")
        }
        Expect::Pointer(_) => field_ptr.as_val(),
    }
}

fn codegen_slice_field<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: hir::SliceFieldAccess,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build.load(cg.ptr_type(), target_ptr, "deref_ptr").into_ptr()
    } else {
        target_ptr
    };

    let (idx, field_ty, ptr_name, value_name) = match access.field {
        hir::SliceField::Ptr => (0, cg.ptr_type(), "slice_ptr_ptr", "slice_ptr"),
        hir::SliceField::Len => (1, cg.ptr_sized_int(), "slice_len_ptr", "slice_len"),
    };

    let slice_ty = cg.slice_type();
    let field_ptr = cg.build.gep_struct(slice_ty, target_ptr, idx, ptr_name);

    match expect {
        Expect::Value(_) | Expect::Store(_) => cg.build.load(field_ty, field_ptr, value_name),
        Expect::Pointer(_) => field_ptr.as_val(),
    }
}

fn codegen_index<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::IndexAccess<'c>,
) -> llvm::Value {
    let target_ptr = match access.kind {
        hir::IndexKind::Multi(_) => codegen_expr_value(cg, target).into_ptr(),
        hir::IndexKind::Slice(_)
        | hir::IndexKind::Array(_)
        | hir::IndexKind::ArrayEnum(_)
        | hir::IndexKind::ArrayCore => codegen_expr_pointer(cg, target),
    };
    let target_ptr = if access.deref.is_some() {
        cg.build.load(cg.ptr_type(), target_ptr, "deref_ptr").into_ptr()
    } else {
        target_ptr
    };

    let mut index_val = codegen_expr_value(cg, access.index);
    if let hir::IndexKind::ArrayEnum(_) = access.kind {
        index_val = codegen_cast_op(
            cg,
            index_val,
            cg.int_type(hir::IntType::Usize),
            hir::CastKind::IntU_Extend,
        );
    }

    let bound = match access.kind {
        // never bounds checked:
        hir::IndexKind::Multi(_) => None,
        hir::IndexKind::Slice(_) => {
            let slice_len_ptr =
                cg.build.gep_struct(cg.slice_type(), target_ptr, 1, "slice_len_ptr");
            let len = cg.build.load(cg.ptr_sized_int(), slice_len_ptr, "slice_len");
            Some(len)
        }
        // skip if index is proven to be in bounds:
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            match access.index {
                hir::Expr::Const(hir::ConstValue::Int { val, .. }, _) if *val < len => None,
                _ => Some(cg.const_usize(len)),
            }
        }
        // skip if index is constant, meaning it cannot be corrupted:
        hir::IndexKind::ArrayEnum(enum_id) => {
            let len = cg.hir.enum_data(enum_id).variants.len() as u64;
            match access.index {
                hir::Expr::Const(
                    hir::ConstValue::Variant { .. } | hir::ConstValue::VariantPoly { .. },
                    _,
                ) => None,
                _ => Some(cg.const_usize(len)),
            }
        }
        hir::IndexKind::ArrayCore => {
            let elem_concrete = context::substitute_type(cg, access.elem_ty, &[]);
            let array_ty = *cg.structs_poly.get(&(cg.hir.core.array, &[elem_concrete])).unwrap();
            let len_ptr = cg.build.gep_struct(array_ty, target_ptr, 0, "array.len.ptr");
            Some(cg.build.load(cg.int_type(hir::IntType::Usize), len_ptr, "array.len"))
        }
    };

    if let Some(bound) = bound {
        let check_bb = cg.append_bb("bounds_check_err");
        let exit_bb = cg.append_bb("bounds_check_ok");
        let cond = codegen_binary_op(
            cg,
            hir::BinOp::Cmp_Int(CmpPred::GreaterEq, hir::BoolType::Bool, hir::IntType::Usize),
            index_val,
            bound,
        );
        cg.build.cond_br(cond, check_bb, exit_bb);
        cg.build.position_at_end(check_bb);

        let struct_id = cg.hir.core.source_loc.unwrap();
        let proc_data = cg.hir.proc_data(cg.proc.proc_id);
        let fields = hir::source_location(cg.session, proc_data.origin_id, access.offset);
        let values = cg.hir.arena.alloc_slice(&fields); //borrow checker, forced to allocate in the arena!
        let struct_ = hir::ConstStruct { values, poly_types: &[] };
        let struct_ = cg.hir.arena.alloc(struct_); //borrow checker, forced to allocate in the arena!
        let value = hir::ConstValue::Struct { struct_id, struct_ };
        let loc = codegen_expr_value(cg, &hir::Expr::Const(value, hir::ConstID::dummy()));

        let (fn_val, fn_ty) = emit_mod::codegen_function(cg, cg.hir.core.index_out_of_bounds, &[]);
        let _ = cg.build.call(fn_ty, fn_val, &[index_val, bound, loc], "call_val");
        cg.build.unreachable();
        cg.build.position_at_end(exit_bb);
    }

    let elem_ty = cg.ty(access.elem_ty);
    let elem_ptr = match access.kind {
        hir::IndexKind::Multi(_) => {
            cg.build.gep_inbounds(elem_ty, target_ptr, &[index_val], "multi_elem_ptr")
        }
        hir::IndexKind::Slice(_) => {
            let slice_ptr = cg.build.load(cg.ptr_type(), target_ptr, "slice_ptr").into_ptr();
            cg.build.gep_inbounds(elem_ty, slice_ptr, &[index_val], "slice_elem_ptr")
        }
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            let array_ty = llvm::array_type(elem_ty, len);
            cg.build.gep_inbounds(
                array_ty,
                target_ptr,
                &[cg.const_usize(0), index_val],
                "array_elem_ptr",
            )
        }
        hir::IndexKind::ArrayEnum(enum_id) => {
            let len = cg.hir.enum_data(enum_id).variants.len() as u64;
            let array_ty = llvm::array_type(elem_ty, len);
            cg.build.gep_inbounds(
                array_ty,
                target_ptr,
                &[cg.const_usize(0), index_val],
                "array_elem_ptr",
            )
        }
        hir::IndexKind::ArrayCore => {
            let elem_concrete = context::substitute_type(cg, access.elem_ty, &[]);
            let array_ty = *cg.structs_poly.get(&(cg.hir.core.array, &[elem_concrete])).unwrap();
            let data_ptr = cg.build.gep_struct(array_ty, target_ptr, 2, "array.data.ptr");
            let data = cg.build.load(cg.ptr_type(), data_ptr, "array.data");
            cg.build.gep_inbounds(elem_ty, data.into_ptr(), &[index_val], "array_elem_ptr")
        }
    };

    match expect {
        Expect::Value(_) | Expect::Store(_) => cg.build.load(elem_ty, elem_ptr, "elem_val"),
        Expect::Pointer(_) => elem_ptr.as_val(),
    }
}

fn codegen_cast<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    target: &hir::Expr<'c>,
    into: &hir::Type<'c>,
    kind: hir::CastKind,
) -> llvm::Value {
    let val = codegen_expr_value(cg, target);
    let into = cg.ty(*into);
    codegen_cast_op(cg, val, into, kind)
}

fn codegen_cast_op(
    cg: &mut Codegen,
    val: llvm::Value,
    into: llvm::Type,
    kind: hir::CastKind,
) -> llvm::Value {
    use hir::CastKind;
    use llvm::OpCode;

    match kind {
        CastKind::Error | CastKind::StringUntyped_NoOp => unreachable!(),
        CastKind::Char_NoOp => val,
        CastKind::Rawptr_NoOp => val,

        CastKind::Int_NoOp => val,
        CastKind::Int_Trunc => cg.build.cast(OpCode::LLVMTrunc, val, into, "icast"),
        CastKind::IntS_Extend => cg.build.cast(OpCode::LLVMSExt, val, into, "icast"),
        CastKind::IntU_Extend => cg.build.cast(OpCode::LLVMZExt, val, into, "icast"),
        CastKind::IntS_to_Float => cg.build.cast(OpCode::LLVMSIToFP, val, into, "icast"),
        CastKind::IntU_to_Float => cg.build.cast(OpCode::LLVMUIToFP, val, into, "icast"),

        CastKind::Float_Trunc => cg.build.cast(OpCode::LLVMFPTrunc, val, into, "fcast"),
        CastKind::Float_Extend => cg.build.cast(OpCode::LLVMFPExt, val, into, "fcast"),
        CastKind::Float_to_IntS => cg.build.cast(OpCode::LLVMFPToSI, val, into, "fcast"),
        CastKind::Float_to_IntU => cg.build.cast(OpCode::LLVMFPToUI, val, into, "fcast"),

        CastKind::Bool_Trunc => cg.build.cast(OpCode::LLVMTrunc, val, into, "bcast"),
        CastKind::Bool_Extend => cg.build.cast(OpCode::LLVMZExt, val, into, "bcast"),
        CastKind::Bool_NoOp_to_Int => val,
        CastKind::Bool_Trunc_to_Int => cg.build.cast(OpCode::LLVMTrunc, val, into, "bcast"),
        CastKind::Bool_Extend_to_Int => cg.build.cast(OpCode::LLVMZExt, val, into, "bcast"),

        CastKind::Enum_NoOp_to_Int => val,
        CastKind::Enum_Trunc_to_Int => cg.build.cast(OpCode::LLVMTrunc, val, into, "ecast"),
        CastKind::EnumS_Extend_to_Int => cg.build.cast(OpCode::LLVMSExt, val, into, "ecast"),
        CastKind::EnumU_Extend_to_Int => cg.build.cast(OpCode::LLVMZExt, val, into, "ecast"),
    }
}

fn codegen_param_var(cg: &mut Codegen, expect: Expect, param_id: hir::ParamID) -> llvm::Value {
    let param_ptr = cg.proc.param_ptrs[param_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let param = cg.hir.proc_data(cg.proc.proc_id).param(param_id);
            let ptr_ty = cg.ty(param.ty);
            cg.build.load(ptr_ty, param_ptr, "param_val")
        }
        Expect::Pointer(_) => param_ptr.as_val(),
    }
}

fn codegen_variable(cg: &mut Codegen, expect: Expect, var_id: hir::VariableID) -> llvm::Value {
    let var_ptr = cg.proc.variable_ptrs[var_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let var = cg.hir.proc_data(cg.proc.proc_id).variable(var_id);
            let var_ty = cg.ty(var.ty);
            cg.build.load(var_ty, var_ptr, "var_val")
        }
        Expect::Pointer(_) => var_ptr.as_val(),
    }
}

fn codegen_global_var(cg: &mut Codegen, expect: Expect, global_id: hir::GlobalID) -> llvm::Value {
    let global_ptr = cg.globals[global_id.index()].as_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let data = cg.hir.global_data(global_id);
            let global_ty = cg.ty(data.ty);
            cg.build.load(global_ty, global_ptr, "global_val")
        }
        Expect::Pointer(_) => global_ptr.as_val(),
    }
}

fn codegen_variant<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    input: &[&hir::Expr<'c>],
    poly_types: &'c [hir::Type<'c>],
) -> Option<llvm::Value> {
    let data = cg.hir.enum_data(enum_id);
    let variant = data.variant(variant_id);
    let with_fields = data.flag_set.contains(hir::EnumFlag::WithFields);

    //@generating each time
    let tag = match variant.kind {
        hir::VariantKind::Default(id) => cg.hir.variant_eval_values[id.index()],
        hir::VariantKind::Constant(id) => cg.hir.const_eval_values[id.index()],
    };
    let tag = codegen_const(cg, tag);
    if !with_fields {
        return Some(maybe_alloc_copy(cg, expect, tag));
    }

    let poly_types = context::substitute_types(cg, poly_types, cg.proc.poly_types);
    let enum_ty = cg.ty(hir::Type::Enum(enum_id, poly_types)).as_st();
    let layout = cg.variant_layout((enum_id, variant_id, poly_types));

    let enum_ptr = cg.entry_alloca(enum_ty.as_ty(), "enum_init");
    let array_ptr = cg.build.gep_struct(enum_ty, enum_ptr, 0, "enum_bytes_ptr");
    cg.build.store(tag, enum_ptr);

    for (idx, expr) in input.iter().copied().enumerate() {
        let offset = cg.const_usize(layout.field_offset[idx + 1]);
        let field_ptr = cg.build.gep_inbounds(
            enum_ty.field_ty(0),
            array_ptr,
            &[cg.const_usize(0), offset],
            "variant_field_ptr",
        );
        codegen_expr_store(cg, expr, field_ptr);
    }

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            Some(cg.build.load(enum_ty.as_ty(), enum_ptr, "enum_value"))
        }
        Expect::Pointer(_) => Some(enum_ptr.as_val()),
    }
}

//@ret in function can cause type mismatch on ir level
//example: i64 vs Handle.{ ptr }, external and normal fn confilict
pub fn codegen_call_direct<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    proc_id: hir::ProcID,
    input: &[&hir::Expr<'c>],
    poly_types: &'c [hir::Type<'c>],
) -> Option<llvm::Value> {
    let data: &hir::ProcData<'_> = cg.hir.proc_data(proc_id);
    let data_return_ty = data.return_ty;
    if data.flag_set.contains(hir::ProcFlag::Intrinsic) {
        let ret_val = codegen_call_intrinsic(cg, proc_id, input, poly_types);
        return ret_val.map(|value| maybe_alloc_copy(cg, expect, value));
    }

    let is_external = data.flag_set.contains(hir::ProcFlag::External);
    let offset = cg.cache.values.start();
    let mut ret_ptr = None;

    if is_external && emit_mod::win_x64_parameter_type(cg, data_return_ty).by_pointer {
        let ptr = match expect {
            Expect::Value(_) | Expect::Pointer(_) => {
                let alloc_ty = cg.ty(data_return_ty);
                cg.entry_alloca(alloc_ty, "c_call_sret")
            }
            Expect::Store(store_ptr) => store_ptr,
        };
        ret_ptr = Some(ptr);
        cg.cache.values.push(ptr.as_val());
    }

    for (idx, expr) in input.iter().copied().enumerate() {
        //@follow same param passing rules for c_variadics, no types available.
        let data = cg.hir.proc_data(proc_id);
        if idx >= data.params.len() {
            let value = codegen_expr_value(cg, expr);
            cg.cache.values.push(value);
            continue;
        }

        let param = data.param(hir::ParamID::new(idx));
        let value = if is_external {
            let abi = emit_mod::win_x64_parameter_type(cg, param.ty);
            if abi.by_pointer {
                //@copy by caller to prevent mutation, check correctness.
                let alloc_ty = cg.ty(param.ty);
                let copy_ptr = cg.entry_alloca(alloc_ty, "c_call_copy");
                codegen_expr_store(cg, expr, copy_ptr);
                copy_ptr.as_val()
            } else if let hir::Type::Struct(_, _)
            | hir::Type::ArrayStatic(_)
            | hir::Type::ArrayEnumerated(_) = param.ty
            {
                let value_ptr = codegen_expr_pointer(cg, expr);
                cg.build.load(abi.pass_ty, value_ptr, "c_call_register")
            } else {
                codegen_expr_value(cg, expr)
            }
        } else {
            codegen_expr_value(cg, expr)
        };

        cg.cache.values.push(value);
    }

    let (fn_val, fn_ty) = emit_mod::codegen_function(cg, proc_id, poly_types);

    let input_values = cg.cache.values.view(offset);
    let ret_val = cg.build.call(fn_ty, fn_val, input_values, "call_val");
    cg.cache.values.pop_view(offset);

    let data = cg.hir.proc_data(proc_id);
    if data.return_ty.is_never() {
        cg.build.unreachable();
    }

    if let Some(ptr) = ret_ptr {
        return match expect {
            Expect::Pointer(_) => Some(ptr.as_val()),
            _ => {
                let ptr_ty = cg.ty(data.return_ty);
                Some(cg.build.load(ptr_ty, ptr, "c_call_ptr_ret_val"))
            }
        };
    }
    ret_val.map(|value| maybe_alloc_copy(cg, expect, value))
}

//@handle ABI for indirect calls aswell.
//@set correct calling conv. does llvm fn_ty have call conv?
// add calling conv to proc pointer type?
fn codegen_call_indirect<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    indirect: &hir::CallIndirect<'c>,
) -> Option<llvm::Value> {
    let fn_val = codegen_expr_value(cg, target).into_fn();
    let proc_ty = indirect.proc_ty;
    let is_external = proc_ty.flag_set.contains(hir::ProcFlag::External);

    let offset = cg.cache.values.start();
    let mut ret_ptr = None;

    if is_external && emit_mod::win_x64_parameter_type(cg, proc_ty.return_ty).by_pointer {
        let ptr = match expect {
            Expect::Value(_) | Expect::Pointer(_) => {
                let alloc_ty = cg.ty(proc_ty.return_ty);
                cg.entry_alloca(alloc_ty, "c_call_sret")
            }
            Expect::Store(store_ptr) => store_ptr,
        };
        ret_ptr = Some(ptr);
        cg.cache.values.push(ptr.as_val());
    }

    for (idx, expr) in indirect.input.iter().copied().enumerate() {
        //@follow same param passing rules for c_variadics, no types available.
        if idx >= proc_ty.params.len() {
            let value = codegen_expr_value(cg, expr);
            cg.cache.values.push(value);
            continue;
        }

        let param = &proc_ty.params[idx];
        let value = if is_external {
            let abi = emit_mod::win_x64_parameter_type(cg, param.ty);
            if abi.by_pointer {
                //@copy by caller to prevent mutation, check correctness.
                let alloc_ty = cg.ty(proc_ty.return_ty);
                let copy_ptr = cg.entry_alloca(alloc_ty, "c_call_copy");
                codegen_expr_store(cg, expr, copy_ptr);
                copy_ptr.as_val()
            } else if let hir::Type::Struct(_, _)
            | hir::Type::ArrayStatic(_)
            | hir::Type::ArrayEnumerated(_) = param.ty
            {
                let value_ptr = codegen_expr_pointer(cg, expr);
                cg.build.load(abi.pass_ty, value_ptr, "c_call_register")
            } else {
                codegen_expr_value(cg, expr)
            }
        } else {
            codegen_expr_value(cg, expr)
        };

        cg.cache.values.push(value);
    }

    let fn_ty = cg.proc_type(proc_ty);
    let input_values = cg.cache.values.view(offset);
    let ret_val = cg.build.call(fn_ty, fn_val, input_values, "icall_val");
    cg.cache.values.pop_view(offset);

    if proc_ty.return_ty.is_never() {
        cg.build.unreachable();
    }

    if let Some(ptr) = ret_ptr {
        return match expect {
            Expect::Pointer(_) => Some(ptr.as_val()),
            _ => {
                let ptr_ty = cg.ty(proc_ty.return_ty);
                Some(cg.build.load(ptr_ty, ptr, "c_call_ret_val"))
            }
        };
    }
    ret_val.map(|value| maybe_alloc_copy(cg, expect, value))
}

pub fn codegen_call_intrinsic<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    proc_id: hir::ProcID,
    input: &[&hir::Expr<'c>],
    poly_types: &'c [hir::Type<'c>],
) -> Option<llvm::Value> {
    const RELAXED: llvm::Ordering = llvm::Ordering::LLVMAtomicOrderingMonotonic;
    const ACQUIRE: llvm::Ordering = llvm::Ordering::LLVMAtomicOrderingAcquire;
    const RELEASE: llvm::Ordering = llvm::Ordering::LLVMAtomicOrderingRelease;
    const ACQREL: llvm::Ordering = llvm::Ordering::LLVMAtomicOrderingAcquireRelease;
    const SEQCST: llvm::Ordering = llvm::Ordering::LLVMAtomicOrderingSequentiallyConsistent;

    let ty = if let Some(ty) = poly_types.get(0) { *ty } else { hir::Type::Error };
    let data = cg.hir.proc_data(proc_id);

    match cg.session.intern_name.get(data.name.id) {
        "size_of" => {
            let origin = cg.hir.proc_data(cg.proc.proc_id).origin_id;
            let src = SourceRange::new(origin, TextRange::zero());
            let layout_res = layout::type_layout(cg, poly_types[0], cg.proc.poly_types, src);
            let size = layout_res.map(|l| l.size).unwrap_or(0);
            Some(llvm::const_int(cg.ptr_sized_int(), size, false))
        }
        "align_of" => {
            let origin = cg.hir.proc_data(cg.proc.proc_id).origin_id;
            let src = SourceRange::new(origin, TextRange::zero());
            let layout_res = layout::type_layout(cg, poly_types[0], cg.proc.poly_types, src);
            let align = layout_res.map(|l| l.align).unwrap_or(1);
            Some(llvm::const_int(cg.ptr_sized_int(), align, false))
        }
        "transmute" => {
            let value = codegen_expr_value(cg, input[0]);
            let from = cg.ty(poly_types[0]);
            let into = cg.ty(poly_types[1]);
            if llvm::type_equals(from, into) {
                return Some(value);
            }
            let from_kind = llvm::type_kind(from);
            let into_kind = llvm::type_kind(into);

            if is_numeric(from_kind) && is_numeric(into_kind) {
                return Some(cg.build.bitcast(value, into, "transmute.bitcast"));
            }
            if from_kind == llvm::TypeKind::LLVMPointerTypeKind
                && into_kind == llvm::TypeKind::LLVMIntegerTypeKind
            {
                return Some(cg.build.ptr_to_int(value, into, "transmute.ptr_to_int"));
            }
            if from_kind == llvm::TypeKind::LLVMIntegerTypeKind
                && into_kind == llvm::TypeKind::LLVMPointerTypeKind
            {
                return Some(cg.build.int_to_ptr(value, into, "transmute.int_to_ptr"));
            }

            let copy_ptr = cg.entry_alloca(from, "transmute.copy");
            cg.build.store(value, copy_ptr);
            Some(cg.build.load(into, copy_ptr, "transmute.into"))
        }
        "from_raw_parts" => {
            let ptr = codegen_expr_value(cg, input[0]);
            let len = codegen_expr_value(cg, input[1]);
            Some(codegen_from_raw_parts(cg, ptr, len))
        }
        "load_relaxed" => atomic_load(cg, input, poly_types, RELAXED),
        "load_acquire" => atomic_load(cg, input, poly_types, ACQUIRE),
        "load_seqcst" => atomic_load(cg, input, poly_types, SEQCST),

        "store_relaxed" => atomic_store(cg, input, RELAXED),
        "store_release" => atomic_store(cg, input, RELEASE),
        "store_seqcst" => atomic_store(cg, input, SEQCST),

        "add_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAdd, RELAXED),
        "add_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAdd, ACQUIRE),
        "add_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAdd, RELEASE),
        "add_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAdd, ACQREL),
        "add_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAdd, SEQCST),

        "sub_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpSub, RELAXED),
        "sub_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpSub, ACQUIRE),
        "sub_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpSub, RELEASE),
        "sub_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpSub, ACQREL),
        "sub_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpSub, SEQCST),

        "and_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAnd, RELAXED),
        "and_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAnd, ACQUIRE),
        "and_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAnd, RELEASE),
        "and_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAnd, ACQREL),
        "and_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpAnd, SEQCST),

        "nand_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpNand, RELAXED),
        "nand_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpNand, ACQUIRE),
        "nand_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpNand, RELEASE),
        "nand_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpNand, ACQREL),
        "nand_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpNand, SEQCST),

        "or_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpOr, RELAXED),
        "or_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpOr, ACQUIRE),
        "or_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpOr, RELEASE),
        "or_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpOr, ACQREL),
        "or_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpOr, SEQCST),

        "xor_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXor, RELAXED),
        "xor_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXor, ACQUIRE),
        "xor_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXor, RELEASE),
        "xor_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXor, ACQREL),
        "xor_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXor, SEQCST),

        "exchange_relaxed" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXchg, RELAXED),
        "exchange_acquire" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXchg, ACQUIRE),
        "exchange_release" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXchg, RELEASE),
        "exchange_acqrel" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXchg, ACQREL),
        "exchange_seqcst" => rmw(cg, input, llvm::RMWBinOp::LLVMAtomicRMWBinOpXchg, SEQCST),

        "compare_exchange_relaxed_relaxed" => cmpxchg(cg, input, ty, RELAXED, RELAXED, false),
        "compare_exchange_relaxed_acquire" => cmpxchg(cg, input, ty, RELAXED, ACQUIRE, false),
        "compare_exchange_relaxed_seqcst" => cmpxchg(cg, input, ty, RELAXED, SEQCST, false),
        "compare_exchange_relaxed_relaxed_weak" => cmpxchg(cg, input, ty, RELAXED, RELAXED, true),
        "compare_exchange_relaxed_acquire_weak" => cmpxchg(cg, input, ty, RELAXED, ACQUIRE, true),
        "compare_exchange_relaxed_seqcst_weak" => cmpxchg(cg, input, ty, RELAXED, SEQCST, true),

        "compare_exchange_acquire_relaxed" => cmpxchg(cg, input, ty, ACQUIRE, RELAXED, false),
        "compare_exchange_acquire_acquire" => cmpxchg(cg, input, ty, ACQUIRE, ACQUIRE, false),
        "compare_exchange_acquire_seqcst" => cmpxchg(cg, input, ty, ACQUIRE, SEQCST, false),
        "compare_exchange_acquire_relaxed_weak" => cmpxchg(cg, input, ty, ACQUIRE, RELAXED, true),
        "compare_exchange_acquire_acquire_weak" => cmpxchg(cg, input, ty, ACQUIRE, ACQUIRE, true),
        "compare_exchange_acquire_seqcst_weak" => cmpxchg(cg, input, ty, ACQUIRE, SEQCST, true),

        "compare_exchange_release_relaxed" => cmpxchg(cg, input, ty, RELEASE, RELAXED, false),
        "compare_exchange_release_acquire" => cmpxchg(cg, input, ty, RELEASE, ACQUIRE, false),
        "compare_exchange_release_seqcst" => cmpxchg(cg, input, ty, RELEASE, SEQCST, false),
        "compare_exchange_release_relaxed_weak" => cmpxchg(cg, input, ty, RELEASE, RELAXED, true),
        "compare_exchange_release_acquire_weak" => cmpxchg(cg, input, ty, RELEASE, ACQUIRE, true),
        "compare_exchange_release_seqcst_weak" => cmpxchg(cg, input, ty, RELEASE, SEQCST, true),

        "compare_exchange_acqrel_relaxed" => cmpxchg(cg, input, ty, ACQREL, RELAXED, false),
        "compare_exchange_acqrel_acquire" => cmpxchg(cg, input, ty, ACQREL, ACQUIRE, false),
        "compare_exchange_acqrel_seqcst" => cmpxchg(cg, input, ty, ACQREL, SEQCST, false),
        "compare_exchange_acqrel_relaxed_weak" => cmpxchg(cg, input, ty, ACQREL, RELAXED, true),
        "compare_exchange_acqrel_acquire_weak" => cmpxchg(cg, input, ty, ACQREL, ACQUIRE, true),
        "compare_exchange_acqrel_seqcst_weak" => cmpxchg(cg, input, ty, ACQREL, SEQCST, true),

        "compare_exchange_seqcst_relaxed" => cmpxchg(cg, input, ty, SEQCST, RELAXED, false),
        "compare_exchange_seqcst_acquire" => cmpxchg(cg, input, ty, SEQCST, ACQUIRE, false),
        "compare_exchange_seqcst_seqcst" => cmpxchg(cg, input, ty, SEQCST, SEQCST, false),
        "compare_exchange_seqcst_relaxed_weak" => cmpxchg(cg, input, ty, SEQCST, RELAXED, true),
        "compare_exchange_seqcst_acquire_weak" => cmpxchg(cg, input, ty, SEQCST, ACQUIRE, true),
        "compare_exchange_seqcst_seqcst_weak" => cmpxchg(cg, input, ty, SEQCST, SEQCST, true),
        _ => unreachable!(),
    }
}

fn codegen_from_raw_parts(cg: &mut Codegen, ptr: llvm::Value, len: llvm::Value) -> llvm::Value {
    let undef = llvm::undef(cg.slice_type().as_ty());
    let first = cg.build.insert_value(undef, ptr, 0, "raw_parts.ptr");
    cg.build.insert_value(first, len, 1, "raw_parts.ptr_len")
}

fn atomic_load<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    input: &[&hir::Expr<'c>],
    poly_types: &'c [hir::Type<'c>],
    order: llvm::Ordering,
) -> Option<llvm::Value> {
    let ptr_ty = cg.ty(poly_types[0]);
    let src = codegen_expr_value(cg, input[0]).into_ptr();

    let inst = cg.build.load(ptr_ty, src, "atomic_load");
    cg.build.set_ordering(inst.as_inst(), order);
    Some(inst)
}

fn atomic_store<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    input: &[&hir::Expr<'c>],
    order: llvm::Ordering,
) -> Option<llvm::Value> {
    let dst = codegen_expr_value(cg, input[0]).into_ptr();
    let value = codegen_expr_value(cg, input[1]);

    let inst = cg.build.store(value, dst);
    cg.build.set_ordering(inst, order);
    None
}

fn rmw<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    input: &[&hir::Expr<'c>],
    op: llvm::RMWBinOp,
    order: llvm::Ordering,
) -> Option<llvm::Value> {
    let dst = codegen_expr_value(cg, input[0]).into_ptr();
    let rhs = codegen_expr_value(cg, input[1]);
    Some(cg.build.atomic_rmw(op, dst, rhs, order))
}

fn cmpxchg<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    input: &[&hir::Expr<'c>],
    ty: hir::Type<'c>,
    success: llvm::Ordering,
    failure: llvm::Ordering,
    weak: bool,
) -> Option<llvm::Value> {
    let dst = codegen_expr_value(cg, input[0]).into_ptr();
    let expect = codegen_expr_value(cg, input[1]);
    let new = codegen_expr_value(cg, input[2]);

    let inst = cg.build.atomic_cmp_xchg(dst, expect, new, success, failure);
    if weak {
        cg.build.set_weak(inst, true);
    }
    let value = cg.build.extract_value(inst, 0, "xchg_value");
    let ok = cg.build.extract_value(inst, 1, "xchg_ok");

    //@hack, bad
    let poly = cg.hir.arena.alloc_slice(&[ty]);
    let struct_ty = cg.ty(hir::Type::Struct(cg.hir.core.exchange_res, poly));

    let undef = llvm::undef(struct_ty);
    let with_value = cg.build.insert_value(undef, value, 0, "xchg_value");
    let with_value_ok = cg.build.insert_value(with_value, ok, 1, "xchg_value_ok");
    Some(with_value_ok)
}

fn is_numeric(kind: llvm::TypeKind) -> bool {
    matches!(
        kind,
        llvm::TypeKind::LLVMIntegerTypeKind
            | llvm::TypeKind::LLVMFloatTypeKind
            | llvm::TypeKind::LLVMDoubleTypeKind
    )
}

fn codegen_variadic_arg<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    arg: &hir::VariadicArg<'c>,
) -> llvm::Value {
    let any_ty = cg.structs[cg.hir.core.any.unwrap().index()];
    let array_ty = llvm::array_type(any_ty.as_ty(), arg.exprs.len() as u64);
    let array_ptr = cg.entry_alloca(array_ty, "any_array");

    let arg_types = context::substitute_types(cg, arg.types, &[]);
    cg.info.var_args.push((array_ptr.as_val(), arg_types));

    for (idx, expr) in arg.exprs.iter().copied().enumerate() {
        let array_gep = cg.build.gep_inbounds(
            array_ty,
            array_ptr,
            &[cg.const_usize(0), cg.const_usize(idx as u64)],
            "any_array.idx",
        );
        let data_ptr = cg.build.gep_struct(any_ty, array_gep, 0, "any.data");
        let value_ptr = codegen_expr_pointer_to_expr_value(cg, expr);
        cg.build.store(value_ptr.as_val(), data_ptr);
    }

    let array_len = cg.const_usize(arg.exprs.len() as u64);
    codegen_from_raw_parts(cg, array_ptr.as_val(), array_len)
}

fn codegen_struct_init<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    struct_id: hir::StructID,
    input: &[&'c hir::Expr<'c>],
    poly_types: &'c [hir::Type<'c>],
) -> Option<llvm::Value> {
    let struct_ty = cg.ty(hir::Type::Struct(struct_id, poly_types)).as_st();
    let struct_ptr = cg.entry_alloca(struct_ty.as_ty(), "struct_init");

    for (idx, expr) in input.iter().copied().enumerate() {
        let field_ptr = cg.build.gep_struct(struct_ty, struct_ptr, idx as u32, "field_ptr");
        codegen_expr_store(cg, expr, field_ptr);
    }

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            Some(cg.build.load(struct_ty.as_ty(), struct_ptr, "struct_val"))
        }
        Expect::Pointer(_) => Some(struct_ptr.as_val()),
    }
}

fn codegen_array_init<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    array: &hir::ArrayInit<'c>,
) -> Option<llvm::Value> {
    let elem_ty = cg.ty(array.elem_ty);
    let array_ty = llvm::array_type(elem_ty, array.input.len() as u64);
    let array_ptr = cg.entry_alloca(array_ty, "array_init");

    let mut indices = [cg.const_usize(0), cg.const_usize(0)];
    for (idx, expr) in array.input.iter().copied().enumerate() {
        indices[1] = cg.const_usize(idx as u64);
        let elem_ptr = cg.build.gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
        codegen_expr_store(cg, expr, elem_ptr);
    }

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            Some(cg.build.load(array_ty, array_ptr, "array_val"))
        }
        Expect::Pointer(_) => Some(array_ptr.as_val()),
    }
}

fn codegen_array_repeat<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    array: &hir::ArrayRepeat<'c>,
) -> Option<llvm::Value> {
    let elem_ty = cg.ty(array.elem_ty);
    let array_ty = llvm::array_type(elem_ty, array.len);
    let array_ptr = match expect {
        Expect::Value(_) | Expect::Pointer(_) => cg.entry_alloca(array_ty, "array_repeat"),
        Expect::Store(ptr_val) => ptr_val,
    };

    let copied_val = codegen_expr_value(cg, array.value);
    let count_ptr = cg.entry_alloca(cg.ptr_sized_int(), "rep_count");
    cg.build.store(cg.const_usize(0), count_ptr);

    let entry_bb = cg.append_bb("rep_entry");
    let body_bb = cg.append_bb("rep_body");
    let exit_bb = cg.append_bb("rep_exit");

    cg.build.br(entry_bb);
    cg.build.position_at_end(entry_bb);
    let count_val = cg.build.load(cg.ptr_sized_int(), count_ptr, "rep_val");
    let repeat_val = cg.const_usize(array.len);
    let cond = codegen_binary_op(
        cg,
        hir::BinOp::Cmp_Int(CmpPred::Less, hir::BoolType::Bool, hir::IntType::Usize),
        count_val,
        repeat_val,
    );
    cg.build.cond_br(cond, body_bb, exit_bb);

    cg.build.position_at_end(body_bb);
    let indices = [cg.const_usize(0), count_val];
    let elem_ptr = cg.build.gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
    cg.build.store(copied_val, elem_ptr);

    let count_inc = codegen_binary_op(
        cg,
        hir::BinOp::Add_Int(hir::IntType::Usize),
        count_val,
        cg.const_usize(1),
    );
    cg.build.store(count_inc, count_ptr);
    cg.build.br(entry_bb);
    cg.build.position_at_end(exit_bb);

    match expect {
        Expect::Value(_) => Some(cg.build.load(array_ty, array_ptr, "array_val")),
        Expect::Pointer(_) => Some(array_ptr.as_val()),
        Expect::Store(_) => None,
    }
}

fn codegen_deref<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    rhs: &hir::Expr<'c>,
    ref_ty: hir::Type<'c>,
) -> llvm::Value {
    let ptr_val = codegen_expr_value(cg, rhs).into_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let ptr_ty = cg.ty(ref_ty);
            cg.build.load(ptr_ty, ptr_val, "deref_val")
        }
        Expect::Pointer(_) => ptr_val.as_val(),
    }
}

fn codegen_address<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let address = codegen_expr_pointer(cg, rhs).as_val();
    match expect {
        Expect::Pointer(true) => maybe_alloc_copy(cg, expect, address),
        _ => address,
    }
}

fn codegen_unary<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    op: hir::UnOp,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let rhs = codegen_expr_value(cg, rhs);
    match op {
        hir::UnOp::Neg_Int(_) => cg.build.neg(rhs, "un"),
        hir::UnOp::Neg_Float(_) => cg.build.fneg(rhs, "un"),
        hir::UnOp::BitNot(_) => cg.build.not(rhs, "un"),
        hir::UnOp::LogicNot(_) => {
            let int_ty = llvm::typeof_value(rhs);
            let one = llvm::const_int(int_ty, 1, false);
            cg.build.bin_op(llvm::OpCode::LLVMXor, rhs, one, "un")
        }
    }
}

fn codegen_binary<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    op: hir::BinOp,
    lhs: &hir::Expr<'c>,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let lhs = codegen_expr_value(cg, lhs);
    match op {
        hir::BinOp::LogicAnd(bool_ty) => codegen_binary_circuit(cg, op, lhs, rhs, false, bool_ty),
        hir::BinOp::LogicOr(bool_ty) => codegen_binary_circuit(cg, op, lhs, rhs, true, bool_ty),
        _ => {
            let rhs = codegen_expr_value(cg, rhs);
            codegen_binary_op(cg, op, lhs, rhs)
        }
    }
}

fn codegen_array_binary<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    op: hir::BinOp,
    array: &hir::ArrayBinary<'c>,
) -> Option<llvm::Value> {
    let lhs_ptr = codegen_expr_pointer(cg, array.lhs);
    let rhs_ptr = codegen_expr_pointer(cg, array.rhs);
    codegen_array_binary_op(cg, expect, op, array, lhs_ptr, rhs_ptr)
}

pub fn codegen_array_binary_op<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    op: hir::BinOp,
    array: &hir::ArrayBinary<'c>,
    lhs_ptr: llvm::ValuePtr,
    rhs_ptr: llvm::ValuePtr,
) -> Option<llvm::Value> {
    let elem_ty = bin_op_operand_type(cg, op);
    let array_ty = llvm::array_type(elem_ty, array.len);
    let array_ptr = cg.entry_alloca(array_ty, "array_binary");

    let count_ptr = cg.entry_alloca(cg.ptr_sized_int(), "rep_count");
    cg.build.store(cg.const_usize(0), count_ptr);

    let entry_bb = cg.append_bb("rep_entry");
    let body_bb = cg.append_bb("rep_body");
    let exit_bb = cg.append_bb("rep_exit");

    cg.build.br(entry_bb);
    cg.build.position_at_end(entry_bb);
    let count_val = cg.build.load(cg.ptr_sized_int(), count_ptr, "rep_val");
    let repeat_val = cg.const_usize(array.len);
    let cond = codegen_binary_op(
        cg,
        hir::BinOp::Cmp_Int(CmpPred::Less, hir::BoolType::Bool, hir::IntType::Usize),
        count_val,
        repeat_val,
    );
    cg.build.cond_br(cond, body_bb, exit_bb);

    cg.build.position_at_end(body_bb);
    let indices = [cg.const_usize(0), count_val];
    let lhs_elem_ptr = cg.build.gep_inbounds(array_ty, lhs_ptr, &indices, "lhs_elem_ptr");
    let rhs_elem_ptr = cg.build.gep_inbounds(array_ty, rhs_ptr, &indices, "rhs_elem_ptr");
    let lhs = cg.build.load(elem_ty, lhs_elem_ptr, "lhs_val");
    let rhs = cg.build.load(elem_ty, rhs_elem_ptr, "rhs_val");
    let bin = codegen_binary_op(cg, op, lhs, rhs);
    let elem_ptr = cg.build.gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
    cg.build.store(bin, elem_ptr);

    let count_inc = codegen_binary_op(
        cg,
        hir::BinOp::Add_Int(hir::IntType::Usize),
        count_val,
        cg.const_usize(1),
    );
    cg.build.store(count_inc, count_ptr);
    cg.build.br(entry_bb);
    cg.build.position_at_end(exit_bb);

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            Some(cg.build.load(array_ty, array_ptr, "array_val"))
        }
        Expect::Pointer(_) => Some(array_ptr.as_val()),
    }
}

fn codegen_binary_circuit<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    op: hir::BinOp,
    lhs: llvm::Value,
    rhs: &hir::Expr<'c>,
    exit_val: bool,
    bool_ty: hir::BoolType,
) -> llvm::Value {
    let value_name = if exit_val { "logic_or" } else { "logic_and" };
    let next_name = if exit_val { "or_next" } else { "and_next" };
    let exit_name = if exit_val { "or_exit" } else { "and_exit" };

    let start_bb = cg.build.insert_bb();
    let next_bb = cg.append_bb(next_name);
    let exit_bb = cg.append_bb(exit_name);

    let lhs_cond = convert_bool_to_i1(cg, lhs);
    let then_bb = if exit_val { exit_bb } else { next_bb };
    let else_bb = if exit_val { next_bb } else { exit_bb };
    cg.build.cond_br(lhs_cond, then_bb, else_bb);

    cg.build.position_at_end(next_bb);
    let rhs = codegen_expr_value(cg, rhs);
    let bin_val = codegen_binary_op(cg, op, lhs, rhs);
    let bin_val_bb = cg.build.insert_bb();

    cg.build.br(exit_bb);
    cg.build.position_at_end(exit_bb);

    let phi = cg.build.phi(cg.bool_type(bool_ty), value_name);
    let values = [bin_val, codegen_const_bool(cg, exit_val, bool_ty)];
    let blocks = [bin_val_bb, start_bb];
    cg.build.phi_add_incoming(phi, &values, &blocks);
    phi
}

pub fn codegen_binary_op(
    cg: &mut Codegen,
    op: hir::BinOp,
    lhs: llvm::Value,
    rhs: llvm::Value,
) -> llvm::Value {
    use llvm::{IntPred, OpCode};
    match op {
        hir::BinOp::Add_Int(_) => cg.build.bin_op(OpCode::LLVMAdd, lhs, rhs, "bin"),
        hir::BinOp::Add_Float(_) => cg.build.bin_op(OpCode::LLVMFAdd, lhs, rhs, "bin"),
        hir::BinOp::Sub_Int(_) => cg.build.bin_op(OpCode::LLVMSub, lhs, rhs, "bin"),
        hir::BinOp::Sub_Float(_) => cg.build.bin_op(OpCode::LLVMFSub, lhs, rhs, "bin"),
        hir::BinOp::Mul_Int(_) => cg.build.bin_op(OpCode::LLVMMul, lhs, rhs, "bin"),
        hir::BinOp::Mul_Float(_) => cg.build.bin_op(OpCode::LLVMFMul, lhs, rhs, "bin"),
        hir::BinOp::Div_Int(int_ty) => {
            let op = if int_ty.is_signed() { OpCode::LLVMSDiv } else { OpCode::LLVMUDiv };
            cg.build.bin_op(op, lhs, rhs, "bin")
        }
        hir::BinOp::Div_Float(_) => cg.build.bin_op(OpCode::LLVMFDiv, lhs, rhs, "bin"),
        hir::BinOp::Rem_Int(int_ty) => {
            let op = if int_ty.is_signed() { OpCode::LLVMSRem } else { OpCode::LLVMURem };
            cg.build.bin_op(op, lhs, rhs, "bin")
        }
        hir::BinOp::BitAnd(_) => cg.build.bin_op(OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::BitOr(_) => cg.build.bin_op(OpCode::LLVMOr, lhs, rhs, "bin"),
        hir::BinOp::BitXor(_) => cg.build.bin_op(OpCode::LLVMXor, lhs, rhs, "bin"),
        hir::BinOp::BitShl(lhs_ty, cast) => {
            let rhs = codegen_cast_op(cg, rhs, cg.int_type(lhs_ty), cast);
            cg.build.bin_op(OpCode::LLVMShl, lhs, rhs, "bin")
        }
        hir::BinOp::BitShr(lhs_ty, cast) => {
            let op = if lhs_ty.is_signed() { OpCode::LLVMAShr } else { OpCode::LLVMLShr };
            let rhs = codegen_cast_op(cg, rhs, cg.int_type(lhs_ty), cast);
            cg.build.bin_op(op, lhs, rhs, "bin")
        }
        hir::BinOp::Eq_Int_Other(bool_ty) => {
            let value = cg.build.icmp(IntPred::LLVMIntEQ, lhs, rhs, "bin");
            convert_i1_to_bool(cg, value, bool_ty)
        }
        hir::BinOp::NotEq_Int_Other(bool_ty) => {
            let value = cg.build.icmp(IntPred::LLVMIntNE, lhs, rhs, "bin");
            convert_i1_to_bool(cg, value, bool_ty)
        }
        hir::BinOp::Cmp_Int(pred, bool_ty, int_ty) => {
            let pred = cmp_int_predicate(pred, int_ty);
            let value = cg.build.icmp(pred, lhs, rhs, "bin");
            convert_i1_to_bool(cg, value, bool_ty)
        }
        hir::BinOp::Cmp_Float(pred, bool_ty, _) => {
            let pred = cmp_float_predicate(pred);
            let value = cg.build.fcmp(pred, lhs, rhs, "bin");
            convert_i1_to_bool(cg, value, bool_ty)
        }
        hir::BinOp::Cmp_String(pred, bool_ty, string_ty) => {
            let (fn_val, fn_ty) = match string_ty {
                hir::StringType::String => {
                    emit_mod::codegen_function(cg, cg.hir.core.string_equals, &[])
                }
                hir::StringType::CString => {
                    emit_mod::codegen_function(cg, cg.hir.core.cstring_equals, &[])
                }
                hir::StringType::Untyped => unreachable!(),
            };
            let mut value = cg.build.call(fn_ty, fn_val, &[lhs, rhs], "bin_str_cmp").unwrap();

            if pred == CmpPred::NotEq {
                let bool = cg.bool_type(hir::BoolType::Bool);
                let one = llvm::const_int(bool, 1, false);
                value = cg.build.bin_op(llvm::OpCode::LLVMXor, value, one, "bin_str_not")
            }
            if bool_ty != hir::BoolType::Bool {
                let into_ty = cg.bool_type(bool_ty);
                value = cg.build.cast(OpCode::LLVMZExt, value, into_ty, "bin_str_bcast")
            }
            value
        }
        hir::BinOp::LogicAnd(_) => cg.build.bin_op(llvm::OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::LogicOr(_) => cg.build.bin_op(llvm::OpCode::LLVMOr, lhs, rhs, "bin"),
    }
}

fn cmp_int_predicate(pred: CmpPred, int_ty: hir::IntType) -> llvm::IntPred {
    use llvm::IntPred;
    match pred {
        CmpPred::Eq => IntPred::LLVMIntEQ,
        CmpPred::NotEq => IntPred::LLVMIntNE,
        CmpPred::Less => {
            if int_ty.is_signed() {
                IntPred::LLVMIntSLT
            } else {
                IntPred::LLVMIntULT
            }
        }
        CmpPred::LessEq => {
            if int_ty.is_signed() {
                IntPred::LLVMIntSLE
            } else {
                IntPred::LLVMIntULE
            }
        }
        CmpPred::Greater => {
            if int_ty.is_signed() {
                IntPred::LLVMIntSGT
            } else {
                IntPred::LLVMIntUGT
            }
        }
        CmpPred::GreaterEq => {
            if int_ty.is_signed() {
                IntPred::LLVMIntSGE
            } else {
                IntPred::LLVMIntUGE
            }
        }
    }
}

fn cmp_float_predicate(pred: CmpPred) -> llvm::FloatPred {
    use llvm::FloatPred;
    match pred {
        CmpPred::Eq => FloatPred::LLVMRealOEQ,
        CmpPred::NotEq => FloatPred::LLVMRealONE,
        CmpPred::Less => FloatPred::LLVMRealOLT,
        CmpPred::LessEq => FloatPred::LLVMRealOLE,
        CmpPred::Greater => FloatPred::LLVMRealOGT,
        CmpPred::GreaterEq => FloatPred::LLVMRealOGE,
    }
}

fn convert_i1_to_bool(cg: &mut Codegen, val: llvm::Value, bool_ty: hir::BoolType) -> llvm::Value {
    if bool_ty == hir::BoolType::Bool {
        return val;
    }
    let into_ty = cg.bool_type(bool_ty);
    cg.build.cast(llvm::OpCode::LLVMZExt, val, into_ty, "i1_to_bool")
}

fn convert_bool_to_i1(cg: &mut Codegen, val: llvm::Value) -> llvm::Value {
    let into_ty = llvm::typeof_value(val);
    if llvm::type_equals(into_ty, cg.bool_type(hir::BoolType::Bool)) {
        return val;
    }
    let one = llvm::const_int(into_ty, 1, false);
    cg.build.icmp(llvm::IntPred::LLVMIntEQ, val, one, "bool_to_i1")
}

//only valid for supported array & assign binary ops
pub fn bin_op_operand_type(cg: &Codegen, op: hir::BinOp) -> llvm::Type {
    match op {
        hir::BinOp::Add_Int(ty) => cg.int_type(ty),
        hir::BinOp::Add_Float(ty) => cg.float_type(ty),
        hir::BinOp::Sub_Int(ty) => cg.int_type(ty),
        hir::BinOp::Sub_Float(ty) => cg.float_type(ty),
        hir::BinOp::Mul_Int(ty) => cg.int_type(ty),
        hir::BinOp::Mul_Float(ty) => cg.float_type(ty),
        hir::BinOp::Div_Int(ty) => cg.int_type(ty),
        hir::BinOp::Div_Float(ty) => cg.float_type(ty),
        hir::BinOp::Rem_Int(ty) => cg.int_type(ty),
        hir::BinOp::BitAnd(ty) => cg.int_type(ty),
        hir::BinOp::BitOr(ty) => cg.int_type(ty),
        hir::BinOp::BitXor(ty) => cg.int_type(ty),
        hir::BinOp::BitShl(ty, _) => cg.int_type(ty),
        hir::BinOp::BitShr(ty, _) => cg.int_type(ty),
        hir::BinOp::Eq_Int_Other(_)
        | hir::BinOp::NotEq_Int_Other(_)
        | hir::BinOp::Cmp_Int(_, _, _)
        | hir::BinOp::Cmp_Float(_, _, _)
        | hir::BinOp::Cmp_String(_, _, _)
        | hir::BinOp::LogicAnd(_)
        | hir::BinOp::LogicOr(_) => unreachable!(),
    }
}

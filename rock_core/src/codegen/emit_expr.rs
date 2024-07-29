use super::context::{Codegen, ProcCodegen};
use super::emit_stmt::{codegen_block, BlockKind, TailAllocaStatus};
use crate::ast;
use crate::hir;
use crate::intern::InternID;
use inkwell::types::{AsTypeRef, BasicType};
use inkwell::values::{self, AsValueRef};

pub fn codegen_expr_value<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let alloca_id = proc_cg.push_tail_alloca();
    if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca(alloca_id)) {
        value
    } else {
        match proc_cg.tail_alloca[alloca_id.index()] {
            TailAllocaStatus::NoValue => panic!("expected tail value"),
            TailAllocaStatus::WithValue(ptr, ptr_ty) => {
                cg.builder.build_load(ptr_ty, ptr, "tail_val").unwrap()
            }
        }
    }
}

pub fn codegen_expr_value_ptr<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
) -> values::PointerValue<'ctx> {
    let alloca_id = proc_cg.push_tail_alloca();
    if let Some(value) = codegen_expr(cg, proc_cg, true, expr, BlockKind::TailAlloca(alloca_id)) {
        value.into_pointer_value()
    } else {
        match proc_cg.tail_alloca[alloca_id.index()] {
            TailAllocaStatus::NoValue => panic!("expected tail value"),
            TailAllocaStatus::WithValue(ptr, ptr_ty) => cg
                .builder
                .build_load(ptr_ty, ptr, "tail_val")
                .unwrap()
                .into_pointer_value(),
        }
    }
}

pub fn codegen_expr_value_optional<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    let alloca_id = proc_cg.push_tail_alloca();
    if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca(alloca_id)) {
        Some(value)
    } else {
        match proc_cg.tail_alloca[alloca_id.index()] {
            TailAllocaStatus::NoValue => None,
            TailAllocaStatus::WithValue(ptr, ptr_ty) => {
                Some(cg.builder.build_load(ptr_ty, ptr, "tail_val").unwrap())
            }
        }
    }
}

pub fn codegen_block_value_optional<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    block: hir::Block<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    let alloca_id = proc_cg.push_tail_alloca();
    codegen_block(cg, proc_cg, block, BlockKind::TailAlloca(alloca_id));
    match proc_cg.tail_alloca[alloca_id.index()] {
        TailAllocaStatus::NoValue => None,
        TailAllocaStatus::WithValue(ptr, ptr_ty) => {
            Some(cg.builder.build_load(ptr_ty, ptr, "tail_val").unwrap())
        }
    }
}

#[must_use]
pub fn codegen_expr<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    expr: &'ctx hir::Expr<'ctx>,
    kind: BlockKind<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    use hir::Expr;
    match *expr {
        Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        Expr::Const { value } => Some(codegen_const_value(cg, value)),
        Expr::If { if_ } => {
            codegen_if(cg, proc_cg, if_, kind);
            None
        }
        Expr::Block { block } => {
            codegen_block(cg, proc_cg, block, kind);
            None
        }
        Expr::Match { match_ } => {
            codegen_match(cg, proc_cg, match_, kind);
            None
        }
        Expr::Match2 { match_ } => todo!("codegen match2"),
        Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => Some(codegen_struct_field(
            cg, proc_cg, expect_ptr, target, struct_id, field_id, deref,
        )),
        Expr::SliceField {
            target,
            field,
            deref,
        } => Some(codegen_slice_field(
            cg, proc_cg, expect_ptr, target, field, deref,
        )),
        Expr::Index { target, access } => {
            Some(codegen_index(cg, proc_cg, expect_ptr, target, access))
        }
        Expr::Slice { target, access } => {
            Some(codegen_slice(cg, proc_cg, expect_ptr, target, access))
        }
        Expr::Cast { target, into, kind } => Some(codegen_cast(cg, proc_cg, target, into, kind)),
        Expr::LocalVar { local_id } => Some(codegen_local_var(cg, proc_cg, expect_ptr, local_id)),
        Expr::ParamVar { param_id } => Some(codegen_param_var(cg, proc_cg, expect_ptr, param_id)),
        Expr::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, expect_ptr, global_id)),
        Expr::Variant {
            enum_id,
            variant_id,
            input,
        } => todo!("enum variant expr"),
        Expr::CallDirect { proc_id, input } => codegen_call_direct(cg, proc_cg, proc_id, input),
        Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, proc_cg, target, indirect)
        }
        Expr::StructInit { struct_id, input } => {
            codegen_struct_init(cg, proc_cg, struct_id, input, expect_ptr, kind)
        }
        Expr::ArrayInit { array_init } => {
            codegen_array_init(cg, proc_cg, array_init, expect_ptr, kind)
        }
        Expr::ArrayRepeat { array_repeat } => Some(codegen_array_repeat(cg, proc_cg, array_repeat)),
        Expr::Deref { rhs, ptr_ty } => Some(codegen_deref(cg, proc_cg, expect_ptr, rhs, *ptr_ty)),
        Expr::Address { rhs } => Some(codegen_address(cg, proc_cg, rhs)),
        Expr::Unary { op, rhs } => Some(codegen_unary(cg, proc_cg, op, rhs)),
        Expr::Binary {
            op,
            lhs,
            rhs,
            lhs_signed_int,
        } => Some(codegen_binary(cg, proc_cg, op, lhs, rhs, lhs_signed_int)),
    }
}

fn type_is_unsigned_int(ty: ast::BasicType) -> bool {
    match ty {
        ast::BasicType::S8
        | ast::BasicType::S16
        | ast::BasicType::S32
        | ast::BasicType::S64
        | ast::BasicType::Ssize => false,
        ast::BasicType::U8
        | ast::BasicType::U16
        | ast::BasicType::U32
        | ast::BasicType::U64
        | ast::BasicType::Usize => true,
        _ => false,
    }
}

#[must_use]
#[allow(unsafe_code)]
pub fn codegen_const_value<'ctx>(
    cg: &Codegen<'ctx>,
    value: hir::ConstValue<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    match value {
        hir::ConstValue::Error => panic!("codegen unexpected ConstValue::Error"),
        hir::ConstValue::Null => cg.ptr_type.const_zero().into(),
        hir::ConstValue::Bool { val } => cg.context.bool_type().const_int(val as u64, false).into(),
        hir::ConstValue::Int { val, neg, int_ty } => {
            let int_type = cg.basic_type_into_int(int_ty.into_basic());
            let signed = int_ty.is_signed();

            if neg {
                let negative = -(val as i64);
                int_type.const_int(negative as u64, signed).into()
            } else {
                int_type.const_int(val, signed).into()
            }
        }
        hir::ConstValue::IntS(val) => todo!("codegen: ConstValue::IntSigned"),
        hir::ConstValue::IntU(val) => todo!("codegen: ConstValue::IntUnsigned"),
        hir::ConstValue::Float { val, float_ty } => {
            let ty = float_ty.into_basic();
            cg.basic_type_into_float(ty).const_float(val).into()
        }
        hir::ConstValue::Char { val } => cg.context.i32_type().const_int(val as u64, false).into(),
        hir::ConstValue::String { id, c_string } => codegen_lit_string(cg, id, c_string),
        hir::ConstValue::Procedure { proc_id } => {
            let function = cg.function_values[proc_id.index()];
            function.as_global_value().as_pointer_value().into()
        }
        hir::ConstValue::EnumVariant { enum_ } => {
            todo!("enum const");
            //let variant = cg.hir.enum_data(enum_id).variant(variant_id);
            //codegen_const_value(cg, cg.hir.const_eval_value(variant.value))
        }
        hir::ConstValue::Struct { struct_ } => {
            use llvm_sys::core::LLVMConstNamedStruct;
            let mut values = Vec::with_capacity(struct_.fields.len());
            let struct_ty = cg.struct_type(struct_.struct_id).as_type_ref();

            for value_id in struct_.fields {
                let value = codegen_const_value(cg, cg.hir.const_value(*value_id));
                values.push(value.as_value_ref());
            }

            unsafe {
                let struct_value =
                    LLVMConstNamedStruct(struct_ty, values.as_mut_ptr(), values.len() as u32);
                values::BasicValueEnum::new(struct_value)
            }
        }
        hir::ConstValue::Array { array } => {
            use llvm_sys::core::LLVMConstArray2;
            let mut values = Vec::with_capacity(array.values.len());
            let mut elem_ty = None;

            //@fails on empty array due to how elem_ty is inferred
            for value_id in array.values {
                let value = codegen_const_value(cg, cg.hir.const_value(*value_id));
                elem_ty = Some(value.get_type().as_type_ref());
                values.push(value.as_value_ref());
            }

            unsafe {
                let array_value =
                    LLVMConstArray2(elem_ty.unwrap(), values.as_mut_ptr(), values.len() as u64);
                values::BasicValueEnum::new(array_value)
            }
        }
        hir::ConstValue::ArrayRepeat { value, len } => {
            use llvm_sys::core::LLVMConstArray2;
            let value = codegen_const_value(cg, cg.hir.const_value(value));
            let elem_ty = value.get_type().as_type_ref();

            //@find more optimal way to initialize const array with repeated values? 09.06.24
            let mut values = Vec::new();
            values.resize(len as usize, value.as_value_ref());

            unsafe {
                let array_value =
                    LLVMConstArray2(elem_ty, values.as_mut_ptr(), values.len() as u64);
                values::BasicValueEnum::new(array_value)
            }
        }
    }
}

fn codegen_lit_string<'ctx>(
    cg: &Codegen<'ctx>,
    id: InternID,
    c_string: bool,
) -> values::BasicValueEnum<'ctx> {
    let global_ptr = cg.string_lits[id.index()].as_pointer_value();

    if c_string {
        global_ptr.into()
    } else {
        let string = cg.hir.intern_string.get_str(id);
        let bytes_len = cg.ptr_sized_int_type.const_int(string.len() as u64, false);
        let slice_value = cg
            .context
            .const_struct(&[global_ptr.into(), bytes_len.into()], false);
        slice_value.into()
    }
}

//@simplify variable mutation and block flow
fn codegen_if<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    if_: &'ctx hir::If<'ctx>,
    kind: BlockKind<'ctx>,
) {
    let mut body_bb = cg.append_bb(proc_cg, "if_body");
    let exit_bb = cg.append_bb(proc_cg, "if_exit");

    let mut next_bb = if !if_.branches.is_empty() || if_.else_block.is_some() {
        cg.insert_bb(body_bb, "if_next")
    } else {
        exit_bb
    };

    let cond = codegen_expr_value(cg, proc_cg, if_.entry.cond);
    cg.build_cond_br(cond, body_bb, next_bb);

    cg.position_at_end(body_bb);
    codegen_block(cg, proc_cg, if_.entry.block, kind);
    cg.build_br_no_term(exit_bb);

    for (idx, branch) in if_.branches.iter().enumerate() {
        let last = idx + 1 == if_.branches.len();
        let create_next = !last || if_.else_block.is_some();

        body_bb = cg.insert_bb(next_bb, "if_body");
        cg.position_at_end(next_bb);

        let cond = codegen_expr_value(cg, proc_cg, branch.cond);
        next_bb = if create_next {
            cg.insert_bb(body_bb, "if_next")
        } else {
            exit_bb
        };
        cg.build_cond_br(cond, body_bb, next_bb);

        cg.position_at_end(body_bb);
        codegen_block(cg, proc_cg, branch.block, kind);
        cg.build_br_no_term(exit_bb);
    }

    if let Some(block) = if_.else_block {
        cg.position_at_end(next_bb);
        codegen_block(cg, proc_cg, block, kind);
        cg.build_br_no_term(exit_bb);
    }
    cg.position_at_end(exit_bb);
}

fn codegen_match<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    match_: &hir::Match<'ctx>,
    kind: BlockKind<'ctx>,
) {
    let insert_bb = cg.get_insert_bb();
    let on_value = codegen_expr_value(cg, proc_cg, match_.on_expr);
    let exit_bb = cg.append_bb(proc_cg, "match_exit");

    let mut cases = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms {
        if arm.unreachable {
            continue;
        }
        let value = codegen_const_value(cg, cg.hir.const_value(arm.pat));
        let case_bb = cg.append_bb(proc_cg, "match_case");
        cases.push((value.into_int_value(), case_bb));

        cg.position_at_end(case_bb);
        codegen_block(cg, proc_cg, arm.block, kind);
        cg.build_br_no_term(exit_bb);
    }

    let else_block = if let Some(fallback) = match_.fallback {
        let fallback_bb = cg.append_bb(proc_cg, "match_fallback");
        cg.position_at_end(fallback_bb);
        codegen_block(cg, proc_cg, fallback, kind);
        cg.build_br_no_term(exit_bb);
        fallback_bb
    } else {
        exit_bb
    };

    cg.position_at_end(insert_bb);
    cg.builder
        .build_switch(on_value.into_int_value(), else_block, &cases)
        .unwrap();
    cg.position_at_end(exit_bb);
}

fn codegen_struct_field<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    struct_id: hir::StructID,
    field_id: hir::FieldID,
    deref: bool,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr_value_ptr(cg, proc_cg, target);
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type, target, "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target
    };

    let field_ptr = cg
        .builder
        .build_struct_gep(
            cg.struct_type(struct_id),
            target_ptr,
            field_id.index() as u32,
            "field_ptr",
        )
        .unwrap();

    if expect_ptr {
        field_ptr.into()
    } else {
        let field = cg.hir.struct_data(struct_id).field(field_id);
        let field_ty = cg.type_into_basic(field.ty);
        cg.builder
            .build_load(field_ty, field_ptr, "field_val")
            .unwrap()
    }
}

fn codegen_slice_field<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr<'ctx>,
    field: hir::SliceField,
    deref: bool,
) -> values::BasicValueEnum<'ctx> {
    assert!(
        !expect_ptr,
        "slice access `expect_ptr` cannot be true, slice fields are not addressable"
    );
    let target = codegen_expr_value_ptr(cg, proc_cg, target);
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type, target, "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target
    };

    let (field_id, field_ty, ptr_name, value_name) = match field {
        hir::SliceField::Ptr => (
            0,
            cg.ptr_type.as_basic_type_enum(),
            "slice_ptr_ptr",
            "slice_ptr",
        ),
        hir::SliceField::Len => (
            1,
            cg.ptr_sized_int_type.as_basic_type_enum(),
            "slice_len_ptr",
            "slice_len",
        ),
    };

    let field_ptr = cg
        .builder
        .build_struct_gep(cg.slice_type, target_ptr, field_id, ptr_name)
        .unwrap();
    cg.builder
        .build_load(field_ty, field_ptr, value_name)
        .unwrap()
}

//@change to eprintf, re-use panic message strings (not generated for each panic) 07.05.24
// panics should be hooks into core library panicking module
// it could define how panic works (eg: calling epintf + exit + unreachable?)
fn codegen_panic_conditional<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    cond: values::IntValue<'ctx>,
    printf_args: &[values::BasicMetadataValueEnum<'ctx>],
) {
    let panic_block = cg
        .context
        .append_basic_block(proc_cg.function, "panic_block");
    let else_block = cg.context.append_basic_block(proc_cg.function, "block");
    cg.builder
        .build_conditional_branch(cond, panic_block, else_block)
        .unwrap();

    let c_exit = cg
        .c_functions
        .get(&cg.hir.intern_name.get_id("exit").expect("exit c function"))
        .cloned()
        .expect("exit c function added");
    let c_printf = cg
        .c_functions
        .get(
            &cg.hir
                .intern_name
                .get_id("printf")
                .expect("printf c function"),
        )
        .cloned()
        .expect("printf c function added");

    //@print to stderr instead of stdout & have better panic handling api 04.05.24
    // this is first draft of working panic messages and exit
    cg.builder.position_at_end(panic_block);
    cg.builder.build_call(c_printf, printf_args, "").unwrap();
    cg.builder
        .build_call(
            c_exit,
            &[cg.context.i32_type().const_int(1, true).into()],
            "",
        )
        .unwrap();
    cg.builder.build_unreachable().unwrap();

    cg.builder.position_at_end(else_block);
}

//@fix how bounds check is done 31.05.24
// possible feature is to not bounds check indexing with constant values
// that are proven to be in bounds of static array type, store flag in hir
#[allow(unsafe_code)]
fn codegen_index<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    access: &'ctx hir::IndexAccess,
) -> values::BasicValueEnum<'ctx> {
    //@should expect pointer always be true? 08.05.24
    // in case of slices that just delays the load?
    let target = codegen_expr_value_ptr(cg, proc_cg, target);
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type, target, "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target
    };

    let index = codegen_expr_value(cg, proc_cg, access.index).into_int_value();

    let elem_ptr = match access.kind {
        hir::IndexKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type, target_ptr, "slice_val")
                .unwrap()
                .into_struct_value();
            let ptr = cg
                .builder
                .build_extract_value(slice, 0, "slice_ptr")
                .unwrap()
                .into_pointer_value();
            let len = cg
                .builder
                .build_extract_value(slice, 1, "slice_len")
                .unwrap()
                .into_int_value();

            let panic_cond = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::UGE, index, len, "bounds_check")
                .unwrap();
            let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: index `%llu` out of bounds, slice len = `%llu`\n\n";
            let messsage_ptr = cg
                .builder
                .build_global_string_ptr(message, "panic_index_out_of_bounds")
                .unwrap()
                .as_pointer_value();
            codegen_panic_conditional(
                cg,
                proc_cg,
                panic_cond,
                &[messsage_ptr.into(), index.into(), len.into()],
            );

            //@i64 mul is probably wrong when dealing with non 64bit targets 07.05.24
            let elem_size = cg.context.i64_type().const_int(elem_size, false);
            let byte_offset =
                codegen_bin_op(cg, ast::BinOp::Mul, index.into(), elem_size.into(), false)
                    .into_int_value();
            unsafe {
                cg.builder
                    .build_in_bounds_gep(cg.context.i8_type(), ptr, &[byte_offset], "elem_ptr")
                    .unwrap()
            }
        }
        hir::IndexKind::Array { array } => unsafe {
            let len = cg.array_static_len(array.len);
            let len = cg.ptr_sized_int_type.const_int(len, false);

            let panic_cond = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::UGE, index, len, "bounds_check")
                .unwrap();
            let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: index `%llu` out of bounds, array len = `%llu`\n\n";
            let messsage_ptr = cg
                .builder
                .build_global_string_ptr(message, "panic_index_out_of_bounds")
                .unwrap()
                .as_pointer_value();
            codegen_panic_conditional(
                cg,
                proc_cg,
                panic_cond,
                &[messsage_ptr.into(), index.into(), len.into()],
            );

            cg.builder
                .build_in_bounds_gep(
                    cg.array_type(array),
                    target_ptr,
                    &[cg.ptr_sized_int_type.const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        },
    };

    if expect_ptr {
        elem_ptr.into()
    } else {
        let elem_ty = cg.type_into_basic(access.elem_ty);
        cg.builder
            .build_load(elem_ty, elem_ptr, "elem_val")
            .unwrap()
    }
}

//@all of this and index needs to be reworked, consider core library implementation
// instead of hardcoding llvm ir
// but index does need to be like an intrinsic with minimal codesize impact?
fn codegen_slice<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    access: &'ctx hir::SliceAccess,
) -> values::BasicValueEnum<'ctx> {
    //@should expect pointer always be true? 08.05.24
    // in case of slices that just delays the load?
    // causes problem when slicing multiple times into_pointer_value() gets called on new_slice_value that is not a pointer
    let target = codegen_expr_value_ptr(cg, proc_cg, target);
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type, target, "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target
    };

    match access.kind {
        hir::SliceKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type, target_ptr, "slice_val")
                .unwrap()
                .into_struct_value();
            let slice_len = cg
                .builder
                .build_extract_value(slice, 1, "slice_len")
                .unwrap()
                .into_int_value();
            let slice_ptr = cg
                .builder
                .build_extract_value(slice, 0, "slice_ptr")
                .unwrap()
                .into_pointer_value();

            let lower = match access.range.lower {
                Some(lower) => Some(codegen_expr_value(cg, proc_cg, lower).into_int_value()),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => {
                    Some(codegen_expr_value(cg, proc_cg, upper).into_int_value())
                }
                hir::SliceRangeEnd::Inclusive(upper) => {
                    Some(codegen_expr_value(cg, proc_cg, upper).into_int_value())
                }
            };

            match (lower, upper) {
                // slice is unchanged
                //@slice and its components are still extracted above even in this no-op 08.05.24
                (None, None) => {
                    return if expect_ptr {
                        //@returning pointer to same slice? 08.05.24
                        // that can be misleading? or no-op like this makes sence?
                        // probably this is valid and will reduce all RangeFull sling operations into one
                        target_ptr.into()
                    } else {
                        slice.into()
                    };
                }
                // upper is provided
                (None, Some(upper)) => {
                    let predicate =
                        if matches!(access.range.upper, hir::SliceRangeEnd::Exclusive(..)) {
                            inkwell::IntPredicate::UGT // upper > len
                        } else {
                            inkwell::IntPredicate::UGE // upper >= len
                        };
                    let panic_cond = cg
                        .builder
                        .build_int_compare(predicate, upper, slice_len, "slice_upper_bound")
                        .unwrap();
                    let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: slice upper `%llu` out of bounds, slice len = `%llu`\n\n";
                    let messsage_ptr = cg
                        .builder
                        .build_global_string_ptr(message, "panic_index_out_of_bounds")
                        .unwrap()
                        .as_pointer_value();
                    codegen_panic_conditional(
                        cg,
                        proc_cg,
                        panic_cond,
                        &[messsage_ptr.into(), upper.into(), slice_len.into()],
                    );

                    // sub 1 in case of exclusive range
                    let new_slice_len =
                        if matches!(access.range.upper, hir::SliceRangeEnd::Exclusive(..)) {
                            //codegen_bin_op(
                            //    cg,
                            //    ast::BinOp::Sub,
                            //    upper.into(),
                            //    cg.ptr_sized_int_type.const_int(1, false).into(),
                            //    false,
                            //)
                            //.into_int_value()
                            upper
                        } else {
                            codegen_bin_op(
                                cg,
                                ast::BinOp::Add,
                                upper.into(),
                                cg.ptr_sized_int_type.const_int(1, false).into(),
                                false,
                            )
                            .into_int_value()
                        };

                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type;

                    let new_slice = cg
                        .builder
                        .build_alloca(slice_type, "new_slice_ptr")
                        .unwrap();
                    let new_slice_0 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 0, "new_slice_ptr_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_0, slice_ptr).unwrap();
                    let new_slice_1 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 1, "new_slice_len_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_1, new_slice_len).unwrap();

                    if expect_ptr {
                        new_slice.into()
                    } else {
                        cg.builder
                            .build_load(slice_type, new_slice, "new_slice")
                            .unwrap()
                    }
                }
                // lower is provided
                (Some(lower), None) => {
                    // @temp
                    panic!("slice slicing lower.. not implemented");
                }
                // lower and uppoer are provided
                (Some(lower), Some(upper)) => {
                    // @temp
                    panic!("slice slicing lower..upper not implemented");
                }
            }
        }
        hir::SliceKind::Array { array } => {
            let len = cg.array_static_len(array.len);
            let len = cg.ptr_sized_int_type.const_int(len, false);

            let lower = match access.range.lower {
                Some(lower) => Some(codegen_expr_value(cg, proc_cg, lower).into_int_value()),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => {
                    Some(codegen_expr_value(cg, proc_cg, upper).into_int_value())
                }
                hir::SliceRangeEnd::Inclusive(upper) => {
                    Some(codegen_expr_value(cg, proc_cg, upper).into_int_value())
                }
            };

            match (lower, upper) {
                (None, None) => {
                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type;

                    let new_slice = cg
                        .builder
                        .build_alloca(slice_type, "new_slice_ptr")
                        .unwrap();
                    let new_slice_0 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 0, "new_slice_ptr_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_0, target_ptr).unwrap();
                    let new_slice_1 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 1, "new_slice_len_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_1, len).unwrap();

                    if expect_ptr {
                        new_slice.into()
                    } else {
                        cg.builder
                            .build_load(slice_type, new_slice, "new_slice")
                            .unwrap()
                    }
                }
                (None, Some(_)) => todo!("array slicing ..upper not implemented"),
                (Some(_), None) => todo!("array slicing lower..upper not implemented"),
                (Some(_), Some(_)) => todo!("array slicing lower..upper not implemented"),
            }
        }
    }
}

fn codegen_cast<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    target: &'ctx hir::Expr,
    into: &'ctx hir::Type,
    kind: hir::CastKind,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr_value(cg, proc_cg, target);
    let into = cg.type_into_basic(*into);
    let op = match kind {
        hir::CastKind::Error => panic!("codegen unexpected hir::CastKind::Error"),
        hir::CastKind::NoOp => return target,
        hir::CastKind::Integer_Trunc => values::InstructionOpcode::Trunc,
        hir::CastKind::Sint_Sign_Extend => values::InstructionOpcode::SExt,
        hir::CastKind::Uint_Zero_Extend => values::InstructionOpcode::ZExt,
        hir::CastKind::Float_to_Sint => values::InstructionOpcode::FPToSI,
        hir::CastKind::Float_to_Uint => values::InstructionOpcode::FPToUI,
        hir::CastKind::Sint_to_Float => values::InstructionOpcode::SIToFP,
        hir::CastKind::Uint_to_Float => values::InstructionOpcode::UIToFP,
        hir::CastKind::Float_Trunc => values::InstructionOpcode::FPTrunc,
        hir::CastKind::Float_Extend => values::InstructionOpcode::FPExt,
    };
    cg.builder.build_cast(op, target, into, "cast_val").unwrap()
}

fn codegen_local_var<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    expect_ptr: bool,
    local_id: hir::LocalID,
) -> values::BasicValueEnum<'ctx> {
    let local_ptr = proc_cg.local_vars[local_id.index()];

    if expect_ptr {
        local_ptr.into()
    } else {
        let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
        let local_ty = cg.type_into_basic(local.ty);
        cg.builder
            .build_load(local_ty, local_ptr, "local_val")
            .unwrap()
    }
}

fn codegen_param_var<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    expect_ptr: bool,
    param_id: hir::ParamID,
) -> values::BasicValueEnum<'ctx> {
    let param_ptr = proc_cg.param_vars[param_id.index()];

    if expect_ptr {
        param_ptr.into()
    } else {
        let param = cg.hir.proc_data(proc_cg.proc_id).param(param_id);
        let param_ty = cg.type_into_basic(param.ty);
        cg.builder
            .build_load(param_ty, param_ptr, "param_val")
            .unwrap()
    }
}

fn codegen_const_var<'ctx>(
    cg: &Codegen<'ctx>,
    const_id: hir::ConstID,
) -> values::BasicValueEnum<'ctx> {
    cg.consts[const_id.index()]
}

fn codegen_global_var<'ctx>(
    cg: &Codegen<'ctx>,
    expect_ptr: bool,
    global_id: hir::GlobalID,
) -> values::BasicValueEnum<'ctx> {
    let global = cg.globals[global_id.index()];
    let global_ptr = global.as_pointer_value();

    if expect_ptr {
        global_ptr.into()
    } else {
        let global_ty = global.get_initializer().expect("initialized").get_type();
        cg.builder
            .build_load(global_ty, global_ptr, "global_val")
            .unwrap()
    }
}

fn codegen_call_direct<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    proc_id: hir::ProcID,
    input: &'ctx [&'ctx hir::Expr],
) -> Option<values::BasicValueEnum<'ctx>> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr_value(cg, proc_cg, expr);
        input_values.push(value.into());
    }

    let function = cg.function_values[proc_id.index()];
    let call_val = cg
        .builder
        .build_direct_call(function, &input_values, "call_val")
        .unwrap();
    call_val.try_as_basic_value().left()
}

fn codegen_call_indirect<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    target: &'ctx hir::Expr,
    indirect: &'ctx hir::CallIndirect,
) -> Option<values::BasicValueEnum<'ctx>> {
    let function_ptr = codegen_expr_value(cg, proc_cg, target).into_pointer_value();

    let mut input_values = Vec::with_capacity(indirect.input.len());
    for &expr in indirect.input {
        let value = codegen_expr_value(cg, proc_cg, expr);
        input_values.push(value.into());
    }

    let function = cg.function_type(&indirect.proc_ty);
    let call_val = cg
        .builder
        .build_indirect_call(function, function_ptr, &input_values, "call_val")
        .unwrap();
    call_val.try_as_basic_value().left()
}

fn codegen_struct_init<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    struct_id: hir::StructID,
    input: &'ctx [hir::FieldInit<'ctx>],
    expect_ptr: bool,
    kind: BlockKind<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    let struct_ty = cg.struct_type(struct_id);
    let (struct_ptr, elided) = if let BlockKind::TailStore(target_ptr) = kind {
        (target_ptr, true)
    } else {
        (
            cg.entry_insert_alloca(proc_cg, struct_ty.into(), "struct_init"),
            false,
        )
    };

    for field_init in input {
        let field_ptr = cg
            .builder
            .build_struct_gep(
                struct_ty,
                struct_ptr,
                field_init.field_id.index() as u32,
                "field_ptr",
            )
            .unwrap();
        if let Some(value) = codegen_expr(
            cg,
            proc_cg,
            false,
            field_init.expr,
            BlockKind::TailStore(field_ptr),
        ) {
            cg.builder.build_store(field_ptr, value).unwrap();
        }
    }

    if expect_ptr {
        Some(struct_ptr.into())
    } else if elided {
        None
    } else {
        Some(
            cg.builder
                .build_load(struct_ty, struct_ptr, "struct_val")
                .unwrap(),
        )
    }
}

#[allow(unsafe_code)]
fn codegen_array_init<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    array_init: &'ctx hir::ArrayInit<'ctx>,
    expect_ptr: bool,
    kind: BlockKind<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    let elem_ty = cg.type_into_basic(array_init.elem_ty);
    let array_ty = elem_ty.array_type(array_init.input.len() as u32);
    let (array_ptr, elided) = if let BlockKind::TailStore(target_ptr) = kind {
        (target_ptr, true)
    } else {
        (
            cg.entry_insert_alloca(proc_cg, array_ty.into(), "array_init"),
            false,
        )
    };

    for (idx, &expr) in array_init.input.iter().enumerate() {
        let index = cg.ptr_sized_int_type.const_int(idx as u64, false);
        let elem_ptr = unsafe {
            cg.builder
                .build_in_bounds_gep(
                    array_ty,
                    array_ptr,
                    &[cg.ptr_sized_int_type.const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        };
        if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailStore(elem_ptr))
        {
            cg.builder.build_store(elem_ptr, value).unwrap();
        }
    }

    if expect_ptr {
        Some(array_ptr.into())
    } else if elided {
        None
    } else {
        Some(
            cg.builder
                .build_load(array_ty, array_ptr, "array_val")
                .unwrap(),
        )
    }
}

fn codegen_array_repeat<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    array_repeat: &'ctx hir::ArrayRepeat<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `array repeat` not supported")
}

fn codegen_deref<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    rhs: &'ctx hir::Expr<'ctx>,
    ptr_ty: hir::Type<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let value = codegen_expr_value(cg, proc_cg, rhs);
    let value_ptr = value.into_pointer_value();

    if expect_ptr {
        value_ptr.into()
    } else {
        cg.builder
            .build_load(cg.type_into_basic(ptr_ty), value_ptr, "deref_value")
            .unwrap()
    }
}

fn codegen_address<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    rhs: &'ctx hir::Expr<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    codegen_expr(cg, proc_cg, true, rhs, BlockKind::TailIgnore).expect("value")
}

fn codegen_unary<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    op: hir::UnOp,
    rhs: &'ctx hir::Expr<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let rhs = codegen_expr_value(cg, proc_cg, rhs);

    match op {
        hir::UnOp::Neg_Int(_) => cg
            .builder
            .build_int_neg(rhs.into_int_value(), "un_temp")
            .unwrap()
            .into(),
        hir::UnOp::Neg_Float(_) => cg
            .builder
            .build_float_neg(rhs.into_float_value(), "un_temp")
            .unwrap()
            .into(),
        hir::UnOp::BitNot(_) | hir::UnOp::LogicNot => cg
            .builder
            .build_not(rhs.into_int_value(), "un_temp")
            .unwrap()
            .into(),
    }
}

fn codegen_binary<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    op: ast::BinOp,
    lhs: &'ctx hir::Expr<'ctx>,
    rhs: &'ctx hir::Expr<'ctx>,
    lhs_signed_int: bool,
) -> values::BasicValueEnum<'ctx> {
    let lhs = codegen_expr_value(cg, proc_cg, lhs);
    let rhs = codegen_expr_value(cg, proc_cg, rhs);
    codegen_bin_op(cg, op, lhs, rhs, lhs_signed_int)
}

pub fn codegen_bin_op<'ctx>(
    cg: &Codegen<'ctx>,
    op: ast::BinOp,
    lhs: values::BasicValueEnum<'ctx>,
    rhs: values::BasicValueEnum<'ctx>,
    lhs_signed_int: bool,
) -> values::BasicValueEnum<'ctx> {
    match op {
        ast::BinOp::Add => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_add(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_add(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `+` can only be applied to int, float"),
        },
        ast::BinOp::Sub => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_sub(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_sub(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `-` can only be applied to int, float"),
        },
        ast::BinOp::Mul => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_mul(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_mul(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `*` can only be applied to int, float"),
        },
        ast::BinOp::Div => match lhs {
            values::BasicValueEnum::IntValue(lhs) => {
                if lhs_signed_int {
                    cg.builder
                        .build_int_signed_div(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                } else {
                    cg.builder
                        .build_int_unsigned_div(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                }
            }
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_div(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `/` can only be applied to int, float"),
        },
        ast::BinOp::Rem => match lhs {
            values::BasicValueEnum::IntValue(lhs) => {
                if lhs_signed_int {
                    cg.builder
                        .build_int_signed_rem(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                } else {
                    cg.builder
                        .build_int_unsigned_rem(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                }
            }
            _ => panic!("codegen: binary `%` can only be applied to int"),
        },
        ast::BinOp::BitAnd => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitOr => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitXor => cg
            .builder
            .build_xor(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitShl => cg
            .builder
            .build_left_shift(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitShr => cg
            .builder
            .build_right_shift(
                lhs.into_int_value(),
                rhs.into_int_value(),
                lhs_signed_int,
                "bin_temp",
            )
            .unwrap()
            .into(),
        ast::BinOp::IsEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::PointerValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    lhs,
                    rhs.into_pointer_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `==` can only be applied to int, float"),
        },
        ast::BinOp::NotEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::ONE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::PointerValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    lhs,
                    rhs.into_pointer_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `!=` can only be applied to int, float, rawptr"),
        },
        ast::BinOp::Less => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SLT
                    } else {
                        inkwell::IntPredicate::ULT
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OLT,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `<` can only be applied to int, float"),
        },
        ast::BinOp::LessEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SLE
                    } else {
                        inkwell::IntPredicate::ULE
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OLE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `<=` can only be applied to int, float"),
        },
        ast::BinOp::Greater => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SGT
                    } else {
                        inkwell::IntPredicate::UGT
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OGT,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `>` can only be applied to int, float"),
        },
        ast::BinOp::GreaterEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SGE
                    } else {
                        inkwell::IntPredicate::UGE
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OGE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `>=` can only be applied to int, float"),
        },
        ast::BinOp::LogicAnd => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::LogicOr => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
    }
}

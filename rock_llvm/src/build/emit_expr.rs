use super::context::{Codegen, Expect, ProcCodegen};
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::hir;
use rock_core::intern::InternID;

pub fn codegen_expr_value<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> llvm::Value {
    let value_id = proc_cg.add_tail_value();
    if let Some(value) = codegen_expr(cg, proc_cg, expr, Expect::Value(Some(value_id))) {
        value
    } else if let Some(tail) = proc_cg.tail_value(value_id) {
        cg.build.load(tail.value_ty, tail.value_ptr, "tail_val")
    } else {
        unreachable!();
    }
}

pub fn codegen_expr_value_opt<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> Option<llvm::Value> {
    let value_id = proc_cg.add_tail_value();
    if let Some(value) = codegen_expr(cg, proc_cg, expr, Expect::Value(Some(value_id))) {
        Some(value)
    } else if let Some(tail) = proc_cg.tail_value(value_id) {
        Some(cg.build.load(tail.value_ty, tail.value_ptr, "tail_val"))
    } else {
        None
    }
}

pub fn codegen_expr_tail<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    expr: &hir::Expr<'c>,
) {
    if let Some(value) = codegen_expr(cg, proc_cg, expr, expect) {
        match expect {
            Expect::Value(Some(value_id)) => {
                if let Some(tail) = proc_cg.tail_value(value_id) {
                    cg.build.store(value, tail.value_ptr);
                } else {
                    let value_ty = llvm::typeof_value(value);
                    let value_ptr = cg.entry_alloca(proc_cg, value_ty, "tail_ptr");
                    proc_cg.set_tail_value(value_id, value_ptr, value_ty);
                    cg.build.store(value, value_ptr);
                }
            }
            Expect::Value(None) => {}
            Expect::Pointer => unreachable!(),
            Expect::Store(ptr_val) => cg.build.store(value, ptr_val),
        }
    }
}

pub fn codegen_expr_pointer<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> llvm::ValuePtr {
    codegen_expr(cg, proc_cg, expr, Expect::Pointer)
        .unwrap()
        .into_ptr()
}

pub fn codegen_expr_store<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
    ptr_val: llvm::ValuePtr,
) {
    if let Some(value) = codegen_expr(cg, proc_cg, expr, Expect::Store(ptr_val)) {
        cg.build.store(value, ptr_val);
    }
}

fn codegen_expr<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
    expect: Expect,
) -> Option<llvm::Value> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { value } => Some(codegen_const(cg, value)),
        hir::Expr::If { if_ } => {
            codegen_if(cg, proc_cg, expect, if_);
            None
        }
        hir::Expr::Block { block } => {
            emit_stmt::codegen_block(cg, proc_cg, expect, block);
            None
        }
        hir::Expr::Match { match_ } => unimplemented!("emit match"),
        hir::Expr::Match2 { match_ } => unimplemented!("emit match2"),
        hir::Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => Some(codegen_struct_field(
            cg, proc_cg, expect, target, struct_id, field_id, deref,
        )),
        hir::Expr::SliceField {
            target,
            field,
            deref,
        } => Some(codegen_slice_field(
            cg, proc_cg, expect, target, field, deref,
        )),
        hir::Expr::Index { target, access } => {
            Some(codegen_index(cg, proc_cg, expect, target, access))
        }
        hir::Expr::Slice { target, access } => unimplemented!("emit slice"),
        hir::Expr::Cast { target, into, kind } => {
            Some(codegen_cast(cg, proc_cg, target, into, kind))
        }
        hir::Expr::LocalVar { local_id } => Some(codegen_local_var(cg, proc_cg, expect, local_id)),
        hir::Expr::ParamVar { param_id } => Some(codegen_param_var(cg, proc_cg, expect, param_id)),
        hir::Expr::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        hir::Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, expect, global_id)),
        hir::Expr::Variant {
            enum_id,
            variant_id,
            input,
        } => unimplemented!("emit variant"),
        hir::Expr::CallDirect { proc_id, input } => {
            codegen_call_direct(cg, proc_cg, proc_id, input)
        }
        hir::Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, proc_cg, target, indirect)
        }
        hir::Expr::StructInit { struct_id, input } => {
            codegen_struct_init(cg, proc_cg, expect, struct_id, input)
        }
        hir::Expr::ArrayInit { array_init } => codegen_array_init(cg, proc_cg, expect, array_init),
        hir::Expr::ArrayRepeat { array_repeat } => {
            codegen_array_repeat(cg, proc_cg, expect, array_repeat)
        }
        hir::Expr::Deref { rhs, ptr_ty } => Some(codegen_deref(cg, proc_cg, expect, rhs, *ptr_ty)),
        hir::Expr::Address { rhs } => Some(codegen_address(cg, proc_cg, rhs)),
        hir::Expr::Unary { op, rhs } => Some(codegen_unary(cg, proc_cg, op, rhs)),
        hir::Expr::Binary { op, lhs, rhs } => Some(codegen_binary(cg, proc_cg, op, lhs, rhs)),
    }
}

pub fn codegen_const(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    match value {
        hir::ConstValue::Error => unreachable!(),
        hir::ConstValue::Null => codegen_const_null(cg),
        hir::ConstValue::Bool { val } => codegen_const_bool(cg, val),
        hir::ConstValue::Int { val, int_ty, .. } => codegen_const_int(cg, val, int_ty),
        hir::ConstValue::IntS(_) => unimplemented!(),
        hir::ConstValue::IntU(_) => unimplemented!(),
        hir::ConstValue::Float { val, float_ty } => codegen_const_float(cg, val, float_ty),
        hir::ConstValue::Char { val } => codegen_const_char(cg, val),
        hir::ConstValue::String { id, c_string } => codegen_const_string(cg, id, c_string),
        hir::ConstValue::Procedure { proc_id } => cg.procs[proc_id.index()].0.as_ptr().as_val(),
        hir::ConstValue::Variant { variant } => codegen_const_variant(cg, variant),
        hir::ConstValue::Struct { struct_ } => codegen_const_struct(cg, struct_),
        hir::ConstValue::Array { array } => codegen_const_array(cg, array),
        hir::ConstValue::ArrayRepeat { value, len } => codegen_const_array_repeat(cg, value, len),
    }
}

#[inline]
fn codegen_const_null(cg: &Codegen) -> llvm::Value {
    llvm::const_all_zero(cg.ptr_type())
}

#[inline]
fn codegen_const_bool(cg: &Codegen, val: bool) -> llvm::Value {
    llvm::const_int(cg.bool_type(), val as u64, false)
}

#[inline]
fn codegen_const_int(cg: &Codegen, val: u64, int_ty: hir::BasicInt) -> llvm::Value {
    llvm::const_int(cg.basic_type(int_ty.into_basic()), val, int_ty.is_signed())
}

#[inline]
fn codegen_const_float(cg: &Codegen, val: f64, float_ty: hir::BasicFloat) -> llvm::Value {
    llvm::const_float(cg.basic_type(float_ty.into_basic()), val)
}

#[inline]
fn codegen_const_char(cg: &Codegen, val: char) -> llvm::Value {
    llvm::const_int(cg.basic_type(ast::BasicType::U32), val as u64, false)
}

fn codegen_const_string(cg: &Codegen, id: InternID, c_string: bool) -> llvm::Value {
    let global_ptr = cg.string_lits[id.index()].as_ptr();

    if c_string {
        global_ptr.as_val()
    } else {
        let string = cg.hir.intern_string.get_str(id);
        let slice_len = cg.const_usize(string.len() as u64);
        llvm::const_struct_inline(&[global_ptr.as_val(), slice_len], false)
    }
}

fn codegen_const_variant(cg: &Codegen, variant: &hir::ConstVariant) -> llvm::Value {
    unimplemented!()
}

fn codegen_const_struct(cg: &Codegen, struct_: &hir::ConstStruct) -> llvm::Value {
    let mut values = Vec::with_capacity(struct_.fields.len());
    for &value_id in struct_.fields {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    let struct_ty = cg.struct_type(struct_.struct_id);
    llvm::const_struct_named(struct_ty, &values)
}

fn codegen_const_array(cg: &Codegen, array: &hir::ConstArray) -> llvm::Value {
    let mut values = Vec::with_capacity(array.len as usize);
    for &value_id in array.values {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    //@type of zero sized arrays is not stored, will cras
    let elem_ty = llvm::typeof_value(values[0]);
    llvm::const_array(elem_ty, &values)
}

fn codegen_const_array_repeat(cg: &Codegen, value_id: hir::ConstValueID, len: u64) -> llvm::Value {
    let mut values = Vec::with_capacity(len as usize);
    let value = codegen_const(cg, cg.hir.const_value(value_id));
    values.resize(len as usize, value);

    //@zero sized array repeat is counter intuitive
    let elem_ty = llvm::typeof_value(values[0]);
    llvm::const_array(elem_ty, &values)
}

//@simplify
fn codegen_if<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    if_: &hir::If<'c>,
) {
    let mut body_bb = cg.append_bb(proc_cg, "if_body");
    let exit_bb = cg.append_bb(proc_cg, "if_exit");

    let mut next_bb = if !if_.branches.is_empty() || if_.else_block.is_some() {
        cg.append_bb(proc_cg, "if_next")
    } else {
        exit_bb
    };

    let cond = codegen_expr_value(cg, proc_cg, if_.entry.cond);
    cg.build.cond_br(cond, body_bb, next_bb);

    cg.build.position_at_end(body_bb);
    emit_stmt::codegen_block(cg, proc_cg, expect, if_.entry.block);
    cg.build_br_no_term(exit_bb);

    for (idx, branch) in if_.branches.iter().enumerate() {
        let last = idx + 1 == if_.branches.len();
        let create_next = !last || if_.else_block.is_some();

        body_bb = cg.append_bb(proc_cg, "if_body");
        cg.build.position_at_end(next_bb);

        let cond = codegen_expr_value(cg, proc_cg, branch.cond);
        next_bb = if create_next {
            cg.append_bb(proc_cg, "if_next")
        } else {
            exit_bb
        };
        cg.build.cond_br(cond, body_bb, next_bb);

        cg.build.position_at_end(body_bb);
        emit_stmt::codegen_block(cg, proc_cg, expect, branch.block);
        cg.build_br_no_term(exit_bb);
    }

    if let Some(block) = if_.else_block {
        cg.build.position_at_end(next_bb);
        emit_stmt::codegen_block(cg, proc_cg, expect, block);
        cg.build_br_no_term(exit_bb);
    }
    cg.build.position_at_end(exit_bb);
}

fn codegen_struct_field<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    struct_id: hir::StructID,
    field_id: hir::FieldID,
    deref: bool,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if deref {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
    } else {
        target_ptr
    };

    let field_ptr = cg.build.gep_struct(
        cg.struct_type(struct_id),
        target_ptr,
        field_id.raw(),
        "field_ptr",
    );

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let field = cg.hir.struct_data(struct_id).field(field_id);
            let field_ty = cg.ty(field.ty);
            cg.build.load(field_ty, field_ptr, "field_val")
        }
        Expect::Pointer => target_ptr.as_val(),
    }
}

fn codegen_slice_field<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    field: hir::SliceField,
    deref: bool,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if deref {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
    } else {
        target_ptr
    };

    let (idx, field_ty, ptr_name, value_name) = match field {
        hir::SliceField::Ptr => (0, cg.ptr_type(), "slice_ptr_ptr", "slice_ptr"),
        hir::SliceField::Len => (1, cg.ptr_sized_int(), "slice_len_ptr", "slice_len"),
    };

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let slice_ty = cg.slice_type();
            let field_ptr = cg.build.gep_struct(slice_ty, target_ptr, idx, ptr_name);
            cg.build.load(field_ty, field_ptr, value_name)
        }
        Expect::Pointer => unreachable!(),
    }
}

//@no bounds check
fn codegen_index<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::IndexAccess<'c>,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if access.deref {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
    } else {
        target_ptr
    };

    let index_val = codegen_expr_value(cg, proc_cg, access.index);

    let elem_ptr = match access.kind {
        hir::IndexKind::Slice { elem_size } => {
            let elem_size = cg.const_usize(elem_size);
            let byte_offset = codegen_binary_op(cg, hir::BinOp::Mul_Int, index_val, elem_size);
            let slice_ptr = cg
                .build
                .load(cg.ptr_type(), target_ptr, "slice_ptr")
                .into_ptr();
            cg.build.gep_inbounds(
                cg.basic_type(ast::BasicType::U8),
                slice_ptr,
                &[byte_offset],
                "elem_ptr",
            )
        }
        hir::IndexKind::Array { array } => cg.build.gep_inbounds(
            cg.array_type(array),
            target_ptr,
            &[cg.const_usize_zero(), index_val],
            "elem_ptr",
        ),
    };

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            cg.build.load(cg.ty(access.elem_ty), elem_ptr, "elem_val")
        }
        Expect::Pointer => elem_ptr.as_val(),
    }
}

fn codegen_cast<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    target: &hir::Expr<'c>,
    into: &hir::Type,
    kind: hir::CastKind,
) -> llvm::Value {
    use llvm::OpCode;
    let val = codegen_expr_value(cg, proc_cg, target);
    let into_ty = cg.ty(*into);

    match kind {
        hir::CastKind::Error => unreachable!(),
        hir::CastKind::NoOp => val,
        hir::CastKind::Int_Trunc => cg.build.cast(OpCode::LLVMTrunc, val, into_ty, "cast"),
        hir::CastKind::IntS_Sign_Extend => cg.build.cast(OpCode::LLVMSExt, val, into_ty, "cast"),
        hir::CastKind::IntU_Zero_Extend => cg.build.cast(OpCode::LLVMZExt, val, into_ty, "cast"),
        hir::CastKind::IntS_to_Float => cg.build.cast(OpCode::LLVMSIToFP, val, into_ty, "cast"),
        hir::CastKind::IntU_to_Float => cg.build.cast(OpCode::LLVMUIToFP, val, into_ty, "cast"),
        hir::CastKind::Float_to_IntS => cg.build.cast(OpCode::LLVMFPToSI, val, into_ty, "cast"),
        hir::CastKind::Float_to_IntU => cg.build.cast(OpCode::LLVMFPToUI, val, into_ty, "cast"),
        hir::CastKind::Float_Trunc => cg.build.cast(OpCode::LLVMFPTrunc, val, into_ty, "cast"),
        hir::CastKind::Float_Extend => cg.build.cast(OpCode::LLVMFPExt, val, into_ty, "cast"),
    }
}

fn codegen_local_var(
    cg: &Codegen,
    proc_cg: &ProcCodegen,
    expect: Expect,
    local_id: hir::LocalID,
) -> llvm::Value {
    let local_ptr = proc_cg.local_ptrs[local_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
            let local_ty = cg.ty(local.ty);
            cg.build.load(local_ty, local_ptr, "local_val")
        }
        Expect::Pointer => local_ptr.as_val(),
    }
}

fn codegen_param_var(
    cg: &Codegen,
    proc_cg: &ProcCodegen,
    expect: Expect,
    param_id: hir::ParamID,
) -> llvm::Value {
    let param_ptr = proc_cg.param_ptrs[param_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let param = cg.hir.proc_data(proc_cg.proc_id).param(param_id);
            let param_ty = cg.ty(param.ty);
            cg.build.load(param_ty, param_ptr, "param_val")
        }
        Expect::Pointer => param_ptr.as_val(),
    }
}

fn codegen_const_var(cg: &Codegen, const_id: hir::ConstID) -> llvm::Value {
    cg.consts[const_id.index()]
}

fn codegen_global_var(cg: &Codegen, expect: Expect, global_id: hir::GlobalID) -> llvm::Value {
    let global_ptr = cg.globals[global_id.index()].as_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let data = cg.hir.global_data(global_id);
            let global_ty = cg.ty(data.ty);
            cg.build.load(global_ty, global_ptr, "global_val")
        }
        Expect::Pointer => global_ptr.as_val(),
    }
}

fn codegen_call_direct<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    proc_id: hir::ProcID,
    input: &[&hir::Expr<'c>],
) -> Option<llvm::Value> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr_value(cg, proc_cg, expr);
        input_values.push(value);
    }

    let (fn_val, fn_ty) = cg.procs[proc_id.index()];
    cg.build.call(fn_ty, fn_val, &input_values, "call_val")
}

fn codegen_call_indirect<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    target: &hir::Expr<'c>,
    indirect: &hir::CallIndirect<'c>,
) -> Option<llvm::Value> {
    let fn_val = codegen_expr_value(cg, proc_cg, target).into_fn();

    let mut input_values = Vec::with_capacity(indirect.input.len());
    for &expr in indirect.input {
        let value = codegen_expr_value(cg, proc_cg, expr);
        input_values.push(value);
    }

    let fn_ty = cg.proc_type(indirect.proc_ty);
    cg.build.call(fn_ty, fn_val, &input_values, "call_ival")
}

fn codegen_struct_init<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    struct_id: hir::StructID,
    input: &[hir::FieldInit<'c>],
) -> Option<llvm::Value> {
    let struct_ty = cg.struct_type(struct_id);
    let struct_ptr = match expect {
        Expect::Value(_) | Expect::Pointer => {
            cg.entry_alloca(proc_cg, struct_ty.as_ty(), "struct_init")
        }
        Expect::Store(ptr_val) => ptr_val,
    };

    for field_init in input {
        let field_ptr = cg.build.gep_struct(
            struct_ty,
            struct_ptr,
            field_init.field_id.raw(),
            "field_ptr",
        );
        codegen_expr_store(cg, proc_cg, field_init.expr, field_ptr);
    }

    match expect {
        Expect::Value(_) => Some(cg.build.load(struct_ty.as_ty(), struct_ptr, "struct_val")),
        Expect::Pointer => Some(struct_ptr.as_val()),
        Expect::Store(_) => None,
    }
}

fn codegen_array_init<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    array_init: &hir::ArrayInit<'c>,
) -> Option<llvm::Value> {
    let elem_ty = cg.ty(array_init.elem_ty);
    let array_ty = llvm::array_type(elem_ty, array_init.input.len() as u64);
    let array_ptr = match expect {
        Expect::Value(_) | Expect::Pointer => cg.entry_alloca(proc_cg, array_ty, "array_init"),
        Expect::Store(ptr_val) => ptr_val,
    };

    let mut indices = [cg.const_usize_zero(), cg.const_usize_zero()];
    for (idx, &expr) in array_init.input.iter().enumerate() {
        indices[1] = cg.const_usize(idx as u64);
        let elem_ptr = cg
            .build
            .gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
        codegen_expr_store(cg, proc_cg, expr, elem_ptr);
    }

    match expect {
        Expect::Value(_) => Some(cg.build.load(array_ty, array_ptr, "array_val")),
        Expect::Pointer => Some(array_ptr.as_val()),
        Expect::Store(_) => None,
    }
}

fn codegen_array_repeat<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    array_repeat: &hir::ArrayRepeat<'c>,
) -> Option<llvm::Value> {
    let elem_ty = cg.ty(array_repeat.elem_ty);
    let array_ty = llvm::array_type(elem_ty, array_repeat.len);
    let array_ptr = match expect {
        Expect::Value(_) | Expect::Pointer => cg.entry_alloca(proc_cg, array_ty, "array_repeat"),
        Expect::Store(ptr_val) => ptr_val,
    };

    let copied_val = codegen_expr_value(cg, proc_cg, array_repeat.expr);
    let count_ptr = cg.entry_alloca(proc_cg, cg.ptr_sized_int(), "rep_count");
    cg.build.store(cg.const_usize_zero(), count_ptr);

    let entry_bb = cg.append_bb(proc_cg, "rep_entry");
    let body_bb = cg.append_bb(proc_cg, "rep_body");
    let exit_bb = cg.append_bb(proc_cg, "rep_exit");

    cg.build.br(entry_bb);
    cg.build.position_at_end(entry_bb);
    let count_val = cg.build.load(cg.ptr_sized_int(), count_ptr, "rep_val");
    let repeat_val = cg.const_usize(array_repeat.len);
    let cond = codegen_binary_op(cg, hir::BinOp::Less_IntU, count_val, repeat_val);
    cg.build.cond_br(cond, body_bb, exit_bb);

    cg.build.position_at_end(body_bb);
    let indices = [cg.const_usize_zero(), count_val];
    let elem_ptr = cg
        .build
        .gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
    cg.build.store(copied_val, elem_ptr);

    let count_inc = codegen_binary_op(cg, hir::BinOp::Add_Int, count_val, cg.const_usize_one());
    cg.build.store(count_inc, count_ptr);
    cg.build.br(entry_bb);
    cg.build.position_at_end(exit_bb);

    match expect {
        Expect::Value(_) => Some(cg.build.load(array_ty, array_ptr, "array_val")),
        Expect::Pointer => Some(array_ptr.as_val()),
        Expect::Store(_) => None,
    }
}

fn codegen_deref<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    rhs: &hir::Expr<'c>,
    ptr_ty: hir::Type,
) -> llvm::Value {
    let ptr_val = codegen_expr_value(cg, proc_cg, rhs).into_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let ptr_ty = cg.ty(ptr_ty);
            cg.build.load(ptr_ty, ptr_val, "deref_val")
        }
        Expect::Pointer => ptr_val.as_val(),
    }
}

fn codegen_address<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    codegen_expr_pointer(cg, proc_cg, rhs).as_val()
}

fn codegen_unary<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    op: hir::UnOp,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let rhs = codegen_expr_value(cg, proc_cg, rhs);

    match op {
        hir::UnOp::Neg_Int => cg.build.neg(rhs, "un"),
        hir::UnOp::Neg_Float => cg.build.fneg(rhs, "un"),
        hir::UnOp::BitNot => cg.build.not(rhs, "un"),
        hir::UnOp::LogicNot => cg.build.not(rhs, "un"),
    }
}

fn codegen_binary<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    op: hir::BinOp,
    lhs: &hir::Expr<'c>,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let lhs = codegen_expr_value(cg, proc_cg, lhs);
    let rhs = codegen_expr_value(cg, proc_cg, rhs);
    codegen_binary_op(cg, op, lhs, rhs)
}

pub fn codegen_binary_op(
    cg: &Codegen,
    op: hir::BinOp,
    lhs: llvm::Value,
    rhs: llvm::Value,
) -> llvm::Value {
    use llvm::{FloatPred, IntPred, OpCode};
    match op {
        hir::BinOp::Add_Int => cg.build.bin_op(OpCode::LLVMAdd, lhs, rhs, "bin"),
        hir::BinOp::Add_Float => cg.build.bin_op(OpCode::LLVMFAdd, lhs, rhs, "bin"),
        hir::BinOp::Sub_Int => cg.build.bin_op(OpCode::LLVMSub, lhs, rhs, "bin"),
        hir::BinOp::Sub_Float => cg.build.bin_op(OpCode::LLVMFSub, lhs, rhs, "bin"),
        hir::BinOp::Mul_Int => cg.build.bin_op(OpCode::LLVMMul, lhs, rhs, "bin"),
        hir::BinOp::Mul_Float => cg.build.bin_op(OpCode::LLVMFMul, lhs, rhs, "bin"),
        hir::BinOp::Div_IntS => cg.build.bin_op(OpCode::LLVMSDiv, lhs, rhs, "bin"),
        hir::BinOp::Div_IntU => cg.build.bin_op(OpCode::LLVMUDiv, lhs, rhs, "bin"),
        hir::BinOp::Div_Float => cg.build.bin_op(OpCode::LLVMFDiv, lhs, rhs, "bin"),
        hir::BinOp::Rem_IntS => cg.build.bin_op(OpCode::LLVMSRem, lhs, rhs, "bin"),
        hir::BinOp::Rem_IntU => cg.build.bin_op(OpCode::LLVMURem, lhs, rhs, "bin"),
        hir::BinOp::BitAnd => cg.build.bin_op(OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::BitOr => cg.build.bin_op(OpCode::LLVMOr, lhs, rhs, "bin"),
        hir::BinOp::BitXor => cg.build.bin_op(OpCode::LLVMXor, lhs, rhs, "bin"),
        hir::BinOp::BitShl => cg.build.bin_op(OpCode::LLVMShl, lhs, rhs, "bin"),
        hir::BinOp::BitShr_IntS => cg.build.bin_op(OpCode::LLVMAShr, lhs, rhs, "bin"),
        hir::BinOp::BitShr_IntU => cg.build.bin_op(OpCode::LLVMLShr, lhs, rhs, "bin"),
        hir::BinOp::IsEq_Int => cg.build.icmp(IntPred::LLVMIntEQ, lhs, rhs, "bin"),
        hir::BinOp::IsEq_Float => cg.build.fcmp(FloatPred::LLVMRealOEQ, lhs, rhs, "bin"),
        hir::BinOp::NotEq_Int => cg.build.icmp(IntPred::LLVMIntNE, lhs, rhs, "bin"),
        hir::BinOp::NotEq_Float => cg.build.fcmp(FloatPred::LLVMRealONE, lhs, rhs, "bin"),
        hir::BinOp::Less_IntS => cg.build.icmp(IntPred::LLVMIntSLT, lhs, rhs, "bin"),
        hir::BinOp::Less_IntU => cg.build.icmp(IntPred::LLVMIntULT, lhs, rhs, "bin"),
        hir::BinOp::Less_Float => cg.build.fcmp(FloatPred::LLVMRealOLT, lhs, rhs, "bin"),
        hir::BinOp::LessEq_IntS => cg.build.icmp(IntPred::LLVMIntSLE, lhs, rhs, "bin"),
        hir::BinOp::LessEq_IntU => cg.build.icmp(IntPred::LLVMIntULE, lhs, rhs, "bin"),
        hir::BinOp::LessEq_Float => cg.build.fcmp(FloatPred::LLVMRealOLE, lhs, rhs, "bin"),
        hir::BinOp::Greater_IntS => cg.build.icmp(IntPred::LLVMIntSGT, lhs, rhs, "bin"),
        hir::BinOp::Greater_IntU => cg.build.icmp(IntPred::LLVMIntUGT, lhs, rhs, "bin"),
        hir::BinOp::Greater_Float => cg.build.fcmp(FloatPred::LLVMRealOGT, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_IntS => cg.build.icmp(IntPred::LLVMIntSGE, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_IntU => cg.build.icmp(IntPred::LLVMIntUGE, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_Float => cg.build.fcmp(FloatPred::LLVMRealOGE, lhs, rhs, "bin"),
        hir::BinOp::LogicAnd => cg.build.bin_op(llvm::OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::LogicOr => cg.build.bin_op(llvm::OpCode::LLVMOr, lhs, rhs, "bin"),
    }
}

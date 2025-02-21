use super::context::{Codegen, Expect};
use super::emit_mod;
use super::emit_stmt;
use crate::llvm;
use rock_core::hir::{self, CmpPred};
use rock_core::intern::LitID;
use rock_core::text::{self, TextOffset};

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
            Expect::Pointer => unreachable!(),
            Expect::Store(ptr_val) => cg.build.store(value, ptr_val),
        }
    }
}

pub fn codegen_expr_pointer<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
) -> llvm::ValuePtr {
    codegen_expr(cg, expr, Expect::Pointer).unwrap().into_ptr()
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

fn codegen_expr<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expr: &hir::Expr<'c>,
    expect: Expect,
) -> Option<llvm::Value> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { value } => Some(codegen_const_expr(cg, expect, value)),
        hir::Expr::If { if_ } => {
            codegen_if(cg, expect, if_);
            None
        }
        hir::Expr::Block { block } => {
            emit_stmt::codegen_block(cg, expect, block);
            None
        }
        hir::Expr::Match { kind, match_ } => {
            codegen_match(cg, expect, kind, match_);
            None
        }
        hir::Expr::StructField { target, access } => {
            Some(codegen_struct_field(cg, expect, target, &access))
        }
        hir::Expr::SliceField { target, access } => {
            Some(codegen_slice_field(cg, expect, target, &access))
        }
        hir::Expr::Index { target, access } => Some(codegen_index(cg, expect, target, access)),
        hir::Expr::Slice { target, access } => unimplemented!("slicing"),
        hir::Expr::Cast { target, into, kind } => Some(codegen_cast(cg, target, into, kind)),
        hir::Expr::Transmute { target, into } => Some(codegen_transmute(cg, expect, target, into)),
        hir::Expr::CallerLocation { .. } => Some(codegen_caller_location(cg, expect)),
        hir::Expr::ParamVar { param_id } => Some(codegen_param_var(cg, expect, param_id)),
        hir::Expr::Variable { var_id } => Some(codegen_variable(cg, expect, var_id)),
        hir::Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, expect, global_id)),
        hir::Expr::Variant { enum_id, variant_id, input } => {
            codegen_variant(cg, expect, enum_id, variant_id, input)
        }
        hir::Expr::CallDirect { proc_id, input, start } => {
            codegen_call_direct(cg, expect, proc_id, input, start)
        }
        hir::Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, expect, target, indirect)
        }
        hir::Expr::StructInit { struct_id, input } => {
            codegen_struct_init(cg, expect, struct_id, input)
        }
        hir::Expr::ArrayInit { array } => codegen_array_init(cg, expect, array),
        hir::Expr::ArrayRepeat { array } => codegen_array_repeat(cg, expect, array),
        hir::Expr::Deref { rhs, ref_ty, .. } => Some(codegen_deref(cg, expect, rhs, *ref_ty)),
        hir::Expr::Address { rhs } => Some(codegen_address(cg, rhs)),
        hir::Expr::Unary { op, rhs } => Some(codegen_unary(cg, op, rhs)),
        hir::Expr::Binary { op, lhs, rhs } => Some(codegen_binary(cg, op, lhs, rhs)),
    }
}

fn codegen_const_expr(cg: &mut Codegen, expect: Expect, value: hir::ConstValue) -> llvm::Value {
    //pointer expect examples:
    // 1) "string".len - @should be const folded instead, `&` on slice fields is not allowed anyways. 04.01.25
    // 2) &[1, 2, 3] - @currently never can occur, no folding is done in `non-const` blocks. 04.01.25
    let value = codegen_const(cg, value);
    if let Expect::Pointer = expect {
        let temp_ptr = cg.entry_alloca(llvm::typeof_value(value), "temp_const");
        cg.build.store(value, temp_ptr);
        return temp_ptr.as_val();
    }
    value
}

pub fn codegen_const(cg: &mut Codegen, value: hir::ConstValue) -> llvm::Value {
    match value {
        hir::ConstValue::Void => llvm::const_struct_named(cg.void_val_type(), &[]),
        hir::ConstValue::Null => llvm::const_zeroed(cg.ptr_type()),
        hir::ConstValue::Bool { val, bool_ty } => codegen_const_bool(cg, val, bool_ty),
        hir::ConstValue::Int { val, neg, int_ty } => {
            let ext_val = if neg { (!val).wrapping_add(1) } else { val };
            llvm::const_int(cg.int_type(int_ty), ext_val, int_ty.is_signed())
        }
        hir::ConstValue::Float { val, float_ty } => llvm::const_float(cg.float_type(float_ty), val),
        hir::ConstValue::Char { val } => llvm::const_int(cg.char_type(), val as u64, false),
        hir::ConstValue::String { val, string_ty } => codegen_const_string(cg, val, string_ty),
        hir::ConstValue::Procedure { proc_id } => cg.procs[proc_id.index()].0.as_ptr().as_val(),
        hir::ConstValue::Variant { enum_id, variant_id } => {
            codegen_const_variant(cg, enum_id, variant_id)
        }
        hir::ConstValue::Struct { struct_ } => codegen_const_struct(cg, struct_),
        hir::ConstValue::Array { array } => codegen_const_array(cg, array),
        hir::ConstValue::ArrayRepeat { array } => codegen_const_array_repeat(cg, array),
        hir::ConstValue::ArrayEmpty { elem_ty } => llvm::const_array(cg.ty(*elem_ty), &[]),
    }
}

fn codegen_const_bool(cg: &Codegen, val: bool, bool_ty: hir::BoolType) -> llvm::Value {
    llvm::const_int(cg.bool_type(bool_ty), val as u64, false)
}

fn codegen_const_string(cg: &Codegen, val: LitID, string_ty: hir::StringType) -> llvm::Value {
    let global_ptr = cg.string_lits[val.index()].as_ptr();
    match string_ty {
        hir::StringType::String => {
            let string = cg.session.intern_lit.get(val);
            let slice_len = cg.const_usize(string.len() as u64);
            llvm::const_struct_named(cg.slice_type(), &[global_ptr.as_val(), slice_len])
        }
        hir::StringType::CString => global_ptr.as_val(),
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

fn codegen_const_struct(cg: &mut Codegen, struct_: &hir::ConstStruct) -> llvm::Value {
    let offset = cg.cache.values.start();
    for value in struct_.values {
        let value = codegen_const(cg, *value);
        cg.cache.values.push(value);
    }
    let values = cg.cache.values.view(offset.clone());

    let struct_ty = cg.struct_type(struct_.struct_id);
    let struct_ = llvm::const_struct_named(struct_ty, values);
    cg.cache.values.pop_view(offset);
    struct_
}

fn codegen_const_array(cg: &mut Codegen, array: &hir::ConstArray) -> llvm::Value {
    let offset = cg.cache.values.start();
    for value in array.values {
        let value = codegen_const(cg, *value);
        cg.cache.values.push(value);
    }
    let values = cg.cache.values.view(offset.clone());

    let elem_ty = llvm::typeof_value(values[0]);
    let array = llvm::const_array(elem_ty, &values);
    cg.cache.values.pop_view(offset);
    array
}

fn codegen_const_array_repeat(cg: &mut Codegen, array: &hir::ConstArrayRepeat) -> llvm::Value {
    let offset = cg.cache.values.start();
    let value = codegen_const(cg, array.value);
    for _ in 0..array.len {
        cg.cache.values.push(value);
    }
    let values = cg.cache.values.view(offset.clone());

    let elem_ty = llvm::typeof_value(values[0]);
    let array = llvm::const_array(elem_ty, &values);
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

//@getting enum variant tag is repetative
fn codegen_match<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    kind: hir::MatchKind,
    match_: &hir::Match<'c>,
) {
    #[inline]
    fn extract_slice_len_if_needed(
        cg: &mut Codegen,
        kind: hir::MatchKind,
        value: llvm::Value,
    ) -> llvm::Value {
        if let hir::MatchKind::String = kind {
            cg.build.extract_value(value, 1, "slice_len")
        } else {
            value
        }
    }

    let (on_value, enum_ptr, bind_by_pointer) = match kind {
        hir::MatchKind::Int { .. }
        | hir::MatchKind::Bool { .. }
        | hir::MatchKind::Char
        | hir::MatchKind::String => {
            let on_value = codegen_expr_value(cg, match_.on_expr);
            (on_value, None, false)
        }
        hir::MatchKind::Enum { enum_id, ref_mut } => {
            //@dont always expect a pointer if enum is fieldless (ir quality)
            let enum_ptr = codegen_expr_pointer(cg, match_.on_expr);
            let enum_ptr = if ref_mut.is_some() {
                cg.build.load(cg.ptr_type(), enum_ptr, "deref").into_ptr()
            } else {
                enum_ptr
            };

            let enum_data = cg.hir.enum_data(enum_id);
            let tag_ty = cg.int_type(enum_data.tag_ty.resolved_unwrap());
            let on_value = cg.build.load(tag_ty, enum_ptr, "enum_tag");
            (on_value, Some(enum_ptr), ref_mut.is_some())
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
                let pat_value = codegen_const(cg, value);
                let v = extract_slice_len_if_needed(cg, kind, pat_value);
                cg.cache.cases.push((v, arm_bb));
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
                    let variant_ty = &cg.variants[enum_id.index()];
                    let variant_ty = variant_ty[variant_id.index()].expect("variant ty");

                    for (field_idx, var_id) in bind_ids.iter().copied().enumerate() {
                        let proc_data = cg.hir.proc_data(cg.proc.proc_id);
                        let bind_var = proc_data.variable(var_id);

                        let field_ptr = cg.build.gep_struct(
                            variant_ty,
                            enum_ptr.unwrap(),
                            field_idx as u32 + 1,
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
                            let pat_value = codegen_const(cg, value);
                            let v = extract_slice_len_if_needed(cg, kind, pat_value);
                            cg.cache.cases.push((v, arm_bb));
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
    let on_value = extract_slice_len_if_needed(cg, kind, on_value);
    let else_bb = wild_bb.unwrap_or(exit_bb);

    let cases = cg.cache.cases.view(offset.clone());
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
    access: &hir::StructFieldAccess,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build.load(cg.ptr_type(), target_ptr, "deref_ptr").into_ptr()
    } else {
        target_ptr
    };

    let field_ptr = cg.build.gep_struct(
        cg.struct_type(access.struct_id),
        target_ptr,
        access.field_id.raw(),
        "field_ptr",
    );

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let field = cg.hir.struct_data(access.struct_id).field(access.field_id);
            let field_ty = cg.ty(field.ty);
            cg.build.load(field_ty, field_ptr, "field_val")
        }
        Expect::Pointer => field_ptr.as_val(),
    }
}

fn codegen_slice_field<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::SliceFieldAccess,
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
        Expect::Pointer => unreachable!(),
    }
}

//@no bounds check
fn codegen_index<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::IndexAccess<'c>,
) -> llvm::Value {
    //@also handle slice by value?
    // sometimes causes proc results to stack allocate.
    let target_ptr = match access.kind {
        hir::IndexKind::Multi(_) => codegen_expr_value(cg, target).into_ptr(),
        hir::IndexKind::Slice(_) | hir::IndexKind::Array(_) => codegen_expr_pointer(cg, target),
    };
    let target_ptr = if access.deref.is_some() {
        cg.build.load(cg.ptr_type(), target_ptr, "deref_ptr").into_ptr()
    } else {
        target_ptr
    };

    let index_val = codegen_expr_value(cg, access.index);

    let bound = match access.kind {
        hir::IndexKind::Multi(_) => None,
        hir::IndexKind::Slice(_) => {
            let slice_len_ptr =
                cg.build.gep_struct(cg.slice_type(), target_ptr, 1, "slice_len_ptr");
            let len = cg.build.load(cg.ptr_sized_int(), slice_len_ptr, "slice_len");
            Some(len)
        }
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            Some(cg.const_usize(len))
        }
    };

    if let Some(bound) = bound {
        let check_bb = cg.append_bb("bounds_check");
        let exit_bb = cg.append_bb("bounds_exit");
        let cond = codegen_binary_op(
            cg,
            hir::BinOp::Cmp_Int(CmpPred::GreaterEq, hir::BoolType::Bool, hir::IntType::Usize),
            index_val,
            bound,
        );
        cg.build.cond_br(cond, check_bb, exit_bb);
        cg.build.position_at_end(check_bb);
        //@insert panic call (cannot get reference to it currently)
        cg.build.br(exit_bb);
        cg.build.position_at_end(exit_bb);
    }

    let elem_ty = cg.ty(access.elem_ty);
    let elem_ptr = match access.kind {
        hir::IndexKind::Multi(_) => {
            cg.build.gep(elem_ty, target_ptr, &[index_val], "multi_elem_ptr")
        }
        hir::IndexKind::Slice(_) => {
            let slice_ptr = cg.build.load(cg.ptr_type(), target_ptr, "slice_ptr").into_ptr();
            cg.build.gep(elem_ty, slice_ptr, &[index_val], "slice_elem_ptr")
        }
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            let array_ty = llvm::array_type(elem_ty, len);
            cg.build.gep(array_ty, target_ptr, &[cg.const_usize(0), index_val], "array_elem_ptr")
        }
    };

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            cg.build.load(cg.ty(access.elem_ty), elem_ptr, "elem_val")
        }
        Expect::Pointer => elem_ptr.as_val(),
    }
}

fn codegen_cast<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    target: &hir::Expr<'c>,
    into: &hir::Type,
    kind: hir::CastKind,
) -> llvm::Value {
    use hir::CastKind;
    use llvm::OpCode;

    let val = codegen_expr_value(cg, target);
    let into_ty = cg.ty(*into);

    match kind {
        CastKind::Error => unreachable!(),
        CastKind::Char_NoOp => val,
        CastKind::Rawptr_NoOp => val,

        CastKind::Int_NoOp => val,
        CastKind::Int_Trunc => cg.build.cast(OpCode::LLVMTrunc, val, into_ty, "icast"),
        CastKind::IntS_Extend => cg.build.cast(OpCode::LLVMSExt, val, into_ty, "icast"),
        CastKind::IntU_Extend => cg.build.cast(OpCode::LLVMZExt, val, into_ty, "icast"),
        CastKind::IntS_to_Float => cg.build.cast(OpCode::LLVMSIToFP, val, into_ty, "icast"),
        CastKind::IntU_to_Float => cg.build.cast(OpCode::LLVMUIToFP, val, into_ty, "icast"),

        CastKind::Float_Trunc => cg.build.cast(OpCode::LLVMFPTrunc, val, into_ty, "fcast"),
        CastKind::Float_Extend => cg.build.cast(OpCode::LLVMFPExt, val, into_ty, "fcast"),
        CastKind::Float_to_IntS => cg.build.cast(OpCode::LLVMFPToSI, val, into_ty, "fcast"),
        CastKind::Float_to_IntU => cg.build.cast(OpCode::LLVMFPToUI, val, into_ty, "fcast"),

        CastKind::Bool_Trunc => cg.build.cast(OpCode::LLVMTrunc, val, into_ty, "bcast"),
        CastKind::Bool_Extend => cg.build.cast(OpCode::LLVMZExt, val, into_ty, "bcast"),
        CastKind::Bool_NoOp_to_Int => val,
        CastKind::Bool_Trunc_to_Int => cg.build.cast(OpCode::LLVMTrunc, val, into_ty, "bcast"),
        CastKind::Bool_Extend_to_Int => cg.build.cast(OpCode::LLVMZExt, val, into_ty, "bcast"),

        CastKind::Enum_NoOp_to_Int => val,
        CastKind::Enum_Trunc_to_Int => cg.build.cast(OpCode::LLVMTrunc, val, into_ty, "ecast"),
        CastKind::EnumS_Extend_to_Int => cg.build.cast(OpCode::LLVMSExt, val, into_ty, "ecast"),
        CastKind::EnumU_Extend_to_Int => cg.build.cast(OpCode::LLVMZExt, val, into_ty, "ecast"),
    }
}

fn codegen_transmute<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    into: &hir::Type,
) -> llvm::Value {
    let val = codegen_expr_value(cg, target);
    let into_ty = cg.ty(*into);

    let local_ptr = cg.entry_alloca(into_ty, "transmute_local");
    cg.build.store(val, local_ptr);

    match expect {
        Expect::Value(_) | Expect::Store(_) => cg.build.load(into_ty, local_ptr, "transmute_val"),
        Expect::Pointer => local_ptr.as_val(),
    }
}

fn codegen_caller_location(cg: &mut Codegen, expect: Expect) -> llvm::Value {
    let param_ptr = cg.proc.param_ptrs.last().copied().unwrap();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            cg.build.load(cg.location_ty.as_ty(), param_ptr, "caller_location_val")
        }
        Expect::Pointer => param_ptr.as_val(),
    }
}

fn codegen_param_var(cg: &mut Codegen, expect: Expect, param_id: hir::ParamID) -> llvm::Value {
    let param_ptr = cg.proc.param_ptrs[param_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let param = cg.hir.proc_data(cg.proc.proc_id).param(param_id);
            cg.build.load(cg.ty(param.ty), param_ptr, "param_val")
        }
        Expect::Pointer => param_ptr.as_val(),
    }
}

fn codegen_variable(cg: &mut Codegen, expect: Expect, var_id: hir::VariableID) -> llvm::Value {
    let var_ptr = cg.proc.variable_ptrs[var_id.index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let var = cg.hir.proc_data(cg.proc.proc_id).variable(var_id);
            cg.build.load(cg.ty(var.ty), var_ptr, "var_val")
        }
        Expect::Pointer => var_ptr.as_val(),
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
        Expect::Pointer => global_ptr.as_val(),
    }
}

fn codegen_variant<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    input: &&[&hir::Expr<'c>],
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

    if with_fields {
        let enum_ty = cg.enum_type(enum_id);
        let enum_ptr = match expect {
            Expect::Value(_) | Expect::Pointer => cg.entry_alloca(enum_ty, "enum_init"),
            Expect::Store(into_ptr) => into_ptr,
        };

        cg.build.store(tag, enum_ptr);
        if !variant.fields.is_empty() {
            let variant_ty = &cg.variants[enum_id.index()];
            let variant_ty = variant_ty[variant_id.index()].expect("variant ty");

            for (idx, expr) in input.iter().enumerate() {
                let field_ptr =
                    cg.build.gep_struct(variant_ty, enum_ptr, idx as u32 + 1, "variant_field_ptr");
                codegen_expr_store(cg, expr, field_ptr);
            }
        }

        match expect {
            Expect::Value(_) => Some(cg.build.load(enum_ty, enum_ptr, "enum_value")),
            Expect::Pointer => Some(enum_ptr.as_val()),
            Expect::Store(_) => None,
        }
    } else {
        match expect {
            Expect::Value(_) | Expect::Store(_) => Some(tag),
            Expect::Pointer => {
                let enum_ptr = cg.entry_alloca(llvm::typeof_value(tag), "enum_init");
                cg.build.store(tag, enum_ptr);
                Some(enum_ptr.as_val())
            }
        }
    }
}

//@set correct calling conv for the call itself?
fn codegen_call_direct<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    proc_id: hir::ProcID,
    input: &[&hir::Expr<'c>],
    start: TextOffset,
) -> Option<llvm::Value> {
    let offset = cg.cache.values.start();
    for (idx, expr) in input.iter().copied().enumerate() {
        let proc_data = cg.hir.proc_data(proc_id);
        //@hack for variadics
        if idx >= proc_data.params.len() {
            let value = codegen_expr_value(cg, expr);
            cg.cache.values.push(value);
            continue;
        }
        let param = proc_data.param(hir::ParamID::new(idx));

        //@copy pasta from fn_val generation
        let is_external = proc_data.flag_set.contains(hir::ProcFlag::External);
        let value = if is_external && emit_mod::win64_abi_pass_by_pointer(cg, param.ty) {
            codegen_expr_pointer(cg, expr).as_val()
        } else {
            codegen_expr_value(cg, expr)
        };
        cg.cache.values.push(value);
    }

    let proc_data = cg.hir.proc_data(proc_id);
    if proc_data.flag_set.contains(hir::ProcFlag::CallerLocation) {
        let call_origin_id = cg.hir.proc_data(cg.proc.proc_id).origin_id;
        let call_origin = cg.session.module.get(call_origin_id);
        let call_file = cg.session.vfs.file(call_origin.file_id());
        let location = text::find_text_location(&call_file.source, start, &call_file.line_ranges);
        let line = llvm::const_int(cg.int_type(hir::IntType::U32), location.line() as u64, false);
        let col = llvm::const_int(cg.int_type(hir::IntType::U32), location.col() as u64, false);

        let path = call_file.path().to_str().unwrap();
        let string_val = llvm::const_string(&cg.context, path, true);
        let string_ty = llvm::typeof_value(string_val);
        let global = cg.module.add_global("rock.string.path", string_val, string_ty, true, true);

        let slice_len = cg.const_usize(path.len() as u64);
        let string_slice =
            llvm::const_struct_named(cg.slice_type(), &[global.as_ptr().as_val(), slice_len]);
        let location = llvm::const_struct_inline(&cg.context, &[line, col, string_slice], false);
        cg.cache.values.push(location);
    }

    let (fn_val, fn_ty) = cg.procs[proc_id.index()];
    let input_values = cg.cache.values.view(offset.clone());
    let ret_val = cg.build.call(fn_ty, fn_val, &input_values, "call_val")?;
    cg.cache.values.pop_view(offset);

    match expect {
        Expect::Pointer => {
            let ty = llvm::typeof_value(ret_val);
            let ptr = cg.entry_alloca(ty, "call_val_ptr");
            cg.build.store(ret_val, ptr);
            Some(ptr.as_val())
        }
        _ => Some(ret_val),
    }
}

//@handle ABI for indirect calls aswell
//@set correct calling conv,
// add calling conv to proc pointer type?
// need to support #ccall directive specifically for proc pointer type
fn codegen_call_indirect<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    target: &hir::Expr<'c>,
    indirect: &hir::CallIndirect<'c>,
) -> Option<llvm::Value> {
    let fn_val = codegen_expr_value(cg, target).into_fn();

    let offset = cg.cache.values.start();
    for &expr in indirect.input {
        let value = codegen_expr_value(cg, expr);
        cg.cache.values.push(value);
    }

    let fn_ty = cg.proc_type(indirect.proc_ty);
    let input_values = cg.cache.values.view(offset.clone());
    let ret_val = cg.build.call(fn_ty, fn_val, &input_values, "icall_val")?;
    cg.cache.values.pop_view(offset);

    match expect {
        Expect::Pointer => {
            let ty = llvm::typeof_value(ret_val);
            let ptr = cg.entry_alloca(ty, "icall_val_ptr");
            cg.build.store(ret_val, ptr);
            Some(ptr.as_val())
        }
        _ => Some(ret_val),
    }
}

fn codegen_struct_init<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    struct_id: hir::StructID,
    input: &[hir::FieldInit<'c>],
) -> Option<llvm::Value> {
    let struct_ty = cg.struct_type(struct_id);
    let struct_ptr = match expect {
        Expect::Value(_) | Expect::Pointer => cg.entry_alloca(struct_ty.as_ty(), "struct_init"),
        Expect::Store(ptr_val) => ptr_val,
    };

    for field_init in input {
        let field_ptr =
            cg.build.gep_struct(struct_ty, struct_ptr, field_init.field_id.raw(), "field_ptr");
        codegen_expr_store(cg, field_init.expr, field_ptr);
    }

    match expect {
        Expect::Value(_) => Some(cg.build.load(struct_ty.as_ty(), struct_ptr, "struct_val")),
        Expect::Pointer => Some(struct_ptr.as_val()),
        Expect::Store(_) => None,
    }
}

fn codegen_array_init<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    array: &hir::ArrayInit<'c>,
) -> Option<llvm::Value> {
    let elem_ty = cg.ty(array.elem_ty);
    let array_ty = llvm::array_type(elem_ty, array.input.len() as u64);
    let array_ptr = match expect {
        Expect::Value(_) | Expect::Pointer => cg.entry_alloca(array_ty, "array_init"),
        Expect::Store(ptr_val) => ptr_val,
    };

    let mut indices = [cg.const_usize(0), cg.const_usize(0)];
    for (idx, &expr) in array.input.iter().enumerate() {
        indices[1] = cg.const_usize(idx as u64);
        let elem_ptr = cg.build.gep_inbounds(array_ty, array_ptr, &indices, "elem_ptr");
        codegen_expr_store(cg, expr, elem_ptr);
    }

    match expect {
        Expect::Value(_) => Some(cg.build.load(array_ty, array_ptr, "array_val")),
        Expect::Pointer => Some(array_ptr.as_val()),
        Expect::Store(_) => None,
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
        Expect::Value(_) | Expect::Pointer => cg.entry_alloca(array_ty, "array_repeat"),
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

    let count_inc = codegen_binary_op(cg, hir::BinOp::Add_Int, count_val, cg.const_usize(1));
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
    cg: &mut Codegen<'c, '_, '_>,
    expect: Expect,
    rhs: &hir::Expr<'c>,
    ref_ty: hir::Type,
) -> llvm::Value {
    let ptr_val = codegen_expr_value(cg, rhs).into_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let ptr_ty = cg.ty(ref_ty);
            cg.build.load(ptr_ty, ptr_val, "deref_val")
        }
        Expect::Pointer => ptr_val.as_val(),
    }
}

fn codegen_address<'c>(cg: &mut Codegen<'c, '_, '_>, rhs: &hir::Expr<'c>) -> llvm::Value {
    codegen_expr_pointer(cg, rhs).as_val()
}

fn codegen_unary<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    op: hir::UnOp,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let rhs = codegen_expr_value(cg, rhs);
    match op {
        hir::UnOp::Neg_Int => cg.build.neg(rhs, "un"),
        hir::UnOp::Neg_Float => cg.build.fneg(rhs, "un"),
        hir::UnOp::BitNot => cg.build.not(rhs, "un"),
        hir::UnOp::LogicNot => {
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
        hir::BinOp::Add_Int => cg.build.bin_op(OpCode::LLVMAdd, lhs, rhs, "bin"),
        hir::BinOp::Add_Float => cg.build.bin_op(OpCode::LLVMFAdd, lhs, rhs, "bin"),
        hir::BinOp::Sub_Int => cg.build.bin_op(OpCode::LLVMSub, lhs, rhs, "bin"),
        hir::BinOp::Sub_Float => cg.build.bin_op(OpCode::LLVMFSub, lhs, rhs, "bin"),
        hir::BinOp::Mul_Int => cg.build.bin_op(OpCode::LLVMMul, lhs, rhs, "bin"),
        hir::BinOp::Mul_Float => cg.build.bin_op(OpCode::LLVMFMul, lhs, rhs, "bin"),
        hir::BinOp::Div_Int(int_ty) => {
            let op = if int_ty.is_signed() { OpCode::LLVMSDiv } else { OpCode::LLVMUDiv };
            cg.build.bin_op(op, lhs, rhs, "bin")
        }
        hir::BinOp::Div_Float => cg.build.bin_op(OpCode::LLVMFDiv, lhs, rhs, "bin"),
        hir::BinOp::Rem_Int(int_ty) => {
            let op = if int_ty.is_signed() { OpCode::LLVMSRem } else { OpCode::LLVMURem };
            cg.build.bin_op(op, lhs, rhs, "bin")
        }
        hir::BinOp::BitAnd => cg.build.bin_op(OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::BitOr => cg.build.bin_op(OpCode::LLVMOr, lhs, rhs, "bin"),
        hir::BinOp::BitXor => cg.build.bin_op(OpCode::LLVMXor, lhs, rhs, "bin"),
        hir::BinOp::BitShl => cg.build.bin_op(OpCode::LLVMShl, lhs, rhs, "bin"),
        hir::BinOp::BitShr(int_ty) => {
            let op = if int_ty.is_signed() { OpCode::LLVMAShr } else { OpCode::LLVMLShr };
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
                hir::StringType::String => cg.procs[cg.hir.core.string_equals.index()],
                hir::StringType::CString => cg.procs[cg.hir.core.cstring_equals.index()],
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

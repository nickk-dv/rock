use super::context::{Codegen, Expect, ProcCodegen};
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub fn codegen_expr_value<'c>(
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> llvm::ValuePtr {
    codegen_expr(cg, proc_cg, expr, Expect::Pointer)
        .unwrap()
        .into_ptr()
}

pub fn codegen_expr_store<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
    ptr_val: llvm::ValuePtr,
) {
    if let Some(value) = codegen_expr(cg, proc_cg, expr, Expect::Store(ptr_val)) {
        cg.build.store(value, ptr_val);
    }
}

fn codegen_expr<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
    expect: Expect,
) -> Option<llvm::Value> {
    match expr.kind {
        hir::ExprKind::Error => unreachable!(),
        hir::ExprKind::Const { value } => Some(codegen_const_in_proc(cg, proc_cg, expect, value)),
        hir::ExprKind::If { if_ } => {
            codegen_if(cg, proc_cg, expect, if_);
            None
        }
        hir::ExprKind::Block { block } => {
            emit_stmt::codegen_block(cg, proc_cg, expect, block);
            None
        }
        hir::ExprKind::Match { kind, match_ } => {
            codegen_match(cg, proc_cg, expect, kind, match_);
            None
        }
        hir::ExprKind::StructField { target, access } => {
            Some(codegen_struct_field(cg, proc_cg, expect, target, &access))
        }
        hir::ExprKind::SliceField { target, access } => {
            Some(codegen_slice_field(cg, proc_cg, expect, target, &access))
        }
        hir::ExprKind::Index { target, access } => {
            Some(codegen_index(cg, proc_cg, expect, target, access))
        }
        hir::ExprKind::Slice { target, access } => unimplemented!("slicing"),
        hir::ExprKind::Cast { target, into, kind } => {
            Some(codegen_cast(cg, proc_cg, target, into, kind))
        }
        hir::ExprKind::ParamVar { param_id } => {
            Some(codegen_param_var(cg, proc_cg, expect, param_id))
        }
        hir::ExprKind::LocalVar { local_id } => {
            Some(codegen_local_var(cg, proc_cg, expect, local_id))
        }
        hir::ExprKind::LocalBind { local_bind_id } => {
            Some(codegen_local_bind_var(cg, proc_cg, expect, local_bind_id))
        }
        hir::ExprKind::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        hir::ExprKind::GlobalVar { global_id } => Some(codegen_global_var(cg, expect, global_id)),
        hir::ExprKind::Variant {
            enum_id,
            variant_id,
            input,
        } => Some(codegen_variant(
            cg, proc_cg, expect, enum_id, variant_id, input,
        )),
        hir::ExprKind::CallDirect { proc_id, input } => {
            codegen_call_direct(cg, proc_cg, proc_id, input)
        }
        hir::ExprKind::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, proc_cg, target, indirect)
        }
        hir::ExprKind::StructInit { struct_id, input } => {
            codegen_struct_init(cg, proc_cg, expect, struct_id, input)
        }
        hir::ExprKind::ArrayInit { array_init } => {
            codegen_array_init(cg, proc_cg, expect, array_init)
        }
        hir::ExprKind::ArrayRepeat { array_repeat } => {
            codegen_array_repeat(cg, proc_cg, expect, array_repeat)
        }
        hir::ExprKind::Deref { rhs, ref_ty, .. } => {
            Some(codegen_deref(cg, proc_cg, expect, rhs, *ref_ty))
        }
        hir::ExprKind::Address { rhs } => Some(codegen_address(cg, proc_cg, rhs)),
        hir::ExprKind::Unary { op, rhs } => Some(codegen_unary(cg, proc_cg, op, rhs)),
        hir::ExprKind::Binary { op, lhs, rhs } => Some(codegen_binary(cg, proc_cg, op, lhs, rhs)),
    }
}

pub fn codegen_const(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    match value {
        hir::ConstValue::Void => codegen_const_void(cg),
        hir::ConstValue::Null => codegen_const_null(cg),
        hir::ConstValue::Bool { val } => codegen_const_bool(cg, val),
        hir::ConstValue::Int { val, int_ty, .. } => codegen_const_int(cg, val, int_ty),
        hir::ConstValue::Float { val, float_ty } => codegen_const_float(cg, val, float_ty),
        hir::ConstValue::Char { val } => codegen_const_char(cg, val),
        hir::ConstValue::String { string_lit } => codegen_const_string(cg, string_lit),
        hir::ConstValue::Procedure { proc_id } => cg.procs[proc_id.raw_index()].0.as_ptr().as_val(),
        hir::ConstValue::Variant { variant } => codegen_const_variant(cg, variant),
        hir::ConstValue::Struct { struct_ } => codegen_const_struct(cg, struct_),
        hir::ConstValue::Array { array } => codegen_const_array(cg, array),
        hir::ConstValue::ArrayRepeat { value, len } => codegen_const_array_repeat(cg, value, len),
    }
}

#[inline]
fn codegen_const_void(cg: &Codegen) -> llvm::Value {
    llvm::const_struct_named(cg.void_val_type(), &[])
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

fn codegen_const_string(cg: &Codegen, string_lit: ast::StringLit) -> llvm::Value {
    let string_idx = string_lit.id.raw_index();
    let global_ptr = cg.string_lits[string_idx].as_ptr();

    if string_lit.c_string {
        global_ptr.as_val()
    } else {
        let string = cg.intern_lit.get(string_lit.id);
        let slice_len = cg.const_usize(string.len() as u64);
        llvm::const_struct_named(cg.slice_type(), &[global_ptr.as_val(), slice_len])
    }
}

fn codegen_const_variant(cg: &Codegen, variant: &hir::ConstVariant) -> llvm::Value {
    let enum_data = cg.hir.enum_data(variant.enum_id);
    let hir_variant = enum_data.variant(variant.variant_id);

    if enum_data.attr_set.contains(hir::EnumFlag::WithFields) {
        unimplemented!("constant variant with fields");
    }

    let variant_tag = match hir_variant.kind {
        hir::VariantKind::Default(id) => cg.hir.variant_tag_values[id.raw_index()],
        hir::VariantKind::Constant(id) => cg.hir.const_eval_value(id),
    };
    codegen_const(cg, variant_tag)
}

fn codegen_const_struct(cg: &Codegen, struct_: &hir::ConstStruct) -> llvm::Value {
    let mut values = Vec::with_capacity(struct_.value_ids.len());
    for &value_id in struct_.value_ids {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    let struct_ty = cg.struct_type(struct_.struct_id);
    llvm::const_struct_named(struct_ty, &values)
}

fn codegen_const_array(cg: &Codegen, array: &hir::ConstArray) -> llvm::Value {
    let mut values = Vec::with_capacity(array.len as usize);
    for &value_id in array.value_ids {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    let elem_ty = if let Some(val) = values.get(0) {
        llvm::typeof_value(*val)
    } else {
        cg.void_val_type().as_ty()
    };
    llvm::const_array(elem_ty, &values)
}

fn codegen_const_array_repeat(cg: &Codegen, value_id: hir::ConstValueID, len: u64) -> llvm::Value {
    let mut values = Vec::with_capacity(len as usize);
    let value = codegen_const(cg, cg.hir.const_value(value_id));
    values.resize(len as usize, value);

    let elem_ty = if let Some(val) = values.get(0) {
        llvm::typeof_value(*val)
    } else {
        cg.void_val_type().as_ty()
    };
    llvm::const_array(elem_ty, &values)
}

fn codegen_const_in_proc(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    expect: Expect,
    value: hir::ConstValue,
) -> llvm::Value {
    let value = codegen_const(cg, value);
    match expect {
        Expect::Value(_) | Expect::Store(_) => value,
        Expect::Pointer => {
            let temp_ptr = cg.entry_alloca(proc_cg, llvm::typeof_value(value), "temp_const");
            cg.build.store(value, temp_ptr);
            temp_ptr.as_val()
        }
    }
}

//@simplify
fn codegen_if<'c>(
    cg: &Codegen<'c, '_, '_>,
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

fn codegen_match<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    kind: hir::MatchKind,
    match_: &hir::Match<'c>,
) {
    match kind {
        hir::MatchKind::Int { .. } => {}
        hir::MatchKind::Bool => {}
        hir::MatchKind::Char => {}
        hir::MatchKind::String => unimplemented!("match on string"),
        hir::MatchKind::Enum { .. } => {
            codegen_match_enum(cg, proc_cg, expect, kind, match_);
            return;
        }
    }

    let on_value = codegen_expr_value(cg, proc_cg, match_.on_expr);
    let insert_bb = cg.build.insert_bb();

    let exit_bb = cg.append_bb(proc_cg, "match_exit");
    let mut wild_bb = None;
    let mut switch_cases = Vec::<(llvm::Value, llvm::BasicBlock)>::with_capacity(match_.arms.len());

    for arm in match_.arms {
        let arm_bb = cg.append_bb(proc_cg, "match_arm");
        cg.build.position_at_end(arm_bb);
        emit_stmt::codegen_block(cg, proc_cg, expect, arm.block);
        cg.build_br_no_term(exit_bb);

        match arm.pat {
            hir::Pat::Wild => {
                assert!(wild_bb.is_none());
                wild_bb = Some(arm_bb);
            }
            hir::Pat::Or(pats) => {
                for pat in pats {
                    match pat {
                        hir::Pat::Wild => {
                            assert!(wild_bb.is_none());
                            wild_bb = Some(arm_bb);
                        }
                        _ => {
                            let value = codegen_match_pat_value(cg, *pat);
                            switch_cases.push((value, arm_bb));
                        }
                    }
                }
            }
            _ => {
                let value = codegen_match_pat_value(cg, arm.pat);
                switch_cases.push((value, arm_bb));
            }
        };
    }

    cg.build.position_at_end(insert_bb);
    let else_bb = wild_bb.unwrap_or(exit_bb);
    let case_count = switch_cases.len() as u32;
    let switch = cg.build.switch(on_value, else_bb, case_count);

    for (case_val, dest_bb) in switch_cases {
        cg.build.add_case(switch, case_val, dest_bb);
    }
    cg.build.position_at_end(exit_bb);
}

fn codegen_match_pat_value<'c>(cg: &Codegen<'c, '_, '_>, pat: hir::Pat) -> llvm::Value {
    match pat {
        hir::Pat::Error => unreachable!(),
        hir::Pat::Wild => unreachable!(),
        hir::Pat::Lit(value) => codegen_const(cg, value),
        hir::Pat::Const(const_id) => codegen_const_var(cg, const_id),
        hir::Pat::Variant(_, _, _) => unimplemented!("match on enum"),
        hir::Pat::Or(_) => unreachable!(),
    }
}

fn codegen_match_enum<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    kind: hir::MatchKind,
    match_: &hir::Match<'c>,
) {
    let (enum_id, ref_mut) = match kind {
        hir::MatchKind::Enum { enum_id, ref_mut } => (enum_id, ref_mut),
        _ => unreachable!(),
    };

    let enum_data = cg.hir.enum_data(enum_id);
    let tag_ty = cg.basic_type(enum_data.tag_ty.resolved_unwrap().into_basic());
    //@always expect pointer, use sepate semantics for no inner value enums (same as variant_init)
    // right now all enum_init are capable to generate a pointer (via entry alloca)
    let enum_ptr = codegen_expr_pointer(cg, proc_cg, match_.on_expr);
    let tag_value = cg.build.load(tag_ty, enum_ptr, "enum_tag");
    let insert_bb = cg.build.insert_bb();

    let exit_bb = cg.append_bb(proc_cg, "match_exit");
    let mut wild_bb = None;
    let mut switch_cases = Vec::<(llvm::Value, llvm::BasicBlock)>::with_capacity(match_.arms.len());

    for arm in match_.arms {
        let arm_bb = cg.append_bb(proc_cg, "match_arm");
        cg.build.position_at_end(arm_bb);

        match arm.pat {
            hir::Pat::Error => todo!(),
            hir::Pat::Wild => {
                assert!(wild_bb.is_none());
                wild_bb = Some(arm_bb);
            }
            hir::Pat::Variant(_, variant_id, bind_ids) => {
                let variant = enum_data.variant(variant_id);
                let variant_tag = match variant.kind {
                    hir::VariantKind::Default(id) => cg.hir.variant_tag_values[id.raw_index()],
                    hir::VariantKind::Constant(id) => cg.hir.const_eval_value(id),
                };
                let variant_tag = codegen_const(cg, variant_tag);
                switch_cases.push((variant_tag, arm_bb));

                if !variant.fields.is_empty() {
                    let variant_ty = &cg.variants[enum_id.raw_index()];
                    let variant_ty = variant_ty[variant_id.raw_index()].expect("variant ty");

                    for bind_id in bind_ids.iter().copied() {
                        let proc_data = cg.hir.proc_data(proc_cg.proc_id);
                        let local_bind = proc_data.local_bind(bind_id);
                        let field_id = local_bind.field_id.unwrap().raw() + 1;

                        let field_ptr = cg.build.gep_struct(
                            variant_ty,
                            enum_ptr,
                            field_id,
                            "variant_field_ptr",
                        );
                        let local_ptr = proc_cg.local_bind_ptrs[bind_id.raw_index()];

                        if ref_mut.is_some() {
                            cg.build.store(field_ptr.as_val(), local_ptr);
                        } else {
                            let field_ty = cg.ty(local_bind.ty);
                            let field_val = cg.build.load(field_ty, field_ptr, "variant_field");
                            cg.build.store(field_val, local_ptr);
                        }
                    }
                }
            }
            hir::Pat::Or(pats) => {
                for pat in pats {
                    match *pat {
                        hir::Pat::Wild => {
                            assert!(wild_bb.is_none());
                            wild_bb = Some(arm_bb);
                        }
                        hir::Pat::Variant(_, variant_id, _) => {
                            let variant = enum_data.variant(variant_id);
                            let variant_tag = match variant.kind {
                                hir::VariantKind::Default(id) => {
                                    cg.hir.variant_tag_values[id.raw_index()]
                                }
                                hir::VariantKind::Constant(id) => cg.hir.const_eval_value(id),
                            };
                            let variant_tag = codegen_const(cg, variant_tag);
                            switch_cases.push((variant_tag, arm_bb));
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => unreachable!(),
        }

        emit_stmt::codegen_block(cg, proc_cg, expect, arm.block);
        cg.build_br_no_term(exit_bb);
    }

    cg.build.position_at_end(insert_bb);
    let else_bb = wild_bb.unwrap_or(exit_bb);
    let case_count = switch_cases.len() as u32;
    let switch = cg.build.switch(tag_value, else_bb, case_count);

    for (case_val, dest_bb) in switch_cases {
        cg.build.add_case(switch, case_val, dest_bb);
    }
    cg.build.position_at_end(exit_bb);
}

fn codegen_struct_field<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::StructFieldAccess,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
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
        Expect::Pointer => target_ptr.as_val(),
    }
}

fn codegen_slice_field<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::SliceFieldAccess,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
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
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    target: &hir::Expr<'c>,
    access: &hir::IndexAccess<'c>,
) -> llvm::Value {
    let target_ptr = codegen_expr_pointer(cg, proc_cg, target);
    let target_ptr = if access.deref.is_some() {
        cg.build
            .load(cg.ptr_type(), target_ptr, "deref_ptr")
            .into_ptr()
    } else {
        target_ptr
    };

    let index_val = codegen_expr_value(cg, proc_cg, access.index);

    let bound = match access.kind {
        hir::IndexKind::Multi(_) => None,
        hir::IndexKind::Slice(_) => {
            let slice_len_ptr =
                cg.build
                    .gep_struct(cg.slice_type(), target_ptr, 1, "slice_len_ptr");
            let len = cg
                .build
                .load(cg.ptr_sized_int(), slice_len_ptr, "slice_len");
            Some(len)
        }
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            Some(cg.const_usize(len))
        }
    };

    if let Some(bound) = bound {
        let check_bb = cg.append_bb(proc_cg, "bounds_check");
        let exit_bb = cg.append_bb(proc_cg, "bounds_exit");
        let cond = codegen_binary_op(cg, hir::BinOp::GreaterEq_IntU, index_val, bound);
        cg.build.cond_br(cond, check_bb, exit_bb);
        cg.build.position_at_end(check_bb);
        //@insert panic call (cannot get reference to it currently)
        cg.build.br(exit_bb); //@insert unrechable as hint?
        cg.build.position_at_end(exit_bb);
    }

    let elem_ty = cg.ty(access.elem_ty);
    let elem_ptr = match access.kind {
        hir::IndexKind::Multi(_) => {
            cg.build
                .gep(elem_ty, target_ptr, &[index_val], "multi_elem_ptr")
        }
        hir::IndexKind::Slice(_) => {
            let slice_ptr = cg
                .build
                .load(cg.ptr_type(), target_ptr, "slice_ptr")
                .into_ptr();
            cg.build
                .gep(elem_ty, slice_ptr, &[index_val], "slice_elem_ptr")
        }
        hir::IndexKind::Array(len) => {
            let len = cg.array_len(len);
            let array_ty = llvm::array_type(elem_ty, len);

            cg.build.gep(
                array_ty,
                target_ptr,
                &[cg.const_usize_zero(), index_val],
                "array_elem_ptr",
            )
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
    cg: &Codegen<'c, '_, '_>,
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

fn codegen_param_var(
    cg: &Codegen,
    proc_cg: &ProcCodegen,
    expect: Expect,
    param_id: hir::ParamID,
) -> llvm::Value {
    let param_ptr = proc_cg.param_ptrs[param_id.raw_index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let param = cg.hir.proc_data(proc_cg.proc_id).param(param_id);
            let param_ty = cg.ty(param.ty);
            cg.build.load(param_ty, param_ptr, "param_val")
        }
        Expect::Pointer => param_ptr.as_val(),
    }
}

fn codegen_local_var(
    cg: &Codegen,
    proc_cg: &ProcCodegen,
    expect: Expect,
    local_id: hir::LocalID,
) -> llvm::Value {
    let local_ptr = proc_cg.local_ptrs[local_id.raw_index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
            let local_ty = cg.ty(local.ty);
            cg.build.load(local_ty, local_ptr, "local_val")
        }
        Expect::Pointer => local_ptr.as_val(),
    }
}

fn codegen_local_bind_var(
    cg: &Codegen,
    proc_cg: &ProcCodegen,
    expect: Expect,
    local_bind_id: hir::LocalBindID,
) -> llvm::Value {
    let local_ptr = proc_cg.local_bind_ptrs[local_bind_id.raw_index()];

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let local = cg.hir.proc_data(proc_cg.proc_id).local_bind(local_bind_id);
            let local_ty = cg.ty(local.ty);
            cg.build.load(local_ty, local_ptr, "local_bind_val")
        }
        Expect::Pointer => local_ptr.as_val(),
    }
}

fn codegen_const_var(cg: &Codegen, const_id: hir::ConstID) -> llvm::Value {
    cg.consts[const_id.raw_index()]
}

fn codegen_global_var(cg: &Codegen, expect: Expect, global_id: hir::GlobalID) -> llvm::Value {
    let global_ptr = cg.globals[global_id.raw_index()].as_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let data = cg.hir.global_data(global_id);
            let global_ty = cg.ty(data.ty);
            cg.build.load(global_ty, global_ptr, "global_val")
        }
        Expect::Pointer => global_ptr.as_val(),
    }
}

//@after use same opts as struct_init
// expect store to avoid another alloca
fn codegen_variant<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    input: &&[&hir::Expr<'c>],
) -> llvm::Value {
    let enum_data = cg.hir.enum_data(enum_id);
    let variant = enum_data.variant(variant_id);

    //@generating each time
    let tag_value = match variant.kind {
        hir::VariantKind::Default(id) => cg.hir.variant_tag_values[id.raw_index()],
        hir::VariantKind::Constant(id) => cg.hir.const_eval_value(id),
    };
    let tag_value = codegen_const(cg, tag_value);

    if enum_data.attr_set.contains(hir::EnumFlag::WithFields) {
        let enum_ty = cg.enum_type(enum_id);
        let enum_ptr = cg.entry_alloca(proc_cg, enum_ty, "enum_init");
        cg.build.store(tag_value, enum_ptr);

        if !variant.fields.is_empty() {
            let variant_ty = &cg.variants[enum_id.raw_index()];
            let variant_ty = variant_ty[variant_id.raw_index()].expect("variant ty");

            for (idx, expr) in input.iter().enumerate() {
                let field_ptr =
                    cg.build
                        .gep_struct(variant_ty, enum_ptr, idx as u32 + 1, "variant_field_ptr");
                codegen_expr_store(cg, proc_cg, expr, field_ptr);
            }
        }

        match expect {
            Expect::Value(_) | Expect::Store(_) => cg.build.load(enum_ty, enum_ptr, "enum_value"),
            Expect::Pointer => enum_ptr.as_val(),
        }
    } else {
        match expect {
            Expect::Value(_) | Expect::Store(_) => tag_value,
            Expect::Pointer => {
                let enum_ptr = cg.entry_alloca(proc_cg, llvm::typeof_value(tag_value), "enum_init");
                cg.build.store(tag_value, enum_ptr);
                enum_ptr.as_val()
            }
        }
    }
}

fn codegen_call_direct<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    proc_id: hir::ProcID,
    input: &[&hir::Expr<'c>],
) -> Option<llvm::Value> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr_value(cg, proc_cg, expr);
        input_values.push(value);
    }

    let (fn_val, fn_ty) = cg.procs[proc_id.raw_index()];
    cg.build.call(fn_ty, fn_val, &input_values, "call_val")
}

fn codegen_call_indirect<'c>(
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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

    let copied_val = codegen_expr_value(cg, proc_cg, array_repeat.value);
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
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    rhs: &hir::Expr<'c>,
    ref_ty: hir::Type,
) -> llvm::Value {
    let ptr_val = codegen_expr_value(cg, proc_cg, rhs).into_ptr();

    match expect {
        Expect::Value(_) | Expect::Store(_) => {
            let ptr_ty = cg.ty(ref_ty);
            cg.build.load(ptr_ty, ptr_val, "deref_val")
        }
        Expect::Pointer => ptr_val.as_val(),
    }
}

fn codegen_address<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    codegen_expr_pointer(cg, proc_cg, rhs).as_val()
}

fn codegen_unary<'c>(
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
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

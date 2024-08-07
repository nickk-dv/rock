use super::context::{Codegen, Expect, ProcCodegen};
use super::emit_expr;
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::fs_env;
use rock_core::hir;

pub fn codegen_module<'c>(hir: hir::Hir<'c>) {
    let mut cg = Codegen::new(hir);
    codegen_string_lits(&mut cg);
    codegen_struct_types(&mut cg);
    codegen_consts(&mut cg);
    codegen_globals(&mut cg);
    codegen_function_values(&mut cg);
    codegen_function_bodies(&mut cg);

    if let Ok(mut cwd) = fs_env::dir_get_current_working() {
        cwd.push("module.ll");
        let string = cg.module.to_string();
        let _ = fs_env::file_create_or_rewrite(&cwd, &string);
    } else {
        //@temp panic, writing ll module is feature of --emit-llvm
        panic!("failed to get cwd");
    }

    //@temp err handling
    if let Err(error) = cg.module.verify() {
        eprintln!("module verify failed:\n{}", error);
    }
}

fn codegen_string_lits(cg: &mut Codegen) {
    for (idx, &string) in cg.hir.intern_string.get_all_strings().iter().enumerate() {
        let c_string = cg.hir.string_is_cstr[idx];
        let str_val = llvm::const_string(string, c_string);
        let str_ty = llvm::typeof_value(str_val);

        let global = cg.module.add_global(
            str_ty,
            "rock_string_lit",
            str_val,
            true,
            true,
            false,
            llvm::Linkage::LLVMInternalLinkage,
        );
        cg.string_lits.push(global);
    }
}

fn codegen_struct_types(cg: &mut Codegen) {
    for _ in 0..cg.hir.structs.len() {
        let opaque = cg.context.struct_create_named("rock_struct");
        cg.structs.push(opaque);
    }

    let mut field_types = Vec::with_capacity(64);

    for (idx, struct_data) in cg.hir.structs.iter().enumerate() {
        field_types.clear();
        for field in struct_data.fields {
            field_types.push(cg.ty(field.ty));
        }
        let opaque = cg.structs[idx];
        cg.context.struct_set_body(opaque, &field_types, false)
    }
}

//@instead codegen all constants in the pool
fn codegen_consts(cg: &mut Codegen) {
    for data in cg.hir.consts.iter() {
        let const_val = emit_expr::codegen_const(cg, cg.hir.const_eval_value(data.value));
        cg.consts.push(const_val);
    }
}

fn codegen_globals(cg: &mut Codegen) {
    for data in cg.hir.globals.iter() {
        let global_ty = cg.ty(data.ty);
        let global_val = emit_expr::codegen_const(cg, cg.hir.const_eval_value(data.value));

        let global = cg.module.add_global(
            global_ty,
            "rock_global",
            global_val,
            data.mutt == ast::Mut::Immutable,
            false,
            data.attr_set.contains(hir::GlobalFlag::ThreadLocal),
            llvm::Linkage::LLVMInternalLinkage,
        );
        cg.globals.push(global);
    }
}

fn codegen_function_values(cg: &mut Codegen) {
    let mut param_types = Vec::with_capacity(64);

    for data in cg.hir.procs.iter() {
        param_types.clear();
        for param in data.params {
            param_types.push(cg.ty(param.ty));
        }

        let is_external = data.attr_set.contains(hir::ProcFlag::External);
        let is_variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
        let is_main = data.attr_set.contains(hir::ProcFlag::Main);

        let name = if is_external || is_main {
            cg.hir.intern_name.get_str(data.name.id)
        } else {
            "rock_proc"
        };
        let linkage = if is_main || is_external {
            llvm::Linkage::LLVMExternalLinkage
        } else {
            llvm::Linkage::LLVMInternalLinkage
        };

        let fn_ty = llvm::function_type(cg.ty(data.return_ty), &param_types, is_variadic);
        let fn_val = cg.module.add_function(name, fn_ty, linkage);
        cg.procs.push((fn_val, fn_ty));
    }
}

//@reuse param & local ptr value vectors
fn codegen_function_bodies(cg: &Codegen) {
    for (idx, data) in cg.hir.procs.iter().enumerate() {
        let block = match data.block {
            Some(block) => block,
            None => continue,
        };

        let fn_val = cg.procs[idx].0;
        let mut proc_cg = ProcCodegen::new(hir::ProcID::new(idx), fn_val);

        let entry_bb = cg.context.append_bb(fn_val, "entry_bb");
        cg.build.position_at_end(entry_bb);

        for param_idx in 0..data.params.len() {
            let param_val = fn_val.param_val(param_idx as u32).unwrap();
            let param_ty = llvm::typeof_value(param_val);

            let param_ptr = cg.build.alloca(param_ty, "param");
            cg.build.store(param_val, param_ptr);
            proc_cg.param_ptrs.push(param_ptr);
        }

        for &local in data.locals {
            let local_ty = cg.ty(local.ty);

            let local_ptr = cg.build.alloca(local_ty, "local");
            proc_cg.local_ptrs.push(local_ptr);
        }

        let value_id = proc_cg.add_tail_value();
        emit_stmt::codegen_block(cg, &mut proc_cg, Expect::Value(Some(value_id)), block);

        let value = if let Some(tail) = proc_cg.tail_value(value_id) {
            Some(cg.build.load(tail.value_ty, tail.value_ptr, "tail_val"))
        } else {
            None
        };
        if !cg.insert_bb_terminated() {
            cg.build.ret(value);
        }
    }
}

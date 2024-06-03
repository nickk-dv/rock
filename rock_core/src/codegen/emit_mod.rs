use super::context::{Codegen, ProcCodegen};
use super::emit_expr::{codegen_block_value_optional, codegen_const_value};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use inkwell::module;
use inkwell::targets;
use inkwell::types::BasicType;

pub fn codegen_module<'ctx>(
    hir: hir::Hir<'ctx>,
    context_llvm: &'ctx inkwell::context::Context,
) -> Result<(module::Module<'ctx>, targets::TargetMachine), ErrorComp> {
    let mut cg = Codegen::new(hir, &context_llvm);
    codegen_string_literals(&mut cg);
    codegen_struct_types(&mut cg);
    codegen_consts(&mut cg);
    codegen_globals(&mut cg);
    codegen_function_values(&mut cg);
    codegen_function_bodies(&cg);
    cg.finish_module()
}

fn codegen_string_literals(cg: &mut Codegen) {
    for (idx, &string) in cg.hir.intern_string.get_all_strings().iter().enumerate() {
        let c_string = cg.hir.string_is_cstr[idx];
        let array_value = cg.context.const_string(string.as_bytes(), c_string);
        let array_ty = array_value.get_type();

        let global = cg.module.add_global(array_ty, None, "rock_string_lit");
        global.set_linkage(module::Linkage::Internal);
        global.set_constant(true);
        global.set_unnamed_addr(true);
        global.set_initializer(&array_value);
        cg.string_lits.push(global);
    }
}

fn codegen_struct_types(cg: &mut Codegen) {
    for _ in 0..cg.hir.structs.len() {
        let opaque = cg.context.opaque_struct_type("rock_struct");
        cg.structs.push(opaque);
    }

    const EXPECT_FIELD_COUNT: usize = 64;
    let mut field_types = Vec::with_capacity(EXPECT_FIELD_COUNT);

    for (idx, struct_data) in cg.hir.structs.iter().enumerate() {
        field_types.clear();
        for field in struct_data.fields {
            field_types.push(cg.type_into_basic(field.ty));
        }
        let opaque = cg.structs[idx];
        opaque.set_body(&field_types, false);
    }
}

fn codegen_consts(cg: &mut Codegen) {
    for data in cg.hir.consts.iter() {
        let value = codegen_const_value(cg, cg.hir.const_eval_value(data.value));
        cg.consts.push(value);
    }
}

fn codegen_globals(cg: &mut Codegen) {
    for data in cg.hir.globals.iter() {
        let global_ty = cg.type_into_basic(data.ty);
        let value = codegen_const_value(cg, cg.hir.const_eval_value(data.value));

        let global = cg.module.add_global(global_ty, None, "rock_global");
        global.set_linkage(module::Linkage::Internal);
        global.set_constant(data.mutt == ast::Mut::Immutable);
        global.set_thread_local(data.thread_local);
        global.set_initializer(&value);
        cg.globals.push(global);
    }
}

fn codegen_function_values(cg: &mut Codegen) {
    let mut param_types = Vec::new();
    for proc_data in cg.hir.procs.iter() {
        param_types.clear();

        for param in proc_data.params {
            param_types.push(cg.type_into_basic_metadata(param.ty));
        }

        //@repeated in Codegen ProcType generation 29.05.24
        let function_ty = match cg.type_into_basic_option(proc_data.return_ty) {
            Some(ty) => ty.fn_type(&param_types, proc_data.is_variadic),
            None => cg
                .context
                .void_type()
                .fn_type(&param_types, proc_data.is_variadic),
        };

        let name = cg.hir.intern_name.get_str(proc_data.name.id);

        //@switch to explicit main flag on proc_data or store ProcID of the entry point in hir instead 29.05.24
        // module of main being 0 is not stable, might put core library as the first Package / Module thats processed
        let is_main = proc_data.origin_id == hir::ModuleID::new(0) && name == "main";
        let is_c_call = proc_data.block.is_none();

        let name = if is_main || is_c_call {
            name
        } else {
            "rock_proc"
        };
        let linkage = if is_main || is_c_call {
            module::Linkage::External
        } else {
            module::Linkage::Internal
        };

        let function = cg.module.add_function(name, function_ty, Some(linkage));
        if proc_data.block.is_none() {
            cg.c_functions.insert(proc_data.name.id, function);
        }
        cg.function_values.push(function);
    }
}

fn codegen_function_bodies(cg: &Codegen) {
    for (idx, proc_data) in cg.hir.procs.iter().enumerate() {
        let block = if let Some(block) = proc_data.block {
            block
        } else {
            continue;
        };

        let function = cg.function_values[idx];

        let entry_block = cg.context.append_basic_block(function, "entry");
        cg.builder.position_at_end(entry_block);

        let mut param_vars = Vec::with_capacity(proc_data.params.len());
        for param_idx in 0..proc_data.params.len() {
            let param_value = function
                .get_nth_param(param_idx as u32)
                .expect("param value");
            let param_ty = param_value.get_type();
            let param_ptr = cg.builder.build_alloca(param_ty, "param").unwrap();
            cg.builder.build_store(param_ptr, param_value).unwrap();
            param_vars.push(param_ptr);
        }

        let mut local_vars = Vec::with_capacity(proc_data.locals.len());
        for &local in proc_data.locals {
            let local_ty = cg.type_into_basic(local.ty);
            let local_ptr = cg.builder.build_alloca(local_ty, "local").unwrap();
            local_vars.push(local_ptr);
        }

        let mut proc_cg = ProcCodegen {
            function,
            proc_id: hir::ProcID::new(idx),
            param_vars,
            local_vars,
            block_info: Vec::new(),
            defer_blocks: Vec::new(),
            next_loop_info: None,
            tail_alloca: Vec::with_capacity(64),
        };

        let block_value = codegen_block_value_optional(cg, &mut proc_cg, block);
        if !cg.insert_bb_has_term() {
            cg.build_ret(block_value);
        }
    }
}

use super::context::{Codegen, Expect};
use super::emit_expr;
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::config::TargetTriple;
use rock_core::hir;
use rock_core::session::Session;

pub fn codegen_module<'c, 's, 's_ref>(
    hir: hir::Hir<'c>,
    target: TargetTriple,
    session: &'s_ref Session<'s>,
) -> (llvm::IRTarget, llvm::IRModule) {
    let mut cg = Codegen::new(hir, target, session);
    codegen_string_lits(&mut cg);
    codegen_enum_types(&mut cg);
    codegen_struct_types(&mut cg);
    codegen_variant_types(&mut cg);
    codegen_consts(&mut cg);
    codegen_globals(&mut cg);
    codegen_function_values(&mut cg);
    codegen_function_bodies(&mut cg);
    (cg.target, cg.module)
}

//@NOTE(8.12.24) currently always null terminate, no "used as cstring" tracking in the compiler.
fn codegen_string_lits(cg: &mut Codegen) {
    for string in cg.session.intern_lit.get_all().iter().copied() {
        let string_val = llvm::const_string(&cg.context, string, true);
        let string_ty = llvm::typeof_value(string_val);

        let global = cg
            .module
            .add_global("rock.string", string_val, string_ty, true, true);
        cg.string_lits.push(global);
    }
}

fn codegen_enum_types(cg: &mut Codegen) {
    for enum_id in (0..cg.hir.enums.len()).map(hir::EnumID::new) {
        let enum_data = cg.hir.enum_data(enum_id);

        let enum_ty = if enum_data.flag_set.contains(hir::EnumFlag::WithFields) {
            let layout = enum_data.layout.resolved_unwrap();
            //@bad api, forced to create hir::ArrayStatic
            let array_ty = hir::ArrayStatic {
                len: hir::ArrayStaticLen::Immediate(layout.size()),
                elem_ty: hir::Type::Basic(ast::BasicType::U8),
            };
            let array_ty = cg.array_type(&array_ty);

            let module_origin = cg.session.module.get(enum_data.origin_id);
            let package_origin = cg.session.graph.package(module_origin.origin());
            let package_name = cg.session.intern_name.get(package_origin.name());
            let enum_name = cg.session.intern_name.get(enum_data.name.id);

            cg.string_buf.clear();
            cg.string_buf.push_str(package_name);
            cg.string_buf.push(':');
            cg.string_buf.push_str(enum_name);

            let enum_ty = cg.context.struct_create_named(&cg.string_buf);
            cg.context.struct_set_body(enum_ty, &[array_ty], false);
            enum_ty.as_ty()
        } else {
            let tag_ty = enum_data.tag_ty.resolved_unwrap();
            cg.basic_type(tag_ty.into_basic())
        };

        cg.enums.push(enum_ty);
    }
}

fn codegen_struct_types(cg: &mut Codegen) {
    for struct_id in (0..cg.hir.structs.len()).map(hir::StructID::new) {
        let struct_data = cg.hir.struct_data(struct_id);
        let module_origin = cg.session.module.get(struct_data.origin_id);
        let package_origin = cg.session.graph.package(module_origin.origin());
        let package_name = cg.session.intern_name.get(package_origin.name());
        let struct_name = cg.session.intern_name.get(struct_data.name.id);

        cg.string_buf.clear();
        cg.string_buf.push_str(package_name);
        cg.string_buf.push(':');
        cg.string_buf.push_str(struct_name);

        let opaque = cg.context.struct_create_named(&cg.string_buf);
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

//@non optimized memory storage for variant type info
fn codegen_variant_types(cg: &mut Codegen) {
    for enum_id in (0..cg.hir.enums.len()).map(hir::EnumID::new) {
        let enum_data = cg.hir.enum_data(enum_id);

        if enum_data.flag_set.contains(hir::EnumFlag::WithFields) {
            let mut variant_types = Vec::with_capacity(enum_data.variants.len());
            let enum_name = cg.session.intern_name.get(enum_data.name.id);

            for variant in enum_data.variants {
                if variant.fields.is_empty() {
                    variant_types.push(None);
                } else {
                    let mut field_types = Vec::with_capacity(variant.fields.len());
                    let tag_ty = cg.basic_type(enum_data.tag_ty.resolved_unwrap().into_basic());
                    field_types.push(tag_ty);
                    for field in variant.fields {
                        field_types.push(cg.ty(field.ty));
                    }

                    let module_origin = cg.session.module.get(enum_data.origin_id);
                    let package_origin = cg.session.graph.package(module_origin.origin());
                    let package_name = cg.session.intern_name.get(package_origin.name());
                    let variant_name = cg.session.intern_name.get(variant.name.id);

                    cg.string_buf.clear();
                    cg.string_buf.push_str(package_name);
                    cg.string_buf.push(':');
                    cg.string_buf.push_str(enum_name);
                    cg.string_buf.push('.');
                    cg.string_buf.push_str(variant_name);

                    let variant_ty = cg.context.struct_create_named(&cg.string_buf);
                    cg.context.struct_set_body(variant_ty, &field_types, false);
                    variant_types.push(Some(variant_ty));
                }
            }
            cg.variants.push(variant_types);
        } else {
            cg.variants.push(Vec::new());
        }
    }
}

fn codegen_consts(cg: &mut Codegen) {
    for data in cg.hir.consts.iter() {
        let value = emit_expr::codegen_const(cg, cg.hir.const_eval_values[data.value.index()]);
        cg.consts.push(value);
    }
}

fn codegen_globals(cg: &mut Codegen) {
    for data in cg.hir.globals.iter() {
        let global_ty = cg.ty(data.ty);
        let global_val = match data.init {
            hir::GlobalInit::Init(eval_id) => {
                emit_expr::codegen_const(cg, cg.hir.const_eval_values[eval_id.index()])
            }
            hir::GlobalInit::Zeroed => llvm::const_all_zero(global_ty),
        };

        let module = cg.session.module.get(data.origin_id);
        let package = cg.session.graph.package(module.origin());
        let package_name = cg.session.intern_name.get(package.name());
        let struct_name = cg.session.intern_name.get(data.name.id);
        cg.string_buf.clear();
        cg.string_buf.push_str(package_name);
        cg.string_buf.push(':');
        cg.string_buf.push_str(struct_name);

        let global = cg.module.add_global(
            &cg.string_buf,
            global_val,
            global_ty,
            data.mutt == ast::Mut::Immutable,
            false,
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
        if data.flag_set.contains(hir::ProcFlag::CallerLocation) {
            param_types.push(cg.location_ty.as_ty());
        }

        //builtin takes precedence over external flag
        let is_external = data.flag_set.contains(hir::ProcFlag::External)
            && !data.flag_set.contains(hir::ProcFlag::Builtin);
        let is_variadic = data.flag_set.contains(hir::ProcFlag::Variadic);
        let is_entry = data.flag_set.contains(hir::ProcFlag::EntryPoint);

        let name = if is_external || is_entry {
            cg.session.intern_name.get(data.name.id)
        } else {
            let module_origin = cg.session.module.get(data.origin_id);
            let module_name = cg.session.intern_name.get(module_origin.name());
            let package_origin = cg.session.graph.package(module_origin.origin());
            let package_name = cg.session.intern_name.get(package_origin.name());
            let proc_name = cg.session.intern_name.get(data.name.id);

            cg.string_buf.clear();
            cg.string_buf.push_str(package_name);
            cg.string_buf.push(':');
            cg.string_buf.push_str(module_name);
            cg.string_buf.push(':');
            cg.string_buf.push_str(proc_name);
            cg.string_buf.as_str()
        };

        let linkage = if is_external || is_entry {
            llvm::Linkage::LLVMExternalLinkage
        } else {
            llvm::Linkage::LLVMInternalLinkage
        };

        let return_ty = match data.return_ty {
            hir::Type::Basic(ast::BasicType::Void | ast::BasicType::Never) => cg.void_type(),
            _ => cg.ty(data.return_ty),
        };

        let fn_ty = llvm::function_type(return_ty, &param_types, is_variadic);
        let fn_val = cg.module.add_function(name, fn_ty, linkage);

        if is_external || is_entry {
            fn_val.set_call_conv(llvm::CallConv::LLVMCCallConv);
        } else {
            fn_val.set_call_conv(llvm::CallConv::LLVMFastCallConv);
        }
        if data.flag_set.contains(hir::ProcFlag::Inline) {
            fn_val.set_attr(cg.attr_cache.inlinehint);
        }
        if data.return_ty.is_never() {
            fn_val.set_attr(cg.attr_cache.noreturn);
        }

        cg.procs.push((fn_val, fn_ty));
    }
}

fn codegen_function_bodies(cg: &mut Codegen) {
    for proc_idx in 0..cg.hir.procs.len() {
        let fn_val = cg.procs[proc_idx].0;
        let proc_id = hir::ProcID::new(proc_idx);
        let data = cg.hir.proc_data(proc_id);
        cg.proc.reset(proc_id, fn_val);

        if data.flag_set.contains(hir::ProcFlag::External)
            && !data.flag_set.contains(hir::ProcFlag::Builtin)
        {
            continue;
        }

        let entry_bb = cg.context.append_bb(fn_val, "entry_bb");
        cg.build.position_at_end(entry_bb);

        for (param_idx, param) in data.params.iter().enumerate() {
            let name = cg.session.intern_name.get(param.name.id);
            cg.string_buf.clear();
            cg.string_buf.push_str(name);

            let param_ty = cg.ty(param.ty);
            let param_ptr = cg.build.alloca(param_ty, &cg.string_buf);
            cg.proc.param_ptrs.push(param_ptr);

            let param_val = fn_val.param_val(param_idx as u32);
            cg.build.store(param_val, param_ptr);
        }

        if data.flag_set.contains(hir::ProcFlag::CallerLocation) {
            let param_idx = data.params.len();
            let param_ty = cg.location_ty.as_ty();
            let param_ptr = cg.build.alloca(param_ty, "caller_location");
            cg.proc.param_ptrs.push(param_ptr);

            let param_val = fn_val.param_val(param_idx as u32);
            cg.build.store(param_val, param_ptr);
        }

        for local in data.locals {
            let name = cg.session.intern_name.get(local.name.id);
            cg.string_buf.clear();
            cg.string_buf.push_str(name);

            let local_ty = cg.ty(local.ty);
            let local_ptr = cg.build.alloca(local_ty, &cg.string_buf);
            cg.proc.local_ptrs.push(local_ptr);
        }

        for local_bind in data.local_binds {
            let name = cg.session.intern_name.get(local_bind.name.id);
            cg.string_buf.clear();
            cg.string_buf.push_str("bind:");
            cg.string_buf.push_str(name);

            let local_ty = cg.ty(local_bind.ty);
            let local_ptr = cg.build.alloca(local_ty, name);
            cg.proc.local_bind_ptrs.push(local_ptr);
        }

        for for_bind in data.for_binds {
            let name = if for_bind.name.id.raw() == u32::MAX {
                "bind:index(_)"
            } else {
                cg.session.intern_name.get(for_bind.name.id)
            };
            cg.string_buf.clear();
            cg.string_buf.push_str("bind:");
            cg.string_buf.push_str(name);

            let local_ty = cg.ty(for_bind.ty);
            let local_ptr = cg.build.alloca(local_ty, name);
            cg.proc.for_bind_ptrs.push(local_ptr);
        }

        if let Some(block) = data.block {
            let value_id = cg.proc.add_tail_value();
            emit_stmt::codegen_block(cg, Expect::Value(Some(value_id)), block);

            let value = if let Some(tail) = cg.proc.tail_value(value_id) {
                Some(cg.build.load(tail.value_ty, tail.value_ptr, "tail_val"))
            } else {
                None
            };
            if !cg.insert_bb_terminated() {
                cg.build.ret(value);
            }
        } else if data.flag_set.contains(hir::ProcFlag::Builtin) {
            //@hack: generate slice builtins (all are the same)
            let slice_ty = cg.slice_type();
            let slice_ptr = cg.build.alloca(slice_ty.as_ty(), "slice");
            let param_ptr_ptr = cg.proc.param_ptrs[0];
            let param_len_ptr = cg.proc.param_ptrs[1];

            let slice_ptr_ptr = cg.build.gep_struct(slice_ty, slice_ptr, 0, "slice_ptr_ptr");
            let param_ptr = cg.build.load(cg.ptr_type(), param_ptr_ptr, "param_ptr");
            cg.build.store(param_ptr, slice_ptr_ptr);

            let slice_len_ptr = cg.build.gep_struct(slice_ty, slice_ptr, 1, "slice_len_ptr");
            let param_len = cg
                .build
                .load(cg.ptr_sized_int(), param_len_ptr, "param_len");
            cg.build.store(param_len, slice_len_ptr);

            let slice_val = cg.build.load(slice_ty.as_ty(), slice_ptr, "slice_val");
            cg.build.ret(Some(slice_val));
        }
    }
}

use super::context::{self, Codegen, Expect};
use super::emit_expr;
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::hir;
use rock_core::session::{config, Session};

pub fn codegen_module(
    hir: hir::Hir,
    target: config::TargetTriple,
    session: &mut Session,
) -> (llvm::IRTarget, llvm::IRModule) {
    let mut cg = Codegen::new(hir, target, session);
    codegen_string_lits(&mut cg);
    codegen_enum_types(&mut cg);
    codegen_struct_types(&mut cg);
    codegen_function_values(&mut cg);
    codegen_globals(&mut cg);
    codegen_function_bodies(&mut cg);
    codegen_type_info(&mut cg);
    (cg.target, cg.module)
}

fn codegen_string_lits(cg: &mut Codegen) {
    //prepare possible required lit_id's
    cg.session.intern_lit.intern("index out of bounds");
    for module_id in cg.session.module.ids() {
        let _ = hir::source_location(cg.session, module_id, 0.into());
    }

    for string in cg.session.intern_lit.get_all().iter().copied() {
        let string_val = llvm::const_string(&cg.context, string, true);
        let string_ty = llvm::typeof_value(string_val);
        let global = cg.module.add_global("rock.string", Some(string_val), string_ty, true, true);
        cg.string_lits.push(global);
    }
}

fn codegen_enum_types(cg: &mut Codegen) {
    for enum_id in (0..cg.hir.enums.len()).map(hir::EnumID::new) {
        let data = cg.hir.enum_data(enum_id);

        let enum_ty = if data.flag_set.contains(hir::EnumFlag::WithFields) {
            cg.namebuf.clear();
            context::write_symbol_name(cg, data.name.id, data.origin_id, &[]);
            let enum_ty = cg.context.struct_named_create(&cg.namebuf);

            let data = cg.hir.enum_data(enum_id);
            if data.poly_params.is_none() {
                let layout = data.layout.resolved_unwrap();
                let elem_ty = cg.int_type(hir::IntType::U8);
                let array_ty = llvm::array_type(elem_ty, layout.size);
                cg.context.struct_named_set_body(enum_ty, &[array_ty], false);
            }
            enum_ty
        } else {
            cg.int_type(data.tag_ty.resolved_unwrap()).as_st()
        };

        cg.enums.push(enum_ty);
    }
}

fn codegen_struct_types(cg: &mut Codegen) {
    for struct_id in (0..cg.hir.structs.len()).map(hir::StructID::new) {
        let data = cg.hir.struct_data(struct_id);
        cg.namebuf.clear();
        context::write_symbol_name(cg, data.name.id, data.origin_id, &[]);
        let opaque = cg.context.struct_named_create(&cg.namebuf);
        cg.structs.push(opaque);
    }

    let mut field_types = Vec::with_capacity(64);

    for struct_id in (0..cg.hir.structs.len()).map(hir::StructID::new) {
        let data = cg.hir.struct_data(struct_id);
        if data.poly_params.is_some() {
            continue;
        }

        field_types.clear();
        for field in data.fields {
            field_types.push(cg.ty(field.ty));
        }
        let opaque = cg.structs[struct_id.index()];
        cg.context.struct_named_set_body(opaque, &field_types, false)
    }
}

fn codegen_globals(cg: &mut Codegen) {
    for global_id in (0..cg.hir.globals.len()).map(hir::GlobalID::new) {
        let data = cg.hir.global_data(global_id);
        let value = match data.init {
            hir::GlobalInit::Init(eval_id) => {
                let value = cg.hir.const_eval_values[eval_id.index()];
                if emit_expr::const_has_variant_with_ptrs(value, false) {
                    emit_expr::const_writer(cg, value)
                } else {
                    emit_expr::codegen_const(cg, value)
                }
            }
            hir::GlobalInit::Zeroed => llvm::const_zeroed(cg.ty(data.ty)),
        };

        let data = cg.hir.global_data(global_id);
        let global_ty = llvm::typeof_value(value);
        let constant = data.mutt == ast::Mut::Immutable;

        cg.namebuf.clear();
        context::write_symbol_name(cg, data.name.id, data.origin_id, &[]);
        let global = cg.module.add_global(&cg.namebuf, Some(value), global_ty, constant, false);
        cg.globals.push(global);
    }

    codegen_type_id(cg, hir::Type::Char);
    codegen_type_id(cg, hir::Type::Void);
    codegen_type_id(cg, hir::Type::Never);
    codegen_type_id(cg, hir::Type::Rawptr);
    codegen_type_id(cg, hir::Type::Int(hir::IntType::S8));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::S16));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::S32));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::S64));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::Ssize));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::U8));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::U16));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::U32));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::U64));
    codegen_type_id(cg, hir::Type::Int(hir::IntType::Usize));
    codegen_type_id(cg, hir::Type::Float(hir::FloatType::F32));
    codegen_type_id(cg, hir::Type::Float(hir::FloatType::F64));
    codegen_type_id(cg, hir::Type::Bool(hir::BoolType::Bool));
    codegen_type_id(cg, hir::Type::Bool(hir::BoolType::Bool16));
    codegen_type_id(cg, hir::Type::Bool(hir::BoolType::Bool32));
    codegen_type_id(cg, hir::Type::Bool(hir::BoolType::Bool64));
    codegen_type_id(cg, hir::Type::String(hir::StringType::String));
    codegen_type_id(cg, hir::Type::String(hir::StringType::CString));

    cg.type_info_ptr = cg.module.add_global("type_info.ptr", None, cg.ptr_type(), true, false);
}

pub fn codegen_type_id<'c>(cg: &mut Codegen<'c, '_, '_>, ty: hir::Type<'c>) -> u64 {
    *cg.type_ids.entry(ty).or_insert_with(|| {
        let type_id = cg.type_id_types.len() as u64;
        cg.type_id_types.push(ty);
        type_id
    })
}

//@regular lang doesnt support constant references,
// will likely need custom writing logic to represent type info array
fn codegen_type_info(cg: &mut Codegen) {
    let enum_id = cg.hir.core.type_info;
    let mut values = Vec::with_capacity(cg.type_id_types.len());

    for ty in cg.type_id_types.iter().copied() {
        let value = match ty {
            hir::Type::Char => {
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(0),
                    values: &[],
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Void => {
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(1),
                    values: &[],
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Never => {
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(2),
                    values: &[],
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Rawptr => {
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(3),
                    values: &[],
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Int(ty) => {
                let values = cg.hir.arena.alloc_slice(&[hir::ConstValue::Variant {
                    enum_id: cg.hir.core.int_ty,
                    variant_id: hir::VariantID::new(ty as usize),
                }]);
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(4),
                    values,
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Float(ty) => {
                let values = cg.hir.arena.alloc_slice(&[hir::ConstValue::Variant {
                    enum_id: cg.hir.core.float_ty,
                    variant_id: hir::VariantID::new(ty as usize),
                }]);
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(5),
                    values,
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::Bool(ty) => {
                let values = cg.hir.arena.alloc_slice(&[hir::ConstValue::Variant {
                    enum_id: cg.hir.core.bool_ty,
                    variant_id: hir::VariantID::new(ty as usize),
                }]);
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(6),
                    values,
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            hir::Type::String(ty) => {
                let values = cg.hir.arena.alloc_slice(&[hir::ConstValue::Variant {
                    enum_id: cg.hir.core.string_ty,
                    variant_id: hir::VariantID::new(ty as usize),
                }]);
                let variant = hir::ConstVariant {
                    variant_id: hir::VariantID::new(7),
                    values,
                    poly_types: &[],
                };
                let variant = cg.hir.arena.alloc(variant);
                hir::ConstValue::VariantPoly { enum_id, variant }
            }
            _ => unimplemented!("unsupported type info: {:?}", ty),
        };
        values.push(value);
    }

    let values = cg.hir.arena.alloc_slice(&values);
    let array = cg.hir.arena.alloc(hir::ConstArray { values });
    let array = hir::ConstValue::Array { array: &array };
    let types = emit_expr::const_writer(cg, array);

    cg.type_info_arr =
        cg.module.add_global("type_info.arr", Some(types), llvm::typeof_value(types), true, false);
    cg.module.init_global(cg.type_info_ptr, cg.type_info_arr.as_ptr().as_val());
}

fn codegen_function_values(cg: &mut Codegen) {
    for proc_id in (0..cg.hir.procs.len()).map(hir::ProcID::new) {
        if cg.hir.proc_data(proc_id).poly_params.is_none() {
            let fn_res = codegen_function_value(cg, proc_id, &[]);
            cg.procs.push(fn_res);
        } else {
            cg.procs.push((llvm::ValueFn::null(), llvm::TypeFn::null()));
        }
    }
}

fn codegen_function_bodies(cg: &mut Codegen) {
    for proc_id in (0..cg.hir.procs.len()).map(hir::ProcID::new) {
        if cg.hir.proc_data(proc_id).poly_params.is_none() {
            codegen_function_body(cg, proc_id, &[]);
        }
    }
    while let Some((proc_id, poly_types)) = cg.poly_proc_queue.pop() {
        codegen_function_body(cg, proc_id, poly_types);
    }
}

pub fn codegen_function_value<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    proc_id: hir::ProcID,
    poly_types: &'c [hir::Type<'c>],
) -> (llvm::ValueFn, llvm::TypeFn) {
    //@hack setting these poly_types to be used from PolyProc types in proc definition
    let curr_poly = cg.proc.poly_types;
    cg.proc.poly_types = poly_types;

    let data = cg.hir.proc_data(proc_id);
    let is_external = data.flag_set.contains(hir::ProcFlag::External);
    let is_variadic = data.flag_set.contains(hir::ProcFlag::CVariadic);
    let is_entry = data.flag_set.contains(hir::ProcFlag::EntryPoint);
    let mut by_pointer_ret = false;

    let offset = cg.cache.types.start();
    let return_ty = match data.return_ty {
        hir::Type::Void | hir::Type::Never => cg.void_type(),
        _ => {
            if is_external {
                let abi = win_x64_parameter_type(cg, data.return_ty);
                if abi.by_pointer {
                    by_pointer_ret = true;
                    cg.cache.types.push(cg.ptr_type());
                    cg.void_type()
                } else {
                    abi.pass_ty
                }
            } else {
                cg.ty(data.return_ty)
            }
        }
    };

    let data = cg.hir.proc_data(proc_id);
    for param in data.params {
        let ty = if is_external {
            win_x64_parameter_type(cg, param.ty).pass_ty
        } else {
            cg.ty(param.ty)
        };
        cg.cache.types.push(ty);
    }

    let data = cg.hir.proc_data(proc_id);
    let name = if is_external || is_entry {
        cg.session.intern_name.get(data.name.id)
    } else {
        cg.namebuf.clear();
        context::write_symbol_name(cg, data.name.id, data.origin_id, poly_types);
        cg.namebuf.as_str()
    };

    let param_types = cg.cache.types.view(offset);
    let fn_ty = llvm::function_type(return_ty, param_types, is_variadic);
    cg.cache.types.pop_view(offset);

    let (linkage, call_conv) = if is_external || is_entry {
        (llvm::Linkage::LLVMExternalLinkage, llvm::CallConv::LLVMCCallConv)
    } else {
        (llvm::Linkage::LLVMInternalLinkage, llvm::CallConv::LLVMFastCallConv)
    };
    let fn_val = cg.module.add_function(name, fn_ty, linkage);
    fn_val.set_call_conv(call_conv);

    let data = cg.hir.proc_data(proc_id);
    if by_pointer_ret {
        fn_val.set_param_attr(cg.cache.sret, 1);
    }

    if data.flag_set.contains(hir::ProcFlag::Inline) {
        fn_val.set_attr(cg.cache.inlinehint);
    }
    let return_ty = context::substitute_type(cg, data.return_ty, &[]);
    if return_ty.is_never() {
        fn_val.set_attr(cg.cache.noreturn);
    }

    cg.proc.poly_types = curr_poly;
    (fn_val, fn_ty)
}

fn codegen_function_body<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    proc_id: hir::ProcID,
    poly_types: &'c [hir::Type<'c>],
) {
    let data = cg.hir.proc_data(proc_id);
    if data.flag_set.contains(hir::ProcFlag::External) {
        return;
    }

    let fn_val = if poly_types.is_empty() {
        cg.procs[proc_id.index()].0
    } else {
        cg.poly_procs.get(&(proc_id, poly_types)).unwrap().0
    };
    cg.proc.reset(proc_id, fn_val, poly_types);

    let entry_bb = cg.context.append_bb(fn_val, "entry_bb");
    cg.build.position_at_end(entry_bb);

    for (param_idx, param) in data.params.iter().enumerate() {
        let param_ty = cg.ty(param.ty);

        let name = cg.session.intern_name.get(param.name.id);
        cg.namebuf.clear();
        cg.namebuf.push_str(name);

        let param_ptr = cg.build.alloca(param_ty, &cg.namebuf);
        cg.proc.param_ptrs.push(param_ptr);
        let param_val = fn_val.param_val(param_idx as u32);
        cg.build.store(param_val, param_ptr);
    }

    let data = cg.hir.proc_data(proc_id);
    for var in data.variables {
        let var_ty = cg.ty(var.ty);

        let name = cg.session.intern_name.get(var.name.id);
        cg.namebuf.clear();
        cg.namebuf.push_str(name);

        let var_ptr = cg.build.alloca(var_ty, &cg.namebuf);
        cg.proc.variable_ptrs.push(var_ptr);
    }

    let data = cg.hir.proc_data(proc_id);
    if let Some(block) = data.block {
        if data.flag_set.contains(hir::ProcFlag::EntryPoint) {
            emit_expr::codegen_call_direct(cg, Expect::Value(None), cg.hir.core.start, &[], &[]);
        }

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
    }
}

pub struct ParamAbi {
    pub pass_ty: llvm::Type,
    pub by_pointer: bool,
}

//@empty struct should be {size: 4 align: 1}. handle on hir level? how to avoid #repr_c?
//@need #repr_c like flagging for enums, disallow enums with fields, only allow i32 tag_ty.
pub fn win_x64_parameter_type(cg: &Codegen, ty: hir::Type) -> ParamAbi {
    let pass_ty: llvm::Type = match ty {
        hir::Type::Error | hir::Type::Unknown => unreachable!(),
        hir::Type::Char => cg.char_type(),
        hir::Type::Void => cg.void_type(),
        hir::Type::Never => cg.void_type(),
        hir::Type::Rawptr => cg.ptr_type(),
        hir::Type::UntypedChar => unreachable!(),
        hir::Type::Int(int_ty) => cg.int_type(int_ty),
        hir::Type::Float(float_ty) => cg.float_type(float_ty),
        hir::Type::Bool(bool_ty) => cg.bool_type(bool_ty),
        hir::Type::String(string_type) => match string_type {
            hir::StringType::String => {
                return ParamAbi { pass_ty: cg.ptr_type(), by_pointer: true }
            }
            hir::StringType::CString => cg.ptr_type(),
            hir::StringType::Untyped => unreachable!(),
        },
        hir::Type::PolyProc(_, _) => unimplemented!("win x64 poly_proc"),
        hir::Type::PolyEnum(_, _) => unimplemented!("win x64 poly_enum"),
        hir::Type::PolyStruct(_, _) => unimplemented!("win x64 poly_struct"),
        hir::Type::Enum(enum_id, poly_types) => {
            let data = cg.hir.enum_data(enum_id);

            if !poly_types.is_empty() {
                unimplemented!("win x64 poly enum");
            }
            if data.flag_set.contains(hir::EnumFlag::WithFields) {
                unimplemented!("win x64 enum with fields");
            }

            cg.int_type(data.tag_ty.resolved_unwrap())
        }
        hir::Type::Struct(struct_id, poly_types) => {
            let data = cg.hir.struct_data(struct_id);
            let layout = data.layout.resolved_unwrap();

            if !poly_types.is_empty() {
                unimplemented!("win x64 poly struct");
            }

            return match layout.size {
                1 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S8), by_pointer: false },
                2 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S16), by_pointer: false },
                4 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S32), by_pointer: false },
                8 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S64), by_pointer: false },
                _ => ParamAbi { pass_ty: cg.ptr_type(), by_pointer: true },
            };
        }
        hir::Type::Reference(_, _) => cg.ptr_type(),
        hir::Type::MultiReference(_, _) => cg.ptr_type(),
        hir::Type::Procedure(_) => cg.ptr_type(),
        hir::Type::ArraySlice(_) => return ParamAbi { pass_ty: cg.ptr_type(), by_pointer: true },
        hir::Type::ArrayStatic(_) => return ParamAbi { pass_ty: cg.ptr_type(), by_pointer: true },
    };

    ParamAbi { pass_ty, by_pointer: false }
}

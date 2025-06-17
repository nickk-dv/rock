use super::context::{self, Codegen, Expect};
use super::emit_expr;
use super::emit_stmt;
use super::llvm;
use crate::ast;
use crate::error::SourceRange;
use crate::hir;
use crate::hir_lower::layout;
use crate::hir_lower::types;
use crate::intern::LitID;
use crate::session::{ModuleID, Session};
use crate::support::AsStr;
use crate::text::TextRange;

pub fn codegen_module(
    hir: hir::Hir,
    session: &mut Session,
) -> Result<(llvm::IRTarget, llvm::IRModule), ()> {
    let mut cg = Codegen::new(hir, session.config.target, session);
    cg.procs.resize(cg.procs.capacity(), (llvm::ValueFn::null(), llvm::TypeFn::null()));

    codegen_enum_types(&mut cg);
    codegen_struct_types(&mut cg);
    codegen_globals(&mut cg);

    let entry_point = cg.hir.entry_point.unwrap();
    codegen_function(&mut cg, entry_point, &[]);
    while let Some((proc_id, poly_types)) = cg.proc_queue.pop() {
        codegen_function_body(&mut cg, proc_id, poly_types);
    }
    codegen_type_info(&mut cg);

    let (target, module, emit) = (cg.target, cg.module, cg.emit);
    session.move_errors(emit.collect(), vec![]);
    session.result().map(|_| (target, module))
}

pub fn codegen_string_lit(cg: &mut Codegen, val: LitID) -> llvm::ValuePtr {
    if let Some(string_lit) = cg.strings.get(&val) {
        return string_lit.as_ptr();
    }
    let string = cg.session.intern_lit.get(val);
    let string_val = llvm::const_string(&cg.context, string, true);
    let string_ty = llvm::typeof_value(string_val);
    let global = cg.module.add_global("rock.string", Some(string_val), string_ty, true, true);
    cg.strings.insert(val, global);
    global.as_ptr()
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
}

pub fn codegen_function<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    proc_id: hir::ProcID,
    poly_types: &'c [hir::Type<'c>],
) -> (llvm::ValueFn, llvm::TypeFn) {
    if poly_types.is_empty() {
        let fn_res = &cg.procs[proc_id.index()];
        if !fn_res.0.is_null() {
            *fn_res
        } else {
            let fn_res = codegen_function_value(cg, proc_id, &[]);
            cg.procs[proc_id.index()] = fn_res;
            cg.proc_queue.push((proc_id, &[]));
            fn_res
        }
    } else {
        types::expect_concrete(poly_types);
        if let Some(fn_res) = cg.procs_poly.get(&(proc_id, poly_types)) {
            *fn_res
        } else {
            let fn_res = codegen_function_value(cg, proc_id, poly_types);
            cg.procs_poly.insert((proc_id, poly_types), fn_res);
            cg.proc_queue.push((proc_id, poly_types));
            fn_res
        }
    }
}

fn codegen_function_value<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    proc_id: hir::ProcID,
    poly_types: &'c [hir::Type<'c>],
) -> (llvm::ValueFn, llvm::TypeFn) {
    //temporary override of poly_types in proc scope
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

    let linkage = if is_external || is_entry {
        llvm::Linkage::LLVMExternalLinkage
    } else {
        llvm::Linkage::LLVMInternalLinkage
    };
    let fn_val = cg.module.add_function(name, fn_ty, linkage);

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
        cg.procs_poly.get(&(proc_id, poly_types)).unwrap().0
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
pub fn win_x64_parameter_type<'c>(cg: &mut Codegen<'c, '_, '_>, ty: hir::Type<'c>) -> ParamAbi {
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
        hir::Type::ArrayStatic(_) | hir::Type::ArrayEnumerated(_) => {
            return win_x64_aggregate(cg, ty)
        }
    };

    ParamAbi { pass_ty, by_pointer: false }
}

pub fn win_x64_aggregate<'c>(cg: &mut Codegen<'c, '_, '_>, ty: hir::Type<'c>) -> ParamAbi {
    //@no source available
    let src = SourceRange::new(ModuleID::new(0), TextRange::zero());
    let layout = layout::type_layout(cg, ty, &[], src).unwrap(); //@unwrap
    match layout.size {
        1 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S8), by_pointer: false },
        2 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S16), by_pointer: false },
        4 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S32), by_pointer: false },
        8 => ParamAbi { pass_ty: cg.int_type(hir::IntType::S64), by_pointer: false },
        _ => ParamAbi { pass_ty: cg.ptr_type(), by_pointer: true },
    }
}

fn codegen_type_info(cg: &mut Codegen) {
    let next_idx = cg.globals.len();
    cg.info.types_id = hir::GlobalID::new(next_idx);
    cg.info.enums_id = hir::GlobalID::new(next_idx + 1);
    cg.info.structs_id = hir::GlobalID::new(next_idx + 2);
    cg.info.references_id = hir::GlobalID::new(next_idx + 3);
    cg.info.procedures_id = hir::GlobalID::new(next_idx + 4);
    cg.info.slices_id = hir::GlobalID::new(next_idx + 5);
    cg.info.arrays_id = hir::GlobalID::new(next_idx + 6);
    cg.info.variants_id = hir::GlobalID::new(next_idx + 7);
    cg.info.variant_fields_id = hir::GlobalID::new(next_idx + 8);
    cg.info.fields_id = hir::GlobalID::new(next_idx + 9);
    cg.info.param_types_id = hir::GlobalID::new(next_idx + 10);

    register_type_info(cg, hir::Type::Char);
    register_type_info(cg, hir::Type::Void);
    register_type_info(cg, hir::Type::Never);
    register_type_info(cg, hir::Type::Rawptr);
    for ty in hir::IntType::ALL.iter().copied() {
        if ty != hir::IntType::Untyped {
            register_type_info(cg, hir::Type::Int(ty));
        }
    }
    for ty in hir::FloatType::ALL.iter().copied() {
        if ty != hir::FloatType::Untyped {
            register_type_info(cg, hir::Type::Float(ty));
        }
    }
    for ty in hir::BoolType::ALL.iter().copied() {
        if ty != hir::BoolType::Untyped {
            register_type_info(cg, hir::Type::Bool(ty));
        }
    }
    for ty in hir::StringType::ALL.iter().copied() {
        if ty != hir::StringType::Untyped {
            register_type_info(cg, hir::Type::String(ty));
        }
    }
    for idx in 0..cg.info.var_args.len() {
        for ty in cg.info.var_args[idx].1.iter().copied() {
            register_type_info(cg, ty);
        }
    }

    macro_rules! add_type_info_global {
        ($cg:expr, $values:ident, $ty:expr, $name:expr) => {{
            let elem_ty = $cg.ty($ty);
            let array_ty = llvm::array_type(elem_ty, $cg.info.$values.len() as u64);
            $cg.globals.push($cg.module.add_global($name, None, array_ty, true, false));
        }};
    }

    macro_rules! init_type_info_global {
        ($cg:expr, $values:ident, $ty:expr, $id_name:ident) => {{
            let array = if cg.info.$values.is_empty() {
                hir::ConstValue::ArrayEmpty { elem_ty: $cg.hir.arena.alloc($ty) }
            } else {
                let array =
                    hir::ConstArray { values: $cg.hir.arena.alloc_slice(&$cg.info.$values) };
                hir::ConstValue::Array { array: $cg.hir.arena.alloc(array) }
            };
            let value = emit_expr::codegen_const($cg, array);
            $cg.module.init_global($cg.globals[$cg.info.$id_name.index()], value);
        }};
    }

    let type_info_enum = hir::Type::Struct(cg.hir.core.type_info_enum, &[]);
    let type_info_struct = hir::Type::Struct(cg.hir.core.type_info_struct, &[]);
    let type_info_reference = hir::Type::Struct(cg.hir.core.type_info_reference, &[]);
    let type_info_procedure = hir::Type::Struct(cg.hir.core.type_info_procedure, &[]);
    let type_info_array_slice = hir::Type::Struct(cg.hir.core.type_info_array_slice, &[]);
    let type_info_array_static = hir::Type::Struct(cg.hir.core.type_info_array_static, &[]);
    let type_info_variant = hir::Type::Struct(cg.hir.core.type_info_variant, &[]);
    let type_info_variant_field = hir::Type::Struct(cg.hir.core.type_info_variant_field, &[]);
    let type_info_field = hir::Type::Struct(cg.hir.core.type_info_field, &[]);
    let type_info_param_types = hir::Type::Rawptr;

    cg.globals.push(llvm::ValueGlobal::null());
    add_type_info_global!(cg, enums, type_info_enum, "TYPE_INFO_ENUM");
    add_type_info_global!(cg, structs, type_info_struct, "TYPE_INFO_STRUCT");
    add_type_info_global!(cg, references, type_info_reference, "TYPE_INFO_REFERENCE");
    add_type_info_global!(cg, procedures, type_info_procedure, "TYPE_INFO_PROCEDURE");
    add_type_info_global!(cg, slices, type_info_array_slice, "TYPE_INFO_ARRAY_SLICE");
    add_type_info_global!(cg, arrays, type_info_array_static, "TYPE_INFO_ARRAY_STATIC");
    add_type_info_global!(cg, variants, type_info_variant, "TYPE_INFO_VARIANT");
    add_type_info_global!(cg, variant_fields, type_info_variant_field, "TYPE_INFO_VARIANT_FIELD");
    add_type_info_global!(cg, fields, type_info_field, "TYPE_INFO_FIELD");
    add_type_info_global!(cg, param_types, type_info_param_types, "TYPE_INFO_PARAM_TYPE");

    let array = hir::ConstArray { values: cg.hir.arena.alloc_slice(&cg.info.types) };
    let array = hir::ConstValue::Array { array: cg.hir.arena.alloc(array) };
    let value = emit_expr::const_writer(cg, array);
    let global_ty = llvm::typeof_value(value);
    let global = cg.module.add_global("TYPE_INFO", Some(value), global_ty, true, false);
    cg.globals[cg.info.types_id.index()] = global;

    init_type_info_global!(cg, enums, type_info_enum, enums_id);
    init_type_info_global!(cg, structs, type_info_struct, structs_id);
    init_type_info_global!(cg, references, type_info_reference, references_id);
    init_type_info_global!(cg, procedures, type_info_procedure, procedures_id);
    init_type_info_global!(cg, slices, type_info_array_slice, slices_id);
    init_type_info_global!(cg, arrays, type_info_array_static, arrays_id);
    init_type_info_global!(cg, variants, type_info_variant, variants_id);
    init_type_info_global!(cg, variant_fields, type_info_variant_field, variant_fields_id);
    init_type_info_global!(cg, fields, type_info_field, fields_id);
    init_type_info_global!(cg, param_types, type_info_param_types, param_types_id);

    let any_ty = cg.structs[cg.hir.core.any.unwrap().index()];
    let type_info = cg.ty(hir::Type::Enum(cg.hir.core.type_info, &[]));
    let type_info_arr = llvm::array_type(type_info, cg.info.types.len() as u64);
    let type_info_ptr = cg.globals[cg.info.types_id.index()].as_ptr();

    for (inst, types) in cg.info.var_args.iter().copied() {
        let _ = cg.position_after(inst.as_inst());
        let array_ty = llvm::array_type(any_ty.as_ty(), types.len() as u64);
        for (arg_idx, ty) in types.iter().enumerate() {
            let array_gep = cg.build.gep_inbounds(
                array_ty,
                inst.into_ptr(),
                &[cg.const_usize(0), cg.const_usize(arg_idx as u64)],
                "any_array.idx",
            );
            let type_ptr = cg.build.gep_struct(any_ty, array_gep, 1, "any.type");

            let type_id = *cg.info.type_ids.get(ty).unwrap();
            let info_ptr = cg.build.gep_inbounds(
                type_info_arr,
                type_info_ptr,
                &[cg.const_usize(0), cg.const_usize(type_id)],
                "type_info",
            );
            cg.build.store(info_ptr.as_val(), type_ptr);
        }
    }
}

//@implement proc_ty, enumerated array
fn register_type_info<'c>(cg: &mut Codegen<'c, '_, '_>, ty: hir::Type<'c>) -> hir::ConstValue<'c> {
    if let Some(type_id) = cg.info.type_ids.get(&ty).copied() {
        return hir::ConstValue::GlobalIndex { global_id: cg.info.types_id, index: type_id };
    }
    let type_id = cg.info.types.len();
    cg.info.type_ids.insert(ty, type_id as u64);
    cg.info.types.push(hir::ConstValue::Void);

    cg.info.types[type_id] = match ty {
        hir::Type::Error
        | hir::Type::Unknown
        | hir::Type::UntypedChar
        | hir::Type::PolyProc(_, _)
        | hir::Type::PolyEnum(_, _)
        | hir::Type::PolyStruct(_, _) => {
            unreachable!()
        }
        hir::Type::Char => const_enum(cg, cg.hir.core.type_info, 0, &[]),
        hir::Type::Void => const_enum(cg, cg.hir.core.type_info, 1, &[]),
        hir::Type::Never => const_enum(cg, cg.hir.core.type_info, 2, &[]),
        hir::Type::Rawptr => const_enum(cg, cg.hir.core.type_info, 3, &[]),
        hir::Type::Int(ty) => {
            let int_ty = const_enum_simple(cg.hir.core.int_ty, ty as u32);
            const_enum(cg, cg.hir.core.type_info, 4, &[int_ty])
        }
        hir::Type::Float(ty) => {
            let float_ty = const_enum_simple(cg.hir.core.float_ty, ty as u32);
            const_enum(cg, cg.hir.core.type_info, 5, &[float_ty])
        }
        hir::Type::Bool(ty) => {
            let bool_ty = const_enum_simple(cg.hir.core.bool_ty, ty as u32);
            const_enum(cg, cg.hir.core.type_info, 6, &[bool_ty])
        }
        hir::Type::String(ty) => {
            let string_ty = const_enum_simple(cg.hir.core.string_ty, ty as u32);
            const_enum(cg, cg.hir.core.type_info, 7, &[string_ty])
        }
        hir::Type::Enum(id, poly_types) => {
            let layout = cg.enum_layout((id, poly_types));
            let data = cg.hir.enum_data(id);
            let enum_name = cg.session.intern_name.get(data.name.id);
            let tag_ty = data.tag_ty.resolved_unwrap();

            let offset = cg.cache.const_values.start();
            for (idx, variant) in data.variants.iter().enumerate() {
                let variant_id = hir::VariantID::new(idx);
                let variant_name = cg.session.intern_name.get(variant.name.id);
                let variant_layout = cg.variant_layout((id, variant_id, poly_types));

                let offset_field = cg.cache.const_values.start();
                for (field_idx, field) in variant.fields.iter().enumerate() {
                    let field_ty = context::substitute_type(cg, field.ty, poly_types);

                    let values = [
                        register_type_info(cg, field_ty),
                        const_usize(variant_layout.field_offset[field_idx + 1]),
                    ];
                    let field_info = const_struct(cg, cg.hir.core.type_info_variant_field, &values);
                    cg.cache.const_values.push(field_info);
                }

                let slice_index = cg.info.variant_fields.len();
                let field_infos = cg.cache.const_values.view(offset_field);
                for info in field_infos {
                    cg.info.variant_fields.push(*info);
                }
                cg.cache.const_values.pop_view(offset_field);
                let slice_len = cg.info.variant_fields.len() - slice_index;
                let field_slice = hir::ConstValue::GlobalSlice {
                    global_id: cg.info.variant_fields_id,
                    index: slice_index as u32,
                    len: slice_len as u32,
                };

                //@tag values outside i64 range not represented, panic?
                let data = cg.hir.enum_data(id);
                let tag = match data.variant(variant_id).kind {
                    hir::VariantKind::Default(id) => {
                        cg.hir.variant_eval_values[id.index()].into_int_i64()
                    }
                    hir::VariantKind::Constant(id) => {
                        cg.hir.const_eval_values[id.index()].into_int_i64()
                    }
                };
                let values = [
                    const_string(cg.session.intern_lit.intern(variant_name)),
                    hir::ConstValue::from_i64(tag, hir::IntType::S64),
                    field_slice,
                ];
                let info = const_struct(cg, cg.hir.core.type_info_variant, &values);
                cg.cache.const_values.push(info);
            }

            let slice_index = cg.info.variants.len();
            let variant_infos = cg.cache.const_values.view(offset);
            for info in variant_infos {
                cg.info.variants.push(*info);
            }
            cg.cache.const_values.pop_view(offset);
            let slice_len = cg.info.variants.len() - slice_index;
            let variant_slice = hir::ConstValue::GlobalSlice {
                global_id: cg.info.variants_id,
                index: slice_index as u32,
                len: slice_len as u32,
            };

            let values = [
                const_string(cg.session.intern_lit.intern(enum_name)),
                const_enum_simple(cg.hir.core.int_ty, tag_ty as u32),
                variant_slice,
                const_usize(layout.size),
            ];
            let info = const_struct(cg, cg.hir.core.type_info_enum, &values);

            let index = cg.info.enums.len() as u64;
            cg.info.enums.push(info);
            let enum_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.enums_id, index };
            const_enum(cg, cg.hir.core.type_info, 8, &[enum_ptr])
        }
        hir::Type::Struct(id, poly_types) => {
            let layout = cg.struct_layout((id, poly_types));
            let data = cg.hir.struct_data(id);
            let struct_name = cg.session.intern_name.get(data.name.id);

            let offset = cg.cache.const_values.start();
            for (field_idx, field) in data.fields.iter().enumerate() {
                let field_ty = context::substitute_type(cg, field.ty, poly_types);
                let field_name = cg.session.intern_name.get(field.name.id);

                let values = [
                    const_string(cg.session.intern_lit.intern(field_name)),
                    register_type_info(cg, field_ty),
                    const_usize(layout.field_offset[field_idx]),
                ];
                let field_info = const_struct(cg, cg.hir.core.type_info_field, &values);
                cg.cache.const_values.push(field_info);
            }

            let slice_index = cg.info.fields.len();
            let field_infos = cg.cache.const_values.view(offset);
            for info in field_infos {
                cg.info.fields.push(*info);
            }
            cg.cache.const_values.pop_view(offset);
            let slice_len = cg.info.fields.len() - slice_index;
            let field_slice = hir::ConstValue::GlobalSlice {
                global_id: cg.info.fields_id,
                index: slice_index as u32,
                len: slice_len as u32,
            };

            let values = [
                const_string(cg.session.intern_lit.intern(struct_name)),
                field_slice,
                const_usize(layout.total.size),
            ];
            let info = const_struct(cg, cg.hir.core.type_info_struct, &values);

            let index = cg.info.structs.len() as u64;
            cg.info.structs.push(info);
            let struct_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.structs_id, index };
            const_enum(cg, cg.hir.core.type_info, 9, &[struct_ptr])
        }
        hir::Type::Reference(mutt, ref_ty) => {
            let values = [
                const_bool(false),
                const_bool(mutt == ast::Mut::Mutable),
                register_type_info(cg, *ref_ty),
            ];
            let info = const_struct(cg, cg.hir.core.type_info_reference, &values);

            let index = cg.info.references.len() as u64;
            cg.info.references.push(info);
            let ref_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.references_id, index };
            const_enum(cg, cg.hir.core.type_info, 10, &[ref_ptr])
        }
        hir::Type::MultiReference(mutt, ref_ty) => {
            let values = [
                const_bool(true),
                const_bool(mutt == ast::Mut::Mutable),
                register_type_info(cg, *ref_ty),
            ];
            let info = const_struct(cg, cg.hir.core.type_info_reference, &values);

            let index = cg.info.references.len() as u64;
            cg.info.references.push(info);
            let ref_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.references_id, index };
            const_enum(cg, cg.hir.core.type_info, 10, &[ref_ptr])
        }
        hir::Type::Procedure(proc_ty) => unimplemented!(),
        hir::Type::ArraySlice(slice) => {
            let values = [
                const_bool(slice.mutt == ast::Mut::Mutable),
                register_type_info(cg, slice.elem_ty),
            ];
            let info = const_struct(cg, cg.hir.core.type_info_array_slice, &values);

            let index = cg.info.slices.len() as u64;
            cg.info.slices.push(info);
            let slice_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.slices_id, index };
            const_enum(cg, cg.hir.core.type_info, 12, &[slice_ptr])
        }
        hir::Type::ArrayStatic(array) => {
            let values =
                [const_usize(cg.array_len(array.len)), register_type_info(cg, array.elem_ty)];
            let info = const_struct(cg, cg.hir.core.type_info_array_static, &values);

            let index = cg.info.arrays.len() as u64;
            cg.info.arrays.push(info);
            let array_ptr = hir::ConstValue::GlobalIndex { global_id: cg.info.arrays_id, index };
            const_enum(cg, cg.hir.core.type_info, 13, &[array_ptr])
        }
        hir::Type::ArrayEnumerated(array) => unimplemented!(),
    };

    hir::ConstValue::GlobalIndex { global_id: cg.info.types_id, index: type_id as u64 }
}

fn const_enum_simple<'c>(id: hir::EnumID, var: u32) -> hir::ConstValue<'c> {
    hir::ConstValue::Variant { enum_id: id, variant_id: hir::VariantID::new(var as usize) }
}

fn const_enum<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    id: hir::EnumID,
    var: u32,
    values: &[hir::ConstValue<'c>],
) -> hir::ConstValue<'c> {
    let variant = hir::ConstVariant {
        variant_id: hir::VariantID::new(var as usize),
        values: cg.hir.arena.alloc_slice(values),
        poly_types: &[],
    };
    hir::ConstValue::VariantPoly { enum_id: id, variant: cg.hir.arena.alloc(variant) }
}

fn const_struct<'c>(
    cg: &mut Codegen<'c, '_, '_>,
    id: hir::StructID,
    values: &[hir::ConstValue<'c>],
) -> hir::ConstValue<'c> {
    let values = cg.hir.arena.alloc_slice(values);
    let struct_ = hir::ConstStruct { values, poly_types: &[] };
    let struct_ = cg.hir.arena.alloc(struct_);
    hir::ConstValue::Struct { struct_id: id, struct_ }
}

fn const_bool<'c>(val: bool) -> hir::ConstValue<'c> {
    hir::ConstValue::Bool { val, bool_ty: hir::BoolType::Bool }
}

fn const_usize<'c>(val: u64) -> hir::ConstValue<'c> {
    hir::ConstValue::Int { val, neg: false, int_ty: hir::IntType::Usize }
}

fn const_string<'c>(val: LitID) -> hir::ConstValue<'c> {
    hir::ConstValue::String { val, string_ty: hir::StringType::String }
}

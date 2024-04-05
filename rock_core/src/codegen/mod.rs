use crate::ast::BasicType;
use crate::hir;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetData, TargetMachine,
    TargetTriple,
};
use inkwell::types;
use inkwell::OptimizationLevel;
use std::path::Path;

pub fn test_codegen(hir: hir::Hir) {
    let context = Context::create();
    let module = context.create_module("example_test");
    let builder = context.create_builder();

    let i64_type = context.i64_type();
    let fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into()], false);
    let function = module.add_function("sum_of_3numbers", fn_type, None);
    let basic_block = context.append_basic_block(function, "entry");

    builder.position_at_end(basic_block);
    let x = function.get_nth_param(0).unwrap().into_int_value();
    let y = function.get_nth_param(1).unwrap().into_int_value();
    let z = function.get_nth_param(2).unwrap().into_int_value();

    let sum = builder.build_int_add(x, y, "sum").unwrap();
    let sum = builder.build_int_add(sum, z, "sum").unwrap();
    builder.build_return(Some(&sum)).unwrap();

    let fn_type = i64_type.fn_type(&[], false);
    let function_main = module.add_function("main", fn_type, None);
    let basic_block = context.append_basic_block(function_main, "entry");
    builder.position_at_end(basic_block);
    let value = builder
        .build_call(
            function,
            &[
                i64_type.const_int(1, true).into(),
                i64_type.const_int(2, true).into(),
                i64_type.const_int(3, true).into(),
            ],
            "sum_result",
        )
        .unwrap();
    builder
        .build_return(Some(&value.try_as_basic_value().unwrap_left()))
        .unwrap();

    module.print_to_stderr();

    Target::initialize_x86(&InitializationConfig::default());

    let opt = OptimizationLevel::None;
    let reloc = RelocMode::Default;
    let model = CodeModel::Default;
    let path = Path::new("build/main.o");
    let target = Target::from_name("x86-64").unwrap();

    let target_machine = target
        .create_target_machine(
            &TargetMachine::get_default_triple(),
            "x86-64",
            TargetMachine::get_host_cpu_features()
                .to_str()
                .expect("utf-8"),
            opt,
            reloc,
            model,
        )
        .unwrap();

    assert!(target_machine
        .write_to_file(&module, FileType::Object, &path)
        .is_ok());
}

struct CodegenData<'ctx> {
    struct_types: Vec<types::StructType<'ctx>>,
}

//@perf context.ptr_sized_int_type( could be cached in Codegen
// to avoid extra calls and matching in the call_stack
// same could be done for other types, to not call into llvm context as much
fn type_to_llvm<'ctx>(
    context: &'ctx Context,
    codegen: &CodegenData<'ctx>,
    ty: hir::Type,
    target_data: &TargetData,
) -> Option<types::BasicTypeEnum<'ctx>> {
    let llvm_ty = match ty {
        hir::Type::Error => panic!("unexpected error type during codegen"),
        hir::Type::Basic(basic) => match basic {
            BasicType::Unit => return None,
            BasicType::Bool => context.bool_type().into(),
            BasicType::S8 => context.i8_type().into(),
            BasicType::S16 => context.i16_type().into(),
            BasicType::S32 => context.i32_type().into(),
            BasicType::S64 => context.i64_type().into(),
            BasicType::Ssize => context.ptr_sized_int_type(target_data, None).into(),
            BasicType::U8 => context.i8_type().into(),
            BasicType::U16 => context.i16_type().into(),
            BasicType::U32 => context.i32_type().into(),
            BasicType::U64 => context.i64_type().into(),
            BasicType::Usize => context.ptr_sized_int_type(target_data, None).into(),
            BasicType::F32 => context.f32_type().into(),
            BasicType::F64 => context.f64_type().into(),
            BasicType::Char => context.i32_type().into(),
            BasicType::Rawptr => context.ptr_sized_int_type(target_data, None).into(),
        },
        hir::Type::Enum(_) => todo!(),
        hir::Type::Union(_) => todo!(),
        hir::Type::Struct(id) => codegen.struct_types[id.index()].into(),
        hir::Type::Reference(_, _) => context.ptr_sized_int_type(target_data, None).into(),
        hir::Type::ArraySlice(_) => todo!(),
        hir::Type::ArrayStatic(_) => todo!(),
    };
    Some(llvm_ty)
}

pub fn codegen(hir: hir::Hir) {
    Target::initialize_x86(&InitializationConfig::default());

    let opt = OptimizationLevel::None;
    let reloc = RelocMode::Default;
    let model = CodeModel::Default;
    let path = Path::new("build/main.o");
    let target = Target::from_name("x86-64").unwrap();

    let target_machine = target
        .create_target_machine(
            &TargetMachine::get_default_triple(),
            "x86-64",
            TargetMachine::get_host_cpu_features()
                .to_str()
                .expect("utf-8"),
            opt,
            reloc,
            model,
        )
        .unwrap();

    let context = Context::create();
    let module = context.create_module("rock_module");
    let builder = context.create_builder();

    let mut codegen = CodegenData {
        struct_types: Vec::with_capacity(hir.structs.len()),
    };

    for idx in 0..hir.structs.len() {
        codegen
            .struct_types
            .push(context.opaque_struct_type(&format!("rock_struct_{idx}")));
    }

    let mut field_types = Vec::<types::BasicTypeEnum>::new();
    for (idx, struct_data) in hir.structs.iter().enumerate() {
        field_types.clear();

        for field in struct_data.fields {
            let field_ty = type_to_llvm(
                &context,
                &codegen,
                field.ty,
                &target_machine.get_target_data(),
            );
            //@issue related to unit / void type below
            if let Some(field_ty) = field_ty {
                field_types.push(field_ty);
            }
        }

        //@breaking issue inkwell api takes in BasicTypeEnum for struct body creation
        // which doesnt allow void type which is represented as unit () in rock language
        // this has a side-effect of shifting field ids in relation to generated StructFieldIDs in hir::Expr
        // llvm doesnt seem to explicitly disallow void_type in structures. @05.04.24
        let opaque_struct_type = codegen.struct_types[idx];
        opaque_struct_type.set_body(&field_types, false);
        eprintln!("{}", opaque_struct_type.print_to_string());
    }

    for (idx, proc_data) in hir.procs.iter().enumerate() {
        let function_ty = context.void_type().fn_type(&[], false);
        //@specify linkage based on function kind #[c_call] etc.
        let function = module.add_function(&format!("rock_proc_{idx}"), function_ty, None);
        let basic_block = context.append_basic_block(function, "entry");
        builder.position_at_end(basic_block);
    }

    //@debug print
    module.print_to_stderr();
}

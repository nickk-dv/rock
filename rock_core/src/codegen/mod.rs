use crate::hir;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
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

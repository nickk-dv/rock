use crate::ast;
use crate::error::ErrorComp;
use crate::fs_env;
use crate::hir;
use crate::intern::InternID;
use inkwell::basic_block::BasicBlock;
use inkwell::builder;
use inkwell::context;
use inkwell::module;
use inkwell::targets;
use inkwell::types;
use inkwell::types::BasicType;
use inkwell::values;
use std::collections::HashMap;
use std::path::PathBuf;

struct BuildContext {
    bin_name: String,
    build_kind: BuildKind,
    build_dir: PathBuf,
    executable_path: PathBuf,
}

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}

struct Codegen<'ctx> {
    context: &'ctx context::Context,
    module: module::Module<'ctx>,
    builder: builder::Builder<'ctx>,
    target_machine: targets::TargetMachine,
    string_lits: Vec<values::GlobalValue<'ctx>>,
    structs: Vec<types::StructType<'ctx>>,
    consts: Vec<values::BasicValueEnum<'ctx>>,
    globals: Vec<values::GlobalValue<'ctx>>,
    function_values: Vec<values::FunctionValue<'ctx>>,
    hir: hir::Hir<'ctx>,
    c_functions: HashMap<InternID, values::FunctionValue<'ctx>>,
    ptr_type: types::PointerType<'ctx>,
    ptr_sized_int_type: types::IntType<'ctx>,
    slice_type: types::StructType<'ctx>,
}

struct ProcCodegen<'ctx> {
    proc_id: hir::ProcID,
    function: values::FunctionValue<'ctx>,
    param_vars: Vec<values::PointerValue<'ctx>>,
    local_vars: Vec<values::PointerValue<'ctx>>,
    block_info: Vec<BlockInfo<'ctx>>,
    defer_blocks: Vec<hir::Block<'ctx>>,
    next_loop_info: Option<LoopInfo<'ctx>>,
}

#[derive(Copy, Clone)]
struct BlockInfo<'ctx> {
    defer_count: u32,
    loop_info: Option<LoopInfo<'ctx>>,
}

#[derive(Copy, Clone)]
struct LoopInfo<'ctx> {
    break_bb: BasicBlock<'ctx>,
    continue_bb: BasicBlock<'ctx>,
}

impl<'ctx> ProcCodegen<'ctx> {
    fn set_next_loop_info(&mut self, break_bb: BasicBlock<'ctx>, continue_bb: BasicBlock<'ctx>) {
        self.next_loop_info = Some(LoopInfo {
            break_bb,
            continue_bb,
        });
    }

    fn enter_block(&mut self) {
        self.block_info.push(BlockInfo {
            defer_count: 0,
            loop_info: self.next_loop_info,
        });
        self.next_loop_info = None;
    }

    fn exit_block(&mut self) {
        let last_count = self.block_info.last().unwrap().defer_count;
        for _ in 0..last_count {
            assert!(self.defer_blocks.pop().is_some());
        }
        assert!(self.block_info.pop().is_some());
    }

    fn push_defer_block(&mut self, block: hir::Block<'ctx>) {
        self.block_info.last_mut().unwrap().defer_count += 1;
        self.defer_blocks.push(block);
    }

    fn last_loop_info(&self) -> (LoopInfo<'ctx>, Vec<hir::Block<'ctx>>) {
        let mut defer_count = 0;
        for info in self.block_info.iter().rev() {
            defer_count += info.defer_count;

            if let Some(loop_info) = info.loop_info {
                let total_count = self.defer_blocks.len();
                let range = total_count - defer_count as usize..total_count;
                let defer_blocks = self.defer_blocks[range].to_vec();
                return (loop_info, defer_blocks);
            }
        }
        unreachable!("last loop must exist")
    }

    fn last_defer_blocks(&self) -> Vec<hir::Block<'ctx>> {
        let total_count = self.defer_blocks.len();
        let defer_count = self.block_info.last().unwrap().defer_count;
        let range = total_count - defer_count as usize..total_count;
        self.defer_blocks[range].to_vec()
    }

    //@lifetime problems, need to clone like this (since codegen_expr can mutate this vec) 05.05.24
    fn all_defer_blocks(&self) -> Vec<hir::Block<'ctx>> {
        self.defer_blocks.clone()
    }
}

impl<'ctx> Codegen<'ctx> {
    fn new(hir: hir::Hir<'ctx>, context: &'ctx context::Context) -> Codegen<'ctx> {
        let module = context.create_module("rock_module");
        let builder = context.create_builder();

        targets::Target::initialize_x86(&targets::InitializationConfig::default());
        let target = targets::Target::from_name("x86-64").unwrap();
        let target_machine = target
            .create_target_machine(
                &targets::TargetMachine::get_default_triple(),
                "x86-64",
                targets::TargetMachine::get_host_cpu_features()
                    .to_str()
                    .expect("utf-8"),
                inkwell::OptimizationLevel::None,
                targets::RelocMode::Default,
                targets::CodeModel::Default,
            )
            .unwrap();

        let ptr_type = context.ptr_type(0.into());
        let ptr_sized_int_type =
            context.ptr_sized_int_type(&target_machine.get_target_data(), None);
        let slice_type = context.struct_type(&[ptr_type.into(), ptr_sized_int_type.into()], false);

        Codegen {
            context,
            module,
            builder,
            target_machine,
            string_lits: Vec::with_capacity(hir.intern_string.get_all_strings().len()),
            structs: Vec::with_capacity(hir.structs.len()),
            consts: Vec::with_capacity(hir.consts.len()),
            globals: Vec::with_capacity(hir.globals.len()),
            function_values: Vec::with_capacity(hir.procs.len()),
            hir,
            c_functions: HashMap::new(),
            ptr_type,
            ptr_sized_int_type,
            slice_type,
        }
    }

    fn finish_module(self) -> Result<(module::Module<'ctx>, targets::TargetMachine), ErrorComp> {
        let cwd = fs_env::dir_get_current_working()?;
        self.module.print_to_file(cwd.join("emit_llvm.ll")).unwrap();

        if let Err(error) = self.module.verify() {
            Err(ErrorComp::message(format!(
                "internal codegen error: llvm module verify failed\nreason: {}",
                error
            )))
        } else {
            Ok((self.module, self.target_machine))
        }
    }

    //@duplicated with generation of procedure values and with indirect calls 07.05.24
    fn function_type(&self, proc_ty: &hir::ProcType) -> types::FunctionType<'ctx> {
        let mut param_types =
            Vec::<types::BasicMetadataTypeEnum>::with_capacity(proc_ty.params.len());

        for param in proc_ty.params {
            param_types.push(self.type_into_basic_metadata(*param));
        }

        match self.type_into_basic_option(proc_ty.return_ty) {
            Some(ty) => ty.fn_type(&param_types, proc_ty.is_variadic),
            None => self
                .context
                .void_type()
                .fn_type(&param_types, proc_ty.is_variadic),
        }
    }

    fn array_static_len(&self, len: hir::ArrayStaticLen) -> u64 {
        match len {
            hir::ArrayStaticLen::Immediate(len) => len.expect("array len is known"),
            hir::ArrayStaticLen::ConstEval(eval_id) => match self.hir.const_eval_value(eval_id) {
                hir::ConstValue::Int { val, neg, ty } => val,
                _ => panic!("array len must be int"),
            },
        }
    }

    fn type_into_any(&self, ty: hir::Type) -> types::AnyTypeEnum<'ctx> {
        match ty {
            hir::Type::Error => unreachable!(),
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::S8 => self.context.i8_type().into(),
                ast::BasicType::S16 => self.context.i16_type().into(),
                ast::BasicType::S32 => self.context.i32_type().into(),
                ast::BasicType::S64 => self.context.i64_type().into(),
                ast::BasicType::Ssize => self.ptr_sized_int_type.into(),
                ast::BasicType::U8 => self.context.i8_type().into(),
                ast::BasicType::U16 => self.context.i16_type().into(),
                ast::BasicType::U32 => self.context.i32_type().into(),
                ast::BasicType::U64 => self.context.i64_type().into(),
                ast::BasicType::Usize => self.ptr_sized_int_type.into(),
                ast::BasicType::F16 => self.context.f16_type().into(),
                ast::BasicType::F32 => self.context.f32_type().into(),
                ast::BasicType::F64 => self.context.f64_type().into(),
                ast::BasicType::Bool => self.context.bool_type().into(),
                ast::BasicType::Char => self.context.i32_type().into(),
                ast::BasicType::Rawptr => self.ptr_type.into(),
                ast::BasicType::Void => self.context.void_type().into(),
                ast::BasicType::Never => self.context.void_type().into(), // only expected as procedure return type
            },
            hir::Type::Enum(enum_id) => {
                let basic = self.hir.enum_data(enum_id).basic;
                self.basic_type_into_int(basic).into()
            }
            hir::Type::Union(union_id) => self.union_type(union_id).into(),
            hir::Type::Struct(struct_id) => self.struct_type(struct_id).into(),
            hir::Type::Reference(_, _) => self.ptr_type.into(),
            hir::Type::Procedure(_) => self.ptr_type.into(),
            hir::Type::ArraySlice(_) => self.slice_type.into(),
            hir::Type::ArrayStatic(array) => self.array_type(array).into(),
        }
    }

    fn union_type(&self, union_id: hir::UnionID) -> types::ArrayType<'ctx> {
        let data = self.hir.union_data(union_id);
        let size = data.size_eval.get_size().expect("resolved");
        self.basic_type_into_int(ast::BasicType::U8)
            .array_type(size.size() as u32)
    }

    fn struct_type(&self, struct_id: hir::StructID) -> types::StructType<'ctx> {
        self.structs[struct_id.index()]
    }

    fn array_type(&self, array: &hir::ArrayStatic) -> types::ArrayType<'ctx> {
        // @should use LLVMArrayType2 which takes u64, what not exposed 03.05.24
        //  by inkwell even for llvm 17 (LLVMArrayType was deprecated in this version)
        let elem_ty = self.type_into_basic(array.elem_ty);
        elem_ty.array_type(self.array_static_len(array.len) as u32)
    }

    fn basic_type_into_int(&self, basic: ast::BasicType) -> types::IntType<'ctx> {
        match basic {
            ast::BasicType::S8 => self.context.i8_type(),
            ast::BasicType::S16 => self.context.i16_type(),
            ast::BasicType::S32 => self.context.i32_type(),
            ast::BasicType::S64 => self.context.i64_type(),
            ast::BasicType::Ssize => self.ptr_sized_int_type,
            ast::BasicType::U8 => self.context.i8_type(),
            ast::BasicType::U16 => self.context.i16_type(),
            ast::BasicType::U32 => self.context.i32_type(),
            ast::BasicType::U64 => self.context.i64_type(),
            ast::BasicType::Usize => self.ptr_sized_int_type,
            _ => unreachable!(),
        }
    }

    fn basic_type_into_float(&self, basic: ast::BasicType) -> types::FloatType<'ctx> {
        match basic {
            ast::BasicType::F16 => self.context.f16_type(),
            ast::BasicType::F32 => self.context.f32_type(),
            ast::BasicType::F64 => self.context.f64_type(),
            _ => unreachable!(),
        }
    }

    #[inline]
    fn type_into_basic(&self, ty: hir::Type) -> types::BasicTypeEnum<'ctx> {
        self.type_into_any(ty).try_into().expect("value type")
    }

    #[inline]
    fn type_into_basic_option(&self, ty: hir::Type) -> Option<types::BasicTypeEnum<'ctx>> {
        self.type_into_any(ty).try_into().ok()
    }

    #[inline]
    fn type_into_basic_metadata(&self, ty: hir::Type) -> types::BasicMetadataTypeEnum<'ctx> {
        self.type_into_any(ty).try_into().expect("value type")
    }
}

pub fn codegen(
    hir: hir::Hir,
    bin_name: String,
    build_kind: BuildKind,
    args: Option<Vec<String>>,
) -> Result<(), ErrorComp> {
    let context = create_build_context(bin_name, build_kind)?;
    let context_llvm = context::Context::create();
    let (module, machine) = create_llvm_module(hir, &context_llvm)?;
    build_executable(&context, module, machine)?;
    run_executable(&context, args)?;
    Ok(())
}

fn create_build_context(
    bin_name: String,
    build_kind: BuildKind,
) -> Result<BuildContext, ErrorComp> {
    let mut build_dir = fs_env::dir_get_current_working()?;
    build_dir.push("build");
    fs_env::dir_create(&build_dir, false)?;
    build_dir.push(build_kind.as_str());
    fs_env::dir_create(&build_dir, false)?;

    let mut executable_path = build_dir.clone();
    executable_path.push(&bin_name);
    executable_path.set_extension("exe"); //@assuming windows

    let context = BuildContext {
        bin_name,
        build_kind,
        build_dir,
        executable_path,
    };
    Ok(context)
}

fn create_llvm_module<'ctx>(
    hir: hir::Hir<'ctx>,
    context_llvm: &'ctx context::Context,
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

fn build_executable<'ctx>(
    context: &BuildContext,
    module: module::Module<'ctx>,
    machine: targets::TargetMachine,
) -> Result<(), ErrorComp> {
    let object_path = context.build_dir.join(format!("{}.o", context.bin_name));
    machine
        .write_to_file(&module, targets::FileType::Object, &object_path)
        .map_err(|error| {
            ErrorComp::message(format!(
                "failed to write llvm module as object file\nreason: {}",
                error
            ))
        })?;

    let arg_obj = object_path.to_string_lossy().to_string();
    let arg_out = format!("/out:{}", context.executable_path.to_string_lossy());
    let mut args = vec![arg_obj, arg_out];

    //@check if they need to be comma separated instead of being separate
    match context.build_kind {
        BuildKind::Debug => {
            args.push("/opt:noref".into());
            args.push("/opt:noicf".into());
            args.push("/opt:nolbr".into());
        }
        BuildKind::Release => {
            args.push("/opt:ref".into());
            args.push("/opt:icf".into());
            args.push("/opt:lbr".into());
        }
    }

    //@assuming windows
    if true {
        //sybsystem needs to be specified on windows (console, windows)
        //@only console with `main` entry point is supported, support WinMain when such feature is required 29.05.24
        args.push("/subsystem:console".into());
        // link with C runtime library: libcmt.lib (static), msvcrt.lib (dynamic)
        //@always linking with static C runtime library, support attributes or toml configs 29.05.24
        // to change this if needed, this might be a problem when trying to link C libraries (eg: raylib.lib)
        args.push("/defaultlib:libcmt.lib".into());
    } else {
        panic!("only windows targets are supported");
    }

    // lld-link is called system wide, and requires llvm being installed @29.05.24
    // test and use bundled lld-link relative to install path instead
    let _ = std::process::Command::new("lld-link")
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to link object file `{}`\nreason: {}",
                object_path.to_string_lossy(),
                io_error
            ))
        })?;
    fs_env::file_remove(&object_path)?;
    Ok(())
}

fn run_executable(context: &BuildContext, args: Option<Vec<String>>) -> Result<(), ErrorComp> {
    let args = match args {
        Some(args) => args,
        None => return Ok(()),
    };

    std::process::Command::new(context.executable_path.as_os_str())
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to run executable `{}`\nreason: {}",
                context.executable_path.to_string_lossy(),
                io_error
            ))
        })?;

    Ok(())
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
    let mut param_types = Vec::<types::BasicMetadataTypeEnum>::new();
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
        };
        codegen_block(cg, &mut proc_cg, block, BlockKind::TailReturn);

        //@hack fix-up of single implicit `return void` basic block
        for block in function.get_basic_block_iter() {
            if block.get_terminator().is_none() {
                cg.builder.position_at_end(block);
                cg.builder.build_return(None).unwrap();
                break;
            }
        }
    }
}

fn codegen_const_value<'ctx>(
    cg: &Codegen<'ctx>,
    value: hir::ConstValue<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    match value {
        hir::ConstValue::Error => panic!("codegen unexpected ConstValue::Error"),
        hir::ConstValue::Null => cg.ptr_type.const_zero().into(),
        hir::ConstValue::Bool { val } => cg.context.bool_type().const_int(val as u64, false).into(),
        hir::ConstValue::Int { val, neg, ty } => {
            let ty = ty.expect("const int type");
            //@using sign_extend as neg, most likely incorrect (learn how to properly supply integers to llvm const int) 14.05.24
            cg.basic_type_into_int(ty).const_int(val, neg).into()
        }
        hir::ConstValue::Float { val, ty } => {
            let ty = ty.expect("const float type");
            cg.basic_type_into_float(ty).const_float(val).into()
        }
        hir::ConstValue::Char { val } => cg.context.i32_type().const_int(val as u64, false).into(),
        hir::ConstValue::String { id, c_string } => codegen_lit_string(cg, id, c_string),
        hir::ConstValue::Procedure { proc_id } => cg.function_values[proc_id.index()]
            .as_global_value()
            .as_pointer_value()
            .into(),
        hir::ConstValue::EnumVariant {
            enum_id,
            variant_id,
        } => {
            let variant = cg.hir.enum_data(enum_id).variant(variant_id);
            codegen_const_value(cg, cg.hir.const_eval_value(variant.value))
        }
        hir::ConstValue::Struct { struct_ } => {
            let mut values = Vec::with_capacity(struct_.fields.len());
            for field in struct_.fields {
                let value = codegen_const_value(cg, cg.hir.const_value(*field));
                values.push(value);
            }
            let struct_ty = cg.struct_type(struct_.struct_id);
            struct_ty.const_named_struct(&values).into()
        }
        hir::ConstValue::Array { array } => {
            let mut values = Vec::with_capacity(array.values.len());
            for input in array.values {
                let value = codegen_const_value(cg, cg.hir.const_value(*input));
                //@api for const array is wrong? forcing to use array_value for elements?
                values.push(value.into_array_value());
            }
            //@relying on having a 1+ value to know the type
            let array_ty = values[0].get_type().array_type(array.values.len() as u32);
            array_ty.const_array(&values).into()
        }
        hir::ConstValue::ArrayRepeat { value, len } => {
            todo!("codegen ConstValue::ArrayRepeat unsupported")
        }
    }
}

//@current lit string codegen doesnt deduplicate strings
// by their intern ID, this is temporary.
// global is created for each occurence of the string literal @07.04.24
#[allow(unsafe_code)]
fn codegen_lit_string<'ctx>(
    cg: &Codegen<'ctx>,
    id: InternID,
    c_string: bool,
) -> values::BasicValueEnum<'ctx> {
    let global_ptr = cg.string_lits[id.index()].as_pointer_value();

    if c_string {
        global_ptr.into()
    } else {
        let string = cg.hir.intern_string.get_str(id);
        let bytes_len = cg.ptr_sized_int_type.const_int(string.len() as u64, false);
        let slice_value = cg
            .context
            .const_struct(&[global_ptr.into(), bytes_len.into()], false);
        slice_value.into()
    }
}

//@hir still has tail returned expressions in statements,  and block is an expression
// this results in need to return Optional values from codegen_expr()
// and a lot of unwrap() or expect() calls on always expected values
//@also top level codegen_procedures builds return from tail expr value  if it exists
// hir could potentially generate code without tail returns (not sure yet) @06.04.24
fn codegen_expr<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    expr: &'ctx hir::Expr<'ctx>,
    kind: BlockKind<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    use hir::Expr;
    match *expr {
        Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        Expr::Const { value } => Some(codegen_const_value(cg, value)),
        Expr::If { if_ } => {
            codegen_if(cg, proc_cg, if_, kind);
            None
        }
        Expr::Block { block } => {
            codegen_block(cg, proc_cg, block, kind);
            None
        }
        Expr::Match { match_ } => {
            codegen_match(cg, proc_cg, match_, kind);
            None
        }
        Expr::UnionMember {
            target,
            union_id,
            member_id,
            deref,
        } => Some(codegen_union_member(
            cg, proc_cg, expect_ptr, target, union_id, member_id, deref,
        )),
        Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => Some(codegen_struct_field(
            cg, proc_cg, expect_ptr, target, struct_id, field_id, deref,
        )),
        Expr::SliceField {
            target,
            first_ptr,
            deref,
        } => Some(codegen_slice_field(
            cg, proc_cg, expect_ptr, target, first_ptr, deref,
        )),
        Expr::Index { target, access } => {
            Some(codegen_index(cg, proc_cg, expect_ptr, target, access))
        }
        Expr::Slice { target, access } => {
            Some(codegen_slice(cg, proc_cg, expect_ptr, target, access))
        }
        Expr::Cast { target, into, kind } => Some(codegen_cast(cg, proc_cg, target, into, kind)),
        Expr::LocalVar { local_id } => Some(codegen_local_var(cg, proc_cg, expect_ptr, local_id)),
        Expr::ParamVar { param_id } => Some(codegen_param_var(cg, proc_cg, expect_ptr, param_id)),
        Expr::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, expect_ptr, global_id)),
        Expr::CallDirect { proc_id, input } => codegen_call_direct(cg, proc_cg, proc_id, input),
        Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, proc_cg, target, indirect)
        }
        Expr::UnionInit { union_id, input } => {
            Some(codegen_union_init(cg, proc_cg, expect_ptr, union_id, input))
        }
        Expr::StructInit { struct_id, input } => Some(codegen_struct_init(
            cg, proc_cg, expect_ptr, struct_id, input,
        )),
        Expr::ArrayInit { array_init } => {
            Some(codegen_array_init(cg, proc_cg, expect_ptr, array_init))
        }
        Expr::ArrayRepeat { array_repeat } => Some(codegen_array_repeat(cg, proc_cg, array_repeat)),
        Expr::Address { rhs } => Some(codegen_address(cg, proc_cg, rhs)),
        Expr::Unary { op, rhs } => Some(codegen_unary(cg, proc_cg, op, rhs)),
        Expr::Binary {
            op,
            lhs,
            rhs,
            lhs_signed_int,
        } => Some(codegen_binary(cg, proc_cg, op, lhs, rhs, lhs_signed_int)),
    }
}

fn codegen_if<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    if_: &'ctx hir::If<'ctx>,
    kind: BlockKind<'ctx>,
) {
    let cond_block = cg.context.append_basic_block(proc_cg.function, "if_cond");
    let mut body_block = cg.context.append_basic_block(proc_cg.function, "if_body");
    let exit_block = cg.context.append_basic_block(proc_cg.function, "if_exit");

    // next if_cond or if_else block
    let mut next_block = if !if_.branches.is_empty() || if_.else_block.is_some() {
        cg.context.insert_basic_block_after(body_block, "if_next")
    } else {
        exit_block
    };

    cg.builder.build_unconditional_branch(cond_block).unwrap();
    cg.builder.position_at_end(cond_block);
    let cond =
        codegen_expr(cg, proc_cg, false, if_.entry.cond, BlockKind::TailAlloca).expect("value");
    cg.builder
        .build_conditional_branch(cond.into_int_value(), body_block, next_block)
        .unwrap();

    cg.builder.position_at_end(body_block);
    codegen_block(cg, proc_cg, if_.entry.block, kind);
    cg.builder.build_unconditional_branch(exit_block).unwrap();

    for (idx, branch) in if_.branches.iter().enumerate() {
        let last = idx + 1 == if_.branches.len();
        let create_next = !last || if_.else_block.is_some();

        body_block = cg.context.insert_basic_block_after(next_block, "if_body");

        cg.builder.position_at_end(next_block);
        let cond =
            codegen_expr(cg, proc_cg, false, branch.cond, BlockKind::TailAlloca).expect("value");
        next_block = if create_next {
            cg.context.insert_basic_block_after(body_block, "if_next")
        } else {
            exit_block
        };
        cg.builder
            .build_conditional_branch(cond.into_int_value(), body_block, next_block)
            .unwrap();

        cg.builder.position_at_end(body_block);
        codegen_block(cg, proc_cg, branch.block, kind);
        cg.builder.build_unconditional_branch(exit_block).unwrap();
    }

    if let Some(block) = if_.else_block {
        cg.builder.position_at_end(next_block);
        codegen_block(cg, proc_cg, block, kind);
        cg.builder.build_unconditional_branch(exit_block).unwrap();
    }

    cg.builder.position_at_end(exit_block);
}

#[derive(Copy, Clone)]
enum BlockKind<'ctx> {
    /// dont do anything special with tail value  
    /// used for blocks that expect void
    TailIgnore,
    /// dissalow any tail expression  
    /// used for non addressable expressions
    TailDissalow,
    /// return tail value
    TailReturn,
    /// allocate tail value
    TailAlloca,
    /// store tail value
    TailStore(values::PointerValue<'ctx>),
}

fn codegen_block<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    block: hir::Block<'ctx>,
    kind: BlockKind<'ctx>,
    //@dont return any value from blocks? 31.05.24
    // only simulate that effect via BlockKind::TailStore etc.
) {
    proc_cg.enter_block();

    //@disregarding divergence will lead to trying to generate @31.04.24
    // exiting defers incorrectly, hard problem not sure what todo
    for stmt in block.stmts {
        match *stmt {
            hir::Stmt::Break => {
                let (loop_info, defer_blocks) = proc_cg.last_loop_info();
                codegen_defer_blocks(cg, proc_cg, &defer_blocks);
                cg.builder
                    .build_unconditional_branch(loop_info.break_bb)
                    .unwrap();

                proc_cg.exit_block();
                return;
            }
            hir::Stmt::Continue => {
                let (loop_info, defer_blocks) = proc_cg.last_loop_info();
                codegen_defer_blocks(cg, proc_cg, &defer_blocks);
                cg.builder
                    .build_unconditional_branch(loop_info.continue_bb)
                    .unwrap();

                proc_cg.exit_block();
                return;
            }
            hir::Stmt::Return(expr) => {
                codegen_defer_blocks(cg, proc_cg, proc_cg.all_defer_blocks().as_slice());

                if let Some(expr) = expr {
                    if let Some(value) =
                        codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca)
                    {
                        cg.builder.build_return(Some(&value)).unwrap();
                    } else {
                        cg.builder.build_return(None).unwrap();
                    }
                } else {
                    cg.builder.build_return(None).unwrap();
                }

                proc_cg.exit_block();
                return;
            }
            hir::Stmt::Defer(block) => {
                proc_cg.push_defer_block(*block);
            }
            hir::Stmt::Loop(loop_) => {
                let entry_block = cg
                    .context
                    .append_basic_block(proc_cg.function, "loop_entry");
                let body_block = cg.context.append_basic_block(proc_cg.function, "loop_body");
                let exit_block = cg.context.append_basic_block(proc_cg.function, "loop_exit");

                cg.builder.build_unconditional_branch(entry_block).unwrap();
                proc_cg.set_next_loop_info(exit_block, entry_block);

                match loop_.kind {
                    hir::LoopKind::Loop => {
                        cg.builder.position_at_end(entry_block);
                        cg.builder.build_unconditional_branch(body_block).unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

                        //@hack, might not be valid when break / continue are used 06.05.24
                        // other cfg will make body_block not the actual block we want
                        if body_block.get_terminator().is_none() {
                            cg.builder.position_at_end(body_block);
                            cg.builder.build_unconditional_branch(body_block).unwrap();
                        }
                    }
                    hir::LoopKind::While { cond } => {
                        cg.builder.position_at_end(entry_block);
                        let cond = codegen_expr(cg, proc_cg, false, cond, BlockKind::TailAlloca)
                            .expect("value");
                        cg.builder
                            .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                            .unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

                        //@hack, likely wrong if positioned in the wrong place
                        if cg
                            .builder
                            .get_insert_block()
                            .unwrap()
                            .get_terminator()
                            .is_none()
                        {
                            cg.builder.build_unconditional_branch(entry_block).unwrap();
                        }
                    }
                    hir::LoopKind::ForLoop {
                        local_id,
                        cond,
                        assign,
                    } => {
                        cg.builder.position_at_end(entry_block);
                        codegen_local(cg, proc_cg, local_id);
                        let cond = codegen_expr(cg, proc_cg, false, cond, BlockKind::TailAlloca)
                            .expect("value");
                        cg.builder
                            .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                            .unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

                        //@hack, often invalid (this assignment might need special block) if no iterator abstractions are used
                        // in general loops need to be simplified in Hir, to loops and conditional breaks 06.05.24
                        cg.builder.position_at_end(body_block);
                        codegen_assign(cg, proc_cg, assign);

                        //@hack, likely wrong if positioned in the wrong place
                        if cg
                            .builder
                            .get_insert_block()
                            .unwrap()
                            .get_terminator()
                            .is_none()
                        {
                            cg.builder.build_unconditional_branch(entry_block).unwrap();
                        }
                    }
                }

                cg.builder.position_at_end(exit_block);
            }
            hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
            hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
            hir::Stmt::ExprSemi(expr) => {
                //@are expressions like `5;` valid when output as llvm ir? probably yes
                codegen_expr(cg, proc_cg, false, expr, BlockKind::TailIgnore);
            }
            hir::Stmt::ExprTail(expr) => {
                match kind {
                    BlockKind::TailIgnore => {
                        codegen_defer_blocks(cg, proc_cg, &proc_cg.last_defer_blocks());
                        let _ = codegen_expr(cg, proc_cg, false, expr, kind);

                        proc_cg.exit_block();
                        return;
                    }
                    BlockKind::TailDissalow => panic!("tail expression is dissalowed"),
                    BlockKind::TailReturn => {
                        //@defer might be generated again if tail returns are stacked?
                        // is this correct? 30.05.24
                        codegen_defer_blocks(cg, proc_cg, &proc_cg.all_defer_blocks());
                        //@handle tail return kind differently?
                        if let Some(value) =
                            codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca)
                        {
                            cg.builder.build_return(Some(&value)).unwrap();
                        } else {
                            cg.builder.build_return(None).unwrap();
                        }

                        proc_cg.exit_block();
                        return;
                    }
                    BlockKind::TailAlloca => {
                        panic!("tail alloca is not implemented");
                    }
                    BlockKind::TailStore(target_ptr) => {
                        codegen_defer_blocks(cg, proc_cg, &proc_cg.last_defer_blocks());
                        if let Some(value) = codegen_expr(cg, proc_cg, false, expr, kind) {
                            cg.builder.build_store(target_ptr, value).unwrap();
                        }

                        proc_cg.exit_block();
                        return;
                    }
                }
            }
        }
    }

    //@hack partially fix ignoring divergense when generating block exit `defers`
    // still likely to false generate `defers` in match or if else chains
    let insert_block = cg.builder.get_insert_block().unwrap();
    if insert_block.get_terminator().is_none() {
        codegen_defer_blocks(cg, proc_cg, proc_cg.last_defer_blocks().as_slice());
    }

    proc_cg.exit_block();
    return;
}

//@contents should be generated once, instead of generating all block code each time
// and only branches to defer blocks should be created? hard to design currently @06.05.24
fn codegen_defer_blocks<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    defer_blocks: &[hir::Block<'ctx>],
) {
    if defer_blocks.is_empty() {
        return;
    }

    let mut defer_block = cg
        .context
        .append_basic_block(proc_cg.function, "defer_entry");

    for block in defer_blocks.iter().copied().rev() {
        cg.builder.build_unconditional_branch(defer_block).unwrap();
        cg.builder.position_at_end(defer_block);
        codegen_block(cg, proc_cg, block, BlockKind::TailIgnore);
        defer_block = cg
            .context
            .append_basic_block(proc_cg.function, "defer_next");
    }

    cg.builder.build_unconditional_branch(defer_block).unwrap();
    cg.builder.position_at_end(defer_block);
}

//@variables without value expression are always zero initialized
// theres no way to detect potentially uninitialized variables
// during check and analysis phases, this might change. @06.04.24
fn codegen_local<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    local_id: hir::LocalID,
) {
    let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
    let var_ptr = proc_cg.local_vars[local_id.index()];

    if let Some(expr) = local.value {
        if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailStore(var_ptr)) {
            cg.builder.build_store(var_ptr, value).unwrap();
        }
    } else {
        let zero_value = cg.type_into_basic(local.ty).const_zero();
        cg.builder.build_store(var_ptr, zero_value).unwrap();
    }
}

fn codegen_assign<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    assign: &hir::Assign<'ctx>,
) {
    let lhs_ptr = codegen_expr(cg, proc_cg, true, assign.lhs, BlockKind::TailDissalow)
        .expect("value")
        .into_pointer_value();

    match assign.op {
        ast::AssignOp::Assign => {
            let rhs = codegen_expr(
                cg,
                proc_cg,
                false,
                assign.rhs,
                BlockKind::TailStore(lhs_ptr),
            );
            if let Some(value) = rhs {
                cg.builder.build_store(lhs_ptr, value).unwrap();
            }
        }
        ast::AssignOp::Bin(op) => {
            let lhs_ty = cg.type_into_basic(assign.lhs_ty);
            let lhs_value = cg.builder.build_load(lhs_ty, lhs_ptr, "load_val").unwrap();
            let rhs =
                codegen_expr(cg, proc_cg, false, assign.rhs, BlockKind::TailAlloca).expect("value");
            let bin_value = codegen_bin_op(cg, op, lhs_value, rhs, assign.lhs_signed_int);
            cg.builder.build_store(lhs_ptr, bin_value).unwrap();
        }
    }
}

//@since its an expr semantics are not clear 01.06.24
// will need to store, temp alloca return those values
// generate hir blocks for non block expressions?
fn codegen_match<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    match_: &hir::Match<'ctx>,
    kind: BlockKind<'ctx>,
) {
    let insert_bb = cg.builder.get_insert_block().unwrap();
    let on_value =
        codegen_expr(cg, proc_cg, false, match_.on_expr, BlockKind::TailAlloca).expect("value");
    let exit_bb = cg
        .context
        .append_basic_block(proc_cg.function, "match_exit");

    let mut cases = Vec::with_capacity(match_.arms.len());
    for arm in match_.arms {
        let value = codegen_const_value(cg, cg.hir.const_value(arm.pat));
        let case_bb = cg
            .context
            .append_basic_block(proc_cg.function, "match_case");
        cases.push((value.into_int_value(), case_bb));

        cg.builder.position_at_end(case_bb);
        codegen_expr(cg, proc_cg, false, arm.expr, kind);

        if cg
            .builder
            .get_insert_block()
            .unwrap()
            .get_terminator()
            .is_none()
        {
            cg.builder.build_unconditional_branch(exit_bb).unwrap();
        }
    }

    if let Some(fallback) = match_.fallback {
        panic!("codegen: match with fallback not supported");
    } else {
        cg.builder.position_at_end(insert_bb);
        cg.builder
            .build_switch(on_value.into_int_value(), exit_bb, &cases)
            .unwrap();
        cg.builder.position_at_end(exit_bb);
    }
}

fn codegen_union_member<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    union_id: hir::UnionID,
    member_id: hir::UnionMemberID,
    deref: bool,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr(cg, proc_cg, true, target, BlockKind::TailAlloca).expect("value");
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type, target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    if expect_ptr {
        target_ptr.into()
    } else {
        let member = cg.hir.union_data(union_id).member(member_id);
        let member_ty = cg.type_into_basic(member.ty);
        cg.builder
            .build_load(member_ty, target_ptr, "member_val")
            .unwrap()
    }
}

fn codegen_struct_field<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    struct_id: hir::StructID,
    field_id: hir::StructFieldID,
    deref: bool,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr(cg, proc_cg, true, target, BlockKind::TailAlloca).expect("value");
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type, target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    let field_ptr = cg
        .builder
        .build_struct_gep(
            cg.struct_type(struct_id),
            target_ptr,
            field_id.index() as u32,
            "field_ptr",
        )
        .unwrap();

    if expect_ptr {
        field_ptr.into()
    } else {
        let field = cg.hir.struct_data(struct_id).field(field_id);
        let field_ty = cg.type_into_basic(field.ty);
        cg.builder
            .build_load(field_ty, field_ptr, "field_val")
            .unwrap()
    }
}

fn codegen_slice_field<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr<'ctx>,
    first_ptr: bool,
    deref: bool,
) -> values::BasicValueEnum<'ctx> {
    assert!(
        !expect_ptr,
        "slice access `expect_ptr` cannot be true, slice fields are not addressable"
    );
    let target = codegen_expr(cg, proc_cg, true, target, BlockKind::TailAlloca).expect("value");
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type, target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    let (field_id, field_ty, ptr_name, value_name) = if first_ptr {
        (
            0,
            cg.ptr_type.as_basic_type_enum(),
            "slice_ptr_ptr",
            "slice_ptr",
        )
    } else {
        (
            1,
            cg.ptr_sized_int_type.as_basic_type_enum(),
            "slice_len_ptr",
            "slice_len",
        )
    };

    let field_ptr = cg
        .builder
        .build_struct_gep(cg.slice_type, target_ptr, field_id, ptr_name)
        .unwrap();
    cg.builder
        .build_load(field_ty, field_ptr, value_name)
        .unwrap()
}

//@change to eprintf, re-use panic message strings (not generated for each panic) 07.05.24
// panics should be hooks into core library panicking module
// it could define how panic works (eg: calling epintf + exit + unreachable?)
fn codegen_panic_conditional<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    cond: values::IntValue<'ctx>,
    printf_args: &[values::BasicMetadataValueEnum<'ctx>],
) {
    let panic_block = cg
        .context
        .append_basic_block(proc_cg.function, "panic_block");
    let else_block = cg.context.append_basic_block(proc_cg.function, "block");
    cg.builder
        .build_conditional_branch(cond, panic_block, else_block)
        .unwrap();

    let c_exit = cg
        .c_functions
        .get(&cg.hir.intern_name.get_id("exit").expect("exit c function"))
        .cloned()
        .expect("exit c function added");
    let c_printf = cg
        .c_functions
        .get(
            &cg.hir
                .intern_name
                .get_id("printf")
                .expect("printf c function"),
        )
        .cloned()
        .expect("printf c function added");

    //@print to stderr instead of stdout & have better panic handling api 04.05.24
    // this is first draft of working panic messages and exit
    cg.builder.position_at_end(panic_block);
    cg.builder.build_call(c_printf, printf_args, "").unwrap();
    cg.builder
        .build_call(
            c_exit,
            &[cg.context.i32_type().const_int(1, true).into()],
            "",
        )
        .unwrap();
    cg.builder.build_unreachable().unwrap();

    cg.builder.position_at_end(else_block);
}

//@fix how bounds check is done 31.05.24
// possible feature is to not bounds check indexing with constant values
// that are proven to be in bounds of static array type, store flag in hir
#[allow(unsafe_code)]
fn codegen_index<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    access: &'ctx hir::IndexAccess,
) -> values::BasicValueEnum<'ctx> {
    //@should expect pointer always be true? 08.05.24
    // in case of slices that just delays the load?
    let target = codegen_expr(cg, proc_cg, true, target, BlockKind::TailAlloca).expect("value");
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type, target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    let index = codegen_expr(cg, proc_cg, false, access.index, BlockKind::TailAlloca)
        .expect("value")
        .into_int_value();

    let elem_ptr = match access.kind {
        hir::IndexKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type, target_ptr, "slice_val")
                .unwrap()
                .into_struct_value();
            let ptr = cg
                .builder
                .build_extract_value(slice, 0, "slice_ptr")
                .unwrap()
                .into_pointer_value();
            let len = cg
                .builder
                .build_extract_value(slice, 1, "slice_len")
                .unwrap()
                .into_int_value();

            let panic_cond = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::UGE, index, len, "bounds_check")
                .unwrap();
            let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: index `%llu` out of bounds, slice len = `%llu`\n\n";
            let messsage_ptr = cg
                .builder
                .build_global_string_ptr(message, "panic_index_out_of_bounds")
                .unwrap()
                .as_pointer_value();
            codegen_panic_conditional(
                cg,
                proc_cg,
                panic_cond,
                &[messsage_ptr.into(), index.into(), len.into()],
            );

            //@i64 mul is probably wrong when dealing with non 64bit targets 07.05.24
            let elem_size = cg.context.i64_type().const_int(elem_size, false);
            let byte_offset =
                codegen_bin_op(cg, ast::BinOp::Mul, index.into(), elem_size.into(), false)
                    .into_int_value();
            unsafe {
                cg.builder
                    .build_in_bounds_gep(cg.context.i8_type(), ptr, &[byte_offset], "elem_ptr")
                    .unwrap()
            }
        }
        hir::IndexKind::Array { array } => unsafe {
            let len = cg.array_static_len(array.len);
            let len = cg.ptr_sized_int_type.const_int(len, false);

            let panic_cond = cg
                .builder
                .build_int_compare(inkwell::IntPredicate::UGE, index, len, "bounds_check")
                .unwrap();
            let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: index `%llu` out of bounds, array len = `%llu`\n\n";
            let messsage_ptr = cg
                .builder
                .build_global_string_ptr(message, "panic_index_out_of_bounds")
                .unwrap()
                .as_pointer_value();
            codegen_panic_conditional(
                cg,
                proc_cg,
                panic_cond,
                &[messsage_ptr.into(), index.into(), len.into()],
            );

            cg.builder
                .build_in_bounds_gep(
                    cg.array_type(array),
                    target_ptr,
                    &[cg.ptr_sized_int_type.const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        },
    };

    if expect_ptr {
        elem_ptr.into()
    } else {
        let elem_ty = cg.type_into_basic(access.elem_ty);
        cg.builder
            .build_load(elem_ty, elem_ptr, "elem_val")
            .unwrap()
    }
}

fn codegen_slice<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &'ctx hir::Expr,
    access: &'ctx hir::SliceAccess,
) -> values::BasicValueEnum<'ctx> {
    //@should expect pointer always be true? 08.05.24
    // in case of slices that just delays the load?
    // causes problem when slicing multiple times into_pointer_value() gets called on new_slice_value that is not a pointer
    let target = codegen_expr(cg, proc_cg, true, target, BlockKind::TailAlloca).expect("value");
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type, target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    match access.kind {
        hir::SliceKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type, target_ptr, "slice_val")
                .unwrap()
                .into_struct_value();
            let slice_len = cg
                .builder
                .build_extract_value(slice, 1, "slice_len")
                .unwrap()
                .into_int_value();
            let slice_ptr = cg
                .builder
                .build_extract_value(slice, 0, "slice_ptr")
                .unwrap()
                .into_pointer_value();

            let lower = match access.range.lower {
                Some(lower) => Some(
                    codegen_expr(cg, proc_cg, false, lower, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
                hir::SliceRangeEnd::Inclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
            };

            match (lower, upper) {
                // slice is unchanged
                //@slice and its components are still extracted above even in this no-op 08.05.24
                (None, None) => {
                    return if expect_ptr {
                        //@returning pointer to same slice? 08.05.24
                        // that can be misleading? or no-op like this makes sence?
                        // probably this is valid and will reduce all RangeFull sling operations into one
                        target_ptr.into()
                    } else {
                        slice.into()
                    };
                }
                // upper is provided
                (None, Some(upper)) => {
                    let predicate =
                        if matches!(access.range.upper, hir::SliceRangeEnd::Exclusive(..)) {
                            inkwell::IntPredicate::UGT // upper > len
                        } else {
                            inkwell::IntPredicate::UGE // upper >= len
                        };
                    let panic_cond = cg
                        .builder
                        .build_int_compare(predicate, upper, slice_len, "slice_upper_bound")
                        .unwrap();
                    let message = "thread `name` panicked at src/some_file.rock:xx:xx\nreason: slice upper `%llu` out of bounds, slice len = `%llu`\n\n";
                    let messsage_ptr = cg
                        .builder
                        .build_global_string_ptr(message, "panic_index_out_of_bounds")
                        .unwrap()
                        .as_pointer_value();
                    codegen_panic_conditional(
                        cg,
                        proc_cg,
                        panic_cond,
                        &[messsage_ptr.into(), upper.into(), slice_len.into()],
                    );

                    // sub 1 in case of exclusive range
                    let new_slice_len =
                        if matches!(access.range.upper, hir::SliceRangeEnd::Exclusive(..)) {
                            //codegen_bin_op(
                            //    cg,
                            //    ast::BinOp::Sub,
                            //    upper.into(),
                            //    cg.ptr_sized_int_type.const_int(1, false).into(),
                            //    false,
                            //)
                            //.into_int_value()
                            upper
                        } else {
                            codegen_bin_op(
                                cg,
                                ast::BinOp::Add,
                                upper.into(),
                                cg.ptr_sized_int_type.const_int(1, false).into(),
                                false,
                            )
                            .into_int_value()
                        };

                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type;

                    let new_slice = cg
                        .builder
                        .build_alloca(slice_type, "new_slice_ptr")
                        .unwrap();
                    let new_slice_0 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 0, "new_slice_ptr_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_0, slice_ptr).unwrap();
                    let new_slice_1 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 1, "new_slice_len_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_1, new_slice_len).unwrap();

                    if expect_ptr {
                        new_slice.into()
                    } else {
                        cg.builder
                            .build_load(slice_type, new_slice, "new_slice")
                            .unwrap()
                    }
                }
                // lower is provided
                (Some(lower), None) => {
                    // @temp
                    panic!("slice slicing lower.. not implemented");
                }
                // lower and uppoer are provided
                (Some(lower), Some(upper)) => {
                    // @temp
                    panic!("slice slicing lower..upper not implemented");
                }
            }
        }
        hir::SliceKind::Array { array } => {
            let len = cg.array_static_len(array.len);
            let len = cg.ptr_sized_int_type.const_int(len, false);

            let lower = match access.range.lower {
                Some(lower) => Some(
                    codegen_expr(cg, proc_cg, false, lower, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
                hir::SliceRangeEnd::Inclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper, BlockKind::TailAlloca)
                        .expect("value")
                        .into_int_value(),
                ),
            };

            match (lower, upper) {
                (None, None) => {
                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type;

                    let new_slice = cg
                        .builder
                        .build_alloca(slice_type, "new_slice_ptr")
                        .unwrap();
                    let new_slice_0 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 0, "new_slice_ptr_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_0, target_ptr).unwrap();
                    let new_slice_1 = cg
                        .builder
                        .build_struct_gep(slice_type, new_slice, 1, "new_slice_len_ptr")
                        .unwrap();
                    cg.builder.build_store(new_slice_1, len).unwrap();

                    if expect_ptr {
                        new_slice.into()
                    } else {
                        cg.builder
                            .build_load(slice_type, new_slice, "new_slice")
                            .unwrap()
                    }
                }
                (None, Some(_)) => todo!("array slicing ..upper not implemented"),
                (Some(_), None) => todo!("array slicing lower..upper not implemented"),
                (Some(_), Some(_)) => todo!("array slicing lower..upper not implemented"),
            }
        }
    }
}

fn codegen_cast<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    target: &'ctx hir::Expr,
    into: &'ctx hir::Type,
    kind: hir::CastKind,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr(cg, proc_cg, false, target, BlockKind::TailAlloca).expect("value");
    let into = cg.type_into_basic(*into);
    let op = match kind {
        hir::CastKind::Error => panic!("codegen unexpected hir::CastKind::Error"),
        hir::CastKind::NoOp => return target,
        hir::CastKind::Integer_Trunc => values::InstructionOpcode::Trunc,
        hir::CastKind::Sint_Sign_Extend => values::InstructionOpcode::SExt,
        hir::CastKind::Uint_Zero_Extend => values::InstructionOpcode::ZExt,
        hir::CastKind::Float_to_Sint => values::InstructionOpcode::FPToSI,
        hir::CastKind::Float_to_Uint => values::InstructionOpcode::FPToUI,
        hir::CastKind::Sint_to_Float => values::InstructionOpcode::SIToFP,
        hir::CastKind::Uint_to_Float => values::InstructionOpcode::UIToFP,
        hir::CastKind::Float_Trunc => values::InstructionOpcode::FPTrunc,
        hir::CastKind::Float_Extend => values::InstructionOpcode::FPExt,
    };
    cg.builder.build_cast(op, target, into, "cast_val").unwrap()
}

fn codegen_local_var<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    expect_ptr: bool,
    local_id: hir::LocalID,
) -> values::BasicValueEnum<'ctx> {
    let local_ptr = proc_cg.local_vars[local_id.index()];

    if expect_ptr {
        local_ptr.into()
    } else {
        let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
        let local_ty = cg.type_into_basic(local.ty);
        cg.builder
            .build_load(local_ty, local_ptr, "local_val")
            .unwrap()
    }
}

fn codegen_param_var<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &ProcCodegen<'ctx>,
    expect_ptr: bool,
    param_id: hir::ProcParamID,
) -> values::BasicValueEnum<'ctx> {
    let param_ptr = proc_cg.param_vars[param_id.index()];

    if expect_ptr {
        param_ptr.into()
    } else {
        let param = cg.hir.proc_data(proc_cg.proc_id).param(param_id);
        let param_ty = cg.type_into_basic(param.ty);
        cg.builder
            .build_load(param_ty, param_ptr, "param_val")
            .unwrap()
    }
}

fn codegen_const_var<'ctx>(
    cg: &Codegen<'ctx>,
    const_id: hir::ConstID,
) -> values::BasicValueEnum<'ctx> {
    cg.consts[const_id.index()]
}

fn codegen_global_var<'ctx>(
    cg: &Codegen<'ctx>,
    expect_ptr: bool,
    global_id: hir::GlobalID,
) -> values::BasicValueEnum<'ctx> {
    let global = cg.globals[global_id.index()];
    let global_ptr = global.as_pointer_value();

    if expect_ptr {
        global_ptr.into()
    } else {
        let global_ty = global.get_initializer().expect("initialized").get_type();
        cg.builder
            .build_load(global_ty, global_ptr, "global_val")
            .unwrap()
    }
}

fn codegen_call_direct<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    proc_id: hir::ProcID,
    input: &'ctx [&'ctx hir::Expr],
) -> Option<values::BasicValueEnum<'ctx>> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca).expect("value");
        input_values.push(value.into());
    }

    let function = cg.function_values[proc_id.index()];
    let call_val = cg
        .builder
        .build_direct_call(function, &input_values, "call_val")
        .unwrap();
    call_val.try_as_basic_value().left()
}

fn codegen_call_indirect<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    target: &'ctx hir::Expr,
    indirect: &'ctx hir::CallIndirect,
) -> Option<values::BasicValueEnum<'ctx>> {
    let function_ptr = codegen_expr(cg, proc_cg, false, target, BlockKind::TailAlloca)
        .expect("value")
        .into_pointer_value();

    let mut input_values = Vec::with_capacity(indirect.input.len());
    for &expr in indirect.input {
        let value = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca).expect("value");
        input_values.push(value.into());
    }

    let function = cg.function_type(&indirect.proc_ty);
    let call_val = cg
        .builder
        .build_indirect_call(function, function_ptr, &input_values, "call_val")
        .unwrap();
    call_val.try_as_basic_value().left()
}

fn codegen_union_init<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    union_id: hir::UnionID,
    input: hir::UnionMemberInit<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let union_ty = cg.union_type(union_id);
    //@this alloca needs to be in entry block 30.05.24
    let union_ptr = cg.builder.build_alloca(union_ty, "union_temp").unwrap();

    if let Some(value) = codegen_expr(
        cg,
        proc_cg,
        false,
        input.expr,
        BlockKind::TailStore(union_ptr),
    ) {
        cg.builder.build_store(union_ptr, value).unwrap();
    }

    if expect_ptr {
        union_ptr.into()
    } else {
        cg.builder
            .build_load(union_ty, union_ptr, "union_val")
            .unwrap()
    }
}

fn codegen_struct_init<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    struct_id: hir::StructID,
    input: &'ctx [hir::StructFieldInit<'ctx>],
) -> values::BasicValueEnum<'ctx> {
    let struct_ty = cg.struct_type(struct_id);
    //@this alloca needs to be in entry block 30.05.24
    let struct_ptr = cg.builder.build_alloca(struct_ty, "struct_temp").unwrap();

    for field_init in input {
        let field_ptr = cg
            .builder
            .build_struct_gep(
                struct_ty,
                struct_ptr,
                field_init.field_id.index() as u32,
                "field_ptr",
            )
            .unwrap();
        if let Some(value) = codegen_expr(
            cg,
            proc_cg,
            false,
            field_init.expr,
            BlockKind::TailStore(field_ptr),
        ) {
            cg.builder.build_store(field_ptr, value).unwrap();
        }
    }

    if expect_ptr {
        struct_ptr.into()
    } else {
        cg.builder
            .build_load(struct_ty, struct_ptr, "struct_val")
            .unwrap()
    }
}

#[allow(unsafe_code)]
fn codegen_array_init<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    array_init: &'ctx hir::ArrayInit<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let elem_ty = cg.type_into_basic(array_init.elem_ty);
    let array_ty = elem_ty.array_type(array_init.input.len() as u32);
    //@this alloca needs to be in entry block 30.05.24
    let array_ptr = cg.builder.build_alloca(array_ty, "array_temp").unwrap();

    for (idx, &expr) in array_init.input.iter().enumerate() {
        let index = cg.ptr_sized_int_type.const_int(idx as u64, false);
        let elem_ptr = unsafe {
            cg.builder
                .build_in_bounds_gep(
                    array_ty,
                    array_ptr,
                    &[cg.ptr_sized_int_type.const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        };
        if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailStore(elem_ptr))
        {
            cg.builder.build_store(elem_ptr, value).unwrap();
        }
    }

    if expect_ptr {
        array_ptr.into()
    } else {
        cg.builder
            .build_load(array_ty, array_ptr, "array_val")
            .unwrap()
    }
}

fn codegen_array_repeat<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    array_repeat: &'ctx hir::ArrayRepeat<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `array repeat` not supported")
}

fn codegen_address<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    rhs: &'ctx hir::Expr<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    //@semantics arent stable @14.04.24
    let rhs = codegen_expr(cg, proc_cg, true, rhs, BlockKind::TailDissalow).expect("value");
    if rhs.is_pointer_value() {
        return rhs;
    }
    //@addr can sometimes be adress of a value, or of temporary @08.04.24
    // constant values wont behave correctly: &5, 5 needs to be stack allocated
    // this addr of temporaries need to be supported with explicit stack allocation
    // (this might just work, since pointer values are still on the stack) eg: `&value.x.y`
    //@multiple adresses get shrinked into 1, which isnt how type system threats this eg: `& &[1, 2, 3]`
    //@temporary allocation and referencing should not be supported @14.04.24
    // but things like array literals and struct or union literals should work
    // since those result in allocation already being made
    // and can be passed by reference seamlessly
    let ty = rhs.get_type();
    let ptr = cg.builder.build_alloca(ty, "temp_addr_val").unwrap();
    cg.builder.build_store(ptr, rhs).unwrap();
    ptr.into()
}

fn codegen_unary<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    op: ast::UnOp,
    rhs: &'ctx hir::Expr<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    let rhs = codegen_expr(cg, proc_cg, false, rhs, BlockKind::TailAlloca).expect("value");

    match op {
        ast::UnOp::Neg => match rhs {
            values::BasicValueEnum::IntValue(value) => {
                cg.builder.build_int_neg(value, "un_temp").unwrap().into()
            }
            values::BasicValueEnum::FloatValue(value) => {
                cg.builder.build_float_neg(value, "un_temp").unwrap().into()
            }
            _ => panic!("codegen: unary `-` can only be applied to int, float"),
        },
        ast::UnOp::BitNot | ast::UnOp::LogicNot => cg
            .builder
            .build_not(rhs.into_int_value(), "un_temp")
            .unwrap()
            .into(),
        ast::UnOp::Deref => {
            panic!("codegen: unary deref is not supported, pointee_ty is not available");
            /*
            cg
                .builder
                .build_load(pointee_ty, rhs.into_pointer_value(), "un_temp")
                .unwrap(),
            */
        }
    }
}

fn codegen_binary<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    op: ast::BinOp,
    lhs: &'ctx hir::Expr<'ctx>,
    rhs: &'ctx hir::Expr<'ctx>,
    lhs_signed_int: bool,
) -> values::BasicValueEnum<'ctx> {
    let lhs = codegen_expr(cg, proc_cg, false, lhs, BlockKind::TailAlloca).expect("value");
    let rhs = codegen_expr(cg, proc_cg, false, rhs, BlockKind::TailAlloca).expect("value");
    codegen_bin_op(cg, op, lhs, rhs, lhs_signed_int)
}

fn codegen_bin_op<'ctx>(
    cg: &Codegen<'ctx>,
    op: ast::BinOp,
    lhs: values::BasicValueEnum<'ctx>,
    rhs: values::BasicValueEnum<'ctx>,
    lhs_signed_int: bool,
) -> values::BasicValueEnum<'ctx> {
    match op {
        ast::BinOp::Add => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_add(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_add(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `+` can only be applied to int, float"),
        },
        ast::BinOp::Sub => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_sub(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_sub(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `-` can only be applied to int, float"),
        },
        ast::BinOp::Mul => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_mul(lhs, rhs.into_int_value(), "bin_temp")
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_mul(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `*` can only be applied to int, float"),
        },
        ast::BinOp::Div => match lhs {
            values::BasicValueEnum::IntValue(lhs) => {
                if lhs_signed_int {
                    cg.builder
                        .build_int_signed_div(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                } else {
                    cg.builder
                        .build_int_unsigned_div(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                }
            }
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_div(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `/` can only be applied to int, float"),
        },
        ast::BinOp::Rem => match lhs {
            values::BasicValueEnum::IntValue(lhs) => {
                if lhs_signed_int {
                    cg.builder
                        .build_int_signed_rem(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                } else {
                    cg.builder
                        .build_int_unsigned_rem(lhs, rhs.into_int_value(), "bin_temp")
                        .unwrap()
                        .into()
                }
            }
            _ => panic!("codegen: binary `%` can only be applied to int"),
        },
        ast::BinOp::BitAnd => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitOr => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitXor => cg
            .builder
            .build_xor(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitShl => cg
            .builder
            .build_left_shift(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::BitShr => cg
            .builder
            .build_right_shift(
                lhs.into_int_value(),
                rhs.into_int_value(),
                lhs_signed_int,
                "bin_temp",
            )
            .unwrap()
            .into(),
        ast::BinOp::IsEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OEQ,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::PointerValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::EQ,
                    lhs,
                    rhs.into_pointer_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `==` can only be applied to int, float"),
        },
        ast::BinOp::NotEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::ONE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::PointerValue(lhs) => cg
                .builder
                .build_int_compare(
                    inkwell::IntPredicate::NE,
                    lhs,
                    rhs.into_pointer_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `!=` can only be applied to int, float, rawptr"),
        },
        ast::BinOp::Less => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SLT
                    } else {
                        inkwell::IntPredicate::ULT
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OLT,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `<` can only be applied to int, float"),
        },
        ast::BinOp::LessEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SLE
                    } else {
                        inkwell::IntPredicate::ULE
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OLE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `<=` can only be applied to int, float"),
        },
        ast::BinOp::Greater => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SGT
                    } else {
                        inkwell::IntPredicate::UGT
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OGT,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `>` can only be applied to int, float"),
        },
        ast::BinOp::GreaterEq => match lhs {
            values::BasicValueEnum::IntValue(lhs) => cg
                .builder
                .build_int_compare(
                    if lhs_signed_int {
                        inkwell::IntPredicate::SGE
                    } else {
                        inkwell::IntPredicate::UGE
                    },
                    lhs,
                    rhs.into_int_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_compare(
                    inkwell::FloatPredicate::OGE,
                    lhs,
                    rhs.into_float_value(),
                    "bin_temp",
                )
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `>=` can only be applied to int, float"),
        },
        ast::BinOp::LogicAnd => cg
            .builder
            .build_and(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::LogicOr => cg
            .builder
            .build_or(lhs.into_int_value(), rhs.into_int_value(), "bin_temp")
            .unwrap()
            .into(),
        ast::BinOp::Range | ast::BinOp::RangeInc => {
            panic!("codegen: range binary operators are not implemented");
        }
    }
}

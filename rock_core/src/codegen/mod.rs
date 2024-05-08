use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use crate::intern::InternID;
use crate::session::BuildKind;
use inkwell::basic_block::BasicBlock;
use inkwell::builder;
use inkwell::context;
use inkwell::context::AsContextRef;
use inkwell::module;
use inkwell::targets;
use inkwell::types;
use inkwell::types::BasicType;
use inkwell::values;
use std::collections::HashMap;

pub enum BuildConfig {
    Build(BuildKind),
    Run(BuildKind, Vec<String>),
}

impl BuildConfig {
    fn kind(&self) -> BuildKind {
        match *self {
            BuildConfig::Build(kind) => kind,
            BuildConfig::Run(kind, _) => kind,
        }
    }
}

struct Codegen<'ctx> {
    context: &'ctx context::Context,
    module: module::Module<'ctx>,
    builder: builder::Builder<'ctx>,
    target_machine: targets::TargetMachine,
    struct_types: Vec<types::StructType<'ctx>>,
    globals: Vec<values::GlobalValue<'ctx>>,
    function_values: Vec<values::FunctionValue<'ctx>>,
    hir: hir::Hir<'ctx>,
    c_functions: HashMap<InternID, values::FunctionValue<'ctx>>,
}

struct ProcCodegen<'ctx> {
    proc_id: hir::ProcID,
    function: values::FunctionValue<'ctx>,
    param_vars: Vec<values::PointerValue<'ctx>>,
    local_vars: Vec<values::PointerValue<'ctx>>,
    block_info: Vec<BlockInfo<'ctx>>,
    defer_blocks: Vec<&'ctx hir::Expr<'ctx>>,
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

    fn push_defer_block(&mut self, block: &'ctx hir::Expr<'ctx>) {
        self.block_info.last_mut().unwrap().defer_count += 1;
        self.defer_blocks.push(block);
    }

    fn last_loop_info(&self) -> LoopInfo<'ctx> {
        for info in self.block_info.iter().rev() {
            if let Some(loop_info) = info.loop_info {
                return loop_info;
            }
        }
        unreachable!("last loop must exist")
    }

    fn last_defer_blocks(&self) -> Vec<&'ctx hir::Expr<'ctx>> {
        let total_count = self.defer_blocks.len();
        let last_count = self.block_info.last().unwrap().defer_count;
        let range = total_count - last_count as usize..total_count;
        self.defer_blocks[range].to_vec()
    }

    //@lifetime problems, need to clone like this (since codegen_expr can mutate this vec) 05.05.24
    fn all_defer_blocks(&self) -> Vec<&'ctx hir::Expr<'ctx>> {
        self.defer_blocks.clone()
    }
}

impl<'ctx> Codegen<'ctx> {
    fn new(context: &'ctx context::Context, hir: hir::Hir<'ctx>) -> Codegen<'ctx> {
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

        Codegen {
            context,
            module,
            builder,
            target_machine,
            struct_types: Vec::new(),
            globals: Vec::new(),
            function_values: Vec::new(),
            hir,
            c_functions: HashMap::new(),
        }
    }

    fn build_object(self, bin_name: &str, config: BuildConfig) -> Result<(), ErrorComp> {
        self.module.print_to_stderr();
        if let Err(error) = self.module.verify() {
            return Err(ErrorComp::message(format!(
                "internal codegen error: llvm module verify failed\nreason: {}",
                error
            )));
        }

        let mut build_dir = std::env::current_dir().expect("cwd");
        build_dir.push("build");
        let _ = std::fs::create_dir(&build_dir);

        let kind = config.kind();
        match kind {
            BuildKind::Debug => build_dir.push("debug"),
            BuildKind::Release => build_dir.push("release"),
        }
        let _ = std::fs::create_dir(&build_dir);

        let mut object_path = build_dir.clone();
        object_path.push(format!("{}.o", bin_name));
        self.target_machine
            .write_to_file(&self.module, targets::FileType::Object, &object_path)
            .map_err(|error| {
                ErrorComp::message(format!(
                    "failed to write llvm module as object file\nreason: {}",
                    error
                ))
            })?;

        let mut executable_path = build_dir.clone();
        executable_path.push(bin_name);
        #[cfg(windows)]
        executable_path.set_extension("exe");

        //@libcmt.lib is only valid for windows @20.04.24
        // also lld-link is called system wide, and requires llvm being installed
        // test and use bundled lld-link instead
        let _ = std::process::Command::new("lld-link")
            .arg(object_path.as_os_str())
            .arg(format!("/OUT:{}", executable_path.to_string_lossy()))
            .arg("/DEFAULTLIB:libcmt.lib")
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!(
                    "failed to link object file `{}`\nreason: {}",
                    object_path.to_string_lossy(),
                    io_error
                ))
            })?;
        let _ = std::fs::remove_file(object_path);

        if let BuildConfig::Run(_, args) = config {
            std::process::Command::new(executable_path.as_os_str())
                .args(args)
                .status()
                .map_err(|io_error| {
                    ErrorComp::message(format!(
                        "failed to run executable `{}`\nreason: {}",
                        executable_path.to_string_lossy(),
                        io_error
                    ))
                })?;
        }
        Ok(())
    }

    //@perf, could be cached once to avoid llvm calls and extra checks
    // profile difference on actual release setup and bigger codebases
    fn pointer_sized_int_type(&self) -> types::IntType<'ctx> {
        self.context
            .ptr_sized_int_type(&self.target_machine.get_target_data(), None)
    }

    //@this is a hack untill inkwell pr is merged https://github.com/TheDan64/inkwell/pull/468 @07.04.24
    #[allow(unsafe_code)]
    fn ptr_type(&self) -> types::PointerType<'ctx> {
        unsafe {
            types::PointerType::new(llvm_sys::core::LLVMPointerTypeInContext(
                self.context.as_ctx_ref(),
                0,
            ))
        }
    }

    fn union_type(&self, union_id: hir::UnionID) -> types::ArrayType<'ctx> {
        let data = self.hir.union_data(union_id);
        let size = data.size_eval.get_size().expect("resolved");
        self.type_into_basic(hir::Type::Basic(ast::BasicType::U8))
            .expect("u8")
            .array_type(size.size() as u32)
    }

    fn struct_type(&self, struct_id: hir::StructID) -> types::StructType<'ctx> {
        self.struct_types[struct_id.index()]
    }

    //@duplicated with generation of procedure values and with indirect calls 07.05.24
    fn function_type(&self, proc_ty: &hir::ProcType) -> types::FunctionType<'ctx> {
        let mut param_types =
            Vec::<types::BasicMetadataTypeEnum>::with_capacity(proc_ty.params.len());

        for param in proc_ty.params {
            //@correct disallowing of void and never type would allow to rely on this being a valid value type,
            // clean up llvm type apis after this guarantee is satisfied
            // hir typechecker doesnt do that yet @16.04.24
            if let Some(ty) = self.type_into_basic_metadata(*param) {
                param_types.push(ty);
            }
        }

        match self.type_into_basic(proc_ty.return_ty) {
            Some(ty) => ty.fn_type(&param_types, proc_ty.is_variadic),
            None => self
                .context
                .void_type()
                .fn_type(&param_types, proc_ty.is_variadic),
        }
    }

    fn slice_type(&self) -> types::StructType<'ctx> {
        //@this slice type could be generated once and referenced every time
        self.context.struct_type(
            &[self.ptr_type().into(), self.pointer_sized_int_type().into()],
            false,
        )
    }

    fn array_type(&self, array: &hir::ArrayStatic) -> types::ArrayType<'ctx> {
        // @should use LLVMArrayType2 which takes u64, what not exposed 03.05.24
        //  by inkwell even for llvm 17 (LLVMArrayType was deprecated in this version)
        let elem_ty = self.type_into_basic(array.elem_ty).expect("non void type");
        if let hir::Expr::LitInt { val, .. } = *array.size.0 {
            elem_ty.array_type(val as u32)
        } else {
            panic!("codegen: invalid array static size expression");
        }
    }

    //@any is used as general type
    // unit / void is not accepted by enums that wrap
    // struct field types, proc param types.
    fn type_into_any(&self, ty: hir::Type) -> types::AnyTypeEnum<'ctx> {
        match ty {
            hir::Type::Error => panic!("codegen unexpected hir::Type::Error"),
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::S8 => self.context.i8_type().into(),
                ast::BasicType::S16 => self.context.i16_type().into(),
                ast::BasicType::S32 => self.context.i32_type().into(),
                ast::BasicType::S64 => self.context.i64_type().into(),
                ast::BasicType::Ssize => self.pointer_sized_int_type().into(),
                ast::BasicType::U8 => self.context.i8_type().into(),
                ast::BasicType::U16 => self.context.i16_type().into(),
                ast::BasicType::U32 => self.context.i32_type().into(),
                ast::BasicType::U64 => self.context.i64_type().into(),
                ast::BasicType::Usize => self.pointer_sized_int_type().into(),
                ast::BasicType::F16 => self.context.f16_type().into(),
                ast::BasicType::F32 => self.context.f32_type().into(),
                ast::BasicType::F64 => self.context.f64_type().into(),
                ast::BasicType::Bool => self.context.bool_type().into(),
                ast::BasicType::Char => self.context.i32_type().into(),
                ast::BasicType::Rawptr => self.ptr_type().into(),
                ast::BasicType::Void => self.context.void_type().into(),
                ast::BasicType::Never => panic!("codegen unexpected BasicType::Never"),
            },
            hir::Type::Enum(id) => {
                let basic = self.hir.enum_data(id).basic;
                self.type_into_any(hir::Type::Basic(basic))
            }
            hir::Type::Union(id) => self.union_type(id).into(),
            hir::Type::Struct(struct_id) => self.struct_type(struct_id).into(),
            hir::Type::Reference(_, _) => self.ptr_type().into(),
            hir::Type::Procedure(_) => self.ptr_type().into(),
            hir::Type::ArraySlice(_) => self.slice_type().into(),
            hir::Type::ArrayStatic(array) => self.array_type(array).into(),
        }
    }

    fn type_into_basic(&self, ty: hir::Type) -> Option<types::BasicTypeEnum<'ctx>> {
        self.type_into_any(ty).try_into().ok()
    }

    fn type_into_basic_metadata(
        &self,
        ty: hir::Type,
    ) -> Option<types::BasicMetadataTypeEnum<'ctx>> {
        self.type_into_any(ty).try_into().ok()
    }
}

pub fn codegen(hir: hir::Hir, bin_name: &str, config: BuildConfig) -> Result<(), ErrorComp> {
    let context = context::Context::create();
    let mut cg = Codegen::new(&context, hir);
    codegen_struct_types(&mut cg);
    codegen_globals(&mut cg);
    codegen_function_values(&mut cg);
    codegen_function_bodies(&cg);
    cg.build_object(bin_name, config)
}

//@breaking issue inkwell api takes in BasicTypeEnum for struct body creation
// which doesnt allow void type which is represented as unit () in rock language
// this has a side-effect of shifting field ids in relation to generated StructFieldIDs in hir::Expr
// llvm doesnt seem to explicitly disallow void_type in structures. @05.04.24
//@same applies to unit / void types in procedure params, ProcParamIDs would be synced to llvm param ids
// when unit type params are removed @05.04.24
//@this is general design problem with unit / void type in the language
// currently its allowed to be used freely
fn codegen_struct_types(cg: &mut Codegen) {
    cg.struct_types.reserve_exact(cg.hir.structs.len());

    for _ in 0..cg.hir.structs.len() {
        let opaque = cg.context.opaque_struct_type("rock_struct");
        cg.struct_types.push(opaque);
    }

    let mut field_types = Vec::<types::BasicTypeEnum>::new();
    for (idx, struct_data) in cg.hir.structs.iter().enumerate() {
        field_types.clear();

        for field in struct_data.fields {
            //@read proc param type_into_basic() info message @16.04.24
            // (this field type isnt optional, else FieldIDs are invalidated)
            if let Some(ty) = cg.type_into_basic(field.ty) {
                field_types.push(ty);
            }
        }

        let opaque = cg.struct_types[idx];
        opaque.set_body(&field_types, false);
        eprintln!("{}", opaque.print_to_string());
    }
}

fn codegen_globals(cg: &mut Codegen) {
    cg.globals.reserve_exact(cg.hir.globals.len());

    for data in cg.hir.globals.iter() {
        let global_ty = cg.type_into_basic(data.ty).expect("non void type");
        let global = cg.module.add_global(global_ty, None, "global");
        global.set_constant(data.mutt == ast::Mut::Immutable);
        global.set_linkage(module::Linkage::Private);
        global.set_thread_local(data.thread_local);
        global.set_initializer(&codegen_const_value(
            cg,
            match data.value {
                hir::ConstValueEval::Resolved { value } => value,
                _ => panic!("codegen on unresolved const value"),
            },
        ));
        cg.globals.push(global);
    }
}

fn codegen_function_values(cg: &mut Codegen) {
    cg.function_values.reserve_exact(cg.hir.structs.len());

    let mut param_types = Vec::<types::BasicMetadataTypeEnum>::new();
    for proc_data in cg.hir.procs.iter() {
        param_types.clear();

        for param in proc_data.params {
            //@correct disallowing of void and never type would allow to rely on this being a valid value type,
            // clean up llvm type apis after this guarantee is satisfied
            // hir typechecker doesnt do that yet @16.04.24
            if let Some(ty) = cg.type_into_basic_metadata(param.ty) {
                param_types.push(ty);
            }
        }

        let function_ty = match cg.type_into_basic(proc_data.return_ty) {
            Some(ty) => ty.fn_type(&param_types, proc_data.is_variadic),
            None => cg
                .context
                .void_type()
                .fn_type(&param_types, proc_data.is_variadic),
        };

        //@perf when using incremented names llvm can increment it on its own (lots of string allocations here)
        // main name_id could be cached to avoid string compares and get from pool
        // (switch to explicit main flag on proc_data or in hir instead)
        //@temporary condition to determine if its entry point or not @06.04.24

        let name = cg.hir.intern.get_str(proc_data.name.id);
        let name = if proc_data.block.is_none()
            || (proc_data.origin_id == hir::ModuleID::new(0) && name == "main")
        {
            name
        } else {
            "rock_proc"
        };

        //@specify correct linkage kind (most things should be internal) @16.04.24
        // and c_calls must be correctly linked (currently just leaving default behavior)
        let function = cg.module.add_function(name, function_ty, None);
        if proc_data.block.is_none() {
            cg.c_functions.insert(proc_data.name.id, function);
        }
        cg.function_values.push(function);
    }
}

fn codegen_function_bodies(cg: &Codegen) {
    for (idx, proc_data) in cg.hir.procs.iter().enumerate() {
        if let Some(block) = proc_data.block {
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
                let local_ty = cg.type_into_basic(local.ty).expect("value type");
                let local_ptr = cg.builder.build_alloca(local_ty, "local").unwrap();
                // locals are initialized when declared
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

            if let Some(value) = codegen_expr(cg, &mut proc_cg, false, block) {
                //@hack building return of the tail returned value on the last block
                // last might not be a correct place for it
                let entry = function.get_last_basic_block().expect("last block");
                cg.builder.position_at_end(entry);
                cg.builder.build_return(Some(&value)).unwrap();
            } else {
                //@hack generating implicit return
                //also generate it on last block on regular void return functions?
                // cannot detect if `return;` was already written there
                //@overall all returns must be included in Hir explicitly,
                //so codegen doesnt need to do any work to get correct outputs
                let entry = function.get_last_basic_block().expect("last block");
                if entry.get_terminator().is_none() {
                    cg.builder.position_at_end(entry);
                    cg.builder.build_return(None).unwrap();
                }
            }
        }
    }
}

//@other potentially contant expressions arent supported yet @13.04.24
// to support arrays structs unions etc
// there should be dedicated constant values produced at analysis stage
// hir also currently only supports literal constants for the same reason
fn codegen_const_expr<'ctx>(
    cg: &Codegen<'ctx>,
    expr: hir::ConstExpr,
) -> values::BasicValueEnum<'ctx> {
    match *expr.0 {
        hir::Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        hir::Expr::LitNull => codegen_lit_null(cg),
        hir::Expr::LitBool { val } => codegen_lit_bool(cg, val),
        hir::Expr::LitInt { val, ty } => codegen_lit_int(cg, val, ty),
        hir::Expr::LitFloat { val, ty } => codegen_lit_float(cg, val, ty),
        hir::Expr::LitChar { val } => codegen_lit_char(cg, val),
        hir::Expr::LitString { id, c_string } => codegen_lit_string(cg, id, c_string),
        _ => panic!("codegen unexpected constant expression kind"),
    }
}

fn codegen_const_value<'ctx>(
    cg: &Codegen<'ctx>,
    value: hir::ConstValue,
) -> values::BasicValueEnum<'ctx> {
    match value {
        hir::ConstValue::Error => panic!("codegen unexpected ConstValue::Error"),
        hir::ConstValue::Null => cg.ptr_type().const_zero().into(),
        hir::ConstValue::Bool { val } => cg.context.bool_type().const_int(val as u64, false).into(),
        hir::ConstValue::Int { val, neg } => todo!(),
        hir::ConstValue::Float { val } => todo!(),
        hir::ConstValue::Char { val } => todo!(),
        hir::ConstValue::String { id, c_string } => todo!(),
        hir::ConstValue::Struct { struct_ } => todo!(),
        hir::ConstValue::Array { array } => todo!(),
        hir::ConstValue::ArrayRepeat { value, len } => todo!(),
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
) -> Option<values::BasicValueEnum<'ctx>> {
    use hir::Expr;
    match *expr {
        Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        Expr::LitNull => Some(codegen_lit_null(cg)),
        Expr::LitBool { val } => Some(codegen_lit_bool(cg, val)),
        Expr::LitInt { val, ty } => Some(codegen_lit_int(cg, val, ty)),
        Expr::LitFloat { val, ty } => Some(codegen_lit_float(cg, val, ty)),
        Expr::LitChar { val } => Some(codegen_lit_char(cg, val)),
        Expr::LitString { id, c_string } => Some(codegen_lit_string(cg, id, c_string)),
        Expr::If { if_ } => codegen_if(cg, proc_cg, if_),
        Expr::Block { stmts } => codegen_block(cg, proc_cg, expect_ptr, stmts),
        Expr::Match { match_ } => Some(codegen_match(cg, proc_cg, match_)),
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
        Expr::Procedure { proc_id } => Some(codegen_procedure(cg, proc_id)),
        Expr::CallDirect { proc_id, input } => codegen_call_direct(cg, proc_cg, proc_id, input),
        Expr::CallIndirect { target, indirect } => {
            codegen_call_indirect(cg, proc_cg, target, indirect)
        }
        Expr::EnumVariant {
            enum_id,
            variant_id,
        } => Some(codegen_enum_variant(cg, enum_id, variant_id)),
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

fn codegen_lit_null<'ctx>(cg: &Codegen<'ctx>) -> values::BasicValueEnum<'ctx> {
    cg.ptr_type().const_zero().into()
}

fn codegen_lit_bool<'ctx>(cg: &Codegen<'ctx>, val: bool) -> values::BasicValueEnum<'ctx> {
    cg.context.bool_type().const_int(val as u64, false).into()
}

//@since its always u64 value thats not sign extended in 2s compliment form
// pass sign_extend = false
// constfolding isnt done yet by the compiler, most overflow errors wont be caught early
// unary minus should work on this const int conrrectly when llvm const folds it. @06.04.24
fn codegen_lit_int<'ctx>(
    cg: &Codegen<'ctx>,
    val: u64,
    ty: ast::BasicType,
) -> values::BasicValueEnum<'ctx> {
    //@unsigned values bigger that signed max of that type get flipped (skill issue, 2s compliment)
    let int_type = cg.type_into_any(hir::Type::Basic(ty)).into_int_type();
    int_type.const_int(val, false).into()
}

fn codegen_lit_float<'ctx>(
    cg: &Codegen<'ctx>,
    val: f64,
    ty: ast::BasicType,
) -> values::BasicValueEnum<'ctx> {
    let float_type = cg.type_into_any(hir::Type::Basic(ty)).into_float_type();
    float_type.const_float(val).into()
}

fn codegen_lit_char<'ctx>(cg: &Codegen<'ctx>, val: char) -> values::BasicValueEnum<'ctx> {
    let char_type = cg
        .type_into_any(hir::Type::Basic(ast::BasicType::Char))
        .into_int_type();
    char_type.const_int(val as u64, false).into()
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
    let string = cg.hir.intern.get_str(id);
    let array_value = cg.context.const_string(string.as_bytes(), c_string);
    let array_ty = array_value.get_type();

    let global = cg.module.add_global(array_ty, None, "global_string");
    global.set_constant(true);
    global.set_linkage(module::Linkage::Private);
    global.set_initializer(&array_value);
    let global_ptr = global.as_pointer_value();

    if c_string {
        global_ptr.into()
    } else {
        //@sign extend would likely ruin any len values about MAX signed of that type
        // same problem as const integer codegen @07.04.24
        // also usize from rust, casted int u64, represented by pointer_sized_int is confusing
        // most likely problems wont show up for strings of reasonable len (especially on 64bit)
        let bytes_len = cg
            .pointer_sized_int_type()
            .const_int(string.len() as u64, false);
        let slice_value = cg
            .context
            .const_struct(&[global_ptr.into(), bytes_len.into()], false);
        slice_value.into()
    }
}

fn codegen_if<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    if_: &'ctx hir::If<'ctx>,
) -> Option<values::BasicValueEnum<'ctx>> {
    let cond_block = cg.context.append_basic_block(proc_cg.function, "if_cond");
    let mut body_block = cg.context.append_basic_block(proc_cg.function, "if_body");
    let exit_block = cg.context.append_basic_block(proc_cg.function, "if_exit");

    // next if_cond or if_else block
    let mut next_block = if !if_.branches.is_empty() || if_.fallback.is_some() {
        cg.context.insert_basic_block_after(body_block, "if_next")
    } else {
        exit_block
    };

    cg.builder.build_unconditional_branch(cond_block).unwrap();
    cg.builder.position_at_end(cond_block);
    let cond = codegen_expr(cg, proc_cg, false, if_.entry.cond).expect("value");
    cg.builder
        .build_conditional_branch(cond.into_int_value(), body_block, next_block)
        .unwrap();

    cg.builder.position_at_end(body_block);
    codegen_expr(cg, proc_cg, false, if_.entry.block);
    cg.builder.build_unconditional_branch(exit_block).unwrap();

    for (idx, branch) in if_.branches.iter().enumerate() {
        let last = idx + 1 == if_.branches.len();
        let create_next = !last || if_.fallback.is_some();

        body_block = cg.context.insert_basic_block_after(next_block, "if_body");

        cg.builder.position_at_end(next_block);
        let cond = codegen_expr(cg, proc_cg, false, branch.cond).expect("value");
        next_block = if create_next {
            cg.context.insert_basic_block_after(body_block, "if_next")
        } else {
            exit_block
        };
        cg.builder
            .build_conditional_branch(cond.into_int_value(), body_block, next_block)
            .unwrap();

        cg.builder.position_at_end(body_block);
        codegen_expr(cg, proc_cg, false, branch.block);
        cg.builder.build_unconditional_branch(exit_block).unwrap();
    }

    if let Some(fallback) = if_.fallback {
        cg.builder.position_at_end(next_block);
        codegen_expr(cg, proc_cg, false, fallback);
        cg.builder.build_unconditional_branch(exit_block).unwrap();
    }

    cg.builder.position_at_end(exit_block);

    None
}

fn codegen_block<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    stmts: &'ctx [hir::Stmt<'ctx>],
) -> Option<values::BasicValueEnum<'ctx>> {
    proc_cg.enter_block();

    for (idx, stmt) in stmts.iter().enumerate() {
        match *stmt {
            hir::Stmt::Break => {
                let break_bb = proc_cg.last_loop_info().break_bb;
                //@which defer block to generate? 06.05.24
                cg.builder.build_unconditional_branch(break_bb).unwrap();
            }
            hir::Stmt::Continue => {
                let continue_bb = proc_cg.last_loop_info().continue_bb;
                //@which defer block to generate? 06.05.24
                cg.builder.build_unconditional_branch(continue_bb).unwrap();
            }
            hir::Stmt::Return(expr) => {
                codegen_defer_blocks(cg, proc_cg, proc_cg.all_defer_blocks().as_slice());

                if let Some(expr) = expr {
                    let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
                    cg.builder.build_return(Some(&value)).unwrap();
                } else {
                    cg.builder.build_return(None).unwrap();
                }

                // prevents defer generation on exit
                proc_cg.exit_block();
                return None;
            }
            hir::Stmt::Defer(block) => {
                proc_cg.push_defer_block(block);
            }
            hir::Stmt::ForLoop(for_) => {
                let entry_block = cg
                    .context
                    .append_basic_block(proc_cg.function, "loop_entry");
                let body_block = cg.context.append_basic_block(proc_cg.function, "loop_body");
                let exit_block = cg.context.append_basic_block(proc_cg.function, "loop_exit");

                cg.builder.build_unconditional_branch(entry_block).unwrap();
                proc_cg.set_next_loop_info(exit_block, entry_block);

                match for_.kind {
                    hir::ForKind::Loop => {
                        cg.builder.position_at_end(entry_block);
                        cg.builder.build_unconditional_branch(body_block).unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_expr(cg, proc_cg, false, for_.block);

                        //@might not be valid when break / continue are used 06.05.24
                        // other cfg will make body_block not the actual block we want
                        if body_block.get_terminator().is_none() {
                            cg.builder.position_at_end(body_block);
                            cg.builder.build_unconditional_branch(body_block).unwrap();
                        }
                    }
                    hir::ForKind::While { cond } => {
                        cg.builder.position_at_end(entry_block);
                        let cond = codegen_expr(cg, proc_cg, false, cond).expect("value");
                        cg.builder
                            .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                            .unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_expr(cg, proc_cg, false, for_.block);

                        //@might not be valid when break / continue are used 06.05.24
                        cg.builder.build_unconditional_branch(entry_block).unwrap();
                    }
                    hir::ForKind::ForLoop {
                        local_id,
                        cond,
                        assign,
                    } => {
                        cg.builder.position_at_end(entry_block);
                        codegen_local(cg, proc_cg, local_id);
                        let cond = codegen_expr(cg, proc_cg, false, cond).expect("value");
                        cg.builder
                            .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                            .unwrap();

                        cg.builder.position_at_end(body_block);
                        codegen_expr(cg, proc_cg, false, for_.block);

                        //@often invalid (this assignment might need special block) if no iterator abstractions are used
                        // in general loops need to be simplified in Hir, to loops and conditional breaks 06.05.24
                        cg.builder.position_at_end(body_block);
                        codegen_assign(cg, proc_cg, assign);

                        //@might not be valid when break / continue are used 06.05.24
                        cg.builder.build_unconditional_branch(entry_block).unwrap();
                    }
                }

                cg.builder.position_at_end(exit_block);
            }
            hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
            hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
            hir::Stmt::ExprSemi(expr) => {
                //@are expressions like `5;` valid when output as llvm ir? probably yes
                codegen_expr(cg, proc_cg, false, expr);
            }
            hir::Stmt::ExprTail(expr) => {
                //@assumed to be last code in the block
                // and is return as block value
                assert_eq!(
                    idx + 1,
                    stmts.len(),
                    "codegen Stmt::ExprTail must be the last statement of the block"
                );

                codegen_defer_blocks(cg, proc_cg, proc_cg.last_defer_blocks().as_slice());
                let value = codegen_expr(cg, proc_cg, expect_ptr, expr);
                proc_cg.exit_block();

                return value;
            }
        }
    }

    codegen_defer_blocks(cg, proc_cg, proc_cg.last_defer_blocks().as_slice());
    proc_cg.exit_block();
    None
}

//@contents should be generated once, instead of generating all block code each time
// and only branches to defer blocks should be created? hard to design currently @06.05.24
fn codegen_defer_blocks<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    defer_blocks: &[&'ctx hir::Expr<'ctx>],
) {
    if defer_blocks.is_empty() {
        return;
    }

    let mut defer_block = cg
        .context
        .append_basic_block(proc_cg.function, "defer_entry");

    for block in defer_blocks.iter().rev() {
        cg.builder.build_unconditional_branch(defer_block).unwrap();
        cg.builder.position_at_end(defer_block);
        codegen_expr(cg, proc_cg, false, block);
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

    let value = if let Some(expr) = local.value {
        codegen_expr(cg, proc_cg, false, expr).expect("value")
    } else {
        let var_ty = cg.type_into_basic(local.ty).expect("non void type");
        var_ty.const_zero()
    };
    cg.builder.build_store(var_ptr, value).unwrap();
}

fn codegen_assign<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    assign: &hir::Assign<'ctx>,
) {
    let lhs = codegen_expr(cg, proc_cg, true, assign.lhs).expect("value");
    let rhs = codegen_expr(cg, proc_cg, false, assign.rhs).expect("value");
    let lhs_ptr = lhs.into_pointer_value();

    match assign.op {
        ast::AssignOp::Assign => {
            cg.builder.build_store(lhs_ptr, rhs).unwrap();
        }
        ast::AssignOp::Bin(op) => {
            let lhs_ty = cg.type_into_basic(assign.lhs_ty).expect("value type");
            let lhs_value = cg.builder.build_load(lhs_ty, lhs_ptr, "load_val").unwrap();
            let bin_value = codegen_bin_op(cg, op, lhs_value, rhs, assign.lhs_signed_int);
            cg.builder.build_store(lhs_ptr, bin_value).unwrap();
        }
    }
}

fn codegen_match<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    match_: &hir::Match<'ctx>,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `match` not supported")
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
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type(), target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    if expect_ptr {
        target_ptr.into()
    } else {
        let member = cg.hir.union_data(union_id).member(member_id);
        let member_ty = cg.type_into_basic(member.ty).expect("value");
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
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = if deref {
        cg.builder
            .build_load(cg.ptr_type(), target.into_pointer_value(), "deref_ptr")
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
        let field_ty = cg.type_into_basic(field.ty).expect("value");
        cg.builder
            .build_load(field_ty, field_ptr, "field_val")
            .unwrap()
    }
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
        .get(&cg.hir.intern.get_id("exit").expect("exit c function"))
        .cloned()
        .expect("exit c function added");
    let c_printf = cg
        .c_functions
        .get(&cg.hir.intern.get_id("printf").expect("printf c function"))
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
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type(), target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    let index = codegen_expr(cg, proc_cg, false, access.index)
        .expect("value")
        .into_int_value();

    let elem_ptr = match access.kind {
        hir::IndexKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type(), target_ptr, "slice_val")
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
            let len = match *array.size.0 {
                hir::Expr::LitInt { val, .. } => val,
                _ => unreachable!("array size must be u64"),
            };
            let len = cg.pointer_sized_int_type().const_int(len, false);

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
                    &[cg.pointer_sized_int_type().const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        },
    };

    if expect_ptr {
        elem_ptr.into()
    } else {
        let elem_ty = cg.type_into_basic(access.elem_ty).expect("non void type");
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
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = if access.deref {
        cg.builder
            .build_load(cg.ptr_type(), target.into_pointer_value(), "deref_ptr")
            .unwrap()
            .into_pointer_value()
    } else {
        target.into_pointer_value()
    };

    match access.kind {
        hir::SliceKind::Slice { elem_size } => {
            let slice = cg
                .builder
                .build_load(cg.slice_type(), target_ptr, "slice_val")
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
                    codegen_expr(cg, proc_cg, false, lower)
                        .expect("value")
                        .into_int_value(),
                ),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper)
                        .expect("value")
                        .into_int_value(),
                ),
                hir::SliceRangeEnd::Inclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper)
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
                            //    cg.pointer_sized_int_type().const_int(1, false).into(),
                            //    false,
                            //)
                            //.into_int_value()
                            upper
                        } else {
                            codegen_bin_op(
                                cg,
                                ast::BinOp::Add,
                                upper.into(),
                                cg.pointer_sized_int_type().const_int(1, false).into(),
                                false,
                            )
                            .into_int_value()
                        };

                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type();

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
            let len = match *array.size.0 {
                hir::Expr::LitInt { val, .. } => val,
                _ => unreachable!("array size must be u64"),
            };
            let len = cg.pointer_sized_int_type().const_int(len, false);

            let lower = match access.range.lower {
                Some(lower) => Some(
                    codegen_expr(cg, proc_cg, false, lower)
                        .expect("value")
                        .into_int_value(),
                ),
                None => None,
            };

            let upper = match access.range.upper {
                hir::SliceRangeEnd::Unbounded => None,
                hir::SliceRangeEnd::Exclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper)
                        .expect("value")
                        .into_int_value(),
                ),
                hir::SliceRangeEnd::Inclusive(upper) => Some(
                    codegen_expr(cg, proc_cg, false, upper)
                        .expect("value")
                        .into_int_value(),
                ),
            };

            match (lower, upper) {
                (None, None) => {
                    //@potentially unwanted alloca in loops 08.05.24
                    let slice_type = cg.slice_type();

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
    let target = codegen_expr(cg, proc_cg, false, target).expect("value");
    let into = cg.type_into_basic(*into).expect("non void type");
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
        let local_ty = cg.type_into_basic(local.ty).expect("value type");
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
        let param_ty = cg.type_into_basic(param.ty).expect("value type");
        cg.builder
            .build_load(param_ty, param_ptr, "param_val")
            .unwrap()
    }
}

fn codegen_const_var<'ctx>(
    cg: &Codegen<'ctx>,
    const_id: hir::ConstID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `const var` not supported")
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

fn codegen_enum_variant<'ctx>(
    cg: &Codegen<'ctx>,
    enum_id: hir::EnumID,
    variant_id: hir::EnumVariantID,
) -> values::BasicValueEnum<'ctx> {
    //@generating value for that variant each time
    // this might be fine when constants are folded to single constant value
    // (current impl only supports single literals) @08.04.24
    let variant = cg.hir.enum_data(enum_id).variant(variant_id);
    codegen_const_value(
        cg,
        match variant.value.expect("enum variant value") {
            hir::ConstValueEval::Resolved { value } => value,
            _ => panic!("codegen on unresolved const value"),
        },
    )
}

fn codegen_procedure<'ctx>(
    cg: &Codegen<'ctx>,
    proc_id: hir::ProcID,
) -> values::BasicValueEnum<'ctx> {
    cg.function_values[proc_id.index()]
        .as_global_value()
        .as_pointer_value()
        .into()
}

fn codegen_call_direct<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    proc_id: hir::ProcID,
    input: &'ctx [&'ctx hir::Expr],
) -> Option<values::BasicValueEnum<'ctx>> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
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
    let function_ptr = codegen_expr(cg, proc_cg, false, target)
        .expect("value")
        .into_pointer_value();

    let mut input_values = Vec::with_capacity(indirect.input.len());
    for &expr in indirect.input {
        let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
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
    let union_ptr = cg.builder.build_alloca(union_ty, "union_temp").unwrap();

    let value = codegen_expr(cg, proc_cg, false, input.expr).expect("value");
    cg.builder.build_store(union_ptr, value).unwrap();

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
    let struct_ptr = cg.builder.build_alloca(struct_ty, "struct_temp").unwrap();

    for field_init in input {
        let value = codegen_expr(cg, proc_cg, false, field_init.expr).expect("value");
        let field_ptr = cg
            .builder
            .build_struct_gep(
                struct_ty,
                struct_ptr,
                field_init.field_id.index() as u32,
                "field_ptr",
            )
            .unwrap();
        cg.builder.build_store(field_ptr, value).unwrap();
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
    let elem_ty = cg
        .type_into_basic(array_init.elem_ty)
        .expect("non void type");
    let array_ty = elem_ty.array_type(array_init.input.len() as u32);
    let array_ptr = cg.builder.build_alloca(array_ty, "array_temp").unwrap();
    let index_type = cg.pointer_sized_int_type();

    for (idx, &expr) in array_init.input.iter().enumerate() {
        let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
        //@same possible sign extension problem as with all integers currently
        let index = index_type.const_int(idx as u64, false);
        let elem_ptr = unsafe {
            cg.builder
                .build_in_bounds_gep(
                    array_ty,
                    array_ptr,
                    &[cg.pointer_sized_int_type().const_zero(), index],
                    "elem_ptr",
                )
                .unwrap()
        };
        cg.builder.build_store(elem_ptr, value).unwrap();
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
    let rhs = codegen_expr(cg, proc_cg, true, rhs).expect("value");
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
    let rhs = codegen_expr(cg, proc_cg, false, rhs).expect("value");

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
    let lhs = codegen_expr(cg, proc_cg, false, lhs).expect("value");
    let rhs = codegen_expr(cg, proc_cg, false, rhs).expect("value");
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
            values::BasicValueEnum::FloatValue(lhs) => cg
                .builder
                .build_float_rem(lhs, rhs.into_float_value(), "bin_temp")
                .unwrap()
                .into(),
            _ => panic!("codegen: binary `%` can only be applied to int, float"),
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
            _ => panic!("codegen: binary `!=` can only be applied to int, float"),
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

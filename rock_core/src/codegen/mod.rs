use crate::ast;
use crate::hir;
use inkwell::builder;
use inkwell::context;
use inkwell::module;
use inkwell::targets;
use inkwell::types;
use inkwell::types::BasicType;
use std::path::Path;

struct Codegen<'ctx> {
    context: &'ctx context::Context,
    module: module::Module<'ctx>,
    builder: builder::Builder<'ctx>,
    target_machine: targets::TargetMachine,
    struct_types: Vec<types::StructType<'ctx>>,
}

impl<'ctx> Codegen<'ctx> {
    fn new(context: &'ctx context::Context) -> Codegen<'ctx> {
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
            struct_types: Vec::new(),
            target_machine,
        }
    }

    //@debug printing module,
    // handle build directory missing properly:
    // currently its created by LLVM when requesting the file output
    fn build_object(self) {
        self.module.print_to_stderr();
        let path = Path::new("build/main.o");
        self.target_machine
            .write_to_file(&self.module, targets::FileType::Object, &path)
            .expect("llvm write object")
    }

    //@perf, could be cached once to avoid llvm calls and extra checks
    // profile difference on actual release setup and bigger codebases
    fn pointer_sized_int_type(&self) -> types::IntType<'ctx> {
        self.context
            .ptr_sized_int_type(&self.target_machine.get_target_data(), None)
            .into()
    }

    //@any is used as general type
    // unit / void is not accepted by enums that wrap
    // struct field types, proc param types.
    fn type_into_any(&self, ty: hir::Type) -> types::AnyTypeEnum<'ctx> {
        match ty {
            hir::Type::Error => panic!("unexpected error type during codegen"),
            hir::Type::Basic(basic) => match basic {
                ast::BasicType::Unit => self.context.void_type().into(),
                ast::BasicType::Bool => self.context.bool_type().into(),
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
                ast::BasicType::F32 => self.context.f32_type().into(),
                ast::BasicType::F64 => self.context.f64_type().into(),
                ast::BasicType::Char => self.context.i32_type().into(),
                ast::BasicType::Rawptr => self.pointer_sized_int_type().into(),
            },
            hir::Type::Enum(_) => todo!(),
            hir::Type::Union(_) => todo!(),
            hir::Type::Struct(id) => self.struct_types[id.index()].into(),
            hir::Type::Reference(_, _) => self.pointer_sized_int_type().into(),
            hir::Type::ArraySlice(_) => todo!(),
            hir::Type::ArrayStatic(_) => todo!(),
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

pub fn codegen(hir: hir::Hir) {
    let context = context::Context::create();
    let mut cg = Codegen::new(&context);
    codegen_struct_types(&mut cg, &hir);
    codegen_procedures(&cg, &hir);
    cg.build_object();
}

//@breaking issue inkwell api takes in BasicTypeEnum for struct body creation
// which doesnt allow void type which is represented as unit () in rock language
// this has a side-effect of shifting field ids in relation to generated StructFieldIDs in hir::Expr
// llvm doesnt seem to explicitly disallow void_type in structures. @05.04.24
//@same applies to unit / void types in procedure params, ProcParamIDs would be synced to llvm param ids
// when unit type params are removed @05.04.24
//@this is general design problem with unit / void type in the language
// currently its allowed to be used freely
fn codegen_struct_types(cg: &mut Codegen, hir: &hir::Hir) {
    for idx in 0..hir.structs.len() {
        let name = format!("rock_struct_{idx}");
        let opaque = cg.context.opaque_struct_type(&name);
        cg.struct_types.push(opaque);
    }

    let mut field_types = Vec::<types::BasicTypeEnum>::new();
    for (idx, struct_data) in hir.structs.iter().enumerate() {
        field_types.clear();

        for field in struct_data.fields {
            //@unit types being ignored, they cannot be passed via inkwell api
            // which might be correct with llvm spec, void only used as procedure return type
            match cg.type_into_basic(field.ty) {
                Some(ty) => field_types.push(ty),
                None => {}
            }
        }

        let opaque = cg.struct_types[idx];
        opaque.set_body(&field_types, false);
        eprintln!("{}", opaque.print_to_string());
    }
}

fn codegen_procedures(cg: &Codegen, hir: &hir::Hir) {
    let mut param_types = Vec::<types::BasicMetadataTypeEnum>::new();
    for (idx, proc_data) in hir.procs.iter().enumerate() {
        param_types.clear();

        for param in proc_data.params {
            //@unit types being ignored, they cannot be passed via inkwell api
            // which might be correct with llvm spec, void only used as procedure return type
            match cg.type_into_basic_metadata(param.ty) {
                Some(ty) => param_types.push(ty),
                None => {}
            }
        }

        let function_ty = match cg.type_into_basic(proc_data.return_ty) {
            Some(ty) => ty.fn_type(&param_types, proc_data.is_variadic),
            None => cg
                .context
                .void_type()
                .fn_type(&param_types, proc_data.is_variadic),
        };

        //@specify linkage and name based on function kind #[c_call], `main` function, etc.
        let name = format!("rock_struct_{idx}");
        let function = cg.module.add_function(&name, function_ty, None);

        //@placeholder for expr codegen
        let basic_block = cg.context.append_basic_block(function, "entry");
        cg.builder.position_at_end(basic_block);
        cg.builder.build_return(None).unwrap();
    }
}

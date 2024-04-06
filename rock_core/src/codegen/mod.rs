use crate::ast;
use crate::hir;
use crate::intern::InternID;
use inkwell::builder;
use inkwell::context;
use inkwell::module;
use inkwell::targets;
use inkwell::types;
use inkwell::types::BasicType;
use inkwell::values;
use std::path::Path;

struct Codegen<'ctx> {
    context: &'ctx context::Context,
    module: module::Module<'ctx>,
    builder: builder::Builder<'ctx>,
    target_machine: targets::TargetMachine,
    struct_types: Vec<types::StructType<'ctx>>,
    current_funtion: Option<values::FunctionValue<'ctx>>,
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
            target_machine,
            struct_types: Vec::new(),
            current_funtion: None,
        }
    }

    //@debug printing module,
    // handle build directory missing properly:
    // currently its created by LLVM when requesting the file output
    fn build_object(self) {
        self.module.print_to_stderr();
        if let Err(error) = self.module.verify() {
            eprintln!("codegen module verify failed: \n{}", error);
            return;
        }

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
            hir::Type::Error => panic!("codegen unexpected hir::Type::Error"),
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
    codegen_procedures(&mut cg, &hir);
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

fn codegen_procedures(cg: &mut Codegen, hir: &hir::Hir) {
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

        //@perf when using incremented names llvm can increment it on its own (lots of string allocations here)
        // main name_id could be cached to avoid string compares and get from pool
        // (switch to explicit main flag on proc_data or in hir instead)
        //@temporary condition to determine if its entry point or not @06.04.24
        let name = if proc_data.origin_id == hir::ScopeID::new(0)
            && hir.intern.get_str(proc_data.name.id) == "main"
        {
            "main".to_string()
        } else {
            format!("rock_proc_{idx}")
        };

        //@specify linkage and name based on function kind and attributes #[c_call], etc.
        let function = cg.module.add_function(&name, function_ty, None);
        cg.current_funtion = Some(function);

        if let Some(block) = proc_data.block {
            if let Some(value) = codegen_expr(cg, block) {
                let entry = function.get_first_basic_block().expect("entry block");
                cg.builder.position_at_end(entry);
                cg.builder.build_return(Some(&value)).unwrap();
            }
        }
    }
}

//@hir still has tail returned expressions in statements,  and block is an expression
// this results in need to return Optional values from codegen_expr()
// and a lot of unwrap() or expect() calls on always expected values
//@also top level codegen_procedures builds return from tail expr value  if it exists
// hir could potentially generate code without tail returns (not sure yet) @06.04.24
fn codegen_expr<'ctx>(
    cg: &Codegen<'ctx>,
    expr: &hir::Expr,
) -> Option<values::BasicValueEnum<'ctx>> {
    match *expr {
        hir::Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        hir::Expr::Unit => panic!("codegen unexpected hir::Expr::Unit (unit semantics arent done)"),
        hir::Expr::LitNull => Some(codegen_lit_null(cg).into()),
        hir::Expr::LitBool { val } => Some(codegen_lit_bool(cg, val).into()),
        hir::Expr::LitInt { val, ty } => Some(codegen_lit_int(cg, val, ty).into()),
        hir::Expr::LitFloat { val, ty } => Some(codegen_lit_float(cg, val, ty).into()),
        hir::Expr::LitChar { val } => Some(codegen_lit_char(cg, val).into()),
        hir::Expr::LitString { id } => Some(codegen_lit_string(cg, id)),
        hir::Expr::If { if_ } => Some(codegen_if(cg, if_)),
        hir::Expr::Block { stmts } => codegen_block(cg, stmts),
        hir::Expr::Match { match_ } => Some(codegen_match(cg, match_)),
        hir::Expr::UnionMember { target, id } => Some(codegen_union_member(cg, target, id)),
        hir::Expr::StructField { target, id } => Some(codegen_struct_field(cg, target, id)),
        hir::Expr::Index { target, index } => Some(codegen_index(cg, target, index)),
        hir::Expr::Cast { target, kind } => Some(codegen_cast(cg, target, kind)),
        hir::Expr::LocalVar { local_id } => Some(codegen_local_var(cg, local_id)),
        hir::Expr::ParamVar { param_id } => Some(codegen_param_var(cg, param_id)),
        hir::Expr::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        hir::Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, global_id)),
        hir::Expr::EnumVariant { enum_id, id } => Some(codegen_enum_variant(cg, enum_id, id)),
        hir::Expr::ProcCall { proc_id, input } => Some(codegen_proc_call(cg, proc_id, input)),
        hir::Expr::UnionInit { union_id, input } => Some(codegen_union_init(cg, union_id, input)),
        hir::Expr::StructInit { struct_id, input } => {
            Some(codegen_struct_init(cg, struct_id, input))
        }
        hir::Expr::ArrayInit { input } => Some(codegen_array_init(cg, input)),
        hir::Expr::ArrayRepeat { expr, size } => Some(codegen_array_repeat(cg, expr, size)),
        hir::Expr::Unary { op, rhs } => Some(codegen_unary(cg, op, rhs)),
        hir::Expr::Binary { op, lhs, rhs } => Some(codegen_binary(cg, op, lhs, rhs)),
    }
}

fn codegen_lit_null<'ctx>(cg: &Codegen<'ctx>) -> values::IntValue<'ctx> {
    let ptr_type = cg.pointer_sized_int_type();
    ptr_type.const_zero()
}

fn codegen_lit_bool<'ctx>(cg: &Codegen<'ctx>, val: bool) -> values::IntValue<'ctx> {
    let bool_type = cg.context.bool_type();
    bool_type.const_int(val as u64, false)
}

//@since its always u64 value thats not sign extended in 2s compliment form
// pass sign_extend = false
// constfolding isnt done yet by the compiler, most overflow errors wont be caught early
// unary minus should work on this const int conrrectly when llvm const folds it. @06.04.24
fn codegen_lit_int<'ctx>(
    cg: &Codegen<'ctx>,
    val: u64,
    ty: ast::BasicType,
) -> values::IntValue<'ctx> {
    let int_type = cg.type_into_any(hir::Type::Basic(ty)).into_int_type();
    int_type.const_int(val, false)
}

fn codegen_lit_float<'ctx>(
    cg: &Codegen<'ctx>,
    val: f64,
    ty: ast::BasicType,
) -> values::FloatValue<'ctx> {
    let float_type = cg.type_into_any(hir::Type::Basic(ty)).into_float_type();
    float_type.const_float(val)
}

fn codegen_lit_char<'ctx>(cg: &Codegen<'ctx>, val: char) -> values::IntValue<'ctx> {
    let char_type = cg
        .type_into_any(hir::Type::Basic(ast::BasicType::Char))
        .into_int_type();
    char_type.const_int(val as u64, false)
}

fn codegen_lit_string<'ctx>(cg: &Codegen<'ctx>, id: InternID) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `lit_string` not supported")
}

fn codegen_if<'ctx>(cg: &Codegen<'ctx>, if_: &hir::If) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `if` not supported")
}

fn codegen_block<'ctx>(
    cg: &Codegen<'ctx>,
    stmts: &[hir::Stmt],
) -> Option<values::BasicValueEnum<'ctx>> {
    let function = cg.current_funtion.expect("current function");
    let basic_block = cg.context.append_basic_block(function, "block");
    cg.builder.position_at_end(basic_block);

    for (idx, stmt) in stmts.iter().enumerate() {
        match *stmt {
            hir::Stmt::Break => todo!("codegen `break` not supported"),
            hir::Stmt::Continue => todo!("codegen `continue` not supported"),
            hir::Stmt::Return => {
                cg.builder.build_return(None).unwrap();
            }
            hir::Stmt::ReturnVal(expr) => {
                let value = codegen_expr(cg, expr).expect("value");
                cg.builder.build_return(Some(&value)).unwrap();
            }
            hir::Stmt::Defer(_) => todo!("codegen `defer` not supported"),
            hir::Stmt::ForLoop(_) => todo!("codegen `for loop` not supported"),
            hir::Stmt::Local(_) => todo!("codegen `local` not supported"),
            hir::Stmt::Assign(_) => todo!("codegen `assign` not supported"),
            hir::Stmt::ExprSemi(expr) => {
                //@are expressions like `5;` valid when output as llvm ir?
                // probably yes
                codegen_expr(cg, expr);
            }
            hir::Stmt::ExprTail(expr) => {
                //@assumed to be last code in the block
                // and is return as block value
                assert_eq!(
                    idx + 1,
                    stmts.len(),
                    "codegen Stmt::ExprTail must be the last statement of the block"
                );
                return codegen_expr(cg, expr);
            }
        }
    }

    None
}

fn codegen_match<'ctx>(cg: &Codegen<'ctx>, match_: &hir::Match) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `match` not supported")
}

fn codegen_union_member<'ctx>(
    cg: &Codegen<'ctx>,
    target: &hir::Expr,
    id: hir::UnionMemberID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `union member access` not supported")
}

fn codegen_struct_field<'ctx>(
    cg: &Codegen<'ctx>,
    target: &hir::Expr,
    id: hir::StructFieldID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `struct field access` not supported")
}

fn codegen_index<'ctx>(
    cg: &Codegen<'ctx>,
    target: &hir::Expr,
    index: &hir::Expr,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `index access` not supported")
}

fn codegen_cast<'ctx>(
    cg: &Codegen<'ctx>,
    target: &hir::Expr,
    kind: hir::CastKind,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `cast` not supported")
}

fn codegen_local_var<'ctx>(
    cg: &Codegen<'ctx>,
    local_id: hir::LocalID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `local var` not supported")
}

fn codegen_param_var<'ctx>(
    cg: &Codegen<'ctx>,
    param_id: hir::ProcParamID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `param var` not supported")
}

fn codegen_const_var<'ctx>(
    cg: &Codegen<'ctx>,
    const_id: hir::ConstID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `const var` not supported")
}

fn codegen_global_var<'ctx>(
    cg: &Codegen<'ctx>,
    global_id: hir::GlobalID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `global var` not supported")
}

fn codegen_enum_variant<'ctx>(
    cg: &Codegen<'ctx>,
    enum_id: hir::EnumID,
    id: hir::EnumVariantID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `enum variant` not supported")
}

fn codegen_proc_call<'ctx>(
    cg: &Codegen<'ctx>,
    proc_id: hir::ProcID,
    input: &[&hir::Expr],
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `proc call` not supported")
}

fn codegen_union_init<'ctx>(
    cg: &Codegen<'ctx>,
    union_id: hir::UnionID,
    input: hir::UnionMemberInit,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `union init` not supported")
}

fn codegen_struct_init<'ctx>(
    cg: &Codegen<'ctx>,
    struct_id: hir::StructID,
    input: &[hir::StructFieldInit],
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `struct init` not supported")
}

fn codegen_array_init<'ctx>(
    cg: &Codegen<'ctx>,
    input: &[&hir::Expr],
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `array init` not supported")
}

fn codegen_array_repeat<'ctx>(
    cg: &Codegen<'ctx>,
    expr: &hir::Expr,
    size: hir::ConstExpr,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `array repeat` not supported")
}

fn codegen_unary<'ctx>(
    cg: &Codegen<'ctx>,
    op: ast::UnOp,
    rhs: &hir::Expr,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `unary` not supported")
}

fn codegen_binary<'ctx>(
    cg: &Codegen<'ctx>,
    op: ast::BinOp,
    lhs: &hir::Expr,
    rhs: &hir::Expr,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `binary` not supported")
}

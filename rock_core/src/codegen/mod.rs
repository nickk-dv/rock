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
    function_values: Vec<values::FunctionValue<'ctx>>,
    hir: hir::Hir<'ctx>,
}

struct ProcCodegen<'ctx> {
    proc_id: hir::ProcID,
    function: values::FunctionValue<'ctx>,
    local_vars: Vec<Option<values::PointerValue<'ctx>>>,
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
            function_values: Vec::new(),
            hir,
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
            hir::Type::ArraySlice(slice) => todo!(),
            hir::Type::ArrayStatic(array) => {
                //@array static shoudnt always carry expresion inside
                // when its not declared it might just store u32 or u64 size without allocations
                // store u32 since its expected size for static arrays @06.04.24
                let elem_ty = self.type_into_basic(array.ty).expect("non void type");
                if let hir::Expr::LitInt { val, .. } = *array.size.0 {
                    elem_ty.array_type(val as u32).into()
                } else {
                    panic!("codegen: invalid array static size expression");
                }
            }
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
    let mut cg = Codegen::new(&context, hir);
    codegen_struct_types(&mut cg);
    codegen_function_values(&mut cg);
    codegen_function_bodies(&mut cg);
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

fn codegen_function_values(cg: &mut Codegen) {
    cg.function_values.reserve_exact(cg.hir.structs.len());

    let mut param_types = Vec::<types::BasicMetadataTypeEnum>::new();
    for proc_data in cg.hir.procs.iter() {
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

        let name = cg.hir.intern.get_str(proc_data.name.id);
        let name = if proc_data.block.is_none() {
            name
        } else if proc_data.origin_id == hir::ScopeID::new(0) && name == "main" {
            name
        } else {
            "rock_proc"
        };

        //@specify linkage and name based on function kind and attributes #[c_call], etc.
        let function = cg.module.add_function(&name, function_ty, None);
        cg.function_values.push(function);
    }
}

fn codegen_function_bodies<'ctx>(cg: &Codegen<'ctx>) {
    for (idx, proc_data) in cg.hir.procs.iter().enumerate() {
        if let Some(block) = proc_data.block {
            let function = cg.function_values[idx];

            let mut local_vars = Vec::new();
            local_vars.resize_with(proc_data.body.locals.len(), || None);
            let mut proc_cg = ProcCodegen {
                function,
                proc_id: hir::ProcID::new(idx),
                local_vars,
            };

            let entry_block = cg.context.append_basic_block(proc_cg.function, "entry");
            cg.builder.position_at_end(entry_block);

            if let Some(value) = codegen_expr(cg, &mut proc_cg, false, block) {
                let entry = function.get_first_basic_block().expect("entry block");
                cg.builder.position_at_end(entry);
                cg.builder.build_return(Some(&value)).unwrap();
            } else {
                //@hack generating implicit return
                //also generate it on last block on regular void return functions?
                // cannot detect if `return;` was already written there
                //@overall all returns must be included in Hir explicitly,
                //so codegen doesnt need to do any work to get correct outputs
                let entry = function.get_first_basic_block().expect("entry block");
                if entry.get_terminator().is_none() {
                    cg.builder.position_at_end(entry);
                    cg.builder.build_return(None).unwrap();
                }
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
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    expr: &hir::Expr,
) -> Option<values::BasicValueEnum<'ctx>> {
    use hir::Expr;
    match *expr {
        Expr::Error => panic!("codegen unexpected hir::Expr::Error"),
        Expr::Unit => panic!("codegen unexpected hir::Expr::Unit (unit semantics arent done)"),
        Expr::LitNull => Some(codegen_lit_null(cg)),
        Expr::LitBool { val } => Some(codegen_lit_bool(cg, val)),
        Expr::LitInt { val, ty } => Some(codegen_lit_int(cg, val, ty)),
        Expr::LitFloat { val, ty } => Some(codegen_lit_float(cg, val, ty)),
        Expr::LitChar { val } => Some(codegen_lit_char(cg, val)),
        Expr::LitString { id } => Some(codegen_lit_string(cg, id)),
        Expr::If { if_ } => Some(codegen_if(cg, if_)),
        Expr::Block { stmts } => codegen_block(cg, proc_cg, expect_ptr, stmts),
        Expr::Match { match_ } => Some(codegen_match(cg, match_)),
        Expr::UnionMember {
            target,
            union_id,
            id,
        } => Some(codegen_union_member(cg, target, union_id, id)),
        Expr::StructField {
            target,
            struct_id,
            id,
        } => Some(codegen_struct_field(
            cg, proc_cg, expect_ptr, target, struct_id, id,
        )),
        Expr::Index { target, index } => {
            Some(codegen_index(cg, proc_cg, expect_ptr, target, index))
        }
        Expr::Cast { target, into, kind } => Some(codegen_cast(cg, proc_cg, target, into, kind)),
        Expr::LocalVar { local_id } => Some(codegen_local_var(cg, proc_cg, expect_ptr, local_id)),
        Expr::ParamVar { param_id } => Some(codegen_param_var(cg, param_id)),
        Expr::ConstVar { const_id } => Some(codegen_const_var(cg, const_id)),
        Expr::GlobalVar { global_id } => Some(codegen_global_var(cg, global_id)),
        Expr::EnumVariant { enum_id, id } => Some(codegen_enum_variant(cg, enum_id, id)),
        Expr::ProcCall { proc_id, input } => codegen_proc_call(cg, proc_cg, proc_id, input),
        Expr::UnionInit { union_id, input } => Some(codegen_union_init(cg, union_id, input)),
        Expr::StructInit { struct_id, input } => Some(codegen_struct_init(
            cg, proc_cg, expect_ptr, struct_id, input,
        )),
        Expr::ArrayInit { array_init } => {
            Some(codegen_array_init(cg, proc_cg, expect_ptr, array_init))
        }
        Expr::ArrayRepeat { array_repeat } => Some(codegen_array_repeat(cg, proc_cg, array_repeat)),
        Expr::Unary { op, rhs } => Some(codegen_unary(cg, op, rhs)),
        Expr::Binary { op, lhs, rhs } => Some(codegen_binary(cg, op, lhs, rhs)),
    }
}

fn codegen_lit_null<'ctx>(cg: &Codegen<'ctx>) -> values::BasicValueEnum<'ctx> {
    let ptr_type = cg.pointer_sized_int_type();
    ptr_type.const_zero().into()
}

fn codegen_lit_bool<'ctx>(cg: &Codegen<'ctx>, val: bool) -> values::BasicValueEnum<'ctx> {
    let bool_type = cg.context.bool_type();
    bool_type.const_int(val as u64, false).into()
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

#[allow(unsafe_code)]
fn codegen_lit_string<'ctx>(cg: &Codegen<'ctx>, id: InternID) -> values::BasicValueEnum<'ctx> {
    //@creating always null terminated bytes array in data section
    let string = cg.hir.intern.get_str(id);
    let array_value = cg.context.const_string(string.as_bytes(), true);
    let array_ty = array_value.get_type();
    let global = cg.module.add_global(array_ty, None, "global_string");
    global.set_initializer(&array_value);
    global.set_constant(true);
    global.as_pointer_value().into()
}

fn codegen_if<'ctx>(cg: &Codegen<'ctx>, if_: &hir::If) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `if` not supported")
}

fn codegen_block<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    stmts: &[hir::Stmt],
) -> Option<values::BasicValueEnum<'ctx>> {
    for (idx, stmt) in stmts.iter().enumerate() {
        match *stmt {
            hir::Stmt::Break => todo!("codegen `break` not supported"),
            hir::Stmt::Continue => todo!("codegen `continue` not supported"),
            hir::Stmt::Return => {
                cg.builder.build_return(None).unwrap();
            }
            hir::Stmt::ReturnVal(expr) => {
                let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
                cg.builder.build_return(Some(&value)).unwrap();
            }
            hir::Stmt::Defer(_) => todo!("codegen `defer` not supported"),
            hir::Stmt::ForLoop(for_) => {
                match for_.kind {
                    hir::ForKind::Loop => {}
                    hir::ForKind::While { cond } => todo!("codegen `for while` not supported"),
                    hir::ForKind::ForLoop {
                        local_id,
                        cond,
                        assign,
                    } => todo!("codegen `for c-like` not supported"),
                }

                let body_block = cg.context.append_basic_block(proc_cg.function, "loop_body");
                cg.builder.build_unconditional_branch(body_block).unwrap();
                cg.builder.position_at_end(body_block);
                codegen_expr(cg, proc_cg, false, for_.block);
                cg.builder.position_at_end(body_block);
                cg.builder.build_unconditional_branch(body_block).unwrap();
            }
            hir::Stmt::Local(local_id) => {
                let local = cg.hir.proc_data(proc_cg.proc_id).body.locals[local_id.index()];
                let var_ty = cg.type_into_basic(local.ty).expect("non void type");

                //@variables without value expression are always zero initialized
                // theres no way to detect potentially uninitialized variables
                // during check and analysis phases, this might change. @06.04.24
                let value = if let Some(expr) = local.value {
                    codegen_expr(cg, proc_cg, false, expr).expect("value")
                } else {
                    var_ty.const_zero()
                };

                let var_ptr = cg.builder.build_alloca(var_ty, "local").unwrap();
                cg.builder.build_store(var_ptr, value).unwrap();
                proc_cg.local_vars[local_id.index()] = Some(var_ptr);
            }
            hir::Stmt::Assign(_) => todo!("codegen `assign` not supported"),
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
                return codegen_expr(cg, proc_cg, expect_ptr, expr);
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
    union_id: hir::UnionID,
    id: hir::UnionMemberID,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `union member access` not supported")
}

fn codegen_struct_field<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &hir::Expr,
    struct_id: hir::StructID,
    id: hir::StructFieldID,
) -> values::BasicValueEnum<'ctx> {
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = target.into_pointer_value();

    let struct_ty = cg
        .type_into_basic(hir::Type::Struct(struct_id))
        .expect("non void type");
    let field = cg.hir.struct_data(struct_id).fields[id.index()];
    let field_ty = cg.type_into_basic(field.ty).expect("value");
    let field_ptr = cg
        .builder
        .build_struct_gep(struct_ty, target_ptr, id.index() as u32, "field_ptr")
        .unwrap();

    if expect_ptr {
        field_ptr.into()
    } else {
        cg.builder
            .build_load(field_ty, field_ptr, "field_val")
            .unwrap()
    }
}

#[allow(unsafe_code)]
fn codegen_index<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    target: &hir::Expr,
    index: &hir::Expr,
) -> values::BasicValueEnum<'ctx> {
    todo!("codegen `union index` not supported")

    /*
    let target = codegen_expr(cg, proc_cg, true, target).expect("value");
    let target_ptr = target.into_pointer_value();

    let index = codegen_expr(cg, proc_cg, false, index)
        .expect("value")
        .into_int_value();

    let elem_ptr = unsafe {
        cg.builder
            .build_gep(target_ty, target_ptr, &[index], "elem_ptr")
            .unwrap()
    };

    if expect_ptr {
        elem_ptr.into()
    } else {
        cg.builder
            .build_load(elem_ty, elem_ptr, "elem_val")
            .unwrap()
    }
    */
}

fn codegen_cast<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    target: &hir::Expr,
    into: &hir::Type,
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
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    local_id: hir::LocalID,
) -> values::BasicValueEnum<'ctx> {
    let local = cg.hir.proc_data(proc_cg.proc_id).body.locals[local_id.index()];
    let var_ty = cg.type_into_basic(local.ty).expect("non void type");
    let var_ptr = proc_cg.local_vars[local_id.index()].expect("var ptr");

    if expect_ptr {
        var_ptr.into()
    } else {
        cg.builder.build_load(var_ty, var_ptr, "local_val").unwrap()
    }
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
    proc_cg: &mut ProcCodegen<'ctx>,
    proc_id: hir::ProcID,
    input: &[&hir::Expr],
) -> Option<values::BasicValueEnum<'ctx>> {
    let mut input_values = Vec::with_capacity(input.len());
    for &expr in input {
        let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
        input_values.push(values::BasicMetadataValueEnum::from(value));
    }

    let function = cg.function_values[proc_id.index()];
    let call_val = cg
        .builder
        .build_direct_call(function, &input_values, "call_val")
        .unwrap();
    call_val.try_as_basic_value().left()
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
    proc_cg: &mut ProcCodegen<'ctx>,
    expect_ptr: bool,
    struct_id: hir::StructID,
    input: &[hir::StructFieldInit],
) -> values::BasicValueEnum<'ctx> {
    let struct_ty = cg
        .type_into_basic(hir::Type::Struct(struct_id))
        .expect("non void type");
    let struct_ptr = cg.builder.build_alloca(struct_ty, "struct_temp").unwrap();

    for field_init in input {
        let value = codegen_expr(cg, proc_cg, false, field_init.expr).expect("value");
        let field_idx = field_init.field_id.index() as u32;
        let field_ptr = cg
            .builder
            .build_struct_gep(struct_ty, struct_ptr, field_idx, "field_ptr")
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
    array_init: &hir::ArrayInit,
) -> values::BasicValueEnum<'ctx> {
    let elem_ty = cg
        .type_into_basic(array_init.elem_ty)
        .expect("non void type");
    let array_ty = elem_ty.array_type(array_init.input.len() as u32);
    let array_ptr = cg.builder.build_alloca(array_ty, "array_temp").unwrap();
    let index_type = cg.pointer_sized_int_type();

    for (idx, &expr) in array_init.input.iter().enumerate() {
        let value = codegen_expr(cg, proc_cg, false, expr).expect("value");
        let index_value = index_type.const_int(idx as u64, false);
        let elem_ptr = unsafe {
            cg.builder
                .build_in_bounds_gep(array_ty, array_ptr, &[index_value], "elem_ptr")
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
    array_repeat: &hir::ArrayRepeat,
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

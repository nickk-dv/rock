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
use inkwell::types::{self, BasicType};
use inkwell::values;
use std::collections::HashMap;

pub struct Codegen<'ctx> {
    pub context: &'ctx context::Context,
    pub module: module::Module<'ctx>,
    pub builder: builder::Builder<'ctx>,
    target_machine: targets::TargetMachine,
    pub string_lits: Vec<values::GlobalValue<'ctx>>,
    pub structs: Vec<types::StructType<'ctx>>,
    pub consts: Vec<values::BasicValueEnum<'ctx>>,
    pub globals: Vec<values::GlobalValue<'ctx>>,
    pub function_values: Vec<values::FunctionValue<'ctx>>,
    pub hir: hir::Hir<'ctx>,
    pub c_functions: HashMap<InternID, values::FunctionValue<'ctx>>,
    pub ptr_type: types::PointerType<'ctx>,
    pub ptr_sized_int_type: types::IntType<'ctx>,
    pub slice_type: types::StructType<'ctx>,
}

pub struct ProcCodegen<'ctx> {
    pub proc_id: hir::ProcID,
    pub function: values::FunctionValue<'ctx>,
    pub param_vars: Vec<values::PointerValue<'ctx>>,
    pub local_vars: Vec<values::PointerValue<'ctx>>,
    pub block_info: Vec<BlockInfo<'ctx>>,
    pub defer_blocks: Vec<hir::Block<'ctx>>,
    pub next_loop_info: Option<LoopInfo<'ctx>>,
}

#[derive(Copy, Clone)]
pub struct BlockInfo<'ctx> {
    defer_count: u32,
    loop_info: Option<LoopInfo<'ctx>>,
}

#[derive(Copy, Clone)]
pub struct LoopInfo<'ctx> {
    pub break_bb: BasicBlock<'ctx>,
    pub continue_bb: BasicBlock<'ctx>,
}

impl<'ctx> Codegen<'ctx> {
    pub fn new(hir: hir::Hir<'ctx>, context: &'ctx context::Context) -> Codegen<'ctx> {
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

    pub fn finish_module(
        self,
    ) -> Result<(module::Module<'ctx>, targets::TargetMachine), ErrorComp> {
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
    pub fn function_type(&self, proc_ty: &hir::ProcType) -> types::FunctionType<'ctx> {
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

    pub fn array_static_len(&self, len: hir::ArrayStaticLen) -> u64 {
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

    pub fn union_type(&self, union_id: hir::UnionID) -> types::ArrayType<'ctx> {
        let data = self.hir.union_data(union_id);
        let size = data.size_eval.get_size().expect("resolved");
        self.basic_type_into_int(ast::BasicType::U8)
            .array_type(size.size() as u32)
    }

    pub fn struct_type(&self, struct_id: hir::StructID) -> types::StructType<'ctx> {
        self.structs[struct_id.index()]
    }

    pub fn array_type(&self, array: &hir::ArrayStatic) -> types::ArrayType<'ctx> {
        // @should use LLVMArrayType2 which takes u64, what not exposed 03.05.24
        //  by inkwell even for llvm 17 (LLVMArrayType was deprecated in this version)
        let elem_ty = self.type_into_basic(array.elem_ty);
        elem_ty.array_type(self.array_static_len(array.len) as u32)
    }

    pub fn basic_type_into_int(&self, basic: ast::BasicType) -> types::IntType<'ctx> {
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

    pub fn basic_type_into_float(&self, basic: ast::BasicType) -> types::FloatType<'ctx> {
        match basic {
            ast::BasicType::F16 => self.context.f16_type(),
            ast::BasicType::F32 => self.context.f32_type(),
            ast::BasicType::F64 => self.context.f64_type(),
            _ => unreachable!(),
        }
    }

    #[inline]
    pub fn type_into_basic(&self, ty: hir::Type) -> types::BasicTypeEnum<'ctx> {
        self.type_into_any(ty).try_into().expect("value type")
    }

    #[inline]
    pub fn type_into_basic_option(&self, ty: hir::Type) -> Option<types::BasicTypeEnum<'ctx>> {
        self.type_into_any(ty).try_into().ok()
    }

    #[inline]
    pub fn type_into_basic_metadata(&self, ty: hir::Type) -> types::BasicMetadataTypeEnum<'ctx> {
        self.type_into_any(ty).try_into().expect("value type")
    }

    #[inline]
    #[must_use]
    pub fn append_bb(&self, proc_cg: &ProcCodegen<'ctx>, name: &'static str) -> BasicBlock<'ctx> {
        self.context.append_basic_block(proc_cg.function, name)
    }
    #[inline]
    #[must_use]
    pub fn insert_bb(&self, after_bb: BasicBlock<'ctx>, name: &'static str) -> BasicBlock<'ctx> {
        self.context.insert_basic_block_after(after_bb, name)
    }
    #[inline]
    #[must_use]
    pub fn get_insert_bb(&self) -> BasicBlock<'ctx> {
        self.builder.get_insert_block().unwrap()
    }
    #[inline]
    pub fn position_at_end(&self, bb: BasicBlock<'ctx>) {
        self.builder.position_at_end(bb);
    }
    #[inline]
    pub fn build_br(&self, bb: BasicBlock<'ctx>) {
        self.builder.build_unconditional_branch(bb).unwrap();
    }
    #[inline]
    pub fn build_br_no_term(&self, bb: BasicBlock<'ctx>) {
        let insert_bb = self.get_insert_bb();
        if insert_bb.get_terminator().is_none() {
            self.builder.build_unconditional_branch(bb).unwrap();
        }
    }
    #[inline]
    pub fn build_cond_br(
        &self,
        cond: values::BasicValueEnum<'ctx>,
        then_bb: BasicBlock<'ctx>,
        else_bb: BasicBlock<'ctx>,
    ) {
        self.builder
            .build_conditional_branch(cond.into_int_value(), then_bb, else_bb)
            .unwrap();
    }
    #[inline]
    pub fn build_ret(&self, value: Option<values::BasicValueEnum<'ctx>>) {
        if let Some(value) = value {
            self.builder.build_return(Some(&value)).unwrap();
        } else {
            self.builder.build_return(None).unwrap();
        }
    }
}

impl<'ctx> ProcCodegen<'ctx> {
    pub fn set_next_loop_info(
        &mut self,
        break_bb: BasicBlock<'ctx>,
        continue_bb: BasicBlock<'ctx>,
    ) {
        self.next_loop_info = Some(LoopInfo {
            break_bb,
            continue_bb,
        });
    }

    pub fn enter_block(&mut self) {
        self.block_info.push(BlockInfo {
            defer_count: 0,
            loop_info: self.next_loop_info,
        });
        self.next_loop_info = None;
    }

    pub fn exit_block(&mut self) {
        let last_count = self.block_info.last().unwrap().defer_count;
        for _ in 0..last_count {
            assert!(self.defer_blocks.pop().is_some());
        }
        assert!(self.block_info.pop().is_some());
    }

    pub fn push_defer_block(&mut self, block: hir::Block<'ctx>) {
        self.block_info.last_mut().unwrap().defer_count += 1;
        self.defer_blocks.push(block);
    }

    pub fn last_loop_info(&self) -> (LoopInfo<'ctx>, Vec<hir::Block<'ctx>>) {
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

    pub fn last_defer_blocks(&self) -> Vec<hir::Block<'ctx>> {
        let total_count = self.defer_blocks.len();
        let defer_count = self.block_info.last().unwrap().defer_count;
        let range = total_count - defer_count as usize..total_count;
        self.defer_blocks[range].to_vec()
    }

    //@lifetime problems, need to clone like this (since codegen_expr can mutate this vec) 05.05.24
    pub fn all_defer_blocks(&self) -> Vec<hir::Block<'ctx>> {
        self.defer_blocks.clone()
    }
}

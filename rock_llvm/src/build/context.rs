use crate::llvm;
use rock_core::ast;
use rock_core::hir;
use rock_core::id_impl;

pub struct Codegen<'c> {
    pub context: llvm::IRContext,
    pub module: llvm::IRModule,
    pub build: llvm::IRBuilder,
    pub procs: Vec<(llvm::ValueFn, llvm::TypeFn)>,
    pub structs: Vec<llvm::TypeStruct>,
    pub consts: Vec<llvm::Value>,
    pub globals: Vec<llvm::ValueGlobal>,
    pub string_lits: Vec<llvm::ValueGlobal>,
    pub hir: hir::Hir<'c>,
    cache: CodegenCache,
}

pub struct ProcCodegen<'c> {
    pub proc_id: hir::ProcID,
    pub fn_val: llvm::ValueFn,
    pub param_ptrs: Vec<llvm::ValuePtr>,
    pub local_ptrs: Vec<llvm::ValuePtr>,
    tail_values: Vec<Option<TailValue>>,
    block_stack: Vec<BlockInfo>,
    defer_blocks: Vec<hir::Block<'c>>,
    next_loop_info: Option<LoopInfo>,
}

struct BlockInfo {
    defer_count: u32,
    loop_info: Option<LoopInfo>,
}

#[derive(Copy, Clone)]
pub struct LoopInfo {
    pub break_bb: llvm::BasicBlock,
    pub continue_bb: llvm::BasicBlock,
}

#[derive(Copy, Clone)]
pub enum Expect {
    Value(Option<TailValueID>),
    Pointer,
    Store(llvm::ValuePtr),
}

id_impl!(TailValueID);
#[derive(Copy, Clone)]
pub struct TailValue {
    pub value_ptr: llvm::ValuePtr,
    pub value_ty: llvm::Type,
}

struct CodegenCache {
    int_1: llvm::Type,
    int_8: llvm::Type,
    int_16: llvm::Type,
    int_32: llvm::Type,
    int_64: llvm::Type,
    float_32: llvm::Type,
    float_64: llvm::Type,
    ptr_type: llvm::Type,
    void_type: llvm::Type,
    slice_type: llvm::TypeStruct,
}

impl<'c> Codegen<'c> {
    pub fn new(hir: hir::Hir<'c>) -> Codegen<'c> {
        let context = llvm::IRContext::new();
        let module = llvm::IRModule::new(&context, "rock_module");
        let build = llvm::IRBuilder::new(&context);
        let cache = CodegenCache::new(&context);

        Codegen {
            context,
            module,
            build,
            procs: Vec::with_capacity(hir.procs.len()),
            structs: Vec::with_capacity(hir.structs.len()),
            consts: Vec::with_capacity(hir.consts.len()),
            globals: Vec::with_capacity(hir.globals.len()),
            string_lits: Vec::with_capacity(hir.intern_string.get_all_strings().len()),
            hir,
            cache,
        }
    }

    pub fn ty(&self, ty: hir::Type) -> llvm::Type {
        match ty {
            hir::Type::Error => unreachable!(),
            hir::Type::Basic(basic) => self.basic_type(basic),
            hir::Type::Enum(enum_id) => self.enum_type(enum_id),
            hir::Type::Struct(struct_id) => self.struct_type(struct_id).as_ty(),
            hir::Type::Reference(_, _) => self.cache.ptr_type,
            hir::Type::Procedure(_) => self.cache.ptr_type,
            hir::Type::ArraySlice(_) => self.cache.slice_type.as_ty(),
            hir::Type::ArrayStatic(array) => self.array_type(array),
        }
    }

    pub fn basic_type(&self, basic: ast::BasicType) -> llvm::Type {
        match basic {
            ast::BasicType::S8 => self.cache.int_8,
            ast::BasicType::S16 => self.cache.int_16,
            ast::BasicType::S32 => self.cache.int_32,
            ast::BasicType::S64 => self.cache.int_64,
            ast::BasicType::Ssize => self.cache.int_64, //@assume 64bit
            ast::BasicType::U8 => self.cache.int_8,
            ast::BasicType::U16 => self.cache.int_16,
            ast::BasicType::U32 => self.cache.int_32,
            ast::BasicType::U64 => self.cache.int_64,
            ast::BasicType::Usize => self.cache.int_64, //@assume 64bit
            ast::BasicType::F32 => self.cache.float_32,
            ast::BasicType::F64 => self.cache.float_64,
            ast::BasicType::Bool => self.cache.int_1,
            ast::BasicType::Char => self.cache.int_32,
            ast::BasicType::Rawptr => self.cache.ptr_type,
            ast::BasicType::Void => self.cache.void_type,
            ast::BasicType::Never => self.cache.void_type, //@only expected in proc return type
        }
    }

    pub fn bool_type(&self) -> llvm::Type {
        self.cache.int_1
    }

    pub fn enum_type(&self, enum_id: hir::EnumID) -> llvm::Type {
        unimplemented!()
    }

    pub fn struct_type(&self, struct_id: hir::StructID) -> llvm::TypeStruct {
        self.structs[struct_id.index()]
    }

    pub fn ptr_type(&self) -> llvm::Type {
        self.cache.ptr_type
    }

    pub fn ptr_sized_int(&self) -> llvm::Type {
        //@target dependant
        self.cache.int_64
    }

    pub fn proc_type(&self, proc_ty: &hir::ProcType) -> llvm::TypeFn {
        let mut param_types: Vec<llvm::Type> = Vec::with_capacity(proc_ty.param_types.len());
        for &param_ty in proc_ty.param_types {
            param_types.push(self.ty(param_ty));
        }
        let return_ty = self.ty(proc_ty.return_ty);
        llvm::function_type(return_ty, &param_types, proc_ty.is_variadic)
    }

    pub fn slice_type(&self) -> llvm::TypeStruct {
        self.cache.slice_type
    }

    pub fn array_type(&self, array: &hir::ArrayStatic) -> llvm::Type {
        let elem_ty = self.ty(array.elem_ty);
        let len = self.array_len(array.len);
        llvm::array_type(elem_ty, len)
    }

    pub fn array_len(&self, len: hir::ArrayStaticLen) -> u64 {
        match len {
            hir::ArrayStaticLen::Immediate(len) => len.unwrap(),
            hir::ArrayStaticLen::ConstEval(eval_id) => match self.hir.const_eval_value(eval_id) {
                hir::ConstValue::Int { val, .. } => val,
                _ => unreachable!(),
            },
        }
    }

    #[inline]
    pub fn append_bb(&self, proc_cg: &ProcCodegen, name: &str) -> llvm::BasicBlock {
        self.context.append_bb(proc_cg.fn_val, name)
    }
    #[inline]
    pub fn insert_bb_terminated(&self) -> bool {
        self.build.insert_bb().terminator().is_some()
    }
    #[inline]
    pub fn build_br_no_term(&self, bb: llvm::BasicBlock) {
        if !self.insert_bb_terminated() {
            self.build.br(bb);
        }
    }
    #[must_use]
    pub fn entry_alloca(
        &self,
        proc_cg: &ProcCodegen,
        ty: llvm::Type,
        name: &str,
    ) -> llvm::ValuePtr {
        let insert_bb = self.build.insert_bb();
        let entry_bb = proc_cg.fn_val.entry_bb();

        if let Some(instr) = entry_bb.first_instr() {
            self.build.position_before_instr(instr);
        } else {
            self.build.position_at_end(entry_bb);
        }

        let ptr_val = self.build.alloca(ty, name);
        self.build.position_at_end(insert_bb);
        ptr_val
    }

    #[inline]
    pub fn const_usize(&self, val: u64) -> llvm::Value {
        llvm::const_int(self.ptr_sized_int(), val, false)
    }
    #[inline]
    pub fn const_usize_zero(&self) -> llvm::Value {
        llvm::const_int(self.ptr_sized_int(), 0, false)
    }
    #[inline]
    pub fn const_usize_one(&self) -> llvm::Value {
        llvm::const_int(self.ptr_sized_int(), 1, false)
    }
}

impl<'c> ProcCodegen<'c> {
    pub fn new(proc_id: hir::ProcID, fn_val: llvm::ValueFn) -> ProcCodegen<'c> {
        ProcCodegen {
            proc_id,
            fn_val,
            param_ptrs: Vec::with_capacity(8),
            local_ptrs: Vec::with_capacity(32),
            tail_values: Vec::with_capacity(32),
            block_stack: Vec::with_capacity(8),
            defer_blocks: Vec::new(),
            next_loop_info: None,
        }
    }

    pub fn block_enter(&mut self) {
        let block_info = BlockInfo {
            defer_count: 0,
            loop_info: self.next_loop_info.take(),
        };
        self.block_stack.push(block_info);
    }

    pub fn block_exit(&mut self) {
        let defer_count = self.block_stack.last().unwrap().defer_count;
        for _ in 0..defer_count {
            self.defer_blocks.pop();
        }
        self.block_stack.pop();
    }

    pub fn set_next_loop_info(
        &mut self,
        break_bb: llvm::BasicBlock,
        continue_bb: llvm::BasicBlock,
    ) {
        let loop_info = LoopInfo {
            break_bb,
            continue_bb,
        };
        self.next_loop_info = Some(loop_info);
    }

    pub fn add_defer_block(&mut self, block: hir::Block<'c>) {
        self.block_stack.last_mut().unwrap().defer_count += 1;
        self.defer_blocks.push(block);
    }

    pub fn defer_block(&self, block_idx: usize) -> hir::Block<'c> {
        self.defer_blocks[block_idx]
    }

    pub fn all_defer_blocks(&self) -> std::ops::Range<usize> {
        let total_count = self.defer_blocks.len();
        0..total_count
    }

    pub fn last_defer_blocks(&self) -> std::ops::Range<usize> {
        let total_count = self.defer_blocks.len();
        let defer_count = self.block_stack.last().unwrap().defer_count as usize;
        (total_count - defer_count)..total_count
    }

    pub fn last_loop_info(&self) -> (LoopInfo, std::ops::Range<usize>) {
        let total_count = self.defer_blocks.len();
        let mut defer_count = 0;

        for block_info in self.block_stack.iter().rev() {
            defer_count += block_info.defer_count as usize;
            if let Some(loop_info) = block_info.loop_info {
                return (loop_info, (total_count - defer_count)..total_count);
            }
        }
        unreachable!()
    }

    pub fn add_tail_value(&mut self) -> TailValueID {
        let value_id = TailValueID::new(self.tail_values.len());
        self.tail_values.push(None);
        value_id
    }

    pub fn tail_value(&self, value_id: TailValueID) -> Option<TailValue> {
        self.tail_values[value_id.index()]
    }

    pub fn set_tail_value(
        &mut self,
        value_id: TailValueID,
        value_ptr: llvm::ValuePtr,
        value_ty: llvm::Type,
    ) {
        self.tail_values[value_id.index()] = Some(TailValue {
            value_ptr,
            value_ty,
        });
    }
}

impl CodegenCache {
    fn new(context: &llvm::IRContext) -> CodegenCache {
        CodegenCache {
            int_1: context.int_1(),
            int_8: context.int_8(),
            int_16: context.int_16(),
            int_32: context.int_32(),
            int_64: context.int_64(),
            float_32: context.float_32(),
            float_64: context.float_64(),
            ptr_type: context.ptr_type(),
            void_type: context.void_type(),
            //@assume 64bit
            slice_type: context.struct_type_inline(&[context.ptr_type(), context.int_64()], false),
        }
    }
}

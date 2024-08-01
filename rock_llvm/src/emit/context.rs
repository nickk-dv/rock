use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub struct Codegen<'c> {
    pub context: llvm::IRContext,
    pub module: llvm::IRModule,
    pub build: llvm::IRBuilder,
    pub procs: Vec<llvm::ValueFn>,
    pub structs: Vec<llvm::Type>,
    pub consts: Vec<llvm::Value>,
    pub globals: Vec<llvm::Value>,
    pub string_lits: Vec<llvm::Value>,
    pub hir: hir::Hir<'c>,
    cache: CodegenCache,
}

pub struct ProcCodegen<'c> {
    pub proc_id: hir::ProcID,
    pub fn_val: llvm::ValueFn,
    pub param_ptrs: Vec<llvm::Value>,
    pub local_ptrs: Vec<llvm::Value>,
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
    slice_type: llvm::Type,
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
            hir::Type::Struct(struct_id) => self.struct_type(struct_id),
            hir::Type::Reference(_, _) => self.cache.ptr_type,
            hir::Type::Procedure(_) => self.cache.ptr_type,
            hir::Type::ArraySlice(_) => self.cache.slice_type,
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

    pub fn enum_type(&self, enum_id: hir::EnumID) -> llvm::Type {
        unimplemented!()
    }

    pub fn struct_type(&self, struct_id: hir::StructID) -> llvm::Type {
        self.structs[struct_id.index()]
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
}

impl<'c> ProcCodegen<'c> {
    pub fn new(proc_id: hir::ProcID, fn_val: llvm::ValueFn) -> ProcCodegen<'c> {
        ProcCodegen {
            proc_id,
            fn_val,
            param_ptrs: Vec::with_capacity(8),
            local_ptrs: Vec::with_capacity(32),
            block_stack: Vec::with_capacity(16),
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

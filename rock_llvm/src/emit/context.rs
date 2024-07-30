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

pub struct ProcCodegen {
    pub proc_id: hir::ProcID,
    pub fn_val: llvm::ValueFn,
    pub param_ptrs: Vec<llvm::Value>,
    pub local_ptrs: Vec<llvm::Value>,
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
        unimplemented!()
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

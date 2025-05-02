use std::collections::HashMap;

use crate::llvm;
use rock_core::config::TargetTriple;
use rock_core::hir;
use rock_core::session::Session;
use rock_core::support::TempBuffer;

pub struct Codegen<'c, 's, 'sref> {
    pub proc: ProcCodegen,
    pub context: llvm::IRContext,
    pub target: llvm::IRTarget,
    pub module: llvm::IRModule,
    pub build: llvm::IRBuilder,
    pub procs: Vec<(llvm::ValueFn, llvm::TypeFn)>,
    pub enums: Vec<llvm::Type>,
    pub variants: Vec<Vec<Option<llvm::TypeStruct>>>,
    pub structs: Vec<llvm::TypeStruct>,
    pub globals: Vec<llvm::ValueGlobal>,
    pub string_lits: Vec<llvm::ValueGlobal>,
    pub hir: hir::Hir<'c>,
    pub session: &'sref mut Session<'s>,
    pub string_buf: String,
    pub cache: CodegenCache,
    pub poly_structs: HashMap<hir::StructKey<'c>, llvm::TypeStruct>,
}

pub struct ProcCodegen {
    pub proc_id: hir::ProcID,
    pub fn_val: llvm::ValueFn,
    pub param_ptrs: Vec<llvm::ValuePtr>,
    pub variable_ptrs: Vec<llvm::ValuePtr>,
    tail_values: Vec<Option<TailValue>>,
    block_stack: Vec<BlockInfo>,
    next_loop_info: Option<LoopInfo>,
}

struct BlockInfo {
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

rock_core::define_id!(pub TailValueID);
#[derive(Copy, Clone)]
pub struct TailValue {
    pub value_ptr: llvm::ValuePtr,
    pub value_ty: llvm::Type,
}

pub struct CodegenCache {
    int_1: llvm::Type,
    int_8: llvm::Type,
    int_16: llvm::Type,
    int_32: llvm::Type,
    int_64: llvm::Type,
    float_32: llvm::Type,
    float_64: llvm::Type,
    void_type: llvm::Type,
    ptr_type: llvm::Type,
    ptr_sized_int: llvm::Type,
    slice_type: llvm::TypeStruct,
    void_val_type: llvm::TypeStruct,
    pub sret: llvm::Attribute,
    pub noreturn: llvm::Attribute,
    pub inlinehint: llvm::Attribute,
    pub values: TempBuffer<llvm::Value>,
    pub cases: TempBuffer<(llvm::Value, llvm::BasicBlock)>,
}

impl<'c, 's, 'sref> Codegen<'c, 's, 'sref> {
    pub fn new(
        hir: hir::Hir<'c>,
        triple: TargetTriple,
        session: &'sref mut Session<'s>,
    ) -> Codegen<'c, 's, 'sref> {
        let mut context = llvm::IRContext::new();
        let target = llvm::IRTarget::new(triple, session.config.build);
        let module = llvm::IRModule::new(&context, &target, "rock_module");
        let cache = CodegenCache::new(&mut context, &target);
        let build = llvm::IRBuilder::new(&context, cache.void_val_type);

        Codegen {
            proc: ProcCodegen::new(),
            context,
            target,
            module,
            build,
            procs: Vec::with_capacity(hir.procs.len()),
            enums: Vec::with_capacity(hir.enums.len()),
            variants: Vec::with_capacity(hir.enums.len()),
            structs: Vec::with_capacity(hir.structs.len()),
            globals: Vec::with_capacity(hir.globals.len()),
            string_lits: Vec::with_capacity(session.intern_lit.get_all().len()),
            hir,
            session,
            string_buf: String::with_capacity(256),
            cache,
            poly_structs: HashMap::with_capacity(256),
        }
    }

    pub fn ty(&mut self, ty: hir::Type<'c>) -> llvm::Type {
        self.ty_impl(ty, &[])
    }

    pub fn ty_impl(&mut self, ty: hir::Type<'c>, poly_types: &[hir::Type<'c>]) -> llvm::Type {
        match ty {
            hir::Type::Error | hir::Type::Unknown => unreachable!(),
            hir::Type::Char => self.cache.int_32,
            hir::Type::Void => self.cache.void_val_type.as_ty(),
            hir::Type::Never => self.cache.void_val_type.as_ty(),
            hir::Type::Rawptr => self.cache.ptr_type,
            hir::Type::UntypedChar => unreachable!(),
            hir::Type::Int(int_ty) => self.int_type(int_ty),
            hir::Type::Float(float_ty) => self.float_type(float_ty),
            hir::Type::Bool(bool_ty) => self.bool_type(bool_ty),
            hir::Type::String(string_ty) => match string_ty {
                hir::StringType::String => self.cache.slice_type.as_ty(),
                hir::StringType::CString => self.cache.ptr_type,
                hir::StringType::Untyped => unreachable!(),
            },
            hir::Type::PolyProc(_, _) => unimplemented!("codegen poly_proc type"),
            hir::Type::PolyEnum(_, _) => unimplemented!("codegen poly_enum type"),
            hir::Type::PolyStruct(_, idx) => self.ty(poly_types[idx]),
            hir::Type::Enum(enum_id, poly_types) => {
                if !poly_types.is_empty() {
                    unimplemented!("codegen polymorphic enum type")
                }
                self.enum_type(enum_id)
            }
            hir::Type::Struct(struct_id, poly_types) => {
                if poly_types.is_empty() {
                    self.struct_type(struct_id).as_ty()
                } else {
                    let key = (struct_id, poly_types);
                    if let Some(t) = self.poly_structs.get(&key) {
                        t.as_ty()
                    } else {
                        let data = self.hir.struct_data(struct_id);
                        let name = self.session.intern_name.get(data.name.id);
                        self.string_buf.clear();
                        self.string_buf.push_str(name);
                        self.string_buf.push_str("_Poly_");

                        let opaque = self.context.struct_named_create(&self.string_buf);
                        self.poly_structs.insert(key, opaque);

                        let mut field_types = Vec::with_capacity(64); //@cache
                        for field in data.fields {
                            field_types.push(self.ty_impl(field.ty, poly_types));
                        }
                        self.context.struct_named_set_body(opaque, &field_types, false);

                        opaque.as_ty()
                    }
                }
            }
            hir::Type::Reference(_, _) => self.cache.ptr_type,
            hir::Type::MultiReference(_, _) => self.cache.ptr_type,
            hir::Type::Procedure(_) => self.cache.ptr_type,
            hir::Type::ArraySlice(_) => self.cache.slice_type.as_ty(),
            hir::Type::ArrayStatic(array) => {
                llvm::array_type(self.ty(array.elem_ty), self.array_len(array.len))
            }
        }
    }

    pub fn char_type(&self) -> llvm::Type {
        self.cache.int_32
    }

    pub fn int_type(&self, int_ty: hir::IntType) -> llvm::Type {
        match int_ty {
            hir::IntType::S8 | hir::IntType::U8 => self.cache.int_8,
            hir::IntType::S16 | hir::IntType::U16 => self.cache.int_16,
            hir::IntType::S32 | hir::IntType::U32 => self.cache.int_32,
            hir::IntType::S64 | hir::IntType::U64 => self.cache.int_64,
            hir::IntType::Ssize | hir::IntType::Usize => self.cache.ptr_sized_int,
            hir::IntType::Untyped => unreachable!(),
        }
    }

    pub fn float_type(&self, float_ty: hir::FloatType) -> llvm::Type {
        match float_ty {
            hir::FloatType::F32 => self.cache.float_32,
            hir::FloatType::F64 => self.cache.float_64,
            hir::FloatType::Untyped => unreachable!(),
        }
    }

    pub fn bool_type(&self, bool_ty: hir::BoolType) -> llvm::Type {
        match bool_ty {
            hir::BoolType::Bool => self.cache.int_1,
            hir::BoolType::Bool16 => self.cache.int_16,
            hir::BoolType::Bool32 => self.cache.int_32,
            hir::BoolType::Bool64 => self.cache.int_64,
            hir::BoolType::Untyped => unreachable!(),
        }
    }

    pub fn void_type(&self) -> llvm::Type {
        self.cache.void_type
    }

    pub fn void_val_type(&self) -> llvm::TypeStruct {
        self.cache.void_val_type
    }

    pub fn enum_type(&self, enum_id: hir::EnumID) -> llvm::Type {
        self.enums[enum_id.index()]
    }

    pub fn struct_type(&self, struct_id: hir::StructID) -> llvm::TypeStruct {
        self.structs[struct_id.index()]
    }

    pub fn ptr_type(&self) -> llvm::Type {
        self.cache.ptr_type
    }

    pub fn ptr_sized_int(&self) -> llvm::Type {
        self.cache.ptr_sized_int
    }

    pub fn proc_type(&mut self, proc_ty: &hir::ProcType<'c>) -> llvm::TypeFn {
        let mut param_types: Vec<llvm::Type> = Vec::with_capacity(proc_ty.params.len());
        for param in proc_ty.params {
            param_types.push(self.ty(param.ty));
        }
        let return_ty = self.ty(proc_ty.return_ty);
        let is_variadic = proc_ty.flag_set.contains(hir::ProcFlag::CVariadic);
        llvm::function_type(return_ty, &param_types, is_variadic)
    }

    pub fn slice_type(&self) -> llvm::TypeStruct {
        self.cache.slice_type
    }

    pub fn array_len(&self, len: hir::ArrayStaticLen) -> u64 {
        match len {
            hir::ArrayStaticLen::Immediate(len) => len,
            hir::ArrayStaticLen::ConstEval(eval_id) => {
                match self.hir.const_eval_values[eval_id.index()] {
                    hir::ConstValue::Int { val, .. } => val,
                    _ => unreachable!(),
                }
            }
        }
    }

    #[inline]
    pub fn append_bb(&mut self, name: &str) -> llvm::BasicBlock {
        self.context.append_bb(self.proc.fn_val, name)
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
    pub fn entry_alloca(&mut self, ty: llvm::Type, name: &str) -> llvm::ValuePtr {
        let insert_bb = self.build.insert_bb();
        let entry_bb = self.proc.fn_val.entry_bb();

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
}

impl ProcCodegen {
    pub fn new() -> ProcCodegen {
        ProcCodegen {
            proc_id: hir::ProcID::dummy(),
            fn_val: llvm::ValueFn::null(),
            param_ptrs: Vec::with_capacity(16),
            variable_ptrs: Vec::with_capacity(128),
            tail_values: Vec::with_capacity(32),
            block_stack: Vec::with_capacity(16),
            next_loop_info: None,
        }
    }

    pub fn reset(&mut self, proc_id: hir::ProcID, fn_val: llvm::ValueFn) {
        self.proc_id = proc_id;
        self.fn_val = fn_val;
        self.param_ptrs.clear();
        self.variable_ptrs.clear();
        self.tail_values.clear();
        self.block_stack.clear();
        self.next_loop_info = None;
    }

    pub fn block_enter(&mut self) {
        let block_info = BlockInfo { loop_info: self.next_loop_info.take() };
        self.block_stack.push(block_info);
    }

    pub fn block_exit(&mut self) {
        self.block_stack.pop();
    }

    pub fn set_next_loop_info(
        &mut self,
        break_bb: llvm::BasicBlock,
        continue_bb: llvm::BasicBlock,
    ) {
        let loop_info = LoopInfo { break_bb, continue_bb };
        self.next_loop_info = Some(loop_info);
    }

    pub fn last_loop_info(&self) -> LoopInfo {
        for block in self.block_stack.iter().rev() {
            if let Some(loop_info) = block.loop_info {
                return loop_info;
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
        let value = TailValue { value_ptr, value_ty };
        self.tail_values[value_id.index()] = Some(value);
    }
}

impl CodegenCache {
    fn new(context: &mut llvm::IRContext, target: &llvm::IRTarget) -> CodegenCache {
        let ptr_type = context.ptr_type();
        let ptr_sized_int = target.ptr_sized_int(context);
        let slice_type = context.struct_named_create("rock.slice");
        let void_val_type = context.struct_named_create("rock.void");
        context.struct_named_set_body(slice_type, &[ptr_type, ptr_sized_int], false);
        context.struct_named_set_body(void_val_type, &[], false);

        CodegenCache {
            int_1: context.int_1(),
            int_8: context.int_8(),
            int_16: context.int_16(),
            int_32: context.int_32(),
            int_64: context.int_64(),
            float_32: context.float_32(),
            float_64: context.float_64(),
            void_type: context.void_type(),
            ptr_type,
            ptr_sized_int,
            slice_type,
            void_val_type,
            sret: context.attr_create("sret"),
            noreturn: context.attr_create("noreturn"),
            inlinehint: context.attr_create("inlinehint"),
            values: TempBuffer::new(128),
            cases: TempBuffer::new(128),
        }
    }
}

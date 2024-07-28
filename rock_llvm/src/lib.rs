mod sys;

use std::ffi::c_char;
use sys::core;

pub struct IRContext {
    context: sys::LLVMContextRef,
    module: IRModule,
    builder: IRBuilder,
    cstr_buf: CStrBuffer,
}

pub struct IRModule {
    module: sys::LLVMModuleRef,
    cstr_buf: CStrBuffer,
}

pub struct IRBuilder {
    builder: sys::LLVMBuilderRef,
    cstr_buf: CStrBuffer,
}

pub struct BasicBlock(sys::LLVMBasicBlockRef);
pub type OpCode = sys::LLVMOpcode;
pub type IntPredicate = sys::LLVMIntPredicate;
pub type FloatPredicate = sys::LLVMRealPredicate;

pub struct Value(sys::LLVMValueRef);
pub struct ValueFn(sys::LLVMValueRef);
pub struct ValueGlobal(sys::LLVMValueRef);
pub struct ValueInstr(sys::LLVMValueRef);
pub struct Type(sys::LLVMTypeRef);
pub struct TypeFn(sys::LLVMTypeRef);

struct CStrBuffer(String);

impl IRContext {
    pub fn new(module_name: &str) -> IRContext {
        let context = unsafe { core::LLVMContextCreate() };
        let module = IRModule::new(context, module_name);
        let builder = IRBuilder::new(context);

        IRContext {
            context,
            module,
            builder,
            cstr_buf: CStrBuffer::new(),
        }
    }

    pub fn module(&mut self) -> &mut IRModule {
        &mut self.module
    }
    pub fn builder(&mut self) -> &mut IRBuilder {
        &mut self.builder
    }

    pub fn int_1(&self) -> Type {
        Type(unsafe { core::LLVMInt1TypeInContext(self.context) })
    }
    pub fn int_8(&self) -> Type {
        Type(unsafe { core::LLVMInt8TypeInContext(self.context) })
    }
    pub fn int_16(&self) -> Type {
        Type(unsafe { core::LLVMInt16TypeInContext(self.context) })
    }
    pub fn int_32(&self) -> Type {
        Type(unsafe { core::LLVMInt32TypeInContext(self.context) })
    }
    pub fn int_64(&self) -> Type {
        Type(unsafe { core::LLVMInt64TypeInContext(self.context) })
    }
    pub fn int_128(&self) -> Type {
        Type(unsafe { core::LLVMInt128TypeInContext(self.context) })
    }
    pub fn float_16(&self) -> Type {
        Type(unsafe { core::LLVMHalfTypeInContext(self.context) })
    }
    pub fn float_32(&self) -> Type {
        Type(unsafe { core::LLVMFloatTypeInContext(self.context) })
    }
    pub fn float_64(&self) -> Type {
        Type(unsafe { core::LLVMDoubleTypeInContext(self.context) })
    }
    pub fn void_type(&self) -> Type {
        Type(unsafe { core::LLVMVoidTypeInContext(self.context) })
    }
    pub fn ptr_type(&self) -> Type {
        Type(unsafe { core::LLVMPointerTypeInContext(self.context, 0) })
    }
    pub fn array_type(&self, elem_ty: Type, len: u64) -> Type {
        Type(unsafe { core::LLVMArrayType2(elem_ty.0, len) })
    }
    pub fn function_type(
        &self,
        return_ty: Type,
        param_types: &[Type],
        is_variadic: bool,
    ) -> TypeFn {
        TypeFn(unsafe {
            core::LLVMFunctionType(
                return_ty.0,
                param_types.as_ptr() as *mut sys::LLVMTypeRef,
                param_types.len() as u32,
                is_variadic as i32,
            )
        })
    }

    pub fn struct_create_named(&mut self, name: &str) -> Type {
        Type(unsafe { core::LLVMStructCreateNamed(self.context, self.cstr_buf.cstr(name)) })
    }
    pub fn struct_set_body(&self, struct_ty: Type, field_types: &[Type], packed: bool) {
        unsafe {
            core::LLVMStructSetBody(
                struct_ty.0,
                field_types.as_ptr() as *mut sys::LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        }
    }
    pub fn struct_type_inline(&self, field_types: &[Type], packed: bool) -> Type {
        Type(unsafe {
            core::LLVMStructTypeInContext(
                self.context,
                field_types.as_ptr() as *mut sys::LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        })
    }

    pub fn fn_entry_bb(&self, fn_val: ValueFn) -> BasicBlock {
        BasicBlock(unsafe { core::LLVMGetEntryBasicBlock(fn_val.0) })
    }
    pub fn append_bb(&mut self, fn_val: ValueFn, name: &str) -> BasicBlock {
        BasicBlock(unsafe {
            core::LLVMAppendBasicBlockInContext(self.context, fn_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn add_global(
        &mut self,
        ty: Type,
        name: &str,
        const_val: Value,
        thread_local: bool,
    ) -> ValueGlobal {
        let name = self.cstr_buf.cstr(name);
        let global = unsafe { core::LLVMAddGlobal(self.module.module, ty.0, name) };
        unsafe { core::LLVMSetInitializer(global, const_val.0) };
        unsafe { core::LLVMSetThreadLocal(global, thread_local as i32) };
        ValueGlobal(global)
    }
}

impl Drop for IRContext {
    fn drop(&mut self) {
        unsafe { core::LLVMShutdown() }
    }
}

impl IRModule {
    fn new(context: sys::LLVMContextRef, name: &str) -> IRModule {
        let mut cstr_buf = CStrBuffer::new();
        let name = cstr_buf.cstr(name);
        let module = unsafe { core::LLVMModuleCreateWithNameInContext(name, context) };

        IRModule { module, cstr_buf }
    }

    pub fn add_function(&mut self, name: &str, fn_ty: TypeFn) -> ValueFn {
        let name = self.cstr_buf.cstr(name);
        let fn_val = unsafe { core::LLVMAddFunction(self.module, name, fn_ty.0) };
        ValueFn(fn_val)
    }
}

impl Drop for IRModule {
    fn drop(&mut self) {
        unsafe { core::LLVMDisposeModule(self.module) }
    }
}

impl IRBuilder {
    fn new(context: sys::LLVMContextRef) -> IRBuilder {
        let builder = unsafe { core::LLVMCreateBuilderInContext(context) };

        IRBuilder {
            builder,
            cstr_buf: CStrBuffer::new(),
        }
    }

    pub fn insert_bb(&self) -> BasicBlock {
        BasicBlock(unsafe { core::LLVMGetInsertBlock(self.builder) })
    }
    pub fn position_at_end(&self, bb: BasicBlock) {
        unsafe { core::LLVMPositionBuilderAtEnd(self.builder, bb.0) }
    }
    pub fn position_before_instr(&self, instr: ValueInstr) {
        unsafe { core::LLVMPositionBuilderBefore(self.builder, instr.0) }
    }
    pub fn position_at_instr(&self, bb: BasicBlock, instr: ValueInstr) {
        unsafe { core::LLVMPositionBuilder(self.builder, bb.0, instr.0) }
    }

    pub fn build_ret_void(&self) {
        let _ = unsafe { core::LLVMBuildRetVoid(self.builder) };
    }
    pub fn build_ret(&self, val: Value) {
        let _ = unsafe { core::LLVMBuildRet(self.builder, val.0) };
    }
    pub fn build_br(&self, dest_bb: BasicBlock) {
        let _ = unsafe { core::LLVMBuildBr(self.builder, dest_bb.0) };
    }
    pub fn build_cond_br(&self, cond: Value, then_bb: BasicBlock, else_bb: BasicBlock) {
        let _ = unsafe { core::LLVMBuildCondBr(self.builder, cond.0, then_bb.0, else_bb.0) };
    }
    pub fn build_switch(&self, val: Value, else_bb: BasicBlock, case_count: u32) -> ValueInstr {
        ValueInstr(unsafe { core::LLVMBuildSwitch(self.builder, val.0, else_bb.0, case_count) })
    }
    pub fn build_add_case(&self, switch: ValueInstr, case_val: Value, dest_bb: BasicBlock) {
        unsafe { core::LLVMAddCase(switch.0, case_val.0, dest_bb.0) }
    }
    pub fn build_unreachable(&self) {
        let _ = unsafe { core::LLVMBuildUnreachable(self.builder) };
    }

    pub fn build_bin_op(&mut self, op: OpCode, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildBinOp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn build_neg(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn build_fneg(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildFNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn build_not(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNot(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }

    pub fn build_alloca(&mut self, ty: Type, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildAlloca(self.builder, ty.0, self.cstr_buf.cstr(name)) })
    }
    pub fn build_load(&mut self, ptr_ty: Type, ptr_val: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildLoad2(self.builder, ptr_ty.0, ptr_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn build_store(&self, val: Value, ptr_val: Value) -> Value {
        Value(unsafe { core::LLVMBuildStore(self.builder, val.0, ptr_val.0) })
    }

    pub fn build_inbounds_gep(
        &mut self,
        ptr_ty: Type,
        ptr_val: Value,
        indices: &[Value],
        name: &str,
    ) -> Value {
        Value(unsafe {
            core::LLVMBuildInBoundsGEP2(
                self.builder,
                ptr_ty.0,
                ptr_val.0,
                indices.as_ptr() as *mut sys::LLVMValueRef,
                indices.len() as u32,
                self.cstr_buf.cstr(name),
            )
        })
    }

    pub fn build_struct_gep(
        &mut self,
        ptr_ty: Type,
        ptr_val: Value,
        idx: u32,
        name: &str,
    ) -> Value {
        Value(unsafe {
            core::LLVMBuildStructGEP2(
                self.builder,
                ptr_ty.0,
                ptr_val.0,
                idx,
                self.cstr_buf.cstr(name),
            )
        })
    }

    pub fn build_cast(&mut self, op: OpCode, val: Value, into_ty: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildCast(self.builder, op, val.0, into_ty.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn build_icmp(&mut self, op: IntPredicate, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildICmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn build_fcmp(&mut self, op: FloatPredicate, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildFCmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }

    pub fn build_call(
        &mut self,
        fn_ty: TypeFn,
        fn_val: ValueFn,
        args: &[Value],
        name: &str,
    ) -> Value {
        Value(unsafe {
            core::LLVMBuildCall2(
                self.builder,
                fn_ty.0,
                fn_val.0,
                args.as_ptr() as *mut sys::LLVMValueRef,
                args.len() as u32,
                self.cstr_buf.cstr(name),
            )
        })
    }
}

impl Drop for IRBuilder {
    fn drop(&mut self) {
        unsafe { core::LLVMDisposeBuilder(self.builder) }
    }
}

impl BasicBlock {
    pub fn terminator(&self) -> Option<ValueInstr> {
        let instr = unsafe { core::LLVMGetBasicBlockTerminator(self.0) };
        if !instr.is_null() {
            Some(ValueInstr(instr))
        } else {
            None
        }
    }
    pub fn first_instr(&self) -> ValueInstr {
        ValueInstr(unsafe { core::LLVMGetFirstInstruction(self.0) })
    }
    pub fn last_instr(&self) -> ValueInstr {
        ValueInstr(unsafe { core::LLVMGetLastInstruction(self.0) })
    }
}

impl CStrBuffer {
    fn new() -> CStrBuffer {
        CStrBuffer(String::with_capacity(64))
    }
    fn cstr(&mut self, string: &str) -> *const c_char {
        self.0.clear();
        self.0.push_str(string);
        self.0.push('\0');
        self.0.as_ptr() as *const c_char
    }
}

use crate::sys;
use crate::sys::core;
use std::ffi::c_char;

pub struct IRContext {
    context: sys::LLVMContextRef,
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

struct CStrBuffer(String);

#[derive(Copy, Clone)]
pub struct BasicBlock(sys::LLVMBasicBlockRef);

#[derive(Copy, Clone)]
pub struct Value(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueFn(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueInstr(sys::LLVMValueRef);

#[derive(Copy, Clone)]
pub struct Type(sys::LLVMTypeRef);
#[derive(Copy, Clone)]
pub struct TypeFn(sys::LLVMTypeRef);

pub type OpCode = sys::LLVMOpcode;
pub type Linkage = sys::LLVMLinkage;
pub type IntPredicate = sys::LLVMIntPredicate;
pub type FloatPredicate = sys::LLVMRealPredicate;

impl IRContext {
    pub fn new() -> IRContext {
        IRContext {
            context: unsafe { core::LLVMContextCreate() },
            cstr_buf: CStrBuffer::new(),
        }
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

    pub fn append_bb(&mut self, fn_val: ValueFn, name: &str) -> BasicBlock {
        BasicBlock(unsafe {
            core::LLVMAppendBasicBlockInContext(self.context, fn_val.0, self.cstr_buf.cstr(name))
        })
    }
}

impl Drop for IRContext {
    fn drop(&mut self) {
        unsafe { core::LLVMShutdown() }
    }
}

impl IRModule {
    pub fn new(context: &IRContext, name: &str) -> IRModule {
        let mut cstr_buf = CStrBuffer::new();
        let name = cstr_buf.cstr(name);
        let module = unsafe { core::LLVMModuleCreateWithNameInContext(name, context.context) };
        IRModule { module, cstr_buf }
    }

    #[must_use]
    pub fn add_function(&mut self, name: &str, fn_ty: TypeFn, linkage: Linkage) -> ValueFn {
        let name = self.cstr_buf.cstr(name);
        let fn_val = unsafe { core::LLVMAddFunction(self.module, name, fn_ty.0) };

        unsafe { core::LLVMSetLinkage(fn_val, linkage) };
        ValueFn(fn_val)
    }

    #[must_use]
    pub fn add_global(
        &mut self,
        ty: Type,
        name: &str,
        const_val: Value,
        constant: bool,
        unnamed_addr: bool,
        thread_local: bool,
        linkage: Linkage,
    ) -> Value {
        let name = self.cstr_buf.cstr(name);
        let global_val = unsafe { core::LLVMAddGlobal(self.module, ty.0, name) };
        let unnamed_addr = if unnamed_addr {
            sys::LLVMUnnamedAddr::LLVMGlobalUnnamedAddr
        } else {
            sys::LLVMUnnamedAddr::LLVMNoUnnamedAddr
        };

        unsafe { core::LLVMSetInitializer(global_val, const_val.0) };
        unsafe { core::LLVMSetGlobalConstant(global_val, constant as i32) };
        unsafe { core::LLVMSetUnnamedAddress(global_val, unnamed_addr) };
        unsafe { core::LLVMSetThreadLocal(global_val, thread_local as i32) };
        unsafe { core::LLVMSetLinkage(global_val, linkage) };
        Value(global_val)
    }
}

impl Drop for IRModule {
    fn drop(&mut self) {
        unsafe { core::LLVMDisposeModule(self.module) }
    }
}

impl IRBuilder {
    pub fn new(context: &IRContext) -> IRBuilder {
        IRBuilder {
            builder: unsafe { core::LLVMCreateBuilderInContext(context.context) },
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

    pub fn ret_void(&self) {
        let _ = unsafe { core::LLVMBuildRetVoid(self.builder) };
    }
    pub fn ret(&self, val: Value) {
        let _ = unsafe { core::LLVMBuildRet(self.builder, val.0) };
    }
    pub fn br(&self, dest_bb: BasicBlock) {
        let _ = unsafe { core::LLVMBuildBr(self.builder, dest_bb.0) };
    }
    pub fn cond_br(&self, cond: Value, then_bb: BasicBlock, else_bb: BasicBlock) {
        let _ = unsafe { core::LLVMBuildCondBr(self.builder, cond.0, then_bb.0, else_bb.0) };
    }
    pub fn switch(&self, val: Value, else_bb: BasicBlock, case_count: u32) -> ValueInstr {
        ValueInstr(unsafe { core::LLVMBuildSwitch(self.builder, val.0, else_bb.0, case_count) })
    }
    pub fn add_case(&self, switch: ValueInstr, case_val: Value, dest_bb: BasicBlock) {
        unsafe { core::LLVMAddCase(switch.0, case_val.0, dest_bb.0) }
    }
    pub fn unreachable(&self) {
        let _ = unsafe { core::LLVMBuildUnreachable(self.builder) };
    }

    pub fn bin_op(&mut self, op: OpCode, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildBinOp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn neg(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn fneg(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildFNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn not(&mut self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNot(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }

    pub fn alloca(&mut self, ty: Type, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildAlloca(self.builder, ty.0, self.cstr_buf.cstr(name)) })
    }
    pub fn load(&mut self, ptr_ty: Type, ptr_val: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildLoad2(self.builder, ptr_ty.0, ptr_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn store(&self, val: Value, ptr_val: Value) -> Value {
        Value(unsafe { core::LLVMBuildStore(self.builder, val.0, ptr_val.0) })
    }

    pub fn gep_inbounds(
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

    pub fn gep_struct(&mut self, ptr_ty: Type, ptr_val: Value, idx: u32, name: &str) -> Value {
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

    pub fn cast(&mut self, op: OpCode, val: Value, into_ty: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildCast(self.builder, op, val.0, into_ty.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn icmp(&mut self, op: IntPredicate, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildICmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn fcmp(&mut self, op: FloatPredicate, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildFCmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }

    pub fn call(&mut self, fn_ty: TypeFn, fn_val: ValueFn, args: &[Value], name: &str) -> Value {
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

impl ValueFn {
    pub fn entry_bb(&self) -> BasicBlock {
        BasicBlock(unsafe { core::LLVMGetEntryBasicBlock(self.0) })
    }
    pub fn param_val(&self, param_idx: u32) -> Option<Value> {
        if param_idx < unsafe { core::LLVMCountParams(self.0) } {
            Some(Value(unsafe { core::LLVMGetParam(self.0, param_idx) }))
        } else {
            None
        }
    }
}

pub fn const_all_zero(ty: Type) -> Value {
    Value(unsafe { core::LLVMConstNull(ty.0) })
}
pub fn const_all_ones(ty: Type) -> Value {
    Value(unsafe { core::LLVMConstAllOnes(ty.0) })
}
pub fn const_int(int_ty: Type, val: u64, sign_extend: bool) -> Value {
    Value(unsafe { core::LLVMConstInt(int_ty.0, val, sign_extend as i32) })
}
pub fn const_float(float_ty: Type, val: f64) -> Value {
    Value(unsafe { core::LLVMConstReal(float_ty.0, val) })
}
pub fn const_string(string: &str, dont_null_terminate: bool) -> Value {
    Value(unsafe {
        core::LLVMConstString(
            string.as_ptr() as *const c_char,
            string.len() as u32,
            dont_null_terminate as i32,
        )
    })
}
pub fn const_array(elem_ty: Type, const_vals: &[Value]) -> Value {
    Value(unsafe {
        core::LLVMConstArray2(
            elem_ty.0,
            const_vals.as_ptr() as *mut sys::LLVMValueRef,
            const_vals.len() as u64,
        )
    })
}
pub fn const_struct_inline(const_vals: &[Value], packed: bool) -> Value {
    Value(unsafe {
        core::LLVMConstStruct(
            const_vals.as_ptr() as *mut sys::LLVMValueRef,
            const_vals.len() as u32,
            packed as i32,
        )
    })
}
pub fn const_struct_named(struct_ty: Type, const_vals: &[Value]) -> Value {
    Value(unsafe {
        core::LLVMConstNamedStruct(
            struct_ty.0,
            const_vals.as_ptr() as *mut sys::LLVMValueRef,
            const_vals.len() as u32,
        )
    })
}

pub fn array_type(elem_ty: Type, len: u64) -> Type {
    Type(unsafe { core::LLVMArrayType2(elem_ty.0, len) })
}

pub fn function_type(return_ty: Type, param_types: &[Type], is_variadic: bool) -> TypeFn {
    TypeFn(unsafe {
        core::LLVMFunctionType(
            return_ty.0,
            param_types.as_ptr() as *mut sys::LLVMTypeRef,
            param_types.len() as u32,
            is_variadic as i32,
        )
    })
}

pub fn typeof_value(val: Value) -> Type {
    Type(unsafe { core::LLVMTypeOf(val.0) })
}

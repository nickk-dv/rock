//! `llvm` module provides safe api on top  
//! of `sys` bindings to LLVM-C api.  
//! It does not aim to fully safeguard  
//! against type and value mismatch errors.

use crate::sys;
use crate::sys::analysis;
use crate::sys::core;
use crate::sys::target;
use rock_core::config::{TargetArch, TargetTriple};
use rock_core::support::AsStr;
use std::cell::UnsafeCell;
use std::ffi::c_char;

pub struct IRTarget {
    target_data: target::LLVMTargetDataRef,
    target_machine: target::LLVMTargetMachineRef,
}

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

struct CStrBuffer(UnsafeCell<String>);

#[derive(Copy, Clone)]
pub struct BasicBlock(sys::LLVMBasicBlockRef);

#[derive(Copy, Clone)]
pub struct Value(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValuePtr(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueFn(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueGlobal(sys::LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueInstr(sys::LLVMValueRef);

#[derive(Copy, Clone)]
pub struct Type(sys::LLVMTypeRef);
#[derive(Copy, Clone)]
pub struct TypeFn(sys::LLVMTypeRef);
#[derive(Copy, Clone)]
pub struct TypeStruct(sys::LLVMTypeRef);

pub type OpCode = sys::LLVMOpcode;
pub type Linkage = sys::LLVMLinkage;
pub type CallConv = sys::LLVMCallConv;
pub type IntPred = sys::LLVMIntPredicate;
pub type FloatPred = sys::LLVMRealPredicate;

impl IRTarget {
    pub fn new(triple: TargetTriple) -> IRTarget {
        match triple.arch() {
            TargetArch::x86_64 => unsafe {
                target::LLVMInitializeX86AsmParser();
                target::LLVMInitializeX86AsmPrinter();
                target::LLVMInitializeX86Disassembler();
                target::LLVMInitializeX86Target();
                target::LLVMInitializeX86TargetInfo();
                target::LLVMInitializeX86TargetMC();
            },
            TargetArch::Arm_64 => unsafe {
                target::LLVMInitializeAArch64AsmParser();
                target::LLVMInitializeAArch64AsmPrinter();
                target::LLVMInitializeAArch64Disassembler();
                target::LLVMInitializeAArch64Target();
                target::LLVMInitializeAArch64TargetInfo();
                target::LLVMInitializeAArch64TargetMC();
            },
        }

        let ctriple = cstring_from_str(triple.as_str());
        let mut target = std::mem::MaybeUninit::uninit();
        let mut err_str = std::mem::MaybeUninit::uninit();
        let code = unsafe {
            target::LLVMGetTargetFromTriple(
                ctriple.as_ptr(),
                target.as_mut_ptr(),
                err_str.as_mut_ptr(),
            )
        };
        if code == 1 {
            let err_str = unsafe { err_str.assume_init() };
            panic!(
                "failed to create target from triple `{}`:\n{}",
                triple.as_str(),
                llvm_string_to_owned(err_str)
            );
        }
        let target = unsafe { target.assume_init() };
        assert!(!target.is_null());

        let target_machine = unsafe {
            target::LLVMCreateTargetMachine(
                target,
                ctriple.as_ptr(),
                cstring_empty(),
                cstring_empty(),
                target::LLVMCodeGenOptLevel::LLVMCodeGenLevelNone, //@base on options (debug / release)
                target::LLVMRelocMode::LLVMRelocDefault,
                target::LLVMCodeModel::LLVMCodeModelDefault,
            )
        };
        let target_data = unsafe { target::LLVMCreateTargetDataLayout(target_machine) };

        IRTarget {
            target_data,
            target_machine,
        }
    }

    pub fn ptr_sized_int(&self, context: &IRContext) -> Type {
        Type(unsafe { target::LLVMIntPtrTypeInContext(context.context, self.target_data) })
    }
}

impl Drop for IRTarget {
    fn drop(&mut self) {
        unsafe { target::LLVMDisposeTargetData(self.target_data) }
        unsafe { target::LLVMDisposeTargetMachine(self.target_machine) }
    }
}

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

    pub fn struct_create_named(&self, name: &str) -> TypeStruct {
        TypeStruct(unsafe { core::LLVMStructCreateNamed(self.context, self.cstr_buf.cstr(name)) })
    }
    pub fn struct_set_body(&self, struct_ty: TypeStruct, field_types: &[Type], packed: bool) {
        unsafe {
            core::LLVMStructSetBody(
                struct_ty.0,
                field_types.as_ptr() as *mut sys::LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        }
    }
    pub fn struct_type_inline(&self, field_types: &[Type], packed: bool) -> TypeStruct {
        TypeStruct(unsafe {
            core::LLVMStructTypeInContext(
                self.context,
                field_types.as_ptr() as *mut sys::LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        })
    }

    pub fn append_bb(&self, fn_val: ValueFn, name: &str) -> BasicBlock {
        BasicBlock(unsafe {
            core::LLVMAppendBasicBlockInContext(self.context, fn_val.0, self.cstr_buf.cstr(name))
        })
    }
}

impl IRModule {
    pub fn new(context: &IRContext, target: &IRTarget, name: &str) -> IRModule {
        let cstr_buf = CStrBuffer::new();
        let name = cstr_buf.cstr(name);
        let module = unsafe { core::LLVMModuleCreateWithNameInContext(name, context.context) };
        unsafe { target::LLVMSetModuleDataLayout(module, target.target_data) };
        IRModule { module, cstr_buf }
    }

    #[must_use]
    pub fn add_function(&self, name: &str, fn_ty: TypeFn, linkage: Linkage) -> ValueFn {
        let name = self.cstr_buf.cstr(name);
        let fn_val = unsafe { core::LLVMAddFunction(self.module, name, fn_ty.0) };

        unsafe { core::LLVMSetLinkage(fn_val, linkage) };
        ValueFn(fn_val)
    }

    #[must_use]
    pub fn add_global(
        &self,
        ty: Type,
        name: &str,
        const_val: Value,
        constant: bool,
        unnamed_addr: bool,
        thread_local: bool,
        linkage: Linkage,
    ) -> ValueGlobal {
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
        ValueGlobal(global_val)
    }

    pub fn to_string(&self) -> String {
        let llvm_str = unsafe { core::LLVMPrintModuleToString(self.module) };
        llvm_string_to_owned(llvm_str)
    }

    pub fn verify(&self) -> Result<(), String> {
        let mut err_str = std::mem::MaybeUninit::uninit();
        let action = analysis::LLVMVerifierFailureAction::LLVMReturnStatusAction;
        let code = unsafe { analysis::LLVMVerifyModule(self.module, action, err_str.as_mut_ptr()) };

        if code == 1 {
            let err_str = unsafe { err_str.assume_init() };
            Err(llvm_string_to_owned(err_str))
        } else {
            Ok(())
        }
    }

    pub fn emit_to_file(&self, target: &IRTarget, filename: &str) -> Result<(), String> {
        let mut err_str = std::mem::MaybeUninit::uninit();
        let codegen = target::LLVMCodeGenFileType::LLVMObjectFile;
        let code = unsafe {
            target::LLVMTargetMachineEmitToFile(
                target.target_machine,
                self.module,
                self.cstr_buf.cstr(filename) as *mut c_char,
                codegen,
                err_str.as_mut_ptr(),
            )
        };

        if code == 1 {
            let err_str = unsafe { err_str.assume_init() };
            Err(llvm_string_to_owned(err_str))
        } else {
            Ok(())
        }
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

    pub fn ret(&self, val: Option<Value>) {
        if let Some(val) = val {
            let _ = unsafe { core::LLVMBuildRet(self.builder, val.0) };
        } else {
            let _ = unsafe { core::LLVMBuildRetVoid(self.builder) };
        }
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

    pub fn bin_op(&self, op: OpCode, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildBinOp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn neg(&self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn fneg(&self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildFNeg(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }
    pub fn not(&self, val: Value, name: &str) -> Value {
        Value(unsafe { core::LLVMBuildNot(self.builder, val.0, self.cstr_buf.cstr(name)) })
    }

    pub fn alloca(&self, ty: Type, name: &str) -> ValuePtr {
        ValuePtr(unsafe { core::LLVMBuildAlloca(self.builder, ty.0, self.cstr_buf.cstr(name)) })
    }
    pub fn load(&self, ptr_ty: Type, ptr_val: ValuePtr, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildLoad2(self.builder, ptr_ty.0, ptr_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn store(&self, val: Value, ptr_val: ValuePtr) {
        unsafe { core::LLVMBuildStore(self.builder, val.0, ptr_val.0) };
    }

    pub fn gep(&self, ptr_ty: Type, ptr_val: ValuePtr, indices: &[Value], name: &str) -> ValuePtr {
        ValuePtr(unsafe {
            core::LLVMBuildGEP2(
                self.builder,
                ptr_ty.0,
                ptr_val.0,
                indices.as_ptr() as *mut sys::LLVMValueRef,
                indices.len() as u32,
                self.cstr_buf.cstr(name),
            )
        })
    }

    pub fn gep_inbounds(
        &self,
        ptr_ty: Type,
        ptr_val: ValuePtr,
        indices: &[Value],
        name: &str,
    ) -> ValuePtr {
        ValuePtr(unsafe {
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

    pub fn gep_struct(
        &self,
        ptr_ty: TypeStruct,
        ptr_val: ValuePtr,
        idx: u32,
        name: &str,
    ) -> ValuePtr {
        ValuePtr(unsafe {
            core::LLVMBuildStructGEP2(
                self.builder,
                ptr_ty.0,
                ptr_val.0,
                idx,
                self.cstr_buf.cstr(name),
            )
        })
    }

    pub fn cast(&self, op: OpCode, val: Value, into_ty: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildCast(self.builder, op, val.0, into_ty.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn icmp(&self, op: IntPred, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildICmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn fcmp(&self, op: FloatPred, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildFCmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }

    pub fn phi(&self, ty: Type, name: &str) -> Value {
        let name = self.cstr_buf.cstr(name);
        Value(unsafe { core::LLVMBuildPhi(self.builder, ty.0, name) })
    }
    pub fn phi_add_incoming(&self, phi: Value, values: &[Value], blocks: &[BasicBlock]) {
        let count = values.len() as u32;
        unsafe {
            let values_ptr: *const sys::LLVMValueRef = std::mem::transmute(values.as_ptr());
            let blocks_ptr: *const sys::LLVMBasicBlockRef = std::mem::transmute(blocks.as_ptr());
            core::LLVMAddIncoming(phi.0, values_ptr, blocks_ptr, count);
        }
    }

    pub fn call(
        &self,
        fn_ty: TypeFn,
        fn_val: ValueFn,
        args: &[Value],
        name: &str,
    ) -> Option<Value> {
        let return_ty = unsafe { core::LLVMGetReturnType(fn_ty.0) };
        let return_kind = unsafe { core::LLVMGetTypeKind(return_ty) };

        let name = match return_kind {
            sys::LLVMTypeKind::LLVMVoidTypeKind => cstring_empty(),
            _ => self.cstr_buf.cstr(name),
        };

        let call_value = Value(unsafe {
            core::LLVMBuildCall2(
                self.builder,
                fn_ty.0,
                fn_val.0,
                args.as_ptr() as *mut sys::LLVMValueRef,
                args.len() as u32,
                name,
            )
        });

        match return_kind {
            sys::LLVMTypeKind::LLVMVoidTypeKind => None,
            _ => Some(call_value),
        }
    }
}

impl Drop for IRBuilder {
    fn drop(&mut self) {
        unsafe { core::LLVMDisposeBuilder(self.builder) }
    }
}

impl CStrBuffer {
    fn new() -> CStrBuffer {
        CStrBuffer(UnsafeCell::new(String::with_capacity(64)))
    }
    fn cstr(&self, name: &str) -> *const c_char {
        let string = unsafe { &mut *self.0.get() };
        string.clear();
        string.push_str(name);
        string.push('\0');
        string.as_ptr() as *const c_char
    }
}

impl BasicBlock {
    pub fn terminator(&self) -> Option<ValueInstr> {
        ValueInstr::new_opt(unsafe { core::LLVMGetBasicBlockTerminator(self.0) })
    }
    pub fn first_instr(&self) -> Option<ValueInstr> {
        ValueInstr::new_opt(unsafe { core::LLVMGetFirstInstruction(self.0) })
    }
}

impl Value {
    pub fn into_ptr(self) -> ValuePtr {
        let ty = typeof_value(self);
        let ty_kind = unsafe { core::LLVMGetTypeKind(ty.0) };

        match ty_kind {
            sys::LLVMTypeKind::LLVMPointerTypeKind => ValuePtr(self.0),
            _ => unreachable!(), //@panics when trying to get ptr from constant var field
        }
    }
    pub fn into_fn(self) -> ValueFn {
        let ty = typeof_value(self);
        let ty_kind = unsafe { core::LLVMGetTypeKind(ty.0) };

        match ty_kind {
            sys::LLVMTypeKind::LLVMPointerTypeKind | sys::LLVMTypeKind::LLVMFunctionTypeKind => {
                ValueFn(self.0)
            }
            _ => unreachable!(),
        }
    }
}

impl ValuePtr {
    #[inline]
    pub fn as_val(self) -> Value {
        Value(self.0)
    }
}

impl ValueFn {
    #[inline]
    pub fn as_ptr(self) -> ValuePtr {
        ValuePtr(self.0)
    }
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
    pub fn set_call_conv(&self, cc: CallConv) {
        unsafe { core::LLVMSetFunctionCallConv(self.0, cc) };
    }
}

impl ValueGlobal {
    #[inline]
    pub fn as_ptr(self) -> ValuePtr {
        ValuePtr(self.0)
    }
}

impl ValueInstr {
    #[inline]
    fn new_opt(raw: sys::LLVMValueRef) -> Option<ValueInstr> {
        if raw.is_null() {
            None
        } else {
            Some(ValueInstr(raw))
        }
    }
}

impl TypeStruct {
    #[inline]
    pub fn as_ty(self) -> Type {
        Type(self.0)
    }
}

pub fn const_all_zero(ty: Type) -> Value {
    Value(unsafe { core::LLVMConstNull(ty.0) })
}
pub fn const_int(int_ty: Type, val: u64, sign_extend: bool) -> Value {
    Value(unsafe { core::LLVMConstInt(int_ty.0, val, sign_extend as i32) })
}
pub fn const_float(float_ty: Type, val: f64) -> Value {
    Value(unsafe { core::LLVMConstReal(float_ty.0, val) })
}

pub fn const_string(ctx: &IRContext, string: &str, null_terminate: bool) -> Value {
    Value(unsafe {
        core::LLVMConstStringInContext2(
            ctx.context,
            string.as_ptr() as *const c_char,
            string.len(),
            (!null_terminate) as i32,
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
pub fn const_struct_named(struct_ty: TypeStruct, const_vals: &[Value]) -> Value {
    Value(unsafe {
        core::LLVMConstNamedStruct(
            struct_ty.0,
            const_vals.as_ptr() as *mut sys::LLVMValueRef,
            const_vals.len() as u32,
        )
    })
}
pub fn const_bitcast(const_val: Value, into_ty: Type) -> Value {
    Value(unsafe { core::LLVMConstBitCast(const_val.0, into_ty.0) })
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

fn llvm_string_to_owned(llvm_str: *mut c_char) -> String {
    assert!(!llvm_str.is_null());
    let cstr = unsafe { std::ffi::CStr::from_ptr(llvm_str) };
    let string = cstr.to_string_lossy().into_owned();
    unsafe { core::LLVMDisposeMessage(llvm_str) };
    string
}

fn cstring_from_str(string: &str) -> std::ffi::CString {
    std::ffi::CString::new(string).unwrap()
}

fn cstring_empty() -> *const c_char {
    "\0".as_ptr() as *const c_char
}

use llvm_sys as sys;
use llvm_sys::analysis;
use llvm_sys::core;
use llvm_sys::prelude::*;
use llvm_sys::target as tg;
use llvm_sys::target_machine as tm;
use llvm_sys::transforms::pass_builder as pass;
use rock_core::session::config::{Build, TargetArch, TargetTriple};
use rock_core::support::AsStr;
use std::ffi::c_char;

pub struct IRContext {
    context: LLVMContextRef,
    cstr_buf: CStringBuffer,
}

pub struct IRTarget {
    target_data: tg::LLVMTargetDataRef,
    target_machine: tm::LLVMTargetMachineRef,
}

pub struct IRModule {
    module: LLVMModuleRef,
    cstr_buf: CStringBuffer,
}

pub struct IRBuilder {
    builder: LLVMBuilderRef,
    cstr_buf: CStringBuffer,
    void_val_type: TypeStruct,
}

struct CStringBuffer(String);
pub type OpCode = sys::LLVMOpcode;
pub type TypeKind = sys::LLVMTypeKind;
pub type Linkage = sys::LLVMLinkage;
pub type IntPred = sys::LLVMIntPredicate;
pub type FloatPred = sys::LLVMRealPredicate;
pub type Ordering = sys::LLVMAtomicOrdering;
pub type RMWBinOp = sys::LLVMAtomicRMWBinOp;

#[derive(Copy, Clone)]
pub struct Type(LLVMTypeRef);
#[derive(Copy, Clone)]
pub struct TypeFn(LLVMTypeRef);
#[derive(Copy, Clone)]
pub struct TypeStruct(LLVMTypeRef);

#[derive(Copy, Clone)]
pub struct Value(LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValuePtr(LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueFn(LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueGlobal(LLVMValueRef);
#[derive(Copy, Clone)]
pub struct ValueInstr(LLVMValueRef);

#[derive(Copy, Clone)]
pub struct Attribute(LLVMAttributeRef);
#[derive(Copy, Clone)]
pub struct BasicBlock(LLVMBasicBlockRef);

impl IRContext {
    pub fn new() -> IRContext {
        IRContext { context: unsafe { core::LLVMContextCreate() }, cstr_buf: CStringBuffer::new() }
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

    pub fn append_bb(&mut self, fn_val: ValueFn, name: &str) -> BasicBlock {
        BasicBlock(unsafe {
            core::LLVMAppendBasicBlockInContext(self.context, fn_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn attr_create(&mut self, name: &str) -> Attribute {
        unsafe {
            let id = core::LLVMGetEnumAttributeKindForName(name.as_ptr() as *const i8, name.len());
            Attribute(core::LLVMCreateEnumAttribute(self.context, id, 0))
        }
    }

    pub fn struct_named_create(&mut self, name: &str) -> TypeStruct {
        TypeStruct(unsafe { core::LLVMStructCreateNamed(self.context, self.cstr_buf.cstr(name)) })
    }
    pub fn struct_named_set_body(&self, struct_ty: TypeStruct, field_types: &[Type], packed: bool) {
        unsafe {
            core::LLVMStructSetBody(
                struct_ty.0,
                field_types.as_ptr() as *mut LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        }
    }
    pub fn struct_type_inline(&self, field_types: &[Type], packed: bool) -> TypeStruct {
        TypeStruct(unsafe {
            core::LLVMStructTypeInContext(
                self.context,
                field_types.as_ptr() as *mut LLVMTypeRef,
                field_types.len() as u32,
                packed as i32,
            )
        })
    }
}

impl IRTarget {
    pub fn new(triple: TargetTriple, build: Build) -> IRTarget {
        match triple.arch() {
            TargetArch::x86_64 => unsafe {
                tg::LLVMInitializeX86Target();
                tg::LLVMInitializeX86TargetMC();
                tg::LLVMInitializeX86TargetInfo();
                tg::LLVMInitializeX86AsmPrinter();
            },
            TargetArch::Arm_64 => unsafe {
                tg::LLVMInitializeAArch64Target();
                tg::LLVMInitializeAArch64TargetMC();
                tg::LLVMInitializeAArch64TargetInfo();
                tg::LLVMInitializeAArch64AsmPrinter();
            },
        }

        let ctriple = std::ffi::CString::new(triple.as_str()).unwrap();
        let mut target = std::mem::MaybeUninit::uninit();
        let mut err_str = std::mem::MaybeUninit::uninit();
        let code = unsafe {
            tm::LLVMGetTargetFromTriple(ctriple.as_ptr(), target.as_mut_ptr(), err_str.as_mut_ptr())
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

        let opt_level = match build {
            Build::Debug => tm::LLVMCodeGenOptLevel::LLVMCodeGenLevelNone,
            Build::Release => tm::LLVMCodeGenOptLevel::LLVMCodeGenLevelAggressive,
        };
        let target_machine = unsafe {
            tm::LLVMCreateTargetMachine(
                target,
                ctriple.as_ptr(),
                c"".as_ptr(),
                c"".as_ptr(),
                opt_level,
                tm::LLVMRelocMode::LLVMRelocDefault,
                tm::LLVMCodeModel::LLVMCodeModelDefault,
            )
        };
        let target_data = unsafe { tm::LLVMCreateTargetDataLayout(target_machine) };

        IRTarget { target_data, target_machine }
    }

    pub fn ptr_sized_int(&self, context: &IRContext) -> Type {
        Type(unsafe { tg::LLVMIntPtrTypeInContext(context.context, self.target_data) })
    }
}

impl Drop for IRTarget {
    fn drop(&mut self) {
        unsafe { tg::LLVMDisposeTargetData(self.target_data) }
        unsafe { tm::LLVMDisposeTargetMachine(self.target_machine) }
    }
}

impl IRModule {
    pub fn new(context: &IRContext, target: &IRTarget, name: &str) -> IRModule {
        let mut cstr_buf = CStringBuffer::new();
        let name = cstr_buf.cstr(name);
        let module = unsafe { core::LLVMModuleCreateWithNameInContext(name, context.context) };
        unsafe { tg::LLVMSetModuleDataLayout(module, target.target_data) };
        IRModule { module, cstr_buf }
    }

    #[must_use]
    pub fn add_function(&mut self, name: &str, fn_ty: TypeFn, linkage: Linkage) -> ValueFn {
        let name = self.cstr_buf.cstr(name);
        let fn_val = unsafe { core::LLVMAddFunction(self.module, name, fn_ty.0) };
        unsafe { core::LLVMSetLinkage(fn_val, linkage) }
        ValueFn(fn_val)
    }

    #[must_use]
    pub fn add_global(
        &mut self,
        name: &str,
        value: Option<Value>,
        global_ty: Type,
        constant: bool,
        unnamed_addr: bool,
    ) -> ValueGlobal {
        let name = self.cstr_buf.cstr(name);
        let global = unsafe { core::LLVMAddGlobal(self.module, global_ty.0, name) };
        let unnamed_addr = if unnamed_addr {
            sys::LLVMUnnamedAddr::LLVMGlobalUnnamedAddr
        } else {
            sys::LLVMUnnamedAddr::LLVMNoUnnamedAddr
        };
        unsafe {
            value.map(|v| core::LLVMSetInitializer(global, v.0));
            core::LLVMSetGlobalConstant(global, constant as i32);
            core::LLVMSetUnnamedAddress(global, unnamed_addr);
            core::LLVMSetLinkage(global, Linkage::LLVMInternalLinkage);
        }
        ValueGlobal(global)
    }

    pub fn init_global(&self, global: ValueGlobal, value: Value) {
        unsafe { core::LLVMSetInitializer(global.0, value.0) }
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

    pub fn run_optimization_passes(&mut self, target: &IRTarget, passes: &str) {
        unsafe {
            let options = pass::LLVMCreatePassBuilderOptions();
            let err = pass::LLVMRunPasses(
                self.module,
                self.cstr_buf.cstr(passes),
                target.target_machine,
                options,
            );
            pass::LLVMDisposePassBuilderOptions(options);
            //@error handle the LLVMErrorRef from LLVMRunPasses
            assert!(err.is_null(), "LLVMRunPasses() failed");
        }
    }

    pub fn emit_to_file(&mut self, target: &IRTarget, filename: &str) -> Result<(), String> {
        let mut err_str = std::mem::MaybeUninit::uninit();
        let codegen = tm::LLVMCodeGenFileType::LLVMObjectFile;
        let code = unsafe {
            tm::LLVMTargetMachineEmitToFile(
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
    pub fn new(context: &IRContext, void_val_type: TypeStruct) -> IRBuilder {
        IRBuilder {
            builder: unsafe { core::LLVMCreateBuilderInContext(context.context) },
            cstr_buf: CStringBuffer::new(),
            void_val_type,
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
    pub fn next_inst(&self, inst: ValueInstr) -> Option<ValueInstr> {
        ValueInstr::new_opt(unsafe { core::LLVMGetNextInstruction(inst.0) })
    }
    pub fn inst_block(&self, inst: ValueInstr) -> BasicBlock {
        BasicBlock(unsafe { core::LLVMGetInstructionParent(inst.0) })
    }

    pub fn ret(&self, val: Option<Value>) {
        if let Some(val) = val {
            if typeof_value(val).0 == self.void_val_type.0 {
                let _ = unsafe { core::LLVMBuildRetVoid(self.builder) };
            } else {
                let _ = unsafe { core::LLVMBuildRet(self.builder, val.0) };
            }
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

    pub fn alloca(&mut self, ty: Type, name: &str) -> ValuePtr {
        ValuePtr(unsafe { core::LLVMBuildAlloca(self.builder, ty.0, self.cstr_buf.cstr(name)) })
    }
    pub fn load(&mut self, ptr_ty: Type, ptr_val: ValuePtr, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildLoad2(self.builder, ptr_ty.0, ptr_val.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn store(&self, val: Value, ptr_val: ValuePtr) -> ValueInstr {
        unsafe { ValueInstr(core::LLVMBuildStore(self.builder, val.0, ptr_val.0)) }
    }

    pub fn gep_inbounds(
        &mut self,
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
                indices.as_ptr() as *mut LLVMValueRef,
                indices.len() as u32,
                self.cstr_buf.cstr(name),
            )
        })
    }

    pub fn gep_struct(
        &mut self,
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

    pub fn set_weak(&self, cmp_xchg_inst: Value, weak: bool) {
        unsafe {
            core::LLVMSetWeak(cmp_xchg_inst.0, weak as i32);
        }
    }
    pub fn set_ordering(&self, access_inst: ValueInstr, order: Ordering) {
        unsafe {
            core::LLVMSetOrdering(access_inst.0, order);
        }
    }

    pub fn ptr_to_int(&mut self, val: Value, into: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildPtrToInt(self.builder, val.0, into.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn int_to_ptr(&mut self, val: Value, into: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildIntToPtr(self.builder, val.0, into.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn bitcast(&mut self, val: Value, into: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildBitCast(self.builder, val.0, into.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn cast(&mut self, op: OpCode, val: Value, into: Type, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildCast(self.builder, op, val.0, into.0, self.cstr_buf.cstr(name))
        })
    }

    pub fn icmp(&mut self, op: IntPred, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildICmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }
    pub fn fcmp(&mut self, op: FloatPred, lhs: Value, rhs: Value, name: &str) -> Value {
        Value(unsafe {
            core::LLVMBuildFCmp(self.builder, op, lhs.0, rhs.0, self.cstr_buf.cstr(name))
        })
    }

    pub fn phi(&mut self, ty: Type, name: &str) -> Value {
        let name = self.cstr_buf.cstr(name);
        Value(unsafe { core::LLVMBuildPhi(self.builder, ty.0, name) })
    }
    pub fn phi_add_incoming(&self, phi: Value, values: &[Value], blocks: &[BasicBlock]) {
        let count = values.len() as u32;
        unsafe {
            core::LLVMAddIncoming(
                phi.0,
                values.as_ptr() as *mut LLVMValueRef,
                blocks.as_ptr() as *mut LLVMBasicBlockRef,
                count,
            );
        }
    }

    pub fn call(
        &mut self,
        fn_ty: TypeFn,
        fn_val: ValueFn,
        args: &[Value],
        name: &str,
    ) -> Option<Value> {
        let return_ty = unsafe { core::LLVMGetReturnType(fn_ty.0) };
        let return_kind = unsafe { core::LLVMGetTypeKind(return_ty) };

        let name = match return_kind {
            sys::LLVMTypeKind::LLVMVoidTypeKind => self.cstr_buf.cstr(""),
            _ => self.cstr_buf.cstr(name),
        };

        let call_value = Value(unsafe {
            core::LLVMBuildCall2(
                self.builder,
                fn_ty.0,
                fn_val.0,
                args.as_ptr() as *mut LLVMValueRef,
                args.len() as u32,
                name,
            )
        });

        match return_kind {
            sys::LLVMTypeKind::LLVMVoidTypeKind => None,
            _ => Some(call_value),
        }
    }

    pub fn extract_value(&mut self, agg_val: Value, index: u32, name: &str) -> Value {
        unsafe {
            Value(core::LLVMBuildExtractValue(
                self.builder,
                agg_val.0,
                index,
                self.cstr_buf.cstr(name),
            ))
        }
    }
    pub fn insert_value(&mut self, agg_val: Value, val: Value, index: u32, name: &str) -> Value {
        unsafe {
            Value(core::LLVMBuildInsertValue(
                self.builder,
                agg_val.0,
                val.0,
                index,
                self.cstr_buf.cstr(name),
            ))
        }
    }

    pub fn atomic_rmw(&self, op: RMWBinOp, ptr: ValuePtr, val: Value, order: Ordering) -> Value {
        unsafe { Value(core::LLVMBuildAtomicRMW(self.builder, op, ptr.0, val.0, order, 0)) }
    }
    pub fn atomic_cmp_xchg(
        &self,
        ptr: ValuePtr,
        cmp: Value,
        new: Value,
        success: Ordering,
        failure: Ordering,
    ) -> Value {
        unsafe {
            Value(core::LLVMBuildAtomicCmpXchg(
                self.builder,
                ptr.0,
                cmp.0,
                new.0,
                success,
                failure,
                0,
            ))
        }
    }

    pub fn memset(&self, target: ValuePtr, byte: Value, size: Value, align: u32) {
        unsafe {
            let _ = core::LLVMBuildMemSet(self.builder, target.0, byte.0, size.0, align);
        }
    }
}

impl Drop for IRBuilder {
    fn drop(&mut self) {
        unsafe { core::LLVMDisposeBuilder(self.builder) }
    }
}

impl CStringBuffer {
    fn new() -> CStringBuffer {
        CStringBuffer(String::with_capacity(256))
    }
    fn cstr(&mut self, name: &str) -> *const c_char {
        self.0.clear();
        self.0.push_str(name);
        self.0.push('\0');
        self.0.as_ptr() as *const c_char
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
    pub fn as_inst(self) -> ValueInstr {
        ValueInstr(self.0)
    }
    pub fn into_ptr(self) -> ValuePtr {
        let ty = typeof_value(self);
        let ty_kind = unsafe { core::LLVMGetTypeKind(ty.0) };

        match ty_kind {
            sys::LLVMTypeKind::LLVMPointerTypeKind => ValuePtr(self.0),
            _ => panic!("internal: `Value::into_ptr` type kind `{}` not a pointer", ty_kind as i32),
        }
    }
    pub fn into_fn(self) -> ValueFn {
        let ty = typeof_value(self);
        let ty_kind = unsafe { core::LLVMGetTypeKind(ty.0) };

        match ty_kind {
            sys::LLVMTypeKind::LLVMPointerTypeKind => ValueFn(self.0),
            _ => panic!("internal: `Value::into_fn` type kind `{}` not a pointer", ty_kind as i32),
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
    pub fn null() -> ValueFn {
        ValueFn(std::ptr::null_mut())
    }
    #[inline]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
    #[inline]
    pub fn as_val(self) -> Value {
        Value(self.0)
    }
    pub fn entry_bb(&self) -> BasicBlock {
        BasicBlock(unsafe { core::LLVMGetEntryBasicBlock(self.0) })
    }
    pub fn param_val(&self, param_idx: u32) -> Value {
        let param_count = unsafe { core::LLVMCountParams(self.0) };
        if param_idx < param_count {
            return Value(unsafe { core::LLVMGetParam(self.0, param_idx) });
        }
        panic!("internal: `ValueFn::param_val` out of bounds param `{param_idx} >= {param_count}`");
    }
    pub fn set_attr(self, attr: Attribute) {
        unsafe {
            core::LLVMAddAttributeAtIndex(self.0, sys::LLVMAttributeFunctionIndex, attr.0);
        }
    }
    pub fn set_param_attr(self, attr: Attribute, param_n: u32) {
        unsafe {
            core::LLVMAddAttributeAtIndex(self.0, param_n, attr.0);
        }
    }
}

impl ValueGlobal {
    #[inline]
    pub fn null() -> ValueGlobal {
        ValueGlobal(std::ptr::null_mut())
    }
    #[inline]
    pub fn as_ptr(self) -> ValuePtr {
        ValuePtr(self.0)
    }
    pub fn value_type(self) -> Type {
        unsafe {
            let ty = core::LLVMGlobalGetValueType(self.0);
            assert!(!ty.is_null());
            Type(ty)
        }
    }
}

impl ValueInstr {
    #[inline]
    fn new_opt(raw: LLVMValueRef) -> Option<ValueInstr> {
        if raw.is_null() {
            None
        } else {
            Some(ValueInstr(raw))
        }
    }
}

impl Type {
    #[inline]
    pub fn as_st(self) -> TypeStruct {
        TypeStruct(self.0)
    }
}

impl TypeFn {
    #[inline]
    pub fn null() -> TypeFn {
        TypeFn(std::ptr::null_mut())
    }
}

impl TypeStruct {
    #[inline]
    pub fn as_ty(self) -> Type {
        Type(self.0)
    }
    #[inline]
    pub fn field_ty(self, idx: u32) -> Type {
        let ty = unsafe { core::LLVMStructGetTypeAtIndex(self.0, idx) };
        assert!(!ty.is_null());
        Type(ty)
    }
}

pub fn const_zeroed(ty: Type) -> Value {
    Value(unsafe { core::LLVMConstNull(ty.0) })
}
pub fn undef(ty: Type) -> Value {
    Value(unsafe { core::LLVMGetUndef(ty.0) })
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
pub fn const_array(elem_ty: Type, values: &[Value]) -> Value {
    Value(unsafe {
        core::LLVMConstArray2(elem_ty.0, values.as_ptr() as *mut LLVMValueRef, values.len() as u64)
    })
}
pub fn const_struct_inline(ctx: &IRContext, values: &[Value], packed: bool) -> Value {
    Value(unsafe {
        core::LLVMConstStructInContext(
            ctx.context,
            values.as_ptr() as *mut LLVMValueRef,
            values.len() as u32,
            packed as i32,
        )
    })
}
pub fn const_struct_named(struct_ty: TypeStruct, values: &[Value]) -> Value {
    Value(unsafe {
        core::LLVMConstNamedStruct(
            struct_ty.0,
            values.as_ptr() as *mut LLVMValueRef,
            values.len() as u32,
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
            param_types.as_ptr() as *mut LLVMTypeRef,
            param_types.len() as u32,
            is_variadic as i32,
        )
    })
}

#[inline(always)]
pub fn typeof_value(val: Value) -> Type {
    Type(unsafe { core::LLVMTypeOf(val.0) })
}
#[inline(always)]
pub fn type_kind(ty: Type) -> TypeKind {
    unsafe { core::LLVMGetTypeKind(ty.0) }
}
#[inline(always)]
pub fn type_equals(a: Type, b: Type) -> bool {
    a.0 == b.0
}

fn llvm_string_to_owned(llvm_str: *mut c_char) -> String {
    assert!(!llvm_str.is_null());
    let cstr = unsafe { std::ffi::CStr::from_ptr(llvm_str) };
    let string = cstr.to_string_lossy().into_owned();
    unsafe { core::LLVMDisposeMessage(llvm_str) };
    string
}

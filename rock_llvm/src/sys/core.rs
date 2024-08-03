use super::*;
use std::ffi::{c_char, c_double, c_uint, c_ulonglong};

// core
extern "C" {
    pub fn LLVMShutdown();
}

// context
extern "C" {
    pub fn LLVMContextCreate() -> LLVMContextRef;
}

// module
extern "C" {
    pub fn LLVMModuleCreateWithNameInContext(
        name: *const c_char,
        ctx: LLVMContextRef,
    ) -> LLVMModuleRef;
    pub fn LLVMDisposeModule(m: LLVMModuleRef);

    pub fn LLVMAddFunction(
        m: LLVMModuleRef,
        name: *const c_char,
        fn_ty: LLVMTypeRef,
    ) -> LLVMValueRef;
}

// types
extern "C" {
    pub fn LLVMGetTypeKind(ty: LLVMTypeRef) -> LLVMTypeKind;

    pub fn LLVMInt1TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt8TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt16TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt32TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt64TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt128TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;

    pub fn LLVMHalfTypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMFloatTypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMDoubleTypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;

    pub fn LLVMVoidTypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMPointerTypeInContext(ctx: LLVMContextRef, addr_space: c_uint) -> LLVMTypeRef;
    pub fn LLVMArrayType2(elem_ty: LLVMTypeRef, len: u64) -> LLVMTypeRef;

    pub fn LLVMFunctionType(
        return_ty: LLVMTypeRef,
        param_types: *mut LLVMTypeRef,
        param_count: c_uint,
        is_variadic: LLVMBool,
    ) -> LLVMTypeRef;
    pub fn LLVMGetReturnType(fn_ty: LLVMTypeRef) -> LLVMTypeRef;

    pub fn LLVMStructCreateNamed(ctx: LLVMContextRef, name: *const c_char) -> LLVMTypeRef;
    pub fn LLVMStructSetBody(
        struct_ty: LLVMTypeRef,
        field_types: *mut LLVMTypeRef,
        field_count: c_uint,
        packed: LLVMBool,
    );
    pub fn LLVMStructTypeInContext(
        ctx: LLVMContextRef,
        field_types: *mut LLVMTypeRef,
        field_count: c_uint,
        packed: LLVMBool,
    ) -> LLVMTypeRef;
}

// value
extern "C" {
    pub fn LLVMTypeOf(Val: LLVMValueRef) -> LLVMTypeRef;

    pub fn LLVMConstNull(ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstAllOnes(ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstInt(
        int_ty: LLVMTypeRef,
        val: c_ulonglong,
        sign_extend: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstReal(real_ty: LLVMTypeRef, val: c_double) -> LLVMValueRef;

    pub fn LLVMConstString(
        str: *const c_char,
        len: c_uint,
        dont_null_terminate: LLVMBool,
    ) -> LLVMValueRef;

    pub fn LLVMConstArray2(
        elem_ty: LLVMTypeRef,
        const_vals: *mut LLVMValueRef,
        len: u64,
    ) -> LLVMValueRef;

    pub fn LLVMConstStruct(
        const_vals: *mut LLVMValueRef,
        count: c_uint,
        packed: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstNamedStruct(
        struct_ty: LLVMTypeRef,
        const_vals: *mut LLVMValueRef,
        count: c_uint,
    ) -> LLVMValueRef;

    pub fn LLVMSetLinkage(global_val: LLVMValueRef, linkage: LLVMLinkage);
    pub fn LLVMSetUnnamedAddress(global_val: LLVMValueRef, unnamed_addr: LLVMUnnamedAddr);

    pub fn LLVMAddGlobal(m: LLVMModuleRef, ty: LLVMTypeRef, name: *const c_char) -> LLVMValueRef;
    pub fn LLVMSetInitializer(global_val: LLVMValueRef, const_val: LLVMValueRef);
    pub fn LLVMSetThreadLocal(global_val: LLVMValueRef, thread_local: LLVMBool);
    pub fn LLVMSetGlobalConstant(global_val: LLVMValueRef, constant: LLVMBool);

    pub fn LLVMCountParams(fn_val: LLVMValueRef) -> c_uint;
    pub fn LLVMGetParam(fn_val: LLVMValueRef, idx: c_uint) -> LLVMValueRef;
}

// basic block
extern "C" {
    pub fn LLVMGetBasicBlockTerminator(bb: LLVMBasicBlockRef) -> LLVMValueRef;
    pub fn LLVMGetEntryBasicBlock(fn_val: LLVMValueRef) -> LLVMBasicBlockRef;
    pub fn LLVMGetFirstInstruction(bb: LLVMBasicBlockRef) -> LLVMValueRef;
    pub fn LLVMGetLastInstruction(bb: LLVMBasicBlockRef) -> LLVMValueRef;
    pub fn LLVMAppendBasicBlockInContext(
        ctx: LLVMContextRef,
        fn_val: LLVMValueRef,
        name: *const c_char,
    ) -> LLVMBasicBlockRef;
}

extern "C" {
    // builder
    pub fn LLVMCreateBuilderInContext(ctx: LLVMContextRef) -> LLVMBuilderRef;
    pub fn LLVMDisposeBuilder(b: LLVMBuilderRef);
    pub fn LLVMGetInsertBlock(b: LLVMBuilderRef) -> LLVMBasicBlockRef;
    pub fn LLVMPositionBuilderAtEnd(b: LLVMBuilderRef, bb: LLVMBasicBlockRef);
    pub fn LLVMPositionBuilderBefore(b: LLVMBuilderRef, instr: LLVMValueRef);
    pub fn LLVMPositionBuilder(b: LLVMBuilderRef, bb: LLVMBasicBlockRef, instr: LLVMValueRef);

    //terminators
    pub fn LLVMBuildRetVoid(b: LLVMBuilderRef) -> LLVMValueRef;
    pub fn LLVMBuildRet(b: LLVMBuilderRef, val: LLVMValueRef) -> LLVMValueRef;
    pub fn LLVMBuildBr(b: LLVMBuilderRef, dest_bb: LLVMBasicBlockRef) -> LLVMValueRef;
    pub fn LLVMBuildCondBr(
        b: LLVMBuilderRef,
        cond: LLVMValueRef,
        then_bb: LLVMBasicBlockRef,
        else_bb: LLVMBasicBlockRef,
    ) -> LLVMValueRef;
    pub fn LLVMBuildSwitch(
        b: LLVMBuilderRef,
        val: LLVMValueRef,
        else_bb: LLVMBasicBlockRef,
        case_count: c_uint,
    ) -> LLVMValueRef;
    pub fn LLVMAddCase(switch: LLVMValueRef, case_val: LLVMValueRef, dest_bb: LLVMBasicBlockRef);
    pub fn LLVMBuildUnreachable(b: LLVMBuilderRef) -> LLVMValueRef;

    //arithmetic
    pub fn LLVMBuildBinOp(
        b: LLVMBuilderRef,
        op: LLVMOpcode,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNeg(b: LLVMBuilderRef, val: LLVMValueRef, name: *const c_char) -> LLVMValueRef;
    pub fn LLVMBuildFNeg(b: LLVMBuilderRef, val: LLVMValueRef, name: *const c_char)
        -> LLVMValueRef;
    pub fn LLVMBuildNot(b: LLVMBuilderRef, val: LLVMValueRef, name: *const c_char) -> LLVMValueRef;

    //memory
    pub fn LLVMBuildAlloca(b: LLVMBuilderRef, ty: LLVMTypeRef, name: *const c_char)
        -> LLVMValueRef;
    pub fn LLVMBuildLoad2(
        b: LLVMBuilderRef,
        ptr_ty: LLVMTypeRef,
        ptr_val: LLVMValueRef,
        name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildStore(
        b: LLVMBuilderRef,
        val: LLVMValueRef,
        ptr_val: LLVMValueRef,
    ) -> LLVMValueRef;
    pub fn LLVMBuildInBoundsGEP2(
        b: LLVMBuilderRef,
        ptr_ty: LLVMTypeRef,
        ptr_val: LLVMValueRef,
        indices: *mut LLVMValueRef,
        indices_count: c_uint,
        name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildStructGEP2(
        b: LLVMBuilderRef,
        ptr_ty: LLVMTypeRef,
        ptr_val: LLVMValueRef,
        idx: c_uint,
        name: *const c_char,
    ) -> LLVMValueRef;

    //casts
    pub fn LLVMBuildCast(
        b: LLVMBuilderRef,
        op: LLVMOpcode,
        val: LLVMValueRef,
        into_ty: LLVMTypeRef,
        name: *const c_char,
    ) -> LLVMValueRef;

    // comparisons
    pub fn LLVMBuildICmp(
        b: LLVMBuilderRef,
        op: LLVMIntPredicate,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFCmp(
        b: LLVMBuilderRef,
        op: LLVMRealPredicate,
        lhs: LLVMValueRef,
        rhs: LLVMValueRef,
        name: *const c_char,
    ) -> LLVMValueRef;

    // miscellaneous
    pub fn LLVMBuildCall2(
        b: LLVMBuilderRef,
        fn_ty: LLVMTypeRef,
        fn_val: LLVMValueRef,
        args: *mut LLVMValueRef,
        arg_count: c_uint,
        name: *const c_char,
    ) -> LLVMValueRef;
}

use super::*;
use std::ffi::{c_char, c_double, c_uint, c_ulonglong};

// core
extern "C" {
    pub fn LLVMShutdown();
    pub fn LLVMGetVersion(Major: *mut c_uint, Minor: *mut c_uint, Patch: *mut c_uint);
    pub fn LLVMCreateMessage(Message: *const c_char) -> *mut c_char;
    pub fn LLVMDisposeMessage(Message: *mut c_char);
}

// context
extern "C" {
    pub fn LLVMContextCreate() -> LLVMContextRef;
    pub fn LLVMGetGlobalContext() -> LLVMContextRef;
}

// module
extern "C" {
    pub fn LLVMModuleCreateWithNameInContext(
        ModuleID: *const c_char,
        C: LLVMContextRef,
    ) -> LLVMModuleRef;
    pub fn LLVMDisposeModule(M: LLVMModuleRef);

    pub fn LLVMAddFunction(
        M: LLVMModuleRef,
        Name: *const c_char,
        FunctionTy: LLVMTypeRef,
    ) -> LLVMValueRef;
}

// types
extern "C" {
    pub fn LLVMInt1TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt8TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt16TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt32TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt64TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt128TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;

    pub fn LLVMHalfTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMFloatTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMDoubleTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;

    pub fn LLVMVoidTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMPointerTypeInContext(C: LLVMContextRef, AddressSpace: c_uint) -> LLVMTypeRef;
    pub fn LLVMArrayType2(ElementType: LLVMTypeRef, ElementCount: u64) -> LLVMTypeRef;

    pub fn LLVMFunctionType(
        ReturnType: LLVMTypeRef,
        ParamTypes: *mut LLVMTypeRef,
        ParamCount: c_uint,
        IsVarArg: LLVMBool,
    ) -> LLVMTypeRef;

    pub fn LLVMStructCreateNamed(C: LLVMContextRef, Name: *const c_char) -> LLVMTypeRef;
    pub fn LLVMStructSetBody(
        StructTy: LLVMTypeRef,
        ElementTypes: *mut LLVMTypeRef,
        ElementCount: c_uint,
        Packed: LLVMBool,
    );
    pub fn LLVMStructTypeInContext(
        C: LLVMContextRef,
        ElementTypes: *mut LLVMTypeRef,
        ElementCount: c_uint,
        Packed: LLVMBool,
    ) -> LLVMTypeRef;
}

// value
extern "C" {
    pub fn LLVMGetUndef(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstNull(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstAllOnes(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstInt(IntTy: LLVMTypeRef, N: c_ulonglong, SignExtend: LLVMBool) -> LLVMValueRef;
    pub fn LLVMConstReal(RealTy: LLVMTypeRef, N: c_double) -> LLVMValueRef;

    pub fn LLVMConstString(
        Str: *const c_char,
        Length: c_uint,
        DontNullTerminate: LLVMBool,
    ) -> LLVMValueRef;

    pub fn LLVMConstArray2(
        ElementTy: LLVMTypeRef,
        ConstantVals: *mut LLVMValueRef,
        Length: u64,
    ) -> LLVMValueRef;

    pub fn LLVMConstStruct(
        ConstantVals: *mut LLVMValueRef,
        Count: c_uint,
        Packed: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstNamedStruct(
        StructTy: LLVMTypeRef,
        ConstantVals: *mut LLVMValueRef,
        Count: c_uint,
    ) -> LLVMValueRef;

    pub fn LLVMAddGlobal(M: LLVMModuleRef, Ty: LLVMTypeRef, Name: *const c_char) -> LLVMValueRef;
    pub fn LLVMSetInitializer(GlobalVar: LLVMValueRef, ConstantVal: LLVMValueRef);
    pub fn LLVMSetThreadLocal(GlobalVar: LLVMValueRef, IsThreadLocal: LLVMBool);
}

// basic block
extern "C" {
    //@todo
}

// builder
extern "C" {
    //@todo
}

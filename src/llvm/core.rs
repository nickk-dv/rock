use super::types::*;
use std::ffi::*;

// Core
extern "C" {
    pub fn LLVMShutdown(); // Deallocate and destroy all ManagedStatic variables.
    pub fn LLVMGetVersion(Major: *mut c_uint, Minor: *mut c_uint, Patch: *mut c_uint);
    pub fn LLVMCreateMessage(Message: *const c_char) -> *mut c_char;
    pub fn LLVMDisposeMessage(Message: *mut c_char);
}

// Context
extern "C" {
    pub fn LLVMContextCreate() -> LLVMContextRef;
    pub fn LLVMGetGlobalContext() -> LLVMContextRef;
    pub fn LLVMContextDispose(C: LLVMContextRef);
}

// Module
extern "C" {
    pub fn LLVMModuleCreateWithName(ModuleID: *const c_char) -> LLVMModuleRef;
    pub fn LLVMModuleCreateWithNameInContext(
        ModuleID: *const c_char,
        C: LLVMContextRef,
    ) -> LLVMModuleRef;
    pub fn LLVMCloneModule(M: LLVMModuleRef) -> LLVMModuleRef;
    pub fn LLVMDisposeModule(M: LLVMModuleRef);
    pub fn LLVMGetModuleContext(M: LLVMModuleRef) -> LLVMContextRef;
    pub fn LLVMAddFunction(
        M: LLVMModuleRef,
        Name: *const c_char,
        FunctionTy: LLVMTypeRef,
    ) -> LLVMValueRef;
}

#[repr(C)]
pub enum LLVMTypeKind {
    LLVMVoidTypeKind,
    LLVMHalfTypeKind,
    LLVMFloatTypeKind,
    LLVMDoubleTypeKind,
    LLVMX86_FP80TypeKind,
    LLVMFP128TypeKind,
    LLVMPPC_FP128TypeKind,
    LLVMLabelTypeKind,
    LLVMIntegerTypeKind,
    LLVMFunctionTypeKind,
    LLVMStructTypeKind,
    LLVMArrayTypeKind,
    LLVMPointerTypeKind,
    LLVMVectorTypeKind,
    LLVMMetadataTypeKind,
    LLVMX86_MMXTypeKind,
    LLVMTokenTypeKind,
    LLVMScalableVectorTypeKind,
    LLVMBFloatTypeKind,
    LLVMX86_AMXTypeKind,
    LLVMTargetExtTypeKind,
}

// Types
extern "C" {
    // General
    pub fn LLVMGetTypeKind(Ty: LLVMTypeRef) -> LLVMTypeKind;
    pub fn LLVMTypeIsSized(Ty: LLVMTypeRef) -> LLVMBool;
    pub fn LLVMGetTypeContext(Ty: LLVMTypeRef) -> LLVMContextRef;
    pub fn LLVMDumpType(Val: LLVMTypeRef);
    pub fn LLVMPrintTypeToString(Val: LLVMTypeRef) -> *mut c_char;

    // Int
    pub fn LLVMInt1TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt8TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt16TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt32TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt64TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMInt128TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMIntTypeInContext(C: LLVMContextRef, NumBits: c_uint) -> LLVMTypeRef;
    pub fn LLVMInt1Type() -> LLVMTypeRef;
    pub fn LLVMInt8Type() -> LLVMTypeRef;
    pub fn LLVMInt16Type() -> LLVMTypeRef;
    pub fn LLVMInt32Type() -> LLVMTypeRef;
    pub fn LLVMInt64Type() -> LLVMTypeRef;
    pub fn LLVMInt128Type() -> LLVMTypeRef;
    pub fn LLVMIntType(NumBits: c_uint) -> LLVMTypeRef;
    pub fn LLVMGetIntTypeWidth(IntegerTy: LLVMTypeRef) -> c_uint;

    // Float
    pub fn LLVMHalfTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMBFloatTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMFloatTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMDoubleTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMX86FP80TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMFP128TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMPPCFP128TypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMHalfType() -> LLVMTypeRef;
    pub fn LLVMBFloatType() -> LLVMTypeRef;
    pub fn LLVMFloatType() -> LLVMTypeRef;
    pub fn LLVMDoubleType() -> LLVMTypeRef;
    pub fn LLVMX86FP80Type() -> LLVMTypeRef;
    pub fn LLVMFP128Type() -> LLVMTypeRef;
    pub fn LLVMPPCFP128Type() -> LLVMTypeRef;

    // Function
    pub fn LLVMFunctionType(
        ReturnType: LLVMTypeRef,
        ParamTypes: *mut LLVMTypeRef,
        ParamCount: c_uint,
        IsVarArg: LLVMBool,
    ) -> LLVMTypeRef;
    pub fn LLVMIsFunctionVarArg(FunctionTy: LLVMTypeRef) -> LLVMBool;
    pub fn LLVMGetReturnType(FunctionTy: LLVMTypeRef) -> LLVMTypeRef;
    pub fn LLVMCountParamTypes(FunctionTy: LLVMTypeRef) -> c_uint;
    pub fn LLVMGetParamTypes(FunctionTy: LLVMTypeRef, Dest: *mut LLVMTypeRef);

    // Struct
    pub fn LLVMStructTypeInContext(
        C: LLVMContextRef,
        ElementTypes: *mut LLVMTypeRef,
        ElementCount: c_uint,
        Packed: LLVMBool,
    ) -> LLVMTypeRef;
    pub fn LLVMStructType(
        ElementTypes: *mut LLVMTypeRef,
        ElementCount: c_uint,
        Packed: LLVMBool,
    ) -> LLVMTypeRef;
    pub fn LLVMStructCreateNamed(C: LLVMContextRef, Name: *const c_char) -> LLVMTypeRef;
    pub fn LLVMGetStructName(Ty: LLVMTypeRef) -> *const c_char;
    pub fn LLVMStructSetBody(
        StructTy: LLVMTypeRef,
        ElementTypes: *mut LLVMTypeRef,
        ElementCount: c_uint,
        Packed: LLVMBool,
    );
    pub fn LLVMCountStructElementTypes(StructTy: LLVMTypeRef) -> c_uint;
    pub fn LLVMGetStructElementTypes(StructTy: LLVMTypeRef, Dest: *mut LLVMTypeRef);
    pub fn LLVMStructGetTypeAtIndex(StructTy: LLVMTypeRef, i: c_uint) -> LLVMTypeRef;
    pub fn LLVMIsPackedStruct(StructTy: LLVMTypeRef) -> LLVMBool;
    pub fn LLVMIsOpaqueStruct(StructTy: LLVMTypeRef) -> LLVMBool;
    pub fn LLVMIsLiteralStruct(StructTy: LLVMTypeRef) -> LLVMBool;

    // Sequential
    pub fn LLVMGetElementType(Ty: LLVMTypeRef) -> LLVMTypeRef;
    pub fn LLVMGetSubtypes(Tp: LLVMTypeRef, Arr: *mut LLVMTypeRef);
    pub fn LLVMGetNumContainedTypes(Tp: LLVMTypeRef) -> c_uint;
    pub fn LLVMArrayType2(ElementType: LLVMTypeRef, ElementCount: u64) -> LLVMTypeRef;
    pub fn LLVMGetArrayLength2(ArrayTy: LLVMTypeRef) -> u64;
    pub fn LLVMPointerType(ElementType: LLVMTypeRef, AddressSpace: c_uint) -> LLVMTypeRef;
    pub fn LLVMPointerTypeIsOpaque(Ty: LLVMTypeRef) -> LLVMBool;
    pub fn LLVMPointerTypeInContext(C: LLVMContextRef, AddressSpace: c_uint) -> LLVMTypeRef; // Create an opaque pointer type in a context.
    pub fn LLVMGetPointerAddressSpace(PointerTy: LLVMTypeRef) -> c_uint;
    pub fn LLVMVectorType(ElementType: LLVMTypeRef, ElementCount: c_uint) -> LLVMTypeRef;
    pub fn LLVMScalableVectorType(ElementType: LLVMTypeRef, ElementCount: c_uint) -> LLVMTypeRef;
    pub fn LLVMGetVectorSize(VectorTy: LLVMTypeRef) -> c_uint;

    // Other
    pub fn LLVMVoidTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMLabelTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMX86MMXTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMX86AMXTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMTokenTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMMetadataTypeInContext(C: LLVMContextRef) -> LLVMTypeRef;
    pub fn LLVMVoidType() -> LLVMTypeRef;
    pub fn LLVMLabelType() -> LLVMTypeRef;
    pub fn LLVMX86MMXType() -> LLVMTypeRef;
    pub fn LLVMX86AMXType() -> LLVMTypeRef;
    pub fn LLVMTargetExtTypeInContext(
        C: LLVMContextRef,
        Name: *const c_char,
        TypeParams: *mut LLVMTypeRef,
        TypeParamCount: c_uint,
        IntParams: *mut c_uint,
        IntParamCount: c_uint,
    ) -> LLVMTypeRef;
}

#[repr(C)]
pub enum LLVMValueKind {
    LLVMArgumentValueKind,
    LLVMBasicBlockValueKind,
    LLVMMemoryUseValueKind,
    LLVMMemoryDefValueKind,
    LLVMMemoryPhiValueKind,
    LLVMFunctionValueKind,
    LLVMGlobalAliasValueKind,
    LLVMGlobalIFuncValueKind,
    LLVMGlobalVariableValueKind,
    LLVMBlockAddressValueKind,
    LLVMConstantExprValueKind,
    LLVMConstantArrayValueKind,
    LLVMConstantStructValueKind,
    LLVMConstantVectorValueKind,
    LLVMUndefValueValueKind,
    LLVMConstantAggregateZeroValueKind,
    LLVMConstantDataArrayValueKind,
    LLVMConstantDataVectorValueKind,
    LLVMConstantIntValueKind,
    LLVMConstantFPValueKind,
    LLVMConstantPointerNullValueKind,
    LLVMConstantTokenNoneValueKind,
    LLVMMetadataAsValueValueKind,
    LLVMInlineAsmValueKind,
    LLVMInstructionValueKind,
    LLVMPoisonValueKind,
    LLVMConstantTargetNoneValueKind,
}

// Values
extern "C" {
    // General
    pub fn LLVMTypeOf(Val: LLVMValueRef) -> LLVMTypeRef;
    pub fn LLVMGetValueKind(Val: LLVMValueRef) -> LLVMValueKind;
    pub fn LLVMGetValueName2(Val: LLVMValueRef, Length: *mut usize) -> *const c_char;
    pub fn LLVMSetValueName2(Val: LLVMValueRef, Name: *const c_char, NameLen: usize);
    pub fn LLVMDumpValue(Val: LLVMValueRef);
    pub fn LLVMPrintValueToString(Val: LLVMValueRef) -> *mut c_char;
    pub fn LLVMReplaceAllUsesWith(OldVal: LLVMValueRef, NewVal: LLVMValueRef);
    pub fn LLVMIsConstant(Val: LLVMValueRef) -> LLVMBool;
    pub fn LLVMIsUndef(Val: LLVMValueRef) -> LLVMBool;
    pub fn LLVMIsPoison(Val: LLVMValueRef) -> LLVMBool;
    pub fn LLVMIsAMDNode(Val: LLVMValueRef) -> LLVMValueRef;
    pub fn LLVMIsAValueAsMetadata(Val: LLVMValueRef) -> LLVMValueRef;
    pub fn LLVMIsAMDString(Val: LLVMValueRef) -> LLVMValueRef;

    // Usage
    pub fn LLVMGetFirstUse(Val: LLVMValueRef) -> LLVMUseRef;
    pub fn LLVMGetNextUse(U: LLVMUseRef) -> LLVMUseRef;
    pub fn LLVMGetUser(U: LLVMUseRef) -> LLVMValueRef;
    pub fn LLVMGetUsedValue(U: LLVMUseRef) -> LLVMValueRef;

    // User value
    pub fn LLVMGetOperand(Val: LLVMValueRef, Index: c_uint) -> LLVMValueRef;
    pub fn LLVMGetOperandUse(Val: LLVMValueRef, Index: c_uint) -> LLVMUseRef;
    pub fn LLVMSetOperand(User: LLVMValueRef, Index: c_uint, Val: LLVMValueRef);
    pub fn LLVMGetNumOperands(Val: LLVMValueRef) -> c_int;

    // Constant
    pub fn LLVMConstNull(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMConstAllOnes(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMGetUndef(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMGetPoison(Ty: LLVMTypeRef) -> LLVMValueRef;
    pub fn LLVMIsNull(Val: LLVMValueRef) -> LLVMBool;
    pub fn LLVMConstPointerNull(Ty: LLVMTypeRef) -> LLVMValueRef;

    // Constant scalar
    pub fn LLVMConstInt(IntTy: LLVMTypeRef, N: c_ulonglong, SignExtend: LLVMBool) -> LLVMValueRef;
    pub fn LLVMConstReal(RealTy: LLVMTypeRef, N: c_double) -> LLVMValueRef;

    // Constant composite
    pub fn LLVMConstStringInContext(
        C: LLVMContextRef,
        Str: *const c_char,
        Length: c_uint,
        DontNullTerminate: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstString(
        Str: *const c_char,
        Length: c_uint,
        DontNullTerminate: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstNamedStruct(
        StructTy: LLVMTypeRef,
        ConstantVals: *mut LLVMValueRef,
        Count: c_uint,
    ) -> LLVMValueRef;
    pub fn LLVMConstStructInContext(
        C: LLVMContextRef,
        ConstantVals: *mut LLVMValueRef,
        Count: c_uint,
        Packed: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMConstStruct(
        ConstantVals: *mut LLVMValueRef,
        Count: c_uint,
        Packed: LLVMBool,
    ) -> LLVMValueRef;
    pub fn LLVMGetAggregateElement(C: LLVMValueRef, idx: c_uint) -> LLVMValueRef;
    pub fn LLVMConstVector(ScalarConstantVals: *mut LLVMValueRef, Size: c_uint) -> LLVMValueRef;
}

// Basic Block
extern "C" {}

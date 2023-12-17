use super::types::*;
use std::ffi::*;

// Core
extern "C" {
    pub fn LLVMShutdown(); // Deallocate and destroy all ManagedStatic variables.
    pub fn LLVMIsMultithreaded() -> LLVMBool;
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
extern "C" {
    pub fn LLVMAppendBasicBlockInContext(
        C: LLVMContextRef,
        Fn: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMBasicBlockRef;
    pub fn LLVMAppendBasicBlock(Fn: LLVMValueRef, Name: *const c_char) -> LLVMBasicBlockRef;
    pub fn LLVMInsertBasicBlockInContext(
        C: LLVMContextRef,
        BB: LLVMBasicBlockRef,
        Name: *const c_char,
    ) -> LLVMBasicBlockRef;
    pub fn LLVMInsertBasicBlock(
        InsertBeforeBB: LLVMBasicBlockRef,
        Name: *const c_char,
    ) -> LLVMBasicBlockRef;
}

// Builder
extern "C" {
    pub fn LLVMCreateBuilderInContext(C: LLVMContextRef) -> LLVMBuilderRef;
    pub fn LLVMCreateBuilder() -> LLVMBuilderRef;
    pub fn LLVMDisposeBuilder(Builder: LLVMBuilderRef);
    pub fn LLVMPositionBuilderAtEnd(Builder: LLVMBuilderRef, Block: LLVMBasicBlockRef);
    pub fn LLVMGetInsertBlock(Builder: LLVMBuilderRef) -> LLVMBasicBlockRef;

    // Terminators
    pub fn LLVMBuildRet(arg1: LLVMBuilderRef, V: LLVMValueRef) -> LLVMValueRef;
    pub fn LLVMBuildRetVoid(arg1: LLVMBuilderRef) -> LLVMValueRef;
    pub fn LLVMBuildAggregateRet(
        arg1: LLVMBuilderRef,
        RetVals: *mut LLVMValueRef,
        N: c_uint,
    ) -> LLVMValueRef;
    pub fn LLVMBuildBr(arg1: LLVMBuilderRef, Dest: LLVMBasicBlockRef) -> LLVMValueRef;
    pub fn LLVMBuildCondBr(
        arg1: LLVMBuilderRef,
        If: LLVMValueRef,
        Then: LLVMBasicBlockRef,
        Else: LLVMBasicBlockRef,
    ) -> LLVMValueRef;
    pub fn LLVMBuildSwitch(
        arg1: LLVMBuilderRef,
        V: LLVMValueRef,
        Else: LLVMBasicBlockRef,
        NumCases: c_uint,
    ) -> LLVMValueRef;
    pub fn LLVMBuildUnreachable(B: LLVMBuilderRef) -> LLVMValueRef;
    pub fn LLVMAddCase(Switch: LLVMValueRef, OnVal: LLVMValueRef, Dest: LLVMBasicBlockRef);

    // Arithmetic
    pub fn LLVMBuildAdd(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNSWAdd(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNUWAdd(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFAdd(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildSub(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNSWSub(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNUWSub(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFSub(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildMul(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNSWMul(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNUWMul(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFMul(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildUDiv(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildExactUDiv(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildSDiv(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildExactSDiv(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFDiv(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildURem(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildSRem(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFRem(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildShl(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildLShr(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildAShr(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildAnd(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildOr(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildXor(
        arg1: LLVMBuilderRef,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildBinOp(
        B: LLVMBuilderRef,
        Op: LLVMOpcode,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNeg(arg1: LLVMBuilderRef, V: LLVMValueRef, Name: *const c_char)
        -> LLVMValueRef;
    pub fn LLVMBuildNSWNeg(B: LLVMBuilderRef, V: LLVMValueRef, Name: *const c_char)
        -> LLVMValueRef;
    pub fn LLVMBuildNUWNeg(B: LLVMBuilderRef, V: LLVMValueRef, Name: *const c_char)
        -> LLVMValueRef;
    pub fn LLVMBuildFNeg(
        arg1: LLVMBuilderRef,
        V: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildNot(arg1: LLVMBuilderRef, V: LLVMValueRef, Name: *const c_char)
        -> LLVMValueRef;

    // Casts
    pub fn LLVMBuildCast(
        B: LLVMBuilderRef,
        Op: LLVMOpcode,
        Val: LLVMValueRef,
        DestTy: LLVMTypeRef,
        Name: *const c_char,
    ) -> LLVMValueRef;

    // Comparisons
    pub fn LLVMBuildICmp(
        arg1: LLVMBuilderRef,
        Op: LLVMIntPredicate,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildFCmp(
        arg1: LLVMBuilderRef,
        Op: LLVMRealPredicate,
        LHS: LLVMValueRef,
        RHS: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;

    // Memory
    pub fn LLVMBuildAlloca(
        arg1: LLVMBuilderRef,
        Ty: LLVMTypeRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildLoad2(
        arg1: LLVMBuilderRef,
        Ty: LLVMTypeRef,
        PointerVal: LLVMValueRef,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildStore(
        arg1: LLVMBuilderRef,
        Val: LLVMValueRef,
        Ptr: LLVMValueRef,
    ) -> LLVMValueRef;
    pub fn LLVMBuildGEP2(
        B: LLVMBuilderRef,
        Ty: LLVMTypeRef,
        Pointer: LLVMValueRef,
        Indices: *mut LLVMValueRef,
        NumIndices: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildInBoundsGEP2(
        B: LLVMBuilderRef,
        Ty: LLVMTypeRef,
        Pointer: LLVMValueRef,
        Indices: *mut LLVMValueRef,
        NumIndices: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;
    pub fn LLVMBuildStructGEP2(
        B: LLVMBuilderRef,
        Ty: LLVMTypeRef,
        Pointer: LLVMValueRef,
        Idx: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;

    // Other
    pub fn LLVMBuildCall2(
        B: LLVMBuilderRef,
        FnTy: LLVMTypeRef,
        Fn: LLVMValueRef,
        Args: *mut LLVMValueRef,
        NumArgs: c_uint,
        Name: *const c_char,
    ) -> LLVMValueRef;
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMOpcode {
    /* Terminator Instructions */
    LLVMRet = 1,
    LLVMBr = 2,
    LLVMSwitch = 3,
    LLVMIndirectBr = 4,
    LLVMInvoke = 5,
    LLVMUnreachable = 7,
    LLVMCallBr = 67,

    /* Standard Unary Operators */
    LLVMFNeg = 66,

    /* Standard Binary Operators */
    LLVMAdd = 8,
    LLVMFAdd = 9,
    LLVMSub = 10,
    LLVMFSub = 11,
    LLVMMul = 12,
    LLVMFMul = 13,
    LLVMUDiv = 14,
    LLVMSDiv = 15,
    LLVMFDiv = 16,
    LLVMURem = 17,
    LLVMSRem = 18,
    LLVMFRem = 19,

    /* Logical Operators */
    LLVMShl = 20,
    LLVMLShr = 21,
    LLVMAShr = 22,
    LLVMAnd = 23,
    LLVMOr = 24,
    LLVMXor = 25,

    /* Memory Operators */
    LLVMAlloca = 26,
    LLVMLoad = 27,
    LLVMStore = 28,
    LLVMGetElementPtr = 29,

    /* Cast Operators */
    LLVMTrunc = 30,
    LLVMZExt = 31,
    LLVMSExt = 32,
    LLVMFPToUI = 33,
    LLVMFPToSI = 34,
    LLVMUIToFP = 35,
    LLVMSIToFP = 36,
    LLVMFPTrunc = 37,
    LLVMFPExt = 38,
    LLVMPtrToInt = 39,
    LLVMIntToPtr = 40,
    LLVMBitCast = 41,
    LLVMAddrSpaceCast = 60,

    /* Other Operators */
    LLVMICmp = 42,
    LLVMFCmp = 43,
    LLVMPHI = 44,
    LLVMCall = 45,
    LLVMSelect = 46,
    LLVMUserOp1 = 47,
    LLVMUserOp2 = 48,
    LLVMVAArg = 49,
    LLVMExtractElement = 50,
    LLVMInsertElement = 51,
    LLVMShuffleVector = 52,
    LLVMExtractValue = 53,
    LLVMInsertValue = 54,
    LLVMFreeze = 68,

    /* Atomic operators */
    LLVMFence = 55,
    LLVMAtomicCmpXchg = 56,
    LLVMAtomicRMW = 57,

    /* Exception Handling Operators */
    LLVMResume = 58,
    LLVMLandingPad = 59,
    LLVMCleanupRet = 61,
    LLVMCatchRet = 62,
    LLVMCatchPad = 63,
    LLVMCleanupPad = 64,
    LLVMCatchSwitch = 65,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
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

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
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

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMIntPredicate {
    LLVMIntEQ = 32, // equal
    LLVMIntNE,      // not equal
    LLVMIntUGT,     // unsigned greater than
    LLVMIntUGE,     // unsigned greater or equal
    LLVMIntULT,     // unsigned less than
    LLVMIntULE,     // unsigned less or equal
    LLVMIntSGT,     // signed greater than
    LLVMIntSGE,     // signed greater or equal
    LLVMIntSLT,     // signed less than
    LLVMIntSLE,     // signed less or equal
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMRealPredicate {
    LLVMRealPredicateFalse, // Always false
    LLVMRealOEQ,            // True if ordered and equal
    LLVMRealOGT,            // True if ordered and greater than
    LLVMRealOGE,            // True if ordered and greater than or equal
    LLVMRealOLT,            // True if ordered and less than
    LLVMRealOLE,            // True if ordered and less than or equal
    LLVMRealONE,            // True if ordered and operands are unequal
    LLVMRealORD,            // True if ordered (no nans)
    LLVMRealUNO,            // True if unordered: isnan(X) | isnan(Y)
    LLVMRealUEQ,            // True if unordered or equal
    LLVMRealUGT,            // True if unordered or greater than
    LLVMRealUGE,            // True if unordered, greater than, or equal
    LLVMRealULT,            // True if unordered or less than
    LLVMRealULE,            // True if unordered, less than, or equal
    LLVMRealUNE,            // True if unordered or not equal
    LLVMRealPredicateTrue,  // Always true
}

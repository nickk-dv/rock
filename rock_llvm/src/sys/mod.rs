pub mod core;

pub type LLVMBool = std::ffi::c_int;
pub type LLVMContextRef = *mut LLVMContext;
pub type LLVMModuleRef = *mut LLVMModule;
pub type LLVMTypeRef = *mut LLVMType;
pub type LLVMValueRef = *mut LLVMValue;
pub type LLVMBasicBlockRef = *mut LLVMBasicBlock;
pub type LLVMBuilderRef = *mut LLVMBuilder;

pub enum LLVMContext {}
pub enum LLVMModule {}
pub enum LLVMType {}
pub enum LLVMValue {}
pub enum LLVMBasicBlock {}
pub enum LLVMBuilder {}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum LLVMOpcode {
    // terminator
    LLVMRet = 1,
    LLVMBr = 2,
    LLVMSwitch = 3,
    LLVMIndirectBr = 4,
    LLVMInvoke = 5,
    LLVMUnreachable = 7,
    LLVMCallBr = 67,
    // unary operators
    LLVMFNeg = 66,
    // binary Operators
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
    // logical operators
    LLVMShl = 20,
    LLVMLShr = 21,
    LLVMAShr = 22,
    LLVMAnd = 23,
    LLVMOr = 24,
    LLVMXor = 25,
    // memory operators
    LLVMAlloca = 26,
    LLVMLoad = 27,
    LLVMStore = 28,
    LLVMGetElementPtr = 29,
    // cast operators
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
    // other operators
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
    // atomic operators
    LLVMFence = 55,
    LLVMAtomicCmpXchg = 56,
    LLVMAtomicRMW = 57,
    // exception handling operators
    LLVMResume = 58,
    LLVMLandingPad = 59,
    LLVMCleanupRet = 61,
    LLVMCatchRet = 62,
    LLVMCatchPad = 63,
    LLVMCleanupPad = 64,
    LLVMCatchSwitch = 65,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum LLVMTypeKind {
    LLVMVoidTypeKind = 0,
    LLVMHalfTypeKind = 1,
    LLVMFloatTypeKind = 2,
    LLVMDoubleTypeKind = 3,
    LLVMX86_FP80TypeKind = 4,
    LLVMFP128TypeKind = 5,
    LLVMPPC_FP128TypeKind = 6,
    LLVMLabelTypeKind = 7,
    LLVMIntegerTypeKind = 8,
    LLVMFunctionTypeKind = 9,
    LLVMStructTypeKind = 10,
    LLVMArrayTypeKind = 11,
    LLVMPointerTypeKind = 12,
    LLVMVectorTypeKind = 13,
    LLVMMetadataTypeKind = 14,
    LLVMX86_MMXTypeKind = 15,
    LLVMTokenTypeKind = 16,
    LLVMScalableVectorTypeKind = 17,
    LLVMBFloatTypeKind = 18,
    LLVMX86_AMXTypeKind = 19,
    LLVMTargetExtTypeKind = 20,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum LLVMLinkage {
    LLVMExternalLinkage = 0,
    LLVMAvailableExternallyLinkage = 1,
    LLVMLinkOnceAnyLinkage = 2,
    LLVMLinkOnceODRLinkage = 3,
    LLVMLinkOnceODRAutoHideLinkage = 4,
    LLVMWeakAnyLinkage = 5,
    LLVMWeakODRLinkage = 6,
    LLVMAppendingLinkage = 7,
    LLVMInternalLinkage = 8,
    LLVMPrivateLinkage = 9,
    LLVMDLLImportLinkage = 10,
    LLVMDLLExportLinkage = 11,
    LLVMExternalWeakLinkage = 12,
    LLVMGhostLinkage = 13,
    LLVMCommonLinkage = 14,
    LLVMLinkerPrivateLinkage = 15,
    LLVMLinkerPrivateWeakLinkage = 16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum LLVMUnnamedAddr {
    LLVMNoUnnamedAddr,     // address of the global is significant
    LLVMLocalUnnamedAddr,  // address of the global is locally insignificant.
    LLVMGlobalUnnamedAddr, // address of the global is globally insignificant.
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum LLVMIntPredicate {
    LLVMIntEQ = 32,  // equal
    LLVMIntNE = 33,  // not equal
    LLVMIntUGT = 34, // unsigned greater than
    LLVMIntUGE = 35, // unsigned greater or equal
    LLVMIntULT = 36, // unsigned less than
    LLVMIntULE = 37, // unsigned less or equal
    LLVMIntSGT = 38, // signed greater than
    LLVMIntSGE = 39, // signed greater or equal
    LLVMIntSLT = 40, // signed less than
    LLVMIntSLE = 41, // signed less or equal
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum LLVMRealPredicate {
    LLVMRealFalse = 0, // always false
    LLVMRealOEQ = 1,   // true if ordered and equal
    LLVMRealOGT = 2,   // true if ordered and greater than
    LLVMRealOGE = 3,   // true if ordered and greater than or equal
    LLVMRealOLT = 4,   // true if ordered and less than
    LLVMRealOLE = 5,   // true if ordered and less than or equal
    LLVMRealONE = 6,   // true if ordered and operands are unequal
    LLVMRealORD = 7,   // true if ordered (no nans)
    LLVMRealUNO = 8,   // true if unordered: isnan(X) | isnan(Y)
    LLVMRealUEQ = 9,   // true if unordered or equal
    LLVMRealUGT = 10,  // true if unordered or greater than
    LLVMRealUGE = 11,  // true if unordered, greater than, or equal
    LLVMRealULT = 12,  // true if unordered or less than
    LLVMRealULE = 13,  // true if unordered, less than, or equal
    LLVMRealUNE = 14,  // true if unordered or not equal
    LLVMRealTrue = 15, // always true
}

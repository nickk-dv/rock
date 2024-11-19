//! `sys` module provides bindings to LLVM-C api.  
//! Bindings are based on `llvm_sys 181.0.0` and  
//! designed to work with `LLVM 18.1.8` release.

pub type LLVMContextRef = *mut LLVMContext;
pub type LLVMModuleRef = *mut LLVMModule;
pub type LLVMBuilderRef = *mut LLVMBuilder;
pub type LLVMTypeRef = *mut LLVMType;
pub type LLVMValueRef = *mut LLVMValue;
pub type LLVMBasicBlockRef = *mut LLVMBasicBlock;
pub type LLVMBool = std::ffi::c_int;

pub enum LLVMContext {}
pub enum LLVMModule {}
pub enum LLVMBuilder {}
pub enum LLVMType {}
pub enum LLVMValue {}
pub enum LLVMBasicBlock {}

#[repr(C)]
#[allow(unused)]
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
#[allow(unused)]
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
#[allow(unused)]
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
#[allow(unused)]
#[derive(Copy, Clone)]
pub enum LLVMUnnamedAddr {
    LLVMNoUnnamedAddr,     // address of the global is significant
    LLVMLocalUnnamedAddr,  // address of the global is locally insignificant.
    LLVMGlobalUnnamedAddr, // address of the global is globally insignificant.
}

#[repr(C)]
#[allow(unused)]
#[derive(Copy, Clone)]
pub enum LLVMCallConv {
    LLVMCCallConv = 0,
    LLVMFastCallConv = 8,
    LLVMColdCallConv = 9,
    LLVMGHCCallConv = 10,
    LLVMHiPECallConv = 11,
    LLVMAnyRegCallConv = 13,
    LLVMPreserveMostCallConv = 14,
    LLVMPreserveAllCallConv = 15,
    LLVMSwiftCallConv = 16,
    LLVMCXXFASTTLSCallConv = 17,
    LLVMX86StdcallCallConv = 64,
    LLVMX86FastcallCallConv = 65,
    LLVMARMAPCSCallConv = 66,
    LLVMARMAAPCSCallConv = 67,
    LLVMARMAAPCSVFPCallConv = 68,
    LLVMMSP430INTRCallConv = 69,
    LLVMX86ThisCallCallConv = 70,
    LLVMPTXKernelCallConv = 71,
    LLVMPTXDeviceCallConv = 72,
    LLVMSPIRFUNCCallConv = 75,
    LLVMSPIRKERNELCallConv = 76,
    LLVMIntelOCLBICallConv = 77,
    LLVMX8664SysVCallConv = 78,
    LLVMWin64CallConv = 79,
    LLVMX86VectorCallCallConv = 80,
    LLVMHHVMCallConv = 81,
    LLVMHHVMCCallConv = 82,
    LLVMX86INTRCallConv = 83,
    LLVMAVRINTRCallConv = 84,
    LLVMAVRSIGNALCallConv = 85,
    LLVMAVRBUILTINCallConv = 86,
    LLVMAMDGPUVSCallConv = 87,
    LLVMAMDGPUGSCallConv = 88,
    LLVMAMDGPUPSCallConv = 89,
    LLVMAMDGPUCSCallConv = 90,
    LLVMAMDGPUKERNELCallConv = 91,
    LLVMX86RegCallCallConv = 92,
    LLVMAMDGPUHSCallConv = 93,
    LLVMMSP430BUILTINCallConv = 94,
    LLVMAMDGPULSCallConv = 95,
    LLVMAMDGPUESCallConv = 96,
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
#[allow(unused)]
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

pub mod analysis {
    use super::*;
    use std::ffi::c_char;

    #[repr(C)]
    #[allow(unused)]
    #[derive(Copy, Clone)]
    pub enum LLVMVerifierFailureAction {
        LLVMAbortProcessAction = 0,
        LLVMPrintMessageAction = 1,
        LLVMReturnStatusAction = 2,
    }

    extern "C" {
        pub fn LLVMVerifyModule(
            m: LLVMModuleRef,
            action: LLVMVerifierFailureAction,
            out_msg: *mut *mut c_char,
        ) -> LLVMBool;
    }
}

pub mod core {
    use super::*;
    use std::ffi::{c_char, c_double, c_uint};

    // core
    extern "C" {
        pub fn LLVMDisposeMessage(Message: *mut c_char);
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
        pub fn LLVMPrintModuleToString(m: LLVMModuleRef) -> *mut c_char;

        pub fn LLVMAddFunction(
            m: LLVMModuleRef,
            name: *const c_char,
            fn_ty: LLVMTypeRef,
        ) -> LLVMValueRef;
        pub fn LLVMSetFunctionCallConv(fn_val: LLVMValueRef, cc: LLVMCallConv);
    }

    // types
    extern "C" {
        pub fn LLVMGetTypeKind(ty: LLVMTypeRef) -> LLVMTypeKind;

        pub fn LLVMInt1TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
        pub fn LLVMInt8TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
        pub fn LLVMInt16TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
        pub fn LLVMInt32TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;
        pub fn LLVMInt64TypeInContext(ctx: LLVMContextRef) -> LLVMTypeRef;

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
        pub fn LLVMConstInt(int_ty: LLVMTypeRef, val: u64, sign_extend: LLVMBool) -> LLVMValueRef;
        pub fn LLVMConstReal(real_ty: LLVMTypeRef, val: c_double) -> LLVMValueRef;

        pub fn LLVMConstStringInContext2(
            ctx: LLVMContextRef,
            str: *const c_char,
            len: usize,
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

        pub fn LLVMConstBitCast(const_val: LLVMValueRef, into_ty: LLVMTypeRef) -> LLVMValueRef;

        pub fn LLVMAddGlobal(
            m: LLVMModuleRef,
            ty: LLVMTypeRef,
            name: *const c_char,
        ) -> LLVMValueRef;
        pub fn LLVMSetInitializer(global_val: LLVMValueRef, const_val: LLVMValueRef);
        pub fn LLVMSetGlobalConstant(global_val: LLVMValueRef, constant: LLVMBool);
        pub fn LLVMSetUnnamedAddress(global_val: LLVMValueRef, unnamed_addr: LLVMUnnamedAddr);
        pub fn LLVMSetThreadLocal(global_val: LLVMValueRef, thread_local: LLVMBool);
        pub fn LLVMSetLinkage(global_val: LLVMValueRef, linkage: LLVMLinkage);

        pub fn LLVMCountParams(fn_val: LLVMValueRef) -> c_uint;
        pub fn LLVMGetParam(fn_val: LLVMValueRef, idx: c_uint) -> LLVMValueRef;
    }

    // basic block
    extern "C" {
        pub fn LLVMGetBasicBlockTerminator(bb: LLVMBasicBlockRef) -> LLVMValueRef;
        pub fn LLVMGetEntryBasicBlock(fn_val: LLVMValueRef) -> LLVMBasicBlockRef;
        pub fn LLVMGetFirstInstruction(bb: LLVMBasicBlockRef) -> LLVMValueRef;
        pub fn LLVMAppendBasicBlockInContext(
            ctx: LLVMContextRef,
            fn_val: LLVMValueRef,
            name: *const c_char,
        ) -> LLVMBasicBlockRef;
    }

    // builder
    extern "C" {
        pub fn LLVMCreateBuilderInContext(ctx: LLVMContextRef) -> LLVMBuilderRef;
        pub fn LLVMDisposeBuilder(b: LLVMBuilderRef);
        pub fn LLVMGetInsertBlock(b: LLVMBuilderRef) -> LLVMBasicBlockRef;
        pub fn LLVMPositionBuilderAtEnd(b: LLVMBuilderRef, bb: LLVMBasicBlockRef);
        pub fn LLVMPositionBuilderBefore(b: LLVMBuilderRef, instr: LLVMValueRef);

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
        pub fn LLVMAddCase(
            switch: LLVMValueRef,
            case_val: LLVMValueRef,
            dest_bb: LLVMBasicBlockRef,
        );
        pub fn LLVMBuildUnreachable(b: LLVMBuilderRef) -> LLVMValueRef;

        pub fn LLVMBuildBinOp(
            b: LLVMBuilderRef,
            op: LLVMOpcode,
            lhs: LLVMValueRef,
            rhs: LLVMValueRef,
            name: *const c_char,
        ) -> LLVMValueRef;
        pub fn LLVMBuildNeg(
            b: LLVMBuilderRef,
            val: LLVMValueRef,
            name: *const c_char,
        ) -> LLVMValueRef;
        pub fn LLVMBuildFNeg(
            b: LLVMBuilderRef,
            val: LLVMValueRef,
            name: *const c_char,
        ) -> LLVMValueRef;
        pub fn LLVMBuildNot(
            b: LLVMBuilderRef,
            val: LLVMValueRef,
            name: *const c_char,
        ) -> LLVMValueRef;

        pub fn LLVMBuildAlloca(
            b: LLVMBuilderRef,
            ty: LLVMTypeRef,
            name: *const c_char,
        ) -> LLVMValueRef;
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

        pub fn LLVMBuildGEP2(
            b: LLVMBuilderRef,
            ptr_ty: LLVMTypeRef,
            ptr_val: LLVMValueRef,
            indices: *mut LLVMValueRef,
            indices_count: c_uint,
            name: *const c_char,
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

        pub fn LLVMBuildCast(
            b: LLVMBuilderRef,
            op: LLVMOpcode,
            val: LLVMValueRef,
            into_ty: LLVMTypeRef,
            name: *const c_char,
        ) -> LLVMValueRef;

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

        pub fn LLVMBuildPhi(
            b: LLVMBuilderRef,
            ty: LLVMTypeRef,
            name: *const c_char,
        ) -> LLVMValueRef;
        pub fn LLVMAddIncoming(
            phi_node: LLVMValueRef,
            incoming_values: *const LLVMValueRef,
            incoming_blocks: *const LLVMBasicBlockRef,
            count: c_uint,
        );

        pub fn LLVMBuildCall2(
            b: LLVMBuilderRef,
            fn_ty: LLVMTypeRef,
            fn_val: LLVMValueRef,
            args: *mut LLVMValueRef,
            arg_count: c_uint,
            name: *const c_char,
        ) -> LLVMValueRef;

        pub fn LLVMBuildExtractValue(
            b: LLVMBuilderRef,
            agg_val: LLVMValueRef,
            index: c_uint,
            name: *const c_char,
        ) -> LLVMValueRef;
    }
}

pub mod target {
    use super::*;
    use std::ffi::c_char;

    pub type LLVMTargetRef = *mut LLVMTarget;
    pub type LLVMTargetDataRef = *mut LLVMTargetData;
    pub type LLVMTargetMachineRef = *mut LLVMTargetMachine;

    pub enum LLVMTarget {}
    pub enum LLVMTargetData {}
    pub enum LLVMTargetMachine {}

    #[repr(C)]
    #[allow(unused)]
    #[derive(Copy, Clone)]
    pub enum LLVMCodeGenOptLevel {
        LLVMCodeGenLevelNone = 0,
        LLVMCodeGenLevelLess = 1,
        LLVMCodeGenLevelDefault = 2,
        LLVMCodeGenLevelAggressive = 3,
    }

    #[repr(C)]
    #[allow(unused)]
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone)]
    pub enum LLVMRelocMode {
        LLVMRelocDefault = 0,
        LLVMRelocStatic = 1,
        LLVMRelocPIC = 2,
        LLVMRelocDynamicNoPic = 3,
        LLVMRelocROPI = 4,
        LLVMRelocRWPI = 5,
        LLVMRelocROPI_RWPI = 6,
    }

    #[repr(C)]
    #[allow(unused)]
    #[derive(Copy, Clone)]
    pub enum LLVMCodeModel {
        LLVMCodeModelDefault = 0,
        LLVMCodeModelJITDefault = 1,
        LLVMCodeModelTiny = 2,
        LLVMCodeModelSmall = 3,
        LLVMCodeModelKernel = 4,
        LLVMCodeModelMedium = 5,
        LLVMCodeModelLarge = 6,
    }

    #[repr(C)]
    #[allow(unused)]
    #[derive(Copy, Clone)]
    pub enum LLVMCodeGenFileType {
        LLVMAssemblyFile = 0,
        LLVMObjectFile = 1,
    }

    extern "C" {
        pub fn LLVMGetTargetFromTriple(
            triple: *const c_char,
            target: *mut LLVMTargetRef,
            error_msg: *mut *mut c_char,
        ) -> LLVMBool;

        pub fn LLVMCreateTargetMachine(
            target: LLVMTargetRef,
            triple: *const c_char,
            cpu: *const c_char,
            features: *const c_char,
            opt_level: LLVMCodeGenOptLevel,
            reloc_mode: LLVMRelocMode,
            code_model: LLVMCodeModel,
        ) -> LLVMTargetMachineRef;
        pub fn LLVMDisposeTargetMachine(tm: LLVMTargetMachineRef);

        pub fn LLVMCreateTargetDataLayout(tm: LLVMTargetMachineRef) -> LLVMTargetDataRef;
        pub fn LLVMDisposeTargetData(td: LLVMTargetDataRef);
        pub fn LLVMSetModuleDataLayout(m: LLVMModuleRef, td: LLVMTargetDataRef);
        pub fn LLVMIntPtrTypeInContext(ctx: LLVMContextRef, td: LLVMTargetDataRef) -> LLVMTypeRef;

        pub fn LLVMTargetMachineEmitToFile(
            tm: LLVMTargetMachineRef,
            m: LLVMModuleRef,
            filename: *mut c_char,
            codegen: LLVMCodeGenFileType,
            error_msg: *mut *mut c_char,
        ) -> LLVMBool;

        pub fn LLVMInitializeX86AsmParser();
        pub fn LLVMInitializeX86AsmPrinter();
        pub fn LLVMInitializeX86Disassembler();
        pub fn LLVMInitializeX86Target();
        pub fn LLVMInitializeX86TargetInfo();
        pub fn LLVMInitializeX86TargetMC();

        pub fn LLVMInitializeAArch64AsmParser();
        pub fn LLVMInitializeAArch64AsmPrinter();
        pub fn LLVMInitializeAArch64Disassembler();
        pub fn LLVMInitializeAArch64Target();
        pub fn LLVMInitializeAArch64TargetInfo();
        pub fn LLVMInitializeAArch64TargetMC();
    }
}

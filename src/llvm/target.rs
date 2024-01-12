use super::types::*;
use std::ffi::*;

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMByteOrdering {
    LLVMBigEndian = 0,
    LLVMLittleEndian = 1,
}

pub type LLVMTargetDataRef = *mut LLVMOpaqueTargetData;
pub type LLVMTargetLibraryInfoRef = *mut LLVMOpaqueTargetLibraryInfoData;

pub enum LLVMOpaqueTargetData {}
pub enum LLVMOpaqueTargetLibraryInfoData {}

extern "C" {
    pub fn LLVMGetModuleDataLayout(M: LLVMModuleRef) -> LLVMTargetDataRef; // Get the data layout for a module.
    pub fn LLVMSetModuleDataLayout(M: LLVMModuleRef, R: LLVMTargetDataRef); // Set the data layout for a module.
    pub fn LLVMCreateTargetData(StringRep: *const c_char) -> LLVMTargetDataRef; // Create target data from a target layout string.
    pub fn LLVMAddTargetLibraryInfo(TLI: LLVMTargetLibraryInfoRef, PM: LLVMPassManagerRef);
    pub fn LLVMCopyStringRepOfTargetData(TD: LLVMTargetDataRef) -> *mut c_char;
    pub fn LLVMByteOrder(TD: LLVMTargetDataRef) -> LLVMByteOrdering;
    pub fn LLVMPointerSize(TD: LLVMTargetDataRef) -> c_uint;
    pub fn LLVMPointerSizeForAS(TD: LLVMTargetDataRef, AS: c_uint) -> c_uint;
    pub fn LLVMIntPtrType(TD: LLVMTargetDataRef) -> LLVMTypeRef;
    pub fn LLVMIntPtrTypeForAS(TD: LLVMTargetDataRef, AS: c_uint) -> LLVMTypeRef;
    pub fn LLVMIntPtrTypeInContext(C: LLVMContextRef, TD: LLVMTargetDataRef) -> LLVMTypeRef;
    pub fn LLVMIntPtrTypeForASInContext(
        C: LLVMContextRef,
        TD: LLVMTargetDataRef,
        AS: c_uint,
    ) -> LLVMTypeRef;
    pub fn LLVMSizeOfTypeInBits(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_ulonglong;
    pub fn LLVMStoreSizeOfType(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_ulonglong;
    pub fn LLVMABISizeOfType(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_ulonglong;
    pub fn LLVMABIAlignmentOfType(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_uint;
    pub fn LLVMCallFrameAlignmentOfType(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_uint;
    pub fn LLVMPreferredAlignmentOfType(TD: LLVMTargetDataRef, Ty: LLVMTypeRef) -> c_uint;
    pub fn LLVMPreferredAlignmentOfGlobal(TD: LLVMTargetDataRef, GlobalVar: LLVMValueRef)
        -> c_uint;
    pub fn LLVMElementAtOffset(
        TD: LLVMTargetDataRef,
        StructTy: LLVMTypeRef,
        Offset: c_ulonglong,
    ) -> c_uint;
    pub fn LLVMOffsetOfElement(
        TD: LLVMTargetDataRef,
        StructTy: LLVMTypeRef,
        Element: c_uint,
    ) -> c_ulonglong;
    pub fn LLVMDisposeTargetData(TD: LLVMTargetDataRef);
}

extern "C" {
    pub fn LLVM_InitializeAllTargetInfos();
    pub fn LLVM_InitializeAllTargets();
    pub fn LLVM_InitializeAllTargetMCs();
    pub fn LLVM_InitializeAllAsmPrinters();
    pub fn LLVM_InitializeAllAsmParsers();
    pub fn LLVM_InitializeAllDisassemblers();

    pub fn LLVM_InitializeNativeTarget() -> LLVMBool; // Returns 1 on failure.
    pub fn LLVM_InitializeNativeAsmParser() -> LLVMBool; // Returns 1 on failure.
    pub fn LLVM_InitializeNativeAsmPrinter() -> LLVMBool; // Returns 1 on failure.
    pub fn LLVM_InitializeNativeDisassembler() -> LLVMBool; // Returns 1 on failure.

    pub fn LLVMInitializeAMDGPUTargetInfo();
    pub fn LLVMInitializeAMDGPUTarget();
    pub fn LLVMInitializeAMDGPUTargetMC();
    pub fn LLVMInitializeAMDGPUAsmPrinter();
    pub fn LLVMInitializeAMDGPUAsmParser();
    // Disassembler?

    pub fn LLVMInitializeSystemZTargetInfo();
    pub fn LLVMInitializeSystemZTarget();
    pub fn LLVMInitializeSystemZTargetMC();
    pub fn LLVMInitializeSystemZAsmPrinter();
    pub fn LLVMInitializeSystemZAsmParser();
    pub fn LLVMInitializeSystemZDisassembler();

    pub fn LLVMInitializeHexagonTargetInfo();
    pub fn LLVMInitializeHexagonTarget();
    pub fn LLVMInitializeHexagonTargetMC();
    pub fn LLVMInitializeHexagonAsmPrinter();
    // AsmParser?
    pub fn LLVMInitializeHexagonDisassembler();

    pub fn LLVMInitializeNVPTXTargetInfo();
    pub fn LLVMInitializeNVPTXTarget();
    pub fn LLVMInitializeNVPTXTargetMC();
    pub fn LLVMInitializeNVPTXAsmPrinter();
    // AsmParser?

    pub fn LLVMInitializeMSP430TargetInfo();
    pub fn LLVMInitializeMSP430Target();
    pub fn LLVMInitializeMSP430TargetMC();
    pub fn LLVMInitializeMSP430AsmPrinter();
    // AsmParser?

    pub fn LLVMInitializeXCoreTargetInfo();
    pub fn LLVMInitializeXCoreTarget();
    pub fn LLVMInitializeXCoreTargetMC();
    pub fn LLVMInitializeXCoreAsmPrinter();
    // AsmParser?
    pub fn LLVMInitializeXCoreDisassembler();

    pub fn LLVMInitializeMipsTargetInfo();
    pub fn LLVMInitializeMipsTarget();
    pub fn LLVMInitializeMipsTargetMC();
    pub fn LLVMInitializeMipsAsmPrinter();
    pub fn LLVMInitializeMipsAsmParser();
    pub fn LLVMInitializeMipsDisassembler();

    pub fn LLVMInitializeAArch64TargetInfo();
    pub fn LLVMInitializeAArch64Target();
    pub fn LLVMInitializeAArch64TargetMC();
    pub fn LLVMInitializeAArch64AsmPrinter();
    pub fn LLVMInitializeAArch64AsmParser();
    pub fn LLVMInitializeAArch64Disassembler();

    pub fn LLVMInitializeARMTargetInfo();
    pub fn LLVMInitializeARMTarget();
    pub fn LLVMInitializeARMTargetMC();
    pub fn LLVMInitializeARMAsmPrinter();
    pub fn LLVMInitializeARMAsmParser();
    pub fn LLVMInitializeARMDisassembler();

    pub fn LLVMInitializePowerPCTargetInfo();
    pub fn LLVMInitializePowerPCTarget();
    pub fn LLVMInitializePowerPCTargetMC();
    pub fn LLVMInitializePowerPCAsmPrinter();
    pub fn LLVMInitializePowerPCAsmParser();
    pub fn LLVMInitializePowerPCDisassembler();

    pub fn LLVMInitializeSparcTargetInfo();
    pub fn LLVMInitializeSparcTarget();
    pub fn LLVMInitializeSparcTargetMC();
    pub fn LLVMInitializeSparcAsmPrinter();
    pub fn LLVMInitializeSparcAsmParser();
    pub fn LLVMInitializeSparcDisassembler();

    pub fn LLVMInitializeX86TargetInfo();
    pub fn LLVMInitializeX86Target();
    pub fn LLVMInitializeX86TargetMC();
    pub fn LLVMInitializeX86AsmPrinter();
    pub fn LLVMInitializeX86AsmParser();
    pub fn LLVMInitializeX86Disassembler();

    pub fn LLVMInitializeBPFTargetInfo();
    pub fn LLVMInitializeBPFTarget();
    pub fn LLVMInitializeBPFTargetMC();
    pub fn LLVMInitializeBPFAsmPrinter();
    pub fn LLVMInitializeBPFAsmParser();
    pub fn LLVMInitializeBPFDisassembler();

    pub fn LLVMInitializeLanaiTargetInfo();
    pub fn LLVMInitializeLanaiTarget();
    pub fn LLVMInitializeLanaiTargetMC();
    pub fn LLVMInitializeLanaiAsmPrinter();
    pub fn LLVMInitializeLanaiAsmParser();
    pub fn LLVMInitializeLanaiDisassembler();

    pub fn LLVMInitializeRISCVTargetInfo();
    pub fn LLVMInitializeRISCVTarget();
    pub fn LLVMInitializeRISCVTargetMC();
    pub fn LLVMInitializeRISCVAsmPrinter();
    pub fn LLVMInitializeRISCVAsmParser();
    pub fn LLVMInitializeRISCVDisassembler();

    pub fn LLVMInitializeLoongArchTargetInfo();
    pub fn LLVMInitializeLoongArchTarget();
    pub fn LLVMInitializeLoongArchTargetMC();
    pub fn LLVMInitializeLoongArchAsmPrinter();
    pub fn LLVMInitializeLoongArchAsmParser();
    pub fn LLVMInitializeLoongArchDisassembler();

    pub fn LLVMInitializeWebAssemblyTargetInfo();
    pub fn LLVMInitializeWebAssemblyTarget();
    pub fn LLVMInitializeWebAssemblyTargetMC();
    pub fn LLVMInitializeWebAssemblyAsmPrinter();
    pub fn LLVMInitializeWebAssemblyAsmParser();
    pub fn LLVMInitializeWebAssemblyDisassembler();
}
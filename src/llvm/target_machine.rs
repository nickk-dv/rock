use super::target::LLVMTargetDataRef;
use super::types::*;
use std::ffi::*;

pub type LLVMTargetRef = *mut LLVMTarget;
pub type LLVMTargetMachineRef = *mut LLVMOpaqueTargetMachine;

pub enum LLVMTarget {}
pub enum LLVMOpaqueTargetMachine {}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMCodeGenOptLevel {
    LLVMCodeGenLevelNone = 0,
    LLVMCodeGenLevelLess = 1,
    LLVMCodeGenLevelDefault = 2,
    LLVMCodeGenLevelAggressive = 3,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy, PartialEq)]
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
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMCodeGenFileType {
    LLVMAssemblyFile = 0,
    LLVMObjectFile = 1,
}

extern "C" {
    pub fn LLVMGetFirstTarget() -> LLVMTargetRef;
    pub fn LLVMGetNextTarget(T: LLVMTargetRef) -> LLVMTargetRef;
    pub fn LLVMGetTargetFromName(Name: *const c_char) -> LLVMTargetRef;
    pub fn LLVMGetTargetFromTriple(
        Triple: *const c_char,
        T: *mut LLVMTargetRef,
        ErrorMessage: *mut *mut c_char,
    ) -> LLVMBool;
    pub fn LLVMGetTargetName(T: LLVMTargetRef) -> *const c_char;
    pub fn LLVMGetTargetDescription(T: LLVMTargetRef) -> *const c_char;
    pub fn LLVMTargetHasJIT(T: LLVMTargetRef) -> LLVMBool;
    pub fn LLVMTargetHasTargetMachine(T: LLVMTargetRef) -> LLVMBool;
    pub fn LLVMTargetHasAsmBackend(T: LLVMTargetRef) -> LLVMBool;
    pub fn LLVMCreateTargetMachine(
        T: LLVMTargetRef,
        Triple: *const c_char,
        CPU: *const c_char,
        Features: *const c_char,
        Level: LLVMCodeGenOptLevel,
        Reloc: LLVMRelocMode,
        CodeModel: LLVMCodeModel,
    ) -> LLVMTargetMachineRef;
    pub fn LLVMDisposeTargetMachine(T: LLVMTargetMachineRef);
    pub fn LLVMGetTargetMachineTarget(T: LLVMTargetMachineRef) -> LLVMTargetRef;
    pub fn LLVMGetTargetMachineTriple(T: LLVMTargetMachineRef) -> *mut c_char;
    pub fn LLVMGetTargetMachineCPU(T: LLVMTargetMachineRef) -> *mut c_char;
    pub fn LLVMGetTargetMachineFeatureString(T: LLVMTargetMachineRef) -> *mut c_char;
    pub fn LLVMCreateTargetDataLayout(T: LLVMTargetMachineRef) -> LLVMTargetDataRef; // Create a DataLayout based on the target machine.
    pub fn LLVMSetTargetMachineAsmVerbosity(T: LLVMTargetMachineRef, VerboseAsm: LLVMBool);
    pub fn LLVMTargetMachineEmitToFile(
        T: LLVMTargetMachineRef,
        M: LLVMModuleRef,
        Filename: *mut c_char,
        codegen: LLVMCodeGenFileType,
        ErrorMessage: *mut *mut c_char,
    ) -> LLVMBool;
    pub fn LLVMTargetMachineEmitToMemoryBuffer(
        T: LLVMTargetMachineRef,
        M: LLVMModuleRef,
        codegen: LLVMCodeGenFileType,
        ErrorMessage: *mut *mut c_char,
        OutMemBuf: *mut LLVMMemoryBufferRef,
    ) -> LLVMBool;

    pub fn LLVMGetDefaultTargetTriple() -> *mut c_char;
    pub fn LLVMNormalizeTargetTriple(triple: *const c_char) -> *mut c_char; // Normalize a target triple. The result needs to be disposed with LLVMDisposeMessage.
    pub fn LLVMGetHostCPUName() -> *mut c_char; // Get the host CPU as a string. The result needs to be disposed with LLVMDisposeMessage.
    pub fn LLVMGetHostCPUFeatures() -> *mut c_char; // Get the host CPU's features as a string. The result needs to be disposed with LLVMDisposeMessage.
    pub fn LLVMAddAnalysisPasses(T: LLVMTargetMachineRef, PM: LLVMPassManagerRef);
}

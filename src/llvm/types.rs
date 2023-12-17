use std::ffi::*;

pub type LLVMBool = c_int;
pub type LLVMMemoryBufferRef = *mut LLVMMemoryBuffer;
pub type LLVMContextRef = *mut LLVMContext;
pub type LLVMModuleRef = *mut LLVMModule;
pub type LLVMTypeRef = *mut LLVMType;
pub type LLVMValueRef = *mut LLVMValue;
pub type LLVMBasicBlockRef = *mut LLVMBasicBlock;
pub type LLVMMetadataRef = *mut LLVMOpaqueMetadata;
pub type LLVMNamedMDNodeRef = *mut LLVMOpaqueNamedMDNode;
pub type LLVMValueMetadataEntry = *mut LLVMOpaqueValueMetadataEntry;
pub type LLVMBuilderRef = *mut LLVMBuilder;
pub type LLVMDIBuilderRef = *mut LLVMOpaqueDIBuilder;
pub type LLVMModuleProviderRef = *mut LLVMModuleProvider;
pub type LLVMPassManagerRef = *mut LLVMPassManager;
pub type LLVMUseRef = *mut LLVMUse;
pub type LLVMDiagnosticInfoRef = *mut LLVMDiagnosticInfo;
pub type LLVMComdatRef = *mut LLVMComdat;
pub type LLVMModuleFlagEntry = *mut LLVMOpaqueModuleFlagEntry;
pub type LLVMJITEventListenerRef = *mut LLVMOpaqueJITEventListener;
pub type LLVMAttributeRef = *mut LLVMOpaqueAttributeRef;

pub enum LLVMMemoryBuffer {}
pub enum LLVMContext {}
pub enum LLVMModule {}
pub enum LLVMType {}
pub enum LLVMValue {}
pub enum LLVMBasicBlock {}
pub enum LLVMOpaqueMetadata {}
pub enum LLVMOpaqueNamedMDNode {}
pub enum LLVMOpaqueValueMetadataEntry {}
pub enum LLVMBuilder {}
pub enum LLVMOpaqueDIBuilder {}
pub enum LLVMModuleProvider {}
pub enum LLVMPassManager {}
pub enum LLVMUse {}
pub enum LLVMDiagnosticInfo {}
pub enum LLVMComdat {}
pub enum LLVMOpaqueModuleFlagEntry {}
pub enum LLVMOpaqueJITEventListener {}
pub enum LLVMOpaqueAttributeRef {}

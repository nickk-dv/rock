pub mod core;

use std::ffi::c_int;

pub type LLVMBool = c_int;
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

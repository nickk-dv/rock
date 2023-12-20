use super::types::*;
use std::ffi::*;

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum LLVMVerifierFailureAction {
    LLVMAbortProcessAction = 0,
    LLVMPrintMessageAction = 1,
    LLVMReturnStatusAction = 2,
}

extern "C" {
    pub fn LLVMVerifyModule(
        M: LLVMModuleRef,
        Action: LLVMVerifierFailureAction,
        OutMessage: *mut *mut c_char,
    ) -> LLVMBool;
    pub fn LLVMVerifyFunction(Fn: LLVMValueRef, Action: LLVMVerifierFailureAction) -> LLVMBool;
    pub fn LLVMViewFunctionCFG(Fn: LLVMValueRef);
    pub fn LLVMViewFunctionCFGOnly(Fn: LLVMValueRef);
}

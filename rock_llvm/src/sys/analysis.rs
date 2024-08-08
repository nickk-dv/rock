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

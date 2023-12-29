#[derive(Copy, Clone)]
pub enum CheckError {
    SymbolRedefinition,
    ExternalProcRedefinition,
    ProcParamRedefinition,
    EnumVariantRedefinition,
    StructFieldRedefinition,

    MainProcMissing,
    MainProcVariadic,
    MainProcExternal,
    MainProcWrongRetType,
}

pub struct CheckErrorData {
    pub message: &'static str,
    pub help: Option<&'static str>,
}

impl CheckErrorData {
    fn new(message: &'static str, help: Option<&'static str>) -> Self {
        Self { message, help }
    }
}

impl CheckError {
    pub fn get_data(&self) -> CheckErrorData {
        match self {
            CheckError::SymbolRedefinition => CheckErrorData::new("symbol redefinition", None),
            CheckError::ExternalProcRedefinition => CheckErrorData::new("external procedure with redefinition", Some("import and use one of existing procedures, redefinition will cause linker errors")),
            CheckError::ProcParamRedefinition => CheckErrorData::new("procedure parameter redefinition", None),
            CheckError::EnumVariantRedefinition => CheckErrorData::new("enum variant redefinition", None),
            CheckError::StructFieldRedefinition => CheckErrorData::new("struct field redefinition", None),
            
            CheckError::MainProcMissing => CheckErrorData::new("main procedure is not found in src/main.lang", Some("define the entry point: `main :: () -> s32 { return 0; }`")), //@unstable file ext .lang
            CheckError::MainProcVariadic => CheckErrorData::new("main procedure cannot be variadic", Some("remove `..` from input parameters")),
            CheckError::MainProcExternal => CheckErrorData::new("main procedure cannot be external", Some("remove `c_call` directive")), //@unstable directive name
            CheckError::MainProcWrongRetType => CheckErrorData::new("main procedure must return s32", Some("change return type to: `-> s32`")),
        }
    }
}

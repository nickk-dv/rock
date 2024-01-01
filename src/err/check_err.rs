#[derive(Copy, Clone)]
pub enum CheckError {
    ParseSrcDirMissing,
    ParseLibFileMissing,
    ParseMainFileMissing,
    ParseModRedefinition,
    ParseModBothPathsExist,
    ParseModBothPathsMissing,
    ParseModCycle,

    SymbolRedefinition,
    ExternalProcRedefinition,
    ProcParamRedefinition,
    EnumVariantRedefinition,
    StructFieldRedefinition,

    MainProcMissing,
    MainProcVariadic,
    MainProcExternal,
    MainProcHasParams,
    MainProcWrongRetType,

    ImportModuleAccessMissing,
    SuperUsedFromRootModule,
    ModuleNotDefined,
    ModuleIsPrivate,
    ImportFromItself,
    ImportItself,
    ImportSymbolNotDefined,
    ImportSymbolIsPrivate,
    ImportSymbolAlreadyDefined,
    ImporySymbolAlreadyImported,
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
            CheckError::ParseSrcDirMissing => CheckErrorData::new("missing `src` directory", Some("make sure that current directory is set to the project directory")),
            CheckError::ParseLibFileMissing => CheckErrorData::new("missing `src/lib.lang` file", Some("the root module `lib.lang` of library package must exist")), //@unstable file ext .lang
            CheckError::ParseMainFileMissing => CheckErrorData::new("missing `src/main.lang` file", Some("the root module `main.lang` of executable package must exist")), //@unstable file ext .lang
            CheckError::ParseModRedefinition => CheckErrorData::new("module redefinition", None),
            CheckError::ParseModBothPathsExist => CheckErrorData::new("both module filepaths exist", Some("only one filepath may exist:")),
            CheckError::ParseModBothPathsMissing => CheckErrorData::new("both module filepaths are missing", Some("at least one filepath must exist:")),
            CheckError::ParseModCycle => CheckErrorData::new("module definition results in a cycle", Some("module paths that form a cycle:")),
            
            CheckError::SymbolRedefinition => CheckErrorData::new("symbol redefinition", None),
            CheckError::ExternalProcRedefinition => CheckErrorData::new("external procedure with redefinition", Some("import and use one of existing procedures, redefinition will cause linker errors")),
            CheckError::ProcParamRedefinition => CheckErrorData::new("procedure parameter redefinition", None),
            CheckError::EnumVariantRedefinition => CheckErrorData::new("enum variant redefinition", None),
            CheckError::StructFieldRedefinition => CheckErrorData::new("struct field redefinition", None),
            
            CheckError::MainProcMissing => CheckErrorData::new("main procedure is not found in src/main.lang", Some("define the entry point `main :: () -> s32 { return 0; }`")), //@unstable file ext .lang
            CheckError::MainProcVariadic => CheckErrorData::new("main procedure cannot be variadic", Some("remove `..` from input parameters")),
            CheckError::MainProcExternal => CheckErrorData::new("main procedure cannot be external", Some("remove `c_call` directive")), //@unstable directive name
            CheckError::MainProcHasParams => CheckErrorData::new("main procedure cannot have input parameters", Some("remove input parameters")),
            CheckError::MainProcWrongRetType => CheckErrorData::new("main procedure must return `s32`", Some("change return type to `-> s32`")),
            
            CheckError::ImportModuleAccessMissing => CheckErrorData::new("import missing module access path", Some("specify module access path before the import target")),
            CheckError::SuperUsedFromRootModule => CheckErrorData::new("using `super` in the root module", Some("`super` refers to the parent module, which doesnt exist for the root module")),
            CheckError::ModuleNotDefined => CheckErrorData::new("module is not defined", None),
            CheckError::ModuleIsPrivate => CheckErrorData::new("module is private", None),
            CheckError::ImportFromItself => CheckErrorData::new("unnecessary import from itself", Some("remove this import declaration")),
            CheckError::ImportItself => CheckErrorData::new("cannot import module into itself", Some("remove this import declaration")),
            CheckError::ImportSymbolNotDefined => CheckErrorData::new("imported symbol is not defined in target module", None),
            CheckError::ImportSymbolIsPrivate => CheckErrorData::new("imported symbol is private", Some("cannot import symbols declared without `pub` keyword")),
            CheckError::ImportSymbolAlreadyDefined => CheckErrorData::new("imported symbol is already defined", None),
            CheckError::ImporySymbolAlreadyImported => CheckErrorData::new("imported symbol was already imported", None),
        }
    }
}

use super::error::*;

pub(super) type Message = (&'static str, Option<&'static str>);

impl Into<Message> for CheckError {
    fn into(self) -> Message {
        match self {
            CheckError::ParseSrcDirMissing =>          ("missing `src` directory", Some("make sure that current directory is set to the project directory before running compiler commands")),
            CheckError::ParseLibFileMissing =>         ("missing `src/lib.lang` file", Some("the root module `lib.lang` of library package must exist")), //@unstable file ext .lang
            CheckError::ParseMainFileMissing =>        ("missing `src/main.lang` file", Some("the root module `main.lang` of executable package must exist")), //@unstable file ext .lang
            CheckError::ParseModBothPathsExist =>      ("both module filepaths exist", None),
            CheckError::ParseModBothPathsMissing =>    ("both module filepaths are missing", None),
            CheckError::ParseModCycle =>               ("module definition results in a cycle", None),

            CheckError::ModRedefinition =>             ("module redefinition", None),
            CheckError::SymbolRedefinition =>          ("symbol redefinition", None),
            CheckError::ProcParamRedefinition =>       ("procedure parameter redefinition", None),
            CheckError::EnumVariantRedefinition =>     ("enum variant redefinition", None),
            CheckError::StructFieldRedefinition =>     ("struct field redefinition", None),
            
            CheckError::MainProcMissing =>             ("main procedure is not found in src/main.lang", Some("define the entry point `main :: () -> s32 { return 0; }`")), //@unstable file ext .lang
            CheckError::MainProcVariadic =>            ("main procedure cannot be variadic", Some("remove `..` from input parameters")),
            CheckError::MainProcExternal =>            ("main procedure cannot be external", Some("remove `c_call` directive")), //@unstable directive name
            CheckError::MainProcHasParams =>           ("main procedure cannot have input parameters", Some("remove input parameters")),
            CheckError::MainProcWrongRetType =>        ("main procedure must return `s32`", Some("change return type to `-> s32`")),
            
            CheckError::ImportModuleAccessMissing =>   ("import missing module access path", Some("specify module access path before the import target")),
            CheckError::SuperUsedFromRootModule =>     ("using `super` in the root module", Some("`super` refers to the parent module, which doesnt exist for the root module")),
            CheckError::ModuleIsPrivate =>             ("module is private", None),
            CheckError::ModuleNotFoundInScope =>       ("module is not found in this scope", None),
            CheckError::ModuleNotDeclaredInPath =>     ("module is not declared in referenced module path", None),
            CheckError::ImportFromItself =>            ("importing from itself is redundant", Some("remove this import")),
            CheckError::ImportItself =>                ("importing module into itself is redundant", Some("remove this import")),
            CheckError::ImportWildcardExists =>        ("wildcard import of module already exists", Some("remove this import")),
            CheckError::ImportSymbolNotDefined =>      ("imported symbol is not defined in target module", None),
            CheckError::ImportSymbolIsPrivate =>       ("imported symbol is private", Some("cannot import symbols declared without `pub` keyword")),
            CheckError::ImportSymbolAlreadyDefined =>  ("imported symbol is already defined", None),
            CheckError::ImporySymbolAlreadyImported => ("imported symbol is already imported", Some("remove this symbol import")),

            CheckError::ModuleSymbolConflit =>         ("this module name conflits with others in scope", None), 
        }
    }
}

impl Into<Message> for FileIOError {
    fn into(self) -> Message {
        match self {
            FileIOError::DirRead =>    ("directory read failed", None),
            FileIOError::DirCreate =>  ("directory create failed", None),
            FileIOError::FileRead =>   ("file read failed", None),
            FileIOError::FileCreate => ("file create failed", None),
            FileIOError::FileWrite =>  ("file write failed", None),
        }
    }
}

impl Into<Message> for InternalError {
    fn into(self) -> Message {
        //match self {
        //}
        ("internal", None)
    }
}

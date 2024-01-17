use super::error::*;

pub(super) type Message = (&'static str, Option<&'static str>);

impl Into<Message> for CheckError {
    #[rustfmt::skip]
    fn into(self) -> Message {
        match self {
            CheckError::ParseLibFileMissing =>         ("missing `src/lib.lang` file", Some("the root module `lib.lang` of library package must exist")), //@unstable file ext .lang
            CheckError::ParseMainFileMissing =>        ("missing `src/main.lang` file", Some("the root module `main.lang` of executable package must exist")), //@unstable file ext .lang
            CheckError::ParseModBothPathsExist =>      ("both module filepaths exist", None),
            CheckError::ParseModBothPathsMissing =>    ("both module filepaths are missing", None),
            CheckError::ParseModCycle =>               ("module definition results in a cycle", None),

            CheckError::RedefinitionMod =>             ("module redefinition", None),
            CheckError::RedefinitionProc =>            ("procedure redefinition", None),
            CheckError::RedefinitionType =>            ("type redefinition", None),
            CheckError::RedefinitionGlobal =>          ("global constant redefinition", None),

            CheckError::ProcParamRedefinition =>       ("procedure parameter redefinition", None),
            CheckError::EnumVariantRedefinition =>     ("enum variant redefinition", None),
            CheckError::StructFieldRedefinition =>     ("struct field redefinition", None),
            
            CheckError::MainProcMissing =>             ("main procedure is not found in src/main.lang", Some("define the entry point `main :: () -> s32 { return 0; }`")), //@unstable file ext .lang
            CheckError::MainProcVariadic =>            ("main procedure cannot be variadic", Some("remove `..` from input parameters")),
            CheckError::MainProcExternal =>            ("main procedure cannot be external", Some("remove `c_call` directive")), //@unstable directive name
            CheckError::MainProcHasParams =>           ("main procedure cannot have input parameters", Some("remove input parameters")),
            CheckError::MainProcWrongRetType =>        ("main procedure must return `s32`", Some("change return type to `-> s32`")),
            
            CheckError::SuperUsedFromRootModule =>     ("using `super` in the root module", Some("`super` refers to the parent module, which doesnt exist for the root module")),
            CheckError::ModuleFileReportedMissing =>   ("module is missing a source file, as reported earlier", None),
            CheckError::ImportFromItself =>            ("importing from itself is redundant", Some("remove this import")),
            CheckError::ImportItself =>                ("importing module into itself is redundant", Some("remove this import")),
            CheckError::ImportGlobExists =>            ("glob import of module already exists", Some("remove this import")),
            CheckError::ImportSymbolNotDefined =>      ("imported symbol is not found", None),
            CheckError::ImportSymbolAlreadyImported => ("symbol is already imported", Some("same name cannot be imported multiple times")),

            CheckError::ModuleNotDeclaredInPath =>     ("module is not declared in referenced module path", None),
            CheckError::ProcNotDeclaredInPath =>       ("procedure is not declared in referenced module path", None),
            CheckError::TypeNotDeclaredInPath =>       ("type name is not declared in referenced module path", None),
            
            CheckError::ModuleIsPrivate =>             ("module is private", None),
            CheckError::ProcIsPrivate =>               ("procedure is private", None),
            CheckError::TypeIsPrivate =>               ("type is private", None),

            CheckError::ModuleNotFoundInScope =>       ("module is not found in scope", None),
            CheckError::ProcNotFoundInScope =>         ("procedure is not found in scope", None),
            CheckError::TypeNotFoundInScope =>         ("type name is not found in scope", None),
            CheckError::GlobalNotFoundInScope =>       ("global constant is not found in scope", None),
            CheckError::ModuleSymbolConflict =>        ("module name conflits with others in scope", None),
            CheckError::ProcSymbolConflict =>          ("procedure name conflits with others in scope", None),
            CheckError::TypeSymbolConflict =>          ("type name conflits with others in scope", None),
            CheckError::GlobalSymbolConflict =>        ("global constant name conflits with others in scope", None),
        }
    }
}

impl Into<Message> for FileIOError {
    #[rustfmt::skip]
    fn into(self) -> Message {
        match self {
            FileIOError::DirRead =>       ("directory read failed", None),
            FileIOError::DirCreate =>     ("directory create failed", None),
            FileIOError::FileRead =>      ("file read failed", None),
            FileIOError::FileCreate =>    ("file create failed", None),
            FileIOError::FileWrite =>     ("file write failed", None),
            FileIOError::EnvCommand =>    ("failed to execute the command", None),
            FileIOError::EnvCurrentDir => ("failed to set working directory", None),
        }
    }
}

impl Into<Message> for InternalError {
    #[rustfmt::skip]
    fn into(self) -> Message {
        //match self {
        //}
        ("internal", None)
    }
}

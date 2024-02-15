use crate::ast::ast::*;
use crate::ast::intern::InternID;
use crate::mem::P;
use std::collections::HashMap;

struct Context {
    scopes: Vec<Scope>,
    modules: Vec<ModuleData>,
    globals: Vec<GlobalData>,
    procs: Vec<ProcData>,
    enums: Vec<EnumData>,
    unions: Vec<UnionData>,
    structs: Vec<StructData>,
}

struct ScopeID(u32);
struct ModuleID(u32);
struct GlobalID(u32);
struct ProcID(u32);
struct EnumID(u32);
struct UnionID(u32);
struct StructID(u32);

struct Scope {
    module: P<Module>,
    symbols: HashMap<InternID, Symbol>,
}

enum Symbol {
    Module(ModuleID),
    Global(GlobalID),
    Proc(ProcID),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
}

struct ModuleData {
    scope_id: ScopeID,
    decl: P<ModuleDecl>,
    target_id: Option<ScopeID>,
}

struct GlobalData {
    scope_id: ScopeID,
    decl: P<GlobalDecl>,
}

struct ProcData {
    scope_id: ScopeID,
    decl: P<ProcDecl>,
}

struct EnumData {
    scope_id: ScopeID,
    decl: P<EnumDecl>,
}

struct UnionData {
    scope_id: ScopeID,
    decl: P<UnionDecl>,
    size: usize,
    align: u32,
}

struct StructData {
    scope_id: ScopeID,
    decl: P<StructDecl>,
    size: usize,
    align: u32,
}

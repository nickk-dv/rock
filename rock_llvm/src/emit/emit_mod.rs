use super::context::Codegen;
use crate::llvm;
use rock_core::hir;

pub fn codegen_module<'c>(hir: hir::Hir<'c>) {
    let mut cg = Codegen::new(hir);
    codegen_string_lits(&mut cg);
    codegen_struct_types(&mut cg);
}

fn codegen_string_lits(cg: &mut Codegen) {
    for (idx, &string) in cg.hir.intern_string.get_all_strings().iter().enumerate() {
        let c_string = cg.hir.string_is_cstr[idx];
        let str_value = llvm::const_string(string, c_string);
        let str_type = llvm::typeof_value(str_value);

        let global = cg.module.add_global(
            str_type,
            "rock_string_lit",
            str_value,
            true,
            true,
            false,
            llvm::Linkage::LLVMInternalLinkage,
        );
        cg.string_lits.push(global);
    }
}

fn codegen_struct_types(cg: &mut Codegen) {
    for _ in 0..cg.hir.structs.len() {
        let opaque = cg.context.struct_create_named("rock_struct");
        cg.structs.push(opaque);
    }

    let mut field_types = Vec::with_capacity(64);
    for (idx, struct_data) in cg.hir.structs.iter().enumerate() {
        field_types.clear();
        for field in struct_data.fields {
            field_types.push(cg.ty(field.ty));
        }
        let opaque = cg.structs[idx];
        cg.context.struct_set_body(opaque, &field_types, false)
    }
}

use crate::ast;
use crate::error::ErrorComp;
use crate::fs_env;
use crate::hir;
use crate::session::Session;
use crate::timer::Timer;

struct Codegen {
    buffer: String,
}

impl Codegen {
    fn new() -> Codegen {
        Codegen {
            buffer: String::with_capacity(1024 * 64),
        }
    }

    fn finish(self) -> String {
        self.buffer
    }
    fn write(&mut self, c: char) {
        self.buffer.push(c);
    }
    fn write_str(&mut self, string: &str) {
        self.buffer.push_str(string);
    }
    fn space(&mut self) {
        self.buffer.push(' ');
    }
    fn tab(&mut self) {
        self.buffer.push(' ');
        self.buffer.push(' ');
    }
    fn new_line(&mut self) {
        self.buffer.push('\n');
    }
}

pub fn codegen_module(session: &Session, hir: hir::Hir) -> Result<(), ErrorComp> {
    let mut total = Timer::new();
    let mut timer1 = Timer::new();
    let mut cg = Codegen::new();
    timer1.measure();

    let mut timer2 = Timer::new();
    codegen_string_literals(&mut cg, &hir);
    timer2.measure();

    let mut timer3 = Timer::new();
    codegen_struct_types(&mut cg, &hir);
    timer3.measure();

    let mut timer4 = Timer::new();
    codegen_globals(&mut cg, &hir);
    timer4.measure();

    let mut timer5 = Timer::new();
    let buffer_ll = cg.finish();
    let build_dir = session.cwd().join("build");
    fs_env::dir_create(&build_dir, false)?;
    let debug_dir = build_dir.join("debug");
    fs_env::dir_create(&debug_dir, false)?;
    let module_path = debug_dir.join("codegen_ll_test.ll");
    fs_env::file_create_or_rewrite(&module_path, &buffer_ll)?;
    timer5.measure();

    let mut timer6 = Timer::new();
    let args = vec![
        module_path.to_string_lossy().to_string(),
        "-o".into(),
        "codegen_ll_test.exe".into(),
    ];

    let _ = std::process::Command::new("clang")
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to build llvm ir module: `{}`\nreason: {}",
                module_path.to_string_lossy(),
                io_error
            ))
        })?;
    timer6.measure();

    total.measure();
    timer1.display("codegen ll: buffer alloc 64kb");
    timer2.display("codegen ll: string_literals  ");
    timer3.display("codegen ll: struct_types     ");
    timer4.display("codegen ll: globals          ");
    timer5.display("codegen ll: write to file    ");
    timer6.display("codegen ll: clang build      ");
    total.display("codegen ll: total             ");
    Ok(())
}

fn codegen_string_literals(cg: &mut Codegen, hir: &hir::Hir) {
    for (idx, &string) in hir.intern_string.get_all_strings().iter().enumerate() {
        let c_string = hir.string_is_cstr[idx];

        let byte_len = string.len() + c_string as usize;
        //@immediate shouldn't be an option, look into this
        let array: hir::ArrayStatic = hir::ArrayStatic {
            len: hir::ArrayStaticLen::Immediate(Some(byte_len as u64)),
            elem_ty: hir::Type::Basic(ast::BasicType::U8),
        };

        identifier(
            cg,
            IdentifierKind::Global,
            Some("rock_string_lit"),
            idx as u32,
        );
        cg.space();
        cg.write('=');
        cg.space();

        //@hardcoded attributes for now
        cg.write_str("internal");
        cg.space();
        cg.write_str("unnamed_addr");
        cg.space();
        cg.write_str("constant");
        cg.space();

        array_type(cg, hir, &array);
        cg.space();
        string_literal(cg, string, c_string);
        cg.new_line();
    }
    cg.new_line();
}

fn string_literal(cg: &mut Codegen, string: &str, c_string: bool) {
    cg.write('c');
    cg.write('\"');
    for byte in string.bytes() {
        string_byte(cg, byte);
    }
    if c_string {
        string_byte(cg, 0);
    }
    cg.write('\"');
}

fn string_byte(cg: &mut Codegen, byte: u8) {
    match byte {
        0..=31 | 34 | 127 => {
            let lower = byte & 0x0F;
            let upper = (byte >> 4) & 0x0F;
            let lower_hex = nibble_to_hex(lower);
            let upper_hex = nibble_to_hex(upper);
            cg.write('\\');
            cg.write(upper_hex as char);
            cg.write(lower_hex as char);
        }
        b'\\' => {
            cg.write('\\');
            cg.write('\\');
        }
        _ => cg.write(byte as char),
    }
}

fn nibble_to_hex(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'A' + nibble - 10,
    }
}

fn codegen_struct_types(cg: &mut Codegen, hir: &hir::Hir) {
    for (idx, data) in hir.structs.iter().enumerate() {
        let name = hir.intern_name.get_str(data.name.id);
        identifier(cg, IdentifierKind::Local, Some(name), idx as u32);

        cg.space();
        cg.write('=');
        cg.space();

        cg.write_str("type");
        cg.space();
        cg.write('{');
        cg.space();

        for (field_idx, field) in data.fields.iter().enumerate() {
            ty(cg, hir, field.ty);
            if field_idx + 1 != data.fields.len() {
                cg.write(',');
            }
            cg.space();
        }

        cg.write('}');
        cg.new_line();
    }
    cg.new_line();
}

fn codegen_globals(cg: &mut Codegen, hir: &hir::Hir) {
    for (idx, data) in hir.globals.iter().enumerate() {
        let name = hir.intern_name.get_str(data.name.id);
        identifier(cg, IdentifierKind::Global, Some(name), idx as u32);

        cg.space();
        cg.write('=');
        cg.space();

        cg.write_str("internal");
        cg.space();
        match data.mutt {
            ast::Mut::Mutable => cg.write_str("global"),
            ast::Mut::Immutable => cg.write_str("constant"),
        }
        cg.space();

        ty(cg, hir, data.ty);
        cg.space();
        codegen_const_value(
            cg,
            hir,
            hir.const_eval_value(data.value),
            array_elem_ty(data.ty),
        );
        cg.new_line();
    }
    cg.new_line();
}

fn array_elem_ty(ty: hir::Type) -> Option<hir::Type> {
    match ty {
        hir::Type::ArrayStatic(array) => Some(array.elem_ty),
        _ => None,
    }
}

fn codegen_const_value(
    cg: &mut Codegen,
    hir: &hir::Hir,
    value: hir::ConstValue,
    elem_ty: Option<hir::Type>,
) {
    match value {
        hir::ConstValue::Error => unreachable!(),
        hir::ConstValue::Null => cg.write_str("null"),
        hir::ConstValue::Bool { val } => {
            if val {
                cg.write_str("true");
            } else {
                cg.write_str("false");
            }
        }
        hir::ConstValue::Int { val, ty, .. } => {
            let unsigned = matches!(
                ty,
                ast::BasicType::U8
                    | ast::BasicType::U16
                    | ast::BasicType::U32
                    | ast::BasicType::U64
                    | ast::BasicType::Usize
            );
            let prefix = if unsigned { 'u' } else { 's' };
            cg.write_str(&format!("{}0x{:x}", prefix, val)); //@allocation!
        }
        hir::ConstValue::IntS(_) => todo!("IntS is not implemented"),
        hir::ConstValue::IntU(_) => todo!("IntU is not implemented"),
        //@why is ty optional?
        hir::ConstValue::Float { val, ty } => {
            if ty == Some(ast::BasicType::F64) {
                cg.write_str(&format!("0x{:x}", val.to_bits())); //@allocation!
            } else {
                //@test by printf'ing them values are wrong must likely
                // llvm format for hex floats is busted + no clear docs
                let val_f32 = val as f32;
                cg.write_str(&format!("0x{:x}", val_f32.to_bits()));
                cg.write_str("00000000");
            }
        }
        hir::ConstValue::Char { val } => cg.write_str(&format!("u0x{:x}", val as u32)), //@allocation!
        hir::ConstValue::String { id, c_string } => {
            if c_string {
                identifier(
                    cg,
                    IdentifierKind::Global,
                    Some("rock_string_lit"),
                    id.raw(),
                );
            } else {
                cg.write('{');
                cg.space();
                cg.write_str("ptr");
                cg.space();
                identifier(
                    cg,
                    IdentifierKind::Global,
                    Some("rock_string_lit"),
                    id.raw(),
                );
                cg.write(',');
                cg.space();
                basic_type(cg, ast::BasicType::U64); //@assuming 64bit
                cg.space();

                let string = hir.intern_string.get_str(id);
                let byte_len = string.len(); //@not capturing \00 even if it exists (correct)
                cg.write_str(&format!("{}", byte_len)); //@allocation!

                cg.space();
                cg.write('}');
            }
        }
        hir::ConstValue::Procedure { proc_id } => cg.write_str("<const procedure>"),
        hir::ConstValue::EnumVariant {
            enum_id,
            variant_id,
        } => {
            let variant = hir.enum_data(enum_id).variant(variant_id);
            let value = hir.const_eval_value(variant.value);
            codegen_const_value(cg, hir, value, None);
        }
        hir::ConstValue::Struct { struct_ } => {
            cg.write('{');
            cg.space();
            let data = hir.struct_data(struct_.struct_id);
            for (idx, value_id) in struct_.fields.iter().enumerate() {
                let field = data.field(hir::StructFieldID::new(idx));
                ty(cg, hir, field.ty);
                cg.space();
                codegen_const_value(cg, hir, hir.const_value(*value_id), array_elem_ty(field.ty));
                if idx + 1 != struct_.fields.len() {
                    cg.write(',');
                }
                cg.space();
            }
            cg.write('}');
        }
        hir::ConstValue::Array { array } => {
            cg.write('[');
            let elem_ty = elem_ty.unwrap();
            for (idx, value_id) in array.values.iter().enumerate() {
                ty(cg, hir, elem_ty);
                cg.space();
                codegen_const_value(cg, hir, hir.const_value(*value_id), array_elem_ty(elem_ty));
                if idx + 1 != array.values.len() {
                    cg.write(',');
                    cg.space();
                }
            }
            cg.write(']');
        }
        hir::ConstValue::ArrayRepeat { value, len } => {
            cg.write('[');
            let elem_ty = elem_ty.unwrap();
            for idx in 0..len {
                //@repeated generation (wasteful, no way to specify array repeat in llvm)
                ty(cg, hir, elem_ty);
                cg.space();
                codegen_const_value(cg, hir, hir.const_value(value), array_elem_ty(elem_ty));
                if idx + 1 != len {
                    cg.write(',');
                    cg.space();
                }
            }
            cg.write(']');
        }
    }
}

enum IdentifierKind {
    Local,
    Global,
}

//@make sure ident versioning is used correctly
// in different scopes and no collisions are possible
fn identifier(cg: &mut Codegen, kind: IdentifierKind, name: Option<&str>, version: u32) {
    match kind {
        IdentifierKind::Local => cg.write('%'),
        IdentifierKind::Global => cg.write('@'),
    }
    if let Some(name) = name {
        cg.write_str(name);
        // only use dot for globals as llvm does?
        cg.write('.');
    }
    cg.write_str(&version.to_string()); //@allocation!
}

fn ty(cg: &mut Codegen, hir: &hir::Hir, ty: hir::Type) {
    match ty {
        hir::Type::Error => unreachable!(),
        hir::Type::Basic(basic) => basic_type(cg, basic),
        hir::Type::Enum(enum_id) => basic_type(cg, hir.enum_data(enum_id).basic),
        hir::Type::Struct(struct_id) => struct_type(cg, hir, struct_id),
        hir::Type::Reference(_, _) => cg.write_str("ptr"),
        hir::Type::Procedure(_) => cg.write_str("ptr"),
        hir::Type::ArraySlice(_) => cg.write_str("{ ptr, i64 }"),
        hir::Type::ArrayStatic(array) => array_type(cg, hir, array),
    }
}

fn basic_type(cg: &mut Codegen, basic: ast::BasicType) {
    let string = match basic {
        ast::BasicType::S8 => "i8",
        ast::BasicType::S16 => "i16",
        ast::BasicType::S32 => "i32",
        ast::BasicType::S64 => "i64",
        ast::BasicType::Ssize => "i64", //@assume 64bit
        ast::BasicType::U8 => "i8",
        ast::BasicType::U16 => "i16",
        ast::BasicType::U32 => "i32",
        ast::BasicType::U64 => "i64",
        ast::BasicType::Usize => "i64", //@assume 64bit
        ast::BasicType::F16 => "half",
        ast::BasicType::F32 => "float",
        ast::BasicType::F64 => "double",
        ast::BasicType::Bool => "i1",
        ast::BasicType::Char => "i32",
        ast::BasicType::Rawptr => "ptr",
        ast::BasicType::Void => "void",
        ast::BasicType::Never => "void", //@assumed to only be used in proc return_ty, not confirmed
    };
    cg.write_str(string);
}

fn struct_type(cg: &mut Codegen, hir: &hir::Hir, struct_id: hir::StructID) {
    let data = hir.struct_data(struct_id);
    let name = hir.intern_name.get_str(data.name.id);
    identifier(cg, IdentifierKind::Local, Some(name), struct_id.raw());
}

fn array_type(cg: &mut Codegen, hir: &hir::Hir, array: &hir::ArrayStatic) {
    let len = match array.len {
        hir::ArrayStaticLen::Immediate(len) => len.expect("array len is known"),
        hir::ArrayStaticLen::ConstEval(eval_id) => match hir.const_eval_value(eval_id) {
            hir::ConstValue::Int { val, .. } => val,
            _ => panic!("array len must be int"),
        },
    };

    cg.write('[');
    cg.write_str(&len.to_string()); //@allocation!
    cg.write_str(" x ");
    ty(cg, hir, array.elem_ty);
    cg.write(']');
}

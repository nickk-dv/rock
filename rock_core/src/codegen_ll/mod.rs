use crate::ast;
use crate::hir;
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

pub fn codegen_module<'ctx>(hir: hir::Hir<'ctx>) {
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
    total.measure();

    timer1.display("codegen ll: buffer pre allocation 64kb");
    timer2.display("codegen ll: string_literals");
    timer3.display("codegen ll: struct_types   ");
    total.display("codegen ll: total");

    let buffer = cg.finish();
    eprintln!("[CODEGEN LL]");
    eprint!("{}", buffer);
    eprintln!("[CODEGEN LL END]");
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
            write_type(cg, hir, field.ty);
            if !(field_idx + 1 == data.fields.len()) {
                cg.write(',');
            }
            cg.space();
        }

        cg.write('}');
        cg.new_line();
    }
    cg.new_line();
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

fn write_type(cg: &mut Codegen, hir: &hir::Hir, ty: hir::Type) {
    match ty {
        hir::Type::Error => unreachable!(),
        hir::Type::Basic(basic) => cg.write_str(basic_type(basic)),
        hir::Type::Enum(enum_id) => {
            let basic = hir.enum_data(enum_id).basic;
            cg.write_str(basic_type(basic));
        }
        hir::Type::Struct(struct_id) => {
            let data = hir.struct_data(struct_id);
            let name = hir.intern_name.get_str(data.name.id);
            identifier(cg, IdentifierKind::Local, Some(name), struct_id.raw());
        }
        hir::Type::Reference(_, _) => cg.write_str("ptr"),
        hir::Type::Procedure(_) => cg.write_str("ptr"),
        hir::Type::ArraySlice(_) => cg.write_str("{ ptr, i64 }"),
        hir::Type::ArrayStatic(array) => array_type(cg, hir, array),
    }
}

fn array_type(cg: &mut Codegen, hir: &hir::Hir, array: &hir::ArrayStatic) {
    let len = match array.len {
        hir::ArrayStaticLen::Immediate(len) => len.expect("array len is known"),
        hir::ArrayStaticLen::ConstEval(eval_id) => match hir.const_eval_value(eval_id) {
            hir::ConstValue::Int { val, neg, ty } => val,
            _ => panic!("array len must be int"),
        },
    };

    cg.write('[');
    cg.write_str(&len.to_string()); //@allocation!
    cg.write_str(" x ");
    write_type(cg, hir, array.elem_ty);
    cg.write(']');
}

const fn basic_type(basic: ast::BasicType) -> &'static str {
    match basic {
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
    }
}

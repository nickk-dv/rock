use crate::ast;
use crate::error::ErrorComp;
use crate::fs_env;
use crate::hir;
use crate::intern::InternID;
use crate::session::Session;
use crate::timer::Timer;

struct Codegen {
    buffer: String,
    builder: Builder,
    context: ProcContext,
}

struct Builder {
    named_version: u32,
    unnamed_version: u32,
}

struct ProcContext {
    param_values: Vec<LocalValue>,
    local_values: Vec<LocalValue>,
}

impl Codegen {
    fn new() -> Codegen {
        Codegen {
            buffer: String::with_capacity(1024 * 64),
            builder: Builder::new(),
            context: ProcContext::new(),
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

impl Builder {
    fn new() -> Builder {
        Builder {
            named_version: 0,
            unnamed_version: 0,
        }
    }

    fn reset(&mut self) {
        *self = Builder::new();
    }

    //@only using unnamed_version:
    #[must_use]
    fn build_local(&mut self) -> LocalValue {
        let value = LocalValue(self.unnamed_version);
        self.unnamed_version += 1;
        value
    }
}

impl ProcContext {
    fn new() -> ProcContext {
        ProcContext {
            param_values: Vec::new(),
            local_values: Vec::new(),
        }
    }
}

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }

    pub fn opt_level(self) -> &'static str {
        match self {
            BuildKind::Debug => "O0",
            BuildKind::Release => "O3",
        }
    }
}

#[allow(unused)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
enum TargetTriple {
    X86_64_Pc_Windows_Msvc,
    X86_64_Unknown_Linux_Gnu,
}

impl TargetTriple {
    fn as_str(self) -> &'static str {
        match self {
            TargetTriple::X86_64_Pc_Windows_Msvc => "x86_64-pc-windows-msvc",
            TargetTriple::X86_64_Unknown_Linux_Gnu => "x86_64-unknown-linux-gnu",
        }
    }

    fn default_host() -> TargetTriple {
        #[cfg(all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"))]
        const DEFAULT_HOST: TargetTriple = TargetTriple::X86_64_Pc_Windows_Msvc;
        #[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
        const DEFAULT_HOST: TargetTriple = TargetTriple::X86_64_Unknown_Linux_Gnu;

        // build can only succeed when build target
        // matches one of supported `rock` target triples
        DEFAULT_HOST
    }
}

pub fn codegen_module(session: &Session, hir: hir::Hir) -> Result<(), ErrorComp> {
    let mut total = Timer::new();
    let mut timer1 = Timer::new();
    let mut cg = Codegen::new();
    timer1.measure();

    //@set correct data_layout? or rely on default?
    let triple = TargetTriple::default_host();
    let triple_string = format!("target triple = \"{}\"", triple.as_str());
    cg.write_str(&triple_string);
    cg.new_line();
    cg.new_line();

    let mut timer3 = Timer::new();
    codegen_struct_types(&mut cg, &hir);
    cg.new_line();
    timer3.measure();

    let mut timer4 = Timer::new();
    codegen_globals(&mut cg, &hir);
    cg.new_line();
    timer4.measure();

    let mut timer2 = Timer::new();
    codegen_string_literals(&mut cg, &hir);
    timer2.measure();

    let mut timer5 = Timer::new();
    codegen_procedures(&mut cg, &hir);
    timer5.measure();

    let mut timer6 = Timer::new();
    let buffer_ll = cg.finish();
    let build_dir = session.cwd().join("build");
    fs_env::dir_create(&build_dir, false)?;
    let debug_dir = build_dir.join("debug");
    fs_env::dir_create(&debug_dir, false)?;
    let module_path = debug_dir.join("codegen_ll_test.ll");
    fs_env::file_create_or_rewrite(&module_path, &buffer_ll)?;
    timer6.measure();

    let executable_path = debug_dir.join("codegen_ll_test"); //@format not always exe

    //@args and names are not correctly setup, nor the debug, release mode
    // how to expose -v option when needed? (--verbose flag to show clang output always?)
    let mut timer7 = Timer::new();
    let args = vec![
        "-v".to_string(),
        "-fuse-ld=lld".to_string(),
        "-O0".into(),
        module_path.to_string_lossy().into(),
        "-o".into(),
        executable_path.to_string_lossy().into(),
    ];

    let output = std::process::Command::new("clang")
        .args(args)
        .output()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to build llvm ir module with clang: `{}`\nreason: {}",
                module_path.to_string_lossy(),
                io_error
            ))
        })?;

    if !output.status.success() {
        //assert that stdout is empty and remove stdout: stderr: if clang only prints to stder
        return Err(ErrorComp::message(format!(
            "clang build failed\nfull clang and linker output:\n\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }

    timer7.measure();

    total.measure();
    timer1.display("codegen ll: buffer alloc 64kb");
    timer3.display("codegen ll: struct_types     ");
    timer4.display("codegen ll: globals          ");
    timer2.display("codegen ll: string_literals  ");
    timer5.display("codegen ll: procedures       ");
    timer6.display("codegen ll: write to file    ");
    timer7.display("codegen ll: clang build      ");
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

        ident_string_lit(cg, InternID::new(idx));
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
    //@have some safer iteration with correct ID being returned instead of using iter().enumerate()?
    for (idx, data) in hir.structs.iter().enumerate() {
        ident_struct(cg, hir, hir::StructID::new(idx));

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
}

fn codegen_globals(cg: &mut Codegen, hir: &hir::Hir) {
    for (idx, data) in hir.globals.iter().enumerate() {
        ident_global(cg, hir, hir::GlobalID::new(idx));

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

        codegen_const_value(cg, hir, hir.const_eval_value(data.value), true);
        cg.new_line();
    }
}

fn codegen_const_value(cg: &mut Codegen, hir: &hir::Hir, value: hir::ConstValue, with_type: bool) {
    if with_type {
        const_value_type(cg, hir, value);
        if !matches!(value, hir::ConstValue::EnumVariant { .. }) {
            cg.space();
        }
    }

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
        hir::ConstValue::Int { val, int_ty, .. } => {
            let signed = int_ty.is_signed();
            let prefix = if signed { 's' } else { 'u' };
            cg.write_str(&format!("{}0x{:x}", prefix, val)); //@allocation!
        }
        hir::ConstValue::IntS(_) => todo!("IntS is not implemented"),
        hir::ConstValue::IntU(_) => todo!("IntU is not implemented"),
        hir::ConstValue::Float { val, float_ty } => {
            if float_ty == hir::BasicFloat::F64 {
                cg.write_str(&format!("0x{:x}", val.to_bits())); //@allocation!
            } else {
                //@test by printf'ing them values are wrong must likely
                // llvm format for hex floats is busted + no clear docs
                let val_f32 = val as f32;
                cg.write_str(&format!("0x{:x}", val_f32.to_bits())); //@allocation!
                cg.write_str("00000000");
            }
        }
        hir::ConstValue::Char { val } => cg.write_str(&format!("u0x{:x}", val as u32)), //@allocation!
        hir::ConstValue::String { id, c_string } => {
            if c_string {
                ident_string_lit(cg, id);
            } else {
                cg.write('{');
                cg.space();
                cg.write_str("ptr");
                cg.space();
                ident_string_lit(cg, id);
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
        hir::ConstValue::EnumVariant { enum_ } => {
            todo!("variant codegen");
            //let variant = hir.enum_data(enum_id).variant(variant_id);
            //@disabled
            //let value = hir.const_eval_value(variant.value);
            //codegen_const_value(cg, hir, value, with_type);
        }
        hir::ConstValue::Struct { struct_ } => {
            cg.write('{');
            cg.space();
            for (idx, value_id) in struct_.fields.iter().enumerate() {
                codegen_const_value(cg, hir, hir.const_value(*value_id), with_type);
                if idx + 1 != struct_.fields.len() {
                    cg.write(',');
                }
                cg.space();
            }
            cg.write('}');
        }
        hir::ConstValue::Array { array } => {
            cg.write('[');
            for (idx, value_id) in array.values.iter().enumerate() {
                codegen_const_value(cg, hir, hir.const_value(*value_id), with_type);
                if idx + 1 != array.values.len() {
                    cg.write(',');
                    cg.space();
                }
            }
            cg.write(']');
        }
        hir::ConstValue::ArrayRepeat { value, len } => {
            cg.write('[');
            for idx in 0..len {
                //@repeated generation (wasteful, no way to specify array repeat in llvm)
                codegen_const_value(cg, hir, hir.const_value(value), with_type);
                if idx + 1 != len {
                    cg.write(',');
                    cg.space();
                }
            }
            cg.write(']');
        }
    }
}

//@type needs to be inferred from constant itself
// (problem with empty array types)
// type is part of the constant value in llvm syntax
fn const_value_type(cg: &mut Codegen, hir: &hir::Hir, value: hir::ConstValue) {
    match value {
        hir::ConstValue::Error => unreachable!(),
        hir::ConstValue::Null => basic_type(cg, ast::BasicType::Rawptr),
        hir::ConstValue::Bool { .. } => basic_type(cg, ast::BasicType::Bool),
        hir::ConstValue::Int { int_ty, .. } => basic_type(cg, int_ty.into_basic()),
        hir::ConstValue::IntS(_) => todo!(),
        hir::ConstValue::IntU(_) => todo!(),
        hir::ConstValue::Float { float_ty, .. } => basic_type(cg, float_ty.into_basic()),
        hir::ConstValue::Char { .. } => basic_type(cg, ast::BasicType::U32),
        hir::ConstValue::String { c_string, .. } => {
            if c_string {
                basic_type(cg, ast::BasicType::Rawptr);
            } else {
                slice_type(cg);
            }
        }
        hir::ConstValue::Procedure { .. } => basic_type(cg, ast::BasicType::Rawptr),
        hir::ConstValue::EnumVariant { .. } => {}
        hir::ConstValue::Struct { struct_ } => struct_type(cg, hir, struct_.struct_id),
        hir::ConstValue::Array { array } => {
            //@zero sized array types will wont be generated 05.07.24
            // since type cannot be infered, zero sized arrays are currently
            // allowed in other parts or the compiler, but they are problematic
            // and might be removed + array repeat with 0 len is also semantically problematic
            // since value wont be stored anywere after its evaluated
            let first_value_id = array.values[0];
            let value = hir.const_value(first_value_id);

            cg.write('[');
            cg.write_str(&format!("{}", array.values.len())); //@allocation!
            cg.write_str(" x ");
            //@will break with enum variants, fix enum variant generation behavior
            const_value_type(cg, hir, value);
            cg.write(']');
        }
        hir::ConstValue::ArrayRepeat { value, len } => {
            let value = hir.const_value(value);

            cg.write('[');
            cg.write_str(&format!("{}", len)); //@allocation!
            cg.write_str(" x ");
            //@will break with enum variants, fix enum variant generation behavior
            const_value_type(cg, hir, value);
            cg.write(']');
        }
    }
}

fn ident_string_lit(cg: &mut Codegen, id: InternID) {
    cg.write('@');
    cg.write_str("string");
    cg.write('.');
    cg.write('l');
    cg.write_str(&format!("{}", id.raw())); //@allocation!
}

fn ident_struct(cg: &mut Codegen, hir: &hir::Hir, id: hir::StructID) {
    let name_id = hir.struct_data(id).name.id;
    let name = hir.intern_name.get_str(name_id);
    cg.write('%');
    cg.write_str(name);
    cg.write('.');
    cg.write('s');
    cg.write_str(&format!("{}", id.raw())); //@allocation!
}

fn ident_global(cg: &mut Codegen, hir: &hir::Hir, id: hir::GlobalID) {
    let name_id = hir.global_data(id).name.id;
    let name = hir.intern_name.get_str(name_id);
    cg.write('@');
    cg.write_str(name);
    cg.write('.');
    cg.write('g');
    cg.write_str(&format!("{}", id.raw())); //@allocation!
}

fn ident_procedure(cg: &mut Codegen, hir: &hir::Hir, id: hir::ProcID) {
    let data = hir.proc_data(id);
    let name = hir.intern_name.get_str(data.name.id);
    let raw_name = data.attr_set.contains(hir::ProcFlag::External)
        || data.attr_set.contains(hir::ProcFlag::Main);

    cg.write('@');
    cg.write_str(name);
    if !raw_name {
        cg.write('.');
        cg.write_str(&format!("{}", id.raw())); //@allocation!
    }
}

//@fully avoids name conflits with globals
// have some named variation for named locals
fn ident_local_value(cg: &mut Codegen, value: LocalValue) {
    cg.write('%');
    cg.write_str(&format!("{}", value.0)); //@allocation!
}

fn write_value(cg: &mut Codegen, hir: &hir::Hir, value: Value, with_type: bool) {
    match value {
        Value::Local(value) => ident_local_value(cg, value),
        Value::Const(value) => codegen_const_value(cg, hir, value, with_type),
    }
}

#[derive(Copy, Clone)]
struct LocalValue(u32);

#[derive(Copy, Clone)]
enum Value<'hir> {
    Local(LocalValue),
    Const(hir::ConstValue<'hir>),
}

//@test if this is faster for numbers
trait CodegenWriter {
    fn write(self, cg: &mut Codegen);
}

impl CodegenWriter for u32 {
    #[allow(unsafe_code)]
    fn write(mut self, cg: &mut Codegen) {
        const MAX_DIGITS: usize = 10;
        let mut buffer = [0u8; MAX_DIGITS];
        let mut i = MAX_DIGITS;

        if self == 0 {
            buffer[MAX_DIGITS - 1] = b'0';
            i = MAX_DIGITS - 1;
        } else {
            while self > 0 {
                i -= 1;
                let digit = b'0' + (self % 10) as u8;
                unsafe {
                    *buffer.get_unchecked_mut(i) = digit;
                }
                self /= 10;
            }
        }

        let bytes = unsafe { buffer.get_unchecked(i..) };
        let string: &str = unsafe { std::str::from_utf8_unchecked(bytes) };
        cg.write_str(string)
    }
}

impl CodegenWriter for u64 {
    #[allow(unsafe_code)]
    fn write(mut self, cg: &mut Codegen) {
        const MAX_DIGITS: usize = 20;
        let mut buffer = [0u8; MAX_DIGITS];
        let mut i: usize = MAX_DIGITS;

        if self == 0 {
            buffer[MAX_DIGITS - 1] = b'0';
            i = MAX_DIGITS - 1;
        } else {
            while self > 0 {
                i -= 1;
                let digit = b'0' + (self % 10) as u8;
                unsafe {
                    *buffer.get_unchecked_mut(i) = digit;
                }
                self /= 10;
            }
        }

        let bytes = unsafe { buffer.get_unchecked(i..) };
        let string: &str = unsafe { std::str::from_utf8_unchecked(bytes) };
        cg.write_str(string)
    }
}

fn ty(cg: &mut Codegen, hir: &hir::Hir, ty: hir::Type) {
    match ty {
        hir::Type::Error => unreachable!(),
        hir::Type::Basic(basic) => basic_type(cg, basic),
        hir::Type::Enum(enum_id) => basic_type(cg, hir.enum_data(enum_id).int_ty.into_basic()),
        hir::Type::Struct(struct_id) => struct_type(cg, hir, struct_id),
        hir::Type::Reference(_, _) => cg.write_str("ptr"),
        hir::Type::Procedure(_) => cg.write_str("ptr"),
        hir::Type::ArraySlice(_) => slice_type(cg),
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
    ident_struct(cg, hir, struct_id);
}

fn slice_type(cg: &mut Codegen) {
    cg.write_str("{ ptr, i64 }");
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

fn codegen_procedures(cg: &mut Codegen, hir: &hir::Hir) {
    for (idx, data) in hir.procs.iter().enumerate() {
        let external = data.attr_set.contains(hir::ProcFlag::External);
        let variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
        let main = data.attr_set.contains(hir::ProcFlag::Main);

        //@hack to compile
        if !(main || external) {
            continue;
        }

        cg.new_line();
        if external {
            cg.write_str("declare");
        } else {
            cg.write_str("define");
            if !main {
                cg.space();
                cg.write_str("internal");
            }
        }

        cg.space();
        ty(cg, hir, data.return_ty);
        cg.space();
        ident_procedure(cg, hir, hir::ProcID::new(idx));

        cg.write('(');
        for (param_idx, param) in data.params.iter().enumerate() {
            ty(cg, hir, param.ty);
            if !external {
                cg.space();
                let value = cg.builder.build_local();
                ident_local_value(cg, value);
            }
            if variadic || param_idx + 1 != data.params.len() {
                cg.write(',');
                cg.space();
            }
        }
        if variadic {
            cg.write_str("...");
        }
        cg.write(')');

        if let Some(block) = data.block {
            cg.space();
            cg.write('{');
            cg.new_line();

            basic_block(cg, "entry");
            codegen_block(cg, hir, block);
            cg.write('}');
        }

        cg.new_line();
    }
}

//@might change the approach of how tail alloca works
// and instead return optional value directly from the block
fn codegen_block(cg: &mut Codegen, hir: &hir::Hir, block: hir::Block) {
    for stmt in block.stmts {
        match *stmt {
            hir::Stmt::Break => todo!(),
            hir::Stmt::Continue => todo!(),
            //@defer blocks (all_defer_blocks)
            hir::Stmt::Return(expr) => {
                let value = if let Some(expr) = expr {
                    codegen_expr(cg, hir, expr) //@value_optional (to support)
                } else {
                    None
                };
                inst_ret(cg, hir, value);
            }
            hir::Stmt::Defer(_) => todo!(),
            hir::Stmt::Loop(_) => todo!(),
            hir::Stmt::Local(_) => todo!(),
            hir::Stmt::Assign(_) => todo!(),
            hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, hir, expr),
            hir::Stmt::ExprTail(_) => todo!(),
        }
    }
}

fn codegen_expr_semi(cg: &mut Codegen, hir: &hir::Hir, expr: &hir::Expr) {
    codegen_expr(cg, hir, expr); // !expect_ptr + tail ignore
}

fn codegen_expr<'hir>(
    cg: &mut Codegen,
    hir: &hir::Hir,
    expr: &'hir hir::Expr,
) -> Option<Value<'hir>> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { value } => Some(Value::Const(value)),
        hir::Expr::If { if_ } => todo!(),
        hir::Expr::Block { block } => {
            codegen_block(cg, hir, block);
            None
        }
        hir::Expr::Match { match_ } => todo!(),
        hir::Expr::Match2 { match_ } => todo!(),
        hir::Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => todo!(),
        hir::Expr::SliceField {
            target,
            field,
            deref,
        } => todo!(),
        hir::Expr::Index { target, access } => todo!(),
        hir::Expr::Slice { target, access } => todo!(),
        hir::Expr::Cast { target, into, kind } => todo!(),
        hir::Expr::LocalVar { local_id } => todo!(),
        hir::Expr::ParamVar { param_id } => todo!(),
        hir::Expr::ConstVar { const_id } => todo!(),
        hir::Expr::GlobalVar { global_id } => todo!(),
        hir::Expr::Variant {
            enum_id,
            variant_id,
            input,
        } => todo!(),
        hir::Expr::CallDirect { proc_id, input } => codegen_call_direct(cg, hir, proc_id, input),
        hir::Expr::CallIndirect { target, indirect } => todo!(),
        hir::Expr::StructInit { struct_id, input } => todo!(),
        hir::Expr::ArrayInit { array_init } => todo!(),
        hir::Expr::ArrayRepeat { array_repeat } => todo!(),
        hir::Expr::Deref { rhs, ptr_ty } => todo!(),
        hir::Expr::Address { rhs } => todo!(),
        hir::Expr::Unary { op, rhs } => Some(codegen_unary(cg, hir, op, rhs)),
        hir::Expr::Binary {
            op,
            lhs,
            rhs,
            lhs_signed_int,
        } => todo!(),
    }
}

fn codegen_call_direct<'hir>(
    cg: &mut Codegen,
    hir: &hir::Hir,
    proc_id: hir::ProcID,
    input: &[&hir::Expr],
) -> Option<Value<'hir>> {
    let data = hir.proc_data(proc_id);
    let variadic = data.attr_set.contains(hir::ProcFlag::Variadic);
    let returns = !(data.return_ty.is_void() && data.return_ty.is_never());

    //@collect values (skip constants and directly write them instead)
    for &expr in input {
        if let hir::Expr::Const { .. } = expr {
            continue;
        } else {
            panic!("value expr not supported");
        }
    }

    cg.tab();
    let value = if returns {
        let value = cg.builder.build_local();
        ident_local_value(cg, value);
        cg.write_str(" = ");
        Some(Value::Local(value))
    } else {
        None
    };

    cg.write_str("call");
    cg.space();
    ty(cg, hir, data.return_ty);
    cg.space();

    if variadic {
        cg.write('(');
        for param in data.params {
            ty(cg, hir, param.ty);
            cg.write(',');
            cg.space();
        }
        cg.write_str("...");
        cg.write(')');
        cg.space();
    }
    ident_procedure(cg, hir, proc_id);

    cg.write('(');
    for (idx, &expr) in input.iter().enumerate() {
        if let hir::Expr::Const { value } = expr {
            codegen_const_value(cg, hir, *value, true);
        } else {
            panic!("value expr not supported");
        }
        if idx + 1 != input.len() {
            cg.write(',');
            cg.space();
        }
    }
    cg.write(')');
    cg.new_line();

    value
}

fn codegen_call_indirect(
    cg: &mut Codegen,
    hir: &hir::Hir,
    target: &hir::Expr,
    indirect: &hir::CallIndirect,
) {
}

fn codegen_unary<'hir>(
    cg: &mut Codegen,
    hir: &hir::Hir,
    op: hir::UnOp,
    rhs: &hir::Expr,
) -> Value<'hir> {
    let rhs = codegen_expr(cg, hir, rhs).unwrap(); //@expect value

    match op {
        hir::UnOp::Neg_Int(sint_ty) => {
            let lhs = Value::Const(hir::ConstValue::Int {
                val: 0,
                neg: false,
                int_ty: sint_ty.into_int(),
            });
            inst_bin_common(cg, hir, InstBinary::Sub, sint_ty.into_basic(), lhs, rhs)
        }
        hir::UnOp::Neg_Float(float_ty) => inst_fneg(cg, hir, float_ty, rhs),
        hir::UnOp::BitNot(int_ty) => {
            let max_value: u64 = match int_ty {
                hir::BasicInt::S8 => u8::MAX.into(),
                hir::BasicInt::S16 => u16::MAX.into(),
                hir::BasicInt::S32 => u32::MAX.into(),
                hir::BasicInt::S64 => u64::MAX,
                hir::BasicInt::Ssize => u64::MAX,
                hir::BasicInt::U8 => u8::MAX.into(),
                hir::BasicInt::U16 => u16::MAX.into(),
                hir::BasicInt::U32 => u32::MAX.into(),
                hir::BasicInt::U64 => u64::MAX,
                hir::BasicInt::Usize => u64::MAX,
            };

            let lhs = Value::Const(hir::ConstValue::Int {
                val: max_value,
                neg: false,
                int_ty: int_ty,
            });
            inst_bin_common(cg, hir, InstBinary::Xor, int_ty.into_basic(), lhs, rhs)
        }
        hir::UnOp::LogicNot => {
            let lhs = Value::Const(hir::ConstValue::Bool { val: true });
            inst_bin_common(cg, hir, InstBinary::Xor, ast::BasicType::Bool, lhs, rhs)
        }
    }
}

fn basic_block(cg: &mut Codegen, name: &str) {
    cg.write_str(name);
    cg.write(':');
    cg.new_line();
}

fn inst_ret(cg: &mut Codegen, hir: &hir::Hir, value: Option<Value>) {
    cg.tab();
    cg.write_str("ret");
    cg.space();
    if let Some(value) = value {
        write_value(cg, hir, value, true);
    } else {
        basic_type(cg, ast::BasicType::Void);
    }
    cg.new_line();
}

#[must_use]
fn inst_alloca<'hir>(cg: &mut Codegen, hir: &hir::Hir, alloc_ty: hir::Type) -> Value<'hir> {
    cg.tab();
    let value = cg.builder.build_local();
    ident_local_value(cg, value);
    cg.write_str(" = ");

    cg.write_str("alloca");
    cg.space();
    ty(cg, hir, alloc_ty);

    cg.new_line();
    Value::Local(value)
}

#[must_use]
fn inst_load<'hir>(cg: &mut Codegen, hir: &hir::Hir, ptr_ty: hir::Type, ptr: Value) -> Value<'hir> {
    cg.tab();
    let value = cg.builder.build_local();
    ident_local_value(cg, value);
    cg.write_str(" = ");

    cg.write_str("load");
    cg.space();
    ty(cg, hir, ptr_ty);
    cg.write(',');
    cg.space();
    write_value(cg, hir, ptr, true);

    cg.new_line();
    Value::Local(value)
}

fn inst_store(cg: &mut Codegen, hir: &hir::Hir, ptr: Value, value: Value) {
    cg.tab();
    cg.write_str("store");
    cg.space();
    write_value(cg, hir, ptr, true);
    cg.write(',');
    cg.space();
    write_value(cg, hir, value, true);
    cg.new_line();
}

//@temp block name handling
// use `name.bb3` format
fn inst_br(cg: &mut Codegen, dest: &str) {
    cg.tab();
    cg.write_str("br");
    cg.space();

    cg.write_str("label");
    cg.space();
    cg.write_str(dest); //@missing %, use block ident

    cg.new_line();
}

fn inst_br_cond(cg: &mut Codegen, hir: &hir::Hir, cond: Value, dest_true: &str, dest_false: &str) {
    cg.tab();
    cg.write_str("br");
    cg.space();
    basic_type(cg, ast::BasicType::Bool);
    cg.space();
    write_value(cg, hir, cond, false);

    cg.write(',');
    cg.space();
    cg.write_str("label");
    cg.space();
    cg.write_str(dest_true); //@missing %, use block ident

    cg.write(',');
    cg.space();
    cg.write_str("label");
    cg.space();
    cg.write_str(dest_false); //@missing %, use block ident

    cg.new_line();
}

fn inst_fneg<'hir>(
    cg: &mut Codegen,
    hir: &hir::Hir,
    float_ty: hir::BasicFloat,
    rhs: Value,
) -> Value<'hir> {
    cg.tab();
    let value = cg.builder.build_local();
    ident_local_value(cg, value);
    cg.write_str(" = ");

    cg.write_str("fneg");
    cg.space();
    basic_type(cg, float_ty.into_basic());
    cg.space();
    write_value(cg, hir, rhs, false); //@only false case

    cg.new_line();
    Value::Local(value)
}

#[derive(Copy, Clone)]
enum InstBinary {
    Xor,
    Sub,
}

impl InstBinary {
    fn as_str(self) -> &'static str {
        match self {
            InstBinary::Xor => "xor",
            InstBinary::Sub => "sub",
        }
    }
}

fn inst_bin_common<'hir>(
    cg: &mut Codegen,
    hir: &hir::Hir,
    inst: InstBinary,
    basic: ast::BasicType,
    lhs: Value,
    rhs: Value,
) -> Value<'hir> {
    cg.tab();
    let value = cg.builder.build_local();
    ident_local_value(cg, value);
    cg.write_str(" = ");

    cg.write_str(inst.as_str());
    cg.space();
    basic_type(cg, basic);
    cg.space();

    write_value(cg, hir, lhs, false);
    cg.write(',');
    cg.space();
    write_value(cg, hir, rhs, false);

    cg.new_line();
    Value::Local(value)
}

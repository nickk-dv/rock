use crate::ast::ast::*;
use crate::ast::token::Token;
use crate::mem::*;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

// experimental output of single module to .c file
// without prior corectness checking or analysis

pub fn c_out(ast: P<Ast>) {
    println!("emitting experimental c");
    let mut cout = Cout::new(ast);
    cout.emit();
    cout.write_back(PathBuf::from("test/main.c"));
}

struct Cout {
    ast: P<Ast>,
    buf: Vec<u8>,
}

impl Cout {
    fn new(ast: P<Ast>) -> Self {
        Self {
            ast,
            buf: Vec::new(),
        }
    }

    fn write_back(&self, path: PathBuf) {
        let mut file = fs::File::create(path).expect("Failed to create file");
        file.write_all(&self.buf).expect("Failed to write to file");
    }

    fn emit(&mut self) {
        self.emit_module(self.ast.modules[0].copy());
    }

    fn emit_token(&mut self, token: Token) {
        let str = Token::as_str(token);
        self.buf.extend_from_slice(str.as_bytes());
    }

    fn space(&mut self) {
        self.buf.push(b' ');
    }

    fn newline(&mut self) {
        self.buf.push(b'\n');
    }

    fn emit_str(&mut self, str: &'static str) {
        self.buf.extend_from_slice(str.as_bytes());
    }

    fn emit_ident(&mut self, name: Ident) {
        let bytes = self.ast.intern_pool.get_byte_slice(name.id);
        self.buf.extend_from_slice(bytes);
    }

    fn emit_module(&mut self, module: P<Module>) {
        self.emit_basic_typedefs();
        for decl in module.decls {
            self.emit_decl(decl);
        }
    }

    fn emit_basic_typedefs(&mut self) {
        self.emit_str("#include <stdio.h>\n\n"); //@for printf
        self.emit_str("#include <stdint.h>\n\n"); //@for stdint types

        self.emit_str("typedef uint8_t bool;\n");

        self.emit_str("typedef uint8_t s8;\n");
        self.emit_str("typedef uint16_t s16;\n");
        self.emit_str("typedef uint32_t s32;\n");
        self.emit_str("typedef uint64_t s64;\n");
        self.emit_str("typedef uint64_t ssize;\n"); //@target dep

        self.emit_str("typedef int8_t u8;\n");
        self.emit_str("typedef int16_t u16;\n");
        self.emit_str("typedef int32_t u32;\n");
        self.emit_str("typedef int64_t u64;\n");
        self.emit_str("typedef int64_t usize;\n"); //@target dep

        self.emit_str("typedef float f32;\n");
        self.emit_str("typedef double f64;\n");
        self.emit_str("typedef uint32_t char_utf8;\n");
        self.emit_str("typedef void* rawptr;\n");
    }

    // @ will emit declarations in order, non important initially
    fn emit_decl(&mut self, decl: Decl) {
        match decl {
            Decl::Mod(_) => {}
            Decl::Proc(proc_decl) => self.emit_proc_decl(proc_decl),
            Decl::Impl(_) => {}
            Decl::Enum(_) => {}
            Decl::Struct(struct_decl) => self.emit_struct_decl(struct_decl),
            Decl::Global(_) => {}
            Decl::Import(_) => {}
        }
    }

    fn emit_struct_decl(&mut self, struct_decl: P<StructDecl>) {
        if struct_decl.generic_params.is_some() {
            println!("cant emit generic struct");
            return;
        }

        self.emit_str("\ntypedef struct ");
        self.emit_ident(struct_decl.name);
        self.emit_str(" {\n");
        for field in struct_decl.fields.iter() {
            self.emit_str("\t");
            self.emit_type(field.ty);
            self.emit_str(" ");
            self.emit_ident(field.name);
            self.emit_str(";\n");
        }
        self.emit_str("} ");
        self.emit_ident(struct_decl.name);
        self.emit_str(";\n");
    }

    fn emit_type(&mut self, ty: Type) {
        match ty.kind {
            TypeKind::Basic(basic) => self.emit_basic_type(basic),
            TypeKind::Custom(custom) => {}
            TypeKind::ArraySlice(array_slice) => {}
            TypeKind::ArrayStatic(array_static) => {}
        }
    }

    fn emit_basic_type(&mut self, basic: BasicType) {
        match basic {
            BasicType::Bool => self.emit_token(Token::KwBool),
            BasicType::S8 => self.emit_token(Token::KwS8),
            BasicType::S16 => self.emit_token(Token::KwS16),
            BasicType::S32 => self.emit_token(Token::KwS32),
            BasicType::S64 => self.emit_token(Token::KwS64),
            BasicType::Ssize => self.emit_token(Token::KwSsize),
            BasicType::U8 => self.emit_token(Token::KwU8),
            BasicType::U16 => self.emit_token(Token::KwU16),
            BasicType::U32 => self.emit_token(Token::KwU32),
            BasicType::U64 => self.emit_token(Token::KwU64),
            BasicType::Usize => self.emit_token(Token::KwUsize),
            BasicType::F32 => self.emit_token(Token::KwF32),
            BasicType::F64 => self.emit_token(Token::KwF64),
            BasicType::Char => self.emit_str("char_utf8"),
            BasicType::Rawptr => self.emit_token(Token::KwRawptr),
        }
    }

    fn emit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {
        self.newline();
        if let Some(ty) = proc_decl.return_type {
            self.emit_type(ty);
        } else {
            self.emit_str("void")
        }
        self.space();
        self.emit_ident(proc_decl.name);
        self.emit_str("() { printf(\"hello c\"); getchar(); return 0; } \n");
        self.emit_block(proc_decl.block.unwrap()); //@unwrap
    }

    fn emit_block(&mut self, block: P<Block>) {
        //for stmt in block.stmts {
        //    match stmt.kind {
        //        StmtKind::For(_) => todo!(),
        //        StmtKind::Defer(_) => todo!(),
        //        StmtKind::Break => todo!(),
        //        StmtKind::Return(_) => todo!(),
        //        StmtKind::Continue => todo!(),
        //        StmtKind::ExprStmt(v) => todo!(),
        //        StmtKind::Assignment(_) => todo!(),
        //        StmtKind::VarDecl(_) => todo!(),
        //        StmtKind::ProcCall(_) => {}
        //    }
        //}
    }
}

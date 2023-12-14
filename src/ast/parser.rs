use super::*;
use crate::mem::*;
use std::path::PathBuf;

pub struct Parser {
    arena: Arena,
    peek_index: usize,
    tokens: Vec<TokenSpan>,
    sources: Vec<String>,
}

pub struct ParseError {
    context: ParseContext,
    expected: Vec<Token>,
    got_token: TokenSpan,
}

#[derive(Copy, Clone)]
pub enum ParseContext {
    ModuleAccess,
    Type,
    CustomType,
    ArraySliceType,
    ArrayStaticType,
    Decl,
    ModDecl,
    ProcDecl,
    ProcParam,
    EnumDecl,
    EnumVariant,
    StructDecl,
    StructField,
    GlobalDecl,
    ImportDecl,
    Stmt,
    If,
    For,
    Block,
    Defer,
    Break,
    Switch,
    SwitchCase,
    Return,
    Continue,
    VarDecl,
    VarAssign,
    Expr,
    Var,
    Access,
    ArrayAccess,
    Enum,
    Cast,
    Sizeof,
    Literal,
    ProcCall,
    ArrayInit,
    StructInit,
}

impl ParseError {
    pub fn print_temp(&self) {
        print!("parse error: expected: ");
        for token in self.expected.iter() {
            print!("'{}' ", token::Token::as_str(*token))
        }
        println!(
            "\nin {} got: {}",
            self.context.as_str(),
            token::Token::as_str(self.got_token.token)
        );
        println!(
            "span: {} - {}",
            self.got_token.span.start, self.got_token.span.end
        );
    }
}

impl ParseContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseContext::ModuleAccess => "module access",
            ParseContext::Type => "type",
            ParseContext::CustomType => "custom type",
            ParseContext::ArraySliceType => "array slice type",
            ParseContext::ArrayStaticType => "static array type",
            ParseContext::Decl => "declaration",
            ParseContext::ModDecl => "module declaration",
            ParseContext::ProcDecl => "procedure declaration",
            ParseContext::ProcParam => "procedure parameter",
            ParseContext::EnumDecl => "enum declaration",
            ParseContext::EnumVariant => "enum variant",
            ParseContext::StructDecl => "struct declaration",
            ParseContext::StructField => "struct field",
            ParseContext::GlobalDecl => "global declaration",
            ParseContext::ImportDecl => "import declaration",
            ParseContext::Stmt => "statement",
            ParseContext::If => "if statement",
            ParseContext::For => "for loop statement",
            ParseContext::Block => "statement block",
            ParseContext::Defer => "defer statement",
            ParseContext::Break => "break statement",
            ParseContext::Switch => "switch statement",
            ParseContext::SwitchCase => "switch cast",
            ParseContext::Return => "return statement",
            ParseContext::Continue => "continue statement",
            ParseContext::VarDecl => "variable declaration",
            ParseContext::VarAssign => "variable assignment",
            ParseContext::Expr => "expression",
            ParseContext::Var => "variable",
            ParseContext::Access => "access chain",
            ParseContext::ArrayAccess => "array access",
            ParseContext::Enum => "enum expression",
            ParseContext::Cast => "cast expression",
            ParseContext::Sizeof => "sizeof expression",
            ParseContext::Literal => "literal",
            ParseContext::ProcCall => "procedure call",
            ParseContext::ArrayInit => "array initializer",
            ParseContext::StructInit => "struct initializer",
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(1024 * 1024 * 4),
            peek_index: 0,
            tokens: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn error(&self, context: ParseContext, expected: Vec<Token>) -> ParseError {
        ParseError {
            context,
            expected,
            got_token: TokenSpan {
                token: self.peek(),
                span: self.peek_span(),
            },
        }
    }

    pub fn parse_package(&mut self) -> Result<P<Package>, ()> {
        let mut path = PathBuf::new();
        path.push("main.txt");

        match std::fs::read_to_string(&path) {
            Ok(string) => {
                let mut package = self.arena.alloc::<Package>();
                self.set_source_file(string);
                match self.parse_module() {
                    Ok(module) => package.root = module,
                    Err(error) => {
                        error.print_temp();
                        let span = self.peek_span();
                        self.print_substring(span.start, span.end);
                        return Err(());
                    }
                }
                Ok(package)
            }
            Err(err) => {
                println!("file open error: {} path: {}", err, path.display());
                Err(())
            }
        }
    }

    pub fn print_substring(&self, start: u32, end: u32) {
        //@use for error reporting
        if let Some(substring) = self
            .sources
            .last()
            .unwrap()
            .get(start as usize..end as usize)
        {
            println!("Substring: {}", substring);
        } else {
            println!("Invalid range for substring");
        }

        if let Some(substring) = self
            .sources
            .last()
            .unwrap()
            .get((start - 10) as usize..(end + 10) as usize)
        {
            println!("Substring semi expanded: {}", substring);
        } else {
            println!("Invalid range for semi expanded substring");
        }

        if let Some(substring) = self
            .sources
            .last()
            .unwrap()
            .get((start - 20) as usize..(end + 20) as usize)
        {
            println!("Substring expanded: {}", substring);
        } else {
            println!("Invalid range for expanded substring");
        }
    }

    fn set_source_file(&mut self, string: String) {
        self.peek_index = 0;
        let mut lexer = Lexer::new(&string);
        self.tokens = lexer.lex();
        self.sources.push(string);
    }

    fn parse_module(&mut self) -> Result<P<Module>, ParseError> {
        let mut module = self.arena.alloc::<Module>();
        while self.peek() != Token::Eof {
            let decl = self.parse_decl()?;
            module.decls.add(&mut self.arena, decl);
        }
        Ok(module)
    }

    fn parse_ident(&mut self, context: ParseContext) -> Result<Ident, ParseError> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.consume();
            return Ok(Ident { span });
        }
        Err(self.error(context, vec![Token::Ident]))
    }

    fn parse_module_access(&mut self) -> Option<ModuleAccess> {
        if self.peek() != Token::Ident && self.peek_next(1) != Token::ColonColon {
            return None;
        }
        let mut module_access = ModuleAccess { names: List::new() };
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = unsafe {
                self.parse_ident(ParseContext::ModuleAccess)
                    .unwrap_unchecked()
            };
            self.consume();
            module_access.names.add(&mut self.arena, name);
        }
        Some(module_access)
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut tt = Type {
            pointer_level: 0,
            kind: TypeKind::Basic(BasicType::Bool),
        };
        while self.try_consume(Token::Times) {
            tt.pointer_level += 1;
        }
        if let Some(basic_type) = self.try_consume_basic_type() {
            tt.kind = TypeKind::Basic(basic_type);
            return Ok(tt);
        }
        match self.peek() {
            Token::Ident => {
                tt.kind = TypeKind::Custom(self.parse_custom_type()?);
                Ok(tt)
            }
            Token::OpenBracket => {
                tt.kind = match self.peek_next(1) {
                    Token::CloseBracket => TypeKind::ArraySlice(self.parse_array_slice_type()?),
                    _ => TypeKind::ArrayStatic(self.parse_array_static_type()?),
                };
                Ok(tt)
            }
            _ => Err(self.error(ParseContext::Type, vec![Token::Ident, Token::OpenBracket])),
        }
    }

    fn parse_custom_type(&mut self) -> Result<CustomType, ParseError> {
        let module_access = self.parse_module_access();
        let name = self.parse_ident(ParseContext::CustomType)?;
        Ok(CustomType {
            module_access,
            name,
        })
    }

    fn parse_array_slice_type(&mut self) -> Result<P<ArraySliceType>, ParseError> {
        let mut array_slice_type = self.arena.alloc::<ArraySliceType>();
        self.expect_token(Token::OpenBracket, ParseContext::ArraySliceType)?;
        self.expect_token(Token::CloseBracket, ParseContext::ArraySliceType)?;
        array_slice_type.element = self.parse_type()?;
        Ok(array_slice_type)
    }

    fn parse_array_static_type(&mut self) -> Result<P<ArrayStaticType>, ParseError> {
        let mut array_static_type = self.arena.alloc::<ArrayStaticType>();
        self.expect_token(Token::OpenBracket, ParseContext::ArrayStaticType)?;
        array_static_type.size = self.parse_expr()?;
        self.expect_token(Token::CloseBracket, ParseContext::ArrayStaticType)?;
        array_static_type.element = self.parse_type()?;
        Ok(array_static_type)
    }

    fn parse_visibility(&mut self) -> Visibility {
        match self.peek() {
            Token::KwPub => Visibility::Public,
            _ => Visibility::Private,
        }
    }

    fn parse_decl(&mut self) -> Result<Decl, ParseError> {
        match self.peek() {
            Token::KwMod => Ok(Decl::Mod(self.parse_mod_decl()?)),
            Token::KwImport => Ok(Decl::Import(self.parse_import_decl()?)),
            Token::Ident | Token::KwPub => {
                if self.peek_next(1) == Token::KwMod {
                    return Ok(Decl::Mod(self.parse_mod_decl()?));
                }
                let mut offset = 0;
                if self.peek() == Token::KwPub {
                    offset += 1;
                }
                if self.peek_next(offset) != Token::Ident {
                    return Err(self.error(ParseContext::Decl, vec![Token::Ident]));
                }
                if self.peek_next(offset + 1) != Token::ColonColon {
                    return Err(self.error(ParseContext::Decl, vec![Token::ColonColon]));
                }
                match self.peek_next(offset + 2) {
                    Token::OpenParen => Ok(Decl::Proc(self.parse_proc_decl()?)),
                    Token::KwEnum => Ok(Decl::Enum(self.parse_enum_decl()?)),
                    Token::KwStruct => Ok(Decl::Struct(self.parse_struct_decl()?)),
                    _ => Ok(Decl::Global(self.parse_global_decl()?)),
                }
            }
            _ => Err(self.error(
                ParseContext::Decl,
                vec![Token::Ident, Token::KwPub, Token::KwMod, Token::KwImport],
            )),
        }
    }

    fn parse_mod_decl(&mut self) -> Result<P<ModDecl>, ParseError> {
        let mut mod_decl = self.arena.alloc::<ModDecl>();
        mod_decl.visibility = self.parse_visibility();
        self.expect_token(Token::KwMod, ParseContext::ModDecl)?;
        mod_decl.name = self.parse_ident(ParseContext::ModDecl)?;
        self.expect_token(Token::Semicolon, ParseContext::ModDecl)?;
        Ok(mod_decl)
    }

    fn parse_proc_decl(&mut self) -> Result<P<ProcDecl>, ParseError> {
        let mut proc_decl = self.arena.alloc::<ProcDecl>();
        proc_decl.visibility = self.parse_visibility();
        proc_decl.name = self.parse_ident(ParseContext::ProcDecl)?;
        self.expect_token(Token::ColonColon, ParseContext::ProcDecl)?;
        self.expect_token(Token::OpenParen, ParseContext::ProcDecl)?;
        if !self.try_consume(Token::CloseParen) {
            loop {
                if self.try_consume(Token::DotDot) {
                    proc_decl.is_variadic = true;
                    break;
                }
                let param = self.parse_proc_param()?;
                proc_decl.params.add(&mut self.arena, param);
                if !self.try_consume(Token::Comma) {
                    break;
                }
            }
            self.expect_token(Token::CloseParen, ParseContext::ProcDecl)?;
        }
        proc_decl.return_type = if self.try_consume(Token::ArrowThin) {
            Some(self.parse_type()?)
        } else {
            None
        };
        proc_decl.block = if !self.try_consume(Token::Dir_c_call) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(proc_decl)
    }

    fn parse_proc_param(&mut self) -> Result<ProcParam, ParseError> {
        let name = self.parse_ident(ParseContext::ProcParam)?;
        self.expect_token(Token::Colon, ParseContext::ProcParam)?;
        let tt = self.parse_type()?;
        Ok(ProcParam { name, tt })
    }

    fn parse_enum_decl(&mut self) -> Result<P<EnumDecl>, ParseError> {
        let mut enum_decl = self.arena.alloc::<EnumDecl>();
        enum_decl.visibility = self.parse_visibility();
        enum_decl.name = self.parse_ident(ParseContext::EnumDecl)?;
        self.expect_token(Token::ColonColon, ParseContext::EnumDecl)?;
        self.expect_token(Token::KwEnum, ParseContext::EnumDecl)?;
        enum_decl.basic_type = self.try_consume_basic_type();
        self.expect_token(Token::OpenBlock, ParseContext::EnumDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            enum_decl.variants.add(&mut self.arena, variant);
        }
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.parse_ident(ParseContext::EnumVariant)?;
        let expr = if self.try_consume(Token::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_token(Token::Semicolon, ParseContext::EnumVariant)?;
        Ok(EnumVariant { name, expr })
    }

    fn parse_struct_decl(&mut self) -> Result<P<StructDecl>, ParseError> {
        let mut struct_decl = self.arena.alloc::<StructDecl>();
        struct_decl.visibility = self.parse_visibility();
        struct_decl.name = self.parse_ident(ParseContext::StructDecl)?;
        self.expect_token(Token::ColonColon, ParseContext::StructDecl)?;
        self.expect_token(Token::KwStruct, ParseContext::StructDecl)?;
        self.expect_token(Token::OpenBlock, ParseContext::StructDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let field = self.parse_struct_field()?;
            struct_decl.fields.add(&mut self.arena, field);
        }
        Ok(struct_decl)
    }

    fn parse_struct_field(&mut self) -> Result<StructField, ParseError> {
        let name = self.parse_ident(ParseContext::StructField)?;
        self.expect_token(Token::Colon, ParseContext::StructField)?;
        let tt = self.parse_type()?;
        let default = if self.try_consume(Token::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_token(Token::Semicolon, ParseContext::StructField)?;
        Ok(StructField { name, tt, default })
    }

    fn parse_global_decl(&mut self) -> Result<P<GlobalDecl>, ParseError> {
        let mut global_decl = self.arena.alloc::<GlobalDecl>();
        global_decl.visibility = self.parse_visibility();
        global_decl.name = self.parse_ident(ParseContext::GlobalDecl)?;
        if self.try_consume(Token::Colon) {
            global_decl.tt = Some(self.parse_type()?);
            self.expect_token(Token::Colon, ParseContext::GlobalDecl)?;
        } else {
            global_decl.tt = None;
            self.expect_token(Token::ColonColon, ParseContext::GlobalDecl)?;
        }
        global_decl.expr = self.parse_expr()?;
        self.expect_token(Token::Semicolon, ParseContext::GlobalDecl)?;
        Ok(global_decl)
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ParseError> {
        let mut import_decl = self.arena.alloc::<ImportDecl>();
        self.expect_token(Token::KwImport, ParseContext::ImportDecl)?;
        import_decl.module_access = self.parse_module_access();
        import_decl.target = self.parse_import_target()?;
        Ok(import_decl)
    }

    fn parse_import_target(&mut self) -> Result<ImportTarget, ParseError> {
        match self.peek() {
            Token::Ident => {
                let name = self.parse_ident(ParseContext::ImportDecl)?;
                self.expect_token(Token::Semicolon, ParseContext::ImportDecl)?;
                Ok(ImportTarget::Module(name))
            }
            Token::Times => {
                self.consume();
                self.expect_token(Token::Semicolon, ParseContext::ImportDecl)?;
                Ok(ImportTarget::AllSymbols)
            }
            Token::OpenBracket => {
                self.consume();
                let mut symbols = List::<Ident>::new();
                if !self.try_consume(Token::CloseBracket) {
                    loop {
                        let name = self.parse_ident(ParseContext::ImportDecl)?;
                        symbols.add(&mut self.arena, name);
                        if !self.try_consume(Token::Comma) {
                            break;
                        }
                    }
                    self.expect_token(Token::CloseBracket, ParseContext::ImportDecl)?;
                }
                self.expect_token(Token::Semicolon, ParseContext::ImportDecl)?;
                Ok(ImportTarget::SymbolList(symbols))
            }
            _ => Err(self.error(
                ParseContext::ImportDecl,
                vec![Token::Ident, Token::Times, Token::OpenBracket],
            )),
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            Token::KwIf => Ok(Stmt::If(self.parse_if()?)),
            Token::KwFor => Ok(Stmt::For(self.parse_for()?)),
            Token::OpenBlock => Ok(Stmt::Block(self.parse_block()?)),
            Token::KwDefer => Ok(Stmt::Defer(self.parse_defer()?)),
            Token::KwBreak => Ok(self.parse_break()?),
            Token::KwSwitch => Ok(Stmt::Switch(self.parse_switch()?)),
            Token::KwReturn => Ok(Stmt::Return(self.parse_return()?)),
            Token::KwContinue => Ok(self.parse_continue()?),
            Token::Ident => {
                if self.peek_next(1) == Token::Colon {
                    return Ok(Stmt::VarDecl(self.parse_var_decl()?));
                }
                let module_access = self.parse_module_access();
                if self.peek() == Token::Ident && self.peek_next(1) == Token::OpenParen {
                    let stmt = Stmt::ProcCall(self.parse_proc_call(module_access)?);
                    self.expect_token(Token::Semicolon, ParseContext::ProcCall)?;
                    Ok(stmt)
                } else {
                    return Ok(Stmt::VarAssign(self.parse_var_assign(module_access)?));
                }
            }
            _ => Err(self.error(
                ParseContext::Stmt,
                vec![
                    Token::KwIf,
                    Token::KwFor,
                    Token::OpenBlock,
                    Token::KwDefer,
                    Token::KwBreak,
                    Token::KwSwitch,
                    Token::KwReturn,
                    Token::KwContinue,
                    Token::Ident,
                ],
            )),
        }
    }

    fn parse_if(&mut self) -> Result<P<If>, ParseError> {
        let mut if_ = self.arena.alloc::<If>();
        self.expect_token(Token::KwIf, ParseContext::If)?;
        if_.condition = self.parse_expr()?;
        if_.block = self.parse_block()?;
        if_.else_ = self.parse_else()?;
        Ok(if_)
    }

    fn parse_else(&mut self) -> Result<Option<Else>, ParseError> {
        match self.peek() {
            Token::KwIf => Ok(Some(Else::If(self.parse_if()?))),
            Token::OpenBlock => Ok(Some(Else::Block(self.parse_block()?))),
            _ => Ok(None),
        }
    }

    fn parse_for(&mut self) -> Result<P<For>, ParseError> {
        let mut for_ = self.arena.alloc::<For>();
        self.expect_token(Token::KwFor, ParseContext::For)?;

        if self.peek() == Token::OpenBlock {
            for_.block = self.parse_block()?;
            return Ok(for_);
        }

        for_.var_decl = if self.peek() == Token::Ident && self.peek_next(1) == Token::Colon {
            Some(self.parse_var_decl()?)
        } else {
            None
        };
        for_.condition = Some(self.parse_expr()?);
        for_.var_assign = if self.peek_next(-1) == Token::Semicolon {
            let module_access = self.parse_module_access();
            Some(self.parse_var_assign(module_access)?)
        } else {
            None
        };

        for_.block = self.parse_block()?;
        Ok(for_)
    }

    fn parse_block(&mut self) -> Result<P<Block>, ParseError> {
        let mut block = self.arena.alloc::<Block>();
        self.expect_token(Token::OpenBlock, ParseContext::Block)?;
        while !self.try_consume(Token::CloseBlock) {
            let stmt = self.parse_stmt()?;
            block.stmts.add(&mut self.arena, stmt);
        }
        Ok(block)
    }

    fn parse_defer(&mut self) -> Result<P<Block>, ParseError> {
        self.expect_token(Token::KwDefer, ParseContext::Defer)?;
        Ok(self.parse_block()?)
    }

    fn parse_break(&mut self) -> Result<Stmt, ParseError> {
        self.expect_token(Token::KwBreak, ParseContext::Break)?;
        self.expect_token(Token::Semicolon, ParseContext::Break)?;
        Ok(Stmt::Break)
    }

    fn parse_switch(&mut self) -> Result<P<Switch>, ParseError> {
        let mut switch = self.arena.alloc::<Switch>();
        switch.expr = self.parse_expr()?;
        self.expect_token(Token::OpenBlock, ParseContext::Switch)?;
        while !self.try_consume(Token::CloseBlock) {
            let case = self.parse_switch_case()?;
            switch.cases.add(&mut self.arena, case);
        }
        Ok(switch)
    }

    fn parse_switch_case(&mut self) -> Result<SwitchCase, ParseError> {
        let expr = self.parse_expr()?;
        self.expect_token(Token::ArrowWide, ParseContext::SwitchCase)?;
        let block = self.parse_block()?;
        Ok(SwitchCase { expr, block })
    }

    fn parse_return(&mut self) -> Result<P<Return>, ParseError> {
        let mut return_ = self.arena.alloc::<Return>();
        self.expect_token(Token::KwReturn, ParseContext::Return)?;
        return_.expr = if !self.try_consume(Token::Semicolon) {
            let expr = self.parse_expr()?;
            self.expect_token(Token::Semicolon, ParseContext::Return)?;
            Some(expr)
        } else {
            None
        };
        Ok(return_)
    }

    fn parse_continue(&mut self) -> Result<Stmt, ParseError> {
        self.expect_token(Token::KwContinue, ParseContext::Continue)?;
        self.expect_token(Token::Semicolon, ParseContext::Continue)?;
        Ok(Stmt::Continue)
    }

    fn parse_var_decl(&mut self) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.arena.alloc::<VarDecl>();
        var_decl.name = self.parse_ident(ParseContext::VarDecl)?;
        self.expect_token(Token::Colon, ParseContext::VarDecl)?;
        if self.try_consume(Token::Assign) {
            var_decl.tt = None;
            var_decl.expr = Some(self.parse_expr()?);
        } else {
            var_decl.tt = Some(self.parse_type()?);
            var_decl.expr = if self.try_consume(Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            }
        }
        self.expect_token(Token::Semicolon, ParseContext::VarDecl)?;
        Ok(var_decl)
    }

    fn parse_var_assign(
        &mut self,
        module_access: Option<ModuleAccess>,
    ) -> Result<P<VarAssign>, ParseError> {
        let mut var_assign = self.arena.alloc::<VarAssign>();
        var_assign.var = self.parse_var(module_access)?;
        var_assign.op = self.expect_assign_op(ParseContext::VarAssign)?;
        var_assign.expr = self.parse_expr()?;
        self.expect_token(Token::Semicolon, ParseContext::VarAssign)?;
        Ok(var_assign)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_sub_expr(0)
    }

    fn parse_sub_expr(&mut self, min_prec: u32) -> Result<Expr, ParseError> {
        let mut expr_lhs = self.parse_primary_expr()?;
        loop {
            let prec: u32;
            let binary_op: BinaryOp;
            if let Some(op) = self.peek().as_binary_op() {
                binary_op = op;
                prec = op.prec();
                if prec < min_prec {
                    break;
                }
                self.consume();
            } else {
                break;
            }
            let mut bin_expr = self.arena.alloc::<BinaryExpr>();
            bin_expr.op = binary_op;
            bin_expr.lhs = expr_lhs;
            bin_expr.rhs = self.parse_sub_expr(prec + 1)?;
            expr_lhs = Expr::BinaryExpr(bin_expr);
        }
        Ok(expr_lhs)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.try_consume(Token::OpenParen) {
            let expr = self.parse_sub_expr(0)?;
            self.expect_token(Token::CloseParen, ParseContext::Expr)?;
            return Ok(expr);
        }

        if let Some(unary_op) = self.try_consume_unary_op() {
            let mut unary_expr = self.arena.alloc::<UnaryExpr>();
            unary_expr.op = unary_op;
            unary_expr.rhs = self.parse_primary_expr()?;
            return Ok(Expr::UnaryExpr(unary_expr));
        }

        match self.peek() {
            Token::Dot => match self.peek_next(1) {
                Token::OpenBlock => Ok(Expr::StructInit(self.parse_struct_init(None)?)),
                _ => Ok(Expr::Enum(self.parse_enum()?)),
            },
            Token::KwCast => Ok(Expr::Cast(self.parse_cast()?)),
            Token::KwSizeof => Ok(Expr::Sizeof(self.parse_sizeof()?)),
            Token::LitNull
            | Token::LitBool(..)
            | Token::LitInt(..)
            | Token::LitFloat(..)
            | Token::LitChar(..)
            | Token::LitString => Ok(Expr::Literal(self.parse_literal()?)),
            Token::OpenBracket | Token::OpenBlock => Ok(Expr::ArrayInit(self.parse_array_init()?)),
            Token::Ident => {
                let module_access = self.parse_module_access();
                if self.peek() != Token::Ident {
                    return Err(self.error(ParseContext::Expr, vec![Token::Ident]));
                }
                if self.peek_next(1) == Token::OpenParen {
                    Ok(Expr::ProcCall(self.parse_proc_call(module_access)?))
                } else if self.peek_next(1) == Token::Dot && self.peek_next(2) == Token::OpenBlock {
                    Ok(Expr::StructInit(self.parse_struct_init(module_access)?))
                } else {
                    Ok(Expr::Var(self.parse_var(module_access)?))
                }
            }
            _ => Err(self.error(
                ParseContext::Expr,
                vec![
                    Token::Ident,
                    Token::Dot,
                    Token::KwCast,
                    Token::KwSizeof,
                    Token::LitNull, // todo all literals
                    Token::OpenBracket,
                    Token::OpenBlock,
                ],
            )),
        }
    }

    fn parse_var(&mut self, module_access: Option<ModuleAccess>) -> Result<P<Var>, ParseError> {
        let mut var = self.arena.alloc::<Var>();
        var.module_access = module_access;
        var.name = self.parse_ident(ParseContext::Var)?;
        var.access = self.parse_access_chain()?;
        Ok(var)
    }

    fn parse_access_chain(&mut self) -> Result<Option<P<Access>>, ParseError> {
        match self.peek() {
            Token::Dot | Token::OpenBracket => {}
            _ => return Ok(None),
        }
        let access = self.parse_access()?;
        let mut access_last = access;
        while self.peek() == Token::Dot || self.peek() == Token::OpenBracket {
            let access_next = self.parse_access()?;
            access_last.next = Some(access_next);
            access_last = access_next;
        }
        Ok(Some(access))
    }

    fn parse_access(&mut self) -> Result<P<Access>, ParseError> {
        let mut access = self.arena.alloc::<Access>();
        match self.peek() {
            Token::Dot => {
                self.consume();
                access.kind = AccessKind::Ident(self.parse_ident(ParseContext::Access)?);
                Ok(access)
            }
            Token::OpenBracket => {
                self.consume();
                access.kind = AccessKind::Array(self.parse_expr()?);
                self.expect_token(Token::CloseBracket, ParseContext::ArrayAccess)?;
                Ok(access)
            }
            _ => Err(self.error(ParseContext::Access, vec![Token::Dot, Token::OpenBracket])),
        }
    }

    fn parse_enum(&mut self) -> Result<P<Enum>, ParseError> {
        let mut enum_ = self.arena.alloc::<Enum>();
        self.expect_token(Token::Dot, ParseContext::Enum)?;
        enum_.variant = self.parse_ident(ParseContext::Enum)?;
        Ok(enum_)
    }

    fn parse_cast(&mut self) -> Result<P<Cast>, ParseError> {
        let mut cast = self.arena.alloc::<Cast>();
        self.expect_token(Token::KwCast, ParseContext::Cast)?;
        self.expect_token(Token::OpenParen, ParseContext::Cast)?;
        cast.tt = self.parse_type()?;
        self.expect_token(Token::Comma, ParseContext::Cast)?;
        cast.expr = self.parse_expr()?;
        self.expect_token(Token::CloseParen, ParseContext::Cast)?;
        Ok(cast)
    }

    fn parse_sizeof(&mut self) -> Result<P<Sizeof>, ParseError> {
        let mut sizeof = self.arena.alloc::<Sizeof>();
        self.expect_token(Token::KwSizeof, ParseContext::Sizeof)?;
        self.expect_token(Token::OpenParen, ParseContext::Sizeof)?;
        sizeof.tt = self.parse_type()?;
        self.expect_token(Token::CloseParen, ParseContext::Sizeof)?;
        Ok(sizeof)
    }

    fn parse_literal(&mut self) -> Result<P<Literal>, ParseError> {
        let mut literal = self.arena.alloc::<Literal>();
        match self.peek() {
            Token::LitNull => *literal = Literal::Null,
            Token::LitBool(v) => *literal = Literal::Bool(v),
            Token::LitInt(v) => *literal = Literal::Uint(v),
            Token::LitFloat(v) => *literal = Literal::Float(v),
            Token::LitChar(v) => *literal = Literal::Char(v),
            Token::LitString => *literal = Literal::String,
            _ => return Err(self.error(ParseContext::Literal, vec![Token::LitNull])), // any literal
        }
        self.consume();
        Ok(literal)
    }

    fn parse_proc_call(
        &mut self,
        module_access: Option<ModuleAccess>,
    ) -> Result<P<ProcCall>, ParseError> {
        let mut proc_call = self.arena.alloc::<ProcCall>();
        proc_call.module_access = module_access;
        proc_call.name = self.parse_ident(ParseContext::ProcCall)?;
        proc_call.input =
            self.parse_expr_list(Token::OpenParen, Token::CloseParen, ParseContext::ProcCall)?;
        proc_call.access = self.parse_access_chain()?;
        Ok(proc_call)
    }

    fn parse_array_init(&mut self) -> Result<P<ArrayInit>, ParseError> {
        let mut array_init = self.arena.alloc::<ArrayInit>();
        array_init.tt = if self.peek() == Token::OpenBracket {
            Some(self.parse_type()?)
        } else {
            None
        };
        array_init.input =
            self.parse_expr_list(Token::OpenBlock, Token::CloseBlock, ParseContext::ArrayInit)?;
        Ok(array_init)
    }

    fn parse_struct_init(
        &mut self,
        module_access: Option<ModuleAccess>,
    ) -> Result<P<StructInit>, ParseError> {
        let mut struct_init = self.arena.alloc::<StructInit>();
        struct_init.module_access = module_access;
        let has_name = self.peek() == Token::Ident || module_access.is_some();
        struct_init.struct_name = if has_name {
            Some(self.parse_ident(ParseContext::StructInit)?)
        } else {
            None
        };
        self.expect_token(Token::Dot, ParseContext::StructInit)?;
        struct_init.input = self.parse_expr_list(
            Token::OpenBlock,
            Token::CloseBlock,
            ParseContext::StructInit,
        )?;
        Ok(struct_init)
    }

    fn parse_expr_list(
        &mut self,
        start: Token,
        end: Token,
        context: ParseContext,
    ) -> Result<List<Expr>, ParseError> {
        let mut expr_list = List::<Expr>::new();
        self.expect_token(start, context)?;
        if !self.try_consume(end) {
            loop {
                let expr = self.parse_expr()?;
                expr_list.add(&mut self.arena, expr);
                if !self.try_consume(Token::Comma) {
                    break;
                }
            }
            self.expect_token(end, context)?;
        }
        Ok(expr_list)
    }

    fn peek(&self) -> Token {
        unsafe { self.tokens.get_unchecked(self.peek_index).token }
    }

    fn peek_next(&self, offset: isize) -> Token {
        unsafe {
            self.tokens
                .get_unchecked(self.peek_index + offset as usize)
                .token
        }
    }

    fn peek_span(&self) -> Span {
        unsafe { self.tokens.get_unchecked(self.peek_index).span }
    }

    fn consume(&mut self) {
        self.peek_index += 1;
    }

    fn try_consume(&mut self, token: Token) -> bool {
        if token == self.peek() {
            self.consume();
            return true;
        }
        false
    }

    fn try_consume_unary_op(&mut self) -> Option<UnaryOp> {
        match self.peek().as_unary_op() {
            Some(op) => {
                self.consume();
                Some(op)
            }
            None => None,
        }
    }

    fn try_consume_basic_type(&mut self) -> Option<BasicType> {
        match self.peek().as_basic_type() {
            Some(op) => {
                self.consume();
                Some(op)
            }
            None => None,
        }
    }

    fn expect_token(&mut self, token: Token, context: ParseContext) -> Result<(), ParseError> {
        if token == self.peek() {
            self.consume();
            return Ok(());
        }
        Err(self.error(context, vec![token]))
    }

    fn expect_assign_op(&mut self, context: ParseContext) -> Result<AssignOp, ParseError> {
        match self.peek().as_assign_op() {
            Some(op) => {
                self.consume();
                Ok(op)
            }
            None => Err(self.error(context, vec![Token::Assign])), //todo any assign op
        }
    }
}

impl BinaryOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinaryOp::LogicAnd | BinaryOp::LogicOr => 0,
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessEq
            | BinaryOp::GreaterEq
            | BinaryOp::IsEq
            | BinaryOp::NotEq => 1,
            BinaryOp::Plus | BinaryOp::Minus => 2,
            BinaryOp::Times | BinaryOp::Div | BinaryOp::Mod => 3,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => 4,
            BinaryOp::Shl | BinaryOp::Shr => 5,
        }
    }
}

impl Token {
    fn as_unary_op(&self) -> Option<UnaryOp> {
        match self {
            Token::Minus => Some(UnaryOp::Minus),
            Token::BitNot => Some(UnaryOp::BitNot),
            Token::LogicNot => Some(UnaryOp::LogicNot),
            Token::Times => Some(UnaryOp::AddressOf),
            Token::Shl => Some(UnaryOp::Dereference),
            _ => None,
        }
    }

    fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            Token::LogicAnd => Some(BinaryOp::LogicAnd),
            Token::LogicOr => Some(BinaryOp::LogicOr),
            Token::Less => Some(BinaryOp::Less),
            Token::Greater => Some(BinaryOp::Greater),
            Token::LessEq => Some(BinaryOp::LessEq),
            Token::GreaterEq => Some(BinaryOp::GreaterEq),
            Token::IsEq => Some(BinaryOp::IsEq),
            Token::NotEq => Some(BinaryOp::NotEq),
            Token::Plus => Some(BinaryOp::Plus),
            Token::Minus => Some(BinaryOp::Minus),
            Token::Times => Some(BinaryOp::Times),
            Token::Div => Some(BinaryOp::Div),
            Token::Mod => Some(BinaryOp::Mod),
            Token::BitAnd => Some(BinaryOp::BitAnd),
            Token::BitOr => Some(BinaryOp::BitOr),
            Token::BitXor => Some(BinaryOp::BitXor),
            Token::Shl => Some(BinaryOp::Shl),
            Token::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn as_assign_op(&self) -> Option<AssignOp> {
        match self {
            Token::Assign => Some(AssignOp::Assign),
            Token::PlusEq => Some(AssignOp::BinaryOp(BinaryOp::Plus)),
            Token::MinusEq => Some(AssignOp::BinaryOp(BinaryOp::Minus)),
            Token::TimesEq => Some(AssignOp::BinaryOp(BinaryOp::Times)),
            Token::DivEq => Some(AssignOp::BinaryOp(BinaryOp::Div)),
            Token::ModEq => Some(AssignOp::BinaryOp(BinaryOp::Mod)),
            Token::BitAndEq => Some(AssignOp::BinaryOp(BinaryOp::BitAnd)),
            Token::BitOrEq => Some(AssignOp::BinaryOp(BinaryOp::BitOr)),
            Token::BitXorEq => Some(AssignOp::BinaryOp(BinaryOp::BitXor)),
            Token::ShlEq => Some(AssignOp::BinaryOp(BinaryOp::Shl)),
            Token::ShrEq => Some(AssignOp::BinaryOp(BinaryOp::Shr)),
            _ => None,
        }
    }

    fn as_basic_type(&self) -> Option<BasicType> {
        match self {
            Token::KwBool => Some(BasicType::Bool),
            Token::KwS8 => Some(BasicType::S8),
            Token::KwS16 => Some(BasicType::S16),
            Token::KwS32 => Some(BasicType::S32),
            Token::KwS64 => Some(BasicType::S64),
            Token::KwSsize => Some(BasicType::Ssize),
            Token::KwU8 => Some(BasicType::U8),
            Token::KwU16 => Some(BasicType::U16),
            Token::KwU32 => Some(BasicType::U32),
            Token::KwU64 => Some(BasicType::U64),
            Token::KwUsize => Some(BasicType::Usize),
            Token::KwF32 => Some(BasicType::F32),
            Token::KwF64 => Some(BasicType::F64),
            Token::KwChar => Some(BasicType::Char),
            Token::KwRawptr => Some(BasicType::Rawptr),
            _ => None,
        }
    }
}

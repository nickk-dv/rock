use super::token::{self, Token, TokenList, Trivia, T};
use crate::error::{ErrorBuffer, SourceRange};
use crate::errors as err;
use crate::intern::{LitID, StringPool};
use crate::session::ModuleID;
use crate::text::{TextOffset, TextRange};

pub fn lex<'sref>(
    source: &'sref str,
    module_id: ModuleID,
    intern_lit: &'sref mut StringPool<LitID>,
) -> (TokenList, ErrorBuffer) {
    let mut lex = Lexer {
        cursor: 0,
        tokens: TokenList::new(source),
        errors: ErrorBuffer::default(),
        buffer: String::with_capacity(128),
        source,
        module_id,
        intern_lit,
    };
    source_file(&mut lex);
    (lex.tokens, lex.errors)
}

struct Lexer<'s, 'sref> {
    cursor: usize,
    tokens: TokenList,
    errors: ErrorBuffer,
    buffer: String,
    source: &'sref str,
    module_id: ModuleID,
    intern_lit: &'sref mut StringPool<'s, LitID>,
}

impl Lexer<'_, '_> {
    #[inline(always)]
    fn start_range(&self) -> TextOffset {
        (self.cursor as u32).into()
    }
    #[inline(always)]
    fn make_range(&self, start: TextOffset) -> TextRange {
        TextRange::new(start, (self.cursor as u32).into())
    }
    #[inline(always)]
    fn make_src(&self, start: TextOffset) -> SourceRange {
        let range = TextRange::new(start, (self.cursor as u32).into());
        SourceRange::new(self.module_id, range)
    }

    #[inline(always)]
    pub fn peek(&mut self) -> u8 {
        unsafe { *self.source.as_bytes().get_unchecked(self.cursor) }
    }
    #[inline(always)]
    pub fn peek_next(&mut self) -> u8 {
        unsafe { *self.source.as_bytes().get_unchecked(self.cursor + 1) }
    }
    #[inline(always)]
    pub fn peek_utf8(&mut self) -> char {
        let next = unsafe { self.source.get_unchecked(self.cursor..) };
        unsafe { next.chars().next().unwrap_unchecked() }
    }

    #[inline(always)]
    pub fn bump(&mut self) {
        self.cursor += 1;
    }
    #[inline(always)]
    pub fn bump_utf8(&mut self, c: char) {
        self.cursor += c.len_utf8();
    }

    #[inline(always)]
    pub fn at(&mut self, c: u8) -> bool {
        self.peek() == c
    }
    #[inline(always)]
    pub fn at_next(&mut self, c: u8) -> bool {
        self.peek_next() == c
    }
    #[inline(always)]
    pub fn eat(&mut self, c: u8) -> bool {
        if self.at(c) {
            self.bump();
            true
        } else {
            false
        }
    }
}

const SENTINEL: u8 = b'\0';

fn source_file(lex: &mut Lexer) {
    loop {
        lex_whitespace(lex);
        let c = lex.peek();
        match c {
            SENTINEL => break,
            b'\'' => lex_char(lex),
            b'"' => lex_string(lex),
            b'`' => lex_string_raw(lex),
            _ => {
                if c.is_ascii_alphabetic() || c == b'_' {
                    lex_ident(lex);
                } else if c.is_ascii_digit() {
                    lex_number(lex, c);
                } else if c.is_ascii() {
                    lex_symbol(lex, c)
                } else {
                    let start = lex.start_range();
                    let c = lex.peek_utf8();
                    lex.bump_utf8(c);
                    let src = lex.make_src(start);
                    err::lexer_symbol_unknown(&mut lex.errors, src, c);
                }
            }
        }
    }
    lex.tokens.add_token(Token::Eof, lex.start_range());
    lex.tokens.add_token(Token::Eof, lex.start_range());
}

fn lex_whitespace(lex: &mut Lexer) {
    loop {
        let c = lex.peek();
        if c.is_ascii_whitespace() {
            let start = lex.start_range();
            lex.bump();

            while lex.peek().is_ascii_whitespace() {
                lex.bump();
            }

            let range = lex.make_range(start);
            lex.tokens.add_trivia(Trivia::Whitespace, range);
        } else if c == b'/' && lex.at_next(b'/') {
            let start = lex.start_range();
            lex.bump();
            lex.bump();

            let trivia = if lex.eat(b'/') {
                if lex.eat(b'/') {
                    Trivia::ModComment
                } else {
                    Trivia::DocComment
                }
            } else {
                Trivia::LineComment
            };

            loop {
                let c = lex.peek();
                if c == b'\n' || c == b'\r' || c == SENTINEL {
                    break;
                }
                if c.is_ascii() {
                    lex.bump();
                } else {
                    let ch = lex.peek_utf8();
                    lex.bump_utf8(ch);
                }
            }

            let range = lex.make_range(start);
            lex.tokens.add_trivia(trivia, range);
        } else {
            break;
        }
    }
}

fn lex_ident(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.bump();

    loop {
        let c = lex.peek();
        if c.is_ascii_alphanumeric() || c == b'_' {
            lex.bump();
        } else {
            break;
        }
    }

    let range = lex.make_range(start);
    let string = unsafe { lex.source.get_unchecked(range.as_usize()) };
    let token = keyword(string);

    if token == Token::Ident {
        lex.tokens.add_ident(range);
    } else {
        lex.tokens.add_token(token, range.start());
    }
}

fn lex_symbol(lex: &mut Lexer, c: u8) {
    let start = lex.start_range();
    lex.bump();

    let mut token = unsafe { *token::TOKEN_BYTE_TO_SINGLE.get_unchecked(c as usize) };
    if token == Token::Eof {
        let src = lex.make_src(start);
        err::lexer_symbol_unknown(&mut lex.errors, src, c as char);
        return;
    }

    let c = lex.peek();
    let double = Token::glue_double(c, token);
    if token != double {
        token = double;
        lex.bump();
    } else {
        lex.tokens.add_token(token, start);
        return;
    }

    let c = lex.peek();
    let triple = Token::glue_triple(c, token);
    if token != triple {
        token = triple;
        lex.bump();
    }
    lex.tokens.add_token(token, start);
}

fn lex_char(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.bump();

    let fc = lex.peek();
    if let SENTINEL | b'\n' | b'\r' = fc {
        let src = lex.make_src(start);
        err::lexer_char_incomplete(&mut lex.errors, src);
        return;
    }

    let mut inner_tick = false;
    let char = match fc {
        b'\\' => lex_escape(lex),
        b'\'' => {
            inner_tick = true;
            lex.bump();
            '\''
        }
        b'\t' => {
            let start = lex.start_range();
            lex.bump();
            let src = lex.make_src(start);
            err::lexer_char_tab_not_escaped(&mut lex.errors, src);
            '\t'
        }
        _ => {
            let fc = lex.peek_utf8();
            lex.bump_utf8(fc);
            fc
        }
    };
    let terminated = lex.eat(b'\'');

    if inner_tick {
        if !terminated {
            let src = lex.make_src(start);
            err::lexer_char_empty(&mut lex.errors, src);
        } else {
            let src = lex.make_src(start);
            err::lexer_char_quote_not_escaped(&mut lex.errors, src);
        }
    } else if !terminated {
        let src = lex.make_src(start);
        err::lexer_char_not_terminated(&mut lex.errors, src);
    }

    let range = lex.make_range(start);
    lex.tokens.add_char(char, range);
}

fn lex_string(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.bump();
    lex.buffer.clear();
    let mut terminated = false;

    loop {
        match lex.peek() {
            SENTINEL | b'\n' | b'\r' => break,
            b'"' => {
                lex.bump();
                terminated = true;
                break;
            }
            b'\\' => {
                let escape = lex_escape(lex);
                lex.buffer.push(escape);
            }
            _ => {
                let ch = lex.peek_utf8();
                lex.bump_utf8(ch);
                lex.buffer.push(ch);
            }
        }
    }

    let range = lex.make_range(start);
    let id = lex.intern_lit.intern(&lex.buffer);
    lex.tokens.add_string(id, range);

    if !terminated {
        let src = lex.make_src(start);
        err::lexer_string_not_terminated(&mut lex.errors, src);
    }
}

fn lex_string_raw(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.bump();
    lex.buffer.clear();
    let mut terminated = false;

    loop {
        match lex.peek() {
            SENTINEL | b'\n' | b'\r' => break,
            b'`' => {
                lex.bump();
                terminated = true;
                break;
            }
            _ => {
                let ch = lex.peek_utf8();
                lex.bump_utf8(ch);
                lex.buffer.push(ch);
            }
        }
    }

    let range = lex.make_range(start);
    let id = lex.intern_lit.intern(&lex.buffer);
    lex.tokens.add_string(id, range);

    if !terminated {
        let src = lex.make_src(start);
        err::lexer_string_raw_not_terminated(&mut lex.errors, src);
    }
}

fn lex_escape(lex: &mut Lexer) -> char {
    let start = lex.start_range();
    lex.bump();

    let c = lex.peek();
    if c == SENTINEL {
        let src = lex.make_src(start);
        err::lexer_escape_sequence_incomplete(&mut lex.errors, src);
        return '\\';
    };

    let mut hex_escape = false;
    let escaped = match c {
        b'n' => '\n',
        b'r' => '\r',
        b't' => '\t',
        b'\'' => '\'',
        b'\"' => '\"',
        b'\\' => '\\',
        b'x' => {
            lex.bump();
            hex_escape = true;
            lex_escape_hex(lex, start)
        }
        _ => {
            if c.is_ascii_whitespace() {
                let src = lex.make_src(start);
                err::lexer_escape_sequence_incomplete(&mut lex.errors, src);
            } else {
                lex.bump();
                let src = lex.make_src(start);
                err::lexer_escape_sequence_not_supported(&mut lex.errors, src, c as char);
            }
            return '\\';
        }
    };

    if !hex_escape {
        lex.bump();
    }
    escaped
}

fn lex_escape_hex(lex: &mut Lexer, start: TextOffset) -> char {
    const MAX_DIGITS: u32 = 6;
    const SHIFT_BASE: u32 = 4;

    let mut integer: u32 = 0;
    let mut digit_count: u32 = 0;

    if !lex.eat(b'{') {
        let src = lex.make_src(start);
        err::lexer_expect_open_bracket(&mut lex.errors, src);
        return '\\';
    }

    loop {
        let value: u32 = match lex.peek() {
            c @ b'0'..=b'9' => c as u32 - b'0' as u32,
            c @ b'a'..=b'f' => c as u32 - b'a' as u32 + 10,
            c @ b'A'..=b'F' => c as u32 - b'A' as u32 + 10,
            _ => break,
        };
        lex.bump();
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if !lex.eat(b'}') {
        let src = lex.make_src(start);
        err::lexer_expect_close_bracket(&mut lex.errors, src);
        return '\\';
    }
    if digit_count == 0 || digit_count > MAX_DIGITS {
        let src = lex.make_src(start);
        err::lexer_escape_hex_wrong_dc(&mut lex.errors, src, digit_count);
        return '\\';
    }

    match integer.try_into() {
        Ok(escaped) => escaped,
        Err(_) => {
            let src = lex.make_src(start);
            err::lexer_escape_hex_non_utf8(&mut lex.errors, src, integer);
            '\\'
        }
    }
}

fn lex_number(lex: &mut Lexer, fc: u8) {
    let start = lex.start_range();

    if fc == b'0' {
        match lex.peek_next() {
            b'b' => {
                lex.bump();
                lex.bump();
                return lex_integer_bin(lex, start);
            }
            b'x' => {
                lex.bump();
                lex.bump();
                lex_integer_hex(lex, start);
                return;
            }
            _ => {}
        }
    }

    lex.buffer.clear();
    if skip_zero_digits(lex) {
        lex.buffer.push(b'0' as char);
    }
    skip_num_digits(lex);

    if lex.at(b'.') && lex.peek_next().is_ascii_digit() {
        lex_float(lex, start);
    } else {
        lex_integer_dec(lex, start);
    }
}

fn lex_float(lex: &mut Lexer, start: TextOffset) {
    lex.eat(b'.');
    lex.buffer.push('.');
    skip_num_digits(lex);
    let mut error = false;

    if lex.eat(b'e') {
        lex.buffer.push('e');

        if lex.eat(b'+') {
            lex.buffer.push('+');
        } else if lex.eat(b'-') {
            lex.buffer.push('-');
        }

        if lex.peek().is_ascii_digit() {
            skip_num_digits(lex);
        } else {
            error = true;
            let src = lex.make_src(start);
            err::lexer_float_exp_missing_digits(&mut lex.errors, src);
        }
    }

    if error {
        lex.tokens.add_float(0.0, lex.make_range(start));
    } else if let Ok(float) = lex.buffer.parse::<f64>() {
        lex.tokens.add_float(float, lex.make_range(start));
    } else {
        lex.tokens.add_float(0.0, lex.make_range(start));
        let src = lex.make_src(start);
        err::lexer_float_parse_failed(&mut lex.errors, src);
    }
}

fn lex_integer_dec(lex: &mut Lexer, start: TextOffset) {
    let mut integer: u64 = 0;
    let mut overflow = false;

    for c in lex.buffer.bytes() {
        let value: u64 = match c {
            b'0'..=b'9' => c as u64 - b'0' as u64,
            _ => unreachable!(),
        };
        let prev_value = integer;
        integer = integer.wrapping_mul(10).wrapping_add(value);
        overflow = overflow || (integer < prev_value);
    }

    if overflow {
        let src = lex.make_src(start);
        err::lexer_int_dec_overflow(&mut lex.errors, src);
    }
    lex.tokens.add_int(integer, lex.make_range(start));
}

fn lex_integer_bin(lex: &mut Lexer, start: TextOffset) {
    const MAX_DIGITS: u32 = 64;
    const SHIFT_BASE: u32 = 1;

    let mut integer: u64 = 0;
    let mut digit_count: u32 = 0;
    let skipped = skip_zero_digits(lex);

    loop {
        let value: u64 = match lex.peek() {
            c @ b'0'..=b'1' => c as u64 - b'0' as u64,
            c @ b'2'..=b'9' => {
                let start = lex.start_range();
                lex.bump();
                let src = lex.make_src(start);
                err::lexer_int_bin_invalid_digit(&mut lex.errors, src, c as char);
                continue;
            }
            b'_' => {
                lex.bump();
                continue;
            }
            _ => break,
        };
        lex.bump();
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if digit_count == 0 && !skipped {
        let src = lex.make_src(start);
        err::lexer_int_base_missing_digits(&mut lex.errors, src);
    } else if digit_count > MAX_DIGITS {
        let src = lex.make_src(start);
        err::lexer_int_bin_overflow(&mut lex.errors, src, digit_count);
    }
    lex.tokens.add_int(integer, lex.make_range(start));
}

fn lex_integer_hex(lex: &mut Lexer, start: TextOffset) {
    const MAX_DIGITS: u32 = 16;
    const SHIFT_BASE: u32 = 4;

    let mut integer: u64 = 0;
    let mut digit_count: u32 = 0;
    let skipped = skip_zero_digits(lex);

    loop {
        let value: u64 = match lex.peek() {
            c @ b'0'..=b'9' => c as u64 - b'0' as u64,
            c @ b'a'..=b'f' => c as u64 - b'a' as u64 + 10,
            c @ b'A'..=b'F' => c as u64 - b'A' as u64 + 10,
            b'_' => {
                lex.bump();
                continue;
            }
            _ => break,
        };
        lex.bump();
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if digit_count == 0 && !skipped {
        let src = lex.make_src(start);
        err::lexer_int_base_missing_digits(&mut lex.errors, src);
    } else if digit_count > MAX_DIGITS {
        let src = lex.make_src(start);
        err::lexer_int_hex_overflow(&mut lex.errors, src, digit_count);
    }
    lex.tokens.add_int(integer, lex.make_range(start));
}

fn skip_zero_digits(lex: &mut Lexer) -> bool {
    let mut skipped = false;
    loop {
        match lex.peek() {
            b'0' => {
                lex.bump();
                skipped = true;
            }
            b'_' => lex.bump(),
            _ => break,
        };
    }
    skipped
}

fn skip_num_digits(lex: &mut Lexer) {
    loop {
        match lex.peek() {
            c @ b'0'..=b'9' => {
                lex.bump();
                lex.buffer.push(c as char);
            }
            b'_' => lex.bump(),
            _ => return,
        };
    }
}

fn keyword(ident: &str) -> Token {
    match ident {
        "proc" => T![proc],
        "enum" => T![enum],
        "struct" => T![struct],
        "import" => T![import],

        "break" => T![break],
        "continue" => T![continue],
        "return" => T![return],
        "defer" => T![defer],
        "for" => T![for],
        "in" => T![in],
        "let" => T![let],
        "mut" => T![mut],
        "zeroed" => T![zeroed],
        "undefined" => T![undefined],

        "null" => T![null],
        "true" => T![true],
        "false" => T![false],
        "if" => T![if],
        "else" => T![else],
        "match" => T![match],
        "as" => T![as],
        "_" => T![_],

        "s8" => T![s8],
        "s16" => T![s16],
        "s32" => T![s32],
        "s64" => T![s64],
        "u8" => T![u8],
        "u16" => T![u16],
        "u32" => T![u32],
        "u64" => T![u64],
        "f32" => T![f32],
        "f64" => T![f64],
        "bool" => T![bool],
        "bool16" => T![bool16],
        "bool32" => T![bool32],
        "bool64" => T![bool64],
        "string" => T![string],
        "cstring" => T![cstring],
        "char" => T![char],
        "void" => T![void],
        "never" => T![never],
        "rawptr" => T![rawptr],
        _ => T![ident],
    }
}

use crate::error::{Error, ErrorBuffer, ErrorSink, SourceRange};
use crate::errors as err;
use crate::intern::{InternLit, InternPool};
use crate::session::ModuleID;
use crate::text::{TextOffset, TextRange};
use crate::token::{Token, TokenList, Trivia};
use std::{iter::Peekable, str::Chars};

pub fn lex<'src, 's>(
    source: &'src str,
    intern_lit: &'src mut InternPool<'s, InternLit>,
    module_id: ModuleID,
    with_trivia: bool,
) -> (TokenList, ErrorBuffer) {
    let mut lex = Lexer::new(source, intern_lit, module_id, with_trivia);
    source_file(&mut lex);
    (lex.tokens, lex.errors)
}

struct Lexer<'src, 's> {
    cursor: TextOffset,
    chars: Peekable<Chars<'src>>,
    tokens: TokenList,
    buffer: String,
    errors: ErrorBuffer,
    source: &'src str,
    module_id: ModuleID,
    with_trivia: bool,
    intern_lit: &'src mut InternPool<'s, InternLit>,
}

impl<'src, 's> Lexer<'src, 's> {
    fn new(
        source: &'src str,
        intern_lit: &'src mut InternPool<'s, InternLit>,
        module_id: ModuleID,
        with_trivia: bool,
    ) -> Lexer<'src, 's> {
        Lexer {
            cursor: 0.into(),
            chars: source.chars().peekable(),
            tokens: TokenList::new(0), //@no cap estimation
            buffer: String::with_capacity(64),
            errors: ErrorBuffer::default(),
            source,
            module_id,
            with_trivia,
            intern_lit,
        }
    }

    fn start_range(&self) -> TextOffset {
        self.cursor
    }

    fn make_range(&self, start: TextOffset) -> TextRange {
        TextRange::new(start, self.cursor)
    }

    fn make_src(&self, start: TextOffset) -> SourceRange {
        let range = TextRange::new(start, self.cursor);
        SourceRange::new(self.module_id, range)
    }

    pub fn at(&mut self, c: char) -> bool {
        self.peek() == Some(c)
    }

    pub fn at_next(&self, c: char) -> bool {
        self.peek_next() == Some(c)
    }

    pub fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    pub fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().copied()
    }

    pub fn bump(&mut self, c: char) {
        self.cursor += (c.len_utf8() as u32).into();
        self.chars.next();
    }

    pub fn eat(&mut self, c: char) -> bool {
        if self.at(c) {
            self.bump(c);
            true
        } else {
            false
        }
    }
}

impl<'src, 's> ErrorSink for Lexer<'src, 's> {
    fn error(&mut self, error: Error) {
        self.errors.error(error);
    }
    fn error_count(&self) -> usize {
        self.errors.error_count()
    }
}

fn source_file(lex: &mut Lexer) {
    while lex.peek().is_some() {
        lex_whitespace(lex);

        if let Some(c) = lex.peek() {
            match c {
                '\'' => lex_char(lex),
                '`' => lex_string(lex, false, true),
                '"' => lex_string(lex, false, false),
                'c' => match lex.peek_next() {
                    Some('`') => lex_string(lex, true, true),
                    Some('"') => lex_string(lex, true, false),
                    _ => lex_ident(lex, c),
                },
                _ => {
                    if c.is_ascii_digit() {
                        lex_number(lex, c);
                    } else if c.is_ascii_alphabetic() || c == '_' {
                        lex_ident(lex, c);
                    } else {
                        lex_symbol(lex, c)
                    }
                }
            }
        }
    }

    let dummy_range = TextRange::zero();
    lex.tokens.add_token(Token::Eof, dummy_range);
    lex.tokens.add_token(Token::Eof, dummy_range);
}

fn lex_whitespace(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        if c.is_ascii_whitespace() {
            let start = lex.start_range();
            lex.bump(c);

            while let Some(c) = lex.peek() {
                if !c.is_ascii_whitespace() {
                    break;
                }
                lex.bump(c);
            }

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::Whitespace, range);
            }
        } else if c == '/' && lex.at_next('/') {
            let start = lex.start_range();
            lex.bump(c);
            lex.bump('/');

            while let Some(c) = lex.peek() {
                if c == '\r' || c == '\n' {
                    break;
                }
                lex.bump(c);
            }

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::LineComment, range);
            }
        } else {
            break;
        }
    }
}

fn lex_ident(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.bump(fc);

    while let Some(c) = lex.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            lex.bump(c);
        } else {
            break;
        }
    }

    let range = lex.make_range(start);
    let string = &lex.source[range.as_usize()];

    let token = match Token::as_keyword(string) {
        Some(keyword) => keyword,
        None => Token::Ident,
    };
    lex.tokens.add_token(token, range);
}

fn lex_symbol(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.bump(fc);

    let mut token = match Token::from_char(fc) {
        Some(sym) => sym,
        None => {
            err::lexer_unknown_symbol(lex, lex.make_src(start), fc);
            return;
        }
    };

    match lex.peek() {
        Some(c) => match Token::glue_double(c, token) {
            Some(sym) => {
                lex.bump(c);
                token = sym;
            }
            None => {
                lex.tokens.add_token(token, lex.make_range(start));
                return;
            }
        },
        None => {
            lex.tokens.add_token(token, lex.make_range(start));
            return;
        }
    }

    match lex.peek() {
        Some(c) => match Token::glue_triple(c, token) {
            Some(sym) => {
                lex.bump(c);
                token = sym;
            }
            None => {
                lex.tokens.add_token(token, lex.make_range(start));
                return;
            }
        },
        None => {
            lex.tokens.add_token(token, lex.make_range(start));
            return;
        }
    }

    lex.tokens.add_token(token, lex.make_range(start));
}

fn lex_char(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.bump('\'');

    let fc = match lex.peek() {
        Some(c) => {
            if c == '\r' || c == '\n' {
                err::lexer_char_incomplete(lex, lex.make_src(start));
                return;
            }
            c
        }
        None => {
            err::lexer_char_incomplete(lex, lex.make_src(start));
            return;
        }
    };

    let mut inner_tick = false;
    let char = match fc {
        '\\' => lex_escape(lex, false),
        '\'' => {
            inner_tick = true;
            lex.bump(fc);
            fc
        }
        '\t' => {
            let start = lex.start_range();
            lex.bump(fc);
            err::lexer_char_tab_not_escaped(lex, lex.make_src(start));
            fc
        }
        _ => {
            lex.bump(fc);
            fc
        }
    };

    let terminated = lex.at('\'');
    if terminated {
        lex.bump('\'');
    }

    match (inner_tick, terminated) {
        (true, false) => {
            // example: ''
            err::lexer_char_empty(lex, lex.make_src(start));
        }
        (true, true) => {
            // example: '''
            err::lexer_char_quote_not_escaped(lex, lex.make_src(start));
        }
        (false, false) => {
            // example: 'x, '\n
            err::lexer_char_not_terminated(lex, lex.make_src(start));
        }
        (false, true) => {
            // example: 'x', '\n'
            // correctly terminated without inner tick
        }
    }

    let range = lex.make_range(start);
    lex.tokens.add_char(char, range);
}

fn lex_string(lex: &mut Lexer, c_string: bool, mut raw: bool) {
    let start = lex.start_range();
    if c_string {
        lex.bump('c');
    }
    lex.bump('\"');

    lex.buffer.clear();
    let mut range;
    let mut terminated = false;

    loop {
        while let Some(c) = lex.peek() {
            match c {
                '\r' | '\n' => break,
                '`' if raw => {
                    lex.bump(c);
                    terminated = true;
                    break;
                }
                '"' if !raw => {
                    lex.bump(c);
                    terminated = true;
                    break;
                }
                '\\' if !raw => {
                    let escaped = lex_escape(lex, c_string);
                    lex.buffer.push(escaped);
                }
                _ => {
                    lex.bump(c);
                    lex.buffer.push(c);
                }
            }
        }

        range = lex.make_range(start);
        if !terminated {
            break;
        }

        lex_whitespace(lex);
        match lex.peek() {
            Some('`') => raw = true,
            Some('"') => raw = false,
            Some(_) => break,
            None => break,
        }

        lex.bump('\"');
        lex.buffer.push('\n');
        terminated = false;
    }

    let id = lex.intern_lit.intern(&lex.buffer);
    lex.tokens.add_string(id, c_string, range);

    if !terminated {
        let src = SourceRange::new(lex.module_id, range);
        if raw {
            err::lexer_raw_string_not_terminated(lex, src);
        } else {
            err::lexer_string_not_terminated(lex, src);
        }
    }
}

fn lex_escape(lex: &mut Lexer, c_string: bool) -> char {
    let start = lex.start_range();
    lex.bump('\\');

    let c = if let Some(c) = lex.peek() {
        c
    } else {
        err::lexer_escape_sequence_incomplete(lex, lex.make_src(start));
        return '\\';
    };

    let mut hex_escape = false;
    let escaped = match c {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '\'' => '\'',
        '\"' => '\"',
        '\\' => '\\',
        'x' => {
            lex.bump(c);
            hex_escape = true;
            lex_escape_hex(lex, start)
        }
        _ => {
            if c.is_ascii_whitespace() {
                err::lexer_escape_sequence_incomplete(lex, lex.make_src(start));
            } else {
                lex.bump(c);
                err::lexer_escape_sequence_not_supported(lex, lex.make_src(start), c);
            }
            return '\\';
        }
    };

    if !hex_escape {
        lex.bump(c);
    }
    if c_string && escaped == '\0' {
        err::lexer_escape_sequence_cstring_null(lex, lex.make_src(start));
    }
    escaped
}

fn lex_escape_hex(lex: &mut Lexer, start: TextOffset) -> char {
    if !lex.eat('{') {
        err::lexer_expect_open_bracket(lex, lex.make_src(start));
        return '\\';
    }

    const MAX_DIGITS: u32 = 6;
    const SHIFT_BASE: u32 = 4;

    let mut integer: u32 = 0;
    let mut digit_count: u32 = 0;

    while let Some(c) = lex.peek() {
        let value: u32 = match c {
            '0'..='9' => c as u32 - '0' as u32,
            'a'..='f' => c as u32 - 'a' as u32 + 10,
            'A'..='F' => c as u32 - 'A' as u32 + 10,
            _ => break,
        };

        lex.bump(c);
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if !lex.eat('}') {
        err::lexer_expect_close_bracket(lex, lex.make_src(start));
        return '\\';
    }

    if digit_count == 0 || digit_count > MAX_DIGITS {
        err::lexer_escape_hex_wrong_dc(lex, lex.make_src(start), digit_count);
        return '\\';
    }

    match integer.try_into() {
        Ok(escaped) => escaped,
        Err(_) => {
            err::lexer_escape_hex_non_utf8(lex, lex.make_src(start), integer);
            '\\'
        }
    }
}

fn lex_number(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();

    if fc == '0' {
        match lex.peek_next() {
            Some('b') => {
                lex.bump(fc);
                lex.bump('b');
                lex_integer_bin(lex, start);
                return;
            }
            Some('o') => {
                lex.bump(fc);
                lex.bump('o');
                lex_integer_oct(lex, start);
                return;
            }
            Some('x') => {
                lex.bump(fc);
                lex.bump('x');
                lex_integer_hex(lex, start);
                return;
            }
            _ => {}
        }
    }

    lex.buffer.clear();

    if skip_zero_digits(lex) {
        lex.buffer.push('0');
    }
    skip_num_digits(lex);

    if lex.eat('.') {
        skip_num_digits(lex);

        if lex.eat('e') {
            lex.eat('-');
            skip_num_digits(lex);
        }

        if let Ok(float) = lex.buffer.parse::<f64>() {
            lex.tokens.add_float(float, lex.make_range(start));
        } else {
            err::lexer_float_parse_failed(lex, lex.make_src(start));
            lex.tokens.add_float(0.0, lex.make_range(start));
        }
    } else {
        lex_integer_dec(lex, start);
    }
}

#[must_use]
fn skip_zero_digits(lex: &mut Lexer) -> bool {
    let mut skipped = false;
    while let Some(c) = lex.peek() {
        match c {
            '0' => {
                lex.bump(c);
                skipped = true;
            }
            '_' => lex.bump(c),
            _ => break,
        };
    }
    skipped
}

fn skip_num_digits(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        match c {
            '0'..='9' => {
                lex.bump(c);
                lex.buffer.push(c);
            }
            '_' => lex.bump(c),
            _ => return,
        };
    }
}

fn lex_integer_dec(lex: &mut Lexer, start: TextOffset) {
    let mut integer: u64 = 0;
    let mut overflow = false;

    for c in lex.buffer.chars() {
        let value: u64 = match c {
            '0'..='9' => c as u64 - '0' as u64,
            _ => unreachable!(),
        };
        let prev_value = integer;
        integer = integer.wrapping_mul(10).wrapping_add(value);
        overflow = overflow || (integer < prev_value);
    }

    if overflow {
        err::lexer_int_dec_overflow(lex, lex.make_src(start));
    }

    lex.tokens.add_int(integer, lex.make_range(start));
}

fn lex_integer_bin(lex: &mut Lexer, start: TextOffset) {
    const MAX_DIGITS: u32 = 64;
    const SHIFT_BASE: u32 = 1;

    let mut integer: u64 = 0;
    let mut digit_count: u32 = 0;

    let skipped = skip_zero_digits(lex);

    while let Some(c) = lex.peek() {
        let value: u64 = match c {
            '0'..='1' => c as u64 - '0' as u64,
            '2'..='9' => {
                let start = lex.start_range();
                lex.bump(c);
                err::lexer_int_bin_invalid_digit(lex, lex.make_src(start), c);
                continue;
            }
            '_' => {
                lex.bump(c);
                continue;
            }
            _ => break,
        };

        lex.bump(c);
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if digit_count == 0 && !skipped {
        err::lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        err::lexer_int_bin_overflow(lex, lex.make_src(start), digit_count);
    }

    lex.tokens.add_int(integer, lex.make_range(start));
}

fn lex_integer_oct(lex: &mut Lexer, start: TextOffset) {
    const MAX_DIGITS: u32 = 22;
    const SHIFT_BASE: u32 = 3;
    const MAX_FIRST_DIGIT: u64 = 1;

    let mut integer: u64 = 0;
    let mut digit_count: u32 = 0;
    let mut first_digit: u64 = 0;

    let skipped = skip_zero_digits(lex);

    while let Some(c) = lex.peek() {
        let value: u64 = match c {
            '0'..='7' => c as u64 - '0' as u64,
            '8'..='9' => {
                let start = lex.start_range();
                lex.bump(c);
                err::lexer_int_oct_invalid_digit(lex, lex.make_src(start), c);
                continue;
            }
            '_' => {
                lex.bump(c);
                continue;
            }
            _ => break,
        };

        if digit_count == 0 {
            first_digit = value;
        }
        lex.bump(c);
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if digit_count == 0 && !skipped {
        err::lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        err::lexer_int_oct_overflow(lex, lex.make_src(start));
    } else if digit_count == MAX_DIGITS && first_digit > MAX_FIRST_DIGIT {
        err::lexer_int_oct_overflow(lex, lex.make_src(start));
    }

    lex.tokens.add_int(integer, lex.make_range(start));
}

fn lex_integer_hex(lex: &mut Lexer, start: TextOffset) {
    const MAX_DIGITS: u32 = 16;
    const SHIFT_BASE: u32 = 4;

    let mut integer: u64 = 0;
    let mut digit_count: u32 = 0;

    let skipped = skip_zero_digits(lex);

    while let Some(c) = lex.peek() {
        let value: u64 = match c {
            '0'..='9' => c as u64 - '0' as u64,
            'a'..='f' => c as u64 - 'a' as u64 + 10,
            'A'..='F' => c as u64 - 'A' as u64 + 10,
            '_' => {
                lex.bump(c);
                continue;
            }
            _ => break,
        };

        lex.bump(c);
        digit_count += 1;
        integer = (integer << SHIFT_BASE) | value;
    }

    if digit_count == 0 && !skipped {
        err::lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        err::lexer_int_hex_overflow(lex, lex.make_src(start), digit_count);
    }

    lex.tokens.add_int(integer, lex.make_range(start));
}

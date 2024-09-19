use super::lexer::Lexer;
use crate::error::{ErrorComp, SourceRange};
use crate::errors as err;
use crate::text::{TextOffset, TextRange};
use crate::token::{Token, Trivia};

pub fn source_file(lex: &mut Lexer) {
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
            lex.eat(c);
            skip_whitespace(lex);

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::Whitespace, range);
            }
        } else if c == '/' && lex.at_next('/') {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('/');

            skip_line_comment(lex);

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::LineComment, range);
            }
        } else if c == '/' && lex.at_next('*') {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('*');

            let depth = skip_block_comment(lex);
            if depth != 0 {
                err::lexer_block_comment_not_terminated(lex, lex.make_src(start), depth);
            }

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::BlockComment, range);
            }
        } else {
            break;
        }
    }
}

fn skip_whitespace(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        if !c.is_ascii_whitespace() {
            return;
        }
        lex.eat(c);
    }
}

fn skip_line_comment(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        if c == '\r' || c == '\n' {
            return;
        }
        lex.eat(c);
    }
}

fn skip_block_comment(lex: &mut Lexer) -> u32 {
    let mut depth: u32 = 1;
    while let Some(c) = lex.peek() {
        lex.eat(c);
        if c == '/' && lex.at('*') {
            lex.eat('*');
            depth += 1;
        } else if c == '*' && lex.at('/') {
            lex.eat('/');
            depth -= 1;
        }
        if depth == 0 {
            return depth;
        }
    }
    depth
}

fn lex_char(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.eat('\'');

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
            lex.eat(fc);
            fc
        }
        '\t' => {
            let start = lex.start_range();
            lex.eat(fc);
            err::lexer_char_tab_not_escaped(lex, lex.make_src(start));
            fc
        }
        _ => {
            lex.eat(fc);
            fc
        }
    };

    let terminated = lex.at('\'');
    if terminated {
        lex.eat('\'');
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
        lex.eat('c');
    }
    lex.eat('\"');

    let mut range;
    let mut string = String::new();
    let mut terminated = false;

    loop {
        while let Some(c) = lex.peek() {
            match c {
                '\r' | '\n' => break,
                '`' if raw => {
                    lex.eat(c);
                    terminated = true;
                    break;
                }
                '"' if !raw => {
                    lex.eat(c);
                    terminated = true;
                    break;
                }
                '\\' if !raw => {
                    let escaped = lex_escape(lex, c_string);
                    string.push(escaped);
                }
                _ => {
                    lex.eat(c);
                    string.push(c);
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

        lex.eat('\"');
        string.push('\n');
        terminated = false;
    }

    lex.tokens.add_string(string, c_string, range);

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
    lex.eat('\\');

    let c = if let Some(c) = lex.peek() {
        c
    } else {
        err::lexer_escape_sequence_incomplete(lex, lex.make_src(start));
        return '\\';
    };

    let mut hex_escaped = false;
    let escaped = match c {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '0' => '\0',
        '\'' => '\'',
        '\"' => '\"',
        '\\' => '\\',
        'x' => {
            lex.eat(c);
            hex_escaped = true;
            lex_escaped_char(lex, start, 2)
        }
        'u' => {
            lex.eat(c);
            hex_escaped = true;
            lex_escaped_char(lex, start, 4)
        }
        'U' => {
            lex.eat(c);
            hex_escaped = true;
            lex_escaped_char(lex, start, 8)
        }
        _ => {
            if c.is_ascii_whitespace() {
                err::lexer_escape_sequence_incomplete(lex, lex.make_src(start));
            } else {
                lex.eat(c);
                err::lexer_escape_sequence_not_supported(lex, lex.make_src(start), c);
            }
            return '\\';
        }
    };

    if !hex_escaped {
        lex.eat(c);
    }
    if c_string && escaped == '\0' {
        err::lexer_escape_sequence_cstring_null(lex, lex.make_src(start));
    }
    escaped
}

//@rework syntax & errors \x{NN} \u{NNNNNN}
fn lex_escaped_char(lex: &mut Lexer, start: TextOffset, digit_count: u32) -> char {
    let mut hex_char: u32 = 0;

    for hex_char_idx in 0..digit_count {
        let hc = match lex.peek() {
            Some(hc) => {
                if hc.is_ascii_whitespace() {
                    None
                } else {
                    Some(hc)
                }
            }
            None => None,
        };

        if let Some(hc) = hc {
            let hex_value = match hc {
                '0'..='9' => hc as u32 - '0' as u32,
                'a'..='f' => hc as u32 - 'a' as u32 + 10,
                'A'..='F' => hc as u32 - 'A' as u32 + 10,
                _ => {
                    let range = lex.make_range(start);
                    lex.errors.push(ErrorComp::new(
                        format!("hexadecimal digit expected 0-9 or A-F, found `{}`", hc),
                        SourceRange::new(lex.module_id, range),
                        None,
                    ));
                    return '?';
                }
            };
            lex.eat(hc);
            hex_char = (hex_char << 4) | hex_value;
        } else {
            let range = lex.make_range(start);
            lex.errors.push(ErrorComp::new(
                format!(
                    "hexadecimal character must have {} hex digits, found {}",
                    digit_count, hex_char_idx
                ),
                SourceRange::new(lex.module_id, range),
                None,
            ));
            return '?';
        }
    }

    if let Ok(escaped) = hex_char.try_into() {
        escaped
    } else {
        let range = lex.make_range(start);
        lex.errors.push(ErrorComp::new(
            format!("hexadecimal character `{}` is not valid UTF-8", hex_char),
            SourceRange::new(lex.module_id, range),
            None,
        ));
        '?'
    }
}

fn lex_number(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();

    if fc == '0' {
        match lex.peek_next() {
            Some('b') => {
                lex.eat(fc);
                lex.eat('b');
                lex_integer_bin(lex, start);
                return;
            }
            Some('o') => {
                lex.eat(fc);
                lex.eat('o');
                lex_integer_oct(lex, start);
                return;
            }
            Some('x') => {
                lex.eat(fc);
                lex.eat('x');
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

    if lex.at('.') {
        lex.eat('.');
        skip_num_digits(lex);
        //@support exponent

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
                lex.eat(c);
                skipped = true;
            }
            '_' => lex.eat(c),
            _ => break,
        };
    }
    skipped
}

fn skip_num_digits(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        match c {
            '0'..='9' => {
                lex.eat(c);
                lex.buffer.push(c);
            }
            '_' => lex.eat(c),
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
                lex.eat(c);
                err::lexer_int_bin_invalid_digit(lex, lex.make_src(start), c);
                continue;
            }
            '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        lex.eat(c);
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
                lex.eat(c);
                err::lexer_int_oct_invalid_digit(lex, lex.make_src(start), c);
                continue;
            }
            '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        if digit_count == 0 {
            first_digit = value;
        }
        lex.eat(c);
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
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        lex.eat(c);
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

fn lex_ident(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.eat(fc);

    while let Some(c) = lex.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            lex.eat(c);
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
    lex.eat(fc);

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
                lex.eat(c);
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
                lex.eat(c);
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

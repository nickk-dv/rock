use super::lexer::Lexer;
use crate::error::{ErrorComp, SourceRange};
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
                    } else if c == '_' || c.is_ascii_alphabetic() {
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
        } else if c == '/' && matches!(lex.peek_next(), Some('/')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('/');
            skip_line_comment(lex);

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens.add_trivia(Trivia::LineComment, range);
            }
        } else if c == '/' && matches!(lex.peek_next(), Some('*')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('*');
            let depth = skip_block_comment(lex);

            if depth != 0 {
                let range = lex.make_range(start);
                lex.errors.push(ErrorComp::new(
                    format!("missing {} block comment terminators `*/`", depth),
                    SourceRange::new(lex.module_id, range),
                    None,
                ));
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
        if c == '\n' {
            return;
        }
        lex.eat(c);
    }
}

fn skip_block_comment(lex: &mut Lexer) -> u32 {
    let mut depth: u32 = 1;
    while let Some(c) = lex.peek() {
        lex.eat(c);
        if c == '/' && matches!(lex.peek(), Some('*')) {
            lex.eat('*');
            depth += 1;
        } else if c == '*' && matches!(lex.peek(), Some('/')) {
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
            if c == '\n' || c == '\r' {
                let range = lex.make_range(start);
                lex.errors.push(ErrorComp::new(
                    "character literal is incomplete",
                    SourceRange::new(lex.module_id, range),
                    None,
                ));
                return;
            }
            c
        }
        None => {
            let range = lex.make_range(start);
            lex.errors.push(ErrorComp::new(
                "character literal is incomplete",
                SourceRange::new(lex.module_id, range),
                None,
            ));
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
            let range = lex.make_range(start);
            lex.errors.push(ErrorComp::new(
                "character literal tab must be escaped: `\\t`",
                SourceRange::new(lex.module_id, range),
                None,
            ));
            fc
        }
        _ => {
            lex.eat(fc);
            fc
        }
    };

    let terminated = matches!(lex.peek(), Some('\''));
    if terminated {
        lex.eat('\'');
    }
    let range = lex.make_range(start);

    match (inner_tick, terminated) {
        (true, false) => {
            // example [ '' ]
            lex.errors.push(ErrorComp::new(
                "character literal cannot be empty",
                SourceRange::new(lex.module_id, range),
                None,
            ));
        }
        (true, true) => {
            // example [ ''' ]
            lex.errors.push(ErrorComp::new(
                "character literal `'` must be escaped: `\\'`",
                SourceRange::new(lex.module_id, range),
                None,
            ));
        }
        (false, false) => {
            // example [ 'x, '\n ]
            lex.errors.push(ErrorComp::new(
                "character literal not terminated, missing closing `'`",
                SourceRange::new(lex.module_id, range),
                None,
            ));
        }
        (false, true) => {
            // example [ 'x', '\n' ]
            // correctly terminated without inner tick
        }
    }

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
        let message: &str = if raw {
            "raw string literal not terminated, missing closing `"
        } else {
            "string literal not terminated, missing closing \""
        };
        lex.errors.push(ErrorComp::new(
            message,
            SourceRange::new(lex.module_id, range),
            None,
        ));
    }
}

fn lex_escape(lex: &mut Lexer, c_string: bool) -> char {
    let start = lex.start_range();
    lex.eat('\\');

    const INCOMPLETE_MSG: &str =
        "escape sequence is incomplete\nif you meant to use `\\`, escape it: `\\\\`";

    let c = if let Some(c) = lex.peek() {
        c
    } else {
        let range = lex.make_range(start);
        lex.errors.push(ErrorComp::new(
            INCOMPLETE_MSG,
            SourceRange::new(lex.module_id, range),
            None,
        ));
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
                let range = lex.make_range(start);
                lex.errors.push(ErrorComp::new(
                    INCOMPLETE_MSG,
                    SourceRange::new(lex.module_id, range),
                    None,
                ));
            } else {
                lex.eat(c);
                let range = lex.make_range(start);
                lex.errors.push(ErrorComp::new(
                    format!("escape sequence `\\{}` is not supported", c),
                    SourceRange::new(lex.module_id, range),
                    None,
                ));
            }
            return '\\';
        }
    };

    if !hex_escaped {
        lex.eat(c);
    }

    if c_string && escaped == '\0' {
        let range = lex.make_range(start);
        lex.errors.push(ErrorComp::new(
            "c string literals cannot contain any `\\0`\nnull terminator is automatically included",
            SourceRange::new(lex.module_id, range),
            None,
        ));
    }
    escaped
}

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

    if matches!(lex.peek(), Some('.')) {
        lex.eat('.');
        skip_num_digits(lex);
        //@support exponent

        if let Ok(float) = lex.buffer.parse::<f64>() {
            lex.tokens.add_float(float, lex.make_range(start));
        } else {
            lexer_float_parse_failed(lex, lex.make_src(start));
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
        lexer_int_dec_overflow(lex, lex.make_src(start));
    }

    let range = lex.make_range(start);
    lex.tokens.add_int(integer, range);
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
                lexer_int_bin_invalid_digit(lex, lex.make_src(start), c);
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
        lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        lexer_int_bin_overflow(lex, lex.make_src(start), digit_count);
    }

    let range = lex.make_range(start);
    lex.tokens.add_int(integer, range);
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
                lexer_int_oct_invalid_digit(lex, lex.make_src(start), c);
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
        lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        lexer_int_oct_overflow_digits(lex, lex.make_src(start), digit_count);
    } else if digit_count == MAX_DIGITS && first_digit > MAX_FIRST_DIGIT {
        lexer_int_oct_overflow_value(lex, lex.make_src(start));
    }

    let range = lex.make_range(start);
    lex.tokens.add_int(integer, range);
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
        lexer_int_base_missing_digits(lex, lex.make_src(start));
    } else if digit_count > MAX_DIGITS {
        lexer_int_hex_overflow(lex, lex.make_src(start), digit_count);
    }

    let range = lex.make_range(start);
    lex.tokens.add_int(integer, range);
}

fn lex_ident(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.eat(fc);

    while let Some(c) = lex.peek() {
        if c == '_' || c.is_ascii_alphanumeric() {
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

    macro_rules! add_token_and_return {
        ($lex:expr, $start:expr, $token:expr) => {{
            let range = $lex.make_range($start);
            $lex.tokens.add_token($token, range);
            return;
        }};
    }

    let mut token = match Token::from_char(fc) {
        Some(sym) => sym,
        None => {
            let range = lex.make_range(start);
            let extra = if !fc.is_ascii() {
                "\nonly ascii symbols are supported"
            } else {
                ""
            };
            lex.errors.push(ErrorComp::new(
                format!("unknown symbol token {:?}{}", fc, extra),
                SourceRange::new(lex.module_id, range),
                None,
            ));
            return;
        }
    };

    match lex.peek() {
        Some(c) => match Token::glue_double(c, token) {
            Some(sym) => {
                lex.eat(c);
                token = sym;
            }
            None => add_token_and_return!(lex, start, token),
        },
        None => add_token_and_return!(lex, start, token),
    }

    match lex.peek() {
        Some(c) => match Token::glue_triple(c, token) {
            Some(sym) => {
                lex.eat(c);
                token = sym;
            }
            None => add_token_and_return!(lex, start, token),
        },
        None => add_token_and_return!(lex, start, token),
    }

    add_token_and_return!(lex, start, token);
}

//@will be moved to errors.rs when Lexer supports ErrorSink
// if unified error handling will be chosed (handle warnings even if none are possible yet)

fn lexer_int_base_missing_digits(lex: &mut Lexer, src: SourceRange) {
    let msg = "missing digits after integer base prefix";
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_bin_invalid_digit(lex: &mut Lexer, src: SourceRange, digit: char) {
    let msg = format!("invalid digit `{digit}` for base 2 binary integer");
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_oct_invalid_digit(lex: &mut Lexer, src: SourceRange, digit: char) {
    let msg = format!("invalid digit `{digit}` for base 8 octal integer");
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_bin_overflow(lex: &mut Lexer, src: SourceRange, digit_count: u32) {
    let msg = format!(
        "binary integer overflow\nexpected maximum of 64 binary digits, found {digit_count}",
    );
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_oct_overflow_value(lex: &mut Lexer, src: SourceRange) {
    let msg = format!("octal integer overflow\nmaximum value is `0o17_77777_77777_77777_77777`",);
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_oct_overflow_digits(lex: &mut Lexer, src: SourceRange, digit_count: u32) {
    let msg = format!(
        "octal integer overflow\nexpected maximum of 22 octal digits, found {digit_count}",
    );
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_hex_overflow(lex: &mut Lexer, src: SourceRange, digit_count: u32) {
    let msg = format!(
        "hexadecimal integer overflow\nexpected maximum of 16 hexadecimal digits, found {digit_count}",
    );
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_int_dec_overflow(lex: &mut Lexer, src: SourceRange) {
    let msg = format!("decimal integer overflow\nmaximum value is `18_446_744_073_709_551_615`");
    lex.errors.push(ErrorComp::new(msg, src, None));
}

fn lexer_float_parse_failed(lex: &mut Lexer, src: SourceRange) {
    let msg = "failed to parse float literal";
    lex.errors.push(ErrorComp::new(msg, src, None));
}

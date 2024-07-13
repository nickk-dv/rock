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

    let dummy_range = TextRange::empty_at(0.into());
    lex.tokens().add_token(Token::Eof, dummy_range);
    lex.tokens().add_token(Token::Eof, dummy_range);
}

fn lex_whitespace(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        if c.is_ascii_whitespace() {
            let start = lex.start_range();
            lex.eat(c);
            skip_whitespace(lex);

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens().add_trivia(Trivia::Whitespace, range);
            }
        } else if c == '/' && matches!(lex.peek_next(), Some('/')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('/');
            skip_line_comment(lex);

            if lex.with_trivia {
                let range = lex.make_range(start);
                lex.tokens().add_trivia(Trivia::LineComment, range);
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
                lex.tokens().add_trivia(Trivia::BlockComment, range);
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
        lex.eat(c);
        if c == '\n' {
            return;
        }
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

    lex.tokens().add_char(char, range);
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

    lex.tokens().add_string(string, c_string, range);

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
                'A'..='Z' => hc as u32 - 'A' as u32 + 10,
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
        match lex.peek() {
            Some('b') => {
                lex.eat(fc);
                lex.eat('b');
                lex_binary_integer(lex, start);
                return;
            }
            Some('x') => {
                lex.eat(fc);
                lex.eat('x');
                lex_hex_integer(lex, start);
                return;
            }
            _ => {}
        }
    }

    //@float parse not implemented, cannot use `lex_decimal_integer` on its own
    //let integer_base = lex_decimal_integer(lex, start);
    //@todo floating point decimal & exponent

    lex.eat(fc);

    //@old number parsing code
    let mut is_float = false;
    while let Some(c) = lex.peek() {
        if c.is_ascii_digit() {
            lex.eat(c);
        } else if c == '.' && !is_float {
            match lex.peek_next() {
                Some(next) => {
                    if next.is_ascii_digit() {
                        is_float = true;
                        lex.eat(c);
                    } else {
                        break;
                    }
                }
                None => break,
            }
        } else {
            break;
        }
    }

    let range = lex.make_range(start);
    let token = if is_float {
        Token::FloatLit
    } else {
        Token::IntLit
    };
    lex.tokens().add_token(token, range);
}

fn lex_binary_integer(lex: &mut Lexer, start: TextOffset) -> u64 {
    const MAX_BITS: u32 = 64;
    let mut integer: u64 = 0;
    let mut bit_idx: u32 = 0;

    while let Some(c) = lex.peek() {
        let bit: u64 = match c {
            '0' => 0,
            '1' => 1,
            '2'..='9' => {
                let digit_start = lex.start_range();
                lex.eat(c);
                let range = lex.make_range(digit_start);
                lex.errors.push(ErrorComp::new(
                    format!("invalid digit `{}` for base 2 binary integer", c),
                    SourceRange::new(lex.module_id, range),
                    None,
                ));
                continue;
            }
            '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        if bit_idx < MAX_BITS {
            integer = (integer << 1) | bit;
        }
        lex.eat(c);
        bit_idx += 1;
    }

    if bit_idx == 0 {
        lex.errors.push(ErrorComp::new(
            "missing digits after integer base prefix",
            SourceRange::new(lex.module_id, lex.make_range(start)),
            None,
        ));
    } else if bit_idx > MAX_BITS {
        lex.errors.push(ErrorComp::new(
            format!(
                "binary integer overflow\nexpected maximum of 64 bits, found {}",
                bit_idx
            ),
            SourceRange::new(lex.module_id, lex.make_range(start)),
            None,
        ));
    }

    let range = lex.make_range(start);
    lex.tokens().add_token(Token::IntLit, range);
    integer
}

fn lex_hex_integer(lex: &mut Lexer, start: TextOffset) -> u64 {
    const MAX_HEX_DIGITS: u32 = 16;
    let mut integer: u64 = 0;
    let mut hex_digit_idx: u32 = 0;

    while let Some(c) = lex.peek() {
        let hex_value: u64 = match c {
            '0'..='9' => c as u64 - '0' as u64,
            'A'..='Z' => c as u64 - 'A' as u64 + 10,
            '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        if hex_digit_idx < MAX_HEX_DIGITS {
            integer = (integer << 4) | hex_value;
        }
        lex.eat(c);
        hex_digit_idx += 1;
    }

    if hex_digit_idx == 0 {
        lex.errors.push(ErrorComp::new(
            "missing digits after integer base prefix",
            SourceRange::new(lex.module_id, lex.make_range(start)),
            None,
        ));
    } else if hex_digit_idx > MAX_HEX_DIGITS {
        lex.errors.push(ErrorComp::new(
            format!(
                "hexadecimal integer overflow\nexpected maximum of 16 hex digits, found {}",
                hex_digit_idx
            ),
            SourceRange::new(lex.module_id, lex.make_range(start)),
            None,
        ));
    }

    let range = lex.make_range(start);
    lex.tokens().add_token(Token::IntLit, range);
    integer
}

fn lex_decimal_integer(lex: &mut Lexer, start: TextOffset) -> u64 {
    const MAX_DECIMAL_DIGITS: u32 = 20;
    let mut integer: u64 = 0;
    let mut digit_idx: u32 = 0;

    while let Some(c) = lex.peek() {
        match c {
            '0' | '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };
    }

    while let Some(c) = lex.peek() {
        let digit_value: u64 = match c {
            '0'..='9' => c as u64 - '0' as u64,
            '_' => {
                lex.eat(c);
                continue;
            }
            _ => break,
        };

        if digit_idx < MAX_DECIMAL_DIGITS {
            integer = integer * 10 + digit_value;
        }
        lex.eat(c);
        digit_idx += 1;
    }

    if digit_idx > MAX_DECIMAL_DIGITS {
        lex.errors.push(ErrorComp::new(
            format!(
                "decimal integer overflow\nexpected maximum of 20 digits, found {}",
                digit_idx
            ),
            SourceRange::new(lex.module_id, lex.make_range(start)),
            None,
        ));
    }

    integer
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
    lex.tokens().add_token(token, range);
}

fn lex_symbol(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.eat(fc);

    macro_rules! add_token_and_return {
        ($lex:expr, $start:expr, $token:expr) => {{
            let range = $lex.make_range($start);
            $lex.tokens().add_token($token, range);
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

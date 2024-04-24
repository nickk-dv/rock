use super::Lexer;
use crate::error::{ErrorComp, SourceRange};
use crate::text::TextRange;
use crate::token::Token;

pub fn source_file(lex: &mut Lexer) {
    while lex.peek().is_some() {
        lex_whitespace(lex);
        if let Some(c) = lex.peek() {
            match c {
                '\'' => lex_char(lex),
                '\"' => lex_string(lex, false),
                '`' => lex_raw_string(lex, false),
                'c' => match lex.peek_next() {
                    Some('\"') => lex_string(lex, true),
                    Some('`') => lex_raw_string(lex, true),
                    _ => lex_ident(lex, c),
                },
                _ => {
                    if c.is_ascii_digit() {
                        lex_number(lex, c);
                    } else if c == '_' || c.is_alphabetic() {
                        lex_ident(lex, c);
                    } else {
                        lex_symbol(lex, c)
                    }
                }
            }
        }
    }

    let dummy_range = TextRange::empty_at(0.into());
    lex.tokens.add_token(Token::Eof, dummy_range);
    lex.tokens.add_token(Token::Eof, dummy_range);
}

fn lex_whitespace(lex: &mut Lexer) {
    while let Some(c) = lex.peek() {
        if c.is_ascii_whitespace() {
            let start = lex.start_range();
            lex.eat(c);
            skip_whitespace(lex);

            if lex.with_whitespace {
                let range = lex.make_range(start);
                lex.tokens.add_token(Token::Whitespace, range);
            }
        } else if c == '/' && matches!(lex.peek_next(), Some('/')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('/');
            skip_line_comment(lex);

            if lex.with_whitespace {
                let range = lex.make_range(start);
                lex.tokens.add_token(Token::LineComment, range);
            }
        } else if c == '/' && matches!(lex.peek_next(), Some('*')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('*');
            let depth = skip_block_comment(lex);

            if depth != 0 {
                let range = lex.make_range(start);
                lex.error(ErrorComp::error(
                    format!("missing {} block comment terminators `*/`", depth),
                    SourceRange::new(range, lex.file_id),
                    None,
                ));
            }
            if lex.with_whitespace {
                let range = lex.make_range(start);
                lex.tokens.add_token(Token::BlockComment, range);
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

fn skip_block_comment(lex: &mut Lexer) -> i32 {
    let mut depth: i32 = 1;
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
                lex.error(ErrorComp::error(
                    "character literal is incomplete",
                    SourceRange::new(range, lex.file_id),
                    None,
                ));
                return;
            }
            c
        }
        None => {
            let range = lex.make_range(start);
            lex.error(ErrorComp::error(
                "character literal is incomplete",
                SourceRange::new(range, lex.file_id),
                None,
            ));
            return;
        }
    };

    let mut inner_tick = false;
    let char = match fc {
        '\\' => lex_escape(lex),
        '\'' => {
            inner_tick = true;
            lex.eat(fc);
            fc
        }
        '\t' => {
            let start = lex.start_range();
            lex.eat(fc);
            let range = lex.make_range(start);
            lex.error(ErrorComp::error(
                "charater literal tab must be escaped: `\\t`",
                SourceRange::new(range, lex.file_id),
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
            lex.error(ErrorComp::error(
                "character literal cannot be empty",
                SourceRange::new(range, lex.file_id),
                None,
            ));
        }
        (true, true) => {
            // example [ ''' ]
            lex.error(ErrorComp::error(
                "character literal `'` must be escaped: `\\'`",
                SourceRange::new(range, lex.file_id),
                None,
            ));
        }
        (false, false) => {
            // example [ 'x, '\n ]
            lex.error(ErrorComp::error(
                "character literal not terminated, missing closing `'`",
                SourceRange::new(range, lex.file_id),
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

fn lex_string(lex: &mut Lexer, c_string: bool) {
    let start = lex.start_range();
    if c_string {
        lex.eat('c');
    }
    lex.eat('\"');

    let mut string = String::new();
    let mut terminated = false;

    while let Some(c) = lex.peek() {
        match c {
            '\n' | '\r' => break,
            '\"' => {
                lex.eat(c);
                terminated = true;
                break;
            }
            '\\' => {
                let escape = if c_string {
                    lex_escape_c_string(lex)
                } else {
                    lex_escape(lex)
                };
                string.push(escape);
            }
            _ => {
                lex.eat(c);
                string.push(c);
            }
        }
    }

    let range = lex.make_range(start);
    lex.tokens.add_string(string, c_string, range);

    if !terminated {
        lex.error(ErrorComp::error(
            "string literal not terminated, missing closing \"",
            SourceRange::new(range, lex.file_id),
            None,
        ));
    }
}

fn lex_raw_string(lex: &mut Lexer, c_string: bool) {
    let start = lex.start_range();
    if c_string {
        lex.eat('c');
    }
    lex.eat('`');

    let mut string = String::new();
    let mut terminated = false;

    while let Some(c) = lex.peek() {
        match c {
            '\n' | '\r' => break,
            '`' => {
                lex.eat(c);
                terminated = true;
                break;
            }
            _ => {
                lex.eat(c);
                string.push(c);
            }
        }
    }

    let range = lex.make_range(start);
    lex.tokens.add_string(string, c_string, range);

    if !terminated {
        lex.error(ErrorComp::error(
            "raw string literal not terminated, missing closing `",
            SourceRange::new(range, lex.file_id),
            None,
        ));
    }
}

fn lex_escape(lex: &mut Lexer) -> char {
    let start = lex.start_range();
    lex.eat('\\');

    if let Some(c) = lex.peek() {
        let escaped = match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0' => '\0',
            '\'' => '\'',
            '\"' => '\"',
            '\\' => '\\',
            _ => {
                let mut range = lex.make_range(start);
                if c.is_ascii_whitespace() {
                    lex.error(ErrorComp::error(
                        "escape sequence is incomplete\nif you meant `\\`, escape it: `\\\\`",
                        SourceRange::new(range, lex.file_id),
                        None,
                    ));
                } else {
                    range.extend_by((c.len_utf8() as u32).into());
                    lex.error(ErrorComp::error(
                        format!("escape sequence `\\{}` is not supported", c),
                        SourceRange::new(range, lex.file_id),
                        None,
                    ));
                }
                '\\'
            }
        };

        lex.eat(c);
        escaped
    } else {
        let range = lex.make_range(start);
        lex.error(ErrorComp::error(
            "escape sequence is incomplete\nif you meant `\\`, escape it: `\\\\`",
            SourceRange::new(range, lex.file_id),
            None,
        ));
        '\\'
    }
}

fn lex_escape_c_string(lex: &mut Lexer) -> char {
    let start = lex.start_range();
    let escape = lex_escape(lex);
    if escape == '\0' {
        let range = lex.make_range(start);
        lex.error(ErrorComp::error(
            "c string literals cannot contain any `\\0`\nnull terminator is automatically included",
            SourceRange::new(range, lex.file_id),
            None,
        ));
    }
    escape
}

fn lex_number(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.eat(fc);

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
    lex.tokens.add_token(token, range);
}

fn lex_ident(lex: &mut Lexer, fc: char) {
    let start = lex.start_range();
    lex.eat(fc);

    while let Some(c) = lex.peek() {
        if c == '_' || c.is_ascii_digit() || c.is_alphabetic() {
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
            lex.tokens.add_token(Token::Error, lex.make_range(start));
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

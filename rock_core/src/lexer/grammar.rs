use super::lexer::Lexer;
use crate::error::{ErrorComp, SourceRange};
use crate::text::TextRange;
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

    let escaped = match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        '0' => '\0',
        '\'' => '\'',
        '\"' => '\"',
        '\\' => '\\',
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

    lex.eat(c);
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
    lex.tokens().add_token(token, range);
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
                "\nonly ascii characters are supported"
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

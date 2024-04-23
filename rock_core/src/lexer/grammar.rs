use super::Lexer;
use crate::error::{ErrorComp, SourceRange};
use crate::text::{TextOffset, TextRange};
use crate::token::Token;

//@validate missing \0 in c_strings with precise error report 23.04.24
pub fn source_file(lex: &mut Lexer) {
    while lex.peek().is_some() {
        skip_whitespace(lex);
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

    //@parser doesnt lookahead by more then by 1 token, using 4 as failsafe 23.04.24
    for _ in 0..4 {
        let dummy_range = TextRange::empty_at(0.into());
        lex.tokens.add_token(Token::Eof, dummy_range);
    }
}

//@might want to split tokens into line comments / whitescape / block comments
// for use in formatting the syntax tree 23.04.24
fn skip_whitespace(lex: &mut Lexer) {
    let start = lex.start_range();

    while let Some(c) = lex.peek() {
        if c.is_ascii_whitespace() {
            lex.eat(c);
        } else if c == '/' && matches!(lex.peek_next(), Some('/')) {
            lex.eat(c);
            lex.eat('/');
            skip_line_comment(lex);
        } else if c == '/' && matches!(lex.peek_next(), Some('*')) {
            let start = lex.start_range();
            lex.eat(c);
            lex.eat('*');
            skip_block_comment(lex, start);
        } else {
            break;
        }
    }

    if lex.with_whitespace {
        let range = lex.make_range(start);
        if !range.is_empty() {
            lex.tokens.add_token(Token::Whitespace, range)
        }
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

fn skip_block_comment(lex: &mut Lexer, start: TextOffset) {
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
            return;
        }
    }

    if depth != 0 {
        let range = lex.make_range(start);
        lex.error(ErrorComp::error(
            format!("missing {} block comment terminators `*/`", depth),
            SourceRange::new(range, lex.file_id),
            None,
        ));
    }
}

fn lex_escape(lex: &mut Lexer) -> Result<char, bool> {
    if let Some(c) = lex.peek() {
        lex.eat(c); //@always eat?
        match c {
            'n' => Ok('\n'),
            't' => Ok('\t'),
            'r' => Ok('\r'),
            '0' => Ok('\0'),
            '\'' => Ok('\''),
            '\"' => Ok('\"'),
            '\\' => Ok('\\'),
            _ => Err(true),
        }
    } else {
        Err(false)
    }
}

fn lex_char(lex: &mut Lexer) {
    let start = lex.start_range();
    lex.eat('\'');

    let fc = match lex.peek() {
        Some(c) => {
            lex.eat(c);
            c
        }
        None => {
            let range = lex.make_range(start);
            lex.error(ErrorComp::error(
                "expected character literal",
                SourceRange::new(range, lex.file_id),
                None,
            ));
            lex.tokens.add_char(' ', range);
            return;
        }
    };

    let mut terminated = false;
    let char = match fc {
        '\\' => {
            //@self.eat(fc); figure out escape handling later 23.04.24
            match lex_escape(lex) {
                Ok(char) => char,
                Err(invalid) => {
                    if invalid {
                        panic!("char lit invalid escape sequence");
                    }
                    panic!("char lit incomplete escape sequence");
                }
            }
        }
        '\'' => {
            terminated = true;
            fc
        }
        _ => fc,
    };

    let has_escape = matches!(lex.peek(), Some('\''));
    if has_escape {
        lex.eat('\'');
    }
    if terminated && !has_escape {
        // example [ '' ]
        panic!("char literal cannot be empty");
    }
    if terminated && has_escape {
        // example [ ''' ]
        panic!("char literal `'` must be escaped: `\\'`");
    }
    if !terminated && !has_escape {
        // example [ 'x ]
        panic!("char literal not terminated, missing closing `'`");
    }

    let range = lex.make_range(start);
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
            '\r' | '\n' => break,
            '\"' => {
                lex.eat(c);
                terminated = true;
                break;
            }
            '\\' => {
                lex.eat(c); // @is this correct?
                let char = match lex_escape(lex) {
                    Ok(char) => char,
                    Err(invalid) => {
                        if invalid {
                            //@report errors directly in lex_escape 23.04.24
                            let range = lex.make_range(start);
                            eprintln!("at range: {}", &lex.source[range.as_usize()]);
                            panic!("string lit invalid escape sequence");
                        }
                        panic!("string lit incomplete escape sequence");
                    }
                };
                string.push(char);
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
            '\r' | '\n' => break,
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

use super::Token;
use crate::text::TextRange;

pub struct TokenList {
    tokens: Vec<Token>,
    ranges: Vec<TextRange>,
    chars: Vec<char>,
    strings: Vec<String>,
}

impl TokenList {
    pub fn new(cap: usize) -> TokenList {
        TokenList {
            tokens: Vec::with_capacity(cap),
            ranges: Vec::with_capacity(cap),
            chars: Vec::new(),
            strings: Vec::new(),
        }
    }

    pub fn add_token(&mut self, token: Token, range: TextRange) {
        self.tokens.push(token);
        self.ranges.push(range);
    }
    pub fn add_char(&mut self, c: char, range: TextRange) {
        self.tokens.push(Token::CharLit);
        self.ranges.push(range);
        self.chars.push(c);
    }
    pub fn add_string(&mut self, s: String, range: TextRange) {
        self.tokens.push(Token::StringLit);
        self.ranges.push(range);
        self.strings.push(s);
    }

    pub fn get_token(&self, index: usize) -> Token {
        self.tokens[index]
    }
    pub fn get_range(&self, index: usize) -> TextRange {
        self.ranges[index]
    }
    pub fn get_char(&self, index: usize) -> char {
        self.chars[index]
    }
    pub fn get_string(&self, index: usize) -> &str {
        &self.strings[index]
    }
}

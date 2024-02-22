use super::span::Span;
use super::token::Token;

pub struct TokenList {
    tokens: Vec<Token>,
    spans: Vec<Span>,
    chars: Vec<char>,
    strings: Vec<String>,
}

impl TokenList {
    pub fn new(cap: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(cap),
            spans: Vec::with_capacity(cap),
            chars: Vec::new(),
            strings: Vec::new(),
        }
    }

    pub fn add_token(&mut self, token: Token, span: Span) {
        self.tokens.push(token);
        self.spans.push(span);
    }

    pub fn add_char(&mut self, c: char, span: Span) {
        self.tokens.push(Token::CharLit);
        self.spans.push(span);
        self.chars.push(c);
    }

    pub fn add_string(&mut self, s: String, span: Span) {
        self.tokens.push(Token::StringLit);
        self.spans.push(span);
        self.strings.push(s);
    }

    pub fn get_token(&self, index: usize) -> Token {
        unsafe { *self.tokens.get_unchecked(index) }
    }

    pub fn get_span(&self, index: usize) -> Span {
        unsafe { *self.spans.get_unchecked(index) }
    }

    pub fn get_char(&self, index: usize) -> char {
        unsafe { *self.chars.get_unchecked(index) }
    }

    pub fn strings(self) -> Vec<String> {
        self.strings
    }
}

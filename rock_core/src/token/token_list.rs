use super::{Token, Trivia};
use crate::text::TextRange;

pub struct TokenList {
    tokens: Vec<Token>,
    token_ranges: Vec<TextRange>,
    trivias: Vec<Trivia>,
    trivia_ranges: Vec<TextRange>,
    ints: Vec<u64>,
    chars: Vec<char>,
    strings: Vec<(String, bool)>,
}

impl TokenList {
    pub fn new(cap: usize) -> TokenList {
        TokenList {
            tokens: Vec::with_capacity(cap),
            token_ranges: Vec::with_capacity(cap),
            trivias: Vec::new(),
            trivia_ranges: Vec::new(),
            ints: Vec::new(),
            chars: Vec::new(),
            strings: Vec::new(),
        }
    }

    pub fn token(&self, index: usize) -> Token {
        self.tokens[index]
    }
    pub fn token_range(&self, index: usize) -> TextRange {
        self.token_ranges[index]
    }
    pub fn trivia(&self, index: usize) -> Trivia {
        self.trivias[index]
    }
    pub fn trivia_range(&self, index: usize) -> TextRange {
        self.trivia_ranges[index]
    }
    pub fn trivia_count(&self) -> usize {
        self.trivias.len()
    }
    pub fn int(&self, index: usize) -> u64 {
        self.ints[index]
    }
    pub fn char(&self, index: usize) -> char {
        self.chars[index]
    }
    pub fn string(&self, index: usize) -> (&str, bool) {
        let (string, c_string) = &self.strings[index];
        (string, *c_string)
    }

    pub fn add_token(&mut self, token: Token, range: TextRange) {
        self.tokens.push(token);
        self.token_ranges.push(range);
    }
    pub fn add_trivia(&mut self, trivia: Trivia, range: TextRange) {
        self.trivias.push(trivia);
        self.trivia_ranges.push(range);
    }
    pub fn add_int(&mut self, i: u64, range: TextRange) {
        self.tokens.push(Token::IntLit);
        self.token_ranges.push(range);
        self.ints.push(i);
    }
    pub fn add_char(&mut self, c: char, range: TextRange) {
        self.tokens.push(Token::CharLit);
        self.token_ranges.push(range);
        self.chars.push(c);
    }
    pub fn add_string(&mut self, s: String, c_string: bool, range: TextRange) {
        self.tokens.push(Token::StringLit);
        self.token_ranges.push(range);
        self.strings.push((s, c_string));
    }
}

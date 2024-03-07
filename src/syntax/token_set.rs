use crate::ast::token::Token;

#[derive(Clone, Copy)]
pub struct TokenSet(u128);

impl TokenSet {
    pub const EMPTY: TokenSet = TokenSet(0u128);

    pub const fn new(tokens: &[Token]) -> TokenSet {
        let mut bitset = 0u128;
        let mut i = 0;
        while i < tokens.len() {
            bitset |= bit_mask(tokens[i]);
            i += 1;
        }
        TokenSet(bitset)
    }

    pub const fn combine(self, other: TokenSet) -> TokenSet {
        TokenSet(self.0 | other.0)
    }

    pub const fn contains(&self, token: Token) -> bool {
        self.0 & bit_mask(token) != 0
    }
}

const fn bit_mask(token: Token) -> u128 {
    1u128 << (token as usize)
}

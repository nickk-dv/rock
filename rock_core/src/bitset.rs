#[derive(Copy, Clone)]
pub struct BitSet(u32);

impl BitSet {
    pub const EMPTY: BitSet = BitSet(0);

    pub const fn new(flags: &[u32]) -> BitSet {
        let mut bitset = BitSet::EMPTY;
        let mut i = 0;
        while i < flags.len() {
            bitset.0 |= 1 << flags[i];
            i += 1;
        }
        bitset
    }

    pub fn set(&mut self, flag: impl Into<u32>) {
        self.0 |= 1 << flag.into();
    }

    pub fn contains(self, flag: impl Into<u32>) -> bool {
        self.0 & (1 << flag.into()) != 0
    }
}

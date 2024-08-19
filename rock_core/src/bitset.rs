use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct BitSet<T>
where
    T: Copy + Clone + Into<u32>,
{
    mask: u32,
    phantom: PhantomData<T>,
}

impl<T> BitSet<T>
where
    T: Copy + Clone + Into<u32>,
{
    pub fn empty() -> BitSet<T> {
        BitSet {
            mask: 0,
            phantom: PhantomData::default(),
        }
    }

    pub fn set(&mut self, flag: T) {
        self.mask |= 1 << flag.into();
    }

    pub fn contains(self, flag: T) -> bool {
        self.mask & (1 << flag.into()) != 0
    }
}

use crate::support::Arena;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};

crate::define_id!(pub LitID);
crate::define_id!(pub NameID);

pub struct InternPool<'intern, T: InternID> {
    arena: Arena<'intern>,
    values: Vec<&'intern str>,
    intern_map: HashMap<&'intern str, T, Fnv1aHasher>,
}

impl<'intern, T: InternID + Copy> InternPool<'intern, T> {
    pub fn new(cap: usize) -> InternPool<'intern, T> {
        InternPool {
            arena: Arena::new(),
            values: Vec::with_capacity(cap),
            intern_map: HashMap::with_capacity_and_hasher(cap, Fnv1aHasher),
        }
    }

    pub fn intern(&mut self, string: &str) -> T {
        if let Some(id) = self.intern_map.get(string).copied() {
            return id;
        }
        let id = T::from_usize(self.values.len());
        let str = self.arena.alloc_str(string);
        self.values.push(str);
        self.intern_map.insert(str, id);
        id
    }

    pub fn get(&self, id: T) -> &'intern str {
        self.values[id.into_usize()]
    }
    pub fn get_all(&self) -> &[&'intern str] {
        &self.values
    }
    pub fn get_id(&self, string: &str) -> Option<T> {
        self.intern_map.get(string).copied()
    }
}

pub trait InternID {
    fn from_usize(val: usize) -> Self;
    fn into_usize(self) -> usize;
}

impl InternID for LitID {
    #[inline(always)]
    fn from_usize(val: usize) -> Self {
        LitID::new(val)
    }
    #[inline(always)]
    fn into_usize(self) -> usize {
        self.0 as usize
    }
}

impl InternID for NameID {
    #[inline(always)]
    fn from_usize(val: usize) -> NameID {
        NameID::new(val)
    }
    #[inline(always)]
    fn into_usize(self) -> usize {
        self.0 as usize
    }
}

const FNV_OFFSET: u32 = 2166136261;
const FNV_PRIME: u32 = 16777619;

struct Fnv1aHasher;
struct Fnv1a {
    hash: u32,
}

impl BuildHasher for Fnv1aHasher {
    type Hasher = Fnv1a;
    fn build_hasher(&self) -> Fnv1a {
        Fnv1a { hash: FNV_OFFSET }
    }
}

impl Hasher for Fnv1a {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.hash ^= byte as u32;
            self.hash = self.hash.wrapping_mul(FNV_PRIME);
        }
    }
    fn finish(&self) -> u64 {
        self.hash as u64
    }
}

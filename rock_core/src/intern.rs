use crate::support::{Arena, ID};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::marker::PhantomData;

pub struct InternLit;
pub struct InternName;

pub struct InternPool<'intern, T> {
    arena: Arena<'intern>,
    values: Vec<&'intern str>,
    intern_map: HashMap<&'intern str, ID<T>, Fnv1aHasher>,
    phantom: PhantomData<T>,
}

impl<'intern, T> InternPool<'intern, T> {
    pub fn new() -> InternPool<'intern, T> {
        InternPool {
            arena: Arena::new(),
            values: Vec::with_capacity(1024),
            intern_map: HashMap::with_capacity_and_hasher(1024, Fnv1aHasher),
            phantom: PhantomData,
        }
    }

    pub fn intern(&mut self, string: &str) -> ID<T> {
        if let Some(id) = self.intern_map.get(string).cloned() {
            return id;
        }
        let id = ID::new_raw(self.values.len());
        let str = self.arena.alloc_str(string);
        self.values.push(str);
        self.intern_map.insert(str, id);
        id
    }

    pub fn get(&self, id: ID<T>) -> &'intern str {
        self.values[id.raw_index()]
    }
    pub fn get_all(&self) -> &[&'intern str] {
        &self.values
    }
    pub fn get_id(&self, string: &str) -> Option<ID<T>> {
        self.intern_map.get(string).copied()
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

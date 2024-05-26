use crate::arena::Arena;
use crate::id_impl;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};

id_impl!(InternID);
pub struct InternPool<'intern> {
    next: InternID,
    arena: Arena<'intern>,
    strings: Vec<&'intern str>,
    intern_map: HashMap<&'intern str, InternID, Fnv1aHasher>,
}

impl<'intern> InternPool<'intern> {
    pub fn new() -> InternPool<'intern> {
        InternPool {
            next: InternID(0),
            arena: Arena::new(),
            strings: Vec::with_capacity(1024),
            intern_map: HashMap::with_capacity_and_hasher(1024, Fnv1aHasher),
        }
    }

    pub fn intern(&mut self, string: &str) -> InternID {
        if let Some(id) = self.intern_map.get(string).cloned() {
            return id;
        }
        let id = self.next;
        let str = self.arena.alloc_str(string);
        self.next.0 = self.next.0.wrapping_add(1);
        self.strings.push(str);
        self.intern_map.insert(str, id);
        id
    }

    pub fn get_str(&self, id: InternID) -> &str {
        self.strings[id.index()]
    }
    pub fn get_id(&self, string: &str) -> Option<InternID> {
        self.intern_map.get(string).cloned()
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

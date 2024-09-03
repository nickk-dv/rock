use crate::arena::Arena;
use crate::macros::{IndexID, ID};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::marker::PhantomData;

pub struct InternPool<'intern, T: Interned<'intern>> {
    arena: Arena<'intern>,
    values: Vec<T>,
    intern_map: HashMap<&'intern str, ID<T>, Fnv1aHasher>,
    phantom: PhantomData<T>,
}

impl<'intern, T: Interned<'intern>> InternPool<'intern, T> {
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
        let id = ID::new(&self.values);
        let str = self.arena.alloc_str(string);
        self.values.push(T::from_str(str));
        self.intern_map.insert(str, id);
        id
    }

    pub fn get(&self, id: ID<T>) -> &T {
        self.values.id_get(id)
    }
    pub fn get_all(&self) -> &[T] {
        &self.values
    }
    pub fn get_id(&self, value: T) -> Option<ID<T>> {
        self.intern_map.get(value.as_str()).copied()
    }
}

#[derive(Copy, Clone)]
pub struct InternName<'intern>(&'intern str);

#[derive(Copy, Clone)]
pub struct InternString<'intern>(&'intern str);

pub trait Interned<'intern> {
    fn as_str(&self) -> &'intern str;
    fn from_str(string: &'intern str) -> Self;
}

impl<'intern> Interned<'intern> for InternName<'intern> {
    fn as_str(&self) -> &'intern str {
        self.0
    }
    fn from_str(string: &'intern str) -> Self {
        InternName(string)
    }
}

impl<'intern> Interned<'intern> for InternString<'intern> {
    fn as_str(&self) -> &'intern str {
        self.0
    }
    fn from_str(string: &'intern str) -> Self {
        InternString(string)
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

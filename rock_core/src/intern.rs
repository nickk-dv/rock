use crate::support::Arena;
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::hash::Hash;

crate::define_id!(pub LitID);
crate::define_id!(pub NameID);

pub struct InternPool<ID, T> {
    values: Vec<T>,
    intern_map: FxHashMap<T, ID>,
}

pub struct StringPool<'s, ID> {
    arena: Arena<'s>,
    values: Vec<&'s str>,
    intern_map: FxHashMap<&'s str, ID>,
}

impl<ID, T> InternPool<ID, T>
where
    ID: Copy + From<usize> + Into<usize>,
    T: Copy + Eq + Hash,
{
    pub fn new(cap: usize) -> Self {
        InternPool {
            values: Vec::with_capacity(cap),
            intern_map: FxHashMap::with_capacity_and_hasher(cap, FxBuildHasher),
        }
    }
    pub fn intern(&mut self, value: T) -> ID {
        if let Some(id) = self.intern_map.get(&value).copied() {
            return id;
        }
        let id = ID::from(self.values.len());
        self.values.push(value);
        self.intern_map.insert(value, id);
        id
    }
    pub fn get(&self, id: ID) -> T {
        self.values[id.into()]
    }
    pub fn get_all(&self) -> &[T] {
        &self.values
    }
    pub fn get_id(&self, value: T) -> Option<ID> {
        self.intern_map.get(&value).copied()
    }
}

impl<'s, ID> StringPool<'s, ID>
where
    ID: Copy + From<usize> + Into<usize>,
{
    pub fn new(cap: usize) -> Self {
        StringPool {
            arena: Arena::new(),
            values: Vec::with_capacity(cap),
            intern_map: FxHashMap::with_capacity_and_hasher(cap, FxBuildHasher),
        }
    }
    pub fn intern(&mut self, value: &str) -> ID {
        if let Some(id) = self.intern_map.get(&value).copied() {
            return id;
        }
        let id = ID::from(self.values.len());
        let str = self.arena.alloc_str(value);
        self.values.push(str);
        self.intern_map.insert(str, id);
        id
    }
    pub fn get(&self, id: ID) -> &'s str {
        self.values[id.into()]
    }
    pub fn get_all(&self) -> &[&'s str] {
        &self.values
    }
    pub fn get_id(&self, value: &str) -> Option<ID> {
        self.intern_map.get(&value).copied()
    }
}

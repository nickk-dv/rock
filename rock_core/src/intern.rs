use crate::support::Arena;
use rustc_hash::{FxBuildHasher, FxHashMap};

crate::define_id!(pub LitID);
crate::define_id!(pub NameID);

pub struct InternPool<'intern, T: InternID> {
    arena: Arena<'intern>,
    values: Vec<&'intern str>,
    intern_map: FxHashMap<&'intern str, T>,
}

impl<'intern, T: InternID + Copy> InternPool<'intern, T> {
    pub fn new(cap: usize) -> InternPool<'intern, T> {
        InternPool {
            arena: Arena::new(),
            values: Vec::with_capacity(cap),
            intern_map: FxHashMap::with_capacity_and_hasher(cap, FxBuildHasher),
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

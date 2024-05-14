use super::{ConstArray, ConstStruct, ConstValue};
use crate::arena::Arena;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ConstValueID(u32);

pub struct ConstInternPool<'hir> {
    next: ConstValueID,
    arena: Arena<'hir>,
    values: Vec<ConstValue<'hir>>,
    intern_map: HashMap<ConstValue<'hir>, ConstValueID>,
}

impl<'hir> ConstInternPool<'hir> {
    pub fn new() -> ConstInternPool<'hir> {
        ConstInternPool {
            next: ConstValueID(0),
            arena: Arena::new(),
            values: Vec::with_capacity(1024),
            intern_map: HashMap::with_capacity(1024),
        }
    }

    pub fn intern(&mut self, value: ConstValue<'hir>) -> ConstValueID {
        if let Some(id) = self.intern_map.get(&value).cloned() {
            return id;
        }
        let id = self.next;
        self.next.0 = self.next.0.wrapping_add(1);
        self.values.push(value);
        self.intern_map.insert(value, id);
        id
    }

    pub fn get(&self, id: ConstValueID) -> ConstValue<'hir> {
        self.values[id.0 as usize]
    }

    pub fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.arena
    }
}

impl<'hir> Eq for ConstValue<'hir> {}

//@perf: test for hash collision rates and how well this performs in terms of speed 09.05.24
impl<'hir> Hash for ConstValue<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            ConstValue::Error => 0.hash(state),
            ConstValue::Null => 1.hash(state),
            ConstValue::Bool { val } => val.hash(state),
            ConstValue::Int { val, neg, ty } => (val, neg, ty).hash(state),
            ConstValue::Float { val, ty } => (val.to_bits(), ty).hash(state),
            ConstValue::Char { val } => val.hash(state),
            ConstValue::String { id, c_string } => (id, c_string).hash(state),
            ConstValue::Struct { struct_ } => struct_.hash(state),
            ConstValue::Array { array } => array.hash(state),
            ConstValue::ArrayRepeat { value, len } => (value, len).hash(state),
        }
    }
}

impl<'hir> Hash for ConstStruct<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.struct_id.0, self.fields).hash(state);
    }
}

impl<'hir> Hash for ConstArray<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.len, self.values).hash(state);
    }
}

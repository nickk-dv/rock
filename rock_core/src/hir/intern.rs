use super::{ConstArray, ConstEnum, ConstStruct, ConstValue, ConstValueID};
use crate::arena::Arena;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct ConstInternPool<'hir> {
    arena: Arena<'hir>,
    values: Vec<ConstValue<'hir>>,
    intern_map: HashMap<ConstValue<'hir>, ConstValueID>,
}

impl<'hir> ConstInternPool<'hir> {
    pub fn new() -> ConstInternPool<'hir> {
        ConstInternPool {
            arena: Arena::new(),
            values: Vec::with_capacity(1024),
            intern_map: HashMap::with_capacity(1024),
        }
    }

    pub fn intern(&mut self, value: ConstValue<'hir>) -> ConstValueID {
        if let Some(id) = self.intern_map.get(&value).cloned() {
            return id;
        }
        let id = ConstValueID::new(self.values.len());
        self.values.push(value);
        self.intern_map.insert(value, id);
        id
    }

    pub fn get(&self, id: ConstValueID) -> ConstValue<'hir> {
        self.values[id.index()]
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
            ConstValue::Int { val, neg, int_ty } => (val, neg, int_ty).hash(state),
            ConstValue::IntS(val) => val.hash(state),
            ConstValue::IntU(val) => val.hash(state),
            ConstValue::Float { val, float_ty } => (val.to_bits(), float_ty).hash(state),
            ConstValue::Char { val } => val.hash(state),
            ConstValue::String { id, c_string } => (id, c_string).hash(state),
            ConstValue::Procedure { proc_id } => proc_id.0.hash(state),
            ConstValue::EnumVariant { enum_ } => enum_.hash(state),
            ConstValue::Struct { struct_ } => struct_.hash(state),
            ConstValue::Array { array } => array.hash(state),
            ConstValue::ArrayRepeat { value, len } => (value.0, len).hash(state),
        }
    }
}

impl<'hir> Hash for ConstEnum<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.enum_id.0, self.variant_id.0, self.values).hash(state);
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

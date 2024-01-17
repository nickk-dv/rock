use std::collections::HashMap;

pub type InternID = u32;

pub struct InternPool {
    current_id: InternID,
    string_bytes: Vec<u8>,
    string_map: HashMap<u32, InternString>,
    strings: Vec<InternString>,
}

#[derive(Copy, Clone)]
pub struct InternString {
    pub id: InternID,
    pub start: u32,
    pub end: u32,
}

impl InternPool {
    pub fn new() -> Self {
        Self {
            current_id: 0,
            string_bytes: Vec::new(),
            string_map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    pub fn get_byte_slice(&self, id: InternID) -> &[u8] {
        let intern_str = unsafe { self.strings.get_unchecked(id as usize) };
        return &self.string_bytes[intern_str.start as usize..intern_str.end as usize];
    }

    pub fn get_string(&self, id: InternID) -> String {
        let byte_slice = self.get_byte_slice(id);
        unsafe {
            let str = std::str::from_utf8_unchecked(byte_slice);
            str.to_string()
        }
    }

    pub fn get_id_if_exists(&self, bytes: &[u8]) -> Option<InternID> {
        let hash = Self::hash_fnva1(bytes);
        if let Some(intern_str) = self.string_map.get(&hash) {
            if self.compare(intern_str, bytes) {
                return Some(intern_str.id);
            }
        }
        None
    }

    pub fn intern(&mut self, bytes: &[u8]) -> InternID {
        let hash = Self::hash_fnva1(bytes);
        if let Some(intern_str) = self.string_map.get(&hash) {
            if self.compare(intern_str, bytes) {
                return intern_str.id;
            }
        }
        let start = self.string_bytes.len();
        self.string_bytes.extend_from_slice(bytes);
        let end = self.string_bytes.len();
        let intern_str = InternString {
            id: self.current_id,
            start: start as u32,
            end: end as u32,
        };
        self.string_map.insert(hash, intern_str);
        self.strings.push(intern_str);
        self.current_id += 1;
        return self.current_id - 1;
    }

    fn hash_fnva1(bytes: &[u8]) -> u32 {
        const FNV_OFFSET: u32 = 0x811c_9dc5;
        const FNV_PRIME: u32 = 0x0100_0193;

        let mut hash: u32 = FNV_OFFSET;
        for &byte in bytes {
            hash ^= byte as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        return hash;
    }

    fn compare(&self, intern_str: &InternString, bytes: &[u8]) -> bool {
        let len = intern_str.end - intern_str.start;
        if len as usize != bytes.len() {
            return false;
        }
        for i in 0..len {
            if self.string_bytes[intern_str.start as usize + i as usize] != bytes[i as usize] {
                return false;
            }
        }
        true
    }
}

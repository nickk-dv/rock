use std::collections::HashMap;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct InternID(pub u32);
pub const INTERN_DUMMY_ID: InternID = InternID(u32::MAX);

pub struct InternPool {
    next: InternID,
    bytes: Vec<u8>,
    strings: Vec<InternString>,
    intern_map: HashMap<u32, InternID>,
}

struct InternString {
    start: u32,
    end: u32,
}

impl InternPool {
    pub fn new() -> Self {
        Self {
            next: InternID(0),
            bytes: Vec::new(),
            strings: Vec::new(),
            intern_map: HashMap::new(),
        }
    }

    pub fn intern(&mut self, string: &str) -> InternID {
        let hash = Self::hash_djb2(string);
        if let Some(id) = self.intern_map.get(&hash).cloned() {
            if self.string_compare(id, string) {
                return id;
            }
        };

        let start = self.bytes.len() as u32;
        self.bytes.extend_from_slice(string.as_bytes());
        let end = self.bytes.len() as u32;
        self.strings.push(InternString { start, end });

        let id = self.next;
        self.intern_map.insert(hash, id);
        self.next = InternID(self.next.0.wrapping_add(1));
        return id;
    }

    pub fn get_bytes(&self, id: InternID) -> &[u8] {
        let is = unsafe { self.strings.get_unchecked(id.0 as usize) };
        unsafe { self.bytes.get_unchecked(is.start as usize..is.end as usize) }
    }

    pub fn try_get_str_id(&self, string: &str) -> Option<InternID> {
        let hash = Self::hash_djb2(string);
        if let Some(id) = self.intern_map.get(&hash).cloned() {
            if self.string_compare(id, string) {
                return Some(id);
            }
        };
        None
    }

    fn string_compare(&self, id: InternID, string: &str) -> bool {
        let bytes = self.get_bytes(id);
        let slice = unsafe { std::str::from_utf8_unchecked(bytes) };
        string.chars().eq(slice.chars())
    }

    fn hash_djb2(string: &str) -> u32 {
        let mut hash: u32 = 5381;
        for c in string.chars() {
            hash = ((hash << 5).wrapping_add(hash)) ^ c as u32;
        }
        hash
    }
}

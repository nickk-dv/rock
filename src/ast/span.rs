#[derive(Copy, Clone)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn slice<'src>(&self, source: &'src str) -> &'src str {
        let range = self.start as usize..self.end as usize;
        if let Some(slice) = source.get(range) {
            slice
        } else {
            "INVALID SPAN SLICE"
        }
    }
}

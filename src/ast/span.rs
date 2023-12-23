#[derive(Copy, Clone)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn str<'a>(&'a self, string: &'a String) -> &str {
        Self::str_range(self.start, self.end, string)
    }

    pub fn str_range(start: u32, end: u32, string: &String) -> &str {
        &string[start as usize..end as usize]
    }
}

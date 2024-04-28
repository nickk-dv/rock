use std::fmt;

#[derive(Copy, Clone, PartialEq)]
pub struct TextRange {
    start: TextOffset,
    end: TextOffset,
}

#[derive(Copy, Clone, PartialEq)]
pub struct TextLocation {
    line: u32,
    col: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextOffset(u32);

impl TextRange {
    #[inline]
    pub const fn new(start: TextOffset, end: TextOffset) -> TextRange {
        assert!(start.0 <= end.0);
        TextRange { start, end }
    }
    #[inline]
    pub const fn empty_at(offset: TextOffset) -> TextRange {
        TextRange {
            start: offset,
            end: offset,
        }
    }
    #[inline]
    pub const fn start(self) -> TextOffset {
        self.start
    }
    #[inline]
    pub const fn end(self) -> TextOffset {
        self.end
    }
    #[inline]
    pub const fn len(self) -> usize {
        (self.end.0 - self.start.0) as usize
    }
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn as_usize(self) -> std::ops::Range<usize> {
        self.start.into()..self.end.into()
    }
    #[inline]
    pub fn extend_by(&mut self, by: TextOffset) {
        self.end += by;
    }
    #[inline]
    const fn contains_exclusive(self, offset: TextOffset) -> bool {
        offset.0 >= self.start.0 && offset.0 < self.end.0
    }
    #[inline]
    const fn contains_inclusive(self, offset: TextOffset) -> bool {
        offset.0 >= self.start.0 && offset.0 <= self.end.0
    }
}

impl TextLocation {
    #[inline]
    pub const fn new(line: u32, col: u32) -> TextLocation {
        TextLocation { line, col }
    }
    #[inline]
    pub const fn line(&self) -> u32 {
        self.line
    }
    #[inline]
    pub const fn col(&self) -> u32 {
        self.col
    }
}

impl From<u32> for TextOffset {
    #[inline]
    fn from(value: u32) -> TextOffset {
        TextOffset(value)
    }
}

impl From<TextOffset> for u32 {
    #[inline]
    fn from(value: TextOffset) -> u32 {
        value.0
    }
}

impl From<TextOffset> for usize {
    #[inline]
    fn from(value: TextOffset) -> usize {
        value.0 as usize
    }
}

impl std::ops::Add for TextOffset {
    type Output = TextOffset;
    #[inline]
    fn add(self, rhs: TextOffset) -> TextOffset {
        (self.0 + rhs.0).into()
    }
}

impl std::ops::AddAssign for TextOffset {
    #[inline]
    fn add_assign(&mut self, rhs: TextOffset) {
        self.0 = self.0 + rhs.0;
    }
}

impl fmt::Debug for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start.0, self.end.0)
    }
}

impl fmt::Debug for TextOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for TextLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

pub fn find_line_ranges(text: &str) -> Vec<TextRange> {
    let mut ranges = Vec::new();
    let mut range = TextRange::empty_at(0.into());
    for c in text.chars() {
        let size: TextOffset = (c.len_utf8() as u32).into();
        range.extend_by(size);
        if c == '\n' {
            ranges.push(range);
            range = TextRange::empty_at(range.end());
        }
    }
    if !range.is_empty() {
        ranges.push(range);
    }
    ranges
}

pub fn find_text_location(
    text: &str,
    offset: TextOffset,
    line_ranges: &[TextRange],
) -> (TextLocation, TextRange) {
    for (line, range) in line_ranges.iter().enumerate() {
        let contains = if line + 1 == line_ranges.len() {
            range.contains_inclusive(offset)
        } else {
            range.contains_exclusive(offset)
        };

        if contains {
            let prefix_range = TextRange::new(range.start(), offset);
            let prefix = &text[prefix_range.as_usize()];
            return (
                TextLocation::new(line as u32 + 1, prefix.chars().count() as u32 + 1),
                *range,
            );
        }
    }
    //@can fail (most often in current lsp that runs full check pass, and can de-sync), handle gracefully
    panic!(
        "text location not found, offset: {:?} text len: {}",
        offset,
        text.len()
    );
}

#[test]
fn test() {
    use crate::lexer;
    use crate::session::FileID;

    let text = "foo\nbaz";
    let tokens = if let Ok(tokens) = lexer::lex(text, FileID::new(0), false) {
        tokens
    } else {
        panic!("lex failed");
    };

    let line_ranges = find_line_ranges(text);
    let foo_range = TextRange::new(0.into(), 3.into());
    let baz_range = TextRange::new(4.into(), 7.into());
    let (foo_loc_start, _) = find_text_location(text, foo_range.start(), &line_ranges);
    let (foo_loc_end, _) = find_text_location(text, foo_range.end(), &line_ranges);
    let (baz_loc_start, _) = find_text_location(text, baz_range.start(), &line_ranges);
    let (baz_loc_end, _) = find_text_location(text, baz_range.end(), &line_ranges);

    assert_eq!(line_ranges[0], TextRange::new(0.into(), 4.into()));
    assert_eq!(line_ranges[1], TextRange::new(4.into(), 7.into()));
    assert_eq!(tokens.get_range(0), foo_range);
    assert_eq!(tokens.get_range(1), baz_range);
    assert_eq!(foo_loc_start, TextLocation::new(1, 1));
    assert_eq!(foo_loc_end, TextLocation::new(1, 4));
    assert_eq!(baz_loc_start, TextLocation::new(2, 1));
    assert_eq!(baz_loc_end, TextLocation::new(2, 4));
}

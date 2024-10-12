use std::fmt;

/// `TextRange` byte range in text  
/// Invariant: `start <= end`
#[derive(Copy, Clone, PartialEq)]
pub struct TextRange {
    start: TextOffset,
    end: TextOffset,
}

/// `TextOffset` byte offset in text
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextOffset(u32);

/// `TextLocation`  
/// `line` 1 based line number  
/// `col`  1 based utf8 char number
#[derive(Copy, Clone, PartialEq)]
pub struct TextLocation {
    line: u32,
    col: u32,
}

impl TextRange {
    #[inline]
    pub const fn new(start: TextOffset, end: TextOffset) -> TextRange {
        assert!(start.0 <= end.0);
        TextRange { start, end }
    }
    #[inline]
    pub const fn zero() -> TextRange {
        TextRange {
            start: TextOffset(0),
            end: TextOffset(0),
        }
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
    pub const fn len(self) -> u32 {
        self.end.0 - self.start.0
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
    const fn new(line: u32, col: u32) -> TextLocation {
        TextLocation { line, col }
    }
    #[inline]
    pub const fn line(&self) -> u32 {
        self.line
    }
    #[inline]
    pub const fn line_index(&self) -> usize {
        (self.line - 1) as usize
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

impl std::ops::Sub for TextOffset {
    type Output = TextOffset;
    #[inline]
    fn sub(self, rhs: TextOffset) -> TextOffset {
        (self.0 - rhs.0).into()
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

//@opt for re-using existing line_ranges Vec in Vfs
//@opt lines().count() for exact capacity?
pub fn find_line_ranges(text: &str) -> Vec<TextRange> {
    let mut ranges = Vec::new();
    let mut range = TextRange::zero();

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
) -> TextLocation {
    let mut size = line_ranges.len();
    let mut left = 0_usize;
    let mut right = size;

    while left < right {
        let mid = left + size / 2;
        let range = line_ranges[mid];

        let contains = if mid + 1 == line_ranges.len() {
            range.contains_inclusive(offset)
        } else {
            range.contains_exclusive(offset)
        };

        if contains {
            let prefix_range = TextRange::new(range.start(), offset);
            let prefix = &text[prefix_range.as_usize()];
            let location = TextLocation::new(mid as u32 + 1, prefix.chars().count() as u32 + 1);
            return location;
        } else if offset < range.start() {
            right = mid;
        } else {
            left = mid + 1;
        }

        size = right - left;
    }

    let error = format!(
        "internal: failed text::find_text_location()\noffset: {:?}\ntext:\n{}",
        offset, text
    );
    panic!("{error}");
}

#[test]
fn test() {
    use crate::intern::InternPool;
    use crate::lexer;
    use crate::session::ModuleID;
    use crate::support::ID;

    let text = "foo\nbaz";
    let mut intern_lit = InternPool::new(0);
    let (tokens, _) = lexer::lex(text, &mut intern_lit, ModuleID::dummy(), false);

    let line_ranges = find_line_ranges(text);
    let foo_range = TextRange::new(0.into(), 3.into());
    let baz_range = TextRange::new(4.into(), 7.into());
    let foo_loc_start = find_text_location(text, foo_range.start(), &line_ranges);
    let foo_loc_end = find_text_location(text, foo_range.end(), &line_ranges);
    let baz_loc_start = find_text_location(text, baz_range.start(), &line_ranges);
    let baz_loc_end = find_text_location(text, baz_range.end(), &line_ranges);

    assert_eq!(line_ranges[0], TextRange::new(0.into(), 4.into()));
    assert_eq!(line_ranges[1], TextRange::new(4.into(), 7.into()));
    assert_eq!(tokens.token_range(ID::new_raw(0)), foo_range);
    assert_eq!(tokens.token_range(ID::new_raw(1)), baz_range);
    assert_eq!(foo_loc_start, TextLocation::new(1, 1));
    assert_eq!(foo_loc_end, TextLocation::new(1, 4));
    assert_eq!(baz_loc_start, TextLocation::new(2, 1));
    assert_eq!(baz_loc_end, TextLocation::new(2, 4));
}

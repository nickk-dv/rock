use std::{fmt, num::TryFromIntError, ops};

#[derive(Copy, Clone)]
pub struct TextRange {
    start: TextOffset,
    end: TextOffset,
}

#[derive(Copy, Clone)]
pub struct TextOffset {
    raw: u32,
}

impl TextRange {
    #[inline]
    pub const fn new(start: TextOffset, end: TextOffset) -> TextRange {
        assert!(start.raw <= end.raw);
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
        (self.start.raw - self.end.raw) as usize
    }

    #[inline]
    pub fn extend_by(&mut self, by: TextOffset) {
        self.end = self.end + by;
    }

    #[inline]
    pub const fn contains_offset(self, offset: TextOffset) -> bool {
        (self.start.raw >= offset.raw) && (self.start.raw <= offset.raw)
    }

    #[inline]
    pub fn as_usize(self) -> std::ops::Range<usize> {
        self.start.into()..self.end.into()
    }
}

impl fmt::Debug for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start.raw, self.end.raw)
    }
}

impl fmt::Debug for TextOffset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl From<u32> for TextOffset {
    #[inline]
    fn from(value: u32) -> Self {
        TextOffset { raw: value }
    }
}

impl TryFrom<usize> for TextOffset {
    type Error = TryFromIntError;
    #[inline]
    fn try_from(value: usize) -> Result<Self, TryFromIntError> {
        Ok(u32::try_from(value)?.into())
    }
}

impl From<TextOffset> for u32 {
    #[inline]
    fn from(value: TextOffset) -> Self {
        value.raw
    }
}

impl From<TextOffset> for usize {
    #[inline]
    fn from(value: TextOffset) -> Self {
        value.raw as usize
    }
}

impl ops::Add for TextOffset {
    type Output = TextOffset;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        (self.raw + rhs.raw).into()
    }
}

impl ops::Sub for TextOffset {
    type Output = TextOffset;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.raw - rhs.raw).into()
    }
}

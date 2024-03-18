use super::{TextLocation, TextOffset, TextRange};

pub fn line_ranges(text: &str) -> Vec<TextRange> {
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
    if range.len() > 0 {
        ranges.push(range);
    }
    ranges
}

pub fn text_location(
    text: &str,
    offset: TextOffset,
    line_ranges: &[TextRange],
) -> (TextLocation, TextRange) {
    for (line, range) in line_ranges.iter().enumerate() {
        if range.contains_offset(offset) {
            let prefix_range = TextRange::new(range.start(), offset);
            let prefix = &text[prefix_range.as_usize()];
            return (
                TextLocation::new(line as u32 + 1, prefix.chars().count() as u32 + 1),
                *range,
            );
        }
    }
    panic!("text location not found");
}

use lsp_types as lsp;
use rock_core::session::FileData;
use rock_core::text::{self, TextOffset, TextRange};

pub fn string_len_utf16(string: &str) -> u32 {
    string.chars().map(|c| c.len_utf16() as u32).sum()
}

pub fn range_utf16_to_range_utf8(file: &FileData, range: lsp::Range) -> TextRange {
    let start = position_utf16_to_offset_utf8(file, range.start);
    let end = position_utf16_to_offset_utf8(file, range.end);
    TextRange::new(start, end)
}

pub fn range_utf8_to_range_utf16(file: &FileData, range: TextRange) -> lsp::Range {
    let start = offset_utf8_to_position_utf16(file, range.start());
    let end = offset_utf8_to_position_utf16(file, range.end());
    lsp::Range::new(start, end)
}

pub fn position_utf16_to_offset_utf8(file: &FileData, position: lsp::Position) -> TextOffset {
    let line_range = line_range(file, position.line);
    let line_text = &file.source[line_range.as_usize()];

    let mut chars = line_text.chars();
    let mut offset = line_range.start();
    let mut char_utf16: u32 = 0;

    while char_utf16 != position.character {
        if let Some(c) = chars.next() {
            offset += (c.len_utf8() as u32).into();
            char_utf16 += c.len_utf16() as u32;
        } else {
            crate::server_error!(
                "position_utf16_to_offset_utf8() failed\nfile: {}\nline: {}\ncharacter: {}",
                file.path.to_string_lossy(),
                position.line,
                position.character,
            )
        }
    }
    offset
}

pub fn offset_utf8_to_position_utf16(file: &FileData, offset: TextOffset) -> lsp::Position {
    let pos = text::find_text_location(&file.source, offset, &file.line_ranges);
    let line_range = file.line_ranges[pos.line_index() - 1];
    let prefix_range = TextRange::new(line_range.start(), offset);
    let prefix_text = &file.source[prefix_range.as_usize()];
    lsp::Position::new(pos.line() - 1, string_len_utf16(prefix_text))
}

fn line_range(file: &FileData, line: u32) -> TextRange {
    if file.line_ranges.is_empty() {
        TextRange::zero()
    } else if line < file.line_ranges.len() as u32 {
        file.line_ranges[line as usize]
    } else if line == file.line_ranges.len() as u32 {
        let last = file.line_ranges.last().copied().unwrap();
        TextRange::new(last.end(), last.end())
    } else {
        crate::server_error!(
            "line_range() failed\nfile: {}\nline: {line}\nline_ranges.len: {}",
            file.path.to_string_lossy(),
            file.line_ranges.len()
        );
    }
}

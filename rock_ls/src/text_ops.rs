use lsp_types as lsp;
use rock_core::session::FileData;
use rock_core::text::{TextOffset, TextRange};

pub fn file_range_to_text_range(file: &FileData, range: lsp::Range) -> TextRange {
    let start_line = file_line_range(file, range.start.line);
    let end_line = file_line_range(file, range.end.line);
    let start = file_line_char_utf16_to_offset(file, start_line, range.start.character);
    let end = file_line_char_utf16_to_offset(file, end_line, range.end.character);
    TextRange::new(start, end)
}

fn file_line_range(file: &FileData, line: u32) -> TextRange {
    if file.line_ranges.len() == 0 {
        TextRange::zero()
    } else if line < file.line_ranges.len() as u32 {
        file.line_ranges[line as usize]
    } else if line == file.line_ranges.len() as u32 {
        let last = file.line_ranges.last().copied().unwrap();
        TextRange::new(last.end(), last.end())
    } else {
        unreachable!(
            "internal: `file_line_range()` failed\nfile: {}\nline: {line}\nline_ranges.len: {}",
            file.path.to_string_lossy(),
            file.line_ranges.len(),
        );
    }
}

fn file_line_char_utf16_to_offset(
    file: &FileData,
    line_range: TextRange,
    character: u32,
) -> TextOffset {
    let line_text = &file.source[line_range.as_usize()];
    let mut chars = line_text.chars();
    let mut offset = line_range.start();
    let mut char_utf16: u32 = 0;

    while char_utf16 < character {
        if let Some(c) = chars.next() {
            offset += (c.len_utf8() as u32).into();
            char_utf16 += c.len_utf16() as u32;
        } else {
            unreachable!(
                "internal: `file_line_char_utf16_to_offset()` failed\nfile: {}\nline_range: {:?}\ncharacter: {}",
                file.path.to_string_lossy(),
                line_range,
                character,
            )
        }
    }
    offset
}

use super::ansi::{self, Color};
use crate::ast::File;
use crate::text_range::TextRange;
use std::io::{BufWriter, Stderr, Write};

// @temp old code
// @the whole range formatting output system needs to be redisigned
// utf8 works when generating the Loc's from TextRanges
// but its not fully tested and output markers can be miss-aligned

// @is fix possible?
// loc col is correct utf8 character index
// and it matches vscode status bar `Col`
// but goto from console jump to byte instead

pub fn print(
    handle: &mut BufWriter<Stderr>,
    file: &File,
    range: TextRange,
    marker: Option<&str>,
    is_info: bool,
) {
    let format = TextRangeFormat::new(file, range);
    format.print(handle, marker, is_info);
}

pub fn print_simple(file: &File, range: TextRange, marker: Option<&str>, is_info: bool) {
    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    let format = TextRangeFormat::new(file, range);
    format.print(handle, marker, is_info);
    let _ = handle.flush();
}

struct TextRangeFormat<'a> {
    loc: Loc,
    file: &'a File,
    line: String,
    marker_len: usize,
    marker_pad_len: usize,
}

impl<'a> TextRangeFormat<'a> {
    fn new(file: &'a File, range: TextRange) -> Self {
        let mut lex = crate::ast::lexer::Lexer::new(&file.source);
        let line_ranges = lex.lex_line_ranges(); //@temp getting all line ranges, since they are no longer stored in ast
        let loc = find_loc(&line_ranges, range, &file.source);

        let prefix_range = TextRange::new(loc.range.start(), range.start());
        let marker_pad = &file.source[prefix_range.as_usize()];

        let marker_str = &file.source[range.as_usize()];
        let marker_str = if marker_str.contains('\n') {
            marker_str.lines().next().unwrap_or("").trim_end()
        } else {
            marker_str.trim_end()
        };

        Self {
            loc,
            file,
            line: normalize_tab(&file.source[loc.range.as_usize()]),
            marker_len: normalize_tab_len(marker_str),
            marker_pad_len: normalize_tab_len(marker_pad),
        }
    }

    fn print(&self, handle: &mut BufWriter<Stderr>, marker_msg: Option<&str>, is_info: bool) {
        let line_num = self.loc.line.to_string();
        let left_pad = marker_space_slice(line_num.len());
        let marker_pad = marker_space_slice(self.marker_pad_len);
        let marker = if is_info {
            marker_info_slice(self.marker_len)
        } else {
            marker_error_slice(self.marker_len)
        };

        ansi::set_color(handle, Color::Cyan);
        let _ = write!(handle, "{}--> ", left_pad);
        let _ = writeln!(
            handle,
            "{}:{}:{}",
            self.file.path.to_string_lossy(),
            self.loc.line,
            self.loc.col
        );

        let _ = handle.write(left_pad.as_bytes());
        let _ = handle.write(" |\n".as_bytes());

        let _ = handle.write(line_num.as_bytes());
        let _ = handle.write(" | ".as_bytes());
        ansi::reset(handle);
        let _ = handle.write(self.line.as_bytes());
        let _ = handle.write("\n".as_bytes());

        ansi::set_color(handle, Color::Cyan);
        let _ = handle.write(left_pad.as_bytes());
        let _ = handle.write(" | ".as_bytes());
        ansi::reset(handle);

        if is_info {
            ansi::set_color(handle, Color::BoldGreen)
        } else {
            ansi::set_color(handle, Color::BoldRed)
        }
        let _ = handle.write(marker_pad.as_bytes());
        let _ = handle.write(marker.as_bytes());
        let _ = handle.write(" ".as_bytes());

        if let Some(msg) = marker_msg {
            if is_info {
                ansi::set_color(handle, Color::Green)
            } else {
                ansi::set_color(handle, Color::Red)
            }
            let _ = handle.write(msg.as_bytes());
        }
        let _ = handle.write("\n".as_bytes());
        ansi::reset(handle);
    }
}

#[derive(Copy, Clone)]
struct Loc {
    line: u32,
    col: u32,
    range: TextRange,
}

fn find_loc(line_ranges: &[TextRange], range: TextRange, text: &str) -> Loc {
    let mut loc = Loc {
        line: 0,
        col: 1,
        range,
    };
    for line_range in line_ranges {
        loc.line += 1;
        if line_range.contains_offset(range.start()) {
            let prefix_range = TextRange::new(line_range.start(), range.start());
            let prefix = &text[prefix_range.as_usize()];
            let utf8_chars = prefix.chars().count();

            loc.col = 1 + utf8_chars as u32;
            loc.range = *line_range;
            return loc;
        }
    }
    Loc {
        line: 0,
        col: 1,
        range: TextRange::empty_at(0.into()),
    }
}

fn marker_space_slice(size: usize) -> &'static str {
    const MARKER_SPACE: &'static str = "                                                                                                                                ";
    let size = size.clamp(0, MARKER_SPACE.len());
    unsafe { MARKER_SPACE.get_unchecked(0..size) }
}

fn marker_error_slice(size: usize) -> &'static str {
    const MARKER_ERROR: &'static str = "^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^";
    let size = size.clamp(0, MARKER_ERROR.len());
    unsafe { MARKER_ERROR.get_unchecked(0..size) }
}

fn marker_info_slice(size: usize) -> &'static str {
    const MARKER_INFO: &'static str =  "--------------------------------------------------------------------------------------------------------------------------------";
    let size = size.clamp(0, MARKER_INFO.len());
    unsafe { MARKER_INFO.get_unchecked(0..size) }
}

fn normalize_tab(str: &str) -> String {
    const TAB: &'static str = "  ";
    str.replace('\t', TAB)
}

fn normalize_tab_len(str: &str) -> usize {
    let mut len = 0;
    for c in str.chars() {
        len += if c == '\t' { 2 } else { 1 }
    }
    len
}

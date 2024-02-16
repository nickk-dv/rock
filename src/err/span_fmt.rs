use super::ansi::{self, Color};
use crate::ast::parser::File;
use crate::ast::span::*;
use std::io::{BufWriter, Stderr, Write};

pub fn print(
    handle: &mut BufWriter<Stderr>,
    file: &File,
    span: Span,
    marker: Option<&str>,
    is_info: bool,
) {
    let format = SpanFormat::new(file, span);
    format.print(handle, marker, is_info);
}

pub fn print_simple(file: &File, span: Span, marker: Option<&str>, is_info: bool) {
    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    let format = SpanFormat::new(file, span);
    format.print(handle, marker, is_info);
    let _ = handle.flush();
}

struct SpanFormat<'a> {
    loc: Loc,
    file: &'a File,
    line: String,
    marker_len: usize,
    marker_pad_len: usize,
}

impl<'a> SpanFormat<'a> {
    fn new(file: &'a File, span: Span) -> Self {
        let mut lex = crate::ast::lexer::Lexer::new(&file.source);
        let line_spans = lex.lex_line_spans(); //@temp getting all line spans, since they are no longer stored in ast

        let loc = find_loc(&line_spans, span);
        let prefix = Span::new(loc.span.start, span.start);
        let marker_pad = prefix.slice(&file.source);

        let marker_str = span.slice(&file.source);
        let marker_str = if marker_str.contains('\n') {
            marker_str.lines().next().unwrap_or("").trim_end()
        } else {
            marker_str.trim_end()
        };

        Self {
            loc,
            file,
            line: normalize_tab(loc.span.slice(&file.source)),
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
    span: Span,
}

fn find_loc(line_spans: &Vec<Span>, span: Span) -> Loc {
    let mut loc = Loc {
        line: 0,
        col: 1,
        span,
    };
    for line_span in line_spans.iter() {
        loc.line += 1;
        if span.start >= line_span.start && span.start <= line_span.end {
            loc.col = 1 + span.start - line_span.start;
            loc.span = *line_span;
            return loc;
        }
    }
    return loc;
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

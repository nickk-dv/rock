use super::ansi::{self, Color};
use crate::ast::ast::SourceFile;
use crate::ast::span::*;
use std::io::{BufWriter, Stderr, Write};

pub fn print(
    handle: &mut BufWriter<Stderr>,
    source: &SourceFile,
    span: Span,
    marker: Option<&str>,
    is_info: bool,
) {
    let format = SpanFormat::new(source, span);
    format.print(handle, marker, is_info);
}

#[derive(Copy, Clone)]
struct Loc {
    line: u32,
    col: u32,
    span: Span,
}

struct SpanFormat<'a> {
    file: &'a SourceFile,
    loc: Loc,
    line: String,
    line_num: String,
    left_pad: String,
    is_multi_line: bool,
    marker_len: usize,
    marker_pad_len: usize,
}

impl<'a> SpanFormat<'a> {
    fn new(file: &'a SourceFile, span: Span) -> Self {
        let loc = find_loc(&file.line_spans, span);
        let is_multi_line;

        let line = loc.span.slice(&file.source);
        let line = normalize_tab(line);

        let prefix_span = Span::new(loc.span.start, span.start);
        let line_prefix = prefix_span.slice(&file.source);
        let line_prefix = normalize_tab(line_prefix);

        let line_span = span.slice(&file.source);
        let line_span = if line_span.contains('\n') {
            is_multi_line = true;
            line_span.lines().next().unwrap_or("").trim_end()
        } else {
            is_multi_line = false;
            line_span.trim_end()
        };
        let line_span = normalize_tab(line_span);

        Self {
            file,
            loc,
            line,
            line_num: loc.line.to_string(),
            left_pad: replace_all(loc.line.to_string().as_str(), ' '),
            is_multi_line,
            marker_len: line_span.len(),
            marker_pad_len: line_prefix.len(),
        }
    }

    fn print(&self, handle: &mut BufWriter<Stderr>, marker_msg: Option<&str>, is_info: bool) {
        self.print_arrow(handle);
        ansi::set_color(handle, Color::Cyan);
        let _ = writeln!(
            handle,
            "{}:{}:{}",
            self.file.path.to_string_lossy(),
            self.loc.line,
            self.loc.col
        );
        ansi::reset(handle);

        self.print_bar(handle, true);
        self.print_line_bar(handle);
        let _ = writeln!(handle, "{}", self.line);
        self.print_bar(handle, false);

        let marker_pad = " ".repeat(self.marker_pad_len);
        if is_info {
            ansi::set_color(handle, Color::BoldGreen);
            let marker = "-".repeat(self.marker_len);
            match marker_msg {
                Some(message) => {
                    let _ = write!(handle, "{}{}", marker_pad, marker);
                    ansi::set_color(handle, Color::Green);
                    let _ = writeln!(handle, " {}", message);
                }
                None => {
                    let _ = writeln!(handle, "{}{}", marker_pad, marker);
                }
            }
        } else {
            ansi::set_color(handle, Color::BoldRed);
            let marker = "^".repeat(self.marker_len);
            match marker_msg {
                Some(message) => {
                    let _ = writeln!(handle, "{}{} {}", marker_pad, marker, message);
                }
                None => {
                    let _ = writeln!(handle, "{}{}", marker_pad, marker);
                }
            }
        }
        ansi::reset(handle);

        if self.is_multi_line {
            self.print_bar(handle, false);
            ansi::set_color(handle, Color::Cyan);
            let _ = writeln!(handle, "...");
            ansi::reset(handle);
        }
    }

    fn print_arrow(&self, handle: &mut BufWriter<Stderr>) {
        ansi::set_color(handle, Color::Cyan);
        let _ = write!(handle, "{}--> ", self.left_pad);
        ansi::reset(handle);
    }

    fn print_bar(&self, handle: &mut BufWriter<Stderr>, endl: bool) {
        ansi::set_color(handle, Color::Cyan);
        if endl {
            let _ = writeln!(handle, "{} |", self.left_pad);
        } else {
            let _ = write!(handle, "{} | ", self.left_pad);
        }
        ansi::reset(handle);
    }

    fn print_line_bar(&self, handle: &mut BufWriter<Stderr>) {
        ansi::set_color(handle, Color::Cyan);
        let _ = write!(handle, "{} | ", self.line_num);
        ansi::reset(handle);
    }
}

fn replace_all(str: &str, c: char) -> String {
    std::iter::repeat(c).take(str.chars().count()).collect()
}

fn normalize_tab(str: &str) -> String {
    const TAB: &'static str = "  ";
    str.replace('\t', TAB)
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
    //@err internal?
    return loc;
}

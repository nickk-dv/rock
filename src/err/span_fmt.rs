use super::ansi::{self, Color};
use crate::ast::ast::SourceFile;
use crate::ast::span::*;

pub fn print(source: &SourceFile, span: Span, marker: Option<&str>, is_info: bool) {
    let format = SpanFormat::new(source, span);
    format.print(marker, is_info);
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

        let line = loc.span.str(&file.source);
        let line = normalize_tab(line);
        let line_prefix = Span::str_range(loc.span.start, span.start, &file.source);
        let line_prefix = normalize_tab(line_prefix);
        let line_span = span.str(&file.source);
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

    fn print(&self, marker_msg: Option<&str>, is_info: bool) {
        self.print_arrow();
        ansi::set_color(Color::Cyan);
        println!(
            "{}:{}:{}",
            self.file.path.to_string_lossy(),
            self.loc.line,
            self.loc.col
        );
        ansi::reset();

        self.print_bar(true);
        self.print_line_bar();
        println!("{}", self.line);
        self.print_bar(false);

        let marker_pad = " ".repeat(self.marker_pad_len);
        if is_info {
            ansi::set_color(Color::BoldGreen);
            let marker = "-".repeat(self.marker_len);
            match marker_msg {
                Some(message) => {
                    print!("{}{}", marker_pad, marker);
                    ansi::set_color(Color::Green);
                    println!(" {}", message);
                }
                None => println!("{}{}", marker_pad, marker),
            }
        } else {
            ansi::set_color(Color::BoldRed);
            let marker = "^".repeat(self.marker_len);
            match marker_msg {
                Some(message) => println!("{}{} {}", marker_pad, marker, message),
                None => println!("{}{}", marker_pad, marker),
            }
        }
        ansi::reset();

        if self.is_multi_line {
            self.print_bar(false);
            ansi::set_color(Color::Cyan);
            println!("...");
            ansi::reset();
        }
    }

    fn print_arrow(&self) {
        ansi::set_color(Color::Cyan);
        print!("{}--> ", self.left_pad);
        ansi::reset();
    }

    fn print_bar(&self, endl: bool) {
        ansi::set_color(Color::Cyan);
        if endl {
            println!("{} |", self.left_pad);
        } else {
            print!("{} | ", self.left_pad);
        }
        ansi::reset();
    }

    fn print_line_bar(&self) {
        ansi::set_color(Color::Cyan);
        print!("{} | ", self.line_num);
        ansi::reset();
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

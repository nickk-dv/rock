use crate::ast::span::Span;
use crate::ast::{Package, SourceFile, SourceID};

pub fn err(package: &Package, id: SourceID, span: Span) {
    let file_option = package.files.get(id as usize);
    if file_option.is_none() {
        set_color(Color::Red);
        println!("Internal error file id is invalid");
        set_color(Color::Reset);
        return;
    }
    let source = file_option.unwrap();
    let loc = find_loc(source, span);
    let line = &source.file[loc.line_span.start as usize..loc.line_span.end as usize];
    let pre_line = &source.file[loc.line_span.start as usize..span.start as usize];
    let span_line = &source.file[span.start as usize..span.end as usize]; //@clamp to line end

    println!();
    set_color(Color::Red);
    println!("parse error:");
    set_color(Color::Reset);

    let line_str = loc.line.to_string();
    let offset_str: String = std::iter::repeat(' ').take(line_str.len()).collect();
    set_color(Color::Cyan);
    print!("{}--> ", offset_str);
    set_color(Color::Reset);

    println!("{}:{}:{}", source.path.to_string_lossy(), loc.line, loc.col);

    set_color(Color::Cyan);
    println!("{} |", offset_str);
    print!("{} | ", line_str);
    set_color(Color::Reset);

    println!("{}", line);

    let mark_offset_str: String = pre_line
        .chars()
        .map(|c| if c != '\t' { ' ' } else { c })
        .collect();
    let mark_len_str: String = std::iter::repeat('^')
        .take(span_line.chars().count())
        .collect();
    set_color(Color::Cyan);
    print!("{} | ", offset_str);
    set_color(Color::Reset);
    set_color(Color::Red);
    println!("{}{} {}", mark_offset_str, mark_len_str, "unexpected token");
    set_color(Color::Reset);
}

struct Loc {
    line: u32,
    col: u32,
    line_span: Span,
}

fn find_loc(source: &SourceFile, span: Span) -> Loc {
    let mut loc = Loc {
        line: 0,
        col: 1,
        line_span: span,
    };
    for line_span in source.line_spans.iter() {
        loc.line += 1;
        if span.start >= line_span.start && span.start <= line_span.end {
            loc.col = 1 + span.start - line_span.start;
            loc.line_span = *line_span;
            return loc;
        }
    }
    //@err internal
    return loc;
}

fn set_color(color: Color) {
    print!("{}", Color::as_ansi_str(color));
}

enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    Reset,
}

impl Color {
    fn as_ansi_str(color: Color) -> &'static str {
        match color {
            Color::Black => "\x1B[0;30m",
            Color::Red => "\x1B[0;31m",
            Color::Green => "\x1B[0;32m",
            Color::Yellow => "\x1B[0;33m",
            Color::Blue => "\x1B[0;34m",
            Color::Purple => "\x1B[0;35m",
            Color::Cyan => "\x1B[0;36m",
            Color::White => "\x1B[0;37m",
            Color::Reset => "\x1B[0m",
        }
    }
}

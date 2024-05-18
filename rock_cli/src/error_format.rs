use crate::ansi;
use rock_core::error::{
    Diagnostic, DiagnosticCollection, DiagnosticContext, DiagnosticKind, DiagnosticSeverity,
};
use rock_core::session::{File, Session};
use rock_core::text::{self, TextLocation, TextRange};
use std::io::{BufWriter, Stderr, Write};
use std::path::Path;

pub fn print_errors(session: Option<&Session>, diagnostics: DiagnosticCollection) {
    let handle = &mut BufWriter::new(std::io::stderr());
    for warning in diagnostics.warnings() {
        print_diagnostic(
            session,
            warning.diagnostic(),
            DiagnosticSeverity::Warning,
            handle,
        );
    }
    for error in diagnostics.errors() {
        print_diagnostic(
            session,
            error.diagnostic(),
            DiagnosticSeverity::Error,
            handle,
        );
    }
    let _ = handle.flush();
}

struct ContextFmt<'src> {
    file: &'src File,
    path: &'src Path,
    range: TextRange,
    location: TextLocation,
    line_range: TextRange,
    line_num: String,
}

impl<'src> ContextFmt<'src> {
    fn new(session: &'src Session, context: &DiagnosticContext) -> ContextFmt<'src> {
        let file = session.file(context.source().file_id());
        let path = file
            .path
            .strip_prefix(session.cwd())
            .unwrap_or_else(|_| &file.path);

        let range = context.source().range();
        let location = text::find_text_location(&file.source, range.start(), &file.line_ranges);
        let line_range = file.line_ranges[location.line_index()];
        let line_num = location.line().to_string();

        ContextFmt {
            file,
            path,
            range,
            location,
            line_range,
            line_num,
        }
    }

    fn extend_line_num(&mut self, other: &ContextFmt) {
        if self.line_num.len() < other.line_num.len() {
            let pad_len = other.line_num.len() - self.line_num.len();
            self.line_num.extend(std::iter::repeat(' ').take(pad_len));
        }
    }
}

fn print_diagnostic(
    session: Option<&Session>,
    diagnostic: &Diagnostic,
    severity: DiagnosticSeverity,
    handle: &mut BufWriter<Stderr>,
) {
    let message = diagnostic.message().as_str();
    let _ = writeln!(
        handle,
        "\n{}{}: {}{message}{}",
        severity_color(severity),
        severity_name(severity),
        ansi::WHITE_BOLD,
        ansi::RESET
    );

    let (main, info) = match diagnostic.kind() {
        DiagnosticKind::Message => return,
        DiagnosticKind::Context { main, info } => (main, info),
        DiagnosticKind::ContextVec { main, info } => panic!("diagnostic info vec not supported"),
    };

    let session = session.expect("session context");
    let mut main_fmt = ContextFmt::new(session, main);

    if let Some(info) = info {
        let mut info_fmt = ContextFmt::new(session, info);

        main_fmt.extend_line_num(&info_fmt);
        info_fmt.extend_line_num(&main_fmt);
        let line_pad = " ".repeat(main_fmt.line_num.len());

        print_line_bar(handle, &line_pad);
        print_context(handle, &line_pad, &main_fmt, main, severity);
        print_file_link(handle, &line_pad, &main_fmt, false);
        print_context(handle, &line_pad, &info_fmt, info, DiagnosticSeverity::Info);
        print_file_link(handle, &line_pad, &info_fmt, true);
    } else {
        let line_pad = " ".repeat(main_fmt.line_num.len());

        print_line_bar(handle, &line_pad);
        print_context(handle, &line_pad, &main_fmt, main, severity);
        print_file_link(handle, &line_pad, &main_fmt, true);
    }
}

fn print_line_bar(handle: &mut BufWriter<Stderr>, line_pad: &str) {
    let _ = writeln!(handle, "{line_pad} {}│{}", ansi::CYAN, ansi::RESET);
}

fn print_file_link(handle: &mut BufWriter<Stderr>, line_pad: &str, fmt: &ContextFmt, last: bool) {
    let box_char = if last { '└' } else { '├' };
    let _ = writeln!(
        handle,
        "{}{line_pad} {box_char}─ {}:{:?}{}",
        ansi::CYAN,
        fmt.path.to_string_lossy(),
        fmt.location,
        ansi::RESET
    );
    if !last {
        print_line_bar(handle, line_pad);
    }
}

const TAB_SPACE_COUNT: usize = 2;
const TAB_REPLACE_STR: &str = "  ";

fn print_context(
    handle: &mut BufWriter<Stderr>,
    line_pad: &str,
    fmt: &ContextFmt,
    context: &DiagnosticContext,
    severity: DiagnosticSeverity,
) {
    let prefix_range = TextRange::new(fmt.line_range.start(), fmt.range.start());
    let source_range = TextRange::new(fmt.range.start(), fmt.line_range.end().min(fmt.range.end()));

    let line_str = &fmt.file.source[fmt.line_range.as_usize()];
    let prefix_str = &fmt.file.source[prefix_range.as_usize()];
    let source_str = &fmt.file.source[source_range.as_usize()];

    let line = line_str.trim_end().replace('\t', TAB_REPLACE_STR);
    let marker_pad = " ".repeat(normalized_tab_len(prefix_str));
    let marker = severity_marker(severity).repeat(normalized_tab_len(source_str));
    let message = context.message();

    let _ = writeln!(
        handle,
        r#"{}{} │ {}{line}{}
{line_pad} │ {marker_pad}{}{marker} {message}{}"#,
        ansi::CYAN,
        fmt.line_num,
        ansi::RESET,
        ansi::CYAN,
        severity_color(severity),
        ansi::RESET,
    );
}

fn normalized_tab_len(text: &str) -> usize {
    text.chars()
        .map(|c| if c == '\t' { TAB_SPACE_COUNT } else { 1 })
        .sum::<usize>()
}

const fn severity_name(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
    }
}

const fn severity_marker(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => "-",
        DiagnosticSeverity::Error => "^",
        DiagnosticSeverity::Warning => "^",
    }
}

const fn severity_color(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => ansi::GREEN_BOLD,
        DiagnosticSeverity::Error => ansi::RED_BOLD,
        DiagnosticSeverity::Warning => ansi::YELLOW_BOLD,
    }
}

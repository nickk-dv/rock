use super::ansi;
use super::{ErrorComp, ErrorSeverity};
use crate::text;
use crate::text::TextRange;
use crate::vfs::Vfs;
use std::io::{BufWriter, Stderr, Write};

const TAB_SPACE_COUNT: usize = 2;
const TAB_REPLACE_STR: &'static str = "  ";

pub fn print_errors(vfs: &Vfs, errors: &[ErrorComp]) {
    let handle = &mut BufWriter::new(std::io::stderr());
    for error in errors {
        if error.severity == ErrorSeverity::Warning {
            print_error(vfs, error, handle)
        }
    }
    for error in errors {
        if error.severity == ErrorSeverity::Error {
            print_error(vfs, error, handle)
        }
    }
    let _ = handle.flush();
}

fn print_error(vfs: &Vfs, error: &ErrorComp, handle: &mut BufWriter<Stderr>) {
    let (message, severiry) = error.main_message();
    let _ = writeln!(
        handle,
        "\n{}{}: {}{message}{}",
        severity_color(severiry),
        severity_name(severiry),
        ansi::WHITE_BOLD,
        ansi::CLEAR
    );
    for context in error.context_iter() {
        let file = vfs.file(context.source().file_id());

        let range = context.source().range();
        let (location, line_range) =
            text::find::text_location(&file.source, range.start(), &file.line_ranges);
        let prefix_range = TextRange::new(line_range.start(), range.start());
        let source_range = TextRange::new(range.start(), line_range.end().min(range.end()));

        let line_str = &file.source[line_range.as_usize()];
        let prefix_str = &file.source[prefix_range.as_usize()];
        let source_str = &file.source[source_range.as_usize()];

        let line_num = location.line().to_string();
        let line_pad = " ".repeat(line_num.len());
        let line = line_str.trim_end().replace("\t", TAB_REPLACE_STR);
        let marker_pad = " ".repeat(normalized_tab_len(prefix_str));
        let marker = severity_marker(context.severity()).repeat(normalized_tab_len(source_str));
        let message = context.message();

        let _ = writeln!(
            handle,
            r#"{}{line_pad} ┌─ {}:{:?}
{line_pad} │
{line_num} │ {}{line}{}
{line_pad} │ {marker_pad}{}{marker} {message}{}"#,
            ansi::CYAN,
            file.path.to_string_lossy(),
            location,
            ansi::CLEAR,
            ansi::CYAN,
            severity_color(context.severity()),
            ansi::CLEAR,
        );
    }
}

fn normalized_tab_len(text: &str) -> usize {
    text.chars()
        .map(|c| if c == '\t' { TAB_SPACE_COUNT } else { 1 })
        .sum::<usize>()
}

const fn severity_name(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Error => "error",
        ErrorSeverity::Warning => "warning",
        ErrorSeverity::InfoHint => "info",
    }
}

const fn severity_color(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Error => ansi::RED_BOLD,
        ErrorSeverity::Warning => ansi::YELLOW_BOLD,
        ErrorSeverity::InfoHint => ansi::GREEN_BOLD,
    }
}

const fn severity_marker(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Error => "^",
        ErrorSeverity::Warning => "^",
        ErrorSeverity::InfoHint => "-",
    }
}

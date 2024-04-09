use crate::ansi;
use rock_core::error::{ErrorComp, ErrorContext, ErrorSeverity};
use rock_core::session::Session;
use rock_core::text::{self, TextRange};
use std::io::{BufWriter, Stderr, Write};

const TAB_SPACE_COUNT: usize = 2;
const TAB_REPLACE_STR: &str = "  ";

pub fn print_errors(session: Option<&Session>, errors: Vec<ErrorComp>) {
    let handle = &mut BufWriter::new(std::io::stderr());
    for error in errors.iter() {
        if error.get_severity() == ErrorSeverity::Warning {
            print_error(session, error, handle)
        }
    }
    for error in errors.iter() {
        if error.get_severity() == ErrorSeverity::Error {
            print_error(session, error, handle)
        }
    }
    let _ = handle.flush();
}

fn print_error(session: Option<&Session>, error: &ErrorComp, handle: &mut BufWriter<Stderr>) {
    let message = error.get_message();
    let severiry = error.get_severity();

    let _ = writeln!(
        handle,
        "\n{}{}: {}{message}{}",
        severity_color(severiry),
        severity_name(severiry),
        ansi::WHITE_BOLD,
        ansi::CLEAR
    );

    match error.get_data() {
        rock_core::error::ErrorData::None => return,
        rock_core::error::ErrorData::Context { main, info } => {
            let session = session.expect("error session");
            print_context(session, severiry, main, handle);
            if let Some(info) = info {
                print_context(session, ErrorSeverity::Info, info, handle);
            }
        }
    }
}

fn print_context(
    session: &Session,
    severity: ErrorSeverity,
    context: &ErrorContext,
    handle: &mut BufWriter<Stderr>,
) {
    let file = session.file(context.source().file_id());

    let range = context.source().range();
    let (location, line_range) =
        text::find_text_location(&file.source, range.start(), &file.line_ranges);
    let prefix_range = TextRange::new(line_range.start(), range.start());
    let source_range = TextRange::new(range.start(), line_range.end().min(range.end()));

    let line_str = &file.source[line_range.as_usize()];
    let prefix_str = &file.source[prefix_range.as_usize()];
    let source_str = &file.source[source_range.as_usize()];

    let line_num = location.line().to_string();
    let line_pad = " ".repeat(line_num.len());
    let line = line_str.trim_end().replace('\t', TAB_REPLACE_STR);
    let marker_pad = " ".repeat(normalized_tab_len(prefix_str));
    let marker = severity_marker(severity).repeat(normalized_tab_len(source_str));
    let message = context.message();

    let _ = writeln!(
        handle,
        r#"{}{line_pad} │
{line_num} │ {}{line}{}
{line_pad} │ {marker_pad}{}{marker} {message}{}
{line_pad} └─ {}:{:?}{}"#,
        ansi::CYAN,
        ansi::CLEAR,
        ansi::CYAN,
        severity_color(severity),
        ansi::CYAN,
        file.path.to_string_lossy(),
        location,
        ansi::CLEAR
    );
}

fn normalized_tab_len(text: &str) -> usize {
    text.chars()
        .map(|c| if c == '\t' { TAB_SPACE_COUNT } else { 1 })
        .sum::<usize>()
}

const fn severity_name(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Info => "info",
        ErrorSeverity::Error => "error",
        ErrorSeverity::Warning => "warning",
    }
}

const fn severity_color(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Info => ansi::GREEN_BOLD,
        ErrorSeverity::Error => ansi::RED_BOLD,
        ErrorSeverity::Warning => ansi::YELLOW_BOLD,
    }
}

const fn severity_marker(severity: ErrorSeverity) -> &'static str {
    match severity {
        ErrorSeverity::Info => "-",
        ErrorSeverity::Error => "^",
        ErrorSeverity::Warning => "^",
    }
}

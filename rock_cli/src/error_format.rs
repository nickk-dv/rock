use crate::ansi;
use rock_core::error::{
    Diagnostic, DiagnosticCollection, DiagnosticContext, DiagnosticKind, DiagnosticSeverity,
};
use rock_core::session::{File, Session};
use rock_core::text::{self, TextLocation, TextRange};
use std::io::{BufWriter, Stderr, Write};
use std::path::Path;

pub fn print_errors(session: Option<&Session>, diagnostics: DiagnosticCollection) {
    let mut handle = BufWriter::new(std::io::stderr());
    let mut state = StateFmt::new();

    for warning in diagnostics.warnings() {
        print_diagnostic(
            session,
            warning.diagnostic(),
            DiagnosticSeverity::Warning,
            &mut state,
            &mut handle,
        );
    }
    for error in diagnostics.errors() {
        print_diagnostic(
            session,
            error.diagnostic(),
            DiagnosticSeverity::Error,
            &mut state,
            &mut handle,
        );
    }
    let _ = handle.flush();
}

struct StateFmt<'src> {
    line_num_offset: usize,
    context_fmts: Vec<ContextFmt<'src>>,
}

struct ContextFmt<'src> {
    file: &'src File,
    path: &'src Path,
    message: &'src str,
    range: TextRange,
    location: TextLocation,
    line_range: TextRange,
    line_num: String,
    severity: DiagnosticSeverity,
}

impl<'src> StateFmt<'src> {
    fn new() -> StateFmt<'src> {
        StateFmt {
            line_num_offset: 0,
            context_fmts: Vec::with_capacity(8),
        }
    }

    fn reset(&mut self) {
        self.line_num_offset = 0;
        self.context_fmts.clear();
    }

    fn push(&mut self, fmt: ContextFmt<'src>) {
        self.line_num_offset = self.line_num_offset.max(fmt.line_num.len());
        self.context_fmts.push(fmt);
    }
}

impl<'src> ContextFmt<'src> {
    fn new(
        session: &'src Session,
        context: &'src DiagnosticContext,
        severity: DiagnosticSeverity,
    ) -> ContextFmt<'src> {
        let file = session.file(context.source().file_id());
        let path = file
            .path
            .strip_prefix(session.cwd())
            .unwrap_or_else(|_| &file.path);

        let range = context.source().range();
        let location = text::find_text_location(&file.source, range.start(), &file.line_ranges);
        let line_num = location.line().to_string();
        let line_range = file.line_ranges[location.line_index()];

        ContextFmt {
            file,
            path,
            message: context.message(),
            range,
            location,
            line_range,
            line_num,
            severity,
        }
    }
}

fn print_diagnostic<'src>(
    session: Option<&'src Session>,
    diagnostic: &'src Diagnostic,
    severity: DiagnosticSeverity,
    state: &mut StateFmt<'src>,
    handle: &mut BufWriter<Stderr>,
) {
    let message = diagnostic.message().as_str();
    let _ = writeln!(
        handle,
        "{}{}: {}{message}{}",
        severity_color(severity),
        severity_name(severity),
        ansi::WHITE_BOLD,
        ansi::RESET
    );

    match diagnostic.kind() {
        DiagnosticKind::Message => {
            let _ = write!(handle, "\n");
            return;
        }
        DiagnosticKind::Context { main, info } => {
            let session = session.expect("session context");
            state.reset();

            state.push(ContextFmt::new(session, main, severity));
            if let Some(info) = info {
                state.push(ContextFmt::new(session, info, DiagnosticSeverity::Info));
            }
        }
        DiagnosticKind::ContextVec { main, info_vec } => {
            let session = session.expect("session context");
            state.reset();

            state.push(ContextFmt::new(session, main, severity));
            for info in info_vec {
                state.push(ContextFmt::new(session, info, DiagnosticSeverity::Info));
            }
        }
    };

    let line_pad = " ".repeat(state.line_num_offset);
    for (idx, fmt) in state.context_fmts.iter().enumerate() {
        let last = idx + 1 == state.context_fmts.len();
        let line_num_pad = " ".repeat(state.line_num_offset - fmt.line_num.len());
        print_context(handle, fmt, last, &line_pad, &line_num_pad);
    }
    let _ = write!(handle, "\n");
}

fn print_context(
    handle: &mut BufWriter<Stderr>,
    fmt: &ContextFmt,
    last: bool,
    line_pad: &str,
    line_num_pad: &str,
) {
    let prefix_range = TextRange::new(fmt.line_range.start(), fmt.range.start());
    let source_range = TextRange::new(
        fmt.range.start(),
        (fmt.line_range.end() - 1.into()).min(fmt.range.end()),
    );

    let line_str = &fmt.file.source[fmt.line_range.as_usize()];
    let prefix_str = &fmt.file.source[prefix_range.as_usize()];
    let source_str = &fmt.file.source[source_range.as_usize()];

    let line = line_str.trim_end().replace('\t', TAB_REPLACE_STR);
    let marker_pad = " ".repeat(normalized_tab_len(prefix_str));
    let marker = severity_marker(fmt.severity).repeat(normalized_tab_len(source_str));
    let message = fmt.message;

    let c = ansi::CYAN;
    let r = ansi::RESET;
    let box_char = if last { '└' } else { '├' };

    let _ = writeln!(
        handle,
        r#"{line_pad} {c}│
{}{line_num_pad} │{r} {line}
{line_pad} {c}│ {marker_pad}{}{marker} {message}
{line_pad} {c}{box_char}─ {}:{:?}{r}"#,
        fmt.line_num,
        severity_color(fmt.severity),
        fmt.path.to_string_lossy(),
        fmt.location,
    );
}

const TAB_SPACE_COUNT: usize = 2;
const TAB_REPLACE_STR: &str = "  ";

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

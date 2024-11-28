use crate::ansi::AnsiStyle;
use rock_core::error::{
    Diagnostic, DiagnosticContext, DiagnosticData, Error, ErrorBuffer, ErrorWarningBuffer,
    Severity, Warning, WarningBuffer,
};
use rock_core::session::Session;
use rock_core::text::{self, TextLocation, TextRange};
use std::io::{BufWriter, Stderr, Write};
use std::path::Path;

pub fn print_errors(session: Option<&Session>, err: ErrorBuffer) {
    let errors = err.collect();
    print_impl(session, errors, vec![])
}

pub fn print_warnings(session: Option<&Session>, warn: WarningBuffer) {
    let warnings = warn.collect();
    print_impl(session, vec![], warnings)
}

pub fn print_errors_warnings(session: Option<&Session>, errw: ErrorWarningBuffer) {
    let (errors, warnings) = errw.collect();
    print_impl(session, errors, warnings)
}

pub fn print_impl(session: Option<&Session>, errors: Vec<Error>, warnings: Vec<Warning>) {
    let mut state = StateFmt::new();
    let mut handle = BufWriter::new(std::io::stderr());

    for warning in warnings.iter() {
        print_diagnostic(
            session,
            warning.diagnostic(),
            Severity::Warning,
            &mut state,
            &mut handle,
        );
    }
    for error in errors.iter() {
        print_diagnostic(
            session,
            error.diagnostic(),
            Severity::Error,
            &mut state,
            &mut handle,
        );
    }
    let _ = handle.flush();

    std::mem::forget(errors);
    std::mem::forget(warnings);
}

struct StateFmt<'src> {
    style: AnsiStyle,
    line_num_offset: usize,
    context_fmts: Vec<ContextFmt<'src>>,
}

struct ContextFmt<'src> {
    source: &'src str,
    path: &'src Path,
    message: &'src str,
    range: TextRange,
    location: TextLocation,
    line_range: TextRange,
    line_num: String,
    severity: Severity,
}

impl<'src> StateFmt<'src> {
    fn new() -> StateFmt<'src> {
        StateFmt {
            style: AnsiStyle::new(),
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
        severity: Severity,
    ) -> ContextFmt<'src> {
        let module = session.module.get(context.src().module_id());
        let file = session.vfs.file(module.file_id());
        let path = file
            .path()
            .strip_prefix(&session.curr_work_dir)
            .unwrap_or_else(|_| file.path());

        let range = context.src().range();
        let location = text::find_text_location(&file.source, range.start(), &file.line_ranges);
        let line_num = location.line().to_string();
        let line_range = file.line_ranges[location.line_index()];

        ContextFmt {
            source: &file.source,
            path,
            message: context.msg(),
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
    severity: Severity,
    state: &mut StateFmt<'src>,
    handle: &mut BufWriter<Stderr>,
) {
    let wb = state.style.err.white_bold;
    let r = state.style.err.reset;

    let _ = writeln!(
        handle,
        "{}{}: {wb}{}{r}",
        severity_color(&state.style, severity),
        severity_name(severity),
        diagnostic.msg().as_str(),
    );

    match diagnostic.data() {
        DiagnosticData::Message => {
            let _ = writeln!(handle);
            return;
        }
        DiagnosticData::Context { main, info } => {
            let session = session.expect("session context");
            state.reset();

            state.push(ContextFmt::new(session, &main, severity));
            if let Some(info) = info {
                state.push(ContextFmt::new(session, info.context(), Severity::Info));
            }
        }
        DiagnosticData::ContextVec { main, info_vec } => {
            let session = session.expect("session context");
            state.reset();

            state.push(ContextFmt::new(session, &main, severity));
            for info in info_vec {
                state.push(ContextFmt::new(session, info.context(), Severity::Info));
            }
        }
    };

    let line_pad = " ".repeat(state.line_num_offset);
    for (idx, fmt) in state.context_fmts.iter().enumerate() {
        let last = idx + 1 == state.context_fmts.len();
        let line_num_pad = " ".repeat(state.line_num_offset - fmt.line_num.len());
        print_context(state, handle, fmt, last, &line_pad, &line_num_pad);
    }
    let _ = writeln!(handle);
}

fn print_context(
    state: &StateFmt,
    handle: &mut BufWriter<Stderr>,
    fmt: &ContextFmt,
    last: bool,
    line_pad: &str,
    line_num_pad: &str,
) {
    let prefix_range = TextRange::new(fmt.line_range.start(), fmt.range.start());
    let source_range = TextRange::new(fmt.range.start(), fmt.line_range.end().min(fmt.range.end()));

    let line_str = &fmt.source[fmt.line_range.as_usize()];
    let prefix_str = &fmt.source[prefix_range.as_usize()];
    let source_str = (&fmt.source[source_range.as_usize()]).trim_end();

    let line = line_str.trim_end().replace('\t', TAB_REPLACE_STR);
    let marker_pad = " ".repeat(normalized_tab_len(prefix_str));
    let marker = severity_marker(fmt.severity).repeat(normalized_tab_len(source_str));
    let space = if fmt.message.is_empty() { "" } else { " " };
    let message = fmt.message;

    let c = state.style.err.cyan;
    let r = state.style.err.reset;
    let box_char = if last { '└' } else { '├' };

    //@required for tests to be uniform, allocates
    // long-term switch to custom utf8 pathbuf wrapper
    #[cfg(target_os = "windows")]
    let path_display = fmt.path.to_string_lossy().replace('\\', "/");
    #[cfg(not(target_os = "windows"))]
    let path_display = fmt.path.to_string_lossy();

    let _ = writeln!(
        handle,
        r#"{line_pad} {c}│
{}{line_num_pad} │{r} {line}
{line_pad} {c}│ {marker_pad}{}{marker}{space}{message}
{line_pad} {c}{box_char}─ {}:{:?}{r}"#,
        fmt.line_num,
        severity_color(&state.style, fmt.severity),
        path_display,
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

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

const fn severity_marker(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "─",
        Severity::Error | Severity::Warning => "^",
    }
}

const fn severity_color(style: &AnsiStyle, severity: Severity) -> &'static str {
    match severity {
        Severity::Info => style.err.green_bold,
        Severity::Error => style.err.red_bold,
        Severity::Warning => style.err.yellow_bold,
    }
}

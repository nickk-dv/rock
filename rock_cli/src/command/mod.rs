mod execute;
mod format;
mod parse;

use crate::error_format;
use rock_core::error::{DiagnosticCollection, WarningComp};
use rock_core::package::PackageKind;
use rock_core::session::BuildKind;

enum Command {
    New(CommandNew),
    Check,
    Build(CommandBuild),
    Run(CommandRun),
    Help,
    Version,
}

struct CommandNew {
    name: String,
    kind: PackageKind,
    no_git: bool,
}

struct CommandBuild {
    kind: BuildKind,
}

struct CommandRun {
    kind: BuildKind,
    args: Vec<String>,
}

pub fn run() {
    let result = run_impl();
    let diagnostics = DiagnosticCollection::from_result(result);
    error_format::print_errors(None, diagnostics);
}

//@format and command parsing warnings get printed after check / build / run diagnostics, fine? 24.05.24
fn run_impl() -> Result<Vec<WarningComp>, DiagnosticCollection> {
    let (format, warnings) = format::parse().into_result(vec![])?;
    let (command, warnings) = parse::command(format).into_result(warnings)?;
    let ((), warnings) = execute::command(command).into_result(warnings)?;
    Ok(warnings)
}

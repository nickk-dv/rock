mod execute;
mod format;
mod parse;

use crate::error_format;
use rock_core::config::BuildKind;
use rock_core::error::{DiagnosticCollection, ResultComp, WarningComp};
use rock_core::package::manifest::PackageKind;

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
    emit_llvm: bool,
}

struct CommandRun {
    kind: BuildKind,
    emit_llvm: bool,
    args: Vec<String>,
}

pub fn run() {
    let result = run_impl();
    error_format::print_errors(None, DiagnosticCollection::from_result(result));
}

//@feedback print after check / build / run, possibly with timer
fn run_impl() -> Result<Vec<WarningComp>, DiagnosticCollection> {
    let command = match parse::parse() {
        Ok(command) => command,
        Err(errors) => return Err(DiagnosticCollection::new().join_errors(errors)),
    };

    let result = execute::command(command);
    let ((), warnings) = ResultComp::from_error(result).into_result(vec![])?;
    Ok(warnings)
}

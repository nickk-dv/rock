mod execute;
mod format;
mod parse;

use crate::error_format;
use rock_core::error::DiagnosticCollection;
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
    let (format, warnings_0) = match format::parse() {
        Ok(format) => format,
        Err(diagnostics) => {
            error_format::print_errors(None, diagnostics);
            return;
        }
    };
    let command = match parse::command(format) {
        Ok(command) => command,
        Err(errors) => {
            error_format::print_errors(None, DiagnosticCollection::new().join_errors(errors));
            return;
        }
    };
    let ((), warnings_1) = match execute::command(command) {
        Ok(command) => command,
        Err(diagnostics) => {
            error_format::print_errors(None, diagnostics.join_warnings(warnings_0));
            return;
        }
    };
    error_format::print_errors(
        None,
        DiagnosticCollection::new()
            .join_warnings(warnings_0)
            .join_warnings(warnings_1),
    );
}

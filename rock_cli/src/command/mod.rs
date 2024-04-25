mod execute;
mod format;
mod parse;

use crate::error_format;
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
    let format = match format::parse() {
        Ok(format) => format,
        Err(error) => {
            return error_format::print_errors(None, &[error]);
        }
    };
    let command = match parse::command(format) {
        Ok(command) => command,
        Err(errors) => {
            return error_format::print_errors(None, &errors);
        }
    };
    if let Err(errors) = execute::command(command) {
        error_format::print_errors(None, &errors);
    }
}

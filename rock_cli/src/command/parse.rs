use super::format::CommandFormat;
use super::{Command, CommandBuild, CommandNew, CommandRun};
use rock_core::codegen::BuildKind;
use rock_core::error::{ErrorComp, ResultComp, WarningComp};
use rock_core::package::manifest::PackageKind;

pub fn command(format: CommandFormat) -> ResultComp<Command> {
    match format.name.as_str() {
        "n" | "new" => ResultComp::from_error(parse_new(format)),
        "c" | "check" => parse_check(format),
        "b" | "build" => ResultComp::from_errors(parse_build(format)),
        "r" | "run" => ResultComp::from_errors(parse_run(format)),
        "h" | "help" => parse_help(format),
        "v" | "version" => parse_version(format),
        _ => {
            let error = ErrorComp::message(format!(
                "command `{}` does not exist, use `rock help` to learn the usage",
                format.name
            ));
            ResultComp::from_error(Err(error))
        }
    }
}

fn parse_new(format: CommandFormat) -> Result<Command, ErrorComp> {
    let data = CommandNew {
        name: parse_new_name(&format)?,
        kind: parse_package_kind(&format, PackageKind::Bin),
        no_git: parse_bool_flag(&format, "no_git", false),
    };
    Ok(Command::New(data))
}

fn parse_check(format: CommandFormat) -> ResultComp<Command> {
    parse_simple_command(format, "check", Command::Check)
}

fn parse_build(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    let data = CommandBuild {
        kind: parse_build_kind(&format, BuildKind::Debug),
    };
    Ok(Command::Build(data))
}

fn parse_run(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    let data = CommandRun {
        kind: parse_build_kind(&format, BuildKind::Debug),
        args: format.trail_args,
    };
    Ok(Command::Run(data))
}

fn parse_help(format: CommandFormat) -> ResultComp<Command> {
    parse_simple_command(format, "help", Command::Help)
}

fn parse_version(format: CommandFormat) -> ResultComp<Command> {
    parse_simple_command(format, "version", Command::Version)
}

fn parse_simple_command(
    format: CommandFormat,
    full_name: &str,
    command: Command,
) -> ResultComp<Command> {
    if !format.args.is_empty() || !format.options.is_empty() {
        let warning = WarningComp::message(format!(
            "`{}` command does not take any options or arguments",
            full_name
        ));
        ResultComp::Ok((command, vec![warning]))
    } else {
        ResultComp::Ok((command, vec![]))
    }
}

fn parse_bool_flag(format: &CommandFormat, name: &str, default: bool) -> bool {
    if format.options.contains_key(name) {
        true
    } else {
        default
    }
}

fn parse_new_name(format: &CommandFormat) -> Result<String, ErrorComp> {
    if let Some(arg) = format.args.first() {
        Ok(arg.to_string())
    } else {
        Err(ErrorComp::message(
            "missing new package name, use `rock help` to learn the usage",
        ))
    }
}

fn parse_build_kind(format: &CommandFormat, default: BuildKind) -> BuildKind {
    let debug = format.options.contains_key("debug");
    let release = format.options.contains_key("release");

    if debug && release {
        // multiple enum values cannot be set at the same time @25.04.24
        // error ? warn and use default?
        return default;
    }

    if debug {
        return BuildKind::Debug;
    }
    if release {
        return BuildKind::Release;
    }
    default
}

fn parse_package_kind(format: &CommandFormat, default: PackageKind) -> PackageKind {
    let lib = format.options.contains_key("lib");
    let bin = format.options.contains_key("bin");

    if lib && bin {
        // multiple enum values cannot be set at the same time @25.04.24
        // error ? warn and use default?
        return default;
    }

    if lib {
        return PackageKind::Lib;
    }
    if bin {
        return PackageKind::Bin;
    }
    default
}

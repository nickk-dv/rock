use super::format::CommandFormat;
use super::{Command, CommandBuild, CommandNew, CommandRun};
use rock_core::error::ErrorComp;
use rock_core::package::PackageKind;
use rock_core::session::BuildKind;

pub fn command(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    match format.name.as_str() {
        "n" | "new" => parse_new(format),
        "c" | "check" => parse_check(format),
        "b" | "build" => parse_build(format),
        "r" | "run" => parse_run(format),
        "h" | "help" => parse_help(format),
        "v" | "version" => parse_version(format),
        _ => {
            return Err(vec![ErrorComp::message(format!(
                "command `{}` does not exist, use `rock help` to learn the usage",
                format.name
            ))]);
        }
    }
}

fn parse_new(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    let data = CommandNew {
        name: parse_new_name(&format)?,
        kind: parse_package_kind(&format, PackageKind::Bin),
        no_git: parse_bool_flag(&format, "no_git", false),
    };
    Ok(Command::New(data))
}

fn parse_check(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
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

fn parse_help(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    parse_simple_command(format, "help", Command::Help)
}

fn parse_version(format: CommandFormat) -> Result<Command, Vec<ErrorComp>> {
    parse_simple_command(format, "version", Command::Version)
}

fn parse_simple_command(
    format: CommandFormat,
    full_name: &str,
    command: Command,
) -> Result<Command, Vec<ErrorComp>> {
    if !format.args.is_empty() || !format.options.is_empty() {
        return Ok(command);
        //@todo 18.05.24
        //Err(vec![ErrorComp::message_warning(format!(
        //    "`{}` command does not take any options or arguments",
        //    full_name
        //))])
    } else {
        Ok(command)
    }
}

fn parse_bool_flag(format: &CommandFormat, name: &str, default: bool) -> bool {
    if format.options.contains_key(name) {
        true
    } else {
        default
    }
}

fn parse_new_name(format: &CommandFormat) -> Result<String, Vec<ErrorComp>> {
    if let Some(arg) = format.args.first() {
        Ok(arg.to_string())
    } else {
        Err(vec![ErrorComp::message(
            "missing new package name, use `rock help` to learn the usage",
        )])
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

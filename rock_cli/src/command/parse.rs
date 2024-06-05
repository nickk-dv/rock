use super::format::CommandFormat;
use super::{Command, CommandBuild, CommandNew, CommandRun};
use rock_core::codegen::BuildKind;
use rock_core::error::{DiagnosticCollection, ErrorComp, ResultComp, WarningComp};
use rock_core::package::manifest::PackageKind;

pub fn command(format: CommandFormat) -> ResultComp<Command> {
    match format.name.as_str() {
        "n" | "new" => parse_new(format),
        "c" | "check" => parse_simple_command(&format, "check", Command::Check),
        "b" | "build" => parse_build(format),
        "r" | "run" => parse_run(format),
        "h" | "help" => parse_simple_command(&format, "help", Command::Help),
        "v" | "version" => parse_simple_command(&format, "version", Command::Version),
        _ => {
            let error = ErrorComp::message(format!(
                "`{}` command does not exist, use `rock help` to learn the usage",
                format.name
            ));
            ResultComp::from_error(Err(error))
        }
    }
}

fn parse_new(format: CommandFormat) -> ResultComp<Command> {
    let mut diagnostics = DiagnosticCollection::new();
    check_command_args(&format, &mut diagnostics, "new", true, false);
    check_expected_option_set(&format, &mut diagnostics, &["lib", "bin", "no-git"]);

    let name = parse_package_name(&format, &mut diagnostics);
    let kind = parse_package_kind(&format, &mut diagnostics, PackageKind::Bin);
    let no_git = parse_bool_flag(&format, &mut diagnostics, "no-git", false);

    let data = CommandNew { name, kind, no_git };
    ResultComp::new(Command::New(data), diagnostics)
}

fn parse_build(format: CommandFormat) -> ResultComp<Command> {
    let mut diagnostics = DiagnosticCollection::new();
    check_command_args(&format, &mut diagnostics, "build", false, false);
    check_expected_option_set(&format, &mut diagnostics, &["debug", "release"]);

    let kind = parse_build_kind(&format, &mut diagnostics, BuildKind::Debug);

    let data = CommandBuild { kind };
    ResultComp::new(Command::Build(data), diagnostics)
}

fn parse_run(format: CommandFormat) -> ResultComp<Command> {
    let mut diagnostics: DiagnosticCollection = DiagnosticCollection::new();
    check_command_args(&format, &mut diagnostics, "run", false, true);
    check_expected_option_set(&format, &mut diagnostics, &["debug", "release"]);

    let kind = parse_build_kind(&format, &mut diagnostics, BuildKind::Debug);

    let data = CommandRun {
        kind,
        args: format.trail_args,
    };
    ResultComp::new(Command::Run(data), diagnostics)
}

fn parse_simple_command(
    format: &CommandFormat,
    cmd_name: &str,
    command: Command,
) -> ResultComp<Command> {
    if !format.args.is_empty() || !format.options.is_empty() || !format.trail_args.is_empty() {
        let warning = WarningComp::message(format!(
            "`{cmd_name}` command does not take any options or arguments",
        ));
        ResultComp::Ok((command, vec![warning]))
    } else {
        ResultComp::Ok((command, vec![]))
    }
}

fn check_command_args(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    cmd_name: &str,
    has_args: bool,
    has_trail_args: bool,
) {
    if !has_args && !format.args.is_empty() {
        diagnostics.warning(WarningComp::message(format!(
            "`{cmd_name}` command does not take any arguments"
        )));
    }
    if !has_trail_args && !format.trail_args.is_empty() {
        diagnostics.warning(WarningComp::message(format!(
            "`{cmd_name}` command does not take any trailing arguments"
        )));
    }
}

fn check_expected_option_set(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    expected: &[&str],
) {
    for (option, _) in format.options.iter() {
        if !expected.contains(&option.as_str()) {
            diagnostics.warning(WarningComp::message(format!(
                "option `--{option}` is not recognized and will be ignored"
            )));
        }
    }
}

fn check_option_no_args(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    name: &str,
) -> bool {
    if let Some(args) = format.options.get(name) {
        if !args.is_empty() {
            diagnostics.warning(WarningComp::message(format!(
                "option `--{name}` does not take any arguments"
            )));
        }
        true
    } else {
        false
    }
}

fn parse_bool_flag(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    name: &str,
    default: bool,
) -> bool {
    let flag_set = check_option_no_args(format, diagnostics, name);
    if flag_set {
        true
    } else {
        default
    }
}

fn parse_package_name(format: &CommandFormat, diagnostics: &mut DiagnosticCollection) -> String {
    if let Some(arg) = format.args.first() {
        if format.args.len() > 1 {
            diagnostics.warning(WarningComp::message(
                "`new` command expects one argument, other arguments will be ignored",
            ));
        }
        arg.clone()
    } else {
        diagnostics.error(ErrorComp::message(
            "missing new package name, use `rock help` to learn the usage",
        ));
        "error".into()
    }
}

fn parse_build_kind(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    default: BuildKind,
) -> BuildKind {
    let debug_str = BuildKind::Debug.as_str();
    let release_str = BuildKind::Release.as_str();

    let debug = check_option_no_args(format, diagnostics, debug_str);
    let release = check_option_no_args(format, diagnostics, release_str);

    if debug && release {
        diagnostics.error(ErrorComp::message(format!(
            "conflicting options `--{debug_str}` and `--{release_str}` cannot be used together"
        )));
        return default;
    }

    if debug {
        BuildKind::Debug
    } else if release {
        BuildKind::Release
    } else {
        default
    }
}

fn parse_package_kind(
    format: &CommandFormat,
    diagnostics: &mut DiagnosticCollection,
    default: PackageKind,
) -> PackageKind {
    let bin_str = PackageKind::Bin.as_str();
    let lib_str = PackageKind::Lib.as_str();

    let bin = check_option_no_args(format, diagnostics, bin_str);
    let lib = check_option_no_args(format, diagnostics, lib_str);

    if bin && lib {
        diagnostics.error(ErrorComp::message(format!(
            "conflicting options `--{bin_str}` and `--{lib_str}` cannot be used together"
        )));
        return default;
    }

    if bin {
        PackageKind::Bin
    } else if lib {
        PackageKind::Lib
    } else {
        default
    }
}

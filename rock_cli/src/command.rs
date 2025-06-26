use rock_codegen::BuildOptions;
use rock_core::error::ErrorBuffer;
use rock_core::errors as err;
use rock_core::session::config::Build;
use rock_core::session::manifest::PackageKind;
use rock_core::support::AsStr;
use std::collections::{HashMap, HashSet};

pub enum Command {
    New(CommandNew),
    Check(CommandCheck),
    Build(CommandBuild),
    Run(CommandRun),
    Fmt,
    Help,
    Version,
}

pub struct CommandNew {
    pub name: String,
    pub kind: PackageKind,
    pub no_git: bool,
}

pub struct CommandCheck {
    pub warn: bool,
    pub stats: bool,
}

pub struct CommandBuild {
    pub build: Build,
    pub warn: bool,
    pub stats: bool,
    pub options: BuildOptions,
}

pub struct CommandRun {
    pub build: Build,
    pub warn: bool,
    pub stats: bool,
    pub options: BuildOptions,
    pub args: Vec<String>,
}

//==================== PARSE COMMAND ====================

pub fn parse() -> Result<Command, ErrorBuffer> {
    let format = format()?;
    let mut p = CommandParser { err: ErrorBuffer::default(), format };

    let command = match p.format.cmd.as_str() {
        "n" | "new" => command_new(&mut p),
        "c" | "check" => command_check(&mut p),
        "b" | "build" => command_build(&mut p),
        "r" | "run" => command_run(&mut p),
        "f" | "fmt" => command_fmt(&mut p),
        "h" | "help" => command_help(&mut p),
        "v" | "version" => command_version(&mut p),
        _ => {
            err::cmd_unknown(&mut p.err, &p.format.cmd);
            return Err(p.err);
        }
    };

    for (opt, _) in p.format.options {
        err::cmd_option_unknown(&mut p.err, &opt);
    }
    for opt in p.format.duplicates {
        err::cmd_option_duplicate(&mut p.err, &opt);
    }
    p.err.result(command)
}

fn command_new(p: &mut CommandParser) -> Command {
    let name = parse_args_single(p, "name");
    let kind = parse_option_enum_flag(p, PackageKind::Lib, PackageKind::Bin);
    let no_git = parse_option_flag(p, "no-git");
    parse_trail_args_none(p);

    let data = CommandNew { name, kind, no_git };
    Command::New(data)
}

fn command_check(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    let warn = parse_option_flag(p, "warn");
    let stats = parse_option_flag(p, "stats");
    parse_trail_args_none(p);

    let data = CommandCheck { warn, stats };
    Command::Check(data)
}

fn command_build(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    let build = parse_option_enum_flag(p, Build::Release, Build::Debug);
    let warn = parse_option_flag(p, "warn");
    let stats = parse_option_flag(p, "stats");
    let emit_llvm = parse_option_flag(p, "emit-llvm");
    parse_trail_args_none(p);

    let options = BuildOptions { emit_llvm };
    let data = CommandBuild { build, warn, stats, options };
    Command::Build(data)
}

fn command_run(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    let build = parse_option_enum_flag(p, Build::Release, Build::Debug);
    let warn = parse_option_flag(p, "warn");
    let stats = parse_option_flag(p, "stats");
    let emit_llvm = parse_option_flag(p, "emit-llvm");
    let args = parse_trail_args(p);

    let options = BuildOptions { emit_llvm };
    let data = CommandRun { build, warn, stats, options, args };
    Command::Run(data)
}

fn command_fmt(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    parse_trail_args_none(p);
    Command::Fmt
}

fn command_help(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    parse_trail_args_none(p);
    Command::Help
}

fn command_version(p: &mut CommandParser) -> Command {
    parse_args_none(p);
    parse_trail_args_none(p);
    Command::Version
}

struct CommandFormat {
    cmd: String,
    args: Vec<String>,
    options: HashMap<String, Vec<String>>,
    duplicates: HashSet<String>,
    trail_args: Vec<String>,
}

//==================== PARSE ARGS ====================

struct FormatParser {
    args: Vec<String>,
}

fn format() -> Result<CommandFormat, ErrorBuffer> {
    let mut p = FormatParser { args: std::env::args().skip(1).rev().collect() };

    let cmd = match format_eat_arg(&mut p) {
        Some(cmd) => cmd,
        None => {
            let mut err = ErrorBuffer::default();
            err::cmd_name_missing(&mut err);
            return Err(err);
        }
    };
    let args = format_args(&mut p);
    let (options, duplicates) = format_options(&mut p);
    let trail_args = p.args;
    Ok(CommandFormat { cmd, args, options, duplicates, trail_args })
}

fn format_args(p: &mut FormatParser) -> Vec<String> {
    let mut args = Vec::new();

    while let Some(arg) = format_eat_arg(p) {
        args.push(arg);
    }
    args
}

fn format_options(p: &mut FormatParser) -> (HashMap<String, Vec<String>>, HashSet<String>) {
    let mut options = HashMap::new();
    let mut duplicates = HashSet::new();

    while let Some(opt) = format_eat_option(p) {
        let args = format_args(p);

        if options.contains_key(&opt) {
            duplicates.insert(opt);
        } else {
            options.insert(opt, args);
        }
    }
    (options, duplicates)
}

fn format_eat_arg(p: &mut FormatParser) -> Option<String> {
    let next = p.args.last()?;
    if next.starts_with('-') {
        None
    } else {
        p.args.pop()
    }
}

fn format_eat_option(p: &mut FormatParser) -> Option<String> {
    let next = p.args.last()?;
    if next == "--" {
        p.args.pop();
        None
    } else {
        let opt = next.trim_start_matches('-').to_string();
        p.args.pop();
        Some(opt)
    }
}

//==================== PARSE FORMAT ====================

struct CommandParser {
    err: ErrorBuffer,
    format: CommandFormat,
}

fn parse_args_none(p: &mut CommandParser) {
    if !p.format.args.is_empty() {
        err::cmd_expect_no_args(&mut p.err, &p.format.cmd);
    }
}

fn parse_args_single(p: &mut CommandParser, name: &str) -> String {
    if let Some(arg) = p.format.args.first() {
        if p.format.args.len() > 1 {
            err::cmd_expect_single_arg(&mut p.err, &p.format.cmd, name);
        }
        arg.to_string()
    } else {
        err::cmd_expect_single_arg(&mut p.err, &p.format.cmd, name);
        "error".to_string()
    }
}

fn parse_option_flag(p: &mut CommandParser, name: &str) -> bool {
    parse_option_no_args(p, name)
}

fn parse_option_enum_flag<T: Copy + AsStr>(p: &mut CommandParser, value: T, default: T) -> T {
    if parse_option_no_args(p, value.as_str()) {
        value
    } else {
        default
    }
}

fn parse_option_no_args(p: &mut CommandParser, opt: &str) -> bool {
    if let Some(args) = p.format.options.remove(opt) {
        if !args.is_empty() {
            err::cmd_option_expect_no_args(&mut p.err, opt);
        }
        true
    } else {
        false
    }
}

fn parse_trail_args(p: &mut CommandParser) -> Vec<String> {
    p.format.trail_args.clone()
}

fn parse_trail_args_none(p: &mut CommandParser) {
    if !p.format.trail_args.is_empty() {
        err::cmd_expect_no_trail_args(&mut p.err, &p.format.cmd);
    }
}

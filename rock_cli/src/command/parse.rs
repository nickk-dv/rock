use super::format::{self, Arg, CommandFormat};
use super::{Command, CommandBuild, CommandNew, CommandRun};
use rock_core::config::BuildKind;
use rock_core::error::ErrorComp;
use rock_core::errors as err;
use rock_core::package::manifest::PackageKind;
use rock_core::support::AsStr;

pub fn parse() -> Result<Command, Vec<ErrorComp>> {
    let format = format::parse();

    let name = match format.name.as_ref() {
        Some(name) => name.clone(),
        None => {
            let mut errors = Vec::with_capacity(1);
            err::cmd_name_missing(&mut errors);
            return Err(errors);
        }
    };

    let p = CommandParser {
        cmd: name,
        format,
        errors: Vec::new(),
    };

    match p.cmd.as_str() {
        "n" | "new" => parse_new(p),
        "c" | "check" => parse_check(p),
        "b" | "build" => parse_build(p),
        "r" | "run" => parse_run(p),
        "h" | "help" => parse_help(p),
        "v" | "version" => parse_version(p),
        _ => {
            let mut errors = Vec::with_capacity(1);
            err::cmd_unknown(&mut errors, &p.cmd);
            return Err(errors);
        }
    }
}

fn parse_new(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    let name = p.args_single("name");
    let kind = p.option_enum(PackageKind::Bin);
    let no_git = p.option_flag(false, "no-git");
    p.trail_args_none();

    let data = CommandNew { name, kind, no_git };
    p.finish(Command::New(data))
}

fn parse_check(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    p.args_none();
    p.trail_args_none();
    p.finish(Command::Check)
}

fn parse_build(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    let kind = p.option_enum(BuildKind::Debug);
    let emit_llvm = p.option_flag(false, "emit-llvm");
    p.trail_args_none();

    let data = CommandBuild { kind, emit_llvm };
    p.finish(Command::Build(data))
}

fn parse_run(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    let kind = p.option_enum(BuildKind::Debug);
    let emit_llvm = p.option_flag(false, "emit-llvm");
    let args = p.trail_args();

    let data = CommandRun {
        kind,
        emit_llvm,
        args,
    };
    p.finish(Command::Run(data))
}

fn parse_help(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    p.args_none();
    p.trail_args_none();
    p.finish(Command::Help)
}

fn parse_version(mut p: CommandParser) -> Result<Command, Vec<ErrorComp>> {
    p.args_none();
    p.trail_args_none();
    p.finish(Command::Version)
}

struct CommandParser {
    cmd: String,
    format: CommandFormat,
    errors: Vec<ErrorComp>,
}

impl CommandParser {
    fn args_none(&mut self) {
        if !self.format.args.is_empty() {
            err::cmd_expect_no_args(&mut self.errors, &self.cmd);
        }
    }
    fn args_single(&mut self, name: &str) -> String {
        if let Some(arg) = self.format.args.get(0) {
            if self.format.args.len() > 1 {
                err::cmd_expect_single_arg(&mut self.errors, &self.cmd, name);
            }
            arg.to_string()
        } else {
            err::cmd_expect_single_arg(&mut self.errors, &self.cmd, name);
            "error".to_string()
        }
    }

    fn option_flag(&mut self, default: bool, name: &'static str) -> bool {
        if self.option_no_args(name) {
            true
        } else {
            default
        }
    }
    fn option_enum<T: Copy + AsStr>(&mut self, default: T) -> T {
        let mut selected = None;
        let mut variants = T::ALL.iter().copied();

        while let Some(value) = variants.next() {
            if self.option_no_args(value.as_str()) {
                selected = Some(value);
                break;
            }
        }

        if let Some(selected) = selected {
            for other in variants {
                if self.option_no_args(other.as_str()) {
                    let opt = selected.as_str();
                    let other = other.as_str();
                    err::cmd_option_conflict(&mut self.errors, opt, other);
                }
            }
            selected
        } else {
            default
        }
    }
    fn option_no_args(&mut self, opt: &'static str) -> bool {
        if let Some(args) = self.format.options.remove(opt) {
            if !args.is_empty() {
                err::cmd_option_expect_no_args(&mut self.errors, opt);
            }
            if self.format.duplicates.remove(opt) {
                err::cmd_option_duplicate(&mut self.errors, opt);
            }
            true
        } else {
            false
        }
    }
    fn option_with_args(&mut self, opt: &'static str) -> Option<Vec<Arg>> {
        if let Some(args) = self.format.options.remove(opt) {
            if self.format.duplicates.remove(opt) {
                err::cmd_option_duplicate(&mut self.errors, opt);
            }
            Some(args)
        } else {
            None
        }
    }

    fn trail_args_none(&mut self) {
        if !self.format.trail_args.is_empty() {
            err::cmd_expect_no_trail_args(&mut self.errors, &self.cmd);
        }
    }
    fn trail_args(&self) -> Vec<String> {
        self.format.trail_args.clone()
    }

    fn finish(mut self, command: Command) -> Result<Command, Vec<ErrorComp>> {
        for (opt, _) in self.format.options {
            err::cmd_option_unknown(&mut self.errors, &opt);
        }
        for opt in self.format.duplicates {
            err::cmd_option_duplicate(&mut self.errors, &opt);
        }
        if self.errors.is_empty() {
            Ok(command)
        } else {
            Err(self.errors)
        }
    }
}

use crate::cmd::{BuildKind, Command, CommandBuild, CommandNew, CommandRun, ProjectKind};
use rock_core::error::ErrorComp;

pub fn parse_args() -> Result<Command, Vec<ErrorComp>> {
    let parser = Parser::new();
    parser.parse()
}

struct Parser {
    index: usize,
    args: Vec<String>,
}

impl Parser {
    fn new() -> Parser {
        Parser {
            index: 0,
            args: std::env::args().skip(1).collect(),
        }
    }

    fn peek(&self) -> Option<String> {
        self.args.get(self.index).cloned()
    }

    fn eat(&mut self) {
        self.index += 1;
    }

    fn parse(mut self) -> Result<Command, Vec<ErrorComp>> {
        let command = match self.peek() {
            Some(name) => {
                self.eat();
                name
            }
            None => {
                return Err(vec![ErrorComp::error(
                    "command is missing, use `rock help` to learn the usage",
                )]);
            }
        };

        match command.as_str() {
            "n" | "new" => self.parse_new(),
            "c" | "check" => self.parse_check(),
            "b" | "build" => self.parse_build(),
            "r" | "run" => self.parse_run(),
            "h" | "help" => self.parse_help(),
            "v" | "version" => self.parse_version(),
            "init" | "create" | "make" | "start" | "setup" | "initialize" | "begin" | "project" => {
                Err(vec![ErrorComp::error(
                    "did you mean `rock new`? use `rock help` to learn the usage",
                )])
            }
            _ => Err(vec![ErrorComp::error(format!(
                "command `{}` does not exist, use `rock help` to learn the usage",
                command
            ))]),
        }
    }

    fn parse_new(&mut self) -> Result<Command, Vec<ErrorComp>> {
        let name = match self.peek() {
            Some(name) => {
                self.eat();
                name
            }
            None => {
                return Err(vec![ErrorComp::error(
                    "missing package name, use `rock help` to learn the usage",
                )]);
            }
        };
        Ok(Command::New(CommandNew {
            name,
            //@not parsed
            kind: ProjectKind::Bin,
            no_git: false,
        }))
    }

    fn parse_check(&mut self) -> Result<Command, Vec<ErrorComp>> {
        Ok(Command::Check)
    }

    fn parse_build(&mut self) -> Result<Command, Vec<ErrorComp>> {
        //@not parsed
        Ok(Command::Build(CommandBuild {
            kind: BuildKind::Debug,
        }))
    }

    fn parse_run(&mut self) -> Result<Command, Vec<ErrorComp>> {
        //@not parsed
        Ok(Command::Run(CommandRun {
            kind: BuildKind::Debug,
            args: Vec::new(),
        }))
    }

    fn parse_help(&mut self) -> Result<Command, Vec<ErrorComp>> {
        Ok(Command::Help)
    }

    fn parse_version(&mut self) -> Result<Command, Vec<ErrorComp>> {
        Ok(Command::Version)
    }
}

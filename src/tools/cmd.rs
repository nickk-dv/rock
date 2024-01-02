use crate::ast::parser;
use crate::hir::check;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const VERSION_MAJOR: u32 = 0; // major releases
const VERSION_MINOR: u32 = 1; // minor changes
const VERSION_PATCH: u32 = 0; // hotfixes to current release

pub fn cmd_parse() -> Result<(), ()> {
    let cmd = CmdParser::parse()?;
    cmd.execute()?;
    Ok(())
}

fn cmd_new(cmd: &Cmd) -> Result<(), ()> {
    let mut path = PathBuf::new();
    path.push(cmd.args.get(0).unwrap());
    fs::create_dir(path.clone());

    let main_file = r#"main :: () -> i32 {
    return 0;
}
"#;
    let lib_file = r#"
// library project
"#;
    let gitignore_file = r#"build/
"#;
    path.push("src");
    fs::create_dir(path.clone());
    path.push("main.lang");
    let mut file = fs::File::create(path.clone()).unwrap();
    file.write_all(main_file.as_bytes());

    path.pop();
    path.pop();
    path.push(".gitignore");
    let mut file_git = fs::File::create(path).unwrap();
    file_git.write_all(gitignore_file.as_bytes());

    let git_init = std::process::Command::new("git")
        .arg("init")
        .status()
        .expect("Failed to execute command")
        .success();
    Ok(())
}

fn cmd_check() -> Result<(), ()> {
    let mut ast = parser::parse()?;
    check::check(&mut ast)?;
    Ok(())
}

fn cmd_build() -> Result<(), ()> {
    let mut ast = parser::parse()?;
    check::check(&mut ast)?;
    Ok(())
}

fn cmd_run() -> Result<(), ()> {
    let mut ast = parser::parse()?;
    check::check(&mut ast)?;
    Ok(())
}

fn cmd_fmt() -> Result<(), ()> {
    let _ = parser::parse()?;
    Ok(())
}

fn cmd_version() -> Result<(), ()> {
    //@update lang name
    println!(
        "lang version: {}.{}.{}",
        VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH
    );
    Ok(())
}

fn cmd_help() -> Result<(), ()> {
    //@update lang name
    let help = r#"
Usage:
    lang <command>
    command: name [arguments] [options]
    option: -name [arguments]

Commands:
    new, n       Create project
    check, c     Check the program
    build, b     Build the program
    run, r       Build and run the program
    fmt, f       Format source files
    help, h      Print help information
    version, v   Print compiler version

Options:
    new:
      -lib       Create library project
      -exe       Create executable project
      -no_git    Create project without git repository
    build, run:
      -debug     Build in debug mode
      -release   Build in release mode
    run:
      -args      Pass command line arguments to the program
    fmt:
      -file      Format specific source file
"#;
    println!("{}", help);
    Ok(())
}

struct Cmd {
    kind: CmdKind,
    args: Vec<String>,
    options: Vec<CmdOption>,
}

struct CmdOption {
    kind: CmdOptionKind,
    args: Vec<String>,
}

enum CmdKind {
    New,
    Check,
    Build,
    Run,
    Fmt,
    Help,
    Version,
}

impl CmdKind {
    fn from_str(str: &str) -> Result<CmdKind, CmdError> {
        match str {
            "n" | "new" => Ok(CmdKind::New),
            "c" | "check" => Ok(CmdKind::Check),
            "b" | "build" => Ok(CmdKind::Build),
            "r" | "run" => Ok(CmdKind::Run),
            "f" | "fmt" => Ok(CmdKind::Fmt),
            "h" | "help" => Ok(CmdKind::Help),
            "v" | "version" => Ok(CmdKind::Version),
            _ => Err(CmdError::PrimaryInvalid),
        }
    }
}

enum CmdOptionKind {
    NewNoGit,
    BuildRunDebug,
    BuildRunRelease,
    RunArgs,
    FmtFile,
}

impl CmdOptionKind {
    fn from_str(str: &str) -> Result<CmdOptionKind, CmdError> {
        match str {
            "-no_git" => Ok(CmdOptionKind::NewNoGit),
            "-debug" => Ok(CmdOptionKind::BuildRunDebug),
            "-release" => Ok(CmdOptionKind::BuildRunRelease),
            "-args" => Ok(CmdOptionKind::RunArgs),
            "-file" => Ok(CmdOptionKind::FmtFile),
            _ => Err(CmdError::OptionInvalid),
        }
    }
}

enum CmdError {
    PrimaryMissing,
    PrimaryInvalid,
    OptionMissingDash,
    OptionInvalid,
}

struct CmdParser {
    args: Vec<String>,
    peek_index: usize,
}

impl Cmd {
    fn verify(&self) -> Result<(), CmdError> {
        Ok(())
    }

    fn execute(&self) -> Result<(), ()> {
        match self.kind {
            CmdKind::New => cmd_new(self),
            CmdKind::Check => cmd_check(),
            CmdKind::Build => cmd_build(),
            CmdKind::Run => cmd_run(),
            CmdKind::Fmt => cmd_fmt(),
            CmdKind::Help => cmd_help(),
            CmdKind::Version => cmd_version(),
        }
    }
}

impl CmdParser {
    pub fn parse() -> Result<Cmd, ()> {
        let mut cmd_parser = CmdParser::new();
        let result = cmd_parser.parse_cmd();
        match result {
            Ok(cmd) => Ok(cmd),
            Err(_) => Err(()), //@report err
        }
    }

    fn new() -> Self {
        Self {
            args: std::env::args().skip(1).collect(),
            peek_index: 0,
        }
    }

    fn parse_cmd(&mut self) -> Result<Cmd, CmdError> {
        let cmd = Cmd {
            kind: self.parse_primary()?,
            args: self.parse_args(),
            options: self.parse_options()?,
        };
        cmd.verify()?;
        Ok(cmd)
    }

    fn parse_primary(&mut self) -> Result<CmdKind, CmdError> {
        if let Some(arg) = self.try_consume() {
            Ok(CmdKind::from_str(arg)?)
        } else {
            return Err(CmdError::PrimaryMissing);
        }
    }

    fn parse_options(&mut self) -> Result<Vec<CmdOption>, CmdError> {
        let mut options = Vec::new();
        while let Some(arg) = self.try_consume() {
            if !arg.starts_with("-") {
                return Err(CmdError::OptionMissingDash);
            }
            let kind = CmdOptionKind::from_str(arg)?;
            let args = self.parse_args();
            options.push(CmdOption { kind, args });
        }
        Ok(options)
    }

    fn parse_args(&mut self) -> Vec<String> {
        let mut args = Vec::new();
        while let Some(arg) = self.try_consume() {
            if arg.starts_with("-") {
                break;
            }
            args.push(arg.clone());
        }
        args
    }

    fn try_consume(&mut self) -> Option<&String> {
        match self.args.get(self.peek_index) {
            Some(arg) => {
                self.peek_index += 1;
                Some(arg)
            }
            None => None,
        }
    }
}

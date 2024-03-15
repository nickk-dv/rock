use crate::ast;
use crate::ast::ast::Ast;
use crate::ast::parse;
use crate::ast::CompCtx;
use crate::error;
use crate::error::ErrorComp;
use crate::hir_lower;
use crate::mem::Arena;
use crate::vfs;
use std::path::PathBuf;

const VERSION_MAJOR: u32 = 0; // major releases
const VERSION_MINOR: u32 = 1; // minor changes
const VERSION_PATCH: u32 = 0; // hotfixes to current release

pub fn cmd_parse() -> Result<(), ()> {
    let cmd = CmdParser::parse()?;
    cmd.execute()?;
    Ok(())
}

fn cmd_new(cmd: &Cmd) -> Result<(), ErrorComp> {
    //@temp unwrap, no cli arg validation is done
    let package_name = cmd.args.get(0).unwrap();
    validate_name(&package_name)?;

    let cwd = std::env::current_dir().unwrap();
    let package_dir = cwd.join(package_name);
    let src_dir = package_dir.join("src");
    let build_dir = package_dir.join("build");
    let main_path = src_dir.join("main.lang");
    let gitignore_path = package_dir.join(".gitignore");

    make_dir(&package_dir)?;
    make_dir(&src_dir)?;
    make_dir(&build_dir)?;
    make_file(&main_path, "\nproc main() -> s32 {\n\treturn 0;\n}\n")?;
    make_file(&gitignore_path, "build/\n")?;
    git_init(&package_dir)?;

    fn validate_name(name: &str) -> Result<(), ErrorComp> {
        for c in name.chars() {
            let valid = c.is_alphanumeric() || c == '-' || c == '_';
            if !valid {
                return Err(ErrorComp::error(format!(
                "package name must consist only of alphanumeric characters, underscores `_` or hyphens `-`",
            )));
            }
        }
        Ok(())
    }

    fn make_dir(path: &PathBuf) -> Result<(), ErrorComp> {
        if let Err(io_error) = std::fs::create_dir(path) {
            return Err(ErrorComp::error(format!(
                "failed to create directory: {}",
                io_error
            )));
        }
        Ok(())
    }

    fn make_file(path: &PathBuf, text: &str) -> Result<(), ErrorComp> {
        if let Err(io_error) = std::fs::write(path, text) {
            return Err(ErrorComp::error(format!(
                "failed to create file: {}",
                io_error
            )));
        }
        Ok(())
    }

    fn git_init(package_dir: &PathBuf) -> Result<(), ErrorComp> {
        if let Err(io_error) = std::env::set_current_dir(package_dir) {
            return Err(ErrorComp::error(format!(
                "failed to set working directory: {}",
                io_error
            )));
        }
        if let Err(io_error) = std::process::Command::new("git").arg("init").status() {
            return Err(ErrorComp::error(format!(
                "failed to initialize git repository: {}",
                io_error
            )));
        }
        Ok(())
    }

    Ok(())
}

fn cmd_check() -> Result<(), ()> {
    let mut ctx = CompCtx::new();
    let mut ast = Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };
    let errors = parse(&mut ctx, &mut ast);
    error::format::print_errors(&ctx.vfs, &errors);
    eprintln!("ast arena mem usage: {}", ast.arena.mem_usage());

    let hir = match hir_lower::check(&ctx, ast) {
        Ok(hir) => hir,
        Err(errors) => {
            error::format::print_errors(&ctx.vfs, &errors);
            return Err(());
        }
    };
    Ok(())
}

fn cmd_build() -> Result<(), ()> {
    Ok(())
}

fn cmd_run() -> Result<(), ()> {
    Ok(())
}

fn cmd_fmt() -> Result<(), ()> {
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
            CmdKind::New => match cmd_new(self) {
                Err(error) => {
                    let vfs = vfs::Vfs::new();
                    error::format::print_errors(&vfs, &[error]);
                    Err(())
                }
                Ok(()) => Ok(()),
            },
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

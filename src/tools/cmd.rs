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

mod new {
    use crate::error::ansi;
    use crate::error::ErrorComp;
    use std::path::PathBuf;

    pub struct CmdData {
        pub name: String,
        pub kind: ProjectKind,
        pub no_git: bool,
    }

    pub enum ProjectKind {
        Lib,
        Bin,
    }

    pub fn cmd(data: CmdData) -> Result<(), ErrorComp> {
        let cwd = std::env::current_dir().unwrap();
        let root_dir = cwd.join(&data.name);
        let src_dir = root_dir.join("src");
        let build_dir = root_dir.join("build");

        check_name(&data.name)?;
        make_dir(&root_dir)?;
        make_dir(&src_dir)?;
        make_dir(&build_dir)?;

        match data.kind {
            ProjectKind::Lib => {
                let path = src_dir.join("lib.lang");
                make_file(&path, "")?;
            }
            ProjectKind::Bin => {
                let path = src_dir.join("main.lang");
                make_file(&path, "\nproc main() -> s32 {\n\treturn 0;\n}\n")?;
            }
        }
        if !data.no_git {
            let gitignore = root_dir.join(".gitignore");
            make_file(&gitignore, "build/\n")?;
            git_init(&root_dir)?;
        }

        let kind_name = match data.kind {
            ProjectKind::Lib => "library",
            ProjectKind::Bin => "executable",
        };
        println!(
            "{}Created{} {kind_name} `{}` package",
            ansi::GREEN_BOLD,
            ansi::CLEAR,
            data.name,
        );
        Ok(())
    }

    fn check_name(name: &str) -> Result<(), ErrorComp> {
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ErrorComp::error("package name must consist only of alphanumeric characters, underscores `_` or hyphens `-`"));
        }
        Ok(())
    }

    fn make_dir(path: &PathBuf) -> Result<(), ErrorComp> {
        std::fs::create_dir(path).map_err(|io_error| {
            ErrorComp::error(format!("failed to create directory: {}", io_error))
        })
    }

    fn make_file(path: &PathBuf, text: &str) -> Result<(), ErrorComp> {
        std::fs::write(path, text)
            .map_err(|io_error| ErrorComp::error(format!("failed to create file: {}", io_error)))
    }

    fn git_init(package_dir: &PathBuf) -> Result<(), ErrorComp> {
        std::env::set_current_dir(package_dir).map_err(|io_error| {
            ErrorComp::error(format!("failed to set working directory: {}", io_error))
        })?;
        std::process::Command::new("git")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|io_error| {
                ErrorComp::error(format!("failed to initialize git repository: {}", io_error))
            })?;
        Ok(())
    }
}

mod check {
    pub fn cmd() {}
}

pub enum BuildKind {
    Debug,
    Release,
}

mod build {
    use super::BuildKind;
    pub struct CmdData {
        pub kind: BuildKind,
    }

    pub fn cmd(data: CmdData) {}
}

mod run {
    use super::BuildKind;
    pub struct CmdData {
        pub kind: BuildKind,
        pub args: Vec<String>,
    }

    pub fn cmd(data: CmdData) {}
}

mod help {
    use crate::error::ansi;

    pub fn cmd() {
        let g = ansi::GREEN_BOLD;
        let c = ansi::CYAN_BOLD;
        let r = ansi::CLEAR;

        #[rustfmt::skip]
        println!(
r#"
{g}Usage:
  {c}lang <command> [options]

{g}Commands:
  {c}n, new       {r}Create new project
  {c}c, check     {r}Check the program
  {c}b, build     {r}Build the program
  {c}r, run       {r}Build and run the program
  {c}h, help      {r}Print help information
  {c}v, version   {r}Print compiler version

{g}Options:
  {c}new:
    {c}--lib      {r}Create library project
    {c}--bin      {r}Create executable project
    {c}--no_git   {r}Create project without git

  {c}build, run:
    {c}--debug    {r}Build in debug mode
    {c}--release  {r}Build in release mode

  {c}run:
    {c}--args     {r}Pass command line arguments
"#);
    }
}

mod version {
    use std::fmt;

    pub fn cmd() {
        println!("lang version: {VERSION}");
    }

    const VERSION: Version = Version {
        major: 0,
        minor: 1,
        patch: 0,
    };

    struct Version {
        major: u32,
        minor: u32,
        patch: u32,
    }

    impl fmt::Display for Version {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

pub fn cmd_parse() -> Result<(), ()> {
    let cmd = CmdParser::parse()?;
    cmd.execute()?;
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
            CmdKind::New => match new::cmd(new::CmdData {
                name: "temp_name".into(),
                no_git: false,
                kind: new::ProjectKind::Lib,
            }) {
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
            CmdKind::Help => {
                help::cmd();
                Ok(())
            }
            CmdKind::Version => {
                version::cmd();
                Ok(())
            }
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

pub enum Command {
    New(CommandNew),
    Check,
    Build(CommandBuild),
    Run(CommandRun),
    Help,
    Version,
}

pub struct CommandNew {
    pub name: String,
    pub kind: ProjectKind,
    pub no_git: bool,
}

pub struct CommandBuild {
    pub kind: BuildKind,
}

pub struct CommandRun {
    pub kind: BuildKind,
    pub args: Vec<String>,
}

pub enum ProjectKind {
    Lib,
    Bin,
}

pub enum BuildKind {
    Debug,
    Release,
}

pub mod new {
    use super::{CommandNew, ProjectKind};
    use crate::error::{self, ansi, ErrorComp};
    use crate::vfs::Vfs;
    use std::path::PathBuf;

    pub fn cmd(data: CommandNew) {
        if let Err(error) = make_project(data) {
            let vfs = Vfs::new();
            error::format::print_errors(&vfs, &[error]);
        }
    }

    fn make_project(data: CommandNew) -> Result<(), ErrorComp> {
        let cwd = std::env::current_dir().unwrap();
        let root_dir = cwd.join(&data.name);
        let src_dir = root_dir.join("src");
        let build_dir = root_dir.join("build");

        check_name(&data.name)?;
        make_dir(&root_dir)?;
        make_dir(&src_dir)?;
        make_dir(&build_dir)?;

        match data.kind {
            ProjectKind::Lib => make_file(&src_dir.join("lib.lang"), "")?,
            ProjectKind::Bin => make_file(
                &src_dir.join("main.lang"),
                "\nproc main() -> s32 {\n\treturn 0;\n}\n",
            )?,
        }

        if !data.no_git {
            make_file(&root_dir.join(".gitattributes"), "* text eol=lf\n")?;
            make_file(&root_dir.join(".gitignore"), "build/\n")?;
            make_file(&root_dir.join("README.md"), &format!("# {}\n", data.name))?;
            git_init(&root_dir)?;
        }

        let kind_name = match data.kind {
            ProjectKind::Lib => "library",
            ProjectKind::Bin => "executable",
        };
        println!(
            "  {}Created{} {kind_name} `{}` package",
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

pub mod check {
    use crate::ast::{parse, CompCtx};
    use crate::error;
    use crate::hir_lower;

    pub fn cmd() {
        let mut ctx = CompCtx::new();
        let ast = match parse(&mut ctx) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
        let hir = match hir_lower::check(&mut ctx, ast) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
    }
}

pub mod build {
    use super::CommandBuild;
    use crate::ast::{parse, CompCtx};
    use crate::error;
    use crate::hir_lower;

    pub fn cmd(data: CommandBuild) {
        let mut ctx = CompCtx::new();
        let ast = match parse(&mut ctx) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
        let hir = match hir_lower::check(&mut ctx, ast) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
        //@no build
    }
}

pub mod run {
    use super::CommandRun;
    use crate::ast::{parse, CompCtx};
    use crate::error;
    use crate::hir_lower;

    pub fn cmd(data: CommandRun) {
        let mut ctx = CompCtx::new();
        let ast = match parse(&mut ctx) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
        let hir = match hir_lower::check(&mut ctx, ast) {
            Ok(ast) => ast,
            Err(errors) => {
                error::format::print_errors(&ctx.vfs, &errors);
                return;
            }
        };
        //@no build
        //@no run
    }
}

pub mod help {
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
  {c}n, new <name>   {r}Create new project
  {c}c, check        {r}Check the program
  {c}b, build        {r}Build the program
  {c}r, run          {r}Build and run the program
  {c}h, help         {r}Print help information
  {c}v, version      {r}Print compiler version

{g}Options:
  {c}new
    {c}--lib         {r}Create library project
    {c}--bin         {r}Create executable project
    {c}--no_git      {r}Create project without git

  {c}build
    {c}--debug       {r}Build in debug mode
    {c}--release     {r}Build in release mode

  {c}run
    {c}--debug       {r}Run the debug build
    {c}--release     {r}Run the release build
    {c}--args [args] {r}Pass command line arguments
"#);
    }
}

pub mod version {
    use crate::error::ansi;
    use std::fmt;

    pub fn cmd() {
        println!(
            "  {}Lang version:{} {VERSION}",
            ansi::GREEN_BOLD,
            ansi::CLEAR
        );
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

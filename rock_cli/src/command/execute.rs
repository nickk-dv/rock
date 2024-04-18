use super::{Command, CommandBuild, CommandNew, CommandRun};
use crate::ansi;
use crate::error_format;
use rock_core::ast_parse;
use rock_core::codegen;
use rock_core::error::ErrorComp;
use rock_core::hir_lower;
use rock_core::package::{BuildManifest, Manifest, PackageKind, PackageManifest, Semver};
use rock_core::session::Session;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn command(command: Command) -> Result<(), Vec<ErrorComp>> {
    match command {
        Command::New(data) => new(data).map_err(|e| vec![e]),
        Command::Check => check(),
        Command::Build(data) => build(data),
        Command::Run(data) => run(data),
        Command::Help => {
            help();
            Ok(())
        }
        Command::Version => {
            version();
            Ok(())
        }
    }
}

fn check() -> Result<(), Vec<ErrorComp>> {
    let session = Session::new()?;
    if let Err(errors) = inner(&session) {
        error_format::print_errors(Some(&session), &errors);
    }
    return Ok(());

    fn inner(session: &Session) -> Result<(), Vec<ErrorComp>> {
        let ast = ast_parse::parse(&session)?;
        let _ = hir_lower::check(ast)?;
        Ok(())
    }
}

fn build(data: CommandBuild) -> Result<(), Vec<ErrorComp>> {
    let session = Session::new()?;
    if let Err(errors) = inner(&session) {
        error_format::print_errors(Some(&session), &errors);
    }
    return Ok(());

    fn inner(session: &Session) -> Result<(), Vec<ErrorComp>> {
        let ast = ast_parse::parse(&session)?;
        let hir = hir_lower::check(ast)?;
        codegen::codegen(hir);
        Ok(())
    }
}

fn run(data: CommandRun) -> Result<(), Vec<ErrorComp>> {
    let session = Session::new()?;
    if let Err(errors) = inner(&session) {
        error_format::print_errors(Some(&session), &errors);
    }
    return Ok(());

    fn inner(session: &Session) -> Result<(), Vec<ErrorComp>> {
        let ast = ast_parse::parse(&session)?;
        let hir = hir_lower::check(ast)?;
        codegen::codegen(hir);
        Ok(())
    }
}

fn version() {
    let g = ansi::GREEN_BOLD;
    let r = ansi::RESET;
    println!("  {g}Rock version:{r} {}", rock_core::VERSION,);
}

fn help() {
    let g = ansi::GREEN_BOLD;
    let c = ansi::CYAN_BOLD;
    let r = ansi::RESET;

    #[rustfmt::skip]
        println!(
r#"
{g}Usage:
  {c}rock <command> [options]

{g}Commands:
  {c}n, new <name>   {r}Create new package
  {c}c, check        {r}Check the program
  {c}b, build        {r}Build the program
  {c}r, run          {r}Build and run the program
  {c}h, help         {r}Print help information
  {c}v, version      {r}Print compiler version

{g}Options:
  {c}new
    {c}--lib         {r}Create library package
    {c}--bin         {r}Create executable package
    {c}--no_git      {r}Create package without git repo

  {c}build
    {c}--debug       {r}Build in debug mode
    {c}--release     {r}Build in release mode

  {c}run
    {c}--debug       {r}Run the debug build
    {c}--release     {r}Run the release build
    {c}--args [args] {r}Pass command line arguments
"#);
}

pub fn new(data: CommandNew) -> Result<(), ErrorComp> {
    let cwd = std::env::current_dir().unwrap();
    let root_dir = cwd.join(&data.name);
    let src_dir = root_dir.join("src");
    let build_dir = root_dir.join("build");

    check_name(&data.name)?;
    make_dir(&root_dir)?;
    make_dir(&src_dir)?;
    make_dir(&build_dir)?;

    match data.kind {
        PackageKind::Lib => make_file(&src_dir.join("lib.rock"), "")?,
        PackageKind::Bin => make_file(
            &src_dir.join("main.rock"),
            "\nproc main() -> s32 {\n\treturn 0;\n}\n",
        )?,
    }

    let mut dependencies = BTreeMap::new();
    dependencies.insert("core".to_string(), rock_core::VERSION);

    let manifest = Manifest {
        package: PackageManifest {
            name: data.name.clone(),
            kind: data.kind,
            version: Semver::new(0, 1, 0),
        },
        build: if matches!(data.kind, PackageKind::Bin) {
            Some(BuildManifest {
                bin_name: data.name.clone(),
            })
        } else {
            None
        },
        dependencies,
    };
    let manifest = manifest.serialize()?;
    make_file(&root_dir.join("Rock.toml"), &manifest)?;

    if !data.no_git {
        make_file(&root_dir.join(".gitattributes"), "* text eol=lf\n")?;
        make_file(&root_dir.join(".gitignore"), "build/\n")?;
        make_file(&root_dir.join("README.md"), &format!("# {}\n", data.name))?;
        git_init(&root_dir)?;
    }

    let g = ansi::GREEN_BOLD;
    let r = ansi::RESET;
    println!(
        "  {g}Created{r} {} `{}` package",
        data.kind.as_str_full(),
        data.name,
    );
    return Ok(());

    fn check_name(name: &str) -> Result<(), ErrorComp> {
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ErrorComp::message("package name must consist only of alphanumeric characters, underscores `_` or hyphens `-`"));
        }
        Ok(())
    }

    fn make_dir(path: &PathBuf) -> Result<(), ErrorComp> {
        std::fs::create_dir(path).map_err(|io_error| {
            ErrorComp::message(format!("failed to create directory: {}", io_error))
        })
    }

    fn make_file(path: &PathBuf, text: &str) -> Result<(), ErrorComp> {
        std::fs::write(path, text)
            .map_err(|io_error| ErrorComp::message(format!("failed to create file: {}", io_error)))
    }

    fn git_init(package_dir: &PathBuf) -> Result<(), ErrorComp> {
        std::env::set_current_dir(package_dir).map_err(|io_error| {
            ErrorComp::message(format!("failed to set working directory: {}", io_error))
        })?;
        std::process::Command::new("git")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!("failed to initialize git repository: {}", io_error))
            })?;
        Ok(())
    }
}

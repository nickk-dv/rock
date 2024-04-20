use super::{Command, CommandBuild, CommandNew, CommandRun};
use crate::ansi;
use crate::error_format;
use rock_core::ast_parse;
use rock_core::codegen;
use rock_core::error::ErrorComp;
use rock_core::fs_env;
use rock_core::hir_lower;
use rock_core::package::{BuildManifest, Manifest, PackageKind, PackageManifest, Semver};
use rock_core::session::Session;
use std::collections::BTreeMap;

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
        let _ = hir_lower::check(ast, session)?;
        Ok(())
    }
}

fn build(data: CommandBuild) -> Result<(), Vec<ErrorComp>> {
    let session = Session::new()?;
    if let Err(errors) = inner(&session, data) {
        error_format::print_errors(Some(&session), &errors);
    }
    return Ok(());

    fn inner(session: &Session, data: CommandBuild) -> Result<(), Vec<ErrorComp>> {
        let ast = ast_parse::parse(&session)?;
        let hir = hir_lower::check(ast, session)?;
        codegen::codegen(
            hir,
            &session.root_package_bin_name(),
            codegen::BuildConfig::Build(data.kind),
        )
        .map_err(|e| vec![e])?;
        Ok(())
    }
}

fn run(data: CommandRun) -> Result<(), Vec<ErrorComp>> {
    let session = Session::new()?;
    if let Err(errors) = inner(&session, data) {
        error_format::print_errors(Some(&session), &errors);
    }
    return Ok(());

    fn inner(session: &Session, data: CommandRun) -> Result<(), Vec<ErrorComp>> {
        let ast = ast_parse::parse(&session)?;
        let hir = hir_lower::check(ast, session)?;
        codegen::codegen(
            hir,
            &session.root_package_bin_name(),
            codegen::BuildConfig::Run(data.kind, data.args),
        )
        .map_err(|e| vec![e])?;
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
    let cwd = fs_env::dir_get_current()?;
    let root_dir = cwd.join(&data.name);
    let src_dir = root_dir.join("src");
    let build_dir = root_dir.join("build");

    package_name_check(&data.name)?;
    fs_env::dir_create(&root_dir)?;
    fs_env::dir_create(&src_dir)?;
    fs_env::dir_create(&build_dir)?;

    //@print project name in both when core library imports are working @18.04.24
    match data.kind {
        PackageKind::Lib => fs_env::file_create_or_rewrite(&src_dir.join("lib.rock"), "")?,
        PackageKind::Bin => fs_env::file_create_or_rewrite(
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
    fs_env::file_create_or_rewrite(&root_dir.join("Rock.toml"), &manifest)?;

    if !data.no_git {
        fs_env::file_create_or_rewrite(&root_dir.join(".gitignore"), "build/\n")?;
        fs_env::file_create_or_rewrite(&root_dir.join("README.md"), &format!("# {}\n", data.name))?;
        fs_env::dir_set_current(&root_dir)?;
        std::process::Command::new("git")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!(
                    "failed to initialize git repository\nreason:{}",
                    io_error
                ))
            })?;
    }

    let g = ansi::GREEN_BOLD;
    let r = ansi::RESET;
    println!(
        "  {g}Created{r} {} `{}` package",
        data.kind.as_str_full(),
        data.name,
    );
    Ok(())
}

fn package_name_check(name: &str) -> Result<(), ErrorComp> {
    let mut chars = name.chars();
    if let Some(c) = chars.next() {
        if !(c == '_' || c.is_alphabetic()) {
            return Err(ErrorComp::message(format!(
                "package name must be a valid identifier, first `{}` is not allowed",
                c
            )));
        }
    }
    for c in chars {
        if !(c == '_' || c.is_alphabetic() || c.is_ascii_digit()) {
            return Err(ErrorComp::message(format!(
                "package name must be a valid identifier, inner `{}` is not allowed",
                c
            )));
        }
    }
    Ok(())
}

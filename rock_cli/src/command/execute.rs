use super::{Command, CommandBuild, CommandNew, CommandRun};
use crate::ansi;
use crate::error_format;
use rock_core::ast_parse;
use rock_core::error::{DiagnosticCollection, ErrorComp, ResultComp, WarningComp};
use rock_core::fs_env;
use rock_core::hir_lower;
use rock_core::intern::InternPool;
use rock_core::package;
use rock_core::package::manifest::{BuildManifest, Manifest, PackageKind, PackageManifest};
use rock_core::package::semver::Semver;
use rock_core::session::Session;
use std::collections::BTreeMap;

pub fn command(command: Command) -> Result<(), ErrorComp> {
    match command {
        Command::New(data) => new(data),
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

pub fn new(data: CommandNew) -> Result<(), ErrorComp> {
    let cwd = fs_env::dir_get_current_working()?;
    let root_dir = cwd.join(&data.name);
    let src_dir = root_dir.join("src");

    package::verify_name(&data.name)?;
    fs_env::dir_create(&root_dir, true)?;
    fs_env::dir_create(&src_dir, true)?;

    const IMPORT_CORE_IO: &str = "import core:libc.{ printf };\n\n";
    match data.kind {
        PackageKind::Bin => {
            let bin_content = format!(
                "{IMPORT_CORE_IO}proc main() -> s32 {{\n    printf(c\"Bin `{}` works\\n\");\n    return 0;\n}}\n",
                data.name
            );
            fs_env::file_create_or_rewrite(&src_dir.join("main.rock"), &bin_content)?
        }
        PackageKind::Lib => {
            let lib_content = format!(
                "{IMPORT_CORE_IO}proc test() {{\n    printf(c\"Lib `{}` works\\n\");\n}}\n",
                data.name
            );
            fs_env::file_create_or_rewrite(&src_dir.join("test.rock"), &lib_content)?;
        }
    }

    {
        let package = PackageManifest {
            name: data.name.clone(),
            kind: data.kind,
            version: Semver::new(0, 1, 0),
            owner: None,
            authors: None,
            description: None,
        };

        let build = match data.kind {
            PackageKind::Bin => BuildManifest {
                bin_name: Some(data.name.clone()),
                nodefaultlib: None,
                lib_paths: None,
                links: None,
            },
            PackageKind::Lib => BuildManifest {
                bin_name: None,
                nodefaultlib: None,
                lib_paths: None,
                links: None,
            },
        };

        let mut dependencies = BTreeMap::new();
        dependencies.insert("core".to_string(), rock_core::VERSION);

        let manifest = Manifest {
            package,
            build,
            dependencies,
        };

        let manifest_text = package::manifest_serialize(&manifest)?;
        fs_env::file_create_or_rewrite(&root_dir.join("Rock.toml"), &manifest_text)?;
    }

    if !data.no_git {
        fs_env::file_create_or_rewrite(&root_dir.join(".gitignore"), "build/\n")?;
        fs_env::file_create_or_rewrite(&root_dir.join("README.md"), &format!("# {}\n", data.name))?;
        fs_env::dir_set_current_working(&root_dir)?;

        std::process::Command::new("git")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!(
                    "failed to initialize git repository\nreason: {}",
                    io_error
                ))
            })?;
    }

    let g = ansi::GREEN_BOLD;
    let r = ansi::RESET;
    println!(
        "  {g}Created{r} {} `{}` package\n",
        data.kind.as_str_full(),
        data.name,
    );
    Ok(())
}

fn check() -> Result<(), ErrorComp> {
    let (session, intern_name) = Session::new(false, None)?;
    let result = check_impl(&session, intern_name);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn check_impl(
        session: &Session,
        intern_name: InternPool,
    ) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session, intern_name).into_result(vec![])?;
        let (_, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        Ok(warnings)
    }
}

fn build(data: CommandBuild) -> Result<(), ErrorComp> {
    let (session, intern_name) = Session::new(true, None)?;
    let result = build_impl(&session, intern_name, data);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn build_impl(
        session: &Session,
        intern_name: InternPool,
        data: CommandBuild,
    ) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session, intern_name).into_result(vec![])?;
        let (hir, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        let diagnostics = DiagnosticCollection::new().join_warnings(warnings);
        error_format::print_errors(Some(session), diagnostics);

        rock_llvm::codegen(hir);
        Ok(vec![])
        //let result = codegen_ll::codegen_module(session, hir);
        //let (_, warnings) = ResultComp::from_error(result).into_result(vec![])?;
        //Ok(warnings)
    }
}

fn run(data: CommandRun) -> Result<(), ErrorComp> {
    let (session, intern_name) = Session::new(true, None)?;
    let result = run_impl(&session, intern_name, data);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn run_impl(
        session: &Session,
        intern_name: InternPool,
        data: CommandRun,
    ) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session, intern_name).into_result(vec![])?;
        let (hir, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        let diagnostics = DiagnosticCollection::new().join_warnings(warnings);
        error_format::print_errors(Some(session), diagnostics);

        //let result = codegen::codegen(hir, session, data.kind, data.emit_llvm, Some(data.args));
        //let (_, warnings) = ResultComp::from_error(result).into_result(vec![])?;
        //Ok(warnings)
        rock_llvm::codegen(hir);
        Ok(vec![])
    }
}

fn help() {
    let g = ansi::GREEN_BOLD;
    let c = ansi::CYAN_BOLD;
    let r = ansi::RESET;

    #[rustfmt::skip]
    println!(r#"{g}
Usage:
  {c}rock <command> [options]

{g}Commands:
  {c}n, new <name>  {r}Create new package
  {c}c, check       {r}Check the program
  {c}b, build       {r}Build the program
  {c}r, run         {r}Build and run the program
  {c}h, help        {r}Print help information
  {c}v, version     {r}Print compiler version

{g}Options:
  {c}new
    {c}--lib        {r}Create {} package
    {c}--bin        {r}Create {} package
    {c}--no-git     {r}Create package without git repo

  {c}build, run
    {c}--debug      {r}Build in debug mode
    {c}--release    {r}Build in release mode
    {c}--emit-llvm  {r}Save llvm module to file

  {c}run
    {c}-- [args]    {r}Pass command line arguments
"#,
    PackageKind::Lib.as_str_full(),
    PackageKind::Bin.as_str_full());
}

fn version() {
    let g = ansi::GREEN_BOLD;
    let r = ansi::RESET;
    println!("  {g}Rock version:{r} {}\n", rock_core::VERSION);
}

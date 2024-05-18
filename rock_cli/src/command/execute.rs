use super::{Command, CommandBuild, CommandNew, CommandRun};
use crate::ansi;
use crate::error_format;
use rock_core::ast_parse;
use rock_core::codegen;
use rock_core::error::{DiagnosticCollection, ErrorComp, WarningComp};
use rock_core::fs_env;
use rock_core::hir_lower;
use rock_core::package::{BuildManifest, Manifest, PackageKind, PackageManifest, Semver};
use rock_core::session::Session;
use std::collections::BTreeMap;

pub fn command(command: Command) -> Result<((), Vec<WarningComp>), DiagnosticCollection> {
    match command {
        Command::New(data) => match new(data) {
            Ok(()) => Ok(((), vec![])),
            Err(error) => Err(DiagnosticCollection::new().join_errors(vec![error])),
        },
        Command::Check => match check() {
            Ok(()) => Ok(((), vec![])),
            Err(error) => Err(DiagnosticCollection::new().join_errors(vec![error])),
        },
        Command::Build(data) => match build(data) {
            Ok(()) => Ok(((), vec![])),
            Err(error) => Err(DiagnosticCollection::new().join_errors(vec![error])),
        },
        Command::Run(data) => match run(data) {
            Ok(()) => Ok(((), vec![])),
            Err(error) => Err(DiagnosticCollection::new().join_errors(vec![error])),
        },
        Command::Help => {
            help();
            Ok(((), vec![]))
        }
        Command::Version => {
            version();
            Ok(((), vec![]))
        }
    }
}

fn check() -> Result<(), ErrorComp> {
    let session = Session::new()?;
    match inner(&session) {
        Ok(((), warnings)) => error_format::print_errors(
            Some(&session),
            DiagnosticCollection::new().join_warnings(warnings),
        ),
        Err(diagnostics) => {
            error_format::print_errors(Some(&session), diagnostics);
        }
    }
    return Ok(());

    fn inner(session: &Session) -> Result<((), Vec<WarningComp>), DiagnosticCollection> {
        let ast = ast_parse::parse(session)
            .map_err(|errors| DiagnosticCollection::new().join_errors(errors))?;
        let (_, warnings) = hir_lower::check(ast, session)?;
        Ok(((), warnings))
    }
}

fn build(data: CommandBuild) -> Result<(), ErrorComp> {
    let session = Session::new()?;
    match inner(&session, data) {
        Ok(((), warnings)) => error_format::print_errors(
            Some(&session),
            DiagnosticCollection::new().join_warnings(warnings),
        ),
        Err(diagnostics) => {
            error_format::print_errors(Some(&session), diagnostics);
        }
    }
    return Ok(());

    fn inner(
        session: &Session,
        data: CommandBuild,
    ) -> Result<((), Vec<WarningComp>), DiagnosticCollection> {
        let ast = ast_parse::parse(session)
            .map_err(|errors| DiagnosticCollection::new().join_errors(errors))?;
        let (hir, warnings_0) = hir_lower::check(ast, session)?;
        codegen::codegen(
            hir,
            &session.root_package_bin_name(),
            codegen::BuildConfig::Build(data.kind),
        )
        .map_err(|error| {
            DiagnosticCollection::new()
                .join_errors(vec![error])
                .join_warnings(warnings_0.clone())
        })?;
        Ok(((), warnings_0))
    }
}

//@run happens before warnings are printed 18.05.24
fn run(data: CommandRun) -> Result<(), ErrorComp> {
    let session = Session::new()?;
    match inner(&session, data) {
        Ok(((), warnings)) => error_format::print_errors(
            Some(&session),
            DiagnosticCollection::new().join_warnings(warnings),
        ),
        Err(diagnostics) => {
            error_format::print_errors(Some(&session), diagnostics);
        }
    }
    return Ok(());

    fn inner(
        session: &Session,
        data: CommandRun,
    ) -> Result<((), Vec<WarningComp>), DiagnosticCollection> {
        let ast = ast_parse::parse(session)
            .map_err(|errors| DiagnosticCollection::new().join_errors(errors))?;
        let (hir, warnings_0) = hir_lower::check(ast, session)?;
        codegen::codegen(
            hir,
            &session.root_package_bin_name(),
            codegen::BuildConfig::Run(data.kind, data.args),
        )
        .map_err(|error| {
            DiagnosticCollection::new()
                .join_errors(vec![error])
                .join_warnings(warnings_0.clone())
        })?;
        Ok(((), warnings_0))
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
  {c}n, new <name>  {r}Create new package
  {c}c, check       {r}Check the program
  {c}b, build       {r}Build the program
  {c}r, run         {r}Build and run the program
  {c}h, help        {r}Print help information
  {c}v, version     {r}Print compiler version

{g}Options:
  {c}new
    {c}--lib        {r}Create library package
    {c}--bin        {r}Create executable package
    {c}--no_git     {r}Create package without git repo

  {c}build
    {c}--debug      {r}Build in debug mode
    {c}--release    {r}Build in release mode

  {c}run
    {c}--debug      {r}Run the debug build
    {c}--release    {r}Run the release build
    {c}-- [args]    {r}Pass command line arguments
"#);
}

pub fn new(data: CommandNew) -> Result<(), ErrorComp> {
    let cwd = fs_env::dir_get_current_working()?;
    let root_dir = cwd.join(&data.name);
    let src_dir = root_dir.join("src");
    let build_dir = root_dir.join("build");

    package_name_check(&data.name)?;
    fs_env::dir_create(&root_dir, true)?;
    fs_env::dir_create(&src_dir, true)?;
    fs_env::dir_create(&build_dir, true)?;

    let main_content: String = format!(
        r#"import core/io;

proc main() -> s32 {{
    io.printf(c"Hello from {}\n");
    return 0;
}}
"#,
        data.name
    );

    let lib_content: String = format!(
        r#"import core/io;

proc test() {{
    io.printf(c"Lib {} works\n");
}}
"#,
        data.name
    );

    match data.kind {
        PackageKind::Lib => {
            fs_env::file_create_or_rewrite(&src_dir.join("test.rock"), &lib_content)?;
        }
        PackageKind::Bin => {
            fs_env::file_create_or_rewrite(&src_dir.join("main.rock"), &main_content)?
        }
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
        fs_env::dir_set_current_working(&root_dir)?;
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

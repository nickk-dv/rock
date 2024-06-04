use super::{Command, CommandBuild, CommandNew, CommandRun};
use crate::ansi;
use crate::error_format;
use rock_core::ast_parse;
use rock_core::codegen;
use rock_core::error::{DiagnosticCollection, ErrorComp, ResultComp, WarningComp};
use rock_core::fs_env;
use rock_core::hir_lower;
use rock_core::package;
use rock_core::package::manifest::{BuildManifest, Manifest, PackageKind, PackageManifest};
use rock_core::package::semver::Semver;
use rock_core::session::Session;
use std::collections::BTreeMap;

pub fn command(command: Command) -> ResultComp<()> {
    match command {
        Command::New(data) => ResultComp::from_error(new(data)),
        Command::Check => ResultComp::from_error(check()),
        Command::Build(data) => ResultComp::from_error(build(data)),
        Command::Run(data) => ResultComp::from_error(run(data)),
        Command::Help => {
            help();
            ResultComp::Ok(((), vec![]))
        }
        Command::Version => {
            version();
            ResultComp::Ok(((), vec![]))
        }
    }
}

fn check() -> Result<(), ErrorComp> {
    let session = Session::new()?;
    let result = check_impl(&session);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn check_impl(session: &Session) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session).into_result(vec![])?;
        let (_, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        Ok(warnings)
    }
}

fn build(data: CommandBuild) -> Result<(), ErrorComp> {
    let session = Session::new()?;
    let result = build_impl(&session, data);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn build_impl(
        session: &Session,
        data: CommandBuild,
    ) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session).into_result(vec![])?;
        let (hir, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        let bin_name = session.root_package_bin_name();
        let result = codegen::codegen(hir, bin_name, data.kind, None);
        let (_, warnings) = ResultComp::from_error(result).into_result(warnings)?;
        Ok(warnings)
    }
}

//@run happens before warnings are printed 18.05.24
fn run(data: CommandRun) -> Result<(), ErrorComp> {
    let session = Session::new()?;
    let result = run_impl(&session, data);
    error_format::print_errors(Some(&session), DiagnosticCollection::from_result(result));
    return Ok(());

    fn run_impl(
        session: &Session,
        data: CommandRun,
    ) -> Result<Vec<WarningComp>, DiagnosticCollection> {
        let (ast, warnings) = ast_parse::parse(session).into_result(vec![])?;
        let (hir, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
        let bin_name = session.root_package_bin_name();
        let result = codegen::codegen(hir, bin_name, data.kind, Some(data.args));
        let (_, warnings) = ResultComp::from_error(result).into_result(warnings)?;
        Ok(warnings)
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
    {c}--no-git     {r}Create package without git repo

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

    const IMPORT_CORE_IO: &'static str = "import core/io;\n\n";
    match data.kind {
        PackageKind::Bin => {
            let bin_content = format!(
                "{IMPORT_CORE_IO}proc main() -> s32 {{\n    io.printf(c\"Bin `{}` works\\n\");\n    return 0;\n}}\n",
                data.name
            );
            fs_env::file_create_or_rewrite(&src_dir.join("main.rock"), &bin_content)?
        }
        PackageKind::Lib => {
            let lib_content = format!(
                "{IMPORT_CORE_IO}proc test() {{\n    io.printf(c\"Lib `{}` works\\n\");\n}}\n",
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
            authors: None,
            repository: None,
            description: None,
        };

        let build = match data.kind {
            PackageKind::Bin => Some(BuildManifest {
                bin_name: data.name.clone(),
            }),
            PackageKind::Lib => None,
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

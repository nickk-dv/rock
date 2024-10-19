use crate::ansi::AnsiStyle;
use crate::command::{Command, CommandBuild, CommandNew, CommandRun};
use crate::error_print;
use rock_core::config::{BuildKind, Config, TargetTriple};
use rock_core::error::{Error, ErrorWarningBuffer, WarningBuffer};
use rock_core::format;
use rock_core::fs_env;
use rock_core::hir_lower;
use rock_core::package;
use rock_core::package::manifest::{BuildManifest, Manifest, PackageKind, PackageManifest};
use rock_core::package::semver::Semver;
use rock_core::session::{self, Session};
use rock_core::support::AsStr;
use rock_core::syntax::ast_build;
use std::collections::BTreeMap;

pub fn command(command: Command) -> Result<(), Error> {
    match command {
        Command::New(data) => new(data),
        Command::Check => check(),
        Command::Build(data) => build(data),
        Command::Run(data) => run(data),
        Command::Fmt => fmt(),
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

pub fn new(data: CommandNew) -> Result<(), Error> {
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
                "{IMPORT_CORE_IO}proc main() -> s32 {{\n    let _ = printf(c\"Bin `{}` works\\n\");\n    return 0;\n}}\n",
                data.name
            );
            fs_env::file_create_or_rewrite(&src_dir.join("main.rock"), &bin_content)?
        }
        PackageKind::Lib => {
            let lib_content = format!(
                "{IMPORT_CORE_IO}proc test() -> void {{\n    let _ = printf(c\"Lib `{}` works\\n\");\n}}\n",
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
                Error::message(format!(
                    "failed to initialize git repository\nreason: {}",
                    io_error
                ))
            })?;
    }

    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let r = style.out.reset;

    println!(
        "  {g}Created{r} {} `{}` package\n",
        data.kind.as_str_full(),
        data.name,
    );
    Ok(())
}

fn check() -> Result<(), Error> {
    let config = Config::new(TargetTriple::host(), BuildKind::Debug);
    let mut session = session::create_session(config)?;

    match check_impl(&mut session) {
        Ok(warn) => error_print::print_warnings(Some(&session), warn),
        Err(errw) => error_print::print_errors_warnings(Some(&session), errw),
    };
    return Ok(());

    fn check_impl(session: &mut Session) -> Result<WarningBuffer, ErrorWarningBuffer> {
        ast_build::parse_all(session, false)?;
        let (_, warn) = hir_lower::check(session)?;
        Ok(warn)
    }
}

fn build(data: CommandBuild) -> Result<(), Error> {
    let config = Config::new(TargetTriple::host(), data.build_kind);
    let mut session = session::create_session(config)?;

    match build_impl(&mut session, data) {
        Ok(()) => (),
        Err(errw) => error_print::print_errors_warnings(Some(&session), errw),
    }
    return Ok(());

    fn build_impl(session: &mut Session, data: CommandBuild) -> Result<(), ErrorWarningBuffer> {
        ast_build::parse_all(session, false)?;
        let (hir, warn) = hir_lower::check(session)?;
        error_print::print_warnings(Some(&session), warn);

        match rock_llvm::build::build(hir, session, data.options) {
            Ok(_) => {}
            Err(error) => error_print::print_errors(Some(&session), error.into()),
        }

        let style = AnsiStyle::new();
        let g = style.out.green_bold;
        let r = style.out.reset;
        println!("  {g}Finished{r} `{}`\n", data.build_kind.as_str());

        Ok(())
    }
}

fn run(data: CommandRun) -> Result<(), Error> {
    let config = Config::new(TargetTriple::host(), data.build_kind);
    let mut session = session::create_session(config)?;

    match run_impl(&mut session, data) {
        Ok(()) => (),
        Err(errw) => error_print::print_errors_warnings(Some(&session), errw),
    }
    return Ok(());

    fn run_impl(session: &mut Session, data: CommandRun) -> Result<(), ErrorWarningBuffer> {
        ast_build::parse_all(session, false)?;
        let (hir, warn) = hir_lower::check(session)?;
        error_print::print_warnings(Some(&session), warn);

        let bin_path = match rock_llvm::build::build(hir, session, data.options) {
            Ok(path) => path,
            Err(error) => {
                error_print::print_errors(Some(&session), error.into());
                return Ok(());
            }
        };
        let style = AnsiStyle::new();
        let g = style.out.green_bold;
        let r = style.out.reset;
        println!("  {g}Finished{r} `{}`", data.build_kind.as_str());

        let run_path = bin_path
            .strip_prefix(&session.curr_work_dir)
            .unwrap_or_else(|_| &bin_path);
        println!("   {g}Running{r} {}\n", run_path.to_string_lossy());

        match rock_llvm::build::run(bin_path, data.args) {
            Ok(()) => {}
            Err(error) => error_print::print_errors(Some(&session), error.into()),
        };
        Ok(())
    }
}

fn fmt() -> Result<(), Error> {
    let config = Config::new(TargetTriple::host(), BuildKind::Debug);
    let mut session = session::create_session(config)?;

    match fmt_impl(&mut session) {
        Ok(()) => (),
        Err(errw) => error_print::print_errors_warnings(Some(&session), errw),
    }
    return Ok(());

    fn fmt_impl(session: &mut Session) -> Result<(), ErrorWarningBuffer> {
        //@only parse syntax trees? how to handle bin op prec errors?
        ast_build::parse_all(session, true)?;

        let root_package = session.graph.package(session::ROOT_PACKAGE_ID);
        for module_id in root_package.module_ids().iter().copied() {
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id());

            let tree = module.tree_expect();
            let formatted = format::format(tree, &file.source);
            fs_env::file_create_or_rewrite(file.path(), &formatted)?;
        }
        Ok(())
    }
}

fn help() {
    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let c = style.out.cyan_bold;
    let r = style.out.reset;

    #[rustfmt::skip]
    println!(r#"{g}
Usage:
  {c}rock <command> [options]

{g}Commands:
  {c}n, new <name>  {r}Create new package
  {c}c, check       {r}Check the program
  {c}b, build       {r}Build the program
  {c}r, run         {r}Build and run the program
  {c}f, fmt         {r}Format current package
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
    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let r = style.out.reset;

    println!("  {g}Rock version:{r} {}\n", rock_core::VERSION);
}

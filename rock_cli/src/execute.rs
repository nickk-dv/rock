use crate::ansi::AnsiStyle;
use crate::command::{Command, CommandBuild, CommandCheck, CommandNew, CommandRun};
use crate::error_print;
use rock_core::config::{BuildKind, Config, TargetTriple};
use rock_core::error::{Error, ErrorWarningBuffer};
use rock_core::errors as err;
use rock_core::hir_lower;
use rock_core::package;
use rock_core::package::manifest::{BuildManifest, Manifest, PackageKind, PackageManifest};
use rock_core::package::semver::Semver;
use rock_core::session::{self, BuildStats, Session};
use rock_core::support::{os, AsStr, Timer};
use rock_core::syntax;
use rock_core::syntax::format;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn command(command: Command) -> Result<(), Error> {
    match command {
        Command::New(data) => new(data),
        Command::Check(data) => check(data),
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
    let cwd = os::dir_get_current_working()?;
    let root_dir = cwd.join(&data.name);
    let src_dir = root_dir.join("src");

    package::verify_name(&data.name)?;
    os::dir_create(&root_dir, true)?;
    os::dir_create(&src_dir, true)?;

    const IMPORT_PRINTF: &str = "import core:libc.{ printf };\n\n";
    match data.kind {
        PackageKind::Bin => {
            let bin_content = format!(
                "{IMPORT_PRINTF}proc main() s32 {{\n    let _ = printf(c\"Bin `{}` works\\n\");\n    return 0;\n}}\n",
                data.name
            );
            os::file_create(&src_dir.join("main.rock"), &bin_content)?
        }
        PackageKind::Lib => {
            let lib_content = format!(
                "{IMPORT_PRINTF}proc test() void {{\n    let _ = printf(c\"Lib `{}` works\\n\");\n}}\n",
                data.name
            );
            os::file_create(&src_dir.join("test.rock"), &lib_content)?;
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
            PackageKind::Lib => {
                BuildManifest { bin_name: None, nodefaultlib: None, lib_paths: None, links: None }
            }
        };

        let manifest = Manifest { package, build, dependencies: BTreeMap::new() };

        let manifest_text = package::manifest_serialize(&manifest)?;
        os::file_create(&root_dir.join("Rock.toml"), &manifest_text)?;
    }

    if !data.no_git {
        os::file_create(&root_dir.join(".gitignore"), "build/\n")?;
        os::file_create(&root_dir.join("README.md"), &format!("# {}\n", data.name))?;
        os::dir_set_current_working(&root_dir)?;

        std::process::Command::new("git")
            .arg("init")
            .stdout(std::process::Stdio::null())
            .status()
            .map_err(|io_error| {
                Error::message(format!("failed to initialize git repository\nreason: {}", io_error))
            })?;
    }

    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let r = style.out.reset;
    println!("  {g}Created{r} {} `{}` package\n", data.kind.full_name(), data.name,);
    Ok(())
}

fn check(data: CommandCheck) -> Result<(), Error> {
    let timer = Timer::start();
    let config = Config::new(TargetTriple::host(), BuildKind::Debug);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    if let Err(()) = check_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn check_impl(session: &mut Session, data: CommandCheck) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session, false)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let _ = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_warnings(Some(session), warn);

        let style = AnsiStyle::new();
        if data.stats {
            print_stats(&style, &session.stats, false);
        }
        print_build_finished(session, &style, &session.stats);
        Ok(())
    }
}

fn build(data: CommandBuild) -> Result<(), Error> {
    let timer = Timer::start();
    let config = Config::new(TargetTriple::host(), data.build_kind);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    let root_manifest = session.graph.package(session.root_id).manifest();
    if root_manifest.package.kind == PackageKind::Lib {
        return Err(err::cmd_cannot_build_lib_package());
    }
    if let Err(()) = build_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn build_impl(session: &mut Session, data: CommandBuild) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session, false)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let hir = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_warnings(Some(session), warn);
        let _ = rock_llvm::build::build(hir, session, data.options)?;

        let style = AnsiStyle::new();
        if data.stats {
            print_stats(&style, &session.stats, true);
        }
        print_build_finished(session, &style, &session.stats);
        Ok(())
    }
}

fn run(data: CommandRun) -> Result<(), Error> {
    let timer = Timer::start();
    let config = Config::new(TargetTriple::host(), data.build_kind);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    let root_manifest = session.graph.package(session.root_id).manifest();
    if root_manifest.package.kind == PackageKind::Lib {
        return Err(err::cmd_cannot_run_lib_package());
    }
    if let Err(()) = run_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn run_impl(session: &mut Session, data: CommandRun) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session, false)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let hir = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_warnings(Some(session), warn);
        let bin_path = rock_llvm::build::build(hir, session, data.options)?;

        let style = AnsiStyle::new();
        if data.stats {
            print_stats(&style, &session.stats, true);
        }
        print_build_finished(session, &style, &session.stats);
        print_build_running(session, &style, &bin_path);
        rock_llvm::build::run(bin_path, data.args)?;
        Ok(())
    }
}

fn print_stats(style: &AnsiStyle, stats: &BuildStats, build: bool) {
    let g = style.out.green_bold;
    let r = style.out.reset;

    println!(" {g}packages:{r} {}", stats.package_count);
    println!("  {g}modules:{r} {}", stats.module_count);
    println!("    {g}lines:{r} {}", stats.line_count);
    println!("   {g}tokens:{r} {}\n", stats.token_count);

    println!("  {g}session:{r} {:.2} ms", stats.session_ms);
    println!("    {g}parse:{r} {:.2} ms", stats.parse_ms);
    println!("    {g}check:{r} {:.2} ms", stats.check_ms);
    if !build {
        return;
    }
    println!("  {g}llvm-ir:{r} {:.2} ms", stats.llvm_ir_ms);
    println!("   {g}object:{r} {:.2} ms", stats.object_ms);
    println!("     {g}link:{r} {:.2} ms\n", stats.link_ms);
}

fn print_build_finished(session: &Session, style: &AnsiStyle, stats: &BuildStats) {
    let build_kind = session.config.build_kind;
    let description = match build_kind {
        BuildKind::Debug => "unoptimized",
        BuildKind::Release => "optimized",
    };
    let g = style.out.green_bold;
    let r = style.out.reset;
    println!(
        "  {g}Finished{r} `{}` ({}) in {:.2} sec",
        build_kind.as_str(),
        description,
        stats.total_secs(),
    );
}

fn print_build_running(session: &Session, style: &AnsiStyle, bin_path: &PathBuf) {
    let run_path = bin_path.strip_prefix(&session.curr_work_dir).unwrap_or_else(|_| bin_path);
    let g = style.out.green_bold;
    let r = style.out.reset;
    println!("   {g}Running{r} {}\n", run_path.to_string_lossy());
}

fn fmt() -> Result<(), Error> {
    let config = Config::new(TargetTriple::host(), BuildKind::Debug);
    let mut session = session::create_session(config)?;

    if let Err(()) = fmt_impl(&mut session) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn fmt_impl(session: &mut Session) -> Result<(), ()> {
        //@only parse syntax trees for root package, instead of full session & parse
        syntax::parse_all(session, true)?;
        let mut cache = format::FormatterCache::new();

        let root_package = session.graph.package(session.root_id);
        for module_id in root_package.module_ids().iter().copied() {
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id());

            let tree = module.tree_expect();
            let formatted = format::format(tree, &file.source, &file.line_ranges, &mut cache);
            os::file_create(&file.path, &formatted)?;
        }
        Ok(())
    }
}

fn help() {
    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let c = style.out.cyan_bold;
    let r = style.out.reset;

    println!(
        "
{g}Usage:
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

  {c}check, build, run
    {c}--stats      {r}Print compilation stats\n",
        PackageKind::Lib.full_name(),
        PackageKind::Bin.full_name()
    );
}

fn version() {
    let style = AnsiStyle::new();
    let g = style.out.green_bold;
    let r = style.out.reset;

    println!("  {g}Rock version:{r} {}\n", rock_core::VERSION);
}

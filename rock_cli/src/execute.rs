use crate::ansi::AnsiStyle;
use crate::command::{Command, CommandBuild, CommandCheck, CommandNew, CommandRun};
use crate::error_print;
use rock_core::codegen;
use rock_core::error::Error;
use rock_core::errors as err;
use rock_core::hir_lower;
use rock_core::session::config::{Build, Config, TargetTriple};
use rock_core::session::manifest::{self, PackageKind};
use rock_core::session::{self, BuildStats, Session};
use rock_core::support::{os, AsStr, Timer};
use rock_core::syntax::{self, format};
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

    manifest::package_verify_name(&data.name)?;
    os::dir_create(&root_dir, true)?;
    os::dir_create(&src_dir, true)?;
    if let PackageKind::Bin = data.kind {
        os::file_create(&src_dir.join("main.rock"), "proc main() void {}\n")?;
    }

    let package = manifest::PackageManifest {
        name: data.name.clone(),
        kind: data.kind,
        version: manifest::Semver::new(0, 1, 0),
        owner: None,
        authors: None,
        description: None,
    };
    let build = match data.kind {
        PackageKind::Bin => manifest::BuildManifest {
            bin_name: Some(data.name.clone()),
            nodefaultlib: None,
            lib_paths: None,
            links: None,
        },
        PackageKind::Lib => manifest::BuildManifest {
            bin_name: None,
            nodefaultlib: None,
            lib_paths: None,
            links: None,
        },
    };
    let manifest = manifest::Manifest { package, build, dependencies: BTreeMap::new() };
    let manifest_text = manifest::serialize(&manifest)?;
    os::file_create(&root_dir.join("Rock.toml"), &manifest_text)?;

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
    let config = Config::new(TargetTriple::host(), Build::Debug);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    if let Err(()) = check_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn check_impl(session: &mut Session, data: CommandCheck) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let _ = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_session_errors(session);

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
    let config = Config::new(TargetTriple::host(), data.build);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    let root_manifest = &session.graph.package(session.root_id).manifest;
    if root_manifest.package.kind == PackageKind::Lib {
        return Err(err::cmd_cannot_build_lib_package());
    }
    if let Err(()) = build_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn build_impl(session: &mut Session, data: CommandBuild) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let hir = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_session_errors(session);
        if let Err(error) = codegen::build(hir, session, data.options) {
            error_print::print_error(Some(session), error);
            return Err(());
        }

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
    let config = Config::new(TargetTriple::host(), data.build);
    let mut session = session::create_session(config)?;
    session.stats.session_ms = timer.measure_ms();

    let root_manifest = &session.graph.package(session.root_id).manifest;
    if root_manifest.package.kind == PackageKind::Lib {
        return Err(err::cmd_cannot_run_lib_package());
    }
    if let Err(()) = run_impl(&mut session, data) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn run_impl(session: &mut Session, data: CommandRun) -> Result<(), ()> {
        let timer = Timer::start();
        syntax::parse_all(session)?;
        session.stats.parse_ms = timer.measure_ms();

        let timer = Timer::start();
        let hir = hir_lower::check(session)?;
        session.stats.check_ms = timer.measure_ms();

        error_print::print_session_errors(session);
        let bin_path = match codegen::build(hir, session, data.options) {
            Ok(path) => path,
            Err(error) => {
                error_print::print_error(Some(session), error);
                return Err(());
            }
        };

        let style = AnsiStyle::new();
        if data.stats {
            print_stats(&style, &session.stats, true);
        }
        print_build_finished(session, &style, &session.stats);

        print_build_running(session, &style, &bin_path);
        if let Err(error) = codegen::run(bin_path, data.args) {
            error_print::print_error(Some(session), error);
            return Err(());
        }
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
    let build = session.config.build;
    let description = match build {
        Build::Debug => "unoptimized",
        Build::Release => "optimized",
    };
    let g = style.out.green_bold;
    let r = style.out.reset;
    println!(
        "  {g}Finished{r} `{}` ({}) in {:.2} sec",
        build.as_str(),
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
    let config = Config::new(TargetTriple::host(), Build::Debug);
    let mut session = session::format_session(config)?;

    if let Err(()) = fmt_impl(&mut session) {
        error_print::print_session_errors(&session);
    }
    return Ok(());

    fn fmt_impl(session: &mut Session) -> Result<(), ()> {
        syntax::parse_all_trees(session)?;
        let mut cache = format::FormatterCache::new();

        for module_id in session.module.ids() {
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id);
            let tree = module.tree_expect();
            let formatted = format::format(tree, &file.source, &file.line_ranges, &mut cache);

            if let Err(error) = os::file_create(&file.path, &formatted) {
                error_print::print_error(Some(session), error);
            }
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

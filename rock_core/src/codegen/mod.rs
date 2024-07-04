mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use crate::error::ErrorComp;
use crate::fs_env;
use crate::hir;
use crate::session::Session;
use crate::timer::Timer;
use inkwell::module;
use inkwell::targets;
use std::path::PathBuf;

struct BuildContext {
    bin_name: String,
    build_kind: BuildKind,
    build_dir: PathBuf,
    executable_path: PathBuf,
}

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}

pub fn codegen(
    hir: hir::Hir,
    session: &Session,
    build_kind: BuildKind,
    emit_llvm: bool,
    args: Option<Vec<String>>,
) -> Result<(), ErrorComp> {
    let timer = Timer::new();
    let timer_1 = Timer::new();
    let context_llvm = inkwell::context::Context::create();
    let (module, machine) = emit_mod::codegen_module(hir, &context_llvm);
    timer_1.stop("llvm module create");

    let timer_2 = Timer::new();
    let context = create_build_context(session, build_kind)?;
    timer_2.stop("create build context & directories");

    let timer_3 = Timer::new();
    module_verify(&context, &module, emit_llvm)?;
    timer_3.stop("module_verify & maybe print to file");

    let timer_4 = Timer::new();
    build_executable(&context, module, machine, session)?;
    timer_4.stop("build_executable total");
    timer.stop("build total");

    run_executable(&context, args)?;
    Ok(())
}

fn create_build_context(
    session: &Session,
    build_kind: BuildKind,
) -> Result<BuildContext, ErrorComp> {
    let mut build_dir = fs_env::dir_get_current_working()?;
    build_dir.push("build");

    fs_env::dir_create(&build_dir, false)?;
    build_dir.push(build_kind.as_str());
    fs_env::dir_create(&build_dir, false)?;

    let root_package = session.package(Session::ROOT_ID);
    let root_manifest = root_package.manifest();
    let bin_name = if let Some(bin_name) = &root_manifest.build.bin_name {
        bin_name.clone()
    } else {
        root_manifest.package.name.clone()
    };

    let mut executable_path = build_dir.clone();
    executable_path.push(&bin_name);
    executable_path.set_extension("exe"); //@assuming windows

    let context = BuildContext {
        bin_name,
        build_kind,
        build_dir,
        executable_path,
    };
    Ok(context)
}

fn module_verify<'ctx>(
    context: &BuildContext,
    module: &module::Module<'ctx>,
    emit_llvm: bool,
) -> Result<(), ErrorComp> {
    let emit_path = context.build_dir.join(format!("{}.ll", context.bin_name));

    if emit_llvm {
        module.print_to_file(&emit_path).unwrap();
    } else {
        fs_env::file_remove(&emit_path, false)?;
    }

    if let Err(error) = module.verify() {
        return Err(ErrorComp::message(format!(
            "internal codegen error: llvm module verify failed\nreason: {}",
            error
        )));
    }
    Ok(())
}

fn build_executable<'ctx>(
    context: &BuildContext,
    module: module::Module<'ctx>,
    machine: targets::TargetMachine,
    session: &Session,
) -> Result<(), ErrorComp> {
    let object_path = context.build_dir.join(format!("{}.o", context.bin_name));

    let timer_1 = Timer::new();
    machine
        .write_to_file(&module, targets::FileType::Object, &object_path)
        .map_err(|error| {
            ErrorComp::message(format!(
                "failed to write llvm module as object file\nreason: {}",
                error
            ))
        })?;
    timer_1.stop("build executable: write object to file");

    let arg_obj = object_path.to_string_lossy().to_string();
    let arg_out = format!("/out:{}", context.executable_path.to_string_lossy());
    let mut args = vec![arg_obj, arg_out];

    //@check if they need to be comma separated instead of being separate
    match context.build_kind {
        BuildKind::Debug => {
            args.push("/opt:ref".into());
            args.push("/opt:noicf".into());
            args.push("/opt:nolbr".into());
        }
        BuildKind::Release => {
            args.push("/opt:ref".into());
            args.push("/opt:icf".into());
            args.push("/opt:lbr".into());
        }
    }

    //@windows only
    if true {
        //sybsystem needs to be specified on windows (console, windows)
        //@only console with `main` entry point is supported, support WinMain when such feature is required 29.05.24
        args.push("/subsystem:console".into());
        // link with C runtime library: libcmt.lib (static), msvcrt.lib (dynamic)
        //@always linking with static C runtime library, support attributes or toml configs 29.05.24
        // to change this if needed, this might be a problem when trying to link C libraries (eg: raylib.lib)
    } else {
        panic!("only windows targets are supported");
    }

    let mut nodefaultlib = false;

    for package_id in session.package_ids() {
        let package = session.package(package_id);
        let manifest = package.manifest();

        match manifest.build.nodefaultlib {
            Some(true) => nodefaultlib = true,
            _ => {}
        }
        if let Some(lib_paths) = &manifest.build.lib_paths {
            for path in lib_paths {
                let lib_path = package.root_dir.join(path);
                args.push(format!("/libpath:{}", lib_path.to_string_lossy()));
            }
        }
        if let Some(links) = &manifest.build.links {
            for link in links {
                args.push(format!("{}", link));
            }
        }
    }

    if !nodefaultlib {
        //@windows only
        args.push("/defaultlib:libcmt.lib".into());
    }

    let timer_2 = Timer::new();
    // lld-link is called system wide, and requires llvm being installed @29.05.24
    // test and use bundled lld-link relative to install path instead
    let _ = std::process::Command::new("lld-link")
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to link object file `{}`\nreason: {}",
                object_path.to_string_lossy(),
                io_error
            ))
        })?;
    timer_2.stop("build executable: lld-link command run");
    fs_env::file_remove(&object_path, false)?;
    Ok(())
}

fn run_executable(context: &BuildContext, args: Option<Vec<String>>) -> Result<(), ErrorComp> {
    let args = match args {
        Some(args) => args,
        None => return Ok(()),
    };

    std::process::Command::new(context.executable_path.as_os_str())
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to run executable `{}`\nreason: {}",
                context.executable_path.to_string_lossy(),
                io_error
            ))
        })?;

    Ok(())
}

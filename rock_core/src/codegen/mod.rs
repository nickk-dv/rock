mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use crate::error::ErrorComp;
use crate::fs_env;
use crate::hir;
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
    bin_name: String,
    build_kind: BuildKind,
    args: Option<Vec<String>>,
) -> Result<(), ErrorComp> {
    let context_llvm = inkwell::context::Context::create();
    let (module, machine) = emit_mod::codegen_module(hir, &context_llvm)?;
    let context = create_build_context(bin_name, build_kind)?;
    build_executable(&context, module, machine)?;
    run_executable(&context, args)?;
    Ok(())
}

fn create_build_context(
    bin_name: String,
    build_kind: BuildKind,
) -> Result<BuildContext, ErrorComp> {
    let mut build_dir = fs_env::dir_get_current_working()?;
    build_dir.push("build");
    fs_env::dir_create(&build_dir, false)?;
    build_dir.push(build_kind.as_str());
    fs_env::dir_create(&build_dir, false)?;

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

fn build_executable<'ctx>(
    context: &BuildContext,
    module: module::Module<'ctx>,
    machine: targets::TargetMachine,
) -> Result<(), ErrorComp> {
    let object_path = context.build_dir.join(format!("{}.o", context.bin_name));
    machine
        .write_to_file(&module, targets::FileType::Object, &object_path)
        .map_err(|error| {
            ErrorComp::message(format!(
                "failed to write llvm module as object file\nreason: {}",
                error
            ))
        })?;

    let arg_obj = object_path.to_string_lossy().to_string();
    let arg_out = format!("/out:{}", context.executable_path.to_string_lossy());
    let mut args = vec![arg_obj, arg_out];

    //@check if they need to be comma separated instead of being separate
    match context.build_kind {
        BuildKind::Debug => {
            args.push("/opt:noref".into());
            args.push("/opt:noicf".into());
            args.push("/opt:nolbr".into());
        }
        BuildKind::Release => {
            args.push("/opt:ref".into());
            args.push("/opt:icf".into());
            args.push("/opt:lbr".into());
        }
    }

    //@assuming windows
    if true {
        //sybsystem needs to be specified on windows (console, windows)
        //@only console with `main` entry point is supported, support WinMain when such feature is required 29.05.24
        args.push("/subsystem:console".into());
        // link with C runtime library: libcmt.lib (static), msvcrt.lib (dynamic)
        //@always linking with static C runtime library, support attributes or toml configs 29.05.24
        // to change this if needed, this might be a problem when trying to link C libraries (eg: raylib.lib)
        args.push("/defaultlib:libcmt.lib".into());
    } else {
        panic!("only windows targets are supported");
    }

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
    fs_env::file_remove(&object_path)?;
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

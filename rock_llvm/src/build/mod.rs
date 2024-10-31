mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use rock_core::config::{BuildKind, TargetOS};
use rock_core::error::Error;
use rock_core::fs_env;
use rock_core::hir;
use rock_core::session::{self, Session};
use rock_core::support::{AsStr, Timer};
use std::path::PathBuf;

pub struct BuildOptions {
    pub emit_llvm: bool,
}

pub fn build(
    hir: hir::Hir,
    session: &mut Session,
    options: BuildOptions,
) -> Result<PathBuf, Error> {
    let timer = Timer::start();
    let config = session.config;
    let (target, module) = emit_mod::codegen_module(
        hir,
        config.target,
        &session.intern_lit,
        &session.intern_name,
    );
    session.stats.llvm_ir_ms = timer.measure_ms();

    let mut build_path = session.curr_work_dir.join("build");
    fs_env::dir_create(&build_path, false)?;
    build_path.push(config.build_kind.as_str());
    fs_env::dir_create(&build_path, false)?;

    let manifest = session.graph.package(session::ROOT_PACKAGE_ID).manifest();
    let bin_name = match &manifest.build.bin_name {
        Some(name) => name.as_str(),
        None => manifest.package.name.as_str(),
    };
    let bin_path = match session.config.target_os {
        TargetOS::Windows => build_path.join(format!("{bin_name}.exe")),
        TargetOS::Linux => build_path.join(&bin_name),
    };

    if options.emit_llvm {
        fs_env::file_create_or_rewrite(
            &build_path.join(format!("{bin_name}.ll")),
            &module.to_string(),
        )?;
    }

    //@fix module verify errors & return ErrorComp
    let verify_result = module.verify();
    let _ = verify_result.map_err(|e| eprintln!("llvm verify module failed:\n{}", e));

    let object_path = build_path.join(format!("{bin_name}.o"));
    let timer = Timer::start();
    let object_result = module.emit_to_file(&target, object_path.to_str().unwrap());
    session.stats.object_ms = timer.measure_ms();
    object_result.map_err(|e| Error::message(format!("llvm emit object failed:\n{}", e)))?;

    let arg_obj = object_path.to_string_lossy().to_string();
    let mut args: Vec<String> = vec![arg_obj];

    match config.target_os {
        TargetOS::Windows => {
            args.push(format!("/out:{}", bin_path.to_string_lossy()));
        }
        TargetOS::Linux => {
            args.push("-o".into());
            args.push(bin_path.to_string_lossy().into());
        }
    }

    match config.target_os {
        TargetOS::Windows => {
            args.push("/entry:main".into());
            args.push("kernel32.lib".into());
            //args.push("/defaultlib:libcmt.lib".into());
        }
        TargetOS::Linux => args.push("-lc".into()),
    }

    match config.build_kind {
        BuildKind::Debug => match config.target_os {
            TargetOS::Windows => args.push("/debug".into()),
            TargetOS::Linux => {} //@enable debug info
        },
        BuildKind::Release => match config.target_os {
            TargetOS::Windows => {
                args.push("/opt:ref".into());
                args.push("/opt:icf".into());
                args.push("/opt:lbr".into());
            }
            TargetOS::Linux => {
                args.push("--gc-sections".into());
                args.push("--icf=all".into());
                args.push("--strip-debug".into());
            }
        },
    }

    //@when cross compiling linux & macos linkers will have .exe, lld-link wont.
    let install_bin = fs_env::current_exe_path()?.join("bin");
    let linker_path = match config.target_os {
        TargetOS::Windows => install_bin.join("lld-link.exe"),
        TargetOS::Linux => install_bin.join("ld.lld"),
    };

    //@use different command api, capture outputs + error
    let timer = Timer::start();
    let status = std::process::Command::new(linker_path)
        .args(args)
        .status()
        .map_err(|io_error| Error::message(format!("failed to link program:\n{}", io_error)))?;
    session.stats.link_ms = timer.measure_ms();
    Ok(bin_path)
}

pub fn run(bin_path: PathBuf, args: Vec<String>) -> Result<(), Error> {
    let _ = std::process::Command::new(&bin_path)
        .args(args)
        .status()
        .map_err(|io_error| {
            Error::message(format!(
                "failed to run executable `{}`\nreason: {}",
                bin_path.to_string_lossy(),
                io_error
            ))
        })?;
    Ok(())
}

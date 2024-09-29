mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use rock_core::config::{BuildKind, TargetOS, TargetTriple};
use rock_core::error::Error;
use rock_core::fs_env;
use rock_core::hir;
use rock_core::session::Session;
use rock_core::support::AsStr;
use std::path::PathBuf;

pub struct BuildOptions {
    pub kind: BuildKind,
    pub emit_llvm: bool,
}

pub fn build(hir: hir::Hir, session: &Session, options: BuildOptions) -> Result<PathBuf, Error> {
    let triple = TargetTriple::host();
    let target_os = triple.os();
    let (target, module) =
        emit_mod::codegen_module(hir, triple, &session.intern_lit, &session.intern_name);

    let cwd = fs_env::dir_get_current_working()?;
    let mut build_path = cwd.join("build");
    fs_env::dir_create(&build_path, false)?;
    build_path.push(options.kind.as_str());
    fs_env::dir_create(&build_path, false)?;

    let manifest = session.pkg_storage.package(Session::ROOT_ID).manifest();
    let bin_name = match &manifest.build.bin_name {
        Some(name) => name.clone(),
        None => manifest.package.name.clone(),
    };
    let binary_path = match target_os {
        TargetOS::Windows => build_path.join(format!("{bin_name}.exe")),
        TargetOS::Linux => build_path.join(&bin_name),
        TargetOS::Macos => build_path.join(&bin_name),
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
    let object_result = module.emit_to_file(&target, object_path.to_str().unwrap());
    object_result.map_err(|e| Error::message(format!("llvm emit object failed:\n{}", e)))?;

    let arg_obj = object_path.to_string_lossy().to_string();
    let mut args: Vec<String> = vec![arg_obj];

    match target_os {
        TargetOS::Windows => {
            args.push(format!("/out:{}", binary_path.to_string_lossy()));
        }
        TargetOS::Linux | TargetOS::Macos => {
            args.push("-o".into());
            args.push(binary_path.to_string_lossy().into());
        }
    }

    match target_os {
        TargetOS::Windows => args.push("/defaultlib:libcmt.lib".into()),
        TargetOS::Linux | TargetOS::Macos => args.push("-lc".into()),
    }

    match options.kind {
        BuildKind::Debug => match target_os {
            TargetOS::Windows => args.push("/debug".into()),
            TargetOS::Linux => {} //@enable debug info
            TargetOS::Macos => {} //@enable debug info
        },
        BuildKind::Release => match target_os {
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
            TargetOS::Macos => {
                args.push("-dead_strip".into());
                args.push("--icf=all".into());
                args.push("-S".into())
            }
        },
    }

    //@when cross compiling linux & macos linkers will have .exe, lld-link wont.
    let install_bin = fs_env::current_exe_path()?.join("bin");
    let linker_path = match target_os {
        TargetOS::Windows => install_bin.join("lld-link.exe"),
        TargetOS::Linux => install_bin.join("ld.lld"),
        TargetOS::Macos => install_bin.join("ld64.lld"),
    };

    //@use different command api, capture outputs + error
    let status = std::process::Command::new(linker_path)
        .args(args)
        .status()
        .map_err(|io_error| Error::message(format!("failed to link program:\n{}", io_error)))?;
    Ok(binary_path)
}

pub fn run(binary_path: PathBuf, args: Vec<String>) -> Result<(), Error> {
    let _ = std::process::Command::new(&binary_path)
        .args(args)
        .status()
        .map_err(|io_error| {
            Error::message(format!(
                "failed to run executable `{}`\nreason: {}",
                binary_path.to_string_lossy(),
                io_error
            ))
        })?;
    Ok(())
}

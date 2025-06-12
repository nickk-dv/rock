mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;
#[allow(unsafe_code)]
mod llvm;

use crate::error::{Error, ErrorSink};
use crate::errors as err;
use crate::hir;
use crate::session::config::{Build, TargetOS};
use crate::session::Session;
use crate::support::os;
use crate::support::{AsStr, Timer};
use std::path::PathBuf;

pub struct BuildOptions {
    pub emit_llvm: bool,
}

pub fn run(bin_path: PathBuf, args: Vec<String>) -> Result<(), Error> {
    if let Err(error) = std::process::Command::new(&bin_path).args(args).status() {
        Err(err::backend_run_command_failed(error.to_string(), &bin_path))
    } else {
        Ok(())
    }
}

pub fn build(hir: hir::Hir, session: &mut Session, options: BuildOptions) -> Result<PathBuf, ()> {
    let timer = Timer::start();
    let (target, module) = emit_mod::codegen_module(hir, session)?;
    session.stats.llvm_ir_ms = timer.measure_ms();

    match build_impl(session, options, target, module) {
        Ok(bin_path) => Ok(bin_path),
        Err(error) => {
            session.errors.error(error);
            Err(())
        }
    }
}

pub fn build_impl(
    session: &mut Session,
    options: BuildOptions,
    target: llvm::IRTarget,
    mut module: llvm::IRModule,
) -> Result<PathBuf, Error> {
    let config = session.config;

    // setup build output directories
    let mut build_path = session.curr_work_dir.join("build");
    os::dir_create(&build_path, false)?;
    build_path.push(config.build.as_str());
    os::dir_create(&build_path, false)?;

    let root_manifest = &session.graph.package(session.root_id).manifest;
    let bin_name = match &root_manifest.build.bin_name {
        Some(name) => name.as_str(),
        None => root_manifest.package.name.as_str(),
    };
    let bin_path = match session.config.target_os {
        TargetOS::Windows => build_path.join(format!("{bin_name}.exe")),
        TargetOS::Linux => build_path.join(bin_name),
    };

    // emit-llvm & verify module
    if options.emit_llvm {
        os::file_create(&build_path.join(format!("{bin_name}.ll")), &module.to_string())?;
    }
    if let Err(error) = module.verify() {
        return Err(err::backend_module_verify_failed(error));
    }

    // optimize
    if session.config.build == Build::Release {
        let timer = Timer::start();
        module.run_optimization_passes(&target, "default<O3>");
        session.stats.llvm_opt_ms = timer.measure_ms();
        if options.emit_llvm {
            os::file_create(&build_path.join(format!("{bin_name}_opt.ll")), &module.to_string())?;
        }
    }

    // emit object
    let timer = Timer::start();
    let object_path = build_path.join(format!("{bin_name}.o"));
    if let Err(error) = module.emit_to_file(&target, object_path.to_str().unwrap()) {
        return Err(err::backend_emit_object_failed(error));
    };
    session.stats.object_ms = timer.measure_ms();

    // setup args & link executable
    let mut args: Vec<String> = vec![object_path.to_string_lossy().to_string()];

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
        TargetOS::Windows => args.push("/opt:ref".into()),
        TargetOS::Linux => args.push("--gc-sections".into()),
    }
    match config.build {
        Build::Debug => match config.target_os {
            TargetOS::Windows => args.push("/debug".into()),
            TargetOS::Linux => args.push("-g".into()),
        },
        Build::Release => match config.target_os {
            TargetOS::Windows => args.push("/opt:icf".into()),
            TargetOS::Linux => {
                args.push("--icf=all".into());
                args.push("--strip-debug".into());
            }
        },
    }

    let nodefaultlib = session
        .graph
        .all_packages()
        .values()
        .any(|pkg| pkg.manifest.build.nodefaultlib == Some(true));
    if !nodefaultlib {
        match config.target_os {
            TargetOS::Windows => args.push("/defaultlib:libcmt.lib".into()),
            TargetOS::Linux => args.push("-lc".into()),
        }
    }

    for package in session.graph.all_packages().values() {
        let manifest = &package.manifest;

        if let Some(lib_paths) = manifest.build.lib_paths.as_ref() {
            for lib_path in lib_paths {
                let lib_path = package.root_dir.join(lib_path);
                match config.target_os {
                    TargetOS::Windows => {
                        args.push(format!("/libpath:{}", lib_path.to_string_lossy()))
                    }
                    TargetOS::Linux => args.push(format!("-L{}", lib_path.to_string_lossy())),
                }
            }
        }
        if let Some(links) = manifest.build.links.as_ref() {
            for link in links {
                match config.target_os {
                    TargetOS::Windows => args.push(format!("/defaultlib:{link}")),
                    TargetOS::Linux => args.push(format!("-l{link}")),
                }
            }
        }
    }

    let timer = Timer::start();
    let install_bin = os::current_exe_path()?.join("bin");
    let linker_path = match config.target_os {
        TargetOS::Windows => install_bin.join("lld-link.exe"),
        TargetOS::Linux => install_bin.join("ld.lld"),
    };
    match std::process::Command::new(&linker_path).args(&args).output() {
        Ok(output) => {
            if !output.status.success() {
                return Err(err::backend_link_failed(output, args));
            }
        }
        Err(error) => {
            return Err(err::backend_link_command_failed(error.to_string(), &linker_path));
        }
    };
    session.stats.link_ms = timer.measure_ms();

    Ok(bin_path)
}

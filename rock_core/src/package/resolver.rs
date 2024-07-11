use super::semver::Semver;
use crate::error::ErrorComp;
use crate::fs_env;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

//@check status codes or Command's or not?
pub fn resolve_dependencies(dependencies: &BTreeMap<String, Semver>) -> Result<(), ErrorComp> {
    let cwd = fs_env::dir_get_current_working()?;
    let root = fs_env::current_exe_path()?;
    let registry_dir = root.join("registry");
    let index_dir = registry_dir.join("rock-index");
    let cache_dir = registry_dir.join("cache");

    fs_env::dir_create(&registry_dir, false)?;
    fs_env::dir_create(&cache_dir, false)?;

    //@on errors still set back correct cwd!
    if !index_dir.exists() {
        fs_env::dir_set_current_working(&registry_dir)?;
        const INDEX_LINK: &str = "https://github.com/nickk-dv/rock-index";
        let _ = Command::new("git")
            .args(["clone", INDEX_LINK])
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!(
                    "failed command: git clone {}\nreason: {}",
                    INDEX_LINK, io_error
                ))
            })?;
        fs_env::dir_set_current_working(&cwd)?;
    } else {
        fs_env::dir_set_current_working(&index_dir)?;
        let _ = Command::new("git")
            .args(["pull"])
            .status()
            .map_err(|io_error| {
                ErrorComp::message(format!("failed command: git pull\nreason: {}", io_error))
            })?;
        fs_env::dir_set_current_working(&cwd)?;
    }

    for (package_name, &version) in dependencies {
        let index_manifest_path = registry_package_path(&index_dir, package_name);

        //@wip error, dep of which package etc...
        if !index_manifest_path.exists() {
            return Err(ErrorComp::message(format!(
                "package `{}` is not found in rock-index",
                package_name
            )));
        }

        //@assuming single toml file, change to line based json manifest
        let index_manifest = fs_env::file_read_to_string(&index_manifest_path)?;
        let index_manifest =
            super::index_manifest_deserialize(index_manifest, &index_manifest_path)?;
    }

    Ok(())
}

fn registry_package_path(index_dir: &PathBuf, package_name: &str) -> PathBuf {
    let mut path = index_dir.clone();

    match package_name.len() {
        0 => unreachable!(),
        1 => {
            path.push("1");
            path.push(package_name);
        }
        2 => {
            path.push("2");
            path.push(package_name);
        }
        3 => {
            let first = &package_name[0..1];
            path.push("3");
            path.push(first);
            path.push(package_name);
        }
        4.. => {
            let first_two = &package_name[0..2];
            let second_two = &package_name[2..4];
            path.push(first_two);
            path.push(second_two);
            path.push(package_name);
        }
    };

    path
}

use super::semver::Semver;
use crate::error::Error;
use crate::fs_env;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Command;

//@check status codes or Command's or not?
pub fn resolve_dependencies(dependencies: &BTreeMap<String, Semver>) -> Result<(), Error> {
    let cwd = fs_env::dir_get_current_working()?;
    let root = fs_env::current_exe_path()?;
    let registry_dir = root.join("registry");
    let index_dir = registry_dir.join("rock-index");
    let cache_dir = registry_dir.join("cache");

    fs_env::dir_create(&registry_dir, false)?;
    fs_env::dir_create(&cache_dir, false)?;

    //@on errors still set back correct cwd!
    //@dont git pull each time, takes time
    if !index_dir.exists() {
        fs_env::dir_set_current_working(&registry_dir)?;
        const INDEX_LINK: &str = "https://github.com/nickk-dv/rock-index";
        let _ = Command::new("git")
            .args(["clone", INDEX_LINK])
            .status()
            .map_err(|io_error| {
                Error::message(format!(
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
                Error::message(format!("failed command: git pull\nreason: {}", io_error))
            })?;
        fs_env::dir_set_current_working(&cwd)?;
    }

    let mut selected_packages: HashMap<String, Semver> = HashMap::new();

    for (package_name, &version) in dependencies {
        let manifest_path = registry_package_path(&index_dir, package_name);

        if !manifest_path.exists() {
            //@wip error, dep of which package etc...
            return Err(Error::message(format!(
                "package `{}` is not found in rock-index",
                package_name
            )));
        }

        let index_manifest = fs_env::file_read_to_string(&manifest_path)?;
        let index_manifests = super::index_manifest_deserialize(index_manifest, &manifest_path)?;

        let mut selected_manifest = None;
        for manifest in index_manifests {
            if version == manifest.version {
                selected_manifest = Some(manifest);
                break;
            }
        }

        if let Some(selected) = selected_manifest {
            if let Some(&version) = selected_packages.get(package_name) {
                //@conflict or newer version not handled
                return Err(Error::message(format!(
                    "package `{}` version conflict {} and {}\nconflict handling not implemented",
                    package_name, version, selected.version,
                )));
            } else {
                selected_packages.insert(package_name.clone(), selected.version);
            }
        } else {
            return Err(Error::message(format!(
                "dependency package `{}` version {} was not found",
                package_name, version,
            )));
        }
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

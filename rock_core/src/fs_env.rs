use crate::error::ErrorComp;
use std::path::PathBuf;

pub fn current_exe_path() -> Result<PathBuf, ErrorComp> {
    std::env::current_exe().map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to get current executable path\nreason: {}",
            io_error
        ))
    })
}

pub fn dir_get_current_working() -> Result<PathBuf, ErrorComp> {
    std::env::current_dir().map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to get working directory\nreason: {}",
            io_error
        ))
    })
}

pub fn dir_set_current_working(path: &PathBuf) -> Result<(), ErrorComp> {
    std::env::set_current_dir(path).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to set working directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn dir_create(path: &PathBuf, force: bool) -> Result<(), ErrorComp> {
    if !force && path.exists() {
        return Ok(());
    }
    std::fs::create_dir(path).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to create directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn dir_read(path: &PathBuf) -> Result<std::fs::ReadDir, ErrorComp> {
    std::fs::read_dir(path).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to read directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_read_to_string(path: &PathBuf) -> Result<String, ErrorComp> {
    std::fs::read_to_string(path).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to read file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_create_or_rewrite(path: &PathBuf, text: &str) -> Result<(), ErrorComp> {
    std::fs::write(path, text).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to create file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_remove(path: &PathBuf) -> Result<(), ErrorComp> {
    std::fs::remove_file(path).map_err(|io_error| {
        ErrorComp::message(format!(
            "failed to remove file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

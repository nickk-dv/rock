use crate::error::Error;
use std::{io::Read, path::Path, path::PathBuf};

pub fn current_exe_path() -> Result<PathBuf, Error> {
    let mut current_exe = std::env::current_exe().map_err(|io_error| {
        Error::message(format!(
            "failed to get current executable path\nreason: {}",
            io_error
        ))
    })?;
    current_exe.pop();
    Ok(current_exe)
}

pub fn dir_get_current_working() -> Result<PathBuf, Error> {
    std::env::current_dir().map_err(|io_error| {
        Error::message(format!(
            "failed to get working directory\nreason: {}",
            io_error
        ))
    })
}

pub fn dir_set_current_working(path: &PathBuf) -> Result<(), Error> {
    std::env::set_current_dir(path).map_err(|io_error| {
        Error::message(format!(
            "failed to set working directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn dir_create(path: &PathBuf, force: bool) -> Result<(), Error> {
    if !force && path.exists() {
        return Ok(());
    }
    std::fs::create_dir(path).map_err(|io_error| {
        Error::message(format!(
            "failed to create directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn dir_read(path: &PathBuf) -> Result<std::fs::ReadDir, Error> {
    std::fs::read_dir(path).map_err(|io_error| {
        Error::message(format!(
            "failed to read directory: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn dir_entry_validate(
    origin: &PathBuf,
    entry_result: Result<std::fs::DirEntry, std::io::Error>,
) -> Result<std::fs::DirEntry, Error> {
    entry_result.map_err(|io_error| {
        Error::message(format!(
            "failed to read directory entry in: `{}`\nreason: {}",
            origin.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_read_to_string(path: &PathBuf) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|io_error| {
        Error::message(format!(
            "failed to read file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_read_to_string_sentinel(path: &PathBuf) -> Result<String, Error> {
    fn inner(path: &Path) -> std::io::Result<String> {
        let mut file = std::fs::File::open(path)?;
        let size = file.metadata().map(|m| m.len() as usize).ok();
        let mut string = String::new();
        string.try_reserve_exact(size.unwrap_or(0) + 2)?;
        file.read_to_string(&mut string)?;
        string.push_str("\0\0");
        Ok(string)
    }
    inner(path.as_ref()).map_err(|io_error| {
        Error::message(format!(
            "failed to read file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_create_or_rewrite(path: &PathBuf, text: &str) -> Result<(), Error> {
    std::fs::write(path, text).map_err(|io_error| {
        Error::message(format!(
            "failed to create file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn file_remove(path: &PathBuf, force: bool) -> Result<(), Error> {
    if !force && !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|io_error| {
        Error::message(format!(
            "failed to remove file: `{}`\nreason: {}",
            path.to_string_lossy(),
            io_error
        ))
    })
}

pub fn filename_stem(path: &PathBuf) -> Result<&str, Error> {
    let file_stem = path.file_stem().ok_or(Error::message(format!(
        "failed to get filename from: `{}`",
        path.to_string_lossy(),
    )))?;
    file_stem.to_str().ok_or(Error::message(format!(
        "filename is not valid utf-8: `{}`",
        file_stem.to_string_lossy()
    )))
}

pub fn file_extension(path: &PathBuf) -> Option<&str> {
    let extension = path.extension()?;
    extension.to_str()
}

pub fn symlink_forbid(path: &PathBuf) -> Result<(), Error> {
    if path.is_symlink() {
        return Err(Error::message(format!(
            "symbol links are not supported: `{}`",
            path.to_string_lossy()
        )));
    };
    Ok(())
}

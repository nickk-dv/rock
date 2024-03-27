use crate::error::ErrorComp;
use crate::text::{self, TextRange};
use std::path::PathBuf;

pub struct Session {
    cwd: PathBuf,
    files: Vec<File>,
    package: PackageData,
}

pub struct PackageData {
    pub name: String,
    pub is_binary: bool,
}

pub struct File {
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
}

#[derive(Copy, Clone)]
pub struct FileID(u32);

impl Session {
    pub fn new() -> Result<Session, Vec<ErrorComp>> {
        create_session().map_err(|error| vec![error])
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }
    pub fn file(&self, id: FileID) -> &File {
        &self.files[id.index()]
    }
    pub fn file_ids(&self) -> impl Iterator<Item = FileID> {
        (0..self.files.len()).map(FileID::new)
    }
    pub fn package(&self) -> &PackageData {
        &self.package
    }
}

impl FileID {
    fn new(index: usize) -> FileID {
        FileID(index as u32)
    }
    fn index(self) -> usize {
        self.0 as usize
    }
}

//@general display paths as relative to src folder?
// both in errors here, and diagnostic cli formats
// e.g: src/main.rock or ./src/main.rock
fn create_session() -> Result<Session, ErrorComp> {
    let cwd = std::env::current_dir().map_err(|io_error| {
        ErrorComp::error(format!(
            "failed to get current working directory, reason: {}",
            io_error
        ))
    })?;

    let os_name = cwd
        .file_name()
        .ok_or_else(|| ErrorComp::error("failed to get current working directory name"))?;
    let name = os_name
        .to_str()
        .ok_or_else(|| ErrorComp::error("current working directory name is not valid utf-8"))?
        .to_string();

    let src_dir = cwd.join("src");
    let src_bin = src_dir.join("main.rock");
    let src_lib = src_dir.join("lib.rock");

    let is_binary = match (src_bin.exists(), src_lib.exists()) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => {
            return Err(ErrorComp::error(format!(
                "could not find `{}` or `{}`",
                src_bin.to_string_lossy(),
                src_lib.to_string_lossy(),
            )));
        }
        (true, true) => {
            return Err(ErrorComp::error(format!(
                "could not determine package kind\nonly `{}` or `{}` can exist at the same time",
                src_bin.to_string_lossy(),
                src_lib.to_string_lossy(),
            )));
        }
    };
    let package = PackageData { name, is_binary };

    let mut files = Vec::new();
    let mut dir_visits = vec![src_dir];
    let mut source_paths = Vec::new();

    while let Some(dir) = dir_visits.pop() {
        let read_dir = std::fs::read_dir(&dir).map_err(|io_error| {
            ErrorComp::error(format!(
                "failed to read directory: `{}`, reason: {}",
                dir.to_string_lossy(),
                io_error
            ))
        })?;
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "rock" {
                source_paths.push(path);
            } else if path.is_dir() {
                dir_visits.push(path);
            }
        }
    }

    for path in source_paths {
        let source = std::fs::read_to_string(&path).map_err(|io_error| {
            ErrorComp::error(format!(
                "failed to read file: `{}`, reason: {}",
                path.to_string_lossy(),
                io_error
            ))
        })?;
        let line_ranges = text::find_line_ranges(&source);
        files.push(File {
            path,
            source,
            line_ranges,
        });
    }

    Ok(Session {
        cwd,
        files,
        package,
    })
}

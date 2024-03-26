use crate::error::ErrorComp;
use crate::intern::InternPool;
use crate::text::{self, TextRange};
use std::path::PathBuf;

pub struct Session {
    cwd: PathBuf,
    files: Vec<File>,
    intern: InternPool,
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
    pub fn new() -> Result<Session, ErrorComp> {
        create_session()
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
    pub fn intern(&self) -> &InternPool {
        &self.intern
    }
    pub fn intern_mut(&mut self) -> &mut InternPool {
        &mut self.intern
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

fn create_session() -> Result<Session, ErrorComp> {
    let cwd = std::env::current_dir().map_err(|io_error| {
        ErrorComp::error(format!(
            "failed to read current working directory, reason: {}",
            io_error
        ))
    })?;

    let src_dir = cwd.join("src");
    if !src_dir.exists() {
        return Err(ErrorComp::error(format!(
            "could not find `src` directory, source files must be located in `{}`",
            src_dir.to_string_lossy()
        )));
    }

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

    let mut files = Vec::new();

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
        intern: InternPool::new(),
        package: PackageData {
            name: "example".into(), //@infer from name of the forlder
            is_binary: true, //@infer from exitance of  src/main or src/lib and report if both are missing, remove current `src` missing error
        },
    })
}

use crate::text::{self, TextRange};
use std::path::PathBuf;

pub struct Vfs {
    files: Vec<File>,
}

pub struct File {
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
}

#[derive(Copy, Clone)]
pub struct FileID(u32);

impl Vfs {
    pub fn new() -> Vfs {
        Vfs { files: Vec::new() }
    }

    pub fn file(&self, id: FileID) -> &File {
        self.files.get(id.index()).unwrap()
    }

    pub fn register_file(&mut self, path: PathBuf) -> FileID {
        let id = FileID::new(self.files.len());
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => panic!("vfs file read failed: {}", error),
        };
        let line_ranges = text::find_line_ranges(&source);
        self.files.push(File {
            path,
            source,
            line_ranges,
        });
        id
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

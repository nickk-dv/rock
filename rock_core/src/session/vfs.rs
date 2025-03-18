use crate::text::{self, TextRange};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Vfs {
    files: Vec<FileData>,
    paths: HashMap<PathBuf, FileID>,
}

crate::define_id!(pub FileID);
pub struct FileData {
    pub version: u32,
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
}

impl Vfs {
    pub fn new(cap: usize) -> Vfs {
        Vfs { files: Vec::with_capacity(cap), paths: HashMap::with_capacity(cap) }
    }

    #[inline]
    pub fn file(&self, file_id: FileID) -> &FileData {
        &self.files[file_id.index()]
    }
    #[inline]
    pub fn file_mut(&mut self, file_id: FileID) -> &mut FileData {
        &mut self.files[file_id.index()]
    }
    #[inline]
    pub fn path_to_file_id<P: AsRef<Path>>(&self, p: P) -> Option<FileID> {
        self.paths.get(p.as_ref()).copied()
    }

    #[must_use]
    pub fn open<P: AsRef<Path>>(&mut self, path: P, source: String) -> FileID {
        if let Some(file_id) = self.paths.get(path.as_ref()).copied() {
            //@what to do with version?
            let file = self.file_mut(file_id);
            text::find_line_ranges(&mut file.line_ranges, &source);
            file.source = source;
            file_id
        } else {
            let mut line_ranges = Vec::with_capacity(source.lines().count());
            text::find_line_ranges(&mut line_ranges, &source);
            let file =
                FileData { version: 1, path: path.as_ref().to_path_buf(), source, line_ranges };
            assert!(file.path.is_absolute());

            let file_id = FileID(self.files.len() as u32);
            self.files.push(file);
            self.paths.insert(path.as_ref().to_path_buf(), file_id);
            file_id
        }
    }

    pub fn unload(&mut self, file_id: FileID) {
        let file = self.file_mut(file_id);
        file.source = String::new();
        file.line_ranges = Vec::new();
    }
}

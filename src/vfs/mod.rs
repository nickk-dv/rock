use crate::text_range::{TextOffset, TextRange};
use std::path::PathBuf;

struct Vfs {
    files: Vec<File>,
}

struct File {
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
        let line_ranges = compute_line_ranges(&source);
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

fn compute_line_ranges(text: &str) -> Vec<TextRange> {
    let mut ranges = Vec::new();
    let mut range = TextRange::empty_at(0.into());
    for c in text.chars() {
        let size: TextOffset = (c.len_utf8() as u32).into();
        range.extend_by(size);
        if c == '\n' {
            ranges.push(range);
            range = TextRange::empty_at(range.end());
        }
    }
    if range.len() > 0 {
        ranges.push(range);
    }
    ranges
}

//! Source spans and a file registry.
//!
//! Spans are pure byte-offset ranges into a file identified by [`FileId`].
//! The [`FileRegistry`] owns file paths and sources; diagnostic rendering
//! resolves a [`Span`] back to the source via the registry.

use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FileId(pub u32);

impl FileId {
    pub const SYNTHETIC: FileId = FileId(u32::MAX);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// A zero-width span used for synthesised AST nodes.
    pub const fn synthetic() -> Self {
        Self { file: FileId::SYNTHETIC, start: 0, end: 0 }
    }

    pub fn is_synthetic(&self) -> bool {
        self.file == FileId::SYNTHETIC
    }

    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge two spans; both must be from the same file.
    /// Returns the leftmost-to-rightmost extent.
    pub fn merge(self, other: Span) -> Span {
        debug_assert_eq!(self.file, other.file, "cannot merge spans from different files");
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub source: Arc<str>,
}

/// A registry of source files, indexed by [`FileId`].
#[derive(Debug, Default, Clone)]
pub struct FileRegistry {
    entries: Vec<FileEntry>,
}

impl FileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, path: PathBuf, source: impl Into<Arc<str>>) -> FileId {
        let id = FileId(u32::try_from(self.entries.len()).expect("too many files"));
        self.entries.push(FileEntry { path, source: source.into() });
        id
    }

    pub fn get(&self, file: FileId) -> Option<&FileEntry> {
        if file == FileId::SYNTHETIC {
            return None;
        }
        self.entries.get(file.0 as usize)
    }

    pub fn source(&self, file: FileId) -> Option<&str> {
        self.get(file).map(|e| e.source.as_ref())
    }

    pub fn path(&self, file: FileId) -> Option<&std::path::Path> {
        self.get(file).map(|e| e.path.as_path())
    }

    pub fn slice(&self, span: Span) -> Option<&str> {
        let source = self.source(span.file)?;
        source.get(span.start as usize..span.end as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &FileEntry)> {
        self.entries.iter().enumerate().map(|(i, e)| (FileId(i as u32), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_merge_extends_extent() {
        let f = FileId(0);
        let a = Span::new(f, 10, 15);
        let b = Span::new(f, 5, 12);
        assert_eq!(a.merge(b), Span::new(f, 5, 15));
    }

    #[test]
    fn synthetic_span_round_trip() {
        let s = Span::synthetic();
        assert!(s.is_synthetic());
        assert!(s.is_empty());
    }

    #[test]
    fn registry_round_trip() {
        let mut reg = FileRegistry::new();
        let id = reg.add(PathBuf::from("x.surf"), "module X");
        assert_eq!(reg.source(id), Some("module X"));
        let span = Span::new(id, 0, 6);
        assert_eq!(reg.slice(span), Some("module"));
    }

    #[test]
    fn registry_synthetic_file_is_none() {
        let reg = FileRegistry::new();
        assert!(reg.get(FileId::SYNTHETIC).is_none());
    }

    #[test]
    #[should_panic(expected = "different files")]
    fn merge_across_files_panics() {
        Span::new(FileId(0), 0, 1).merge(Span::new(FileId(1), 0, 1));
    }
}

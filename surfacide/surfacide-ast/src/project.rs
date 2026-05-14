//! A `Project` is the union of all `.surf` files discovered under a
//! root path, grouped by their `module` header.
//!
//! Construction is in `surfacide-resolve` (it needs to inspect the
//! parsed files); this module only defines the data types.

use crate::decl::{Decl, ModuleFile};
use crate::ident::QualifiedName;
use crate::span::{FileId, FileRegistry};
use indexmap::IndexMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct Project {
    pub files: FileRegistry,
    pub modules: IndexMap<String, ModuleAggregate>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModuleAggregate {
    pub name: QualifiedName,
    pub private: bool,
    pub files: Vec<FileId>,
    pub decls: Vec<Decl>,
}

impl ModuleAggregate {
    pub fn new(name: QualifiedName) -> Self {
        Self {
            name,
            private: false,
            files: Vec::new(),
            decls: Vec::new(),
        }
    }
}

impl Project {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ingest_file(&mut self, file: FileId, parsed: ModuleFile) {
        let key = parsed.header.name.dotted();
        let entry = self
            .modules
            .entry(key)
            .or_insert_with(|| ModuleAggregate::new(parsed.header.name.clone()));
        entry.private = entry.private || parsed.header.private;
        entry.files.push(file);
        entry.decls.extend(parsed.decls);
    }
}

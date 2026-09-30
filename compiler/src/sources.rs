//! Source texts and paths indexed by stable, loader-assigned source IDs.

use std::path::{Path, PathBuf};

use crate::ast::SourceId;

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single(text: &str) -> Self {
        let mut sources = Self::new();
        sources.push(PathBuf::from("<memory>"), text.to_owned());
        sources
    }

    pub fn push(&mut self, path: PathBuf, text: String) -> SourceId {
        let id = SourceId(self.files.len());
        self.files.push(SourceFile { path, text });
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.0)
    }

    pub fn display_path(&self, id: SourceId) -> Option<&Path> {
        let file = self.get(id)?;
        let base = self.files.first()?.path.parent();
        Some(
            base.and_then(|base| file.path.strip_prefix(base).ok())
                .unwrap_or(&file.path),
        )
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

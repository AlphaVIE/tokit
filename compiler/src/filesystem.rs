//! Experimental path-scoped filesystem read capability.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    Denied,
    NotFound,
    InvalidUtf8,
    Other,
}

impl ReadError {
    pub fn variant(self) -> &'static str {
        match self {
            Self::Denied => "Denied",
            Self::NotFound => "NotFound",
            Self::InvalidUtf8 => "InvalidUtf8",
            Self::Other => "Other",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReadPolicy {
    root: Option<PathBuf>,
}

impl ReadPolicy {
    pub fn from_root(root: Option<&Path>) -> Self {
        Self {
            root: root.and_then(|path| path.canonicalize().ok()),
        }
    }

    pub fn read_text(&self, path: &str) -> Result<String, ReadError> {
        let root = self.root.as_ref().ok_or(ReadError::Denied)?;
        let requested = Path::new(path);
        let resolved = match requested.canonicalize() {
            Ok(resolved) => resolved,
            Err(error) => {
                let parent = requested
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let allowed_parent = parent
                    .canonicalize()
                    .is_ok_and(|resolved| resolved.starts_with(root));
                return Err(if allowed_parent {
                    classify(error.kind())
                } else {
                    ReadError::Denied
                });
            }
        };
        if !resolved.starts_with(root) {
            return Err(ReadError::Denied);
        }
        let bytes = std::fs::read(resolved).map_err(|error| classify(error.kind()))?;
        String::from_utf8(bytes).map_err(|_| ReadError::InvalidUtf8)
    }
}

fn classify(kind: std::io::ErrorKind) -> ReadError {
    match kind {
        std::io::ErrorKind::NotFound => ReadError::NotFound,
        _ => ReadError::Other,
    }
}

//! Experimental path-scoped filesystem capabilities.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoError {
    Denied,
    NotFound,
    InvalidUtf8,
    Other,
}

pub use IoError as ReadError;

impl IoError {
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

#[derive(Clone, Debug, Default)]
pub struct WritePolicy {
    root: Option<PathBuf>,
}

impl WritePolicy {
    pub fn from_root(root: Option<&Path>) -> Self {
        Self {
            root: root.and_then(|path| path.canonicalize().ok()),
        }
    }

    pub fn write_text(&self, path: &str, text: &str) -> Result<(), IoError> {
        let root = self.root.as_ref().ok_or(IoError::Denied)?;
        let requested = Path::new(path);
        let target = match requested.canonicalize() {
            Ok(target) => target,
            Err(_) => {
                // A dangling link must not turn into a write outside the grant.
                if requested
                    .symlink_metadata()
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(IoError::Denied);
                }
                let parent = requested
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let parent = parent.canonicalize().map_err(|_| IoError::Denied)?;
                let name = requested.file_name().ok_or(IoError::Other)?;
                parent.join(name)
            }
        };
        if !target.starts_with(root) {
            return Err(IoError::Denied);
        }
        std::fs::write(target, text.as_bytes()).map_err(|error| classify(error.kind()))
    }
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

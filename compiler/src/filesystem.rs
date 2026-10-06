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
        self.write_bytes(path, text.as_bytes())
    }

    pub fn write_bytes(&self, path: &str, bytes: &[u8]) -> Result<(), IoError> {
        let target = self.target(path)?;
        std::fs::write(target, bytes).map_err(|error| classify(error.kind()))
    }

    /// Create one directory whose parent lies inside the grant.
    pub fn make_dir(&self, path: &str) -> Result<(), IoError> {
        let target = self.target(path)?;
        std::fs::create_dir(target).map_err(|error| classify(error.kind()))
    }

    /// Remove an existing file inside the grant; directories are not removed.
    pub fn remove_file(&self, path: &str) -> Result<(), IoError> {
        let root = self.root.as_ref().ok_or(IoError::Denied)?;
        let resolved = Path::new(path).canonicalize().map_err(|error| {
            if Path::new(path)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .canonicalize()
                .is_ok_and(|parent| parent.starts_with(root))
            {
                classify(error.kind())
            } else {
                IoError::Denied
            }
        })?;
        if !resolved.starts_with(root) || resolved == *root {
            return Err(IoError::Denied);
        }
        std::fs::remove_file(resolved).map_err(|error| classify(error.kind()))
    }

    /// The path a write or creation may touch: an existing target or a new
    /// name in an existing parent, either way inside the grant.
    fn target(&self, path: &str) -> Result<PathBuf, IoError> {
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
        Ok(target)
    }
}

impl ReadPolicy {
    pub fn from_root(root: Option<&Path>) -> Self {
        Self {
            root: root.and_then(|path| path.canonicalize().ok()),
        }
    }

    pub fn read_text(&self, path: &str) -> Result<String, ReadError> {
        let bytes = self.read_bytes(path)?;
        String::from_utf8(bytes).map_err(|_| ReadError::InvalidUtf8)
    }

    pub fn read_bytes(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        let resolved = self.resolve(path)?;
        std::fs::read(resolved).map_err(|error| classify(error.kind()))
    }

    /// Entry names of a directory inside the grant, sorted for determinism.
    pub fn list_dir(&self, path: &str) -> Result<Vec<String>, ReadError> {
        let resolved = self.resolve(path)?;
        let mut names = std::fs::read_dir(resolved)
            .map_err(|error| classify(error.kind()))?
            .map(|entry| {
                let entry = entry.map_err(|error| classify(error.kind()))?;
                entry
                    .file_name()
                    .into_string()
                    .map_err(|_| ReadError::InvalidUtf8)
            })
            .collect::<Result<Vec<_>, _>>()?;
        names.sort();
        Ok(names)
    }

    /// Whether a path inside the grant exists; without a grant nothing exists.
    pub fn exists(&self, path: &str) -> bool {
        self.resolve(path).is_ok()
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, ReadError> {
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
        Ok(resolved)
    }
}

fn classify(kind: std::io::ErrorKind) -> ReadError {
    match kind {
        std::io::ErrorKind::NotFound => ReadError::NotFound,
        _ => ReadError::Other,
    }
}

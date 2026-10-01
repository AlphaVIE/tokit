//! Provisional, content-pinned local source dependencies.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct PackageFile {
    pub path: PathBuf,
    pub source: String,
    pub label: PathBuf,
    pub root: PathBuf,
    pub multi_file: bool,
}

pub struct PackageSource {
    pub path: PathBuf,
    pub source: String,
    pub files: Vec<PackageFile>,
}

struct TreeFile {
    path: PathBuf,
    relative: String,
    bytes: Vec<u8>,
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest.as_ref() {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes))
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(hash_bytes(&bytes))
}

fn collect_tree(root: &Path) -> Result<Vec<TreeFile>, String> {
    fn walk(root: &Path, directory: &Path, files: &mut Vec<TreeFile>) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("cannot read package directory: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot list package directory: {error}"))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("cannot inspect package entry: {error}"))?;
            if file_type.is_symlink() {
                return Err("package trees cannot contain symlinks".to_owned());
            }
            let path = entry.path();
            if file_type.is_dir() {
                walk(root, &path, files)?;
            } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "tok") {
                let relative = path
                    .strip_prefix(root)
                    .expect("walked path has package root")
                    .components()
                    .map(|component| {
                        component
                            .as_os_str()
                            .to_str()
                            .ok_or("package source paths must be UTF-8")
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join("/");
                let bytes = fs::read(&path)
                    .map_err(|error| format!("cannot read package source {relative}: {error}"))?;
                files.push(TreeFile {
                    path,
                    relative,
                    bytes,
                });
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    if files.is_empty() {
        return Err("package tree has no .tok source files".to_owned());
    }
    Ok(files)
}

fn hash_tree(files: &[TreeFile]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tokit-package-tree-v1\0");
    for file in files {
        let name = file.relative.as_bytes();
        hasher.update((name.len() as u64).to_be_bytes());
        hasher.update(name);
        hasher.update((file.bytes.len() as u64).to_be_bytes());
        hasher.update(&file.bytes);
    }
    hex_digest(hasher.finalize())
}

pub fn hash_path(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let root = fs::canonicalize(path)
            .map_err(|error| format!("cannot resolve package directory: {error}"))?;
        Ok(hash_tree(&collect_tree(&root)?))
    } else {
        hash_file(path)
    }
}

fn checked_digest<'a>(name: &str, entry: &'a toml::Table) -> Result<&'a str, String> {
    let digest = entry
        .get("sha256")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("dependency {name} sha256 must be a string"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|character| character.is_ascii_digit() || (b'a'..=b'f').contains(&character))
    {
        return Err(format!(
            "dependency {name} sha256 must be 64 lowercase hex digits"
        ));
    }
    Ok(digest)
}

fn package_file(name: &str, path: PathBuf, expected: &str) -> Result<PackageSource, String> {
    if path.extension().is_none_or(|ext| ext != "tok") {
        return Err(format!("dependency {name} must name a .tok file"));
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("cannot read dependency {name}: {error}"))?;
    if hash_bytes(&bytes) != expected {
        return Err(format!("dependency {name} sha256 mismatch"));
    }
    let source =
        String::from_utf8(bytes).map_err(|_| format!("dependency {name} is not UTF-8 source"))?;
    let root = path
        .parent()
        .expect("canonical file has parent")
        .to_path_buf();
    Ok(PackageSource {
        path: path.clone(),
        source: source.clone(),
        files: vec![PackageFile {
            path,
            source,
            label: PathBuf::from(format!("pkg/{name}.tok")),
            root,
            multi_file: false,
        }],
    })
}

fn package_tree(
    name: &str,
    root: PathBuf,
    entry: &str,
    expected: &str,
) -> Result<PackageSource, String> {
    let relative = Path::new(entry);
    if entry.is_empty()
        || entry.contains('\\')
        || !relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        || relative.extension().is_none_or(|ext| ext != "tok")
    {
        return Err(format!(
            "dependency {name} entry must be a relative .tok path"
        ));
    }
    let files = collect_tree(&root)?;
    if hash_tree(&files) != expected {
        return Err(format!("dependency {name} sha256 mismatch"));
    }
    let entry_path = files
        .iter()
        .find(|file| file.relative == entry)
        .map(|file| file.path.clone())
        .ok_or_else(|| format!("dependency {name} entry is not in the pinned source tree"))?;
    let mut package_files = Vec::new();
    let mut entry_source = None;
    for file in files {
        let source = String::from_utf8(file.bytes)
            .map_err(|_| format!("dependency {name} source {} is not UTF-8", file.relative))?;
        if file.path == entry_path {
            entry_source = Some(source.clone());
        }
        package_files.push(PackageFile {
            path: file.path,
            source,
            label: PathBuf::from(format!("pkg/{name}/{}", file.relative)),
            root: root.clone(),
            multi_file: true,
        });
    }
    Ok(PackageSource {
        path: entry_path,
        source: entry_source.expect("entry was found in pinned files"),
        files: package_files,
    })
}

pub fn load(root_dir: &Path) -> Result<HashMap<String, PackageSource>, String> {
    let manifest = root_dir.join("tok.toml");
    let source =
        fs::read_to_string(&manifest).map_err(|error| format!("cannot read tok.toml: {error}"))?;
    let table: toml::Table =
        toml::from_str(&source).map_err(|error| format!("invalid tok.toml: {error}"))?;
    let dependencies = table
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .ok_or("tok.toml needs a [dependencies] table")?;
    let mut resolved = HashMap::new();
    let mut paths = HashSet::new();
    for (name, value) in dependencies {
        let mut chars = name.chars();
        if !chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(format!("invalid dependency name {name:?}"));
        }
        let entry = value
            .as_table()
            .ok_or_else(|| format!("dependency {name} must be a table"))?;
        let path = entry
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("dependency {name} path must be a string"))?;
        let expected = checked_digest(name, entry)?;
        let relative = Path::new(path);
        if path.is_empty() || relative.is_absolute() {
            return Err(format!("dependency {name} path must be relative"));
        }
        let canonical = fs::canonicalize(root_dir.join(relative))
            .map_err(|error| format!("cannot resolve dependency {name}: {error}"))?;
        let package = if canonical.is_file() {
            if entry.len() != 2 || !entry.contains_key("path") || !entry.contains_key("sha256") {
                return Err(format!(
                    "dependency {name} needs only path and sha256 fields"
                ));
            }
            package_file(name, canonical, expected)?
        } else if canonical.is_dir() {
            if entry.len() != 3
                || !entry.contains_key("path")
                || !entry.contains_key("sha256")
                || !entry.contains_key("entry")
            {
                return Err(format!(
                    "dependency {name} needs path, entry, and sha256 fields"
                ));
            }
            let main = entry
                .get("entry")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("dependency {name} entry must be a string"))?;
            package_tree(name, canonical, main, expected)?
        } else {
            return Err(format!(
                "dependency {name} path must name a file or directory"
            ));
        };
        for file in &package.files {
            if !paths.insert(file.path.clone()) {
                return Err(format!("dependency {name} repeats a package file"));
            }
        }
        resolved.insert(name.clone(), package);
    }
    Ok(resolved)
}

//! Provisional, content-pinned local source dependencies.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub struct PackageSource {
    pub path: PathBuf,
    pub source: String,
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    hex
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Ok(hash_bytes(&bytes))
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
    let mut files = HashSet::new();
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
        if entry.len() != 2 || !entry.contains_key("path") || !entry.contains_key("sha256") {
            return Err(format!(
                "dependency {name} needs only path and sha256 fields"
            ));
        }
        let path = entry
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("dependency {name} path must be a string"))?;
        let expected = entry
            .get("sha256")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("dependency {name} sha256 must be a string"))?;
        if expected.len() != 64
            || !expected
                .bytes()
                .all(|character| character.is_ascii_digit() || (b'a'..=b'f').contains(&character))
        {
            return Err(format!(
                "dependency {name} sha256 must be 64 lowercase hex digits"
            ));
        }
        let relative = Path::new(path);
        if path.is_empty() || relative.is_absolute() {
            return Err(format!("dependency {name} path must be relative"));
        }
        let canonical = fs::canonicalize(root_dir.join(relative))
            .map_err(|error| format!("cannot resolve dependency {name}: {error}"))?;
        if !canonical.is_file() || canonical.extension().is_none_or(|ext| ext != "tok") {
            return Err(format!("dependency {name} must name a .tok file"));
        }
        let bytes = fs::read(&canonical)
            .map_err(|error| format!("cannot read dependency {name}: {error}"))?;
        let actual = hash_bytes(&bytes);
        if actual != expected {
            return Err(format!("dependency {name} sha256 mismatch"));
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| format!("dependency {name} is not UTF-8 source"))?;
        if !files.insert(canonical.clone()) {
            return Err(format!("dependency {name} repeats a package file"));
        }
        resolved.insert(
            name.clone(),
            PackageSource {
                path: canonical,
                source,
            },
        );
    }
    Ok(resolved)
}

//! The official package registry, embedded in `tok` at build time from
//! `packages/`. Packages are materialized into a content-addressed store
//! (`$TOK_HOME/store/<sha256>/`, default `~/.tok`) before loading.

use std::fs;
use std::path::PathBuf;

include!(concat!(env!("OUT_DIR"), "/registry_files.rs"));

#[derive(Clone, Debug)]
pub struct RegistryPackage {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub description: String,
    pub sha256: String,
    manifest: &'static [u8],
    /// `.tok` sources relative to the package directory.
    sources: Vec<(String, &'static [u8])>,
}

/// Every package in the embedded registry, sorted by name.
pub fn packages() -> Vec<RegistryPackage> {
    let mut names: Vec<&str> = FILES
        .iter()
        .filter_map(|(path, _)| path.split_once('/').map(|(name, _)| name))
        .collect();
    names.dedup();
    names
        .into_iter()
        .filter_map(|name| {
            let manifest = FILES
                .iter()
                .find(|(path, _)| *path == format!("{name}/tok.toml"))?
                .1;
            let table: toml::Table = std::str::from_utf8(manifest).ok()?.parse().ok()?;
            let package = table.get("package")?.as_table()?;
            let field = |key: &str| {
                package
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            };
            let prefix = format!("{name}/");
            let sources: Vec<(String, &'static [u8])> = FILES
                .iter()
                .filter_map(|(path, bytes)| {
                    let relative = path.strip_prefix(&prefix)?;
                    relative
                        .ends_with(".tok")
                        .then(|| (relative.to_owned(), *bytes))
                })
                .collect();
            let sha256 = crate::packages::hash_sources(&sources, Some(manifest));
            Some(RegistryPackage {
                name: field("name").filter(|declared| declared == name)?,
                version: field("version")?,
                entry: field("entry")?,
                description: field("description").unwrap_or_default(),
                sha256,
                manifest,
                sources,
            })
        })
        .collect()
}

/// The registry package `name`, optionally at an exact `version`.
pub fn find(name: &str, version: Option<&str>) -> Option<RegistryPackage> {
    packages().into_iter().find(|package| {
        package.name == name && version.is_none_or(|wanted| package.version == wanted)
    })
}

fn store_root() -> Result<PathBuf, String> {
    let home = std::env::var_os("TOK_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".tok"))
        })
        .ok_or("cannot locate the package store; set TOK_HOME")?;
    Ok(home.join("store"))
}

/// The verified store directory holding `package`, writing it if needed.
pub fn materialize(package: &RegistryPackage) -> Result<PathBuf, String> {
    let store = store_root()?;
    let target = store.join(&package.sha256);
    if crate::packages::hash_path(&target).is_ok_and(|digest| digest == package.sha256) {
        return fs::canonicalize(&target)
            .map_err(|error| format!("cannot open package store: {error}"));
    }
    fs::create_dir_all(&store).map_err(|error| format!("cannot create package store: {error}"))?;
    let staging = store.join(format!(
        ".tmp-{}-{}-{}",
        package.sha256,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    ));
    let write = || -> Result<(), String> {
        fs::create_dir_all(&staging).map_err(|error| format!("cannot stage package: {error}"))?;
        fs::write(staging.join("tok.toml"), package.manifest)
            .map_err(|error| format!("cannot stage package manifest: {error}"))?;
        for (relative, bytes) in &package.sources {
            let path = staging.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot stage package: {error}"))?;
            }
            fs::write(&path, bytes)
                .map_err(|error| format!("cannot stage package source: {error}"))?;
        }
        if crate::packages::hash_path(&staging)? != package.sha256 {
            return Err(format!(
                "staged package {} failed verification",
                package.name
            ));
        }
        // A tampered or partial entry is replaced; a concurrent install may win.
        if target.exists()
            && !crate::packages::hash_path(&target).is_ok_and(|digest| digest == package.sha256)
        {
            fs::remove_dir_all(&target)
                .map_err(|error| format!("cannot replace corrupt store entry: {error}"))?;
        }
        if fs::rename(&staging, &target).is_err()
            && !crate::packages::hash_path(&target).is_ok_and(|digest| digest == package.sha256)
        {
            return Err(format!(
                "cannot install package {} into the store",
                package.name
            ));
        }
        Ok(())
    };
    let result = write();
    let _ = fs::remove_dir_all(&staging);
    result?;
    fs::canonicalize(&target).map_err(|error| format!("cannot open package store: {error}"))
}

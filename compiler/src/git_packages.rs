//! Packages fetched from Git repositories. A dependency pins a repository URL,
//! a full commit id, and the package tree digest; the checked-out `.tok` files
//! and `tok.toml` are copied into the content-addressed store, so later builds
//! work offline and a moved or rewritten tag cannot change what is built.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A package checked out into the store.
pub struct Fetched {
    pub directory: PathBuf,
    pub rev: String,
    pub sha256: String,
    pub entry: String,
}

fn validate_url(url: &str) -> Result<(), String> {
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err(format!("invalid git URL {url:?}"));
    }
    Ok(())
}

/// A full hexadecimal commit id (SHA-1 or SHA-256 repositories).
pub fn is_commit(rev: &str) -> bool {
    matches!(rev.len(), 40 | 64) && rev.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn git(args: &[&str], directory: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new(std::env::var_os("TOKIT_GIT").unwrap_or_else(|| "git".into()));
    // Byte-exact checkouts on every platform, so the tree digest is portable.
    command.args(["-c", "core.autocrlf=false", "-c", "core.symlinks=false"]);
    if let Some(directory) = directory {
        command.arg("-C").arg(directory);
    }
    let output = command
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The `[package] entry` of a package manifest.
fn manifest_entry(manifest: &str) -> Result<String, String> {
    let table: toml::Table =
        toml::from_str(manifest).map_err(|error| format!("invalid package tok.toml: {error}"))?;
    table
        .get("package")
        .and_then(|package| package.get("entry"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "the repository's tok.toml needs [package] entry".to_owned())
}

/// Copy the package files of `checkout` (`tok.toml` and every `.tok` file
/// outside `.git`) into `staging`.
fn copy_package(checkout: &Path, staging: &Path) -> Result<(), String> {
    fn walk(root: &Path, directory: &Path, staging: &Path) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if entry.file_name() == ".git" || file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                walk(root, &path, staging)?;
            } else if path.extension().is_some_and(|ext| ext == "tok")
                || (directory == root && entry.file_name() == "tok.toml")
            {
                let target = staging.join(path.strip_prefix(root).expect("walked under root"));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                fs::copy(&path, &target).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
    walk(checkout, checkout, staging).map_err(|error| format!("cannot stage package: {error}"))
}

/// Clone `url` at `rev` (default: its default branch) into the store.
pub fn fetch(url: &str, rev: Option<&str>) -> Result<Fetched, String> {
    validate_url(url)?;
    if let Some(rev) = rev
        && (rev.starts_with('-') || rev.chars().any(char::is_whitespace))
    {
        return Err(format!("invalid git revision {rev:?}"));
    }
    let store = crate::registry::tok_home()?.join("store");
    fs::create_dir_all(&store).map_err(|error| format!("cannot create package store: {error}"))?;
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    );
    let checkout = store.join(format!(".git-{unique}"));
    let staging = store.join(format!(".tmp-git-{unique}"));
    let result = (|| {
        let checkout_text = checkout.to_str().ok_or("store path must be UTF-8")?;
        git(
            &[
                "clone",
                "--quiet",
                "--no-checkout",
                "--",
                url,
                checkout_text,
            ],
            None,
        )?;
        git(
            &["checkout", "--quiet", rev.unwrap_or("HEAD")],
            Some(&checkout),
        )?;
        let commit = git(&["rev-parse", "HEAD"], Some(&checkout))?;
        let manifest = fs::read_to_string(checkout.join("tok.toml"))
            .map_err(|_| "the repository has no tok.toml at its root".to_owned())?;
        let entry = manifest_entry(&manifest)?;
        fs::create_dir_all(&staging).map_err(|error| format!("cannot stage package: {error}"))?;
        copy_package(&checkout, &staging)?;
        let sha256 = crate::packages::hash_path(&staging)?;
        let target = store.join(&sha256);
        if !crate::packages::hash_path(&target).is_ok_and(|digest| digest == sha256) {
            if target.exists() {
                fs::remove_dir_all(&target)
                    .map_err(|error| format!("cannot replace corrupt store entry: {error}"))?;
            }
            if fs::rename(&staging, &target).is_err()
                && !crate::packages::hash_path(&target).is_ok_and(|digest| digest == sha256)
            {
                return Err("cannot install the package into the store".to_owned());
            }
        }
        let directory = fs::canonicalize(&target)
            .map_err(|error| format!("cannot open package store: {error}"))?;
        Ok(Fetched {
            directory,
            rev: commit,
            sha256,
            entry,
        })
    })();
    let _ = fs::remove_dir_all(&checkout);
    let _ = fs::remove_dir_all(&staging);
    result
}

/// The store directory for a pinned dependency, fetching it when missing.
pub fn materialize(url: &str, rev: &str, sha256: &str) -> Result<(PathBuf, String), String> {
    if !is_commit(rev) {
        return Err(format!(
            "git dependency rev must be a full commit id, not {rev:?}"
        ));
    }
    let target = crate::registry::tok_home()?.join("store").join(sha256);
    let directory = if crate::packages::hash_path(&target).is_ok_and(|digest| digest == sha256) {
        fs::canonicalize(&target).map_err(|error| format!("cannot open package store: {error}"))?
    } else {
        let fetched = fetch(url, Some(rev))?;
        if fetched.sha256 != sha256 {
            return Err(format!(
                "git package {url}@{rev} has sha256 {}, but tok.toml pins {sha256}",
                fetched.sha256
            ));
        }
        fetched.directory
    };
    let manifest = fs::read_to_string(directory.join("tok.toml"))
        .map_err(|error| format!("cannot read package tok.toml: {error}"))?;
    Ok((directory, manifest_entry(&manifest)?))
}

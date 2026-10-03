//! Provisional, content-pinned local source dependencies.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

#[derive(Clone)]
pub struct PackageFile {
    pub path: PathBuf,
    pub source: String,
    pub label: PathBuf,
    pub root: PathBuf,
    pub owner: PathBuf,
    pub multi_file: bool,
}

#[derive(Clone)]
pub struct PackageSource {
    pub path: PathBuf,
    pub source: String,
    pub files: Vec<PackageFile>,
    manifest: Option<String>,
}

pub struct PackageGraph {
    pub roots: HashMap<String, PathBuf>,
    pub packages: HashMap<PathBuf, PackageSource>,
    pub children: HashMap<PathBuf, HashMap<String, PathBuf>>,
}

impl PackageGraph {
    pub fn get(&self, name: &str) -> Option<&PackageSource> {
        self.roots
            .get(name)
            .and_then(|owner| self.packages.get(owner))
    }

    pub fn entry(&self, owner: Option<&Path>, name: &str) -> Option<&Path> {
        let target = match owner {
            Some(owner) => self.children.get(owner)?.get(name)?,
            None => self.roots.get(name)?,
        };
        self.packages
            .get(target)
            .map(|package| package.path.as_path())
    }
}

#[derive(Debug)]
pub struct PackageError {
    pub code: &'static str,
    pub message: String,
}

struct LockEntry {
    id: String,
    name: String,
    path: String,
    entry: Option<String>,
    sha256: String,
    manifest_sha256: Option<String>,
    owner: PathBuf,
    sources: Vec<String>,
}

struct TreeFile {
    path: PathBuf,
    relative: String,
    bytes: Vec<u8>,
}

struct StagedFile(PathBuf);

impl StagedFile {
    fn new(root: &Path, label: &str, contents: &[u8]) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("cannot create staged file timestamp: {error}"))?
            .as_nanos();
        for attempt in 0..16 {
            let path = root.join(format!(
                ".tok-{label}-{}-{nonce}-{attempt}.tmp",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let staged = Self(path);
                    file.write_all(contents)
                        .and_then(|()| file.sync_all())
                        .map_err(|error| format!("cannot stage {label}: {error}"))?;
                    return Ok(staged);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("cannot stage {label}: {error}")),
            }
        }
        Err(format!("cannot find a free staged {label} filename"))
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
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

fn hash_tree(files: &[TreeFile], manifest: Option<&[u8]>) -> String {
    let mut hasher = Sha256::new();
    if let Some(manifest) = manifest {
        hasher.update(b"tokit-package-tree-v2\0");
        hasher.update((manifest.len() as u64).to_be_bytes());
        hasher.update(manifest);
    } else {
        hasher.update(b"tokit-package-tree-v1\0");
    }
    for file in files {
        let name = file.relative.as_bytes();
        hasher.update((name.len() as u64).to_be_bytes());
        hasher.update(name);
        hasher.update((file.bytes.len() as u64).to_be_bytes());
        hasher.update(&file.bytes);
    }
    hex_digest(hasher.finalize())
}

fn tree_manifest(root: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(root.join("tok.toml")) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read package tok.toml: {error}")),
    }
}

pub fn hash_path(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let root = fs::canonicalize(path)
            .map_err(|error| format!("cannot resolve package directory: {error}"))?;
        Ok(hash_tree(
            &collect_tree(&root)?,
            tree_manifest(&root)?.as_deref(),
        ))
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
            path: path.clone(),
            source,
            label: if name.contains('/') {
                PathBuf::from(format!("pkg/@/{name}.tok"))
            } else {
                PathBuf::from(format!("pkg/{name}.tok"))
            },
            root,
            owner: path.clone(),
            multi_file: false,
        }],
        manifest: None,
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
    let manifest = tree_manifest(&root)?;
    if hash_tree(&files, manifest.as_deref()) != expected {
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
            label: if name.contains('/') {
                PathBuf::from(format!("pkg/@/{name}/{}", file.relative))
            } else {
                PathBuf::from(format!("pkg/{name}/{}", file.relative))
            },
            root: root.clone(),
            owner: root.clone(),
            multi_file: true,
        });
    }
    Ok(PackageSource {
        path: entry_path,
        source: entry_source.expect("entry was found in pinned files"),
        files: package_files,
        manifest: manifest
            .map(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|_| format!("dependency {name} tok.toml is not UTF-8"))
            })
            .transpose()?,
    })
}

struct GraphBuilder {
    graph: PackageGraph,
    paths: HashSet<PathBuf>,
    active: HashSet<PathBuf>,
    digests: HashMap<PathBuf, String>,
    ids: HashMap<PathBuf, String>,
    entries: Vec<LockEntry>,
}

impl GraphBuilder {
    fn new() -> Self {
        Self {
            graph: PackageGraph {
                roots: HashMap::new(),
                packages: HashMap::new(),
                children: HashMap::new(),
            },
            paths: HashSet::new(),
            active: HashSet::new(),
            digests: HashMap::new(),
            ids: HashMap::new(),
            entries: Vec::new(),
        }
    }

    fn resolve_dependencies(
        &mut self,
        root_dir: &Path,
        source: &str,
        parent: Option<&Path>,
    ) -> Result<(), String> {
        let table: toml::Table =
            toml::from_str(source).map_err(|error| format!("invalid tok.toml: {error}"))?;
        let dependencies = table
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .ok_or("tok.toml needs a [dependencies] table")?;
        let mut names = dependencies.keys().collect::<Vec<_>>();
        names.sort();
        for name in names {
            let mut chars = name.chars();
            if !chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                || !chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
            {
                return Err(format!("invalid dependency name {name:?}"));
            }
            let record = dependencies[name]
                .as_table()
                .ok_or_else(|| format!("dependency {name} must be a table"))?;
            let path = record
                .get("path")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| format!("dependency {name} path must be a string"))?;
            let expected = checked_digest(name, record)?;
            let relative = Path::new(path);
            if path.is_empty() || path.contains('\\') || relative.is_absolute() {
                return Err(format!("dependency {name} path must be relative and use /"));
            }
            let canonical = fs::canonicalize(root_dir.join(relative))
                .map_err(|error| format!("cannot resolve dependency {name}: {error}"))?;
            let mut entry_name = None;
            if canonical.is_file() {
                if record.len() != 2
                    || !record.contains_key("path")
                    || !record.contains_key("sha256")
                {
                    return Err(format!(
                        "dependency {name} needs only path and sha256 fields"
                    ));
                }
            } else if canonical.is_dir() {
                if record.len() != 3
                    || !record.contains_key("path")
                    || !record.contains_key("sha256")
                    || !record.contains_key("entry")
                {
                    return Err(format!(
                        "dependency {name} needs path, entry, and sha256 fields"
                    ));
                }
                entry_name = Some(
                    record
                        .get("entry")
                        .and_then(toml::Value::as_str)
                        .ok_or_else(|| format!("dependency {name} entry must be a string"))?,
                );
            } else {
                return Err(format!(
                    "dependency {name} path must name a file or directory"
                ));
            }
            let owner = canonical.clone();
            if self.active.contains(&owner) {
                return Err(format!("dependency cycle reaches {name}"));
            }
            if let Some(previous) = self.digests.get(&owner) {
                if previous != expected {
                    return Err(format!(
                        "dependency {name} uses one package with conflicting checksums"
                    ));
                }
                let package = self
                    .graph
                    .packages
                    .get(&owner)
                    .expect("resolved package exists");
                if entry_name.is_some_and(|entry| package.path != canonical.join(entry)) {
                    return Err(format!(
                        "dependency {name} uses one package with conflicting entry files"
                    ));
                }
            } else {
                let id = parent.map_or_else(
                    || name.clone(),
                    |parent| format!("{}/{}", self.ids[parent], name),
                );
                let package = if canonical.is_file() {
                    package_file(&id, canonical, expected)?
                } else {
                    package_tree(
                        &id,
                        canonical,
                        entry_name.expect("directory entry checked"),
                        expected,
                    )?
                };
                for file in &package.files {
                    if !self.paths.insert(file.path.clone()) {
                        return Err(format!("dependency {name} overlaps another package source"));
                    }
                }
                let mut sources = package
                    .files
                    .iter()
                    .map(|file| {
                        file.label
                            .to_str()
                            .expect("package label is UTF-8")
                            .to_owned()
                    })
                    .collect::<Vec<_>>();
                sources.sort();
                let manifest_sha256 = package
                    .manifest
                    .as_ref()
                    .map(|source| hash_bytes(source.as_bytes()));
                let manifest = package.manifest.clone();
                self.ids.insert(owner.clone(), id.clone());
                self.digests.insert(owner.clone(), expected.to_owned());
                self.graph.packages.insert(owner.clone(), package);
                self.active.insert(owner.clone());
                if let Some(manifest) = manifest {
                    self.resolve_dependencies(&owner, &manifest, Some(&owner))?;
                }
                self.active.remove(&owner);
                self.entries.push(LockEntry {
                    id,
                    name: name.clone(),
                    path: path.to_owned(),
                    entry: entry_name.map(str::to_owned),
                    sha256: expected.to_owned(),
                    manifest_sha256,
                    owner: owner.clone(),
                    sources,
                });
            }
            match parent {
                Some(parent) => {
                    self.graph
                        .children
                        .entry(parent.to_path_buf())
                        .or_default()
                        .insert(name.clone(), owner);
                }
                None => {
                    self.graph.roots.insert(name.clone(), owner);
                }
            }
        }
        Ok(())
    }
}

fn render_lock(
    manifest_hash: String,
    graph: &PackageGraph,
    mut entries: Vec<LockEntry>,
) -> Result<String, String> {
    let mut lock = toml::Table::new();
    let ids = entries
        .iter()
        .map(|entry| (entry.owner.clone(), entry.id.clone()))
        .collect::<HashMap<_, _>>();
    lock.insert("format".to_owned(), toml::Value::Integer(2));
    lock.insert(
        "compiler".to_owned(),
        toml::Value::String(format!("tokit-compiler/{}", env!("CARGO_PKG_VERSION"))),
    );
    lock.insert(
        "target".to_owned(),
        toml::Value::String("portable-source".to_owned()),
    );
    lock.insert(
        "manifest_sha256".to_owned(),
        toml::Value::String(manifest_hash),
    );
    let mut roots = toml::Table::new();
    for (name, owner) in &graph.roots {
        roots.insert(name.clone(), toml::Value::String(ids[owner].clone()));
    }
    lock.insert("roots".to_owned(), toml::Value::Table(roots));
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    let packages = entries
        .into_iter()
        .map(|entry| {
            let mut record = toml::Table::new();
            record.insert("id".to_owned(), toml::Value::String(entry.id));
            record.insert("name".to_owned(), toml::Value::String(entry.name));
            record.insert("path".to_owned(), toml::Value::String(entry.path));
            if let Some(main) = entry.entry {
                record.insert("entry".to_owned(), toml::Value::String(main));
            }
            record.insert("sha256".to_owned(), toml::Value::String(entry.sha256));
            if let Some(digest) = entry.manifest_sha256 {
                record.insert("manifest_sha256".to_owned(), toml::Value::String(digest));
            }
            record.insert(
                "sources".to_owned(),
                toml::Value::Array(entry.sources.into_iter().map(toml::Value::String).collect()),
            );
            let mut edges = graph
                .children
                .get(&entry.owner)
                .into_iter()
                .flat_map(|dependencies| dependencies.iter())
                .map(|(name, owner)| (name.clone(), owner.clone()))
                .collect::<Vec<_>>();
            edges.sort_by(|left, right| left.0.cmp(&right.0));
            record.insert(
                "dependencies".to_owned(),
                toml::Value::Array(
                    edges
                        .into_iter()
                        .map(|(name, owner)| {
                            let mut edge = toml::Table::new();
                            edge.insert("name".to_owned(), toml::Value::String(name));
                            edge.insert(
                                "target".to_owned(),
                                toml::Value::String(ids[&owner].clone()),
                            );
                            toml::Value::Table(edge)
                        })
                        .collect(),
                ),
            );
            toml::Value::Table(record)
        })
        .collect();
    lock.insert("package".to_owned(), toml::Value::Array(packages));
    toml::to_string(&lock).map_err(|error| format!("cannot serialize tok.lock: {error}"))
}

fn resolve_source(root_dir: &Path, source: &str) -> Result<(PackageGraph, String), String> {
    let mut builder = GraphBuilder::new();
    builder.resolve_dependencies(root_dir, source, None)?;
    let lock = render_lock(
        hash_bytes(source.as_bytes()),
        &builder.graph,
        builder.entries,
    )?;
    Ok((builder.graph, lock))
}

fn resolve(root_dir: &Path) -> Result<(PackageGraph, String), String> {
    let source = fs::read_to_string(root_dir.join("tok.toml"))
        .map_err(|error| format!("cannot read tok.toml: {error}"))?;
    resolve_source(root_dir, &source)
}

fn check_regular_destination(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => Err(format!(
            "{} must be a regular file, not a symlink or directory",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot inspect {}: {error}", path.display())),
    }
}

fn write_manifest_update(
    root_dir: &Path,
    source: &str,
    previous: Option<&str>,
) -> Result<PathBuf, String> {
    let (_, lock) = resolve_source(root_dir, source)?;
    let manifest = root_dir.join("tok.toml");
    let lock_path = root_dir.join("tok.lock");
    check_regular_destination(&manifest)?;
    check_regular_destination(&lock_path)?;
    let manifest_stage = StagedFile::new(root_dir, "manifest", source.as_bytes())?;
    let lock_stage = StagedFile::new(root_dir, "lock", lock.as_bytes())?;
    let previous_stage = previous
        .map(|contents| StagedFile::new(root_dir, "rollback", contents.as_bytes()))
        .transpose()?;
    fs::rename(&manifest_stage.0, &manifest)
        .map_err(|error| format!("cannot replace tok.toml: {error}"))?;
    if let Err(error) = fs::rename(&lock_stage.0, &lock_path) {
        let rollback = match previous_stage {
            Some(stage) => fs::rename(&stage.0, &manifest),
            None => fs::remove_file(&manifest),
        };
        return Err(match rollback {
            Ok(()) => format!("cannot replace tok.lock: {error}; tok.toml restored"),
            Err(restore) => {
                format!("cannot replace tok.lock: {error}; cannot restore tok.toml: {restore}")
            }
        });
    }
    Ok(lock_path)
}

pub fn add_local(
    root_dir: &Path,
    name: &str,
    path: &str,
    entry: Option<&str>,
) -> Result<PathBuf, String> {
    let manifest = root_dir.join("tok.toml");
    check_regular_destination(&manifest)?;
    let previous = match fs::read_to_string(&manifest) {
        Ok(source) => Some(source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot read tok.toml: {error}")),
    };
    let mut document = match &previous {
        Some(source) => source
            .parse::<DocumentMut>()
            .map_err(|error| format!("invalid tok.toml: {error}"))?,
        None => DocumentMut::new(),
    };
    if document.get("dependencies").is_none() {
        document["dependencies"] = Item::Table(Table::new());
    }
    let dependencies = document["dependencies"]
        .as_table_mut()
        .ok_or("tok.toml needs a [dependencies] table")?;
    if dependencies.contains_key(name) {
        return Err(format!("dependency {name} already exists"));
    }
    if path.is_empty() || path.contains('\\') || Path::new(path).is_absolute() {
        return Err("dependency path must be a relative slash-separated path".to_owned());
    }
    let target = root_dir.join(path);
    let metadata = fs::symlink_metadata(&target)
        .map_err(|error| format!("cannot inspect dependency path: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("dependency path must not be a symlink".to_owned());
    }
    if metadata.is_file() && entry.is_some() {
        return Err("single-file dependency cannot have --entry".to_owned());
    }
    if metadata.is_dir() && entry.is_none() {
        return Err("directory dependency needs --entry <relative.tok>".to_owned());
    }
    if !metadata.is_file() && !metadata.is_dir() {
        return Err("dependency path must name a file or directory".to_owned());
    }
    let digest = hash_path(&target)?;
    let mut item = InlineTable::new();
    item.insert("path", Value::from(path));
    if let Some(entry) = entry {
        item.insert("entry", Value::from(entry));
    }
    item.insert("sha256", Value::from(digest));
    dependencies.insert(name, Item::Value(Value::InlineTable(item)));
    write_manifest_update(root_dir, &document.to_string(), previous.as_deref())
}

pub fn remove_local(root_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let manifest = root_dir.join("tok.toml");
    check_regular_destination(&manifest)?;
    let previous =
        fs::read_to_string(&manifest).map_err(|error| format!("cannot read tok.toml: {error}"))?;
    let mut document = previous
        .parse::<DocumentMut>()
        .map_err(|error| format!("invalid tok.toml: {error}"))?;
    let dependencies = document
        .get_mut("dependencies")
        .and_then(Item::as_table_mut)
        .ok_or("tok.toml needs a [dependencies] table")?;
    if dependencies.remove(name).is_none() {
        return Err(format!("unknown dependency {name}"));
    }
    write_manifest_update(root_dir, &document.to_string(), Some(&previous))
}

pub fn write_lock(root_dir: &Path) -> Result<PathBuf, String> {
    let (_, contents) = resolve(root_dir)?;
    let path = root_dir.join("tok.lock");
    check_regular_destination(&path)?;
    let staged = StagedFile::new(root_dir, "lock", contents.as_bytes())?;
    fs::rename(&staged.0, &path).map_err(|error| format!("cannot replace tok.lock: {error}"))?;
    Ok(path)
}

pub fn load_graph(root_dir: &Path) -> Result<PackageGraph, PackageError> {
    let (resolved, expected) = resolve(root_dir).map_err(|message| PackageError {
        code: "E120",
        message,
    })?;
    let actual = fs::read_to_string(root_dir.join("tok.lock")).map_err(|error| PackageError {
        code: "E121",
        message: format!("cannot read tok.lock: {error}; run tok lock <entry.tok>"),
    })?;
    if actual != expected {
        return Err(PackageError {
            code: "E121",
            message: "tok.lock is stale; run tok lock <entry.tok>".to_owned(),
        });
    }
    Ok(resolved)
}

pub fn load(root_dir: &Path) -> Result<HashMap<String, PackageSource>, PackageError> {
    let graph = load_graph(root_dir)?;
    Ok(graph
        .roots
        .iter()
        .map(|(name, owner)| (name.clone(), graph.packages[owner].clone()))
        .collect())
}

//! Deterministic, file-backed module loading.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::ast::{ImportDecl, Program, SourceId, Span};
use crate::diagnostic::Diagnostic;
use crate::module_resolver::{self, ModuleUnit};
use crate::packages;
use crate::sources::SourceMap;

pub struct LoadedProgram {
    pub program: Program,
    pub sources: SourceMap,
    pub modules: Vec<ModuleInfo>,
}

pub struct ModuleInfo {
    pub source_id: SourceId,
    pub imports: Vec<(String, SourceId)>,
    pub exports: Vec<String>,
}

pub struct LoadError {
    pub diagnostic: Diagnostic,
    pub sources: SourceMap,
}

impl LoadError {
    pub fn display(&self) -> String {
        self.diagnostic.display_with_sources(&self.sources)
    }

    pub fn json(&self) -> String {
        self.diagnostic.json_with_sources(&self.sources)
    }
}

struct Loader {
    root_dir: PathBuf,
    sources: SourceMap,
    visited: HashMap<PathBuf, SourceId>,
    active: HashSet<PathBuf>,
    units: Vec<Option<ModuleUnit>>,
    order: Vec<SourceId>,
    dependencies: Option<HashMap<String, packages::PackageSource>>,
    package_files: HashMap<PathBuf, packages::PackageFile>,
}

fn package_relative_path(parent: &Path, root: &Path, relative: &Path) -> Option<PathBuf> {
    let mut path = parent.to_path_buf();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => path.push(name),
            Component::ParentDir => {
                if path == root {
                    return None;
                }
                path.pop();
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    path.starts_with(root).then_some(path)
}

impl Loader {
    fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            sources: SourceMap::new(),
            visited: HashMap::new(),
            active: HashSet::new(),
            units: Vec::new(),
            order: Vec::new(),
            dependencies: None,
            package_files: HashMap::new(),
        }
    }

    fn resolve_import(
        &mut self,
        source_path: &Path,
        import: &ImportDecl,
    ) -> Result<PathBuf, Diagnostic> {
        if let Some(name) = import.path.strip_prefix("pkg:") {
            if self.package_files.contains_key(source_path) {
                return Err(Diagnostic::new(
                    "E120",
                    import.span,
                    "package sources cannot import other packages yet",
                ));
            }
            if self.dependencies.is_none() {
                let dependencies = packages::load(&self.root_dir)
                    .map_err(|message| Diagnostic::new("E120", import.span, message))?;
                for package in dependencies.values() {
                    for file in &package.files {
                        if self.visited.contains_key(&file.path) {
                            return Err(Diagnostic::new(
                                "E120",
                                import.span,
                                "package file was already imported by relative path",
                            ));
                        }
                        self.package_files.insert(file.path.clone(), file.clone());
                    }
                }
                self.dependencies = Some(dependencies);
            }
            let target = self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.get(name))
                .map(|package| package.path.clone())
                .ok_or_else(|| {
                    Diagnostic::new("E120", import.span, format!("unknown dependency {name:?}"))
                })?;
            if let Some(source_id) = self.visited.get(&target)
                && self
                    .sources
                    .get(*source_id)
                    .is_some_and(|source| source.display_override.is_none())
            {
                return Err(Diagnostic::new(
                    "E120",
                    import.span,
                    "package file was already imported by relative path",
                ));
            }
            return Ok(target);
        }
        let relative = Path::new(&import.path);
        if relative.as_os_str().is_empty() || relative.is_absolute() {
            return Err(Diagnostic::new(
                "E118",
                import.span,
                "import path must be relative",
            ));
        }
        let parent = source_path.parent().expect("canonical file has parent");
        let package = self.package_files.get(source_path);
        let boundary = package.map_or(self.root_dir.as_path(), |file| file.root.as_path());
        let canonical = if package.is_some() {
            package_relative_path(parent, boundary, relative).ok_or_else(|| {
                Diagnostic::new("E120", import.span, "import escapes the package directory")
            })?
        } else {
            fs::canonicalize(parent.join(relative)).map_err(|error| {
                Diagnostic::new(
                    "E118",
                    import.span,
                    format!("cannot resolve import {}: {error}", import.path),
                )
            })?
        };
        if !canonical.starts_with(boundary) {
            return Err(Diagnostic::new(
                if package.is_some() { "E120" } else { "E118" },
                import.span,
                if package.is_some() {
                    "import escapes the package directory"
                } else {
                    "import escapes the entry directory"
                },
            ));
        }
        if canonical
            .extension()
            .is_none_or(|extension| extension != "tok")
        {
            return Err(Diagnostic::new(
                "E118",
                import.span,
                "import path must name a .tok file",
            ));
        }
        if package.is_some() && !self.package_files.contains_key(&canonical) {
            return Err(Diagnostic::new(
                "E120",
                import.span,
                "import is not in the pinned package source tree",
            ));
        }
        if package.is_none() && self.package_files.contains_key(&canonical) {
            return Err(Diagnostic::new(
                "E120",
                import.span,
                "package file must be imported through pkg:",
            ));
        }
        Ok(canonical)
    }

    fn visit(&mut self, path: PathBuf, origin: Option<Span>) -> Result<SourceId, Diagnostic> {
        if self.active.contains(&path) {
            return Err(Diagnostic::new(
                "E118",
                origin.unwrap_or_else(|| Span::new(0, 0)),
                format!("import cycle reaches {}", path.display()),
            ));
        }
        if let Some(id) = self.visited.get(&path) {
            return Ok(*id);
        }
        let text = if let Some(file) = self.package_files.get(&path) {
            file.source.clone()
        } else {
            fs::read_to_string(&path).map_err(|error| {
                Diagnostic::new(
                    "E118",
                    origin.unwrap_or_else(|| Span::new(0, 0)),
                    format!("cannot read {}: {error}", path.display()),
                )
            })?
        };
        let label = self.package_files.get(&path).map(|file| file.label.clone());
        let source_id = self.sources.push_labeled(path.clone(), text, label);
        self.units.push(None);
        self.visited.insert(path.clone(), source_id);
        self.active.insert(path.clone());
        let source = &self.sources.get(source_id).expect("registered source").text;
        let parsed = crate::parse_in_source(source, source_id)?;
        if self
            .package_files
            .get(&path)
            .is_some_and(|file| !file.multi_file)
            && !parsed.imports.is_empty()
        {
            return Err(Diagnostic::new(
                "E120",
                parsed.imports[0].span,
                "single-file package dependencies cannot import other modules",
            ));
        }
        let mut local_imports = HashSet::new();
        let mut aliases = HashMap::new();
        for import in &parsed.imports {
            let resolved = self.resolve_import(&path, import)?;
            if !local_imports.insert(resolved.clone()) {
                return Err(Diagnostic::new(
                    "E118",
                    import.span,
                    "duplicate import in one source",
                ));
            }
            let imported = self.visit(resolved, Some(import.span))?;
            aliases.insert(import.alias.clone(), imported);
        }
        self.active.remove(&path);
        self.units[source_id.0] = Some(ModuleUnit {
            source_id,
            program: parsed,
            aliases,
        });
        self.order.push(source_id);
        Ok(source_id)
    }
}

pub fn load(path: &Path) -> Result<LoadedProgram, LoadError> {
    let root = match fs::canonicalize(path) {
        Ok(root) => root,
        Err(error) => {
            let mut sources = SourceMap::new();
            sources.push(path.to_path_buf(), String::new());
            return Err(LoadError {
                diagnostic: Diagnostic::new(
                    "E118",
                    Span::new(0, 0),
                    format!("cannot open entry file: {error}"),
                ),
                sources,
            });
        }
    };
    let root_dir = root
        .parent()
        .expect("canonical entry has parent")
        .to_path_buf();
    let mut loader = Loader::new(root_dir);
    if let Err(diagnostic) = loader.visit(root.clone(), None) {
        if loader.sources.is_empty() {
            loader.sources.push(root, String::new());
        }
        return Err(LoadError {
            diagnostic,
            sources: loader.sources,
        });
    }
    let modules = loader
        .units
        .iter()
        .map(|unit| {
            let unit = unit.as_ref().expect("parsed module");
            let mut imports = unit
                .aliases
                .iter()
                .map(|(alias, target)| (alias.clone(), *target))
                .collect::<Vec<_>>();
            imports.sort_by(|left, right| left.0.cmp(&right.0));
            let mut exports = unit
                .program
                .records
                .iter()
                .filter(|record| record.public)
                .map(|record| record.name.clone())
                .chain(
                    unit.program
                        .enums
                        .iter()
                        .filter(|declaration| declaration.public)
                        .map(|declaration| declaration.name.clone()),
                )
                .chain(
                    unit.program
                        .functions
                        .iter()
                        .filter(|function| function.public)
                        .map(|function| function.name.clone()),
                )
                .collect::<Vec<_>>();
            exports.sort();
            ModuleInfo {
                source_id: unit.source_id,
                imports,
                exports,
            }
        })
        .collect();
    let program = match module_resolver::resolve(&mut loader.units, &loader.order, &loader.sources)
    {
        Ok(program) => program,
        Err(diagnostic) => {
            return Err(LoadError {
                diagnostic,
                sources: loader.sources,
            });
        }
    };
    if let Err(diagnostic) = crate::checker::check(&program) {
        return Err(LoadError {
            diagnostic,
            sources: loader.sources,
        });
    }
    Ok(LoadedProgram {
        program,
        sources: loader.sources,
        modules,
    })
}

//! Deterministic, file-backed module loading.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::{ImportDecl, Program, SourceId, Span};
use crate::diagnostic::Diagnostic;
use crate::module_resolver::{self, ModuleUnit};
use crate::sources::SourceMap;

pub struct LoadedProgram {
    pub program: Program,
    pub sources: SourceMap,
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
        }
    }

    fn resolve_import(&self, parent: &Path, import: &ImportDecl) -> Result<PathBuf, Diagnostic> {
        let relative = Path::new(&import.path);
        if relative.as_os_str().is_empty() || relative.is_absolute() {
            return Err(Diagnostic::new(
                "E118",
                import.span,
                "import path must be relative",
            ));
        }
        let candidate = parent.join(relative);
        let canonical = fs::canonicalize(&candidate).map_err(|error| {
            Diagnostic::new(
                "E118",
                import.span,
                format!("cannot resolve import {}: {error}", import.path),
            )
        })?;
        if !canonical.starts_with(&self.root_dir) {
            return Err(Diagnostic::new(
                "E118",
                import.span,
                "import escapes the entry directory",
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
        let text = fs::read_to_string(&path).map_err(|error| {
            Diagnostic::new(
                "E118",
                origin.unwrap_or_else(|| Span::new(0, 0)),
                format!("cannot read {}: {error}", path.display()),
            )
        })?;
        let source_id = self.sources.push(path.clone(), text);
        self.units.push(None);
        self.visited.insert(path.clone(), source_id);
        self.active.insert(path.clone());
        let source = &self.sources.get(source_id).expect("registered source").text;
        let parsed = crate::parse_in_source(source, source_id)?;
        let mut local_imports = HashSet::new();
        let mut aliases = HashMap::new();
        let parent = path.parent().expect("canonical file has parent");
        for import in &parsed.imports {
            let resolved = self.resolve_import(parent, import)?;
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
    })
}

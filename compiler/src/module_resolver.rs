//! Resolve module-local declarations and direct imported exports before checking.

use std::collections::HashMap;

use crate::ast::{Expr, ExprKind, PatternKind, PlaceStep, Program, SourceId, Span, Stmt, Type};
use crate::builtins;
use crate::diagnostic::Diagnostic;
use crate::sources::SourceMap;

pub struct ModuleUnit {
    pub source_id: SourceId,
    pub program: Program,
    pub aliases: HashMap<String, SourceId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Function,
    Record,
    Enum,
}

struct Symbol {
    canonical: String,
    public: bool,
    kind: SymbolKind,
}

#[derive(Default)]
struct Scope {
    symbols: HashMap<String, Symbol>,
    aliases: HashMap<String, SourceId>,
}

#[derive(Clone, Copy)]
enum UseKind {
    Call,
    Type,
    Enum,
}

impl UseKind {
    fn code(self) -> &'static str {
        match self {
            Self::Call => "E101",
            Self::Type => "E103",
            Self::Enum => "E114",
        }
    }

    fn accepts(self, kind: SymbolKind) -> bool {
        match self {
            Self::Call => matches!(kind, SymbolKind::Function | SymbolKind::Record),
            Self::Type => matches!(kind, SymbolKind::Record | SymbolKind::Enum),
            Self::Enum => kind == SymbolKind::Enum,
        }
    }

    fn builtin(self, name: &str) -> bool {
        match self {
            Self::Call => builtins::is_call(name),
            Self::Type => matches!(name, "IoError" | "TaskError" | "ParseError" | "Map"),
            Self::Enum => matches!(name, "IoError" | "TaskError" | "ParseError"),
        }
    }
}

fn reserved(name: &str) -> bool {
    matches!(
        name,
        "i32"
            | "i64"
            | "f64"
            | "bool"
            | "String"
            | "Bytes"
            | "Unit"
            | "Result"
            | "Option"
            | "Task"
            | "IoError"
            | "TaskError"
            | "ParseError"
            | "read_text"
            | "read_bytes"
            | "write_text"
            | "write_bytes"
            | "lines"
            | "args"
            | "print"
            | "abs"
            | "min"
            | "max"
            | "pow"
            | "sqrt"
            | "floor"
            | "ceil"
            | "round"
            | "exp"
            | "ln"
            | "sin"
            | "cos"
            | "tan"
            | "atan2"
            | "pi"
            | "map"
            | "filter"
            | "any"
            | "all"
            | "fold"
            | "sort_by"
            | "range"
            | "sort"
            | "reverse"
            | "slice"
            | "Map"
            | "get"
            | "get_or"
            | "keys"
            | "values"
            | "remove"
            | "chars"
            | "split"
            | "trim"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "replace"
            | "lower"
            | "upper"
            | "read_line"
            | "read_stdin"
            | "exit"
            | "len"
            | "parse_i32"
            | "parse_i64"
            | "parse_f64"
            | "utf8_bytes"
            | "utf8_decode"
            | "utf8_encode"
            | "utf8_decode_bytes"
            | "bytes_from_i32"
            | "bytes_to_i32"
            | "join"
    )
}

fn canonical_name(id: SourceId, name: &str, sources: &SourceMap) -> String {
    if id.0 == 0 {
        return name.to_owned();
    }
    let path = sources.display_path(id).expect("registered module path");
    let stem = path.with_extension("");
    let prefix = stem
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("::");
    format!("{prefix}::{name}")
}

fn insert_symbol(
    scope: &mut Scope,
    id: SourceId,
    sources: &SourceMap,
    name: &str,
    public: bool,
    kind: SymbolKind,
    span: Span,
) -> Result<(), Diagnostic> {
    if reserved(name) || scope.symbols.contains_key(name) {
        return Err(Diagnostic::new(
            "E106",
            span,
            format!("duplicate or reserved declaration {name}"),
        ));
    }
    scope.symbols.insert(
        name.to_owned(),
        Symbol {
            canonical: canonical_name(id, name, sources),
            public,
            kind,
        },
    );
    Ok(())
}

fn collect_scope(unit: &ModuleUnit, sources: &SourceMap) -> Result<Scope, Diagnostic> {
    let mut scope = Scope {
        aliases: unit.aliases.clone(),
        ..Scope::default()
    };
    for record in &unit.program.records {
        insert_symbol(
            &mut scope,
            unit.source_id,
            sources,
            &record.name,
            record.public,
            SymbolKind::Record,
            record.span,
        )?;
    }
    for declaration in &unit.program.enums {
        insert_symbol(
            &mut scope,
            unit.source_id,
            sources,
            &declaration.name,
            declaration.public,
            SymbolKind::Enum,
            declaration.span,
        )?;
    }
    for function in &unit.program.functions {
        insert_symbol(
            &mut scope,
            unit.source_id,
            sources,
            &function.name,
            function.public,
            SymbolKind::Function,
            function.span,
        )?;
    }
    for import in &unit.program.imports {
        if reserved(&import.alias) || scope.symbols.contains_key(&import.alias) {
            return Err(Diagnostic::new(
                "E118",
                import.span,
                format!("import alias {} conflicts with a declaration", import.alias),
            ));
        }
    }
    Ok(scope)
}

struct Resolver<'a> {
    id: SourceId,
    scopes: &'a [Scope],
    /// Local bindings in scope; calls to them invoke function values.
    locals: std::cell::RefCell<Vec<String>>,
}

impl Resolver<'_> {
    fn lookup(&self, name: &str, kind: UseKind, span: Span) -> Result<String, Diagnostic> {
        let scope = &self.scopes[self.id.0];
        let (owner, local_name) = if let Some((alias, member)) = name.split_once("::") {
            let Some(owner) = scope.aliases.get(alias) else {
                return Err(Diagnostic::new(
                    "E118",
                    span,
                    format!("unknown import alias {alias}"),
                ));
            };
            (*owner, member)
        } else {
            (self.id, name)
        };
        let Some(symbol) = self.scopes[owner.0].symbols.get(local_name) else {
            if owner == self.id && kind.builtin(name) {
                return Ok(name.to_owned());
            }
            return Err(Diagnostic::new(
                kind.code(),
                span,
                format!("unknown module member {name}"),
            ));
        };
        if owner != self.id && !symbol.public {
            return Err(Diagnostic::new(
                "E119",
                span,
                format!("{name} is private to its module"),
            ));
        }
        if !kind.accepts(symbol.kind) {
            return Err(Diagnostic::new(
                kind.code(),
                span,
                format!("{name} is not valid in this position"),
            ));
        }
        Ok(symbol.canonical.clone())
    }

    fn ty(&self, ty: &mut Type, span: Span) -> Result<(), Diagnostic> {
        match ty {
            Type::Named(name) => *name = self.lookup(name, UseKind::Type, span)?,
            Type::Applied(name, args) => {
                *name = self.lookup(name, UseKind::Type, span)?;
                for argument in args {
                    self.ty(argument, span)?;
                }
            }
            Type::Array(inner) | Type::Option(inner) | Type::Task(inner) => self.ty(inner, span)?,
            Type::Fn(params, ret) => {
                for param in params {
                    self.ty(param, span)?;
                }
                self.ty(ret, span)?;
            }
            Type::Result(ok, err) => {
                self.ty(ok, span)?;
                self.ty(err, span)?;
            }
            Type::I32
            | Type::I64
            | Type::F64
            | Type::Bool
            | Type::String
            | Type::Bytes
            | Type::Unit
            | Type::Never
            | Type::Param(_)
            | Type::EmptyArray => {}
        }
        Ok(())
    }

    fn expr(&self, expr: &mut Expr) -> Result<(), Diagnostic> {
        match &mut expr.kind {
            ExprKind::Int(_)
            | ExprKind::I64(_)
            | ExprKind::F64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_)
            | ExprKind::None => {}
            ExprKind::Variant(name, _, payload) => {
                *name = self.lookup(name, UseKind::Enum, expr.span)?;
                if let Some(value) = payload {
                    self.expr(value)?;
                }
            }
            ExprKind::Array(items) => {
                for item in items {
                    self.expr(item)?;
                }
            }
            ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
                self.expr(left)?;
                self.expr(right)?;
            }
            ExprKind::Field(value, _)
            | ExprKind::Not(value)
            | ExprKind::Neg(value)
            | ExprKind::Ok(value)
            | ExprKind::Err(value)
            | ExprKind::Some(value)
            | ExprKind::Try(value)
            | ExprKind::Spawn(value) => self.expr(value)?,
            ExprKind::Call(name, args) => {
                // Declarations win; an unknown name may be a local function value.
                match self.lookup(name, UseKind::Call, expr.span) {
                    Ok(resolved) => *name = resolved,
                    Err(_) if self.locals.borrow().contains(name) => {}
                    Err(error) => return Err(error),
                }
                for argument in args {
                    self.expr(argument)?;
                }
            }
            ExprKind::Apply(callee, args) => {
                self.expr(callee)?;
                for argument in args {
                    self.expr(argument)?;
                }
            }
            ExprKind::Lambda(params, body) => {
                let depth = self.locals.borrow().len();
                for (name, ty) in params.iter_mut() {
                    if let Some(ty) = ty {
                        self.ty(ty, expr.span)?;
                    }
                    self.locals.borrow_mut().push(name.clone());
                }
                let result = self.expr(body);
                self.locals.borrow_mut().truncate(depth);
                result?;
            }
            ExprKind::If(condition, yes, no) => {
                self.expr(condition)?;
                self.expr(yes)?;
                self.expr(no)?;
            }
            ExprKind::Match(value, arms) => {
                self.expr(value)?;
                for (pattern, body) in arms {
                    if let PatternKind::Variant(name, _, _) = &mut pattern.kind {
                        *name = self.lookup(name, UseKind::Enum, pattern.span)?;
                    }
                    let depth = self.locals.borrow().len();
                    if let Some(binding) = pattern.binding() {
                        self.locals.borrow_mut().push(binding.to_owned());
                    }
                    let result = self.expr(body);
                    self.locals.borrow_mut().truncate(depth);
                    result?;
                }
            }
            ExprKind::Block(statements, tail) => {
                let depth = self.locals.borrow().len();
                let result = statements
                    .iter_mut()
                    .try_for_each(|statement| self.stmt(statement))
                    .and_then(|()| tail.as_mut().map_or(Ok(()), |tail| self.expr(tail)));
                self.locals.borrow_mut().truncate(depth);
                result?;
            }
        }
        Ok(())
    }

    fn stmt(&self, stmt: &mut Stmt) -> Result<(), Diagnostic> {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
                ..
            } => {
                if let Some(ty) = ty {
                    self.ty(ty, *span)?;
                }
                self.expr(value)?;
                self.locals.borrow_mut().push(name.clone());
            }
            Stmt::Assign { path, value, .. } => {
                for step in path {
                    if let PlaceStep::Index(index, _) = step {
                        self.expr(index)?;
                    }
                }
                self.expr(value)?;
            }
            Stmt::Push { value, .. } | Stmt::Return { value, .. } | Stmt::Expr(value) => {
                self.expr(value)?
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => {
                self.expr(iterable)?;
                self.locals.borrow_mut().push(name.clone());
                let result = self.expr(body);
                self.locals.borrow_mut().pop();
                result?;
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expr(condition)?;
                self.expr(body)?;
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
        Ok(())
    }

    fn program(&self, program: &mut Program) -> Result<(), Diagnostic> {
        for record in &mut program.records {
            for (_, ty) in &mut record.fields {
                self.ty(ty, record.span)?;
            }
            record.name = self.lookup(&record.name, UseKind::Type, record.span)?;
        }
        for declaration in &mut program.enums {
            for variant in &mut declaration.variants {
                if let Some(payload) = &mut variant.payload {
                    self.ty(payload, declaration.span)?;
                }
            }
            declaration.name = self.lookup(&declaration.name, UseKind::Enum, declaration.span)?;
        }
        for function in &mut program.functions {
            for (_, ty) in &mut function.params {
                self.ty(ty, function.span)?;
            }
            self.ty(&mut function.ret, function.span)?;
            *self.locals.borrow_mut() = function
                .params
                .iter()
                .map(|(name, _)| name.clone())
                .collect();
            self.expr(&mut function.body)?;
            self.locals.borrow_mut().clear();
            function.name = self.lookup(&function.name, UseKind::Call, function.span)?;
        }
        Ok(())
    }
}

pub fn resolve(
    units: &mut [Option<ModuleUnit>],
    order: &[SourceId],
    sources: &SourceMap,
) -> Result<Program, Diagnostic> {
    let scopes = units
        .iter()
        .map(|unit| collect_scope(unit.as_ref().expect("parsed module"), sources))
        .collect::<Result<Vec<_>, _>>()?;
    let mut merged = Program {
        imports: Vec::new(),
        records: Vec::new(),
        enums: Vec::new(),
        functions: Vec::new(),
    };
    for id in order {
        let mut unit = units[id.0].take().expect("module in traversal order");
        Resolver {
            id: *id,
            scopes: &scopes,
            locals: std::cell::RefCell::new(Vec::new()),
        }
        .program(&mut unit.program)?;
        merged.records.extend(unit.program.records);
        merged.enums.extend(unit.program.enums);
        merged.functions.extend(unit.program.functions);
    }
    Ok(merged)
}

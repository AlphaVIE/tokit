#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub source_id: SourceId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self::in_source(SourceId::default(), start, end)
    }

    pub fn in_source(source_id: SourceId, start: usize, end: usize) -> Self {
        Self {
            source_id,
            start,
            end,
        }
    }

    pub fn join(self, other: Self) -> Self {
        assert_eq!(
            self.source_id, other.source_id,
            "cannot join spans from different sources"
        );
        Self {
            source_id: self.source_id,
            start: self.start,
            end: other.end,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    I32,
    I64,
    F64,
    Bool,
    String,
    Bytes,
    Named(String),
    Applied(String, Vec<Type>),
    Param(String),
    Unit,
    Never,
    Array(Box<Type>),
    Option(Box<Type>),
    Task(Box<Type>),
    /// A function value `(params)->ret`.
    Fn(Vec<Type>, Box<Type>),
    EmptyArray,
    Result(Box<Type>, Box<Type>),
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::String => "String",
            Self::Bytes => "Bytes",
            Self::Named(name) => return f.write_str(name),
            Self::Applied(name, args) => {
                write!(f, "{name}<")?;
                for (index, arg) in args.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{arg}")?;
                }
                return f.write_str(">");
            }
            Self::Param(name) => return f.write_str(name),
            Self::Unit => "Unit",
            Self::Never => "never",
            Self::Array(element) => return write!(f, "[{element}]"),
            Self::Option(element) => return write!(f, "Option<{element}>"),
            Self::Task(result) => return write!(f, "Task<{result}>"),
            Self::Fn(params, ret) => {
                f.write_str("(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{param}")?;
                }
                return write!(f, ")->{ret}");
            }
            Self::EmptyArray => "empty array",
            Self::Result(ok, err) => return write!(f, "Result<{ok},{err}>"),
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    And,
    Or,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    /// `if`, `match`, and blocks end with `}` and may stand as statements
    /// without a trailing `;`.
    pub fn is_block_like(&self) -> bool {
        matches!(
            self.kind,
            ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Block(..)
        )
    }

    /// Conservative subset that cannot mutate a caller binding while evaluated.
    /// Unknown forms must use normal value snapshots when an operand is borrowed.
    pub fn is_simple_read(&self) -> bool {
        match &self.kind {
            ExprKind::Int(_)
            | ExprKind::I64(_)
            | ExprKind::F64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => true,
            ExprKind::Not(value) | ExprKind::Neg(value) | ExprKind::Field(value, _) => {
                value.is_simple_read()
            }
            ExprKind::Binary(left, _, right) | ExprKind::Index(left, right) => {
                left.is_simple_read() && right.is_simple_read()
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PatternKind {
    Int(i32),
    I64(i64),
    Wildcard,
    Ok(String),
    Err(String),
    Some(String),
    None,
    Variant(String, String, Option<String>),
    Bool(bool),
    String(String),
}

#[derive(Clone, Debug)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(i32),
    I64(i64),
    F64(u64),
    Bool(bool),
    String(String),
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Variant(String, String, Option<Box<Expr>>),
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Some(Box<Expr>),
    None,
    Try(Box<Expr>),
    Var(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
    Call(String, Vec<Expr>),
    Spawn(Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<(Pattern, Expr)>),
    Block(Vec<Stmt>, Option<Box<Expr>>),
    /// `|x,y:T|body`: parameters with optional types, captured by value.
    Lambda(Vec<(String, Option<Type>)>, Box<Expr>),
    /// Calling a function value read from a field or element: `op.run(x)`.
    Apply(Box<Expr>, Vec<Expr>),
}

/// One step of an assignment target such as `grid[y].cells[x]`. The span
/// covers the target up to and including this step.
#[derive(Clone, Debug)]
pub enum PlaceStep {
    Index(Expr, Span),
    Field(String, Span),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<Type>,
        value: Expr,
        mutable: bool,
        span: Span,
    },
    Assign {
        name: String,
        /// Element and field steps below the binding; empty for `name = value`.
        path: Vec<PlaceStep>,
        value: Expr,
        span: Span,
    },
    Push {
        name: String,
        value: Expr,
        span: Span,
    },
    For {
        name: String,
        iterable: Expr,
        body: Expr,
        span: Span,
    },
    While {
        condition: Expr,
        body: Expr,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Return {
        value: Expr,
        span: Span,
    },
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub public: bool,
    pub type_params: Vec<String>,
    pub params: Vec<(String, Type)>,
    pub ret: Type,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub name: String,
    pub public: bool,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: String,
    pub public: bool,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<Type>,
}

#[derive(Clone, Debug)]
pub struct ImportDecl {
    pub alias: String,
    pub path: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub records: Vec<Record>,
    pub enums: Vec<EnumDecl>,
    pub functions: Vec<Function>,
}

impl Expr {
    /// Names read or called in this expression that are not bound inside it,
    /// in first-use order. Callers decide which of them are captured locals.
    pub fn free_names(&self) -> Vec<String> {
        let mut found = Vec::new();
        collect_free(self, &mut Vec::new(), &mut found);
        found
    }
}

fn note(name: &str, bound: &[String], found: &mut Vec<String>) {
    if !bound.iter().any(|candidate| candidate == name) && !found.iter().any(|seen| seen == name) {
        found.push(name.to_owned());
    }
}

fn collect_free(expr: &Expr, bound: &mut Vec<String>, found: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::I64(_)
        | ExprKind::F64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::None => {}
        ExprKind::Var(name) => note(name, bound, found),
        ExprKind::Call(name, args) => {
            note(name, bound, found);
            args.iter().for_each(|arg| collect_free(arg, bound, found));
        }
        ExprKind::Array(items) => items
            .iter()
            .for_each(|item| collect_free(item, bound, found)),
        ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
            collect_free(left, bound, found);
            collect_free(right, bound, found);
        }
        ExprKind::Field(inner, _)
        | ExprKind::Ok(inner)
        | ExprKind::Err(inner)
        | ExprKind::Some(inner)
        | ExprKind::Try(inner)
        | ExprKind::Not(inner)
        | ExprKind::Neg(inner)
        | ExprKind::Spawn(inner) => collect_free(inner, bound, found),
        ExprKind::Variant(_, _, payload) => {
            if let Some(payload) = payload {
                collect_free(payload, bound, found);
            }
        }
        ExprKind::If(condition, yes, no) => {
            collect_free(condition, bound, found);
            collect_free(yes, bound, found);
            collect_free(no, bound, found);
        }
        ExprKind::Match(value, arms) => {
            collect_free(value, bound, found);
            for (pattern, body) in arms {
                let depth = bound.len();
                bound.extend(pattern.binding().map(str::to_owned));
                collect_free(body, bound, found);
                bound.truncate(depth);
            }
        }
        ExprKind::Block(stmts, tail) => {
            let depth = bound.len();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } => {
                        collect_free(value, bound, found);
                        bound.push(name.clone());
                    }
                    Stmt::Assign {
                        name, path, value, ..
                    } => {
                        note(name, bound, found);
                        for step in path {
                            if let PlaceStep::Index(index, _) = step {
                                collect_free(index, bound, found);
                            }
                        }
                        collect_free(value, bound, found);
                    }
                    Stmt::Push { name, value, .. } => {
                        note(name, bound, found);
                        collect_free(value, bound, found);
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        ..
                    } => {
                        collect_free(iterable, bound, found);
                        bound.push(name.clone());
                        collect_free(body, bound, found);
                        bound.pop();
                    }
                    Stmt::While {
                        condition, body, ..
                    } => {
                        collect_free(condition, bound, found);
                        collect_free(body, bound, found);
                    }
                    Stmt::Return { value, .. } | Stmt::Expr(value) => {
                        collect_free(value, bound, found)
                    }
                    Stmt::Break { .. } | Stmt::Continue { .. } => {}
                }
            }
            if let Some(tail) = tail {
                collect_free(tail, bound, found);
            }
            bound.truncate(depth);
        }
        ExprKind::Lambda(params, body) => {
            let depth = bound.len();
            bound.extend(params.iter().map(|(name, _)| name.clone()));
            collect_free(body, bound, found);
            bound.truncate(depth);
        }
        ExprKind::Apply(callee, args) => {
            collect_free(callee, bound, found);
            args.iter().for_each(|arg| collect_free(arg, bound, found));
        }
    }
}

impl Pattern {
    /// The payload name a matching arm binds, if any.
    pub fn binding(&self) -> Option<&str> {
        match &self.kind {
            PatternKind::Ok(name) | PatternKind::Err(name) | PatternKind::Some(name) => Some(name),
            PatternKind::Variant(_, _, binding) => binding.as_deref(),
            PatternKind::Int(_)
            | PatternKind::I64(_)
            | PatternKind::Wildcard
            | PatternKind::None
            | PatternKind::Bool(_)
            | PatternKind::String(_) => None,
        }
    }
}

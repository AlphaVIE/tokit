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

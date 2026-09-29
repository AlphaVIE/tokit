#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn join(self, other: Self) -> Self {
        Self {
            start: self.start,
            end: other.end,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    I32,
    Bool,
    String,
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
            Self::Bool => "bool",
            Self::String => "String",
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
    Add,
    Sub,
    Mul,
    Div,
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

#[derive(Clone, Debug)]
pub enum PatternKind {
    Int(i32),
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
        ty: Type,
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
    pub type_params: Vec<String>,
    pub params: Vec<(String, Type)>,
    pub ret: Type,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Record {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<(String, Type)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Option<Type>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub records: Vec<Record>,
    pub enums: Vec<EnumDecl>,
    pub functions: Vec<Function>,
}

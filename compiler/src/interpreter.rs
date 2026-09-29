use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

use crate::ast::{Expr, ExprKind, Function, Op, PatternKind, Program, Span, Stmt};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    I32(i32),
    Bool(bool),
    String(String),
    Array(Vec<Value>),
    Record(String, Vec<(String, Value)>),
    Enum(String, String),
    Ok(Box<Value>),
    Err(Box<Value>),
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I32(n) => write!(f, "{n}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value:?}"),
            Self::Array(values) => {
                f.write_str("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{value}")?;
                }
                f.write_str("]")
            }
            Self::Record(name, fields) => {
                write!(f, "{name}(")?;
                for (index, (field, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{field}:{value}")?;
                }
                f.write_str(")")
            }
            Self::Enum(name, variant) => write!(f, "{name}::{variant}"),
            Self::Ok(value) => write!(f, "Ok({value})"),
            Self::Err(value) => write!(f, "Err({value})"),
            Self::Unit => f.write_str("()"),
        }
    }
}

enum Flow {
    Value(Value),
    Return(Value),
}

type Env = HashMap<String, Rc<RefCell<Value>>>;
const MAX_CALL_DEPTH: usize = 32;

macro_rules! take_value {
    ($expr:expr) => {
        match $expr? {
            Flow::Value(value) => value,
            Flow::Return(value) => return Ok(Flow::Return(value)),
        }
    };
}

pub fn run(program: &Program) -> Result<Value, Diagnostic> {
    let main = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .ok_or_else(|| {
            Diagnostic::new("E203", Span { start: 0, end: 0 }, "missing main function")
        })?;
    if !main.params.is_empty() || !main.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E203",
            main.span,
            "main must have no parameters or type parameters",
        ));
    }
    invoke(program, main, Vec::new(), 0)
}

fn invoke(
    program: &Program,
    function: &Function,
    args: Vec<Value>,
    depth: usize,
) -> Result<Value, Diagnostic> {
    if depth >= MAX_CALL_DEPTH {
        return Err(Diagnostic::new(
            "E202",
            function.span,
            "call depth limit exceeded",
        ));
    }
    let env: Env = function
        .params
        .iter()
        .zip(args)
        .map(|((name, _), value)| (name.clone(), Rc::new(RefCell::new(value))))
        .collect();
    match eval(&function.body, &env, program, depth)? {
        Flow::Value(value) | Flow::Return(value) => Ok(value),
    }
}

fn eval(expr: &Expr, env: &Env, program: &Program, depth: usize) -> Result<Flow, Diagnostic> {
    let value = match &expr.kind {
        ExprKind::Int(number) => Value::I32(*number),
        ExprKind::Bool(value) => Value::Bool(*value),
        ExprKind::String(value) => Value::String(value.clone()),
        ExprKind::Variant(name, variant) => Value::Enum(name.clone(), variant.clone()),
        ExprKind::Array(items) => {
            let mut values = Vec::new();
            for item in items {
                values.push(take_value!(eval(item, env, program, depth)));
            }
            Value::Array(values)
        }
        ExprKind::Index(array, index) => {
            let values = take_value!(eval(array, env, program, depth));
            let position = take_value!(eval(index, env, program, depth));
            let (Value::Array(values), Value::I32(position)) = (values, position) else {
                return Err(Diagnostic::new("E204", expr.span, "invalid runtime index"));
            };
            usize::try_from(position)
                .ok()
                .and_then(|position| values.get(position))
                .cloned()
                .ok_or_else(|| Diagnostic::new("E205", expr.span, "array index out of bounds"))?
        }
        ExprKind::Field(value, field) => {
            let value = take_value!(eval(value, env, program, depth));
            let Value::Record(_, fields) = value else {
                return Err(Diagnostic::new(
                    "E204",
                    expr.span,
                    "invalid runtime field access",
                ));
            };
            fields
                .into_iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| value)
                .ok_or_else(|| Diagnostic::new("E204", expr.span, "unknown runtime field"))?
        }
        ExprKind::Ok(inner) => Value::Ok(Box::new(take_value!(eval(inner, env, program, depth)))),
        ExprKind::Err(inner) => Value::Err(Box::new(take_value!(eval(inner, env, program, depth)))),
        ExprKind::Try(inner) => {
            let value = take_value!(eval(inner, env, program, depth));
            match value {
                Value::Ok(value) => *value,
                Value::Err(value) => return Ok(Flow::Return(Value::Err(value))),
                _ => {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid runtime propagation",
                    ));
                }
            }
        }
        ExprKind::Var(name) => {
            env.get(name)
                .map(|cell| cell.borrow().clone())
                .ok_or_else(|| {
                    Diagnostic::new("E204", expr.span, format!("unresolved runtime name {name}"))
                })?
        }
        ExprKind::Binary(left, op, right) => {
            let left = take_value!(eval(left, env, program, depth));
            let right = take_value!(eval(right, env, program, depth));
            binary(left, *op, right, expr.span)?
        }
        ExprKind::Call(name, args) => {
            let mut values = Vec::new();
            for arg in args {
                values.push(take_value!(eval(arg, env, program, depth)));
            }
            if let Some(record) = program.records.iter().find(|record| record.name == *name) {
                return Ok(Flow::Value(Value::Record(
                    name.clone(),
                    record
                        .fields
                        .iter()
                        .map(|(name, _)| name.clone())
                        .zip(values)
                        .collect(),
                )));
            }
            let function = program
                .functions
                .iter()
                .find(|function| function.name == *name)
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E204",
                        expr.span,
                        format!("unresolved runtime function {name}"),
                    )
                })?;
            invoke(program, function, values, depth + 1)?
        }
        ExprKind::If(condition, yes, no) => {
            let condition = take_value!(eval(condition, env, program, depth));
            let branch = if condition == Value::Bool(true) {
                yes
            } else {
                no
            };
            return eval(branch, env, program, depth);
        }
        ExprKind::Match(value, arms) => {
            let scrutinee = take_value!(eval(value, env, program, depth));
            for (pattern, body) in arms {
                let binding = match (&pattern.kind, &scrutinee) {
                    (PatternKind::Ok(name), Value::Ok(value)) => {
                        Some(Some((name.clone(), *value.clone())))
                    }
                    (PatternKind::Err(name), Value::Err(value)) => {
                        Some(Some((name.clone(), *value.clone())))
                    }
                    (PatternKind::Bool(pattern), Value::Bool(value)) if pattern == value => {
                        Some(None)
                    }
                    (PatternKind::Variant(name, variant), Value::Enum(actual, value))
                        if name == actual && variant == value =>
                    {
                        Some(None)
                    }
                    _ => None,
                };
                if let Some(binding) = binding {
                    let mut scope = env.clone();
                    if let Some((name, value)) = binding {
                        scope.insert(name, Rc::new(RefCell::new(value)));
                    }
                    return eval(body, &scope, program, depth);
                }
            }
            return Err(Diagnostic::new(
                "E204",
                expr.span,
                "unmatched runtime value",
            ));
        }
        ExprKind::Block(stmts, tail) => {
            let mut scope = env.clone();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } => {
                        let value = take_value!(eval(value, &scope, program, depth));
                        scope.insert(name.clone(), Rc::new(RefCell::new(value)));
                    }
                    Stmt::Assign { name, value, span } => {
                        let value = take_value!(eval(value, &scope, program, depth));
                        let cell = scope.get(name).ok_or_else(|| {
                            Diagnostic::new(
                                "E204",
                                *span,
                                format!("unresolved runtime name {name}"),
                            )
                        })?;
                        *cell.borrow_mut() = value;
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        span,
                    } => {
                        let iterable = take_value!(eval(iterable, &scope, program, depth));
                        let Value::Array(values) = iterable else {
                            return Err(Diagnostic::new("E204", *span, "invalid runtime iterable"));
                        };
                        for value in values {
                            let mut loop_scope = scope.clone();
                            loop_scope.insert(name.clone(), Rc::new(RefCell::new(value)));
                            take_value!(eval(body, &loop_scope, program, depth));
                        }
                    }
                    Stmt::Return { value, .. } => {
                        let value = take_value!(eval(value, &scope, program, depth));
                        return Ok(Flow::Return(value));
                    }
                    Stmt::Expr(value) => {
                        take_value!(eval(value, &scope, program, depth));
                    }
                }
            }
            if let Some(tail) = tail {
                return eval(tail, &scope, program, depth);
            }
            Value::Unit
        }
    };
    Ok(Flow::Value(value))
}

fn binary(left: Value, op: Op, right: Value, span: Span) -> Result<Value, Diagnostic> {
    if let (Value::String(a), Op::Add, Value::String(b)) = (&left, op, &right) {
        return Ok(Value::String(format!("{a}{b}")));
    }
    if matches!(op, Op::Eq | Op::Ne) {
        return Ok(Value::Bool(if op == Op::Eq {
            left == right
        } else {
            left != right
        }));
    }
    let (Value::I32(a), Value::I32(b)) = (left, right) else {
        return Err(Diagnostic::new("E204", span, "invalid runtime operands"));
    };
    let number = match op {
        Op::Add => a.checked_add(b),
        Op::Sub => a.checked_sub(b),
        Op::Mul => a.checked_mul(b),
        Op::Div => a.checked_div(b),
        Op::Lt => return Ok(Value::Bool(a < b)),
        Op::Le => return Ok(Value::Bool(a <= b)),
        Op::Gt => return Ok(Value::Bool(a > b)),
        Op::Ge => return Ok(Value::Bool(a >= b)),
        Op::Eq | Op::Ne => return Err(Diagnostic::new("E204", span, "invalid runtime operator")),
    };
    number
        .map(Value::I32)
        .ok_or_else(|| Diagnostic::new("E201", span, "integer overflow or division by zero"))
}

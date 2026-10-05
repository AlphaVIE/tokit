use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use crate::ast::{Expr, ExprKind, Function, Op, PatternKind, Program, Span, Stmt};
use crate::builtins;
use crate::diagnostic::Diagnostic;
use crate::filesystem::{ReadPolicy, WritePolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    I32(i32),
    I64(i64),
    Bool(bool),
    String(String),
    Bytes(Arc<Vec<u8>>),
    Array(Vec<Value>),
    Record(String, Vec<(String, Value)>),
    Enum(String, String, Option<Box<Value>>),
    Ok(Box<Value>),
    Err(Box<Value>),
    Some(Box<Value>),
    None,
    Task(Box<Value>),
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I32(n) => write!(f, "{n}"),
            Self::I64(n) => write!(f, "{n}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::String(value) => write!(f, "{value:?}"),
            Self::Bytes(values) => write!(f, "Bytes({values:?})"),
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
            Self::Enum(name, variant, payload) => {
                write!(f, "{name}::{variant}")?;
                if let Some(value) = payload {
                    write!(f, "({value})")?;
                }
                Ok(())
            }
            Self::Ok(value) => write!(f, "Ok({value})"),
            Self::Err(value) => write!(f, "Err({value})"),
            Self::Some(value) => write!(f, "Some({value})"),
            Self::None => f.write_str("None"),
            Self::Task(_) => f.write_str("<task>"),
            Self::Unit => f.write_str("()"),
        }
    }
}

enum Flow {
    Value(Value),
    Return(Value),
    Break,
    Continue,
}

type Env = HashMap<String, Rc<RefCell<Value>>>;
const MAX_CALL_DEPTH: usize = 32;

struct Runtime<'a> {
    read: ReadPolicy,
    write: WritePolicy,
    args: &'a [String],
}

macro_rules! take_value {
    ($expr:expr) => {
        match $expr? {
            Flow::Value(value) => value,
            Flow::Return(value) => return Ok(Flow::Return(value)),
            Flow::Break => return Ok(Flow::Break),
            Flow::Continue => return Ok(Flow::Continue),
        }
    };
}

pub fn run(program: &Program) -> Result<Value, Diagnostic> {
    run_with_read_root(program, None)
}

pub fn run_with_read_root(program: &Program, root: Option<&Path>) -> Result<Value, Diagnostic> {
    run_with_runtime_args(program, root, &[])
}

pub fn run_with_runtime_args(
    program: &Program,
    root: Option<&Path>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    run_with_capabilities(program, root, None, args)
}

pub fn run_with_capabilities(
    program: &Program,
    read_root: Option<&Path>,
    write_root: Option<&Path>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    run_entry(program, "main", read_root, write_root, args)
}

/// Execute a checked, parameterless function in a fresh reference runtime.
pub fn run_named(program: &Program, name: &str, root: Option<&Path>) -> Result<Value, Diagnostic> {
    run_named_with_capabilities(program, name, root, None)
}

pub fn run_named_with_capabilities(
    program: &Program,
    name: &str,
    read_root: Option<&Path>,
    write_root: Option<&Path>,
) -> Result<Value, Diagnostic> {
    run_entry(program, name, read_root, write_root, &[])
}

fn run_entry(
    program: &Program,
    name: &str,
    read_root: Option<&Path>,
    write_root: Option<&Path>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    let runtime = Runtime {
        read: ReadPolicy::from_root(read_root),
        write: WritePolicy::from_root(write_root),
        args,
    };
    let function = program
        .functions
        .iter()
        .find(|function| function.name == name)
        .ok_or_else(|| {
            Diagnostic::new("E203", Span::new(0, 0), format!("missing {name} function"))
        })?;
    if !function.params.is_empty() || !function.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E203",
            function.span,
            format!("{name} must have no parameters or type parameters"),
        ));
    }
    // The reference evaluator uses the host call stack. Give its fixed depth
    // limit enough room even on hosts with small default thread stacks.
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(scope, || invoke(program, function, Vec::new(), 0, &runtime))
            .map_err(|error| {
                Diagnostic::new(
                    "E204",
                    function.span,
                    format!("cannot start evaluator: {error}"),
                )
            })?;
        handle
            .join()
            .map_err(|_| Diagnostic::new("E204", function.span, "reference evaluator panicked"))?
    })
}

fn invoke(
    program: &Program,
    function: &Function,
    args: Vec<Value>,
    depth: usize,
    runtime: &Runtime<'_>,
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
    match eval(&function.body, &env, program, depth, runtime)? {
        Flow::Value(value) | Flow::Return(value) => Ok(value),
        Flow::Break | Flow::Continue => Err(Diagnostic::new(
            "E204",
            function.span,
            "loop control escaped its loop",
        )),
    }
}

fn eval_utf8_builtin(
    name: &str,
    values: &[Value],
    span: Span,
) -> Option<Result<Value, Diagnostic>> {
    if name == builtins::UTF8_BYTES {
        let [Value::String(text)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid utf8_bytes call",
            )));
        };
        return Some(Ok(Value::Array(
            text.bytes()
                .map(|byte| Value::I32(i32::from(byte)))
                .collect(),
        )));
    }
    if name == builtins::UTF8_DECODE {
        let [Value::Array(items)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid utf8_decode call",
            )));
        };
        let bytes = items
            .iter()
            .map(|item| match item {
                Value::I32(value) => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        return Some(Ok(bytes
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map_or(Value::None, |text| {
                Value::Some(Box::new(Value::String(text)))
            })));
    }
    if name == builtins::UTF8_ENCODE {
        let [Value::String(text)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid utf8_encode call",
            )));
        };
        return Some(Ok(Value::Bytes(Arc::new(text.as_bytes().to_vec()))));
    }
    if name == builtins::UTF8_DECODE_BYTES {
        let [Value::Bytes(bytes)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid utf8_decode_bytes call",
            )));
        };
        return Some(Ok(String::from_utf8(bytes.as_ref().clone())
            .ok()
            .map_or(Value::None, |text| {
                Value::Some(Box::new(Value::String(text)))
            })));
    }
    if name == builtins::BYTES_FROM_I32 {
        let [Value::Array(items)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid bytes_from_i32 call",
            )));
        };
        let bytes = items
            .iter()
            .map(|item| match item {
                Value::I32(value) => u8::try_from(*value).ok(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        return Some(Ok(bytes.map_or(Value::None, |bytes| {
            Value::Some(Box::new(Value::Bytes(Arc::new(bytes))))
        })));
    }
    if name == builtins::BYTES_TO_I32 {
        let [Value::Bytes(bytes)] = values else {
            return Some(Err(Diagnostic::new(
                "E204",
                span,
                "invalid bytes_to_i32 call",
            )));
        };
        return Some(Ok(Value::Array(
            bytes
                .iter()
                .map(|byte| Value::I32(i32::from(*byte)))
                .collect(),
        )));
    }
    None
}

fn array_length(value: &Value, span: Span) -> Result<Value, Diagnostic> {
    let length = match value {
        Value::Array(items) => items.len(),
        Value::Bytes(items) => items.len(),
        _ => return Err(Diagnostic::new("E204", span, "invalid len call")),
    };
    let length = i32::try_from(length)
        .map_err(|_| Diagnostic::new("E206", span, "array length exceeds i32"))?;
    Ok(Value::I32(length))
}

fn eval(
    expr: &Expr,
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
    let value = match &expr.kind {
        ExprKind::Int(number) => Value::I32(*number),
        ExprKind::I64(number) => Value::I64(*number),
        ExprKind::Neg(inner) => match take_value!(eval(inner, env, program, depth, runtime)) {
            Value::I32(value) => Value::I32(value.checked_neg().ok_or_else(|| {
                Diagnostic::new("E201", expr.span, "integer overflow or division by zero")
            })?),
            Value::I64(value) => Value::I64(value.checked_neg().ok_or_else(|| {
                Diagnostic::new("E201", expr.span, "integer overflow or division by zero")
            })?),
            _ => return Err(Diagnostic::new("E204", expr.span, "invalid negation value")),
        },
        ExprKind::Bool(value) => Value::Bool(*value),
        ExprKind::String(value) => Value::String(value.clone()),
        ExprKind::Variant(name, variant, payload) => {
            let payload = match payload {
                Some(value) => Some(Box::new(take_value!(eval(
                    value, env, program, depth, runtime
                )))),
                None => None,
            };
            Value::Enum(name.clone(), variant.clone(), payload)
        }
        ExprKind::Array(items) => {
            let mut values = Vec::new();
            for item in items {
                values.push(take_value!(eval(item, env, program, depth, runtime)));
            }
            Value::Array(values)
        }
        ExprKind::Index(array, index) => {
            // A variable read normally copies its value. Indexing only needs
            // the selected element. Borrow only when evaluating the index cannot
            // mutate the binding; otherwise preserve the pre-index value snapshot.
            let borrowed = match &array.kind {
                ExprKind::Var(name) if index.is_simple_read() => Some(
                    env.get(name)
                        .ok_or_else(|| {
                            Diagnostic::new(
                                "E204",
                                array.span,
                                format!("unresolved runtime name {name}"),
                            )
                        })?
                        .borrow(),
                ),
                _ => None,
            };
            let owned = if borrowed.is_none() {
                Some(take_value!(eval(array, env, program, depth, runtime)))
            } else {
                None
            };
            let position = take_value!(eval(index, env, program, depth, runtime));
            let Value::I32(position) = position else {
                return Err(Diagnostic::new("E204", expr.span, "invalid runtime index"));
            };
            let position = usize::try_from(position).ok();
            let values = borrowed.as_deref().or(owned.as_ref());
            match values.expect("array expression has a value") {
                Value::Array(values) => position.and_then(|at| values.get(at)).cloned(),
                Value::Bytes(values) => position
                    .and_then(|at| values.get(at))
                    .map(|byte| Value::I32(i32::from(*byte))),
                _ => return Err(Diagnostic::new("E204", expr.span, "invalid runtime index")),
            }
            .ok_or_else(|| Diagnostic::new("E205", expr.span, "array index out of bounds"))?
        }
        ExprKind::Field(value, field) => {
            let value = take_value!(eval(value, env, program, depth, runtime));
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
        ExprKind::Ok(inner) => Value::Ok(Box::new(take_value!(eval(
            inner, env, program, depth, runtime
        )))),
        ExprKind::Err(inner) => Value::Err(Box::new(take_value!(eval(
            inner, env, program, depth, runtime
        )))),
        ExprKind::Some(inner) => Value::Some(Box::new(take_value!(eval(
            inner, env, program, depth, runtime
        )))),
        ExprKind::None => Value::None,
        ExprKind::Try(inner) => {
            let value = take_value!(eval(inner, env, program, depth, runtime));
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
            let left = take_value!(eval(left, env, program, depth, runtime));
            let right = take_value!(eval(right, env, program, depth, runtime));
            binary(left, *op, right, expr.span)?
        }
        ExprKind::Call(name, args) => {
            if name == builtins::LEN
                && args.len() == 1
                && let ExprKind::Var(variable) = &args[0].kind
            {
                let binding = env.get(variable).ok_or_else(|| {
                    Diagnostic::new(
                        "E204",
                        args[0].span,
                        format!("unresolved runtime name {variable}"),
                    )
                })?;
                return Ok(Flow::Value(array_length(&binding.borrow(), expr.span)?));
            }
            let mut values = Vec::new();
            for arg in args {
                values.push(take_value!(eval(arg, env, program, depth, runtime)));
            }
            if let Some(value) = eval_utf8_builtin(name, &values, expr.span) {
                return Ok(Flow::Value(value?));
            }
            if name == builtins::READ_TEXT {
                let [Value::String(path)] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid read_text call"));
                };
                return Ok(Flow::Value(match runtime.read.read_text(path) {
                    Ok(value) => Value::Ok(Box::new(Value::String(value))),
                    Err(error) => Value::Err(Box::new(Value::Enum(
                        builtins::IO_ERROR.to_owned(),
                        error.variant().to_owned(),
                        None,
                    ))),
                }));
            }
            if name == builtins::READ_BYTES {
                let [Value::String(path)] = values.as_slice() else {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid read_bytes call",
                    ));
                };
                return Ok(Flow::Value(match runtime.read.read_bytes(path) {
                    Ok(bytes) => Value::Ok(Box::new(Value::Bytes(Arc::new(bytes)))),
                    Err(error) => Value::Err(Box::new(Value::Enum(
                        builtins::IO_ERROR.to_owned(),
                        error.variant().to_owned(),
                        None,
                    ))),
                }));
            }
            if name == builtins::WRITE_TEXT {
                let [Value::String(path), Value::String(text)] = values.as_slice() else {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid write_text call",
                    ));
                };
                return Ok(Flow::Value(match runtime.write.write_text(path, text) {
                    Ok(()) => Value::Ok(Box::new(Value::Unit)),
                    Err(error) => Value::Err(Box::new(Value::Enum(
                        builtins::IO_ERROR.to_owned(),
                        error.variant().to_owned(),
                        None,
                    ))),
                }));
            }
            if name == builtins::WRITE_BYTES {
                let [Value::String(path), Value::Bytes(bytes)] = values.as_slice() else {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid write_bytes call",
                    ));
                };
                return Ok(Flow::Value(match runtime.write.write_bytes(path, bytes) {
                    Ok(()) => Value::Ok(Box::new(Value::Unit)),
                    Err(error) => Value::Err(Box::new(Value::Enum(
                        builtins::IO_ERROR.to_owned(),
                        error.variant().to_owned(),
                        None,
                    ))),
                }));
            }
            if name == builtins::LINES {
                let [Value::String(text)] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid lines call"));
                };
                return Ok(Flow::Value(Value::Array(
                    text.lines()
                        .map(|line| Value::String(line.to_owned()))
                        .collect(),
                )));
            }
            if name == builtins::ARGS {
                return Ok(Flow::Value(Value::Array(
                    runtime.args.iter().cloned().map(Value::String).collect(),
                )));
            }
            if name == builtins::LEN {
                let [value] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid len call"));
                };
                return Ok(Flow::Value(array_length(value, expr.span)?));
            }
            if name == builtins::PARSE_I32 || name == builtins::PARSE_I64 {
                let [Value::String(text)] = values.as_slice() else {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid integer parse call",
                    ));
                };
                let digits = text
                    .strip_prefix('+')
                    .or_else(|| text.strip_prefix('-'))
                    .unwrap_or(text);
                if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Ok(Flow::Value(Value::Err(Box::new(Value::Enum(
                        builtins::PARSE_ERROR.to_owned(),
                        "Invalid".to_owned(),
                        None,
                    )))));
                }
                let parsed = if name == builtins::PARSE_I32 {
                    text.parse::<i32>().map(Value::I32)
                } else {
                    text.parse::<i64>().map(Value::I64)
                };
                return Ok(Flow::Value(match parsed {
                    Ok(number) => Value::Ok(Box::new(number)),
                    Err(error) => {
                        let variant = match error.kind() {
                            std::num::IntErrorKind::PosOverflow
                            | std::num::IntErrorKind::NegOverflow => "OutOfRange",
                            _ => "Invalid",
                        };
                        Value::Err(Box::new(Value::Enum(
                            builtins::PARSE_ERROR.to_owned(),
                            variant.to_owned(),
                            None,
                        )))
                    }
                }));
            }
            if name == builtins::WIDEN_I64 {
                let [Value::I32(number)] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid i64 conversion"));
                };
                return Ok(Flow::Value(Value::I64(i64::from(*number))));
            }
            if name == builtins::NARROW_I32 {
                let [Value::I64(number)] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid i32 conversion"));
                };
                return Ok(Flow::Value(match i32::try_from(*number) {
                    Ok(value) => Value::Some(Box::new(Value::I32(value))),
                    Err(_) => Value::None,
                }));
            }
            if name == builtins::JOIN {
                let [Value::Task(value)] = values.as_slice() else {
                    return Err(Diagnostic::new("E204", expr.span, "invalid join call"));
                };
                return Ok(Flow::Value(Value::Ok(value.clone())));
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
            invoke(program, function, values, depth + 1, runtime)?
        }
        ExprKind::Spawn(call) => {
            let value = take_value!(eval(call, env, program, depth, runtime));
            Value::Task(Box::new(value))
        }
        ExprKind::If(condition, yes, no) => {
            let condition = take_value!(eval(condition, env, program, depth, runtime));
            let branch = if condition == Value::Bool(true) {
                yes
            } else {
                no
            };
            return eval(branch, env, program, depth, runtime);
        }
        ExprKind::Match(value, arms) => {
            let scrutinee = take_value!(eval(value, env, program, depth, runtime));
            for (pattern, body) in arms {
                let binding = match (&pattern.kind, &scrutinee) {
                    (PatternKind::Int(pattern), Value::I32(value)) if pattern == value => {
                        Some(None)
                    }
                    (PatternKind::I64(pattern), Value::I64(value)) if pattern == value => {
                        Some(None)
                    }
                    (PatternKind::Wildcard, _) => Some(None),
                    (PatternKind::Ok(name), Value::Ok(value)) => {
                        Some(Some((name.clone(), *value.clone())))
                    }
                    (PatternKind::Err(name), Value::Err(value)) => {
                        Some(Some((name.clone(), *value.clone())))
                    }
                    (PatternKind::Some(name), Value::Some(value)) => {
                        Some(Some((name.clone(), *value.clone())))
                    }
                    (PatternKind::None, Value::None) => Some(None),
                    (PatternKind::Bool(pattern), Value::Bool(value)) if pattern == value => {
                        Some(None)
                    }
                    (
                        PatternKind::Variant(name, variant, binding),
                        Value::Enum(actual, value, payload),
                    ) if name == actual && variant == value => match (binding, payload) {
                        (None, None) => Some(None),
                        (Some(binding), Some(payload)) => {
                            Some(Some((binding.clone(), *payload.clone())))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(binding) = binding {
                    let mut scope = env.clone();
                    if let Some((name, value)) = binding {
                        scope.insert(name, Rc::new(RefCell::new(value)));
                    }
                    return eval(body, &scope, program, depth, runtime);
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
                        let value = take_value!(eval(value, &scope, program, depth, runtime));
                        scope.insert(name.clone(), Rc::new(RefCell::new(value)));
                    }
                    Stmt::Assign { name, value, span } => {
                        let value = take_value!(eval(value, &scope, program, depth, runtime));
                        let cell = scope.get(name).ok_or_else(|| {
                            Diagnostic::new(
                                "E204",
                                *span,
                                format!("unresolved runtime name {name}"),
                            )
                        })?;
                        *cell.borrow_mut() = value;
                    }
                    Stmt::Push { name, value, span } => {
                        let value = take_value!(eval(value, &scope, program, depth, runtime));
                        let cell = scope.get(name).ok_or_else(|| {
                            Diagnostic::new(
                                "E204",
                                *span,
                                format!("unresolved runtime name {name}"),
                            )
                        })?;
                        let mut array = cell.borrow_mut();
                        match (&mut *array, value) {
                            (Value::Array(items), value) => items.push(value),
                            (Value::String(text), Value::String(piece)) => text.push_str(&piece),
                            (Value::Bytes(items), Value::I32(value)) => {
                                let byte = u8::try_from(value).map_err(|_| {
                                    Diagnostic::new("E207", *span, "byte value outside 0..255")
                                })?;
                                Arc::make_mut(items).push(byte);
                            }
                            _ => {
                                return Err(Diagnostic::new("E204", *span, "invalid runtime push"));
                            }
                        }
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        span,
                    } => {
                        let iterable = take_value!(eval(iterable, &scope, program, depth, runtime));
                        let values = match iterable {
                            Value::Array(values) => values,
                            Value::Bytes(bytes) => bytes
                                .iter()
                                .map(|byte| Value::I32(i32::from(*byte)))
                                .collect(),
                            _ => {
                                return Err(Diagnostic::new(
                                    "E204",
                                    *span,
                                    "invalid runtime iterable",
                                ));
                            }
                        };
                        for value in values {
                            let mut loop_scope = scope.clone();
                            loop_scope.insert(name.clone(), Rc::new(RefCell::new(value)));
                            match eval(body, &loop_scope, program, depth, runtime)? {
                                Flow::Value(_) | Flow::Continue => {}
                                Flow::Break => break,
                                Flow::Return(value) => return Ok(Flow::Return(value)),
                            }
                        }
                    }
                    Stmt::While {
                        condition,
                        body,
                        span,
                    } => loop {
                        let value = take_value!(eval(condition, &scope, program, depth, runtime));
                        let Value::Bool(keep_going) = value else {
                            return Err(Diagnostic::new(
                                "E204",
                                *span,
                                "invalid runtime while condition",
                            ));
                        };
                        if !keep_going {
                            break;
                        }
                        match eval(body, &scope, program, depth, runtime)? {
                            Flow::Value(_) => {}
                            Flow::Continue => continue,
                            Flow::Break => break,
                            Flow::Return(value) => return Ok(Flow::Return(value)),
                        }
                    },
                    Stmt::Break { .. } => return Ok(Flow::Break),
                    Stmt::Continue { .. } => return Ok(Flow::Continue),
                    Stmt::Return { value, .. } => {
                        let value = take_value!(eval(value, &scope, program, depth, runtime));
                        return Ok(Flow::Return(value));
                    }
                    Stmt::Expr(value) => {
                        take_value!(eval(value, &scope, program, depth, runtime));
                    }
                }
            }
            if let Some(tail) = tail {
                return eval(tail, &scope, program, depth, runtime);
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
    let number = match (left, right) {
        (Value::I32(a), Value::I32(b)) => {
            let value = match op {
                Op::Add => a.checked_add(b),
                Op::Sub => a.checked_sub(b),
                Op::Mul => a.checked_mul(b),
                Op::Div => a.checked_div(b),
                Op::Lt => return Ok(Value::Bool(a < b)),
                Op::Le => return Ok(Value::Bool(a <= b)),
                Op::Gt => return Ok(Value::Bool(a > b)),
                Op::Ge => return Ok(Value::Bool(a >= b)),
                Op::Eq | Op::Ne => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
            };
            value.map(Value::I32)
        }
        (Value::I64(a), Value::I64(b)) => {
            let value = match op {
                Op::Add => a.checked_add(b),
                Op::Sub => a.checked_sub(b),
                Op::Mul => a.checked_mul(b),
                Op::Div => a.checked_div(b),
                Op::Lt => return Ok(Value::Bool(a < b)),
                Op::Le => return Ok(Value::Bool(a <= b)),
                Op::Gt => return Ok(Value::Bool(a > b)),
                Op::Ge => return Ok(Value::Bool(a >= b)),
                Op::Eq | Op::Ne => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
            };
            value.map(Value::I64)
        }
        _ => return Err(Diagnostic::new("E204", span, "invalid runtime operands")),
    };
    number.ok_or_else(|| Diagnostic::new("E201", span, "integer overflow or division by zero"))
}

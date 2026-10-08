use std::path::Path;
use std::sync::Arc;
use std::{cell::RefCell, rc::Rc};

use crate::ast::{
    Expr, ExprKind, Function, Op, Pattern, PatternKind, PlaceStep, Program, Span, Stmt,
};
use crate::builtins;
use crate::diagnostic::Diagnostic;
use crate::filesystem::{ReadPolicy, WritePolicy};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F64(u64),
    Bool(bool),
    String(String),
    Bytes(Arc<Vec<u8>>),
    Array(Vec<Value>),
    Record(String, Vec<(String, Value)>),
    /// Entries sorted by key; keys are i32, i64, String, or bool.
    Map(Vec<(Value, Value)>),
    Enum(String, String, Option<Box<Value>>),
    Ok(Box<Value>),
    Err(Box<Value>),
    Some(Box<Value>),
    None,
    Task(Box<Value>),
    Closure(Arc<Closure>),
    Conn(Socket),
    Listener(ListenerHandle),
    Unit,
}

/// A TCP connection handle; copies share one socket and compare by identity.
#[derive(Clone)]
pub struct Socket(crate::http::__TokSocket);

impl PartialEq for Socket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Socket {}

impl std::fmt::Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Socket")
    }
}

/// A listening socket; copies share it and compare by identity.
#[derive(Clone)]
pub struct ListenerHandle(crate::http::__TokListener);

impl PartialEq for ListenerHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ListenerHandle {}

impl std::fmt::Debug for ListenerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Listener")
    }
}

/// A lambda value with copies of the locals it reads.
#[derive(Debug)]
pub struct Closure {
    params: Vec<String>,
    body: Arc<Expr>,
    captured: Vec<(String, Value)>,
}

/// Closures compare by identity; the language exposes no closure equality.
impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for Closure {}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I32(n) => write!(f, "{n}"),
            Self::I64(n) => write!(f, "{n}"),
            Self::F64(bits) => write!(f, "{:?}", f64::from_bits(*bits)),
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
            Self::Closure(_) => f.write_str("<fn>"),
            Self::Conn(_) => f.write_str("<conn>"),
            Self::Listener(_) => f.write_str("<listener>"),
            Self::Map(entries) => {
                f.write_str("{")?;
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{key}:{value}")?;
                }
                f.write_str("}")
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

/// Local bindings as a persistent list. Cloning is O(1) and a binding added to
/// a clone stays invisible to the original, which is exactly the behavior of
/// the copied maps this replaces; lookups find the innermost binding first.
#[derive(Clone, Default)]
struct Env(Option<Rc<EnvNode>>);

struct EnvNode {
    name: String,
    cell: Rc<RefCell<Value>>,
    parent: Env,
}

impl Env {
    fn get(&self, name: &str) -> Option<&Rc<RefCell<Value>>> {
        let mut current = &self.0;
        while let Some(node) = current {
            if node.name == name {
                return Some(&node.cell);
            }
            current = &node.parent.0;
        }
        None
    }

    fn insert(&mut self, name: String, cell: Rc<RefCell<Value>>) {
        let parent = std::mem::take(self);
        *self = Env(Some(Rc::new(EnvNode { name, cell, parent })));
    }
}

impl FromIterator<(String, Rc<RefCell<Value>>)> for Env {
    fn from_iter<T: IntoIterator<Item = (String, Rc<RefCell<Value>>)>>(items: T) -> Self {
        let mut env = Env::default();
        for (name, cell) in items {
            env.insert(name, cell);
        }
        env
    }
}
const MAX_CALL_DEPTH: usize = 10_000;
/// Each evaluator thread runs at most this many nested calls; deeper calls
/// continue on a fresh thread so recursion never depends on one host stack.
const CALLS_PER_STACK_SEGMENT: usize = 256;
/// Unoptimized builds need up to ~100 KiB of host stack per Tokit call.
const STACK_SEGMENT_BYTES: usize = 64 * 1024 * 1024;

/// Pseudo-diagnostic code that carries a status requested by `exit`.
const EXIT_REQUEST: &str = "X000";

/// The status of an `exit` call that ended evaluation, if any.
pub fn exit_status(diagnostic: &Diagnostic) -> Option<i32> {
    (diagnostic.code == EXIT_REQUEST)
        .then(|| diagnostic.message.parse().ok())
        .flatten()
}

struct Runtime<'a> {
    read: ReadPolicy,
    write: WritePolicy,
    args: &'a [String],
    /// Origin of the monotonic `clock_ns` builtin.
    started: std::time::Instant,
    /// `--allow-net` grant: one `host:port`, or `*`.
    net: Option<String>,
    /// Declared functions and records by name, so calls do not scan the program.
    functions: std::collections::HashMap<&'a str, &'a Function>,
    records: std::collections::HashSet<&'a str>,
}

/// Capability grants for one program run.
#[derive(Clone, Copy, Debug, Default)]
pub struct Grants<'a> {
    pub read: Option<&'a Path>,
    pub write: Option<&'a Path>,
    pub net: Option<&'a str>,
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
    let grants = Grants {
        read: read_root,
        write: write_root,
        net: None,
    };
    run_entry(program, "main", grants, args)
}

/// Run `main` with every capability grant, including network access.
pub fn run_with_grants(
    program: &Program,
    grants: Grants<'_>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    run_entry(program, "main", grants, args)
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
    let grants = Grants {
        read: read_root,
        write: write_root,
        net: None,
    };
    run_entry(program, name, grants, &[])
}

fn run_entry(
    program: &Program,
    name: &str,
    grants: Grants<'_>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    let runtime = Runtime {
        functions: program
            .functions
            .iter()
            .map(|function| (function.name.as_str(), function))
            .collect(),
        records: program
            .records
            .iter()
            .map(|record| record.name.as_str())
            .collect(),
        read: ReadPolicy::from_root(grants.read),
        write: WritePolicy::from_root(grants.write),
        args,
        started: std::time::Instant::now(),
        net: grants.net.map(str::to_owned),
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
    // The reference evaluator uses the host call stack; `invoke` moves to a
    // new stack segment periodically so only the depth limit bounds recursion.
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(STACK_SEGMENT_BYTES)
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
    if depth > 0 && depth.is_multiple_of(CALLS_PER_STACK_SEGMENT) {
        return std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(STACK_SEGMENT_BYTES)
                .spawn_scoped(scope, || {
                    invoke_frame(program, function, args, depth, runtime)
                })
                .map_err(|error| {
                    Diagnostic::new(
                        "E204",
                        function.span,
                        format!("cannot extend evaluator stack: {error}"),
                    )
                })?
                .join()
                .map_err(|_| {
                    Diagnostic::new("E204", function.span, "reference evaluator panicked")
                })?
        });
    }
    invoke_frame(program, function, args, depth, runtime)
}

fn invoke_frame(
    program: &Program,
    function: &Function,
    args: Vec<Value>,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Value, Diagnostic> {
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
        Value::Map(entries) => entries.len(),
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
        ExprKind::Apply(callee, args) => {
            let Value::Closure(closure) = take_value!(eval(callee, env, program, depth, runtime))
            else {
                return Err(Diagnostic::new(
                    "E204",
                    expr.span,
                    "called a non-function value",
                ));
            };
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(take_value!(eval(arg, env, program, depth, runtime)));
            }
            call_closure(&closure, values, program, depth, runtime, expr.span)?
        }
        ExprKind::Lambda(params, body) => {
            let captured = body
                .free_names()
                .into_iter()
                .filter(|name| !params.iter().any(|(param, _)| param == name))
                .filter_map(|name| {
                    let value = env.get(&name)?.borrow().clone();
                    Some((name, value))
                })
                .collect();
            Value::Closure(Arc::new(Closure {
                params: params.iter().map(|(name, _)| name.clone()).collect(),
                body: Arc::clone(body),
                captured,
            }))
        }
        ExprKind::Int(number) => Value::I32(*number),
        ExprKind::I64(number) => Value::I64(*number),
        ExprKind::F64(bits) => Value::F64(*bits),
        ExprKind::Not(inner) => match take_value!(eval(inner, env, program, depth, runtime)) {
            Value::Bool(value) => Value::Bool(!value),
            _ => {
                return Err(Diagnostic::new(
                    "E204",
                    expr.span,
                    "invalid logical negation",
                ));
            }
        },
        ExprKind::Neg(inner) => match take_value!(eval(inner, env, program, depth, runtime)) {
            Value::I32(value) => Value::I32(value.checked_neg().ok_or_else(|| {
                Diagnostic::new("E201", expr.span, "integer overflow or division by zero")
            })?),
            Value::I64(value) => Value::I64(value.checked_neg().ok_or_else(|| {
                Diagnostic::new("E201", expr.span, "integer overflow or division by zero")
            })?),
            Value::F64(bits) => Value::F64((-f64::from_bits(bits)).to_bits()),
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
            return eval_index(expr, array, index, env, program, depth, runtime);
        }
        ExprKind::Field(value, field) => {
            return eval_field(expr, value, field, env, program, depth, runtime);
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
                Value::Ok(value) | Value::Some(value) => *value,
                Value::Err(value) => return Ok(Flow::Return(Value::Err(value))),
                Value::None => return Ok(Flow::Return(Value::None)),
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
            if matches!(op, Op::And | Op::Or) {
                let Value::Bool(left) = left else {
                    return Err(Diagnostic::new(
                        "E204",
                        expr.span,
                        "invalid logical operand",
                    ));
                };
                if (*op == Op::And && !left) || (*op == Op::Or && left) {
                    Value::Bool(left)
                } else {
                    match take_value!(eval(right, env, program, depth, runtime)) {
                        Value::Bool(right) => Value::Bool(right),
                        _ => {
                            return Err(Diagnostic::new(
                                "E204",
                                expr.span,
                                "invalid logical operand",
                            ));
                        }
                    }
                }
            } else {
                let right = take_value!(eval(right, env, program, depth, runtime));
                binary(left, *op, right, expr.span)?
            }
        }
        ExprKind::Call(name, args) => {
            return eval_call(expr, name, args, env, program, depth, runtime);
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
            return eval_match(expr, value, arms, env, program, depth, runtime);
        }
        ExprKind::Block(stmts, tail) => {
            return eval_block(stmts, tail, env, program, depth, runtime);
        }
    };
    Ok(Flow::Value(value))
}

fn eval_index(
    expr: &Expr,
    array: &Expr,
    index: &Expr,
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
    let value = {
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
    };
    Ok(Flow::Value(value))
}

fn eval_field(
    expr: &Expr,
    value: &Expr,
    field: &str,
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
    let value = {
        if let ExprKind::Var(name) = &value.kind {
            let binding = env.get(name).ok_or_else(|| {
                Diagnostic::new(
                    "E204",
                    value.span,
                    format!("unresolved runtime name {name}"),
                )
            })?;
            let borrowed = binding.borrow();
            let Value::Record(_, fields) = &*borrowed else {
                return Err(Diagnostic::new(
                    "E204",
                    expr.span,
                    "invalid runtime field access",
                ));
            };
            let selected = fields
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| Diagnostic::new("E204", expr.span, "unknown runtime field"))?;
            return Ok(Flow::Value(selected));
        }
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
    };
    Ok(Flow::Value(value))
}

fn eval_call(
    expr: &Expr,
    name: &String,
    args: &[Expr],
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
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
    // Functions, records, and builtins take precedence over local values.
    let function = runtime.functions.get(name.as_str()).copied();
    let declared =
        function.is_some() || builtins::is_call(name) || runtime.records.contains(name.as_str());
    if !declared && let Some(binding) = env.get(name) {
        let Value::Closure(closure) = binding.borrow().clone() else {
            return Err(Diagnostic::new(
                "E204",
                expr.span,
                "called a non-function value",
            ));
        };
        return Ok(Flow::Value(call_closure(
            &closure, values, program, depth, runtime, expr.span,
        )?));
    }
    if let Some(result) = eval_higher_order(name, &values, program, depth, runtime, expr.span) {
        return Ok(Flow::Value(result?));
    }
    if let Some(result) = eval_http(name, &values, program, depth, runtime, expr.span) {
        return Ok(Flow::Value(result?));
    }
    let Some(function) = function else {
        return eval_builtin(expr, name, values, program, runtime);
    };
    Ok(Flow::Value(invoke(
        program,
        function,
        values,
        depth + 1,
        runtime,
    )?))
}

/// Builtins and record constructors never recurse into user code, so they
/// stay out of the evaluator's recursive stack frames.
#[inline(never)]
fn eval_builtin(
    expr: &Expr,
    name: &String,
    values: Vec<Value>,
    program: &Program,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
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
    if let Some(value) = eval_map_builtin(name, &values) {
        return Ok(Flow::Value(value));
    }
    if let Some(value) = eval_crypto_builtin(name, &values) {
        return Ok(Flow::Value(value));
    }
    if let Some(value) = eval_bit_builtin(name, &values, expr.span) {
        return Ok(Flow::Value(value?));
    }
    if let Some(value) = eval_math_builtin(name, &values, expr.span) {
        return Ok(Flow::Value(value?));
    }
    if let Some(value) = eval_array_builtin(name, &values, expr.span) {
        return Ok(Flow::Value(value?));
    }
    if let Some(value) = eval_string_builtin(name, &values) {
        return Ok(Flow::Value(value));
    }
    let io_error = |error: crate::filesystem::IoError| {
        Value::Err(Box::new(Value::Enum(
            builtins::IO_ERROR.to_owned(),
            error.variant().to_owned(),
            None,
        )))
    };
    match (name.as_str(), values.as_slice()) {
        (builtins::LIST_DIR, [Value::String(path)]) => {
            return Ok(Flow::Value(match runtime.read.list_dir(path) {
                Ok(names) => Value::Ok(Box::new(Value::Array(
                    names.into_iter().map(Value::String).collect(),
                ))),
                Err(error) => io_error(error),
            }));
        }
        (builtins::EXISTS, [Value::String(path)]) => {
            return Ok(Flow::Value(Value::Bool(runtime.read.exists(path))));
        }
        (builtins::MAKE_DIR | builtins::REMOVE_FILE, [Value::String(path)]) => {
            let result = if name == builtins::MAKE_DIR {
                runtime.write.make_dir(path)
            } else {
                runtime.write.remove_file(path)
            };
            return Ok(Flow::Value(match result {
                Ok(()) => Value::Ok(Box::new(Value::Unit)),
                Err(error) => io_error(error),
            }));
        }
        (builtins::ENV, [Value::String(key)]) => {
            return Ok(Flow::Value(match std::env::var(key) {
                Ok(value) => Value::Some(Box::new(Value::String(value))),
                Err(_) => Value::None,
            }));
        }
        (builtins::NOW_MS, []) => {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_millis());
            return Ok(Flow::Value(Value::I64(
                i64::try_from(millis).unwrap_or(i64::MAX),
            )));
        }
        (builtins::CLOCK_NS, []) => {
            let nanos = runtime.started.elapsed().as_nanos();
            return Ok(Flow::Value(Value::I64(
                i64::try_from(nanos).unwrap_or(i64::MAX),
            )));
        }
        (builtins::SLEEP_MS, [Value::I64(millis)]) => {
            std::thread::sleep(std::time::Duration::from_millis(
                u64::try_from(*millis).unwrap_or(0),
            ));
            return Ok(Flow::Value(Value::Unit));
        }
        _ => {}
    }
    if name == builtins::PRINT {
        let [Value::String(text)] = values.as_slice() else {
            return Err(Diagnostic::new("E204", expr.span, "invalid print call"));
        };
        println!("{text}");
        return Ok(Flow::Value(Value::Unit));
    }
    if name == builtins::READ_LINE {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        return Ok(Flow::Value(match std::io::stdin().read_line(&mut line) {
            Ok(0) | Err(_) => Value::None,
            Ok(_) => {
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                Value::Some(Box::new(Value::String(line)))
            }
        }));
    }
    if name == builtins::READ_STDIN {
        use std::io::{Read, Write};
        let _ = std::io::stdout().flush();
        let mut bytes = Vec::new();
        let variant = match std::io::stdin().read_to_end(&mut bytes) {
            Ok(_) => match String::from_utf8(bytes) {
                Ok(text) => return Ok(Flow::Value(Value::Ok(Box::new(Value::String(text))))),
                Err(_) => "InvalidUtf8",
            },
            Err(_) => "Other",
        };
        return Ok(Flow::Value(Value::Err(Box::new(Value::Enum(
            builtins::IO_ERROR.to_owned(),
            variant.to_owned(),
            None,
        )))));
    }
    if name == builtins::EXIT {
        let [Value::I32(status)] = values.as_slice() else {
            return Err(Diagnostic::new("E204", expr.span, "invalid exit call"));
        };
        return Err(Diagnostic::new(EXIT_REQUEST, expr.span, status.to_string()));
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
                    std::num::IntErrorKind::PosOverflow | std::num::IntErrorKind::NegOverflow => {
                        "OutOfRange"
                    }
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
    if name == builtins::PARSE_F64 {
        let [Value::String(text)] = values.as_slice() else {
            return Err(Diagnostic::new(
                "E204",
                expr.span,
                "invalid float parse call",
            ));
        };
        return Ok(Flow::Value(match parse_f64(text) {
            Ok(number) => Value::Ok(Box::new(Value::F64(number.to_bits()))),
            Err(variant) => Value::Err(Box::new(Value::Enum(
                builtins::PARSE_ERROR.to_owned(),
                variant.to_owned(),
                None,
            ))),
        }));
    }
    if name == builtins::WIDEN_I64 || name == builtins::NARROW_I32 || name == builtins::TO_F64 {
        let option = |value: Option<Value>| match value {
            Some(value) => Value::Some(Box::new(value)),
            None => Value::None,
        };
        let converted = match (name.as_str(), values.as_slice()) {
            (builtins::WIDEN_I64, [Value::I32(number)]) => Value::I64(i64::from(*number)),
            (builtins::WIDEN_I64, [Value::F64(bits)]) => {
                option(float_to_i64(f64::from_bits(*bits)).map(Value::I64))
            }
            (builtins::NARROW_I32, [Value::I64(number)]) => {
                option(i32::try_from(*number).ok().map(Value::I32))
            }
            (builtins::NARROW_I32, [Value::F64(bits)]) => option(
                float_to_i64(f64::from_bits(*bits))
                    .and_then(|number| i32::try_from(number).ok())
                    .map(Value::I32),
            ),
            (builtins::TO_F64, [Value::I32(number)]) => Value::F64(f64::from(*number).to_bits()),
            (builtins::TO_F64, [Value::I64(number)]) => Value::F64((*number as f64).to_bits()),
            _ => {
                return Err(Diagnostic::new(
                    "E204",
                    expr.span,
                    "invalid numeric conversion",
                ));
            }
        };
        return Ok(Flow::Value(converted));
    }
    if name == builtins::JOIN {
        let [Value::Task(value)] = values.as_slice() else {
            return Err(Diagnostic::new("E204", expr.span, "invalid join call"));
        };
        return Ok(Flow::Value(Value::Ok(value.clone())));
    }
    let builtin_records = builtins::http_records();
    if let Some(record) = program
        .records
        .iter()
        .chain(&builtin_records)
        .find(|record| record.name == *name)
    {
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
    Err(Diagnostic::new(
        "E204",
        expr.span,
        format!("unresolved runtime function {name}"),
    ))
}

/// Whether `value` matches `pattern`, collecting the names it binds.
fn matches_pattern(pattern: &Pattern, value: &Value, bindings: &mut Vec<(String, Value)>) -> bool {
    match (&pattern.kind, value) {
        (PatternKind::Int(pattern), Value::I32(value)) => pattern == value,
        (PatternKind::I64(pattern), Value::I64(value)) => pattern == value,
        (PatternKind::Wildcard, _) => true,
        (PatternKind::Bind(name), value) => {
            bindings.push((name.clone(), value.clone()));
            true
        }
        (PatternKind::Ok(inner), Value::Ok(value))
        | (PatternKind::Err(inner), Value::Err(value))
        | (PatternKind::Some(inner), Value::Some(value)) => matches_pattern(inner, value, bindings),
        (PatternKind::None, Value::None) => true,
        (PatternKind::Bool(pattern), Value::Bool(value)) => pattern == value,
        (PatternKind::String(pattern), Value::String(value)) => pattern == value,
        (PatternKind::Variant(name, variant, inner), Value::Enum(actual, value, payload))
            if name == actual && variant == value =>
        {
            match (inner, payload) {
                (None, None) => true,
                (Some(inner), Some(payload)) => matches_pattern(inner, payload, bindings),
                _ => false,
            }
        }
        _ => false,
    }
}

fn eval_match(
    expr: &Expr,
    value: &Expr,
    arms: &[(Pattern, Expr)],
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
    let scrutinee = take_value!(eval(value, env, program, depth, runtime));
    for (pattern, body) in arms {
        let mut bindings = Vec::new();
        if matches_pattern(pattern, &scrutinee, &mut bindings) {
            let mut scope = env.clone();
            for (name, value) in bindings {
                scope.insert(name, Rc::new(RefCell::new(value)));
            }
            return eval(body, &scope, program, depth, runtime);
        }
    }
    Err(Diagnostic::new(
        "E204",
        expr.span,
        "unmatched runtime value",
    ))
}

fn eval_block(
    stmts: &[Stmt],
    tail: &Option<Box<Expr>>,
    env: &Env,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
) -> Result<Flow, Diagnostic> {
    let mut scope = env.clone();
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let value = take_value!(eval(value, &scope, program, depth, runtime));
                scope.insert(name.clone(), Rc::new(RefCell::new(value)));
            }
            Stmt::Assign {
                name,
                path,
                value,
                span,
            } => {
                // Indices are evaluated left to right before the value.
                let mut indices = Vec::new();
                for step in path {
                    if let PlaceStep::Index(index, _) = step {
                        indices.push(take_value!(eval(index, &scope, program, depth, runtime)));
                    }
                }
                let value = take_value!(eval(value, &scope, program, depth, runtime));
                let cell = scope.get(name).ok_or_else(|| {
                    Diagnostic::new("E204", *span, format!("unresolved runtime name {name}"))
                })?;
                let mut binding = cell.borrow_mut();
                store(&mut binding, path, &mut indices.into_iter(), value)?;
            }
            Stmt::Push { name, value, span } => {
                let value = take_value!(eval(value, &scope, program, depth, runtime));
                let cell = scope.get(name).ok_or_else(|| {
                    Diagnostic::new("E204", *span, format!("unresolved runtime name {name}"))
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
                let iterable = match &iterable.kind {
                    ExprKind::Call(callee, bounds)
                        if callee == builtins::RANGE && bounds.len() == 2 =>
                    {
                        let start = take_value!(eval(&bounds[0], &scope, program, depth, runtime));
                        let end = take_value!(eval(&bounds[1], &scope, program, depth, runtime));
                        let (Value::I32(start), Value::I32(end)) = (start, end) else {
                            return Err(Diagnostic::new("E204", *span, "invalid runtime range"));
                        };
                        for value in range_values(start, end) {
                            let mut loop_scope = scope.clone();
                            loop_scope.insert(name.clone(), Rc::new(RefCell::new(value)));
                            match eval(body, &loop_scope, program, depth, runtime)? {
                                Flow::Value(_) | Flow::Continue => {}
                                Flow::Break => break,
                                Flow::Return(value) => return Ok(Flow::Return(value)),
                            }
                        }
                        continue;
                    }
                    _ => take_value!(eval(iterable, &scope, program, depth, runtime)),
                };
                let values = match iterable {
                    Value::Array(values) => values,
                    Value::Bytes(bytes) => bytes
                        .iter()
                        .map(|byte| Value::I32(i32::from(*byte)))
                        .collect(),
                    _ => {
                        return Err(Diagnostic::new("E204", *span, "invalid runtime iterable"));
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
    Ok(Flow::Value(Value::Unit))
}

fn binary(left: Value, op: Op, right: Value, span: Span) -> Result<Value, Diagnostic> {
    if let (Value::String(a), Op::Add, Value::String(b)) = (&left, op, &right) {
        return Ok(Value::String(format!("{a}{b}")));
    }
    if let (Value::Array(_), Op::Add, Value::Array(_)) = (&left, op, &right) {
        let (Value::Array(mut items), Value::Array(more)) = (left, right) else {
            unreachable!("matched arrays")
        };
        items.extend(more);
        return Ok(Value::Array(items));
    }
    if let (Value::String(a), Value::String(b)) = (&left, &right) {
        match op {
            Op::Lt => return Ok(Value::Bool(a < b)),
            Op::Le => return Ok(Value::Bool(a <= b)),
            Op::Gt => return Ok(Value::Bool(a > b)),
            Op::Ge => return Ok(Value::Bool(a >= b)),
            _ => {}
        }
    }
    if matches!(op, Op::Eq | Op::Ne) {
        let equal = values_equal(&left, &right);
        return Ok(Value::Bool(if op == Op::Eq { equal } else { !equal }));
    }
    let number = match (left, right) {
        (Value::F64(a), Value::F64(b)) => {
            let (a, b) = (f64::from_bits(a), f64::from_bits(b));
            let value = match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => a / b,
                Op::Rem => a % b,
                Op::Lt => return Ok(Value::Bool(a < b)),
                Op::Le => return Ok(Value::Bool(a <= b)),
                Op::Gt => return Ok(Value::Bool(a > b)),
                Op::Ge => return Ok(Value::Bool(a >= b)),
                Op::Eq | Op::Ne | Op::And | Op::Or => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
            };
            return Ok(Value::F64(value.to_bits()));
        }
        (Value::I32(a), Value::I32(b)) => {
            let value = match op {
                Op::Add => a.checked_add(b),
                Op::Sub => a.checked_sub(b),
                Op::Mul => a.checked_mul(b),
                Op::Div => a.checked_div(b),
                // Truncated remainder; MIN % -1 is 0 rather than an overflow.
                Op::Rem => (b != 0).then(|| a.wrapping_rem(b)),
                Op::Lt => return Ok(Value::Bool(a < b)),
                Op::Le => return Ok(Value::Bool(a <= b)),
                Op::Gt => return Ok(Value::Bool(a > b)),
                Op::Ge => return Ok(Value::Bool(a >= b)),
                Op::Eq | Op::Ne => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
                Op::And | Op::Or => {
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
                // Truncated remainder; MIN % -1 is 0 rather than an overflow.
                Op::Rem => (b != 0).then(|| a.wrapping_rem(b)),
                Op::Lt => return Ok(Value::Bool(a < b)),
                Op::Le => return Ok(Value::Bool(a <= b)),
                Op::Gt => return Ok(Value::Bool(a > b)),
                Op::Ge => return Ok(Value::Bool(a >= b)),
                Op::Eq | Op::Ne => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
                Op::And | Op::Or => {
                    return Err(Diagnostic::new("E204", span, "invalid runtime operator"));
                }
            };
            value.map(Value::I64)
        }
        _ => return Err(Diagnostic::new("E204", span, "invalid runtime operands")),
    };
    number.ok_or_else(|| Diagnostic::new("E201", span, "integer overflow or division by zero"))
}

/// Truncates toward zero; NaN and values outside `i64` have no integer result.
fn float_to_i64(value: f64) -> Option<i64> {
    let truncated = value.trunc();
    // -2^63 and 2^63 are exact in f64.
    (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0)
        .contains(&truncated)
        .then_some(truncated as i64)
}

/// Accepts `[+-]digits[.digits][(e|E)[+-]digits]`; non-finite results are out of range.
fn parse_f64(text: &str) -> Result<f64, &'static str> {
    let bytes = text.as_bytes();
    let mut i = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits = |i: &mut usize| {
        let start = *i;
        while bytes.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i > start
    };
    let mut valid = digits(&mut i);
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        valid &= digits(&mut i);
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(bytes.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        valid &= digits(&mut i);
    }
    if !valid || i != bytes.len() {
        return Err("Invalid");
    }
    let number = text.parse::<f64>().map_err(|_| "Invalid")?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err("OutOfRange")
    }
}

/// Pure string builtins, including string `join` and `String(...)` conversions.
fn eval_string_builtin(name: &str, values: &[Value]) -> Option<Value> {
    let text = |value: &str| Value::String(value.to_owned());
    let strings = |items: Vec<&str>| Value::Array(items.into_iter().map(text).collect());
    Some(match (name, values) {
        (
            builtins::TO_STRING,
            [value @ (Value::I32(_) | Value::I64(_) | Value::F64(_) | Value::Bool(_))],
        ) => Value::String(value.to_string()),
        (builtins::JOIN, [Value::Array(parts), Value::String(separator)]) => Value::String(
            parts
                .iter()
                .map(|part| match part {
                    Value::String(part) => part.as_str(),
                    _ => "",
                })
                .collect::<Vec<_>>()
                .join(separator),
        ),
        (builtins::CHARS, [Value::String(value)]) => Value::Array(
            value
                .chars()
                .map(|c| Value::String(c.to_string()))
                .collect(),
        ),
        (builtins::SPLIT, [Value::String(value), Value::String(separator)]) => {
            if separator.is_empty() {
                strings(vec![value])
            } else {
                strings(value.split(separator.as_str()).collect())
            }
        }
        (builtins::TRIM, [Value::String(value)]) => text(value.trim()),
        (builtins::CONTAINS, [Value::String(value), Value::String(part)]) => {
            Value::Bool(value.contains(part.as_str()))
        }
        (builtins::STARTS_WITH, [Value::String(value), Value::String(part)]) => {
            Value::Bool(value.starts_with(part.as_str()))
        }
        (builtins::ENDS_WITH, [Value::String(value), Value::String(part)]) => {
            Value::Bool(value.ends_with(part.as_str()))
        }
        (builtins::REPLACE, [Value::String(value), Value::String(from), Value::String(to)]) => {
            if from.is_empty() {
                text(value)
            } else {
                Value::String(value.replace(from.as_str(), to))
            }
        }
        (builtins::LOWER, [Value::String(value)]) => Value::String(value.to_lowercase()),
        (builtins::UPPER, [Value::String(value)]) => Value::String(value.to_uppercase()),
        _ => return None,
    })
}

/// Assign `value` at `path` below `target`, checking indices and byte ranges.
fn store(
    target: &mut Value,
    path: &[PlaceStep],
    keys: &mut impl Iterator<Item = Value>,
    value: Value,
) -> Result<(), Diagnostic> {
    let Some((step, rest)) = path.split_first() else {
        *target = value;
        return Ok(());
    };
    if let (PlaceStep::Index(..), Value::Map(entries)) = (step, &mut *target) {
        let key = keys
            .next()
            .ok_or_else(|| Diagnostic::new("E204", Span::new(0, 0), "missing map key"))?;
        map_insert(entries, key, value);
        return Ok(());
    }
    match (step, target) {
        (PlaceStep::Index(_, span), Value::Array(items)) => {
            let index = position(keys.next());
            let slot = usize::try_from(index)
                .ok()
                .and_then(|at| items.get_mut(at))
                .ok_or_else(|| Diagnostic::new("E205", *span, "array index out of bounds"))?;
            store(slot, rest, keys, value)
        }
        (PlaceStep::Index(_, span), Value::Bytes(bytes)) => {
            let index = position(keys.next());
            let at = usize::try_from(index)
                .ok()
                .filter(|at| *at < bytes.len())
                .ok_or_else(|| Diagnostic::new("E205", *span, "array index out of bounds"))?;
            let Value::I32(byte) = value else {
                return Err(Diagnostic::new("E204", *span, "invalid runtime byte"));
            };
            let byte = u8::try_from(byte)
                .map_err(|_| Diagnostic::new("E207", *span, "byte value outside 0..255"))?;
            Arc::make_mut(bytes)[at] = byte;
            Ok(())
        }
        (PlaceStep::Field(field, span), Value::Record(_, fields)) => {
            let slot = fields
                .iter_mut()
                .find(|(name, _)| name == field)
                .map(|(_, slot)| slot)
                .ok_or_else(|| Diagnostic::new("E204", *span, "unknown runtime field"))?;
            store(slot, rest, keys, value)
        }
        (PlaceStep::Index(_, span) | PlaceStep::Field(_, span), _) => Err(Diagnostic::new(
            "E204",
            *span,
            "invalid runtime assignment target",
        )),
    }
}

/// The same total order as the native `BTreeMap` keys.
fn key_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    match (left, right) {
        (Value::I32(a), Value::I32(b)) => a.cmp(b),
        (Value::I64(a), Value::I64(b)) => a.cmp(b),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => std::cmp::Ordering::Equal,
    }
}

fn map_find(entries: &[(Value, Value)], key: &Value) -> Result<usize, usize> {
    entries.binary_search_by(|(candidate, _)| key_order(candidate, key))
}

fn map_insert(entries: &mut Vec<(Value, Value)>, key: Value, value: Value) {
    match map_find(entries, &key) {
        Ok(at) => entries[at].1 = value,
        Err(at) => entries.insert(at, (key, value)),
    }
}

/// Pure map builtins; `len` is handled with arrays.
fn eval_map_builtin(name: &str, values: &[Value]) -> Option<Value> {
    Some(match (name, values) {
        (builtins::MAP, []) => Value::Map(Vec::new()),
        (builtins::GET, [Value::Map(entries), key]) => match map_find(entries, key) {
            Ok(at) => Value::Some(Box::new(entries[at].1.clone())),
            Err(_) => Value::None,
        },
        (builtins::GET_OR, [Value::Map(entries), key, fallback]) => match map_find(entries, key) {
            Ok(at) => entries[at].1.clone(),
            Err(_) => fallback.clone(),
        },
        (builtins::CONTAINS, [Value::Map(entries), key]) => {
            Value::Bool(map_find(entries, key).is_ok())
        }
        (builtins::KEYS, [Value::Map(entries)]) => {
            Value::Array(entries.iter().map(|(key, _)| key.clone()).collect())
        }
        (builtins::VALUES, [Value::Map(entries)]) => {
            Value::Array(entries.iter().map(|(_, value)| value.clone()).collect())
        }
        (builtins::REMOVE, [Value::Map(entries), key]) => {
            let mut entries = entries.clone();
            if let Ok(at) = map_find(&entries, key) {
                entries.remove(at);
            }
            Value::Map(entries)
        }
        _ => return None,
    })
}

/// An array position from an evaluated index; non-integers never match.
fn position(index: Option<Value>) -> i32 {
    match index {
        Some(Value::I32(index)) => index,
        _ => -1,
    }
}

/// The order used by `sort`; floats use IEEE 754 total order.
fn value_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    match (left, right) {
        (Value::F64(a), Value::F64(b)) => f64::from_bits(*a).total_cmp(&f64::from_bits(*b)),
        _ => key_order(left, right),
    }
}

/// `==` semantics, where NaN differs from itself and both zeros are equal.
/// Structural `==`, with IEEE semantics for floats at any depth.
fn values_equal(left: &Value, right: &Value) -> bool {
    let all = |left: &[Value], right: &[Value]| {
        left.len() == right.len() && left.iter().zip(right).all(|(a, b)| values_equal(a, b))
    };
    match (left, right) {
        (Value::F64(a), Value::F64(b)) => f64::from_bits(*a) == f64::from_bits(*b),
        (Value::Array(a), Value::Array(b)) => all(a, b),
        (Value::Record(a_name, a), Value::Record(b_name, b)) => {
            a_name == b_name
                && a.len() == b.len()
                && a.iter().zip(b).all(|((_, x), (_, y))| values_equal(x, y))
        }
        (Value::Enum(a_name, a_variant, a), Value::Enum(b_name, b_variant, b)) => {
            a_name == b_name
                && a_variant == b_variant
                && match (a, b) {
                    (Some(a), Some(b)) => values_equal(a, b),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Value::Ok(a), Value::Ok(b))
        | (Value::Err(a), Value::Err(b))
        | (Value::Some(a), Value::Some(b)) => values_equal(a, b),
        _ => left == right,
    }
}

fn range_values(start: i32, end: i32) -> impl Iterator<Item = Value> {
    (start..end).map(Value::I32)
}

fn eval_array_builtin(
    name: &str,
    values: &[Value],
    span: Span,
) -> Option<Result<Value, Diagnostic>> {
    Some(Ok(match (name, values) {
        (builtins::RANGE, [Value::I32(start), Value::I32(end)]) => {
            Value::Array(range_values(*start, *end).collect())
        }
        (builtins::SORT, [Value::Array(items)]) => {
            let mut items = items.clone();
            items.sort_by(value_order);
            Value::Array(items)
        }
        (builtins::REVERSE, [Value::Array(items)]) => {
            Value::Array(items.iter().rev().cloned().collect())
        }
        (builtins::SLICE, [Value::Array(items), Value::I32(from), Value::I32(to)]) => {
            let bounds = usize::try_from(*from).ok().zip(usize::try_from(*to).ok());
            match bounds {
                Some((from, to)) if from <= to && to <= items.len() => {
                    Value::Array(items[from..to].to_vec())
                }
                _ => {
                    return Some(Err(Diagnostic::new(
                        "E205",
                        span,
                        "slice bounds out of range",
                    )));
                }
            }
        }
        (builtins::CONTAINS, [Value::Array(items), wanted]) => {
            Value::Bool(items.iter().any(|item| values_equal(item, wanted)))
        }
        _ => return None,
    }))
}

/// Run `frame` with room for one more call, on a fresh stack segment when due.
fn with_call_stack(
    depth: usize,
    span: Span,
    frame: impl FnOnce() -> Result<Value, Diagnostic> + Send,
) -> Result<Value, Diagnostic> {
    if depth >= MAX_CALL_DEPTH {
        return Err(Diagnostic::new("E202", span, "call depth limit exceeded"));
    }
    if depth > 0 && depth.is_multiple_of(CALLS_PER_STACK_SEGMENT) {
        return std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(STACK_SEGMENT_BYTES)
                .spawn_scoped(scope, frame)
                .map_err(|error| {
                    Diagnostic::new(
                        "E204",
                        span,
                        format!("cannot extend evaluator stack: {error}"),
                    )
                })?
                .join()
                .map_err(|_| Diagnostic::new("E204", span, "reference evaluator panicked"))?
        });
    }
    frame()
}

fn call_closure(
    closure: &Closure,
    args: Vec<Value>,
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let depth = depth + 1;
    with_call_stack(depth, span, move || {
        let env: Env = closure
            .captured
            .iter()
            .cloned()
            .chain(closure.params.iter().cloned().zip(args))
            .map(|(name, value)| (name, Rc::new(RefCell::new(value))))
            .collect();
        match eval(&closure.body, &env, program, depth, runtime)? {
            Flow::Value(value) | Flow::Return(value) => Ok(value),
            Flow::Break | Flow::Continue => Err(Diagnostic::new(
                "E204",
                span,
                "loop control escaped a lambda",
            )),
        }
    })
}

/// Builtins that call function values.
fn eval_higher_order(
    name: &str,
    values: &[Value],
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
    span: Span,
) -> Option<Result<Value, Diagnostic>> {
    let call = |f: &Closure, args: Vec<Value>| call_closure(f, args, program, depth, runtime, span);
    let truth = |value: Value| match value {
        Value::Bool(value) => Ok(value),
        _ => Err(Diagnostic::new(
            "E204",
            span,
            "predicate returned a non-bool",
        )),
    };
    let (items, function, init) = match values {
        [Value::Array(items), Value::Closure(f)] => (items, f, None),
        [Value::Array(items), init, Value::Closure(f)] => (items, f, Some(init)),
        _ => return None,
    };
    let each = |item: &Value| call(function, vec![item.clone()]);
    Some(match (name, init) {
        (builtins::MAP_FN, None) => items
            .iter()
            .map(each)
            .collect::<Result<_, _>>()
            .map(Value::Array),
        (builtins::FILTER, None) => (|| {
            let mut kept = Vec::new();
            for item in items {
                if truth(each(item)?)? {
                    kept.push(item.clone());
                }
            }
            Ok(Value::Array(kept))
        })(),
        (builtins::ANY | builtins::ALL, None) => (|| {
            let wanted = name == builtins::ANY;
            for item in items {
                if truth(each(item)?)? == wanted {
                    return Ok(Value::Bool(wanted));
                }
            }
            Ok(Value::Bool(!wanted))
        })(),
        (builtins::FOLD, Some(init)) => items.iter().try_fold(init.clone(), |total, item| {
            call(function, vec![total, item.clone()])
        }),
        (builtins::SORT_BY, None) => (|| {
            let mut keyed = items
                .iter()
                .map(|item| Ok((each(item)?, item.clone())))
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            keyed.sort_by(|(left, _), (right, _)| value_order(left, right));
            Ok(Value::Array(
                keyed.into_iter().map(|(_, item)| item).collect(),
            ))
        })(),
        _ => return None,
    })
}

fn float(value: f64) -> Value {
    Value::F64(value.to_bits())
}

/// Integer `pow` by squaring with overflow checks; a negative exponent fails.
fn checked_pow<T: Copy>(base: T, exponent: i32, one: T, mul: fn(T, T) -> Option<T>) -> Option<T> {
    let mut exponent = u32::try_from(exponent).ok()?;
    let (mut base, mut result) = (base, one);
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = mul(result, base)?;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = mul(base, base)?;
        }
    }
    Some(result)
}

fn eval_math_builtin(
    name: &str,
    values: &[Value],
    span: Span,
) -> Option<Result<Value, Diagnostic>> {
    let overflow = || Diagnostic::new("E201", span, "integer overflow or division by zero");
    let unary = |f: fn(f64) -> f64| match values {
        [Value::F64(bits)] => Some(float(f(f64::from_bits(*bits)))),
        _ => None,
    };
    let value = match (name, values) {
        (builtins::ABS, [Value::I32(n)]) => n.checked_abs().map(Value::I32).ok_or_else(overflow),
        (builtins::ABS, [Value::I64(n)]) => n.checked_abs().map(Value::I64).ok_or_else(overflow),
        (builtins::ABS, [Value::F64(bits)]) => Ok(float(f64::from_bits(*bits).abs())),
        (builtins::MIN, [Value::I32(a), Value::I32(b)]) => Ok(Value::I32(*a.min(b))),
        (builtins::MIN, [Value::I64(a), Value::I64(b)]) => Ok(Value::I64(*a.min(b))),
        (builtins::MIN, [Value::F64(a), Value::F64(b)]) => {
            Ok(float(f64::from_bits(*a).min(f64::from_bits(*b))))
        }
        (builtins::MAX, [Value::I32(a), Value::I32(b)]) => Ok(Value::I32(*a.max(b))),
        (builtins::MAX, [Value::I64(a), Value::I64(b)]) => Ok(Value::I64(*a.max(b))),
        (builtins::MAX, [Value::F64(a), Value::F64(b)]) => {
            Ok(float(f64::from_bits(*a).max(f64::from_bits(*b))))
        }
        (builtins::POW, [Value::I32(base), Value::I32(exponent)]) => {
            checked_pow(*base, *exponent, 1, i32::checked_mul)
                .map(Value::I32)
                .ok_or_else(overflow)
        }
        (builtins::POW, [Value::I64(base), Value::I32(exponent)]) => {
            checked_pow(*base, *exponent, 1, i64::checked_mul)
                .map(Value::I64)
                .ok_or_else(overflow)
        }
        (builtins::POW, [Value::F64(base), Value::F64(exponent)]) => {
            Ok(float(f64::from_bits(*base).powf(f64::from_bits(*exponent))))
        }
        (builtins::ATAN2, [Value::F64(y), Value::F64(x)]) => {
            Ok(float(f64::from_bits(*y).atan2(f64::from_bits(*x))))
        }
        (builtins::PI, []) => Ok(float(std::f64::consts::PI)),
        (builtins::SQRT, _) => unary(f64::sqrt).ok_or_else(overflow),
        (builtins::FLOOR, _) => unary(f64::floor).ok_or_else(overflow),
        (builtins::CEIL, _) => unary(f64::ceil).ok_or_else(overflow),
        (builtins::ROUND, _) => unary(f64::round).ok_or_else(overflow),
        (builtins::EXP, _) => unary(f64::exp).ok_or_else(overflow),
        (builtins::LN, _) => unary(f64::ln).ok_or_else(overflow),
        (builtins::SIN, _) => unary(f64::sin).ok_or_else(overflow),
        (builtins::COS, _) => unary(f64::cos).ok_or_else(overflow),
        (builtins::TAN, _) => unary(f64::tan).ok_or_else(overflow),
        _ => return None,
    };
    Some(value)
}

fn io_error_value(variant: &str) -> Value {
    Value::Err(Box::new(Value::Enum(
        builtins::IO_ERROR.to_owned(),
        variant.to_owned(),
        None,
    )))
}

fn headers_value(headers: Vec<(String, String)>) -> Value {
    let mut entries = Vec::new();
    for (name, value) in headers {
        map_insert(&mut entries, Value::String(name), Value::String(value));
    }
    Value::Map(entries)
}

fn headers_from(value: &Value) -> Vec<(String, String)> {
    match value {
        Value::Map(entries) => entries
            .iter()
            .filter_map(|(name, value)| match (name, value) {
                (Value::String(name), Value::String(value)) => Some((name.clone(), value.clone())),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn record_field<'a>(value: &'a Value, field: &str) -> Option<&'a Value> {
    match value {
        Value::Record(_, fields) => fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, value)| value),
        _ => None,
    }
}

fn string_field(value: &Value, field: &str) -> String {
    match record_field(value, field) {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn request_value(request: crate::http::__TokHttpRequest) -> Value {
    Value::Record(
        builtins::REQUEST.to_owned(),
        vec![
            ("method".to_owned(), Value::String(request.method)),
            ("path".to_owned(), Value::String(request.path)),
            ("query".to_owned(), Value::String(request.query)),
            ("headers".to_owned(), headers_value(request.headers)),
            ("body".to_owned(), Value::String(request.body)),
        ],
    )
}

fn response_from(response: &Value) -> crate::http::__TokHttpResponse {
    crate::http::__TokHttpResponse {
        status: match record_field(response, "status") {
            Some(Value::I32(status)) => *status,
            _ => 500,
        },
        headers: record_field(response, "headers")
            .map(headers_from)
            .unwrap_or_default(),
        body: string_field(response, "body"),
    }
}

fn response_value(response: crate::http::__TokHttpResponse) -> Value {
    Value::Record(
        builtins::RESPONSE.to_owned(),
        vec![
            ("status".to_owned(), Value::I32(response.status)),
            ("headers".to_owned(), headers_value(response.headers)),
            ("body".to_owned(), Value::String(response.body)),
        ],
    )
}

/// `serve` and `http_request` through the HTTP code shared with native programs.
fn eval_http(
    name: &str,
    values: &[Value],
    program: &Program,
    depth: usize,
    runtime: &Runtime<'_>,
    span: Span,
) -> Option<Result<Value, Diagnostic>> {
    match (name, values) {
        (
            builtins::SERVE,
            // The reference interpreter answers one request at a time, also
            // when a worker count is given.
            [
                Value::String(addr),
                Value::I32(limit),
                ..,
                Value::Closure(handler),
            ],
        ) => {
            if !crate::http::__tok_http_allowed(runtime.net.as_deref(), addr) {
                return Some(Ok(io_error_value("Denied")));
            }
            let mut failure = None;
            let served =
                crate::http::__tok_http_serve(addr, *limit, &mut |request| match call_closure(
                    handler,
                    vec![request_value(request)],
                    program,
                    depth,
                    runtime,
                    span,
                ) {
                    Ok(response) => Some(response_from(&response)),
                    Err(error) => {
                        failure = Some(error);
                        None
                    }
                });
            Some(match (failure, served) {
                (Some(error), _) => Err(error),
                (None, Ok(())) => Ok(Value::Ok(Box::new(Value::Unit))),
                (None, Err(variant)) => Ok(io_error_value(variant)),
            })
        }
        (builtins::TCP_CONNECT, [Value::String(addr)]) => Some(Ok(
            match crate::http::__tok_tcp_connect(runtime.net.as_deref(), addr) {
                Ok(socket) => Value::Ok(Box::new(Value::Conn(Socket(socket)))),
                Err(variant) => io_error_value(variant),
            },
        )),
        (builtins::TCP_SEND, [Value::Conn(socket), Value::Bytes(data)]) => {
            Some(Ok(match crate::http::__tok_tcp_send(&socket.0, data) {
                Ok(()) => Value::Ok(Box::new(Value::Unit)),
                Err(variant) => io_error_value(variant),
            }))
        }
        (builtins::TCP_RECV, [Value::Conn(socket), Value::I32(max)]) => {
            Some(Ok(match crate::http::__tok_tcp_recv(&socket.0, *max) {
                Ok(data) => Value::Ok(Box::new(Value::Bytes(Arc::new(data)))),
                Err(variant) => io_error_value(variant),
            }))
        }
        (builtins::TCP_CLOSE, [Value::Conn(socket)]) => {
            crate::http::__tok_tcp_close(&socket.0);
            Some(Ok(Value::Unit))
        }
        (builtins::LISTEN, [Value::String(addr)]) => Some(Ok(match crate::http::__tok_tcp_listen(
            runtime.net.as_deref(),
            addr,
        ) {
            Ok(listener) => Value::Ok(Box::new(Value::Listener(ListenerHandle(listener)))),
            Err(variant) => io_error_value(variant),
        })),
        (builtins::ACCEPT, [Value::Listener(listener)]) => {
            Some(Ok(match crate::http::__tok_tcp_accept(&listener.0) {
                Ok(socket) => Value::Ok(Box::new(Value::Conn(Socket(socket)))),
                Err(variant) => io_error_value(variant),
            }))
        }
        (builtins::HTTP_READ, [Value::Conn(socket)]) => {
            Some(Ok(match crate::http::__tok_http_read(&socket.0) {
                Ok(request) => Value::Ok(Box::new(request_value(request))),
                Err(variant) => io_error_value(variant),
            }))
        }
        (builtins::HTTP_WRITE, [Value::Conn(socket), response]) => Some(Ok(
            match crate::http::__tok_http_write(&socket.0, &response_from(response)) {
                Ok(()) => Value::Ok(Box::new(Value::Unit)),
                Err(variant) => io_error_value(variant),
            },
        )),
        (
            builtins::HTTP_REQUEST,
            [
                Value::String(method),
                Value::String(url),
                headers,
                Value::String(body),
            ],
        ) => Some(Ok(
            match crate::http::__tok_http_request(
                runtime.net.as_deref(),
                method,
                url,
                &headers_from(headers),
                body,
            ) {
                Ok(response) => Value::Ok(Box::new(response_value(response))),
                Err(variant) => io_error_value(variant),
            },
        )),
        _ => None,
    }
}

/// Hashing, encoding, and random bytes through the code native programs embed.
fn eval_crypto_builtin(name: &str, values: &[Value]) -> Option<Value> {
    use crate::crypto as c;
    let bytes = |data: Vec<u8>| Value::Bytes(Arc::new(data));
    Some(match (name, values) {
        (builtins::SHA256, [Value::Bytes(data)]) => bytes(c::__tok_sha256(data)),
        (builtins::MD5, [Value::Bytes(data)]) => bytes(c::__tok_md5(data)),
        (builtins::SHA1, [Value::Bytes(data)]) => bytes(c::__tok_sha1(data)),
        (builtins::HMAC_SHA256, [Value::Bytes(key), Value::Bytes(data)]) => {
            bytes(c::__tok_hmac_sha256(key, data))
        }
        (builtins::PBKDF2_SHA256, [Value::Bytes(password), Value::Bytes(salt), Value::I32(n)]) => {
            bytes(c::__tok_pbkdf2_sha256(password, salt, *n))
        }
        (builtins::BASE64_ENCODE, [Value::Bytes(data)]) => {
            Value::String(c::__tok_base64_encode(data))
        }
        (builtins::BASE64_DECODE, [Value::String(text)]) => match c::__tok_base64_decode(text) {
            Some(data) => Value::Some(Box::new(bytes(data))),
            None => Value::None,
        },
        (builtins::HEX, [Value::Bytes(data)]) => Value::String(c::__tok_hex(data)),
        (builtins::RANDOM_BYTES, [Value::I32(count)]) => bytes(c::__tok_random_bytes(*count)),
        _ => return None,
    })
}

/// Two's-complement bit operations; shifts outside the width report `E201`.
fn eval_bit_builtin(name: &str, values: &[Value], span: Span) -> Option<Result<Value, Diagnostic>> {
    let shift = |amount: i32, bits: u32| {
        u32::try_from(amount)
            .ok()
            .filter(|amount| *amount < bits)
            .ok_or_else(|| Diagnostic::new("E201", span, "shift amount outside the integer width"))
    };
    Some(match (name, values) {
        (builtins::BIT_AND, [Value::I32(a), Value::I32(b)]) => Ok(Value::I32(a & b)),
        (builtins::BIT_OR, [Value::I32(a), Value::I32(b)]) => Ok(Value::I32(a | b)),
        (builtins::BIT_XOR, [Value::I32(a), Value::I32(b)]) => Ok(Value::I32(a ^ b)),
        (builtins::BIT_NOT, [Value::I32(a)]) => Ok(Value::I32(!a)),
        (builtins::SHL, [Value::I32(a), Value::I32(n)]) => {
            shift(*n, 32).map(|n| Value::I32(a << n))
        }
        (builtins::SHR, [Value::I32(a), Value::I32(n)]) => {
            shift(*n, 32).map(|n| Value::I32(a >> n))
        }
        (builtins::BIT_AND, [Value::I64(a), Value::I64(b)]) => Ok(Value::I64(a & b)),
        (builtins::BIT_OR, [Value::I64(a), Value::I64(b)]) => Ok(Value::I64(a | b)),
        (builtins::BIT_XOR, [Value::I64(a), Value::I64(b)]) => Ok(Value::I64(a ^ b)),
        (builtins::BIT_NOT, [Value::I64(a)]) => Ok(Value::I64(!a)),
        (builtins::SHL, [Value::I64(a), Value::I32(n)]) => {
            shift(*n, 64).map(|n| Value::I64(a << n))
        }
        (builtins::SHR, [Value::I64(a), Value::I32(n)]) => {
            shift(*n, 64).map(|n| Value::I64(a >> n))
        }
        _ => return None,
    })
}

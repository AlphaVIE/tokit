use std::collections::HashMap;

use crate::ast::{Expr, ExprKind, Function, Op, Program, Span, Stmt};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    I32(i32),
    Bool(bool),
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I32(n) => write!(f, "{n}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Unit => f.write_str("()"),
        }
    }
}

enum Flow {
    Value(Value),
    Return(Value),
}

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
    if !main.params.is_empty() {
        return Err(Diagnostic::new(
            "E203",
            main.span,
            "main must have no parameters",
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
    if depth >= 1024 {
        return Err(Diagnostic::new(
            "E202",
            function.span,
            "call depth limit exceeded",
        ));
    }
    let env = function
        .params
        .iter()
        .zip(args)
        .map(|((name, _), value)| (name.clone(), value))
        .collect();
    match eval(&function.body, &env, program, depth)? {
        Flow::Value(value) | Flow::Return(value) => Ok(value),
    }
}

fn eval(
    expr: &Expr,
    env: &HashMap<String, Value>,
    program: &Program,
    depth: usize,
) -> Result<Flow, Diagnostic> {
    let value = match &expr.kind {
        ExprKind::Int(number) => Value::I32(*number),
        ExprKind::Bool(value) => Value::Bool(*value),
        ExprKind::Var(name) => env.get(name).cloned().ok_or_else(|| {
            Diagnostic::new("E204", expr.span, format!("unresolved runtime name {name}"))
        })?,
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
        ExprKind::Block(stmts, tail) => {
            let mut scope = env.clone();
            for stmt in stmts {
                match stmt {
                    Stmt::Let { name, value, .. } => {
                        let value = take_value!(eval(value, &scope, program, depth));
                        scope.insert(name.clone(), value);
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

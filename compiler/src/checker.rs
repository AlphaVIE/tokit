use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, ExprKind, Op, Program, Span, Stmt, Type};
use crate::diagnostic::Diagnostic;

#[derive(Clone)]
struct Signature {
    params: Vec<Type>,
    ret: Type,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        if signatures
            .insert(
                function.name.clone(),
                Signature {
                    params: function.params.iter().map(|(_, ty)| *ty).collect(),
                    ret: function.ret,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::new(
                "E106",
                function.span,
                format!("duplicate function {}", function.name),
            ));
        }
    }
    for function in &program.functions {
        let mut env = HashMap::new();
        for (name, ty) in &function.params {
            if env.insert(name.clone(), *ty).is_some() {
                return Err(Diagnostic::new(
                    "E106",
                    function.span,
                    format!("duplicate parameter {name}"),
                ));
            }
        }
        let body_type = type_of(&function.body, &env, &signatures, function.ret)?;
        require(
            function.ret,
            body_type,
            function.body.span,
            "function result",
        )?;
    }
    Ok(())
}

fn require(expected: Type, actual: Type, span: Span, context: &str) -> Result<(), Diagnostic> {
    if expected == actual || actual == Type::Never {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "E102",
            span,
            format!("{context}: expected {expected}, got {actual}"),
        ))
    }
}

fn type_of(
    expr: &Expr,
    env: &HashMap<String, Type>,
    signatures: &HashMap<String, Signature>,
    return_type: Type,
) -> Result<Type, Diagnostic> {
    match &expr.kind {
        ExprKind::Int(_) => Ok(Type::I32),
        ExprKind::Bool(_) => Ok(Type::Bool),
        ExprKind::Var(name) => env
            .get(name)
            .copied()
            .ok_or_else(|| Diagnostic::new("E101", expr.span, format!("unknown name {name}"))),
        ExprKind::Binary(left, op, right) => {
            let lhs = type_of(left, env, signatures, return_type)?;
            let rhs = type_of(right, env, signatures, return_type)?;
            if lhs == Type::Never || rhs == Type::Never {
                return Ok(Type::Never);
            }
            let result = match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Lt | Op::Le | Op::Gt | Op::Ge
                    if lhs == Type::I32 && rhs == Type::I32 =>
                {
                    if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div) {
                        Type::I32
                    } else {
                        Type::Bool
                    }
                }
                Op::Eq | Op::Ne if lhs == rhs && matches!(lhs, Type::I32 | Type::Bool) => {
                    Type::Bool
                }
                _ => {
                    return Err(Diagnostic::new(
                        "E104",
                        expr.span,
                        format!("invalid operands {lhs} and {rhs} for {op:?}"),
                    ));
                }
            };
            Ok(result)
        }
        ExprKind::Call(name, args) => {
            let signature = signatures.get(name).ok_or_else(|| {
                Diagnostic::new("E101", expr.span, format!("unknown function {name}"))
            })?;
            if signature.params.len() != args.len() {
                return Err(Diagnostic::new(
                    "E105",
                    expr.span,
                    format!(
                        "{name} expects {} arguments, got {}",
                        signature.params.len(),
                        args.len()
                    ),
                ));
            }
            for (arg, expected) in args.iter().zip(&signature.params) {
                let actual = type_of(arg, env, signatures, return_type)?;
                require(*expected, actual, arg.span, "argument")?;
            }
            Ok(signature.ret)
        }
        ExprKind::If(condition, yes, no) => {
            let cond_type = type_of(condition, env, signatures, return_type)?;
            require(Type::Bool, cond_type, condition.span, "condition")?;
            let yes_type = type_of(yes, env, signatures, return_type)?;
            let no_type = type_of(no, env, signatures, return_type)?;
            if yes_type == Type::Never {
                Ok(no_type)
            } else if no_type == Type::Never || yes_type == no_type {
                Ok(yes_type)
            } else {
                Err(Diagnostic::new(
                    "E102",
                    expr.span,
                    format!("branches have different types: {yes_type} and {no_type}"),
                ))
            }
        }
        ExprKind::Block(stmts, tail) => {
            let mut scope = env.clone();
            let mut declared = HashSet::new();
            for (index, stmt) in stmts.iter().enumerate() {
                let has_following = index + 1 < stmts.len() || tail.is_some();
                match stmt {
                    Stmt::Let {
                        name,
                        ty,
                        value,
                        span,
                    } => {
                        if !declared.insert(name.clone()) {
                            return Err(Diagnostic::new(
                                "E106",
                                *span,
                                format!("duplicate binding {name}"),
                            ));
                        }
                        let actual = type_of(value, &scope, signatures, return_type)?;
                        require(*ty, actual, value.span, "binding")?;
                        if actual == Type::Never {
                            if has_following {
                                return Err(Diagnostic::new("E107", *span, "unreachable code"));
                            }
                            return Ok(Type::Never);
                        }
                        scope.insert(name.clone(), *ty);
                    }
                    Stmt::Return { value, span } => {
                        let actual = type_of(value, &scope, signatures, return_type)?;
                        require(return_type, actual, value.span, "return")?;
                        if has_following {
                            return Err(Diagnostic::new("E107", *span, "unreachable code"));
                        }
                        return Ok(Type::Never);
                    }
                    Stmt::Expr(value) => {
                        if type_of(value, &scope, signatures, return_type)? == Type::Never {
                            if has_following {
                                return Err(Diagnostic::new(
                                    "E107",
                                    value.span,
                                    "unreachable code",
                                ));
                            }
                            return Ok(Type::Never);
                        }
                    }
                }
            }
            if let Some(tail) = tail {
                type_of(tail, &scope, signatures, return_type)
            } else {
                Ok(Type::Unit)
            }
        }
    }
}

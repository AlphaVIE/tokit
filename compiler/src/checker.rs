use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, ExprKind, Op, Program, Span, Stmt, Type};
use crate::diagnostic::Diagnostic;

#[derive(Clone)]
struct Signature {
    params: Vec<Type>,
    ret: Type,
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    mutable: bool,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    let mut signatures = HashMap::new();
    for function in &program.functions {
        let signature = Signature {
            params: function.params.iter().map(|(_, ty)| ty.clone()).collect(),
            ret: function.ret.clone(),
        };
        if signatures
            .insert(function.name.clone(), signature)
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
            if env
                .insert(
                    name.clone(),
                    Binding {
                        ty: ty.clone(),
                        mutable: false,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new(
                    "E106",
                    function.span,
                    format!("duplicate parameter {name}"),
                ));
            }
        }
        let body_type = type_of(&function.body, &env, &signatures, &function.ret)?;
        require(
            &function.ret,
            &body_type,
            function.body.span,
            "function result",
        )?;
    }
    Ok(())
}

fn compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual || *actual == Type::Never {
        return true;
    }
    match (expected, actual) {
        (Type::Array(_), Type::EmptyArray) => true,
        (Type::Array(expected), Type::Array(actual)) => compatible(expected, actual),
        (Type::Result(expected_ok, expected_err), Type::Result(actual_ok, actual_err)) => {
            compatible(expected_ok, actual_ok) && compatible(expected_err, actual_err)
        }
        _ => false,
    }
}

fn join(left: &Type, right: &Type) -> Option<Type> {
    if left == right {
        return Some(left.clone());
    }
    if *left == Type::Never {
        return Some(right.clone());
    }
    if *right == Type::Never {
        return Some(left.clone());
    }
    match (left, right) {
        (Type::Array(_), Type::EmptyArray) => Some(left.clone()),
        (Type::EmptyArray, Type::Array(_)) => Some(right.clone()),
        (Type::Array(left), Type::Array(right)) => Some(Type::Array(Box::new(join(left, right)?))),
        (Type::Result(left_ok, left_err), Type::Result(right_ok, right_err)) => Some(Type::Result(
            Box::new(join(left_ok, right_ok)?),
            Box::new(join(left_err, right_err)?),
        )),
        _ => None,
    }
}

fn require(expected: &Type, actual: &Type, span: Span, context: &str) -> Result<(), Diagnostic> {
    if compatible(expected, actual) {
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
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
) -> Result<Type, Diagnostic> {
    match &expr.kind {
        ExprKind::Int(_) => Ok(Type::I32),
        ExprKind::Bool(_) => Ok(Type::Bool),
        ExprKind::Array(values) => {
            let Some(first) = values.first() else {
                return Ok(Type::EmptyArray);
            };
            let mut element = type_of(first, env, signatures, return_type)?;
            if element == Type::Never {
                return Ok(Type::Never);
            }
            for value in values.iter().skip(1) {
                let actual = type_of(value, env, signatures, return_type)?;
                element = join(&element, &actual).ok_or_else(|| {
                    Diagnostic::new(
                        "E102",
                        value.span,
                        format!("array elements have different types: {element} and {actual}"),
                    )
                })?;
            }
            Ok(Type::Array(Box::new(element)))
        }
        ExprKind::Ok(inner) => {
            let inner = type_of(inner, env, signatures, return_type)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(inner), Box::new(Type::Never)))
        }
        ExprKind::Err(inner) => {
            let inner = type_of(inner, env, signatures, return_type)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(Type::Never), Box::new(inner)))
        }
        ExprKind::Try(inner) => {
            let actual = type_of(inner, env, signatures, return_type)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            let Type::Result(ok, err) = actual else {
                return Err(Diagnostic::new(
                    "E111",
                    inner.span,
                    "? requires a Result value",
                ));
            };
            let Type::Result(_, expected_err) = return_type else {
                return Err(Diagnostic::new(
                    "E111",
                    expr.span,
                    "? requires a Result return type",
                ));
            };
            require(expected_err, &err, expr.span, "propagated error")?;
            Ok(*ok)
        }
        ExprKind::Var(name) => env
            .get(name)
            .map(|binding| binding.ty.clone())
            .ok_or_else(|| Diagnostic::new("E101", expr.span, format!("unknown name {name}"))),
        ExprKind::Binary(left, op, right) => {
            let lhs = type_of(left, env, signatures, return_type)?;
            let rhs = type_of(right, env, signatures, return_type)?;
            if lhs == Type::Never || rhs == Type::Never {
                return Ok(Type::Never);
            }
            match op {
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Lt | Op::Le | Op::Gt | Op::Ge
                    if lhs == Type::I32 && rhs == Type::I32 =>
                {
                    if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div) {
                        Ok(Type::I32)
                    } else {
                        Ok(Type::Bool)
                    }
                }
                Op::Eq | Op::Ne if lhs == rhs && matches!(lhs, Type::I32 | Type::Bool) => {
                    Ok(Type::Bool)
                }
                _ => Err(Diagnostic::new(
                    "E104",
                    expr.span,
                    format!("invalid operands {lhs} and {rhs} for {op:?}"),
                )),
            }
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
                require(expected, &actual, arg.span, "argument")?;
            }
            Ok(signature.ret.clone())
        }
        ExprKind::If(condition, yes, no) => {
            let cond_type = type_of(condition, env, signatures, return_type)?;
            require(&Type::Bool, &cond_type, condition.span, "condition")?;
            let yes_type = type_of(yes, env, signatures, return_type)?;
            let no_type = type_of(no, env, signatures, return_type)?;
            join(&yes_type, &no_type).ok_or_else(|| {
                Diagnostic::new(
                    "E102",
                    expr.span,
                    format!("branches have different types: {yes_type} and {no_type}"),
                )
            })
        }
        ExprKind::Block(stmts, tail) => {
            let mut scope = env.clone();
            let mut declared = HashSet::new();
            for (index, stmt) in stmts.iter().enumerate() {
                let has_following = index + 1 < stmts.len() || tail.is_some();
                let (statement_type, span) = match stmt {
                    Stmt::Let {
                        name,
                        ty,
                        value,
                        mutable,
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
                        require(ty, &actual, value.span, "binding")?;
                        scope.insert(
                            name.clone(),
                            Binding {
                                ty: ty.clone(),
                                mutable: *mutable,
                            },
                        );
                        (actual, *span)
                    }
                    Stmt::Assign { name, value, span } => {
                        let binding = scope.get(name).ok_or_else(|| {
                            Diagnostic::new("E101", *span, format!("unknown name {name}"))
                        })?;
                        if !binding.mutable {
                            return Err(Diagnostic::new(
                                "E109",
                                *span,
                                format!("cannot assign immutable binding {name}"),
                            ));
                        }
                        let actual = type_of(value, &scope, signatures, return_type)?;
                        require(&binding.ty, &actual, value.span, "assignment")?;
                        (actual, *span)
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        span,
                    } => {
                        let iter_type = type_of(iterable, &scope, signatures, return_type)?;
                        let Type::Array(element) = iter_type else {
                            return Err(Diagnostic::new(
                                "E110",
                                iterable.span,
                                format!("for requires an array, got {iter_type}"),
                            ));
                        };
                        let mut loop_scope = scope.clone();
                        loop_scope.insert(
                            name.clone(),
                            Binding {
                                ty: *element,
                                mutable: false,
                            },
                        );
                        type_of(body, &loop_scope, signatures, return_type)?;
                        (Type::Unit, *span)
                    }
                    Stmt::Return { value, span } => {
                        let actual = type_of(value, &scope, signatures, return_type)?;
                        require(return_type, &actual, value.span, "return")?;
                        (Type::Never, *span)
                    }
                    Stmt::Expr(value) => {
                        (type_of(value, &scope, signatures, return_type)?, value.span)
                    }
                };
                if statement_type == Type::Never {
                    if has_following {
                        return Err(Diagnostic::new("E107", span, "unreachable code"));
                    }
                    return Ok(Type::Never);
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

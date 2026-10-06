use std::collections::{HashMap, HashSet};

use crate::ast::{
    EnumVariant, Expr, ExprKind, Op, Pattern, PatternKind, PlaceStep, Program, Span, Stmt, Type,
};
use crate::builtins;
use crate::diagnostic::Diagnostic;

#[derive(Clone)]
struct Signature {
    type_params: Vec<String>,
    params: Vec<Type>,
    ret: Type,
    fields: Option<Vec<(String, Type)>>,
    variants: Option<Vec<EnumVariant>>,
    spawn_safe: bool,
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    mutable: bool,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    check_with_types(program).map(|_| ())
}

pub fn check_with_types(program: &Program) -> Result<HashMap<Span, Type>, Diagnostic> {
    if let Some(import) = program.imports.first() {
        return Err(Diagnostic::new(
            "E118",
            import.span,
            "imports require a file-backed program loader",
        ));
    }
    let mut signatures = HashMap::new();
    let mut types = HashMap::new();
    let mut record_names = HashSet::new();
    let mut arities = HashMap::new();
    for builtin in [
        builtins::io_error_decl(),
        builtins::task_error_decl(),
        builtins::parse_error_decl(),
    ] {
        record_names.insert(builtin.name.clone());
        arities.insert(builtin.name.clone(), 0);
        signatures.insert(
            builtin.name.clone(),
            Signature {
                type_params: Vec::new(),
                params: Vec::new(),
                ret: Type::Named(builtin.name),
                fields: None,
                variants: Some(builtin.variants),
                spawn_safe: false,
            },
        );
    }
    for (name, params, ret) in [
        (
            builtins::READ_TEXT,
            vec![Type::String],
            builtins::read_text_result(),
        ),
        (
            builtins::READ_BYTES,
            vec![Type::String],
            builtins::read_bytes_result(),
        ),
        (
            builtins::WRITE_TEXT,
            vec![Type::String, Type::String],
            builtins::write_text_result(),
        ),
        (
            builtins::WRITE_BYTES,
            vec![Type::String, Type::Bytes],
            builtins::write_text_result(),
        ),
        (
            builtins::LINES,
            vec![Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (
            builtins::ARGS,
            Vec::new(),
            Type::Array(Box::new(Type::String)),
        ),
        (builtins::PRINT, vec![Type::String], Type::Unit),
        (builtins::TO_STRING, vec![Type::I32], Type::String),
        (
            builtins::CHARS,
            vec![Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (
            builtins::SPLIT,
            vec![Type::String, Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (builtins::TRIM, vec![Type::String], Type::String),
        (
            builtins::CONTAINS,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::STARTS_WITH,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::ENDS_WITH,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::REPLACE,
            vec![Type::String, Type::String, Type::String],
            Type::String,
        ),
        (builtins::LOWER, vec![Type::String], Type::String),
        (builtins::UPPER, vec![Type::String], Type::String),
        (
            builtins::READ_LINE,
            Vec::new(),
            Type::Option(Box::new(Type::String)),
        ),
        (
            builtins::READ_STDIN,
            Vec::new(),
            builtins::read_text_result(),
        ),
        (builtins::EXIT, vec![Type::I32], Type::Never),
        (
            builtins::PARSE_I32,
            vec![Type::String],
            builtins::parse_i32_result(),
        ),
        (
            builtins::PARSE_I64,
            vec![Type::String],
            builtins::parse_i64_result(),
        ),
        (
            builtins::PARSE_F64,
            vec![Type::String],
            builtins::parse_f64_result(),
        ),
        (builtins::WIDEN_I64, vec![Type::I32], Type::I64),
        (builtins::TO_F64, vec![Type::I32], Type::F64),
        (
            builtins::NARROW_I32,
            vec![Type::I64],
            Type::Option(Box::new(Type::I32)),
        ),
        (
            builtins::UTF8_BYTES,
            vec![Type::String],
            Type::Array(Box::new(Type::I32)),
        ),
        (
            builtins::UTF8_DECODE,
            vec![Type::Array(Box::new(Type::I32))],
            Type::Option(Box::new(Type::String)),
        ),
        (builtins::UTF8_ENCODE, vec![Type::String], Type::Bytes),
        (
            builtins::UTF8_DECODE_BYTES,
            vec![Type::Bytes],
            Type::Option(Box::new(Type::String)),
        ),
        (
            builtins::BYTES_FROM_I32,
            vec![Type::Array(Box::new(Type::I32))],
            Type::Option(Box::new(Type::Bytes)),
        ),
        (
            builtins::BYTES_TO_I32,
            vec![Type::Bytes],
            Type::Array(Box::new(Type::I32)),
        ),
    ] {
        signatures.insert(
            name.to_owned(),
            Signature {
                type_params: Vec::new(),
                params,
                ret,
                fields: None,
                variants: None,
                spawn_safe: false,
            },
        );
    }
    signatures.insert(
        builtins::LEN.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Array(Box::new(Type::Param("T".to_owned())))],
            ret: Type::I32,
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::JOIN.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Task(Box::new(Type::Param("T".to_owned())))],
            ret: Type::Result(
                Box::new(Type::Param("T".to_owned())),
                Box::new(Type::Named(builtins::TASK_ERROR.to_owned())),
            ),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    for record in &program.records {
        if matches!(
            record.name.as_str(),
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
                | "read_text"
                | "read_bytes"
                | "write_text"
                | "write_bytes"
                | "lines"
                | "args"
                | "print"
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
        ) || !record_names.insert(record.name.clone())
        {
            return Err(Diagnostic::new(
                "E106",
                record.span,
                format!("duplicate or reserved type {}", record.name),
            ));
        }
        arities.insert(record.name.clone(), record.type_params.len());
    }
    for enum_decl in &program.enums {
        if matches!(
            enum_decl.name.as_str(),
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
                | "read_text"
                | "read_bytes"
                | "write_text"
                | "write_bytes"
                | "lines"
                | "args"
                | "print"
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
        ) || !record_names.insert(enum_decl.name.clone())
        {
            return Err(Diagnostic::new(
                "E106",
                enum_decl.span,
                format!("duplicate or reserved type {}", enum_decl.name),
            ));
        }
        arities.insert(enum_decl.name.clone(), 0);
        let mut names = HashSet::new();
        for variant in &enum_decl.variants {
            if !names.insert(&variant.name) {
                return Err(Diagnostic::new(
                    "E106",
                    enum_decl.span,
                    format!("duplicate variant {}", variant.name),
                ));
            }
        }
        signatures.insert(
            enum_decl.name.clone(),
            Signature {
                type_params: Vec::new(),
                params: Vec::new(),
                ret: Type::Named(enum_decl.name.clone()),
                fields: None,
                variants: Some(enum_decl.variants.clone()),
                spawn_safe: false,
            },
        );
    }
    for enum_decl in &program.enums {
        for variant in &enum_decl.variants {
            if let Some(payload) = &variant.payload {
                validate_type(payload, &arities, enum_decl.span)?;
            }
        }
    }
    for record in &program.records {
        validate_params(&record.type_params, &record_names, record.span)?;
        for param in &record.type_params {
            if !record
                .fields
                .iter()
                .any(|(_, ty)| mentions_param(ty, param))
            {
                return Err(Diagnostic::new(
                    "E115",
                    record.span,
                    format!("unused record type parameter {param}"),
                ));
            }
        }
        let mut names = HashSet::new();
        for (name, ty) in &record.fields {
            if !names.insert(name) {
                return Err(Diagnostic::new(
                    "E106",
                    record.span,
                    format!("duplicate field {name}"),
                ));
            }
            validate_type(ty, &arities, record.span)?;
        }
        let signature = Signature {
            type_params: record.type_params.clone(),
            params: record.fields.iter().map(|(_, ty)| ty.clone()).collect(),
            ret: if record.type_params.is_empty() {
                Type::Named(record.name.clone())
            } else {
                Type::Applied(
                    record.name.clone(),
                    record
                        .type_params
                        .iter()
                        .map(|name| Type::Param(name.clone()))
                        .collect(),
                )
            },
            fields: Some(record.fields.clone()),
            variants: None,
            spawn_safe: false,
        };
        signatures.insert(record.name.clone(), signature);
    }
    for record in &program.records {
        if has_record_cycle(&record.name, program, &mut HashSet::new()) {
            return Err(Diagnostic::new(
                "E112",
                record.span,
                "recursive record value layout",
            ));
        }
    }
    for enum_decl in &program.enums {
        if enum_has_cycle(&enum_decl.name, program, &mut HashSet::new()) {
            return Err(Diagnostic::new(
                "E112",
                enum_decl.span,
                "recursive enum value layout",
            ));
        }
    }
    for function in &program.functions {
        validate_params(&function.type_params, &record_names, function.span)?;
        for (_, ty) in &function.params {
            validate_type(ty, &arities, function.span)?;
        }
        validate_type(&function.ret, &arities, function.span)?;
        let signature = Signature {
            type_params: function.type_params.clone(),
            params: function.params.iter().map(|(_, ty)| ty.clone()).collect(),
            ret: function.ret.clone(),
            fields: None,
            variants: None,
            spawn_safe: function_is_spawn_safe(&function.name, program, &mut HashSet::new()),
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
        let body_type = type_of(&function.body, &env, &signatures, &function.ret, &mut types)?;
        require(
            &function.ret,
            &body_type,
            function.body.span,
            "function result",
        )?;
    }
    Ok(types)
}

fn validate_params(
    params: &[String],
    names: &HashSet<String>,
    span: Span,
) -> Result<(), Diagnostic> {
    let mut seen = HashSet::new();
    for param in params {
        if !seen.insert(param)
            || names.contains(param)
            || matches!(
                param.as_str(),
                "i32" | "bool" | "String" | "Unit" | "Result" | "Option" | "Task"
            )
        {
            return Err(Diagnostic::new(
                "E106",
                span,
                format!("duplicate or reserved type parameter {param}"),
            ));
        }
    }
    Ok(())
}

fn function_is_spawn_safe(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.name == name)
    else {
        return false;
    };
    if !function.type_params.is_empty() {
        return false;
    }
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let safe = expression_is_spawn_safe(&function.body, program, visiting);
    visiting.remove(name);
    safe
}

fn expression_is_spawn_safe(
    expr: &Expr,
    program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::I64(_)
        | ExprKind::F64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_)
        | ExprKind::None => true,
        ExprKind::Variant(_, _, payload) => payload
            .as_ref()
            .is_none_or(|value| expression_is_spawn_safe(value, program, visiting)),
        ExprKind::Array(items) => items
            .iter()
            .all(|item| expression_is_spawn_safe(item, program, visiting)),
        ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
            expression_is_spawn_safe(left, program, visiting)
                && expression_is_spawn_safe(right, program, visiting)
        }
        ExprKind::Field(value, _)
        | ExprKind::Not(value)
        | ExprKind::Neg(value)
        | ExprKind::Ok(value)
        | ExprKind::Err(value)
        | ExprKind::Some(value)
        | ExprKind::Try(value) => expression_is_spawn_safe(value, program, visiting),
        ExprKind::Call(name, args) => {
            name != builtins::READ_TEXT
                && name != builtins::READ_BYTES
                && name != builtins::WRITE_TEXT
                && name != builtins::WRITE_BYTES
                && name != builtins::ARGS
                && name != builtins::PRINT
                && name != builtins::READ_LINE
                && name != builtins::READ_STDIN
                && name != builtins::EXIT
                && (name != builtins::JOIN || args.len() == 2)
                && (matches!(
                    name.as_str(),
                    builtins::LINES
                        | builtins::LEN
                        | builtins::PARSE_I32
                        | builtins::PARSE_I64
                        | builtins::PARSE_F64
                        | builtins::JOIN
                        | builtins::TO_STRING
                        | builtins::CHARS
                        | builtins::SPLIT
                        | builtins::TRIM
                        | builtins::CONTAINS
                        | builtins::STARTS_WITH
                        | builtins::ENDS_WITH
                        | builtins::REPLACE
                        | builtins::LOWER
                        | builtins::UPPER
                        | builtins::WIDEN_I64
                        | builtins::TO_F64
                        | builtins::NARROW_I32
                        | builtins::UTF8_BYTES
                        | builtins::UTF8_DECODE
                        | builtins::UTF8_ENCODE
                        | builtins::UTF8_DECODE_BYTES
                        | builtins::BYTES_FROM_I32
                        | builtins::BYTES_TO_I32
                ) || program.records.iter().any(|record| record.name == *name)
                    || function_is_spawn_safe(name, program, visiting))
                && args
                    .iter()
                    .all(|arg| expression_is_spawn_safe(arg, program, visiting))
        }
        ExprKind::Spawn(_) => false,
        ExprKind::If(condition, yes, no) => [condition, yes, no]
            .iter()
            .all(|value| expression_is_spawn_safe(value, program, visiting)),
        ExprKind::Match(value, arms) => {
            expression_is_spawn_safe(value, program, visiting)
                && arms
                    .iter()
                    .all(|(_, body)| expression_is_spawn_safe(body, program, visiting))
        }
        ExprKind::Block(stmts, tail) => {
            stmts.iter().all(|stmt| match stmt {
                Stmt::Assign { path, value, .. } => {
                    path.iter().all(|step| match step {
                        PlaceStep::Index(index, _) => {
                            expression_is_spawn_safe(index, program, visiting)
                        }
                        PlaceStep::Field(..) => true,
                    }) && expression_is_spawn_safe(value, program, visiting)
                }
                Stmt::Let { value, .. }
                | Stmt::Push { value, .. }
                | Stmt::Return { value, .. }
                | Stmt::Expr(value) => expression_is_spawn_safe(value, program, visiting),
                Stmt::Break { .. } | Stmt::Continue { .. } => true,
                Stmt::For { iterable, body, .. } => {
                    expression_is_spawn_safe(iterable, program, visiting)
                        && expression_is_spawn_safe(body, program, visiting)
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    expression_is_spawn_safe(condition, program, visiting)
                        && expression_is_spawn_safe(body, program, visiting)
                }
            }) && tail
                .as_ref()
                .is_none_or(|value| expression_is_spawn_safe(value, program, visiting))
        }
    }
}

fn mentions_param(ty: &Type, param: &str) -> bool {
    match ty {
        Type::Param(name) => name == param,
        Type::Applied(_, args) => args.iter().any(|arg| mentions_param(arg, param)),
        Type::Array(element) => mentions_param(element, param),
        Type::Option(element) => mentions_param(element, param),
        Type::Task(result) => mentions_param(result, param),
        Type::Result(ok, err) => mentions_param(ok, param) || mentions_param(err, param),
        _ => false,
    }
}

fn validate_type(
    ty: &Type,
    arities: &HashMap<String, usize>,
    span: Span,
) -> Result<(), Diagnostic> {
    match ty {
        Type::Named(name) if arities.get(name) != Some(&0) => Err(Diagnostic::new(
            "E103",
            span,
            format!("unknown or unapplied type {name}"),
        )),
        Type::Applied(name, args) => {
            if arities.get(name) != Some(&args.len()) {
                return Err(Diagnostic::new(
                    "E103",
                    span,
                    format!("invalid type arguments for {name}"),
                ));
            }
            for arg in args {
                validate_type(arg, arities, span)?;
            }
            Ok(())
        }
        Type::Array(element) => validate_type(element, arities, span),
        Type::Option(element) => validate_type(element, arities, span),
        Type::Task(result) => validate_type(result, arities, span),
        Type::Result(ok, err) => {
            validate_type(ok, arities, span)?;
            validate_type(err, arities, span)
        }
        _ => Ok(()),
    }
}

fn has_record_cycle(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(record) = program.records.iter().find(|record| record.name == name) else {
        return false;
    };
    let args = record
        .type_params
        .iter()
        .map(|param| Type::Param(param.clone()))
        .collect::<Vec<_>>();
    record_has_cycle(name, &args, program, visiting)
}

fn record_has_cycle(
    name: &str,
    args: &[Type],
    program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let Some(record) = program.records.iter().find(|record| record.name == name) else {
        return enum_has_cycle(name, program, visiting);
    };
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let inferred: HashMap<String, Type> = record
        .type_params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    let cycle = record
        .fields
        .iter()
        .any(|(_, ty)| type_has_cycle(&substitute(ty, &inferred), program, visiting));
    visiting.remove(name);
    cycle
}

fn enum_has_cycle(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(decl) = program.enums.iter().find(|decl| decl.name == name) else {
        return false;
    };
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let cycle = decl
        .variants
        .iter()
        .filter_map(|variant| variant.payload.as_ref())
        .any(|payload| type_has_cycle(payload, program, visiting));
    visiting.remove(name);
    cycle
}

fn type_has_cycle(ty: &Type, program: &Program, visiting: &mut HashSet<String>) -> bool {
    match ty {
        Type::Named(name) => record_has_cycle(name, &[], program, visiting),
        Type::Applied(name, args) => record_has_cycle(name, args, program, visiting),
        Type::Array(_) => false,
        Type::Option(element) => type_has_cycle(element, program, visiting),
        Type::Task(_) => false,
        Type::Result(ok, err) => {
            type_has_cycle(ok, program, visiting) || type_has_cycle(err, program, visiting)
        }
        _ => false,
    }
}

fn compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual || *actual == Type::Never {
        return true;
    }
    match (expected, actual) {
        (Type::Array(_), Type::EmptyArray) => true,
        (Type::Array(expected), Type::Array(actual)) => compatible(expected, actual),
        (Type::Option(expected), Type::Option(actual)) => compatible(expected, actual),
        (Type::Task(expected), Type::Task(actual)) => compatible(expected, actual),
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            a_args.iter().zip(b_args).all(|(a, b)| compatible(a, b))
        }
        (Type::Result(expected_ok, expected_err), Type::Result(actual_ok, actual_err)) => {
            compatible(expected_ok, actual_ok) && compatible(expected_err, actual_err)
        }
        _ => false,
    }
}

fn binding_type_is_known(ty: &Type) -> bool {
    match ty {
        Type::Never | Type::EmptyArray => false,
        Type::Array(element) | Type::Option(element) | Type::Task(element) => {
            binding_type_is_known(element)
        }
        Type::Result(ok, err) => binding_type_is_known(ok) && binding_type_is_known(err),
        Type::Applied(_, args) => args.iter().all(binding_type_is_known),
        _ => true,
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
        (Type::Option(left), Type::Option(right)) => {
            Some(Type::Option(Box::new(join(left, right)?)))
        }
        (Type::Task(left), Type::Task(right)) => Some(Type::Task(Box::new(join(left, right)?))),
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            Some(Type::Applied(
                a_name.clone(),
                a_args
                    .iter()
                    .zip(b_args)
                    .map(|(a, b)| join(a, b))
                    .collect::<Option<Vec<_>>>()?,
            ))
        }
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

fn substitute(ty: &Type, inferred: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Param(name) => inferred.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Array(element) => Type::Array(Box::new(substitute(element, inferred))),
        Type::Option(element) => Type::Option(Box::new(substitute(element, inferred))),
        Type::Task(result) => Type::Task(Box::new(substitute(result, inferred))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute(ok, inferred)),
            Box::new(substitute(err, inferred)),
        ),
        Type::Applied(name, args) => Type::Applied(
            name.clone(),
            args.iter().map(|arg| substitute(arg, inferred)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn infer_params(
    pattern: &Type,
    actual: &Type,
    inferred: &mut HashMap<String, Type>,
    span: Span,
) -> Result<(), Diagnostic> {
    if matches!(actual, Type::Never | Type::EmptyArray) {
        return Ok(());
    }
    match (pattern, actual) {
        (Type::Param(name), actual) => {
            if let Some(previous) = inferred.get(name) {
                let common = join(previous, actual).ok_or_else(|| {
                    Diagnostic::new(
                        "E102",
                        span,
                        format!("incompatible type arguments {previous} and {actual}"),
                    )
                })?;
                inferred.insert(name.clone(), common);
                Ok(())
            } else {
                inferred.insert(name.clone(), actual.clone());
                Ok(())
            }
        }
        (Type::Array(a), Type::Array(b)) => infer_params(a, b, inferred, span),
        (Type::Option(a), Type::Option(b)) => infer_params(a, b, inferred, span),
        (Type::Task(a), Type::Task(b)) => infer_params(a, b, inferred, span),
        (Type::Result(a_ok, a_err), Type::Result(b_ok, b_err)) => {
            infer_params(a_ok, b_ok, inferred, span)?;
            infer_params(a_err, b_err, inferred, span)
        }
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            for (a, b) in a_args.iter().zip(b_args) {
                infer_params(a, b, inferred, span)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn match_pattern(
    pattern: &Pattern,
    matched: &Type,
    signatures: &HashMap<String, Signature>,
) -> Result<(String, Option<(String, Type)>), Diagnostic> {
    let invalid = || {
        Diagnostic::new(
            "E116",
            pattern.span,
            format!("pattern does not match {matched}"),
        )
    };
    match (&pattern.kind, matched) {
        (PatternKind::Int(value), Type::I32) => Ok((value.to_string(), None)),
        (PatternKind::I64(value), Type::I64) => Ok((value.to_string(), None)),
        (PatternKind::Wildcard, _) => Ok(("_".to_owned(), None)),
        (PatternKind::Ok(name), Type::Result(ok, _)) => {
            Ok(("Ok".to_owned(), Some((name.clone(), *ok.clone()))))
        }
        (PatternKind::Err(name), Type::Result(_, err)) => {
            Ok(("Err".to_owned(), Some((name.clone(), *err.clone()))))
        }
        (PatternKind::Some(name), Type::Option(element)) => {
            Ok(("Some".to_owned(), Some((name.clone(), *element.clone()))))
        }
        (PatternKind::None, Type::Option(_)) => Ok(("None".to_owned(), None)),
        (PatternKind::Bool(value), Type::Bool) => Ok((value.to_string(), None)),
        (PatternKind::Variant(name, variant, binding), Type::Named(actual)) if name == actual => {
            let declared = signatures
                .get(name)
                .and_then(|signature| signature.variants.as_ref())
                .and_then(|variants| variants.iter().find(|item| item.name == *variant));
            match (declared.and_then(|item| item.payload.as_ref()), binding) {
                (None, None) if declared.is_some() => Ok((variant.clone(), None)),
                (Some(ty), Some(name)) => Ok((variant.clone(), Some((name.clone(), ty.clone())))),
                _ => Err(invalid()),
            }
        }
        _ => Err(invalid()),
    }
}

fn type_of(
    expr: &Expr,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    let inferred = infer(expr, env, signatures, return_type, types)?;
    types.insert(expr.span, inferred.clone());
    Ok(inferred)
}

fn infer(
    expr: &Expr,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    match &expr.kind {
        ExprKind::Int(_) => Ok(Type::I32),
        ExprKind::I64(_) => Ok(Type::I64),
        ExprKind::F64(_) => Ok(Type::F64),
        ExprKind::Neg(value) => {
            let actual = type_of(value, env, signatures, return_type, types)?;
            if matches!(actual, Type::I32 | Type::I64 | Type::F64 | Type::Never) {
                Ok(actual)
            } else {
                Err(Diagnostic::new(
                    "E104",
                    expr.span,
                    "negation requires i32 or i64",
                ))
            }
        }
        ExprKind::Bool(_) => Ok(Type::Bool),
        ExprKind::String(_) => Ok(Type::String),
        ExprKind::Variant(name, variant, payload) => {
            let signature = signatures.get(name).ok_or_else(|| {
                Diagnostic::new("E103", expr.span, format!("unknown enum {name}"))
            })?;
            let Some(variants) = &signature.variants else {
                return Err(Diagnostic::new(
                    "E114",
                    expr.span,
                    format!("{name} is not an enum"),
                ));
            };
            let declared = variants
                .iter()
                .find(|item| item.name == *variant)
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E114",
                        expr.span,
                        format!("unknown variant {name}::{variant}"),
                    )
                })?;
            match (&declared.payload, payload) {
                (None, None) => {}
                (Some(expected), Some(value)) => {
                    let actual = type_of(value, env, signatures, return_type, types)?;
                    if actual == Type::Never {
                        return Ok(Type::Never);
                    }
                    require(expected, &actual, value.span, "enum payload")?;
                }
                _ => {
                    return Err(Diagnostic::new(
                        "E114",
                        expr.span,
                        format!("wrong payload shape for {name}::{variant}"),
                    ));
                }
            }
            Ok(Type::Named(name.clone()))
        }
        ExprKind::Array(values) => {
            let Some(first) = values.first() else {
                return Ok(Type::EmptyArray);
            };
            let mut element = type_of(first, env, signatures, return_type, types)?;
            if element == Type::Never {
                return Ok(Type::Never);
            }
            for value in values.iter().skip(1) {
                let actual = type_of(value, env, signatures, return_type, types)?;
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
        ExprKind::Index(array, index) => {
            let array_type = type_of(array, env, signatures, return_type, types)?;
            let index_type = type_of(index, env, signatures, return_type, types)?;
            if array_type == Type::Never || index_type == Type::Never {
                return Ok(Type::Never);
            }
            require(&Type::I32, &index_type, index.span, "array index")?;
            match array_type {
                Type::Array(element) => Ok(*element),
                Type::Bytes => Ok(Type::I32),
                _ => Err(Diagnostic::new(
                    "E110",
                    array.span,
                    "indexing requires an array or Bytes",
                )),
            }
        }
        ExprKind::Field(value, field) => {
            let actual = type_of(value, env, signatures, return_type, types)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            if !matches!(actual, Type::Named(_) | Type::Applied(..)) {
                return Err(Diagnostic::new(
                    "E113",
                    value.span,
                    "field access requires a record",
                ));
            }
            field_type(actual, field, signatures, expr.span)
        }
        ExprKind::Ok(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(inner), Box::new(Type::Never)))
        }
        ExprKind::Err(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(Type::Never), Box::new(inner)))
        }
        ExprKind::Some(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Option(Box::new(inner)))
        }
        ExprKind::None => Ok(Type::Option(Box::new(Type::Never))),
        ExprKind::Try(inner) => {
            let actual = type_of(inner, env, signatures, return_type, types)?;
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
        ExprKind::Not(inner) => {
            let actual = type_of(inner, env, signatures, return_type, types)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            require(&Type::Bool, &actual, inner.span, "logical negation")?;
            Ok(Type::Bool)
        }
        ExprKind::Binary(left, op, right) => {
            let lhs = type_of(left, env, signatures, return_type, types)?;
            let rhs = type_of(right, env, signatures, return_type, types)?;
            if lhs == Type::Never {
                return Ok(Type::Never);
            }
            if matches!(op, Op::And | Op::Or) {
                return if lhs == Type::Bool && matches!(rhs, Type::Bool | Type::Never) {
                    Ok(Type::Bool)
                } else {
                    Err(Diagnostic::new(
                        "E104",
                        expr.span,
                        format!("invalid operands {lhs} and {rhs} for {op:?}"),
                    ))
                };
            }
            if rhs == Type::Never {
                return Ok(Type::Never);
            }
            match op {
                Op::Add
                | Op::Sub
                | Op::Mul
                | Op::Div
                | Op::Rem
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                    if lhs == rhs && matches!(lhs, Type::I32 | Type::I64 | Type::F64) =>
                {
                    if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem) {
                        Ok(lhs)
                    } else {
                        Ok(Type::Bool)
                    }
                }
                Op::Add if lhs == Type::String && rhs == Type::String => Ok(Type::String),
                Op::Lt | Op::Le | Op::Gt | Op::Ge if lhs == Type::String && rhs == Type::String => {
                    Ok(Type::Bool)
                }
                Op::Eq | Op::Ne
                    if lhs == rhs
                        && matches!(
                            lhs,
                            Type::I32
                                | Type::I64
                                | Type::F64
                                | Type::Bool
                                | Type::String
                                | Type::Bytes
                        ) =>
                {
                    Ok(Type::Bool)
                }
                _ => Err(Diagnostic::new(
                    "E104",
                    expr.span,
                    format!("invalid operands {lhs} and {rhs} for {op:?}"),
                )),
            }
        }
        ExprKind::Call(name, args) if name == builtins::JOIN && args.len() == 2 => {
            let parts = type_of(&args[0], env, signatures, return_type, types)?;
            let separator = type_of(&args[1], env, signatures, return_type, types)?;
            require(
                &Type::Array(Box::new(Type::String)),
                &parts,
                args[0].span,
                "argument",
            )?;
            require(&Type::String, &separator, args[1].span, "argument")?;
            Ok(Type::String)
        }
        ExprKind::Call(name, args) => {
            let signature = signatures.get(name).ok_or_else(|| {
                Diagnostic::new("E101", expr.span, format!("unknown function {name}"))
            })?;
            if signature.variants.is_some() {
                return Err(Diagnostic::new(
                    "E114",
                    expr.span,
                    "enum values require a qualified variant",
                ));
            }
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
            let actuals = args
                .iter()
                .map(|arg| type_of(arg, env, signatures, return_type, types))
                .collect::<Result<Vec<_>, _>>()?;
            if name == builtins::LEN && actuals == [Type::Bytes] {
                return Ok(Type::I32);
            }
            if actuals == [Type::F64] {
                if name == builtins::NARROW_I32 {
                    return Ok(Type::Option(Box::new(Type::I32)));
                }
                if name == builtins::WIDEN_I64 {
                    return Ok(Type::Option(Box::new(Type::I64)));
                }
            }
            if name == builtins::TO_F64 && actuals == [Type::I64] {
                return Ok(Type::F64);
            }
            if name == builtins::TO_STRING
                && matches!(actuals[..], [Type::I64 | Type::F64 | Type::Bool])
            {
                return Ok(Type::String);
            }
            let mut inferred = HashMap::new();
            for ((arg, expected), actual) in args.iter().zip(&signature.params).zip(&actuals) {
                infer_params(expected, actual, &mut inferred, arg.span)?;
            }
            for name in &signature.type_params {
                if !inferred.contains_key(name) {
                    return Err(Diagnostic::new(
                        "E115",
                        expr.span,
                        format!("cannot infer type argument {name}"),
                    ));
                }
            }
            for ((arg, expected), actual) in args.iter().zip(&signature.params).zip(&actuals) {
                require(
                    &substitute(expected, &inferred),
                    actual,
                    arg.span,
                    "argument",
                )?;
            }
            Ok(substitute(&signature.ret, &inferred))
        }
        ExprKind::Spawn(call) => {
            let ExprKind::Call(name, _) = &call.kind else {
                return Err(Diagnostic::new(
                    "E117",
                    call.span,
                    "spawn requires a named function call",
                ));
            };
            let result = type_of(call, env, signatures, return_type, types)?;
            if !signatures
                .get(name)
                .is_some_and(|signature| signature.spawn_safe)
            {
                return Err(Diagnostic::new(
                    "E117",
                    expr.span,
                    format!(
                        "{name} cannot be spawned: only non-generic pure functions are supported"
                    ),
                ));
            }
            Ok(Type::Task(Box::new(result)))
        }
        ExprKind::If(condition, yes, no) => {
            let cond_type = type_of(condition, env, signatures, return_type, types)?;
            require(&Type::Bool, &cond_type, condition.span, "condition")?;
            let yes_type = type_of(yes, env, signatures, return_type, types)?;
            let no_type = type_of(no, env, signatures, return_type, types)?;
            join(&yes_type, &no_type).ok_or_else(|| {
                let message = if no.span.start == no.span.end {
                    format!("if without else must have type Unit, found {yes_type}")
                } else {
                    format!("branches have different types: {yes_type} and {no_type}")
                };
                Diagnostic::new("E102", expr.span, message)
            })
        }
        ExprKind::Match(value, arms) => {
            let matched = type_of(value, env, signatures, return_type, types)?;
            if matched == Type::Never {
                return Ok(Type::Never);
            }
            if matched == Type::Option(Box::new(Type::Never)) {
                return Err(Diagnostic::new(
                    "E115",
                    value.span,
                    "cannot infer Option element type for match",
                ));
            }
            let integer_match = matches!(matched, Type::I32 | Type::I64);
            let expected: HashSet<String> = match &matched {
                Type::I32 | Type::I64 => HashSet::new(),
                Type::Result(_, _) => ["Ok".to_owned(), "Err".to_owned()].into_iter().collect(),
                Type::Option(_) => ["Some".to_owned(), "None".to_owned()].into_iter().collect(),
                Type::Bool => ["true".to_owned(), "false".to_owned()]
                    .into_iter()
                    .collect(),
                Type::Named(name) => signatures
                    .get(name)
                    .and_then(|signature| signature.variants.as_ref())
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "E116",
                            value.span,
                            "match requires integer, result, enum, or bool",
                        )
                    })?
                    .iter()
                    .map(|variant| variant.name.clone())
                    .collect(),
                _ => {
                    return Err(Diagnostic::new(
                        "E116",
                        value.span,
                        "match requires integer, result, enum, or bool",
                    ));
                }
            };
            let mut seen = HashSet::new();
            let mut wildcard = false;
            let mut result = None;
            for (pattern, body) in arms {
                let (key, binding) = match_pattern(pattern, &matched, signatures)?;
                if wildcard {
                    return Err(Diagnostic::new(
                        "E116",
                        pattern.span,
                        "unreachable match arm after wildcard",
                    ));
                }
                if matches!(&pattern.kind, PatternKind::Wildcard) {
                    if !integer_match && seen == expected {
                        return Err(Diagnostic::new(
                            "E116",
                            pattern.span,
                            "wildcard arm is unreachable",
                        ));
                    }
                    wildcard = true;
                } else if !seen.insert(key.clone()) {
                    return Err(Diagnostic::new(
                        "E116",
                        pattern.span,
                        format!("duplicate match arm {key}"),
                    ));
                }
                let mut scope = env.clone();
                if let Some((name, ty)) = binding {
                    scope.insert(name, Binding { ty, mutable: false });
                }
                let branch = type_of(body, &scope, signatures, return_type, types)?;
                result = Some(match result {
                    Some(previous) => join(&previous, &branch).ok_or_else(|| {
                        Diagnostic::new(
                            "E102",
                            body.span,
                            format!("match arms have different types: {previous} and {branch}"),
                        )
                    })?,
                    None => branch,
                });
            }
            if !wildcard && (integer_match || seen != expected) {
                let mut missing: Vec<_> = expected.difference(&seen).cloned().collect();
                if integer_match {
                    missing.push("_".to_owned());
                }
                missing.sort();
                return Err(Diagnostic::new(
                    "E116",
                    expr.span,
                    format!("non-exhaustive match: missing {}", missing.join(",")),
                ));
            }
            Ok(result.unwrap_or(Type::Never))
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
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        let arities: HashMap<String, usize> = signatures
                            .iter()
                            .filter_map(|(name, signature)| {
                                (signature.fields.is_some() || signature.variants.is_some())
                                    .then_some((name.clone(), signature.type_params.len()))
                            })
                            .collect();
                        let binding_ty = if let Some(ty) = ty {
                            validate_type(ty, &arities, *span)?;
                            require(ty, &actual, value.span, "binding")?;
                            ty.clone()
                        } else if binding_type_is_known(&actual) {
                            actual.clone()
                        } else {
                            return Err(Diagnostic::new(
                                "E115",
                                value.span,
                                "binding needs a type annotation for this value",
                            ));
                        };
                        scope.insert(
                            name.clone(),
                            Binding {
                                ty: binding_ty,
                                mutable: *mutable,
                            },
                        );
                        (actual, *span)
                    }
                    Stmt::Assign {
                        name,
                        path,
                        value,
                        span,
                    } => {
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
                        let mut target = binding.ty.clone();
                        for step in path {
                            // The backends read each step's container type.
                            let container = target;
                            target = match step {
                                PlaceStep::Index(index, step_span) => {
                                    let index_type =
                                        type_of(index, &scope, signatures, return_type, types)?;
                                    require(&Type::I32, &index_type, index.span, "array index")?;
                                    types.insert(*step_span, container.clone());
                                    match container {
                                        Type::Array(element) => *element,
                                        Type::Bytes => Type::I32,
                                        _ => {
                                            return Err(Diagnostic::new(
                                                "E110",
                                                *step_span,
                                                "indexed assignment requires an array or Bytes",
                                            ));
                                        }
                                    }
                                }
                                PlaceStep::Field(field, step_span) => {
                                    types.insert(*step_span, container.clone());
                                    field_type(container, field, signatures, *step_span)?
                                }
                            };
                        }
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        require(&target, &actual, value.span, "assignment")?;
                        (actual, *span)
                    }
                    Stmt::Push { name, value, span } => {
                        let binding = scope.get(name).ok_or_else(|| {
                            Diagnostic::new("E101", *span, format!("unknown name {name}"))
                        })?;
                        if !binding.mutable {
                            return Err(Diagnostic::new(
                                "E109",
                                *span,
                                format!("cannot push to immutable binding {name}"),
                            ));
                        }
                        let element = match &binding.ty {
                            Type::Array(element) => element.as_ref(),
                            Type::Bytes => &Type::I32,
                            Type::String => &Type::String,
                            _ => {
                                return Err(Diagnostic::new(
                                    "E110",
                                    *span,
                                    format!(
                                        "push requires an array, Bytes, or String, got {}",
                                        binding.ty
                                    ),
                                ));
                            }
                        };
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        require(element, &actual, value.span, "append value")?;
                        types.insert(*span, binding.ty.clone());
                        (Type::Unit, *span)
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        span,
                    } => {
                        let iter_type = type_of(iterable, &scope, signatures, return_type, types)?;
                        let element = match iter_type {
                            Type::Array(element) => *element,
                            Type::Bytes => Type::I32,
                            _ => {
                                return Err(Diagnostic::new(
                                    "E110",
                                    iterable.span,
                                    format!("for requires an array or Bytes, got {iter_type}"),
                                ));
                            }
                        };
                        let mut loop_scope = scope.clone();
                        loop_scope.insert(
                            name.clone(),
                            Binding {
                                ty: element,
                                mutable: false,
                            },
                        );
                        type_of(body, &loop_scope, signatures, return_type, types)?;
                        (Type::Unit, *span)
                    }
                    Stmt::While {
                        condition,
                        body,
                        span,
                    } => {
                        let condition_type =
                            type_of(condition, &scope, signatures, return_type, types)?;
                        require(
                            &Type::Bool,
                            &condition_type,
                            condition.span,
                            "while condition",
                        )?;
                        type_of(body, &scope, signatures, return_type, types)?;
                        (Type::Unit, *span)
                    }
                    Stmt::Return { value, span } => {
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        require(return_type, &actual, value.span, "return")?;
                        (Type::Never, *span)
                    }
                    Stmt::Break { span } | Stmt::Continue { span } => (Type::Never, *span),
                    Stmt::Expr(value) => (
                        type_of(value, &scope, signatures, return_type, types)?,
                        value.span,
                    ),
                };
                if statement_type == Type::Never {
                    if has_following {
                        return Err(Diagnostic::new("E107", span, "unreachable code"));
                    }
                    return Ok(Type::Never);
                }
            }
            if let Some(tail) = tail {
                type_of(tail, &scope, signatures, return_type, types)
            } else {
                Ok(Type::Unit)
            }
        }
    }
}

/// The type of `field` on a record value of type `record`.
fn field_type(
    record: Type,
    field: &str,
    signatures: &HashMap<String, Signature>,
    span: Span,
) -> Result<Type, Diagnostic> {
    let (name, args) = match record {
        Type::Named(name) => (name, Vec::new()),
        Type::Applied(name, args) => (name, args),
        _ => {
            return Err(Diagnostic::new(
                "E113",
                span,
                "field access requires a record",
            ));
        }
    };
    let signature = signatures
        .get(&name)
        .ok_or_else(|| Diagnostic::new("E113", span, "unknown record"))?;
    let inferred: HashMap<String, Type> = signature.type_params.iter().cloned().zip(args).collect();
    signature
        .fields
        .as_ref()
        .and_then(|fields| fields.iter().find(|(candidate, _)| candidate == field))
        .map(|(_, ty)| substitute(ty, &inferred))
        .ok_or_else(|| Diagnostic::new("E113", span, format!("unknown field {field} on {name}")))
}

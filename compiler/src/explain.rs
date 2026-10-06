//! Deterministic, source-independent summaries of checked Tokit programs.

use std::collections::BTreeSet;

use crate::ast::{Expr, ExprKind, Op, PlaceStep, Program, Stmt};
use crate::builtins;

#[derive(Default)]
pub(crate) struct Facts {
    pub(crate) calls: BTreeSet<String>,
    pub(crate) constructors: BTreeSet<String>,
    pub(crate) record_names: BTreeSet<String>,
    pub(crate) effects: BTreeSet<&'static str>,
    operations: BTreeSet<&'static str>,
}

impl Facts {
    pub(crate) fn with_records(record_names: BTreeSet<String>) -> Self {
        Self {
            record_names,
            ..Self::default()
        }
    }
}

pub(crate) fn visit(expr: &Expr, facts: &mut Facts) {
    match &expr.kind {
        ExprKind::Lambda(_, body) => {
            facts.operations.insert("closure");
            visit(body, facts);
        }
        ExprKind::Apply(callee, args) => {
            facts.operations.insert("function value call");
            visit(callee, facts);
            args.iter().for_each(|arg| visit(arg, facts));
        }
        ExprKind::Int(_)
        | ExprKind::I64(_)
        | ExprKind::F64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_)
        | ExprKind::None => {}
        ExprKind::Variant(_, _, payload) => {
            if let Some(payload) = payload {
                visit(payload, facts);
            }
        }
        ExprKind::Array(values) => {
            for value in values {
                visit(value, facts);
            }
        }
        ExprKind::Index(array, index) => {
            facts.operations.insert("checked array indexing");
            visit(array, facts);
            visit(index, facts);
        }
        ExprKind::Field(value, _) => visit(value, facts),
        ExprKind::Not(value) => visit(value, facts),
        ExprKind::Neg(value) => {
            facts.operations.insert("arithmetic or concatenation");
            visit(value, facts);
        }
        ExprKind::Ok(value) | ExprKind::Err(value) | ExprKind::Some(value) => visit(value, facts),
        ExprKind::Try(value) => {
            facts.operations.insert("error propagation");
            visit(value, facts);
        }
        ExprKind::Binary(left, op, right) => {
            if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem) {
                facts.operations.insert("arithmetic or concatenation");
            }
            visit(left, facts);
            visit(right, facts);
        }
        ExprKind::Call(name, args) => {
            if name == builtins::READ_TEXT {
                facts.operations.insert("filesystem read (requires grant)");
                facts.effects.insert("fs.read");
            } else if name == builtins::READ_BYTES {
                facts
                    .operations
                    .insert("binary filesystem read (requires grant)");
                facts.effects.insert("fs.read");
            } else if name == builtins::WRITE_TEXT {
                facts.operations.insert("filesystem write (requires grant)");
                facts.effects.insert("fs.write");
            } else if name == builtins::WRITE_BYTES {
                facts
                    .operations
                    .insert("binary filesystem write (requires grant)");
                facts.effects.insert("fs.write");
            } else if name == builtins::LINES {
                facts.operations.insert("line splitting");
            } else if name == builtins::ARGS {
                facts.operations.insert("program arguments");
                facts.effects.insert("env.args");
            } else if matches!(
                name.as_str(),
                builtins::RANGE
                    | builtins::SORT
                    | builtins::REVERSE
                    | builtins::SLICE
                    | builtins::MAP_FN
                    | builtins::FILTER
                    | builtins::ANY
                    | builtins::ALL
                    | builtins::FOLD
                    | builtins::SORT_BY
            ) {
                facts.operations.insert("array operation");
            } else if matches!(
                name.as_str(),
                builtins::MAP
                    | builtins::GET
                    | builtins::GET_OR
                    | builtins::KEYS
                    | builtins::VALUES
                    | builtins::REMOVE
            ) {
                facts.operations.insert("map operation");
            } else if matches!(
                name.as_str(),
                builtins::ABS
                    | builtins::MIN
                    | builtins::MAX
                    | builtins::POW
                    | builtins::SQRT
                    | builtins::FLOOR
                    | builtins::CEIL
                    | builtins::ROUND
                    | builtins::EXP
                    | builtins::LN
                    | builtins::SIN
                    | builtins::COS
                    | builtins::TAN
                    | builtins::ATAN2
                    | builtins::PI
            ) {
                facts.operations.insert("math");
            } else if name == builtins::TO_STRING {
                facts.operations.insert("string conversion");
            } else if matches!(
                name.as_str(),
                builtins::CHARS
                    | builtins::SPLIT
                    | builtins::TRIM
                    | builtins::CONTAINS
                    | builtins::STARTS_WITH
                    | builtins::ENDS_WITH
                    | builtins::REPLACE
                    | builtins::LOWER
                    | builtins::UPPER
            ) || (name == builtins::JOIN && args.len() == 2)
            {
                facts.operations.insert("string operation");
            } else if matches!(
                name.as_str(),
                builtins::BIT_AND
                    | builtins::BIT_OR
                    | builtins::BIT_XOR
                    | builtins::BIT_NOT
                    | builtins::SHL
                    | builtins::SHR
            ) {
                facts.operations.insert("bitwise operation");
            } else if name == builtins::RANDOM_BYTES {
                facts.operations.insert("random bytes");
                facts.effects.insert("random");
            } else if matches!(
                name.as_str(),
                builtins::SHA256
                    | builtins::MD5
                    | builtins::SHA1
                    | builtins::HMAC_SHA256
                    | builtins::PBKDF2_SHA256
                    | builtins::BASE64_ENCODE
                    | builtins::BASE64_DECODE
                    | builtins::HEX
            ) {
                facts.operations.insert("hashing or encoding");
            } else if name == builtins::TCP_CONNECT {
                facts.operations.insert("TCP connection");
                facts.effects.insert("net.connect");
            } else if matches!(
                name.as_str(),
                builtins::TCP_SEND | builtins::TCP_RECV | builtins::TCP_CLOSE
            ) {
                facts.operations.insert("TCP transfer");
                facts.effects.insert("net.connect");
            } else if name == builtins::SERVE {
                facts.operations.insert("HTTP server");
                facts.effects.insert("net.listen");
            } else if name == builtins::HTTP_REQUEST {
                facts.operations.insert("HTTP request");
                facts.effects.insert("net.connect");
            } else if name == builtins::LIST_DIR || name == builtins::EXISTS {
                facts.operations.insert("filesystem query");
                facts.effects.insert("fs.read");
            } else if name == builtins::MAKE_DIR || name == builtins::REMOVE_FILE {
                facts.operations.insert("filesystem change");
                facts.effects.insert("fs.write");
            } else if name == builtins::ENV {
                facts.operations.insert("environment variable");
                facts.effects.insert("env.read");
            } else if name == builtins::NOW_MS || name == builtins::CLOCK_NS {
                facts.operations.insert("clock");
                facts.effects.insert("time.read");
            } else if name == builtins::SLEEP_MS {
                facts.operations.insert("sleep");
                facts.effects.insert("time.sleep");
            } else if name == builtins::PRINT {
                facts.operations.insert("standard output");
                facts.effects.insert("io.stdout");
            } else if name == builtins::READ_LINE || name == builtins::READ_STDIN {
                facts.operations.insert("standard input");
                facts.effects.insert("io.stdin");
            } else if name == builtins::EXIT {
                facts.operations.insert("process exit");
                facts.effects.insert("process.exit");
            } else if name == builtins::LEN {
                facts.operations.insert("array or byte length");
            } else if name == builtins::PARSE_I32 || name == builtins::PARSE_I64 {
                facts.operations.insert("integer parsing");
            } else if name == builtins::PARSE_F64 {
                facts.operations.insert("float parsing");
            } else if name == builtins::WIDEN_I64
                || name == builtins::NARROW_I32
                || name == builtins::TO_F64
            {
                facts.operations.insert("numeric conversion");
            } else if name == builtins::UTF8_BYTES {
                facts.operations.insert("UTF-8 encoding");
            } else if name == builtins::UTF8_DECODE {
                facts.operations.insert("UTF-8 decoding");
            } else if name == builtins::UTF8_ENCODE {
                facts.operations.insert("UTF-8 byte encoding");
            } else if name == builtins::UTF8_DECODE_BYTES {
                facts.operations.insert("UTF-8 byte decoding");
            } else if name == builtins::BYTES_FROM_I32 {
                facts.operations.insert("checked byte conversion");
            } else if name == builtins::BYTES_TO_I32 {
                facts.operations.insert("byte expansion");
            } else if name == builtins::JOIN {
                facts.operations.insert("task join");
                facts.effects.insert("task.join");
            }
            if facts.record_names.contains(name) {
                facts.constructors.insert(name.clone());
            } else {
                facts.calls.insert(name.clone());
            }
            for arg in args {
                visit(arg, facts);
            }
        }
        ExprKind::Spawn(call) => {
            facts.operations.insert("task spawn");
            facts.effects.insert("task.spawn");
            visit(call, facts);
        }
        ExprKind::If(condition, yes, no) => {
            facts.operations.insert("branching");
            visit(condition, facts);
            visit(yes, facts);
            visit(no, facts);
        }
        ExprKind::Match(value, arms) => {
            facts.operations.insert("exhaustive matching");
            visit(value, facts);
            for (_, body) in arms {
                visit(body, facts);
            }
        }
        ExprKind::Block(statements, tail) => {
            for statement in statements {
                match statement {
                    Stmt::Let { value, .. } => visit(value, facts),
                    Stmt::Assign { path, value, .. } => {
                        if !path.is_empty() {
                            facts.operations.insert("element or field assignment");
                        }
                        for step in path {
                            if let PlaceStep::Index(index, _) = step {
                                visit(index, facts);
                            }
                        }
                        visit(value, facts);
                    }
                    Stmt::Push { value, .. } => {
                        facts.operations.insert("mutable append");
                        visit(value, facts);
                    }
                    Stmt::For { iterable, body, .. } => {
                        facts.operations.insert("array iteration");
                        visit(iterable, facts);
                        visit(body, facts);
                    }
                    Stmt::While {
                        condition, body, ..
                    } => {
                        facts.operations.insert("conditional iteration");
                        visit(condition, facts);
                        visit(body, facts);
                    }
                    Stmt::Break { .. } => {
                        facts.operations.insert("loop break");
                    }
                    Stmt::Continue { .. } => {
                        facts.operations.insert("loop continue");
                    }
                    Stmt::Return { value, .. } => {
                        facts.operations.insert("early return");
                        visit(value, facts);
                    }
                    Stmt::Expr(value) => visit(value, facts),
                }
            }
            if let Some(tail) = tail {
                visit(tail, facts);
            }
        }
    }
}

fn list<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values: Vec<_> = values.into_iter().collect();
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(", ")
    }
}

/// Describe a program only after it has passed static checking.
pub fn explain(program: &Program) -> String {
    let mut out = String::from("Tokit program\n");
    out.push_str("\nRecords\n");
    if program.records.is_empty() {
        out.push_str("  none\n");
    }
    for record in &program.records {
        out.push_str(&format!(
            "  {}{}\n",
            record.name,
            type_params(&record.type_params)
        ));
        for (name, ty) in &record.fields {
            out.push_str(&format!("    {name}: {ty}\n"));
        }
    }
    out.push_str("\nEnums\n");
    if program.enums.is_empty() {
        out.push_str("  none\n");
    }
    for decl in &program.enums {
        let variants = decl
            .variants
            .iter()
            .map(|variant| match &variant.payload {
                Some(ty) => format!("{}({ty})", variant.name),
                None => variant.name.clone(),
            })
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "  {}: {}\n",
            decl.name,
            list(variants.iter().map(String::as_str))
        ));
    }
    out.push_str("\nFunctions\n");
    if program.functions.is_empty() {
        out.push_str("  none\n");
    }
    for function in &program.functions {
        let params = function
            .params
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  {}{}({params}) -> {}\n",
            function.name,
            type_params(&function.type_params),
            function.ret
        ));
        let mut facts = Facts {
            record_names: program
                .records
                .iter()
                .map(|record| record.name.clone())
                .collect(),
            ..Facts::default()
        };
        visit(&function.body, &mut facts);
        out.push_str(&format!(
            "    calls: {}\n",
            list(facts.calls.iter().map(String::as_str))
        ));
        out.push_str(&format!(
            "    constructs: {}\n",
            list(facts.constructors.iter().map(String::as_str))
        ));
        out.push_str(&format!(
            "    operations: {}\n",
            list(facts.operations.iter().copied())
        ));
    }
    out
}

fn type_params(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

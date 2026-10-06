//! Experimental native bootstrap: checked Tokit AST -> Rust source -> rustc.

use std::collections::HashMap;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ast::{Expr, ExprKind, Op, PatternKind, PlaceStep, Program, Span, Stmt, Type};
use crate::builtins;
use crate::diagnostic::Diagnostic;
use crate::ir::{self, BinaryOp, InstructionKind};
use crate::sources::SourceMap;

const PRELUDE_CORE: &str = include_str!("native_runtime/core.rs.txt");
const PRELUDE_I64: &str = include_str!("native_runtime/i64.rs.txt");
const PRELUDE_BYTES: &str = include_str!("native_runtime/bytes.rs.txt");
const PRELUDE_IO_ERROR: &str = include_str!("native_runtime/io_error.rs.txt");
const PRELUDE_PARSE: &str = include_str!("native_runtime/parse.rs.txt");
const PRELUDE_UTF8: &str = include_str!("native_runtime/utf8.rs.txt");
const PRELUDE_TASKS: &str = include_str!("native_runtime/tasks.rs.txt");
const PRELUDE_IO_HELPERS: &str = include_str!("native_runtime/io_helpers.rs.txt");
const PRELUDE_STRINGS: &str = include_str!("native_runtime/strings.rs.txt");
const PRELUDE_MAPS: &str = include_str!("native_runtime/maps.rs.txt");
const PRELUDE_ARRAYS: &str = include_str!("native_runtime/arrays.rs.txt");
const PRELUDE_MATH: &str = include_str!("native_runtime/math.rs.txt");
const PRELUDE_HTTP: &str = include_str!("native_runtime/http.rs.txt");

fn runtime_prelude(body: &str) -> String {
    let uses_io = body.contains("__tok_read_") || body.contains("__tok_write_");
    let uses_utf8 = body.contains("__tok_utf8_") || body.contains("__tok_bytes_");
    let uses_bytes = uses_io
        || uses_utf8
        || body.contains("__TokBytes")
        || body.contains("__tok_byte_")
        || body.contains("__tok_into_bytes");
    let uses_net = body.contains("__tok_net_");
    let mut prelude = String::from(PRELUDE_CORE);
    if body.contains("i64") {
        prelude.push_str(PRELUDE_I64);
    }
    if uses_bytes {
        prelude.push_str(PRELUDE_BYTES);
    }
    if uses_io || uses_net || body.contains("__TokIoError") || body.contains("__tok_stdin_all") {
        prelude.push_str(PRELUDE_IO_ERROR);
    }
    if body.contains("__TokParseError")
        || body.contains("__tok_parse_i32")
        || body.contains("__tok_parse_i64")
        || body.contains("__tok_parse_f64")
    {
        prelude.push_str(PRELUDE_PARSE);
    }
    if uses_utf8 {
        prelude.push_str(PRELUDE_UTF8);
    }
    if body.contains("__tok_str") {
        prelude.push_str(PRELUDE_STRINGS);
    }
    if body.contains("BTreeMap") || uses_net {
        prelude.push_str(PRELUDE_MAPS);
    }
    if uses_net {
        prelude.push_str(PRELUDE_HTTP);
        prelude.push_str(&http_adapters());
    }
    if body.contains("__tok_math_") {
        prelude.push_str(PRELUDE_MATH);
    }
    if body.contains("__tok_arr_") || body.contains("__tok_range") || body.contains("__tok_concat")
    {
        prelude.push_str(PRELUDE_ARRAYS);
    }
    if body.contains("__TokTask") || body.contains("__tok_spawn") || body.contains("__tok_join") {
        prelude.push_str(PRELUDE_TASKS);
    }
    if uses_io {
        prelude.push_str(PRELUDE_IO_HELPERS);
    }
    prelude
}

fn rust_type(ty: &Type) -> String {
    match ty {
        Type::I32 => "i32".to_owned(),
        Type::I64 => "i64".to_owned(),
        Type::F64 => "f64".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Bytes => "__TokBytes".to_owned(),
        Type::Named(name) if name == builtins::IO_ERROR => "__TokIoError".to_owned(),
        Type::Named(name) if name == builtins::PARSE_ERROR => "__TokParseError".to_owned(),
        Type::Named(name) if name == builtins::TASK_ERROR => "__TokTaskError".to_owned(),
        Type::Named(name) => user_name(name),
        Type::Applied(name, args) if name == builtins::MAP => format!(
            "std::collections::BTreeMap<{},{}>",
            rust_type(&args[0]),
            rust_type(&args[1])
        ),
        Type::Applied(name, args) => format!(
            "{}<{}>",
            user_name(name),
            args.iter().map(rust_type).collect::<Vec<_>>().join(",")
        ),
        Type::Param(name) => user_name(name),
        Type::Unit => "()".to_owned(),
        Type::Array(element) => format!("Vec<{}>", rust_type(element)),
        Type::Option(element) => format!("Option<{}>", rust_type(element)),
        Type::Task(result) => format!("__TokTask<{}>", rust_type(result)),
        Type::Fn(params, ret) => format!(
            "std::sync::Arc<dyn Fn({}) -> {} + Send + Sync>",
            params
                .iter()
                .map(rust_type_fallback)
                .collect::<Vec<_>>()
                .join(","),
            rust_type_fallback(ret)
        ),
        Type::Result(ok, err) => format!("Result<{},{}>", rust_type(ok), rust_type(err)),
        Type::Never => "!".to_owned(),
        Type::EmptyArray => "Vec<()>".to_owned(),
    }
}

fn user_name(name: &str) -> String {
    let mut output = String::from("u_");
    for byte in name.as_bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn enum_path(name: &str, variant: &str) -> String {
    if name == builtins::IO_ERROR {
        format!("__TokIoError::{variant}")
    } else if name == builtins::PARSE_ERROR {
        format!("__TokParseError::{variant}")
    } else if name == builtins::TASK_ERROR {
        format!("__TokTaskError::{variant}")
    } else {
        format!("{}::{}", user_name(name), user_name(variant))
    }
}

fn rust_type_fallback(ty: &Type) -> String {
    match ty {
        Type::Never => "()".to_owned(),
        Type::Array(element) => format!("Vec<{}>", rust_type_fallback(element)),
        Type::Option(element) => format!("Option<{}>", rust_type_fallback(element)),
        Type::Task(result) => format!("__TokTask<{}>", rust_type_fallback(result)),
        Type::Result(ok, err) => format!(
            "Result<{},{}>",
            rust_type_fallback(ok),
            rust_type_fallback(err)
        ),
        Type::Applied(name, args) if name == builtins::MAP => format!(
            "std::collections::BTreeMap<{},{}>",
            rust_type_fallback(&args[0]),
            rust_type_fallback(&args[1])
        ),
        Type::Applied(name, args) => format!(
            "{}<{}>",
            user_name(name),
            args.iter()
                .map(rust_type_fallback)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => rust_type(other),
    }
}

fn location(source: &SourceMap, span: Span) -> (usize, usize, usize) {
    let text = &source
        .get(span.source_id)
        .expect("checked span source must be registered")
        .text;
    let before = text.get(..span.start).unwrap_or(text);
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (span.source_id.0, line, column)
}

/// Checked expression types plus how each user function receives its parameters.
struct EmitContext<'a> {
    types: &'a HashMap<Span, Type>,
    borrowed_params: HashMap<String, Vec<bool>>,
    records: Vec<String>,
}

impl EmitContext<'_> {
    fn get(&self, span: &Span) -> Option<&Type> {
        self.types.get(span)
    }
}

/// Parameters are immutable, so non-scalar arguments can be lent to the callee
/// instead of copied; every read inside the callee still clones its own value.
/// Bare type parameters stay by value because they may be instantiated as scalars.
fn borrowed_param(ty: &Type) -> bool {
    !matches!(
        ty,
        Type::I32 | Type::I64 | Type::F64 | Type::Bool | Type::Unit | Type::Param(_)
    )
}

fn mentions_http_record(ty: &Type) -> bool {
    match ty {
        Type::Named(name) => name == builtins::REQUEST || name == builtins::RESPONSE,
        Type::Applied(_, args) => args.iter().any(mentions_http_record),
        Type::Array(inner) | Type::Option(inner) | Type::Task(inner) => mentions_http_record(inner),
        Type::Result(ok, err) => mentions_http_record(ok) || mentions_http_record(err),
        Type::Fn(params, ret) => {
            params.iter().any(mentions_http_record) || mentions_http_record(ret)
        }
        _ => false,
    }
}

/// Adapters between the shared HTTP code and the program's record types.
fn http_adapters() -> String {
    let request = user_name(builtins::REQUEST);
    let response = user_name(builtins::RESPONSE);
    let field = |name: &str| user_name(name);
    format!(
        r#"fn __tok_net_error(variant: &str) -> __TokIoError {{
    match variant {{ "Denied" => __TokIoError::Denied, "NotFound" => __TokIoError::NotFound, "InvalidUtf8" => __TokIoError::InvalidUtf8, _ => __TokIoError::Other }}
}}
fn __tok_net_serve(addr: String, limit: i32, handler: std::sync::Arc<dyn Fn({request}) -> {response} + Send + Sync>) -> Result<(), __TokIoError> {{
    if !__tok_http_allowed(__TOK_NET_GRANT.get().map(String::as_str), &addr) {{ return Err(__TokIoError::Denied); }}
    __tok_flush();
    __tok_http_serve(&addr, limit, &mut |request| {{
        let response = handler({request} {{ {method}: request.method, {path}: request.path, {query}: request.query, {headers}: request.headers.into_iter().collect(), {body}: request.body }});
        __tok_flush();
        Some(__TokHttpResponse {{ status: response.{status}, headers: response.{headers}.into_iter().collect(), body: response.{body} }})
    }}).map_err(__tok_net_error)
}}
fn __tok_net_request(method: String, url: String, headers: std::collections::BTreeMap<String, String>, body: String) -> Result<{response}, __TokIoError> {{
    let headers = headers.into_iter().collect::<Vec<_>>();
    let response = __tok_http_request(__TOK_NET_GRANT.get().map(String::as_str), &method, &url, &headers, &body).map_err(__tok_net_error)?;
    Ok({response} {{ {status}: response.status, {headers}: response.headers.into_iter().collect(), {body}: response.body }})
}}
"#,
        method = field("method"),
        path = field("path"),
        query = field("query"),
        headers = field("headers"),
        body = field("body"),
        status = field("status"),
    )
}

fn is_map(ty: Option<&Type>) -> bool {
    matches!(ty, Some(Type::Applied(name, _)) if name == builtins::MAP)
}

fn param_type(ty: &Type) -> String {
    if borrowed_param(ty) {
        format!("&{}", rust_type(ty))
    } else {
        rust_type(ty)
    }
}

fn emit_array_borrow(expr: &Expr, source: &SourceMap, types: &EmitContext<'_>) -> String {
    match &expr.kind {
        ExprKind::Var(name) => format!("&{}", user_name(name)),
        _ => format!("&({})", emit_expr(expr, source, types)),
    }
}

fn emit_expr(expr: &Expr, source: &SourceMap, types: &EmitContext<'_>) -> String {
    match &expr.kind {
        ExprKind::Apply(callee, args) => format!(
            "({})({})",
            emit_expr(callee, source, types),
            args.iter()
                .map(|arg| emit_expr(arg, source, types))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExprKind::Lambda(params, body) => {
            let Some(Type::Fn(param_types, ret)) = types.get(&expr.span) else {
                unreachable!("checked lambda has a function type")
            };
            // Copy captured locals so the closure owns them; names that refer to
            // functions, records, or builtins are not locals.
            let captures = body
                .free_names()
                .into_iter()
                .filter(|name| !params.iter().any(|(param, _)| param == name))
                .filter(|name| {
                    !types.borrowed_params.contains_key(name)
                        && !types.records.contains(name)
                        && !builtins::is_call(name)
                })
                .map(|name| format!("let {0} = {0}.clone(); ", user_name(&name)))
                .collect::<String>();
            let signature = params
                .iter()
                .zip(param_types)
                .map(|((name, _), ty)| format!("{}: {}", user_name(name), rust_type_fallback(ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "(std::sync::Arc::new({{ {captures}move |{signature}| -> {} {{ {} }} }}) as {})",
                rust_type_fallback(ret),
                emit_expr(body, source, types),
                rust_type(&Type::Fn(param_types.clone(), ret.clone()))
            )
        }
        ExprKind::Int(value) => format!("{value}i32"),
        ExprKind::I64(value) => format!("{value}i64"),
        ExprKind::F64(bits) => format!("f64::from_bits({bits}u64)"),
        ExprKind::Not(value) => format!("!({})", emit_expr(value, source, types)),
        ExprKind::Neg(value) => {
            if types.get(&value.span) == Some(&Type::F64) {
                return format!("-({})", emit_expr(value, source, types));
            }
            let (source_id, line, column) = location(source, expr.span);
            let helper = if types.get(&value.span) == Some(&Type::I64) {
                "__tok_neg_i64"
            } else {
                "__tok_neg"
            };
            format!(
                "{helper}({},{source_id},{line},{column})",
                emit_expr(value, source, types)
            )
        }
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::String(value) => format!("{:?}.to_owned()", value),
        ExprKind::Variant(name, variant, payload) => {
            let prefix = enum_path(name, variant);
            match payload {
                Some(value) => format!("{prefix}({})", emit_expr(value, source, types)),
                None => prefix,
            }
        }
        ExprKind::Array(values) => format!(
            "vec![{}]",
            values
                .iter()
                .map(|x| emit_expr(x, source, types))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExprKind::Index(array, index) => {
            let (source_id, line, column) = location(source, expr.span);
            let helper = if types.get(&array.span) == Some(&Type::Bytes) {
                "__tok_byte_index"
            } else {
                "__tok_index"
            };
            format!(
                "{helper}({},{},{source_id},{line},{column})",
                if index.is_simple_read() {
                    emit_array_borrow(array, source, types)
                } else {
                    format!("&({})", emit_expr(array, source, types))
                },
                emit_expr(index, source, types)
            )
        }
        ExprKind::Field(value, field) => {
            let base = match &value.kind {
                ExprKind::Var(name) => user_name(name),
                _ => emit_expr(value, source, types),
            };
            format!("({}).{}.clone()", base, user_name(field))
        }
        ExprKind::Ok(inner) => format!("Ok({})", emit_expr(inner, source, types)),
        ExprKind::Err(inner) => format!("Err({})", emit_expr(inner, source, types)),
        ExprKind::Some(inner) => format!("Some({})", emit_expr(inner, source, types)),
        ExprKind::None => "None".to_owned(),
        ExprKind::Try(inner) => format!("({}?)", emit_expr(inner, source, types)),
        ExprKind::Var(name) => format!("{}.clone()", user_name(name)),
        ExprKind::Call(name, args) => {
            let callee = match name.as_str() {
                builtins::READ_TEXT => "__tok_read_text".to_owned(),
                builtins::READ_BYTES => "__tok_read_bytes".to_owned(),
                builtins::WRITE_TEXT => "__tok_write_text".to_owned(),
                builtins::WRITE_BYTES => "__tok_write_bytes".to_owned(),
                builtins::LINES => "__tok_lines".to_owned(),
                builtins::ARGS => "__tok_args".to_owned(),
                builtins::PRINT => "__tok_print".to_owned(),
                builtins::SERVE => "__tok_net_serve".to_owned(),
                builtins::HTTP_REQUEST => "__tok_net_request".to_owned(),
                builtins::LIST_DIR => "__tok_read_list_dir".to_owned(),
                builtins::EXISTS => "__tok_read_exists".to_owned(),
                builtins::MAKE_DIR => "__tok_write_make_dir".to_owned(),
                builtins::REMOVE_FILE => "__tok_write_remove_file".to_owned(),
                builtins::ENV => "__tok_env".to_owned(),
                builtins::NOW_MS => "__tok_now_ms".to_owned(),
                builtins::CLOCK_NS => "__tok_clock_ns".to_owned(),
                builtins::SLEEP_MS => "__tok_sleep_ms".to_owned(),
                builtins::MAP => "std::collections::BTreeMap::new".to_owned(),
                builtins::GET => "__tok_map_get".to_owned(),
                builtins::GET_OR => "__tok_map_get_or".to_owned(),
                builtins::KEYS => "__tok_map_keys".to_owned(),
                builtins::VALUES => "__tok_map_values".to_owned(),
                builtins::REMOVE => "__tok_map_remove".to_owned(),
                builtins::CONTAINS if is_map(types.get(&args[0].span)) => {
                    "__tok_map_contains".to_owned()
                }
                builtins::CONTAINS if matches!(types.get(&args[0].span), Some(Type::Array(_))) => {
                    "__tok_arr_contains".to_owned()
                }
                builtins::RANGE => "__tok_range".to_owned(),
                builtins::PI => return "std::f64::consts::PI".to_owned(),
                builtins::ABS | builtins::MIN | builtins::MAX | builtins::POW => {
                    let suffix = match types.get(&args[0].span) {
                        Some(Type::I64) => "i64",
                        Some(Type::F64) => "f64",
                        _ => "i32",
                    };
                    let (source_id, line, column) = location(source, expr.span);
                    return format!(
                        "__tok_math_{name}_{suffix}({},{source_id},{line},{column})",
                        args.iter()
                            .map(|arg| emit_expr(arg, source, types))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
                builtins::SQRT => "f64::sqrt".to_owned(),
                builtins::FLOOR => "f64::floor".to_owned(),
                builtins::CEIL => "f64::ceil".to_owned(),
                builtins::ROUND => "f64::round".to_owned(),
                builtins::EXP => "f64::exp".to_owned(),
                builtins::LN => "f64::ln".to_owned(),
                builtins::SIN => "f64::sin".to_owned(),
                builtins::COS => "f64::cos".to_owned(),
                builtins::TAN => "f64::tan".to_owned(),
                builtins::ATAN2 => "f64::atan2".to_owned(),
                builtins::MAP_FN => "__tok_arr_map".to_owned(),
                builtins::FILTER => "__tok_arr_filter".to_owned(),
                builtins::ANY => "__tok_arr_any".to_owned(),
                builtins::ALL => "__tok_arr_all".to_owned(),
                builtins::FOLD => "__tok_arr_fold".to_owned(),
                builtins::SORT_BY => "__tok_arr_sort_by".to_owned(),
                builtins::SORT => "__tok_arr_sort".to_owned(),
                builtins::REVERSE => "__tok_arr_reverse".to_owned(),
                builtins::TO_STRING => "__tok_string".to_owned(),
                builtins::JOIN if args.len() == 2 => "__tok_str_join".to_owned(),
                builtins::CHARS => "__tok_str_chars".to_owned(),
                builtins::SPLIT => "__tok_str_split".to_owned(),
                builtins::TRIM => "__tok_str_trim".to_owned(),
                builtins::CONTAINS => "__tok_str_contains".to_owned(),
                builtins::STARTS_WITH => "__tok_str_starts_with".to_owned(),
                builtins::ENDS_WITH => "__tok_str_ends_with".to_owned(),
                builtins::REPLACE => "__tok_str_replace".to_owned(),
                builtins::LOWER => "__tok_str_lower".to_owned(),
                builtins::UPPER => "__tok_str_upper".to_owned(),
                builtins::READ_LINE => "__tok_stdin_line".to_owned(),
                builtins::READ_STDIN => "__tok_stdin_all".to_owned(),
                builtins::EXIT => "__tok_exit".to_owned(),
                builtins::PARSE_I32 => "__tok_parse_i32".to_owned(),
                builtins::PARSE_I64 => "__tok_parse_i64".to_owned(),
                builtins::PARSE_F64 => "__tok_parse_f64".to_owned(),
                builtins::WIDEN_I64 | builtins::NARROW_I32 | builtins::TO_F64 => {
                    let from = match types.get(&args[0].span) {
                        Some(Type::I32) => "i32",
                        Some(Type::I64) => "i64",
                        _ => "f64",
                    };
                    format!("__tok_{name}_from_{from}")
                }
                builtins::UTF8_BYTES => "__tok_utf8_bytes".to_owned(),
                builtins::UTF8_DECODE => "__tok_utf8_decode".to_owned(),
                builtins::UTF8_ENCODE => "__tok_utf8_encode".to_owned(),
                builtins::UTF8_DECODE_BYTES => "__tok_utf8_decode_bytes".to_owned(),
                builtins::BYTES_FROM_I32 => "__tok_bytes_from_i32".to_owned(),
                builtins::BYTES_TO_I32 => "__tok_bytes_to_i32".to_owned(),
                builtins::LEN => "__tok_len".to_owned(),
                builtins::JOIN => "__tok_join".to_owned(),
                // A path cannot be shadowed by a local function value.
                _ if types.borrowed_params.contains_key(name) || types.records.contains(name) => {
                    format!("crate::{}", user_name(name))
                }
                _ => user_name(name),
            };
            if name == builtins::SLICE {
                let (source_id, line, column) = location(source, expr.span);
                return format!(
                    "__tok_arr_slice({},{},{},{source_id},{line},{column})",
                    emit_array_borrow(&args[0], source, types),
                    emit_expr(&args[1], source, types),
                    emit_expr(&args[2], source, types)
                );
            }
            if name == builtins::LEN && is_map(types.get(&args[0].span)) {
                return format!(
                    "__tok_map_len({})",
                    emit_array_borrow(&args[0], source, types)
                );
            }
            if name == builtins::LEN {
                let (source_id, line, column) = location(source, expr.span);
                let borrowed = if types.get(&args[0].span) == Some(&Type::Bytes) {
                    match &args[0].kind {
                        ExprKind::Var(name) => format!("&{}.0", user_name(name)),
                        _ => format!("&({}).0", emit_expr(&args[0], source, types)),
                    }
                } else {
                    emit_array_borrow(&args[0], source, types)
                };
                return format!("__tok_len({},{source_id},{line},{column})", borrowed);
            }
            let borrowed = types.borrowed_params.get(name);
            format!(
                "{callee}({})",
                args.iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        let lent_map = index == 0
                            && (callee.starts_with("__tok_map_")
                                || callee.starts_with("__tok_arr_"));
                        if lent_map || borrowed.is_some_and(|modes| modes[index]) {
                            emit_array_borrow(arg, source, types)
                        } else {
                            emit_expr(arg, source, types)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        ExprKind::Spawn(call) => {
            let ExprKind::Call(name, args) = &call.kind else {
                unreachable!("checked spawn call")
            };
            let mut out = String::from("{");
            for (index, arg) in args.iter().enumerate() {
                write!(
                    out,
                    "let __tok_arg{index} = {};",
                    emit_expr(arg, source, types)
                )
                .expect("writing to String cannot fail");
            }
            let borrowed = types.borrowed_params.get(name);
            let args = (0..args.len())
                .map(|index| {
                    if borrowed.is_some_and(|modes| modes[index]) {
                        format!("&__tok_arg{index}")
                    } else {
                        format!("__tok_arg{index}")
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            write!(out, "__tok_spawn(move || {}({args})) }}", user_name(name))
                .expect("writing to String cannot fail");
            out
        }
        ExprKind::Binary(left, op, right) => {
            let left_type = types.get(&left.span);
            let right_type = types.get(&right.span);
            let left = emit_expr(left, source, types);
            let right = emit_expr(right, source, types);
            match op {
                Op::And => format!("({left} && {right})"),
                Op::Or => format!("({left} || {right})"),
                Op::Add if left_type == Some(&Type::String) => {
                    format!("({left} + &{right})")
                }
                Op::Add if matches!(left_type, Some(Type::Array(_) | Type::EmptyArray)) => {
                    format!("__tok_concat({left}, {right})")
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem
                    if left_type == Some(&Type::F64) =>
                {
                    let symbol = match op {
                        Op::Add => "+",
                        Op::Sub => "-",
                        Op::Mul => "*",
                        Op::Div => "/",
                        Op::Rem => "%",
                        _ => unreachable!(),
                    };
                    format!("({left} {symbol} {right})")
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => {
                    let operation = match op {
                        Op::Add => "add",
                        Op::Sub => "sub",
                        Op::Mul => "mul",
                        Op::Div => "div",
                        Op::Rem => "rem",
                        _ => unreachable!(),
                    };
                    let (source_id, line, column) = location(source, expr.span);
                    let suffix = if left_type == Some(&Type::I64) || right_type == Some(&Type::I64)
                    {
                        "_i64"
                    } else {
                        ""
                    };
                    format!("__tok_{operation}{suffix}({left},{right},{source_id},{line},{column})")
                }
                _ => {
                    let symbol = match op {
                        Op::Eq => "==",
                        Op::Ne => "!=",
                        Op::Lt => "<",
                        Op::Le => "<=",
                        Op::Gt => ">",
                        Op::Ge => ">=",
                        _ => unreachable!(),
                    };
                    format!("({left} {symbol} {right})")
                }
            }
        }
        ExprKind::If(condition, yes, no) => {
            // Rust requires a block after `else`; an `else if` branch is an
            // expression, so wrap it.
            let no = match no.kind {
                ExprKind::Block(..) => emit_expr(no, source, types),
                _ => format!("{{ {} }}", emit_expr(no, source, types)),
            };
            format!(
                "(if {} {} else {no})",
                emit_expr(condition, source, types),
                emit_expr(yes, source, types),
            )
        }
        ExprKind::Match(value, arms) => {
            let arms = arms
                .iter()
                .map(|(pattern, body)| {
                    let pattern = match &pattern.kind {
                        PatternKind::Int(value) => value.to_string(),
                        PatternKind::I64(value) => format!("{value}i64"),
                        PatternKind::Wildcard => "_".to_owned(),
                        PatternKind::Ok(name) => format!("Ok({})", user_name(name)),
                        PatternKind::Err(name) => format!("Err({})", user_name(name)),
                        PatternKind::Some(name) => format!("Some({})", user_name(name)),
                        PatternKind::None => "None".to_owned(),
                        PatternKind::Bool(value) => value.to_string(),
                        PatternKind::String(value) => format!("{value:?}"),
                        PatternKind::Variant(name, variant, binding) => {
                            let prefix = enum_path(name, variant);
                            match binding {
                                Some(name) => format!("{prefix}({})", user_name(name)),
                                None => prefix,
                            }
                        }
                    };
                    format!("{pattern} => {}", emit_expr(body, source, types))
                })
                .collect::<Vec<_>>()
                .join(",");
            let matched = emit_expr(value, source, types);
            let matched = match types.get(&value.span) {
                Some(ty @ (Type::Result(_, _) | Type::Option(_))) => {
                    format!(
                        "{{ let __tok_matched: {} = {matched}; __tok_matched }}",
                        rust_type_fallback(ty)
                    )
                }
                Some(Type::String) => {
                    format!("{{ let __tok_matched: String = {matched}; __tok_matched }}.as_str()")
                }
                _ => matched,
            };
            format!("(match {matched} {{ {arms} }})")
        }
        ExprKind::Block(stmts, tail) => {
            let mut out = String::from("{\n");
            for stmt in stmts {
                out.push_str(&emit_stmt(stmt, source, types));
            }
            if let Some(tail) = tail {
                out.push_str(&emit_expr(tail, source, types));
                out.push('\n');
            }
            out.push('}');
            out
        }
    }
}

fn emit_stmt(stmt: &Stmt, source: &SourceMap, types: &EmitContext<'_>) -> String {
    match stmt {
        Stmt::Let {
            name,
            ty,
            value,
            mutable,
            ..
        } => {
            let binding_ty = ty.as_ref().unwrap_or_else(|| {
                types
                    .get(&value.span)
                    .expect("checked binding has a value type")
            });
            format!(
                "let {}{}: {} = {};\n",
                if *mutable { "mut " } else { "" },
                user_name(name),
                rust_type(binding_ty),
                emit_expr(value, source, types)
            )
        }
        Stmt::Assign {
            name, path, value, ..
        } if path.is_empty() => {
            format!(
                "{} = {};\n",
                user_name(name),
                emit_expr(value, source, types)
            )
        }
        Stmt::Assign {
            name, path, value, ..
        } => {
            // Evaluate indices left to right, then the value, then store.
            let mut out = String::from("{ ");
            let mut place = user_name(name);
            let mut byte_store = None;
            for (position, step) in path.iter().enumerate() {
                match step {
                    PlaceStep::Index(index, span) => {
                        let map = is_map(types.get(span));
                        write!(
                            out,
                            "let __tok_i{position}{} = {}; ",
                            if map { "" } else { ": i32" },
                            emit_expr(index, source, types)
                        )
                        .expect("writing to String cannot fail");
                        let (source_id, line, column) = location(source, *span);
                        if map {
                            byte_store =
                                Some(format!("{place}.insert(__tok_i{position}, __tok_value);"));
                        } else if types.get(span) == Some(&Type::Bytes) {
                            byte_store = Some(format!(
                                "__tok_byte_store(&mut {place}, __tok_i{position}, __tok_value, {source_id}, {line}, {column});"
                            ));
                        } else {
                            place = format!(
                                "(*__tok_slot(&mut {place}, __tok_i{position}, {source_id}, {line}, {column}))"
                            );
                        }
                    }
                    PlaceStep::Field(field, _) => {
                        place = format!("{place}.{}", user_name(field));
                    }
                }
            }
            writeln!(
                out,
                "let __tok_value = {}; {} }}",
                emit_expr(value, source, types),
                byte_store.unwrap_or_else(|| format!("{place} = __tok_value;"))
            )
            .expect("writing to String cannot fail");
            out
        }
        Stmt::Push { name, value, span } => {
            if types.get(span) == Some(&Type::Bytes) {
                let (source_id, line, column) = location(source, *span);
                format!(
                    "__tok_byte_push(&mut {},{},{source_id},{line},{column});\n",
                    user_name(name),
                    emit_expr(value, source, types)
                )
            } else if types.get(span) == Some(&Type::String) {
                format!(
                    "{{ let __tok_push_value = {}; {}.push_str(&__tok_push_value); }}\n",
                    emit_expr(value, source, types),
                    user_name(name)
                )
            } else {
                format!(
                    "{{ let __tok_push_value = {}; {}.push(__tok_push_value); }}\n",
                    emit_expr(value, source, types),
                    user_name(name)
                )
            }
        }
        Stmt::For {
            name,
            iterable,
            body,
            ..
        } => {
            let body_type = types.get(&body.span).expect("checked body has a type");
            let body_expr = emit_expr(body, source, types);
            let discarded = if *body_type == Type::Never {
                format!("let _ = {body_expr};")
            } else {
                format!("let _: {} = {body_expr};", rust_type_fallback(body_type))
            };
            format!(
                "for {} in {} {{ {discarded} }}\n",
                user_name(name),
                if let ExprKind::Call(callee, bounds) = &iterable.kind
                    && callee == builtins::RANGE
                {
                    // Iterate the half-open range directly instead of building it.
                    format!(
                        "({})..({})",
                        emit_expr(&bounds[0], source, types),
                        emit_expr(&bounds[1], source, types)
                    )
                } else if types.get(&iterable.span) == Some(&Type::Bytes) {
                    format!(
                        "__tok_into_bytes({}).into_iter().map(i32::from)",
                        emit_expr(iterable, source, types)
                    )
                } else {
                    emit_expr(iterable, source, types)
                }
            )
        }
        Stmt::While {
            condition, body, ..
        } => {
            let body_type = types.get(&body.span).expect("checked body has a type");
            let body_expr = emit_expr(body, source, types);
            let discarded = if *body_type == Type::Never {
                format!("let _ = {body_expr};")
            } else {
                format!("let _: {} = {body_expr};", rust_type_fallback(body_type))
            };
            format!(
                "while {} {{ {discarded} }}\n",
                emit_expr(condition, source, types)
            )
        }
        Stmt::Return { value, .. } => format!("return {};\n", emit_expr(value, source, types)),
        Stmt::Break { .. } => "break;\n".to_owned(),
        Stmt::Continue { .. } => "continue;\n".to_owned(),
        Stmt::Expr(value) => {
            let ty = types
                .get(&value.span)
                .expect("checked expression has a type");
            let expression = emit_expr(value, source, types);
            if *ty == Type::Never {
                format!("let _ = {expression};\n")
            } else {
                format!("let _: {} = {expression};\n", rust_type_fallback(ty))
            }
        }
    }
}

fn emit_ir_instructions(
    out: &mut String,
    instructions: &[ir::Instruction],
    function: &ir::Function,
    source: &SourceMap,
) {
    for instruction in instructions {
        let value = match instruction.kind {
            InstructionKind::Parameter(index) => user_name(&function.params[index].0),
            InstructionKind::I32(value) => format!("{value}i32"),
            InstructionKind::I64(value) => format!("{value}i64"),
            InstructionKind::F64(bits) => format!("f64::from_bits({bits}u64)"),
            InstructionKind::Bool(value) => value.to_string(),
            InstructionKind::Not(value) => format!("!__tok_v{}", value.0),
            InstructionKind::CheckedNeg(value) => {
                let helper = if instruction.ty == Type::I64 {
                    "__tok_neg_i64"
                } else {
                    "__tok_neg"
                };
                let (source_id, line, column) = location(source, instruction.span);
                format!("{helper}(__tok_v{},{source_id},{line},{column})", value.0)
            }
            InstructionKind::FloatNeg(value) => format!("-__tok_v{}", value.0),
            InstructionKind::Convert(value) => {
                format!("(__tok_v{} as {})", value.0, rust_type(&instruction.ty))
            }
            InstructionKind::Binary(op, left, right) => {
                let left_name = format!("__tok_v{}", left.0);
                let right_name = format!("__tok_v{}", right.0);
                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem
                        if instruction.ty == Type::F64 =>
                    {
                        let symbol = match op {
                            BinaryOp::Add => "+",
                            BinaryOp::Sub => "-",
                            BinaryOp::Mul => "*",
                            BinaryOp::Div => "/",
                            BinaryOp::Rem => "%",
                            _ => unreachable!(),
                        };
                        format!("({left_name} {symbol} {right_name})")
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Rem => {
                        let operation = match op {
                            BinaryOp::Add => "add",
                            BinaryOp::Sub => "sub",
                            BinaryOp::Mul => "mul",
                            BinaryOp::Div => "div",
                            BinaryOp::Rem => "rem",
                            _ => unreachable!(),
                        };
                        let suffix = if instruction.ty == Type::I64 {
                            "_i64"
                        } else {
                            ""
                        };
                        let (source_id, line, column) = location(source, instruction.span);
                        format!(
                            "__tok_{operation}{suffix}({left_name},{right_name},{source_id},{line},{column})"
                        )
                    }
                    _ => {
                        let symbol = match op {
                            BinaryOp::Eq => "==",
                            BinaryOp::Ne => "!=",
                            BinaryOp::Lt => "<",
                            BinaryOp::Le => "<=",
                            BinaryOp::Gt => ">",
                            BinaryOp::Ge => ">=",
                            _ => unreachable!(),
                        };
                        format!("({left_name} {symbol} {right_name})")
                    }
                }
            }
            InstructionKind::Call(ref name, ref args) => format!(
                "{}({})",
                user_name(name),
                args.iter()
                    .map(|value| format!("__tok_v{}", value.0))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            InstructionKind::Conditional {
                condition,
                ref yes,
                ref no,
            } => {
                let mut yes_code = String::new();
                emit_ir_instructions(&mut yes_code, &yes.instructions, function, source);
                let mut no_code = String::new();
                emit_ir_instructions(&mut no_code, &no.instructions, function, source);
                format!(
                    "if __tok_v{} {{ {yes_code} __tok_v{} }} else {{ {no_code} __tok_v{} }}",
                    condition.0, yes.result.0, no.result.0
                )
            }
        };
        writeln!(
            out,
            "let __tok_v{}: {} = {value};",
            instruction.id.0,
            rust_type(&instruction.ty)
        )
        .expect("writing to String cannot fail");
    }
}

fn emit_ir_function(function: &ir::Function, source: &SourceMap) -> String {
    let params = function
        .params
        .iter()
        .map(|(name, ty)| format!("{}: {}", user_name(name), rust_type(ty)))
        .collect::<Vec<_>>()
        .join(",");
    let (source_id, line, column) = location(source, function.span);
    let mut out = format!(
        "fn {}({params}) -> {} {{ let __tok_depth = __TokDepthGuard::enter({source_id},{line},{column});\n",
        user_name(&function.name),
        rust_type(&function.ret)
    );
    emit_ir_instructions(&mut out, &function.instructions, function, source);
    writeln!(out, "__tok_v{} }}", function.result.0).expect("writing to String cannot fail");
    out
}

pub fn emit(program: &Program, source: &str) -> Result<String, Diagnostic> {
    emit_with_sources(program, &SourceMap::single(source))
}

pub fn emit_with_sources(program: &Program, source: &SourceMap) -> Result<String, Diagnostic> {
    let types = crate::checker::check_with_types(program)?;
    let main = program
        .functions
        .iter()
        .find(|f| f.name == "main")
        .ok_or_else(|| Diagnostic::new("E203", Span::new(0, 0), "missing main function"))?;
    if !main.params.is_empty() || !main.type_params.is_empty() {
        return Err(Diagnostic::new(
            "E203",
            main.span,
            "main must have no parameters or type parameters",
        ));
    }
    let mut out = String::new();
    let paths = (0..source.len())
        .map(|id| {
            format!(
                "{:?}",
                source
                    .display_path(crate::ast::SourceId(id))
                    .expect("registered source path")
                    .to_string_lossy()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    writeln!(out, "const __TOK_SOURCES: &[&str] = &[{paths}];")
        .expect("writing to String cannot fail");
    for enum_decl in &program.enums {
        let name = user_name(&enum_decl.name);
        let generic_names = enum_decl
            .type_params
            .iter()
            .map(|param| user_name(param))
            .collect::<Vec<_>>();
        let generics = if generic_names.is_empty() {
            String::new()
        } else {
            format!("<{}>", generic_names.join(","))
        };
        let render_generics = if generic_names.is_empty() {
            String::new()
        } else {
            format!(
                "<{}>",
                generic_names
                    .iter()
                    .map(|param| format!("{param}: __TokRender"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let variants = enum_decl
            .variants
            .iter()
            .map(|variant| match &variant.payload {
                Some(ty) => format!("{}({})", user_name(&variant.name), rust_type(ty)),
                None => user_name(&variant.name),
            })
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            out,
            "#[derive(Clone)] enum {name}{generics} {{ {variants} }}"
        )
        .expect("writing to String cannot fail");
        let arms = enum_decl
            .variants
            .iter()
            .map(|variant| {
                let name = user_name(&variant.name);
                let display = format!("{}::{}", enum_decl.name, variant.name);
                if variant.payload.is_some() {
                    format!(
                        "Self::{name}(value) => format!(\"{display}({{}})\", value.tok_render())"
                    )
                } else {
                    format!("Self::{name} => {display:?}.to_owned()")
                }
            })
            .collect::<Vec<_>>()
            .join(",");
        let arms = if arms.is_empty() {
            "_ => unreachable!()".to_owned()
        } else {
            arms
        };
        writeln!(out, "impl{render_generics} __TokRender for {name}{generics} {{ fn tok_render(&self) -> String {{ match self {{ {arms} }} }} }}")
            .expect("writing to String cannot fail");
    }
    // Built-in HTTP records are emitted only when the program uses them.
    let builtin_records = if types.values().any(mentions_http_record) {
        builtins::http_records()
    } else {
        Vec::new()
    };
    for record in program.records.iter().chain(&builtin_records) {
        let name = user_name(&record.name);
        let generic_names = record
            .type_params
            .iter()
            .map(|param| user_name(param))
            .collect::<Vec<_>>()
            .join(",");
        let generics = if generic_names.is_empty() {
            String::new()
        } else {
            format!("<{generic_names}>")
        };
        let render_bounds = record
            .type_params
            .iter()
            .map(|param| format!("{}: __TokRender", user_name(param)))
            .collect::<Vec<_>>()
            .join(",");
        let render_generics = if render_bounds.is_empty() {
            String::new()
        } else {
            format!("<{render_bounds}>")
        };
        let fields = record
            .fields
            .iter()
            .map(|(field, ty)| format!("{}: {}", user_name(field), rust_type(ty)))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            out,
            "#[derive(Clone)] struct {name}{generics} {{ {fields} }}"
        )
        .expect("writing to String cannot fail");
        let params = record
            .fields
            .iter()
            .map(|(field, ty)| format!("{}: {}", user_name(field), rust_type(ty)))
            .collect::<Vec<_>>()
            .join(",");
        let init = record
            .fields
            .iter()
            .map(|(field, _)| user_name(field))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            out,
            "fn {name}{generics}({params}) -> {name}{generics} {{ {name} {{ {init} }} }}"
        )
        .expect("writing to String cannot fail");
        let rendered = record
            .fields
            .iter()
            .map(|(field, _)| {
                format!(
                    "format!(\"{}:{{}}\", self.{}.tok_render())",
                    field,
                    user_name(field)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let render_expression = if record.fields.is_empty() {
            format!("{:?}.to_owned()", format!("{}()", record.name))
        } else {
            format!(
                "format!(\"{}({{}})\", vec![{rendered}].join(\",\"))",
                record.name
            )
        };
        writeln!(out, "impl{render_generics} __TokRender for {name}{generics} {{ fn tok_render(&self) -> String {{ {render_expression} }} }}")
            .expect("writing to String cannot fail");
    }
    let context = EmitContext {
        types: &types,
        borrowed_params: program
            .functions
            .iter()
            .map(|function| {
                let modes = function
                    .params
                    .iter()
                    .map(|(_, ty)| borrowed_param(ty))
                    .collect();
                (function.name.clone(), modes)
            })
            .collect(),
        records: program
            .records
            .iter()
            .map(|record| record.name.clone())
            .collect(),
    };
    let lowering = ir::LoweringContext::new(program);
    for function in &program.functions {
        if let Some(lowered) = lowering.lower_function(function, &types) {
            out.push_str(&emit_ir_function(&lowered, source));
            continue;
        }
        let bounds = function
            .type_params
            .iter()
            .map(|param| format!("{}: Clone + __TokRender", user_name(param)))
            .collect::<Vec<_>>()
            .join(",");
        let generics = if bounds.is_empty() {
            String::new()
        } else {
            format!("<{bounds}>")
        };
        let params = function
            .params
            .iter()
            .map(|(name, ty)| format!("{}: {}", user_name(name), param_type(ty)))
            .collect::<Vec<_>>()
            .join(",");
        let (source_id, line, column) = location(source, function.span);
        writeln!(
            out,
            "fn {}{generics}({params}) -> {} {{ let __tok_depth = __TokDepthGuard::enter({source_id},{line},{column}); {} }}",
            user_name(&function.name),
            rust_type(&function.ret),
            emit_expr(&function.body, source, &context)
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        out,
        "fn main() {{ let program = std::thread::Builder::new().stack_size(__TOK_STACK_BYTES).spawn(|| {{ __tok_configure_runtime(); {} }}).expect(\"cannot start program thread\"); let failed = program.join().is_err(); __tok_flush(); if failed {{ std::process::exit(101); }} }}",
        if main.ret == Type::Unit {
            format!("{}();", user_name("main"))
        } else {
            format!("__tok_print({}().tok_render());", user_name("main"))
        }
    )
    .expect("writing to String cannot fail");
    let mut generated = runtime_prelude(&out);
    generated.push_str(&out);
    Ok(generated)
}

pub fn build(program: &Program, source: &str, output: &Path) -> Result<(), String> {
    let generated = emit(program, source).map_err(|diagnostic| diagnostic.display(source))?;
    compile_generated(&generated, output)
}

pub fn build_with_sources(
    program: &Program,
    sources: &SourceMap,
    output: &Path,
) -> Result<(), String> {
    let generated = emit_with_sources(program, sources)
        .map_err(|diagnostic| diagnostic.display_with_sources(sources))?;
    compile_generated(&generated, output)
}

fn compile_generated(generated: &str, output: &Path) -> Result<(), String> {
    let generated_path = temporary_source(generated)?;
    let rustc = std::env::var_os("TOKIT_RUSTC").unwrap_or_else(|| "rustc".into());
    let result = Command::new(rustc)
        .arg("--crate-name")
        .arg("tok_native")
        .arg("--edition=2024")
        .arg("-C")
        .arg("opt-level=2")
        .arg(&generated_path.0)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("E302: could not start rustc: {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "E302: generated Rust failed to compile:\n{}",
            String::from_utf8_lossy(&result.stderr)
        ))
    }
}

struct TemporarySource(PathBuf);

impl Drop for TemporarySource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temporary_source(source: &str) -> Result<TemporarySource, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("E301: clock error: {e}"))?
        .as_nanos();
    for attempt in 0..8 {
        let path =
            std::env::temp_dir().join(format!("tokit-{}-{nonce}-{attempt}.rs", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        match file {
            Ok(mut file) => {
                let temporary = TemporarySource(path);
                file.write_all(source.as_bytes())
                    .map_err(|e| format!("E301: could not write generated Rust: {e}"))?;
                return Ok(temporary);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("E301: could not create generated Rust: {error}")),
        }
    }
    Err("E301: could not create a unique generated source file".to_owned())
}

//! Experimental native bootstrap: checked Tokit AST -> Rust source -> rustc.

use std::collections::HashMap;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ast::{Expr, ExprKind, Op, PatternKind, Program, Span, Stmt, Type};
use crate::builtins;
use crate::diagnostic::Diagnostic;
use crate::sources::SourceMap;

const PRELUDE_CORE: &str = include_str!("native_runtime/core.rs.txt");
const PRELUDE_BYTES: &str = include_str!("native_runtime/bytes.rs.txt");
const PRELUDE_IO_ERROR: &str = include_str!("native_runtime/io_error.rs.txt");
const PRELUDE_PARSE: &str = include_str!("native_runtime/parse.rs.txt");
const PRELUDE_UTF8: &str = include_str!("native_runtime/utf8.rs.txt");
const PRELUDE_TASKS: &str = include_str!("native_runtime/tasks.rs.txt");
const PRELUDE_IO_HELPERS: &str = include_str!("native_runtime/io_helpers.rs.txt");

fn runtime_prelude(body: &str) -> String {
    let uses_io = body.contains("__tok_read_") || body.contains("__tok_write_");
    let uses_utf8 = body.contains("__tok_utf8_") || body.contains("__tok_bytes_");
    let uses_bytes = uses_io
        || uses_utf8
        || body.contains("__TokBytes")
        || body.contains("__tok_byte_")
        || body.contains("__tok_into_bytes");
    let mut prelude = String::from(PRELUDE_CORE);
    if uses_bytes {
        prelude.push_str(PRELUDE_BYTES);
    }
    if uses_io || body.contains("__TokIoError") {
        prelude.push_str(PRELUDE_IO_ERROR);
    }
    if body.contains("__TokParseError") || body.contains("__tok_parse_i32") {
        prelude.push_str(PRELUDE_PARSE);
    }
    if uses_utf8 {
        prelude.push_str(PRELUDE_UTF8);
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
        Type::Bool => "bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Bytes => "__TokBytes".to_owned(),
        Type::Named(name) if name == builtins::IO_ERROR => "__TokIoError".to_owned(),
        Type::Named(name) if name == builtins::PARSE_ERROR => "__TokParseError".to_owned(),
        Type::Named(name) if name == builtins::TASK_ERROR => "__TokTaskError".to_owned(),
        Type::Named(name) => user_name(name),
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

fn emit_array_borrow(expr: &Expr, source: &SourceMap, types: &HashMap<Span, Type>) -> String {
    match &expr.kind {
        ExprKind::Var(name) => format!("&{}", user_name(name)),
        _ => format!("&({})", emit_expr(expr, source, types)),
    }
}

fn emit_expr(expr: &Expr, source: &SourceMap, types: &HashMap<Span, Type>) -> String {
    match &expr.kind {
        ExprKind::Int(value) => format!("{value}i32"),
        ExprKind::Neg(value) => {
            let (source_id, line, column) = location(source, expr.span);
            format!(
                "__tok_neg({},{source_id},{line},{column})",
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
                emit_array_borrow(array, source, types),
                emit_expr(index, source, types)
            )
        }
        ExprKind::Field(value, field) => {
            format!(
                "({}).{}.clone()",
                emit_expr(value, source, types),
                user_name(field)
            )
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
                builtins::PARSE_I32 => "__tok_parse_i32".to_owned(),
                builtins::UTF8_BYTES => "__tok_utf8_bytes".to_owned(),
                builtins::UTF8_DECODE => "__tok_utf8_decode".to_owned(),
                builtins::UTF8_ENCODE => "__tok_utf8_encode".to_owned(),
                builtins::UTF8_DECODE_BYTES => "__tok_utf8_decode_bytes".to_owned(),
                builtins::BYTES_FROM_I32 => "__tok_bytes_from_i32".to_owned(),
                builtins::BYTES_TO_I32 => "__tok_bytes_to_i32".to_owned(),
                builtins::LEN => "__tok_len".to_owned(),
                builtins::JOIN => "__tok_join".to_owned(),
                _ => user_name(name),
            };
            if name == builtins::LEN {
                let (source_id, line, column) = location(source, expr.span);
                let borrowed = if types.get(&args[0].span) == Some(&Type::Bytes) {
                    format!("&({}).0", emit_expr(&args[0], source, types))
                } else {
                    emit_array_borrow(&args[0], source, types)
                };
                return format!("__tok_len({},{source_id},{line},{column})", borrowed);
            }
            format!(
                "{callee}({})",
                args.iter()
                    .map(|x| emit_expr(x, source, types))
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
            let args = (0..args.len())
                .map(|index| format!("__tok_arg{index}"))
                .collect::<Vec<_>>()
                .join(",");
            write!(out, "__tok_spawn(move || {}({args})) }}", user_name(name))
                .expect("writing to String cannot fail");
            out
        }
        ExprKind::Binary(left, op, right) => {
            let left_type = types.get(&left.span);
            let left = emit_expr(left, source, types);
            let right = emit_expr(right, source, types);
            match op {
                Op::Add if left_type == Some(&Type::String) => {
                    format!("({left} + &{right})")
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div => {
                    let helper = match op {
                        Op::Add => "add",
                        Op::Sub => "sub",
                        Op::Mul => "mul",
                        Op::Div => "div",
                        _ => unreachable!(),
                    };
                    let (source_id, line, column) = location(source, expr.span);
                    format!("__tok_{helper}({left},{right},{source_id},{line},{column})")
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
        ExprKind::If(condition, yes, no) => format!(
            "(if {} {} else {})",
            emit_expr(condition, source, types),
            emit_expr(yes, source, types),
            emit_expr(no, source, types)
        ),
        ExprKind::Match(value, arms) => {
            let arms = arms
                .iter()
                .map(|(pattern, body)| {
                    let pattern = match &pattern.kind {
                        PatternKind::Int(value) => value.to_string(),
                        PatternKind::Wildcard => "_".to_owned(),
                        PatternKind::Ok(name) => format!("Ok({})", user_name(name)),
                        PatternKind::Err(name) => format!("Err({})", user_name(name)),
                        PatternKind::Some(name) => format!("Some({})", user_name(name)),
                        PatternKind::None => "None".to_owned(),
                        PatternKind::Bool(value) => value.to_string(),
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

fn emit_stmt(stmt: &Stmt, source: &SourceMap, types: &HashMap<Span, Type>) -> String {
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
        Stmt::Assign { name, value, .. } => {
            format!(
                "{} = {};\n",
                user_name(name),
                emit_expr(value, source, types)
            )
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
                if types.get(&iterable.span) == Some(&Type::Bytes) {
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
        let variants = enum_decl
            .variants
            .iter()
            .map(|variant| match &variant.payload {
                Some(ty) => format!("{}({})", user_name(&variant.name), rust_type(ty)),
                None => user_name(&variant.name),
            })
            .collect::<Vec<_>>()
            .join(",");
        writeln!(out, "#[derive(Clone)] enum {name} {{ {variants} }}")
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
        writeln!(out, "impl __TokRender for {name} {{ fn tok_render(&self) -> String {{ match self {{ {arms} }} }} }}")
            .expect("writing to String cannot fail");
    }
    for record in &program.records {
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
    for function in &program.functions {
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
            .map(|(name, ty)| format!("{}: {}", user_name(name), rust_type(ty)))
            .collect::<Vec<_>>()
            .join(",");
        let (source_id, line, column) = location(source, function.span);
        writeln!(
            out,
            "fn {}{generics}({params}) -> {} {{ let __tok_depth = __TokDepthGuard::enter({source_id},{line},{column}); {} }}",
            user_name(&function.name),
            rust_type(&function.ret),
            emit_expr(&function.body, source, &types)
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        out,
        "fn main() {{ __tok_configure_runtime(); println!(\"{{}}\", {}().tok_render()); }}",
        user_name("main")
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

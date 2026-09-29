//! Experimental native bootstrap: checked Tokit AST -> Rust source -> rustc.

use std::collections::HashMap;
use std::fmt::Write;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ast::{Expr, ExprKind, Op, PatternKind, Program, Span, Stmt, Type};
use crate::diagnostic::Diagnostic;

const PRELUDE: &str = r#"trait __TokRender { fn tok_render(&self) -> String; }
impl __TokRender for i32 { fn tok_render(&self) -> String { self.to_string() } }
impl __TokRender for bool { fn tok_render(&self) -> String { self.to_string() } }
impl __TokRender for String { fn tok_render(&self) -> String { format!("{:?}", self) } }
impl __TokRender for () { fn tok_render(&self) -> String { "()".to_owned() } }
impl<T: __TokRender> __TokRender for Vec<T> {
    fn tok_render(&self) -> String {
        format!("[{}]", self.iter().map(|x| x.tok_render()).collect::<Vec<_>>().join(","))
    }
}
impl<T: __TokRender, E: __TokRender> __TokRender for Result<T, E> {
    fn tok_render(&self) -> String {
        match self { Ok(x) => format!("Ok({})", x.tok_render()), Err(x) => format!("Err({})", x.tok_render()) }
    }
}
fn __tok_fail(line: usize, column: usize) -> ! {
    eprintln!("E201@{}:{} integer overflow or division by zero", line, column);
    std::process::exit(1)
}
fn __tok_add(a: i32, b: i32, line: usize, column: usize) -> i32 {
    a.checked_add(b).unwrap_or_else(|| __tok_fail(line, column))
}
fn __tok_sub(a: i32, b: i32, line: usize, column: usize) -> i32 {
    a.checked_sub(b).unwrap_or_else(|| __tok_fail(line, column))
}
fn __tok_mul(a: i32, b: i32, line: usize, column: usize) -> i32 {
    a.checked_mul(b).unwrap_or_else(|| __tok_fail(line, column))
}
fn __tok_div(a: i32, b: i32, line: usize, column: usize) -> i32 {
    a.checked_div(b).unwrap_or_else(|| __tok_fail(line, column))
}
fn __tok_index<T: Clone>(values: Vec<T>, index: i32, line: usize, column: usize) -> T {
    usize::try_from(index).ok().and_then(|i| values.get(i)).cloned().unwrap_or_else(|| {
        eprintln!("E205@{}:{} array index out of bounds", line, column);
        std::process::exit(1)
    })
}
thread_local! { static __TOK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
struct __TokDepthGuard;
impl __TokDepthGuard {
    fn enter(line: usize, column: usize) -> Self {
        __TOK_DEPTH.with(|depth| {
            if depth.get() >= 32 {
                eprintln!("E202@{}:{} call depth limit exceeded", line, column);
                std::process::exit(1);
            }
            depth.set(depth.get() + 1);
        });
        Self
    }
}
impl Drop for __TokDepthGuard {
    fn drop(&mut self) { __TOK_DEPTH.with(|depth| depth.set(depth.get() - 1)); }
}
"#;

fn rust_type(ty: &Type) -> String {
    match ty {
        Type::I32 => "i32".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Named(name) => user_name(name),
        Type::Applied(name, args) => format!(
            "{}<{}>",
            user_name(name),
            args.iter().map(rust_type).collect::<Vec<_>>().join(",")
        ),
        Type::Param(name) => user_name(name),
        Type::Unit => "()".to_owned(),
        Type::Array(element) => format!("Vec<{}>", rust_type(element)),
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

fn rust_type_fallback(ty: &Type) -> String {
    match ty {
        Type::Never => "()".to_owned(),
        Type::Array(element) => format!("Vec<{}>", rust_type_fallback(element)),
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

fn location(source: &str, span: Span) -> (usize, usize) {
    let before = source.get(..span.start).unwrap_or(source);
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, column)
}

fn emit_expr(expr: &Expr, source: &str, types: &HashMap<Span, Type>) -> String {
    match &expr.kind {
        ExprKind::Int(value) => format!("{value}i32"),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::String(value) => format!("{:?}.to_owned()", value),
        ExprKind::Variant(name, variant) => format!("{}::{}", user_name(name), user_name(variant)),
        ExprKind::Array(values) => format!(
            "vec![{}]",
            values
                .iter()
                .map(|x| emit_expr(x, source, types))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExprKind::Index(array, index) => {
            let (line, column) = location(source, expr.span);
            format!(
                "__tok_index({},{},{line},{column})",
                emit_expr(array, source, types),
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
        ExprKind::Try(inner) => format!("({}?)", emit_expr(inner, source, types)),
        ExprKind::Var(name) => format!("{}.clone()", user_name(name)),
        ExprKind::Call(name, args) => format!(
            "{}({})",
            user_name(name),
            args.iter()
                .map(|x| emit_expr(x, source, types))
                .collect::<Vec<_>>()
                .join(",")
        ),
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
                    let (line, column) = location(source, expr.span);
                    format!("__tok_{helper}({left},{right},{line},{column})")
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
                        PatternKind::Ok(name) => format!("Ok({})", user_name(name)),
                        PatternKind::Err(name) => format!("Err({})", user_name(name)),
                        PatternKind::Bool(value) => value.to_string(),
                        PatternKind::Variant(name, variant) => {
                            format!("{}::{}", user_name(name), user_name(variant))
                        }
                    };
                    format!("{pattern} => {}", emit_expr(body, source, types))
                })
                .collect::<Vec<_>>()
                .join(",");
            let matched = emit_expr(value, source, types);
            let matched = match types.get(&value.span) {
                Some(ty @ Type::Result(_, _)) => {
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

fn emit_stmt(stmt: &Stmt, source: &str, types: &HashMap<Span, Type>) -> String {
    match stmt {
        Stmt::Let {
            name,
            ty,
            value,
            mutable,
            ..
        } => format!(
            "let {}{}: {} = {};\n",
            if *mutable { "mut " } else { "" },
            user_name(name),
            rust_type(ty),
            emit_expr(value, source, types)
        ),
        Stmt::Assign { name, value, .. } => {
            format!(
                "{} = {};\n",
                user_name(name),
                emit_expr(value, source, types)
            )
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
                emit_expr(iterable, source, types)
            )
        }
        Stmt::Return { value, .. } => format!("return {};\n", emit_expr(value, source, types)),
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
    let types = crate::checker::check_with_types(program)?;
    let main = program
        .functions
        .iter()
        .find(|f| f.name == "main")
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
    let mut out = String::from(PRELUDE);
    for enum_decl in &program.enums {
        let name = user_name(&enum_decl.name);
        let variants = enum_decl
            .variants
            .iter()
            .map(|variant| user_name(variant))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(out, "#[derive(Clone)] enum {name} {{ {variants} }}")
            .expect("writing to String cannot fail");
        let arms = enum_decl
            .variants
            .iter()
            .map(|variant| {
                format!(
                    "Self::{} => {:?}.to_owned()",
                    user_name(variant),
                    format!("{}::{variant}", enum_decl.name)
                )
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
        let (line, column) = location(source, function.span);
        writeln!(
            out,
            "fn {}{generics}({params}) -> {} {{ let __tok_depth = __TokDepthGuard::enter({line},{column}); {} }}",
            user_name(&function.name),
            rust_type(&function.ret),
            emit_expr(&function.body, source, &types)
        )
        .expect("writing to String cannot fail");
    }
    writeln!(
        out,
        "fn main() {{ println!(\"{{}}\", {}().tok_render()); }}",
        user_name("main")
    )
    .expect("writing to String cannot fail");
    Ok(out)
}

pub fn build(program: &Program, source: &str, output: &Path) -> Result<(), String> {
    let generated = emit(program, source).map_err(|diagnostic| diagnostic.display(source))?;
    let generated_path = temporary_source(&generated)?;
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

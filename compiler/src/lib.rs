pub mod ai_index;
pub mod ast;
pub mod builtins;
pub mod checker;
pub mod diagnostic;
pub mod explain;
pub mod filesystem;
pub mod format;
pub mod interpreter;
pub mod lexer;
pub mod native;
pub mod parser;
pub mod stats;
pub mod test_runner;

use ast::Program;
use diagnostic::Diagnostic;
use interpreter::Value;
use std::path::Path;

pub fn parse(source: &str) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex(source)?;
    parser::Parser::new(tokens).program()
}

pub fn check(source: &str) -> Result<Program, Diagnostic> {
    let program = parse(source)?;
    checker::check(&program)?;
    Ok(program)
}

pub fn run(source: &str) -> Result<Value, Diagnostic> {
    let program = check(source)?;
    interpreter::run(&program)
}

pub fn run_with_read_root(source: &str, root: &Path) -> Result<Value, Diagnostic> {
    let program = check(source)?;
    interpreter::run_with_read_root(&program, Some(root))
}

pub fn run_with_runtime_args(
    source: &str,
    root: Option<&Path>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    let program = check(source)?;
    interpreter::run_with_runtime_args(&program, root, args)
}

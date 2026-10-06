pub mod ai_index;
pub mod ai_patch;
pub mod ast;
pub mod builtins;
pub mod checker;
mod crypto;
pub mod diagnostic;
pub mod explain;
pub mod filesystem;
pub mod format;
mod http;
pub mod interpreter;
pub mod ir;
pub mod lexer;
pub mod lsp;
pub mod module_resolver;
pub mod modules;
pub mod native;
pub mod packages;
pub mod parser;
pub mod sources;
pub mod stats;
pub mod test_runner;

use ast::{Program, SourceId};
use diagnostic::Diagnostic;
use interpreter::Value;
use std::path::Path;

pub fn parse(source: &str) -> Result<Program, Diagnostic> {
    parse_in_source(source, SourceId::default())
}

pub fn parse_in_source(source: &str, source_id: SourceId) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex_in_source(source, source_id)?;
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

pub fn run_with_capabilities(
    source: &str,
    read_root: Option<&Path>,
    write_root: Option<&Path>,
    args: &[String],
) -> Result<Value, Diagnostic> {
    let program = check(source)?;
    interpreter::run_with_capabilities(&program, read_root, write_root, args)
}

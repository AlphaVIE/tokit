pub mod ast;
pub mod checker;
pub mod diagnostic;
pub mod interpreter;
pub mod lexer;
pub mod parser;

use ast::Program;
use diagnostic::Diagnostic;
use interpreter::Value;

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

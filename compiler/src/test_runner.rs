//! Experimental test discovery and reference execution.

use std::path::Path;

use crate::ast::{Span, Type};
use crate::diagnostic::Diagnostic;
use crate::interpreter::{self, Value};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct TestCase {
    pub name: String,
    pub outcome: Outcome,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub cases: Vec<TestCase>,
}

impl Report {
    pub fn failed(&self) -> usize {
        self.cases
            .iter()
            .filter(|case| matches!(&case.outcome, Outcome::Failed(_)))
            .count()
    }

    pub fn display(&self) -> String {
        let mut out = String::new();
        for case in &self.cases {
            match &case.outcome {
                Outcome::Passed => out.push_str(&format!("PASS {}\n", case.name)),
                Outcome::Failed(reason) => {
                    out.push_str(&format!("FAIL {}: {}\n", case.name, reason))
                }
            }
        }
        out.push_str(&format!(
            "{} passed; {} failed",
            self.cases.len() - self.failed(),
            self.failed()
        ));
        out
    }
}

/// Check a source file, validate all test signatures, then run each test.
pub fn run(source: &str, root: Option<&Path>) -> Result<Report, Diagnostic> {
    run_with_capabilities(source, root, None)
}

pub fn run_with_capabilities(
    source: &str,
    read_root: Option<&Path>,
    write_root: Option<&Path>,
) -> Result<Report, Diagnostic> {
    let program = crate::check(source)?;
    let tests = program
        .functions
        .iter()
        .filter(|function| function.name.starts_with("test_"))
        .collect::<Vec<_>>();
    if tests.is_empty() {
        return Err(Diagnostic::new(
            "E203",
            Span { start: 0, end: 0 },
            "no test_ functions found",
        ));
    }
    for function in &tests {
        let valid_return = function.ret == Type::Bool
            || matches!(&function.ret, Type::Result(ok, _) if **ok == Type::Bool);
        if !function.params.is_empty() || !function.type_params.is_empty() || !valid_return {
            return Err(Diagnostic::new(
                "E203",
                function.span,
                format!(
                    "{} must have no parameters or type parameters and return bool or Result<bool,E>",
                    function.name
                ),
            ));
        }
    }

    let cases = tests
        .iter()
        .map(|function| {
            let outcome = match interpreter::run_named_with_capabilities(
                &program,
                &function.name,
                read_root,
                write_root,
            ) {
                Ok(Value::Bool(true)) => Outcome::Passed,
                Ok(Value::Ok(value)) if matches!(value.as_ref(), Value::Bool(true)) => {
                    Outcome::Passed
                }
                Ok(Value::Bool(false)) | Ok(Value::Ok(_)) => {
                    Outcome::Failed("returned false".to_owned())
                }
                Ok(Value::Err(error)) => Outcome::Failed(format!("returned Err({error})")),
                Ok(value) => Outcome::Failed(format!("unexpected result {value}")),
                Err(error) => Outcome::Failed(error.display(source)),
            };
            TestCase {
                name: function.name.clone(),
                outcome,
            }
        })
        .collect();
    Ok(Report { cases })
}

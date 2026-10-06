//! `tok bench` and `tok repl`.

use std::io::{BufRead, Write};

use crate::ast::{Program, Span, Type};

/// The checked `main` that times every parameterless `bench_*` function.
fn bench_main(names: &[String], iterations: u32) -> String {
    let warmup = (iterations / 10).max(1);
    let mut body = String::new();
    for name in names {
        body.push_str(&format!(
            "{{var i=0;while i<{warmup}{{let r={name}();i=i+1;}}let s=clock_ns();var j=0;while j<{iterations}{{let r={name}();j=j+1;}}let e=clock_ns()-s;print(\"{name} \"+String(e/{iterations}i64)+\" ns/iter ({iterations} iterations)\");}}"
        ));
    }
    format!("main()->Unit{{{body}}}")
}

/// Replace `main` with a timing loop over the `bench_*` functions.
pub fn bench_program(
    mut program: Program,
    iterations: u32,
) -> Result<(Program, Vec<String>), String> {
    let names: Vec<String> = program
        .functions
        .iter()
        .filter(|function| {
            function.name.starts_with("bench_")
                && function.params.is_empty()
                && function.type_params.is_empty()
        })
        .map(|function| function.name.clone())
        .collect();
    if names.is_empty() {
        return Err("no parameterless bench_* functions found".to_owned());
    }
    let wrapper = crate::parse(&bench_main(&names, iterations.max(1)))
        .map_err(|error| format!("cannot build benchmark driver: {error:?}"))?;
    program.functions.retain(|function| function.name != "main");
    program.functions.extend(wrapper.functions);
    Ok((program, names))
}

struct Session {
    declarations: Vec<String>,
    statements: Vec<String>,
}

impl Session {
    fn source(&self) -> String {
        self.declarations.join("\n")
    }

    /// Run `expr` after the remembered statements and render its value.
    fn evaluate(&self, expr: &str) -> Result<String, String> {
        let prefix = format!(
            "{}\n__probe()->Unit{{{}let __it=",
            self.source(),
            self.statements.concat()
        );
        let probe = format!("{prefix}{expr};}}");
        let program = crate::parse(&probe).map_err(|error| show(&probe, &error))?;
        let types =
            crate::checker::check_with_types(&program).map_err(|error| show(&probe, &error))?;
        let span = Span::new(prefix.len(), prefix.len() + expr.len());
        let ty = types.get(&span).cloned().unwrap_or(Type::Unit);
        let ret = match ty {
            Type::Never | Type::EmptyArray => "Unit".to_owned(),
            other => other.to_string(),
        };
        let source = format!(
            "{}\nmain()->{ret}{{{}{expr}}}",
            self.source(),
            self.statements.concat()
        );
        let value = crate::run(&source).map_err(|error| show(&source, &error))?;
        Ok(value.to_string())
    }
}

/// Session sources are synthesized, so positions would mislead; show the code and message.
fn show(_source: &str, error: &crate::diagnostic::Diagnostic) -> String {
    format!("{} {}", error.code, error.message)
}

/// Read lines from `input` until EOF or `:quit`, writing results to `output`.
pub fn repl(input: &mut impl BufRead, output: &mut impl Write) -> std::io::Result<()> {
    let mut session = Session {
        declarations: Vec::new(),
        statements: Vec::new(),
    };
    writeln!(
        output,
        "Tokit REPL. Enter declarations, let/var statements, or expressions; :reset, :quit."
    )?;
    let mut line = String::new();
    loop {
        write!(output, "> ")?;
        output.flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        let entry = line.trim();
        match entry {
            "" => continue,
            ":quit" | ":q" => break,
            ":reset" => {
                session.declarations.clear();
                session.statements.clear();
                continue;
            }
            _ => {}
        }
        if entry.starts_with("let ") || entry.starts_with("var ") {
            let statement = if entry.ends_with(';') {
                entry.to_owned()
            } else {
                format!("{entry};")
            };
            let mut candidate = Session {
                declarations: session.declarations.clone(),
                statements: session.statements.clone(),
            };
            candidate.statements.push(statement);
            match candidate.evaluate("()") {
                Ok(_) => session = candidate,
                Err(error) => writeln!(output, "{error}")?,
            }
            continue;
        }
        let declaration = crate::parse(entry).is_ok_and(|program| {
            !program.functions.is_empty()
                || !program.records.is_empty()
                || !program.enums.is_empty()
        });
        if declaration {
            let mut declarations = session.declarations.clone();
            declarations.push(entry.to_owned());
            let source = declarations.join("\n");
            match crate::check(&source) {
                Ok(_) => session.declarations = declarations,
                Err(error) => writeln!(output, "{}", show(&source, &error))?,
            }
            continue;
        }
        match session.evaluate(entry) {
            Ok(value) => writeln!(output, "{value}")?,
            Err(error) => writeln!(output, "{error}")?,
        }
    }
    Ok(())
}

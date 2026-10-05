//! Canonical whitespace for the experimental token grammar.

use crate::diagnostic::Diagnostic;
use crate::lexer::{self, Kind, Token};

fn needs_space(previous: &Token, current: &Token, source: &str) -> bool {
    let joined = format!(
        "{}{}",
        &source[previous.span.start..previous.span.end],
        &source[current.span.start..current.span.end]
    );
    let Ok(tokens) = lexer::lex(&joined) else {
        return true;
    };
    tokens.len() != 3 || tokens[0].kind != previous.kind || tokens[1].kind != current.kind
}

fn append_comments(out: &mut String, trivia: &str) -> bool {
    let mut found = false;
    for line in trivia.lines() {
        if let Some(index) = line.find("//") {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(line[index..].trim_end());
            out.push('\n');
            found = true;
        }
    }
    found
}

/// Format a parseable source file while preserving the lexer token sequence.
pub fn format(source: &str) -> Result<String, Diagnostic> {
    crate::parse(source)?;
    let tokens = lexer::lex(source)?;
    let mut out = String::new();
    let mut previous: Option<&Token> = None;
    let mut last_end = 0;
    let mut brace_depth = 0usize;
    for token in tokens.iter().filter(|token| token.kind != Kind::Eof) {
        let comments = append_comments(&mut out, &source[last_end..token.span.start]);
        if let Some(before) = previous
            && !comments
            && !out.ends_with('\n')
        {
            if before.kind == Kind::RBrace
                && brace_depth == 0
                && matches!(
                    token.kind,
                    Kind::Fn | Kind::Ident(_) | Kind::Struct | Kind::Enum
                )
            {
                out.push('\n');
            } else if needs_space(before, token, source) {
                out.push(' ');
            }
        }
        out.push_str(&source[token.span.start..token.span.end]);
        match token.kind {
            Kind::LBrace => brace_depth += 1,
            Kind::RBrace => brace_depth -= 1,
            _ => {}
        }
        last_end = token.span.end;
        previous = Some(token);
    }
    append_comments(&mut out, &source[last_end..]);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Remove optional `fn` tokens from declarations, preserving every other byte.
pub fn compact_functions(source: &str) -> Result<String, Diagnostic> {
    let program = crate::parse(source)?;
    let tokens = lexer::lex(source)?;
    let mut removals = Vec::new();
    for function in &program.functions {
        let Some(index) = tokens
            .iter()
            .position(|token| token.span.start == function.span.start)
        else {
            continue;
        };
        if tokens[index].kind != Kind::Fn {
            continue;
        }
        let end = tokens[index + 1].span.start;
        let between = &source[tokens[index].span.end..end];
        removals.push((
            tokens[index].span.start,
            if between.chars().all(char::is_whitespace) {
                end
            } else {
                tokens[index].span.end
            },
        ));
    }
    let mut result = source.to_owned();
    for (start, end) in removals.into_iter().rev() {
        result.replace_range(start..end, "");
    }
    crate::parse(&result)?;
    Ok(result)
}

//! Canonical whitespace for the experimental token grammar.

use crate::ast::{Expr, ExprKind, Stmt};
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

/// Replace only parsed primitive type spellings, leaving names and literals intact.
pub fn compact_integer_types(source: &str) -> Result<String, Diagnostic> {
    let mut parser = crate::parser::Parser::new(lexer::lex(source)?);
    parser.program()?;
    let mut result = source.to_owned();
    for (span, replacement) in parser.compactible_types().iter().rev() {
        result.replace_range(span.start..span.end, replacement);
    }
    crate::parse(&result)?;
    Ok(result)
}

/// Tokens that cannot continue an expression, so a block-like statement
/// followed by one of them needs no `;`.
fn starts_statement_or_closes(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Ident(_)
            | Kind::Let
            | Kind::Var
            | Kind::For
            | Kind::While
            | Kind::Return
            | Kind::Break
            | Kind::Continue
            | Kind::If
            | Kind::Match
            | Kind::RBrace
    )
}

/// An `if` chain or block whose branches all end without a value.
fn statement_shaped(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Block(_, tail) => tail.is_none(),
        ExprKind::If(_, yes, no) => statement_shaped(yes) && statement_shaped(no),
        _ => false,
    }
}

struct BlockCompactor<'a> {
    source: &'a str,
    tokens: &'a [Token],
    removals: Vec<(usize, usize)>,
}

impl BlockCompactor<'_> {
    fn token_after(&self, offset: usize) -> Option<usize> {
        self.tokens
            .iter()
            .position(|token| token.span.start >= offset && token.kind != Kind::Eof)
    }

    fn only(&self, start: usize, end: usize, expected: &str) -> bool {
        self.source[start..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .eq(expected.chars())
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::I64(_)
            | ExprKind::F64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_)
            | ExprKind::None => {}
            ExprKind::Array(items) | ExprKind::Call(_, items) => {
                items.iter().for_each(|item| self.expr(item));
            }
            ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Field(inner, _)
            | ExprKind::Ok(inner)
            | ExprKind::Err(inner)
            | ExprKind::Some(inner)
            | ExprKind::Try(inner)
            | ExprKind::Not(inner)
            | ExprKind::Neg(inner)
            | ExprKind::Spawn(inner) => self.expr(inner),
            ExprKind::Variant(_, _, payload) => {
                if let Some(payload) = payload {
                    self.expr(payload);
                }
            }
            ExprKind::If(condition, yes, no) => {
                self.expr(condition);
                self.expr(yes);
                self.expr(no);
                if matches!(&no.kind, ExprKind::Block(stmts, None) if stmts.is_empty())
                    && no.span.start < no.span.end
                    && self.only(yes.span.end, no.span.end, "else{}")
                {
                    self.removals.push((yes.span.end, no.span.end));
                }
            }
            ExprKind::Match(value, arms) => {
                self.expr(value);
                arms.iter().for_each(|(_, body)| self.expr(body));
            }
            ExprKind::Block(stmts, tail) => {
                stmts.iter().for_each(|stmt| self.stmt(stmt));
                if let Some(tail) = tail {
                    self.expr(tail);
                }
            }
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { value, .. }
            | Stmt::Assign { value, .. }
            | Stmt::Push { value, .. }
            | Stmt::Return { value, .. } => self.expr(value),
            Stmt::For { iterable, body, .. } => {
                self.expr(iterable);
                self.expr(body);
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expr(condition);
                self.expr(body);
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
            Stmt::Expr(value) => {
                self.expr(value);
                if !statement_shaped(value) {
                    return;
                }
                let Some(semicolon) = self.token_after(value.span.end) else {
                    return;
                };
                let end = self.tokens[semicolon].span.end;
                if self.tokens[semicolon].kind == Kind::Semicolon
                    && self.only(value.span.end, end, ";")
                    && self
                        .tokens
                        .get(semicolon + 1)
                        .is_some_and(|next| starts_statement_or_closes(&next.kind))
                {
                    self.removals.push((value.span.end, end));
                }
            }
        }
    }
}

/// Remove empty `else{}` branches and the `;` after value-less block statements.
pub fn compact_blocks(source: &str) -> Result<String, Diagnostic> {
    let program = crate::parse(source)?;
    let tokens = lexer::lex(source)?;
    let mut compactor = BlockCompactor {
        source,
        tokens: &tokens,
        removals: Vec::new(),
    };
    for function in &program.functions {
        compactor.expr(&function.body);
    }
    let mut removals = compactor.removals;
    removals.sort_unstable();
    let mut result = source.to_owned();
    for (start, end) in removals.into_iter().rev() {
        result.replace_range(start..end, "");
    }
    crate::parse(&result)?;
    Ok(result)
}

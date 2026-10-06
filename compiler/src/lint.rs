//! `tok lint`: advisory warnings for checked programs. The analysis is
//! name-based within each function, so a shadowed name counts as used when
//! any binding of it is read. Names starting with `_` are never reported.

use std::collections::HashSet;

use crate::ast::{Expr, ExprKind, PlaceStep, Program, Span, Stmt};
use crate::diagnostic::Diagnostic;

#[derive(Default)]
struct Usage {
    reads: HashSet<String>,
    writes: HashSet<String>,
    calls: HashSet<String>,
    bindings: Vec<(String, bool, Span)>,
}

impl Usage {
    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(_)
            | ExprKind::I64(_)
            | ExprKind::F64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::None => {}
            ExprKind::Var(name) => {
                self.reads.insert(name.clone());
            }
            ExprKind::Call(name, args) => {
                self.calls.insert(name.clone());
                self.reads.insert(name.clone());
                args.iter().for_each(|arg| self.expr(arg));
            }
            ExprKind::Array(items) => items.iter().for_each(|item| self.expr(item)),
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
            }
            ExprKind::Match(value, arms) => {
                self.expr(value);
                arms.iter().for_each(|(_, body)| self.expr(body));
            }
            ExprKind::Lambda(_, body) => self.expr(body),
            ExprKind::Apply(callee, args) => {
                self.expr(callee);
                args.iter().for_each(|arg| self.expr(arg));
            }
            ExprKind::Block(stmts, tail) => {
                for stmt in stmts {
                    self.stmt(stmt);
                }
                if let Some(tail) = tail {
                    self.expr(tail);
                }
            }
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name,
                value,
                mutable,
                span,
                ..
            } => {
                self.bindings.push((name.clone(), *mutable, *span));
                self.expr(value);
            }
            Stmt::Assign {
                name, path, value, ..
            } => {
                self.writes.insert(name.clone());
                if !path.is_empty() {
                    // Updating part of a value also depends on the rest of it.
                    self.reads.insert(name.clone());
                }
                for step in path {
                    if let PlaceStep::Index(index, _) = step {
                        self.expr(index);
                    }
                }
                self.expr(value);
            }
            Stmt::Push { name, value, .. } => {
                self.writes.insert(name.clone());
                self.reads.insert(name.clone());
                self.expr(value);
            }
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
            Stmt::Return { value, .. } | Stmt::Expr(value) => self.expr(value),
            Stmt::Break { .. } | Stmt::Continue { .. } => {}
        }
    }
}

/// Warnings sorted by position: W001 unused binding, W002 `var` never
/// changed, W003 private function never called.
pub fn lint(program: &Program) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    let mut called = HashSet::new();
    for function in &program.functions {
        let mut usage = Usage::default();
        usage.expr(&function.body);
        called.extend(usage.calls.iter().cloned());
        for (name, mutable, span) in &usage.bindings {
            if name.starts_with('_') {
                continue;
            }
            if !usage.reads.contains(name) {
                warnings.push(Diagnostic::new(
                    "W001",
                    *span,
                    format!("{name} is never read; remove it or name it _{name}"),
                ));
            } else if *mutable && !usage.writes.contains(name) {
                warnings.push(Diagnostic::new(
                    "W002",
                    *span,
                    format!("{name} is never changed; declare it with let"),
                ));
            }
        }
    }
    for function in &program.functions {
        let entry = function.name == "main"
            || function.name.starts_with("test_")
            || function.name.starts_with("bench_");
        if !function.public
            && !entry
            && !function.name.starts_with('_')
            && !called.contains(&function.name)
        {
            warnings.push(Diagnostic::new(
                "W003",
                function.span,
                format!("private function {} is never called", function.name),
            ));
        }
    }
    warnings.sort_by_key(|warning| (warning.span.source_id.0, warning.span.start));
    warnings
}

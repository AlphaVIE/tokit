//! Structural counts for the checked experimental language subset.

use crate::ast::{Expr, ExprKind, Program, Stmt, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub bytes: usize,
    pub chars: usize,
    pub ast_nodes: usize,
    pub semantic_ops: usize,
    pub functions: usize,
    pub declarations: usize,
    pub dependencies: usize,
}

impl Stats {
    pub fn json(self) -> String {
        format!(
            "{{\"bytes\":{},\"chars\":{},\"ast_nodes\":{},\"semantic_ops\":{},\"functions\":{},\"declarations\":{},\"dependencies\":{}}}",
            self.bytes,
            self.chars,
            self.ast_nodes,
            self.semantic_ops,
            self.functions,
            self.declarations,
            self.dependencies
        )
    }
}

fn count_type(ty: &Type, nodes: &mut usize) {
    *nodes += 1;
    match ty {
        Type::Array(element) => count_type(element, nodes),
        Type::Result(ok, err) => {
            count_type(ok, nodes);
            count_type(err, nodes);
        }
        Type::Applied(_, args) => {
            for arg in args {
                count_type(arg, nodes);
            }
        }
        _ => {}
    }
}

fn count_expr(expr: &Expr, nodes: &mut usize, ops: &mut usize) {
    *nodes += 1;
    match &expr.kind {
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Var(_) => {}
        ExprKind::Variant(_, _, payload) => {
            *ops += 1;
            if let Some(payload) = payload {
                count_expr(payload, nodes, ops);
            }
        }
        ExprKind::Array(values) => {
            *ops += 1;
            for value in values {
                count_expr(value, nodes, ops);
            }
        }
        ExprKind::Index(array, index) | ExprKind::Binary(array, _, index) => {
            *ops += 1;
            count_expr(array, nodes, ops);
            count_expr(index, nodes, ops);
        }
        ExprKind::Field(value, _)
        | ExprKind::Ok(value)
        | ExprKind::Err(value)
        | ExprKind::Try(value) => {
            *ops += 1;
            count_expr(value, nodes, ops);
        }
        ExprKind::Call(_, args) => {
            *ops += 1;
            for arg in args {
                count_expr(arg, nodes, ops);
            }
        }
        ExprKind::If(condition, yes, no) => {
            *ops += 1;
            count_expr(condition, nodes, ops);
            count_expr(yes, nodes, ops);
            count_expr(no, nodes, ops);
        }
        ExprKind::Match(value, arms) => {
            *ops += 1;
            count_expr(value, nodes, ops);
            for (_, body) in arms {
                *nodes += 1; // pattern
                count_expr(body, nodes, ops);
            }
        }
        ExprKind::Block(statements, tail) => {
            for statement in statements {
                count_stmt(statement, nodes, ops);
            }
            if let Some(tail) = tail {
                count_expr(tail, nodes, ops);
            }
        }
    }
}

fn count_stmt(stmt: &Stmt, nodes: &mut usize, ops: &mut usize) {
    *nodes += 1;
    match stmt {
        Stmt::Let { ty, value, .. } => {
            *ops += 1;
            count_type(ty, nodes);
            count_expr(value, nodes, ops);
        }
        Stmt::Assign { value, .. } | Stmt::Return { value, .. } => {
            *ops += 1;
            count_expr(value, nodes, ops);
        }
        Stmt::For { iterable, body, .. } => {
            *ops += 1;
            count_expr(iterable, nodes, ops);
            count_expr(body, nodes, ops);
        }
        Stmt::Expr(expr) => count_expr(expr, nodes, ops),
    }
}

/// Count source and AST structure after parsing and type checking.
pub fn measure(source: &str, program: &Program) -> Stats {
    let mut nodes = 1; // Program root.
    let mut ops = 0;
    for record in &program.records {
        nodes += 1;
        for (_, ty) in &record.fields {
            nodes += 1; // Field declaration.
            count_type(ty, &mut nodes);
        }
    }
    for decl in &program.enums {
        nodes += 1 + decl.variants.len();
        for variant in &decl.variants {
            if let Some(ty) = &variant.payload {
                count_type(ty, &mut nodes);
            }
        }
    }
    for function in &program.functions {
        nodes += 1;
        for (_, ty) in &function.params {
            nodes += 1; // Parameter declaration.
            count_type(ty, &mut nodes);
        }
        count_type(&function.ret, &mut nodes);
        count_expr(&function.body, &mut nodes, &mut ops);
    }
    Stats {
        bytes: source.len(),
        chars: source.chars().count(),
        ast_nodes: nodes,
        semantic_ops: ops,
        functions: program.functions.len(),
        declarations: program.records.len() + program.enums.len() + program.functions.len(),
        dependencies: 0, // Imports are not supported in this subset.
    }
}

//! Readable renderings of a parsed program: expanded Tokit, which parses back
//! to the same program, and indentation-based pseudocode for human review.

use crate::ast::{
    EnumDecl, Expr, ExprKind, Function, Op, Pattern, PatternKind, PlaceStep, Program, Record, Stmt,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Tokit,
    Pseudo,
}

/// Multi-line Tokit with `fn`, full type names, and spaces; it parses back.
pub fn expand(program: &Program) -> String {
    Printer::new(Style::Tokit).program(program)
}

/// Indented pseudocode without braces or semicolons.
pub fn pseudocode(program: &Program) -> String {
    Printer::new(Style::Pseudo).program(program)
}

struct Printer {
    style: Style,
}

const INDENT: &str = "    ";

fn precedence(op: Op) -> u8 {
    match op {
        Op::Or => 0,
        Op::And => 1,
        Op::Eq | Op::Ne => 2,
        Op::Lt | Op::Le | Op::Gt | Op::Ge => 3,
        Op::Add | Op::Sub => 4,
        Op::Mul | Op::Div | Op::Rem => 5,
    }
}

fn string_literal(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn generics(params: &[String]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

/// Whether a parsed `if` has no written `else` (the parser fills in an empty block).
fn missing_else(no: &Expr) -> bool {
    matches!(&no.kind, ExprKind::Block(stmts, None) if stmts.is_empty())
        && no.span.start == no.span.end
}

impl Printer {
    fn new(style: Style) -> Self {
        Self { style }
    }

    fn pseudo(&self) -> bool {
        self.style == Style::Pseudo
    }

    fn program(&self, program: &Program) -> String {
        let mut parts = Vec::new();
        for import in &program.imports {
            parts.push(if self.pseudo() {
                format!(
                    "import {} from {}",
                    import.alias,
                    string_literal(&import.path)
                )
            } else {
                format!(
                    "import {} = {};",
                    import.alias,
                    string_literal(&import.path)
                )
            });
        }
        parts.extend(program.records.iter().map(|record| self.record(record)));
        parts.extend(program.enums.iter().map(|decl| self.enumeration(decl)));
        parts.extend(
            program
                .functions
                .iter()
                .map(|function| self.function(function)),
        );
        let mut out = parts.join("\n\n");
        out.push('\n');
        out
    }

    fn visibility(public: bool) -> &'static str {
        if public { "pub " } else { "" }
    }

    fn record(&self, record: &Record) -> String {
        let fields = record
            .fields
            .iter()
            .map(|(name, ty)| format!("{INDENT}{name}: {ty}"))
            .collect::<Vec<_>>();
        if self.pseudo() {
            format!(
                "{}record {}{}\n{}",
                Self::visibility(record.public),
                record.name,
                generics(&record.type_params),
                fields.join("\n")
            )
        } else {
            format!(
                "{}struct {}{} {{\n{}\n}}",
                Self::visibility(record.public),
                record.name,
                generics(&record.type_params),
                fields.join(",\n")
            )
        }
    }

    fn enumeration(&self, decl: &EnumDecl) -> String {
        let variants = decl
            .variants
            .iter()
            .map(|variant| match &variant.payload {
                Some(payload) => format!("{INDENT}{}({payload})", variant.name),
                None => format!("{INDENT}{}", variant.name),
            })
            .collect::<Vec<_>>();
        if self.pseudo() {
            format!(
                "{}choice {}{} is one of\n{}",
                Self::visibility(decl.public),
                decl.name,
                generics(&decl.type_params),
                variants.join("\n")
            )
        } else {
            format!(
                "{}enum {}{} {{\n{}\n}}",
                Self::visibility(decl.public),
                decl.name,
                generics(&decl.type_params),
                variants.join(",\n")
            )
        }
    }

    fn function(&self, function: &Function) -> String {
        let params = function
            .params
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect::<Vec<_>>()
            .join(", ");
        let header = if self.pseudo() {
            format!(
                "{}function {}{}({params}) returns {}",
                Self::visibility(function.public),
                function.name,
                generics(&function.type_params),
                function.ret
            )
        } else {
            format!(
                "{}fn {}{}({params}) -> {}",
                Self::visibility(function.public),
                function.name,
                generics(&function.type_params),
                function.ret
            )
        };
        format!("{header}{}", self.attach(&function.body, 0))
    }

    /// A block (or other expression used as a body) at `depth`.
    fn block_body(&self, expr: &Expr, depth: usize) -> String {
        let (stmts, tail) = match &expr.kind {
            ExprKind::Block(stmts, tail) => (stmts.as_slice(), tail.as_deref()),
            _ => (&[][..], Some(expr)),
        };
        let inner = INDENT.repeat(depth + 1);
        let mut lines: Vec<String> = stmts
            .iter()
            .map(|stmt| format!("{inner}{}", self.stmt(stmt, depth + 1)))
            .collect();
        if let Some(tail) = tail {
            let value = self.expr(tail, depth + 1);
            lines.push(if self.pseudo() && !tail.is_block_like() {
                format!("{inner}=> {value}")
            } else {
                format!("{inner}{value}")
            });
        }
        if self.pseudo() {
            if lines.is_empty() {
                lines.push(format!("{inner}(nothing)"));
            }
            format!(":\n{}", lines.join("\n"))
        } else if lines.is_empty() {
            "{}".to_owned()
        } else {
            format!("{{\n{}\n{}}}", lines.join("\n"), INDENT.repeat(depth))
        }
    }

    /// A body after a header: ` {...}` in Tokit, `:` plus indented lines in pseudocode.
    fn attach(&self, body: &Expr, depth: usize) -> String {
        let block = self.block_body(body, depth);
        if self.pseudo() {
            block
        } else {
            format!(" {block}")
        }
    }

    fn end(&self, text: String) -> String {
        if self.pseudo() {
            text
        } else {
            format!("{text};")
        }
    }

    fn stmt(&self, stmt: &Stmt, depth: usize) -> String {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                mutable,
                ..
            } => {
                let keyword = match (self.pseudo(), mutable) {
                    (true, true) => "variable",
                    (false, true) => "var",
                    (_, false) => "let",
                };
                let ty = ty.as_ref().map(|ty| format!(": {ty}")).unwrap_or_default();
                self.end(format!(
                    "{keyword} {name}{ty} = {}",
                    self.expr(value, depth)
                ))
            }
            Stmt::Assign {
                name, path, value, ..
            } => {
                let mut target = name.clone();
                for step in path {
                    match step {
                        PlaceStep::Index(index, _) => {
                            target.push_str(&format!("[{}]", self.expr(index, depth)));
                        }
                        PlaceStep::Field(field, _) => target.push_str(&format!(".{field}")),
                    }
                }
                let operator = if self.pseudo() { "becomes" } else { "=" };
                self.end(format!("{target} {operator} {}", self.expr(value, depth)))
            }
            Stmt::Push { name, value, .. } => {
                if self.pseudo() {
                    format!("append {} to {name}", self.expr(value, depth))
                } else {
                    format!("{name}.push({});", self.expr(value, depth))
                }
            }
            Stmt::For {
                name,
                iterable,
                body,
                ..
            } => format!(
                "for {name} in {}{}",
                self.expr(iterable, depth),
                self.attach(body, depth)
            ),
            Stmt::While {
                condition, body, ..
            } => format!(
                "while {}{}",
                self.expr(condition, depth),
                self.attach(body, depth)
            ),
            Stmt::Break { .. } => self.end("break".to_owned()),
            Stmt::Continue { .. } => self.end("continue".to_owned()),
            Stmt::Return { value, .. } => self.end(format!("return {}", self.expr(value, depth))),
            Stmt::Expr(value) => {
                let text = self.expr(value, depth);
                if value.is_block_like() {
                    text
                } else {
                    self.end(text)
                }
            }
        }
    }

    fn operator(&self, op: Op) -> &'static str {
        match (op, self.pseudo()) {
            (Op::And, true) => "and",
            (Op::Or, true) => "or",
            (Op::And, false) => "&&",
            (Op::Or, false) => "||",
            (Op::Add, _) => "+",
            (Op::Sub, _) => "-",
            (Op::Mul, _) => "*",
            (Op::Div, _) => "/",
            (Op::Rem, _) => "%",
            (Op::Eq, _) => "==",
            (Op::Ne, _) => "!=",
            (Op::Lt, _) => "<",
            (Op::Le, _) => "<=",
            (Op::Gt, _) => ">",
            (Op::Ge, _) => ">=",
        }
    }

    fn operand(&self, expr: &Expr, depth: usize, parent: u8, right: bool) -> String {
        let text = self.expr(expr, depth);
        match &expr.kind {
            ExprKind::Binary(_, op, _)
                if precedence(*op) < parent || (right && precedence(*op) == parent) =>
            {
                format!("({text})")
            }
            // Block-like operands cannot start or continue an operator chain.
            ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Block(..) | ExprKind::Lambda(..) => {
                format!("({text})")
            }
            _ => text,
        }
    }

    fn args(&self, args: &[Expr], depth: usize) -> String {
        args.iter()
            .map(|arg| self.expr(arg, depth))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn pattern(pattern: &Pattern) -> String {
        pattern_text(pattern)
    }

    fn expr(&self, expr: &Expr, depth: usize) -> String {
        match &expr.kind {
            ExprKind::Int(value) => value.to_string(),
            ExprKind::I64(value) => format!("{value}i64"),
            ExprKind::F64(bits) => format!("{:?}", f64::from_bits(*bits)),
            ExprKind::Bool(value) => value.to_string(),
            ExprKind::String(value) => string_literal(value),
            ExprKind::Var(name) => name.clone(),
            ExprKind::None => "None".to_owned(),
            ExprKind::Array(items) => format!("[{}]", self.args(items, depth)),
            ExprKind::Index(array, index) => format!(
                "{}[{}]",
                self.operand(array, depth, u8::MAX, false),
                self.expr(index, depth)
            ),
            ExprKind::Field(value, field) => {
                format!("{}.{field}", self.operand(value, depth, u8::MAX, false))
            }
            ExprKind::Variant(name, variant, payload) => match payload {
                Some(payload) => format!("{name}::{variant}({})", self.expr(payload, depth)),
                None => format!("{name}::{variant}"),
            },
            ExprKind::Ok(inner) => format!("Ok({})", self.expr(inner, depth)),
            ExprKind::Err(inner) => format!("Err({})", self.expr(inner, depth)),
            ExprKind::Some(inner) => format!("Some({})", self.expr(inner, depth)),
            ExprKind::Try(inner) => format!("{}?", self.operand(inner, depth, u8::MAX, false)),
            ExprKind::Not(inner) => {
                let operand = self.operand(inner, depth, u8::MAX, false);
                if self.pseudo() {
                    format!("not {operand}")
                } else {
                    format!("!{operand}")
                }
            }
            ExprKind::Neg(inner) => {
                let text = self.expr(inner, depth);
                match inner.kind {
                    ExprKind::Var(_)
                    | ExprKind::Call(..)
                    | ExprKind::Field(..)
                    | ExprKind::Index(..) => {
                        format!("-{text}")
                    }
                    _ => format!("-({text})"),
                }
            }
            ExprKind::Binary(left, op, right) => {
                let level = precedence(*op);
                format!(
                    "{} {} {}",
                    self.operand(left, depth, level, false),
                    self.operator(*op),
                    self.operand(right, depth, level, true)
                )
            }
            ExprKind::Call(name, args) => format!("{name}({})", self.args(args, depth)),
            ExprKind::Apply(callee, args) => format!(
                "{}({})",
                self.operand(callee, depth, u8::MAX, false),
                self.args(args, depth)
            ),
            ExprKind::Spawn(call) => format!("spawn {}", self.expr(call, depth)),
            ExprKind::If(condition, yes, no) => {
                let mut text = format!(
                    "if {}{}",
                    self.expr(condition, depth),
                    self.attach(yes, depth)
                );
                if !missing_else(no) {
                    let indent = INDENT.repeat(depth);
                    let separator = if self.pseudo() {
                        format!("\n{indent}")
                    } else {
                        " ".to_owned()
                    };
                    match &no.kind {
                        ExprKind::If(..) => {
                            text.push_str(&format!("{separator}else {}", self.expr(no, depth)));
                        }
                        _ => text
                            .push_str(&format!("{separator}else {}", self.block_body(no, depth))),
                    }
                }
                text
            }
            ExprKind::Match(value, arms) => {
                let inner = INDENT.repeat(depth + 1);
                let lines = arms
                    .iter()
                    .map(|(pattern, body)| {
                        let body = self.expr(body, depth + 1);
                        if self.pseudo() {
                            format!("{inner}case {} => {body}", Self::pattern(pattern))
                        } else {
                            format!("{inner}{} => {body},", Self::pattern(pattern))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if self.pseudo() {
                    format!("match {}:\n{lines}", self.expr(value, depth))
                } else {
                    format!(
                        "match {} {{\n{lines}\n{}}}",
                        self.expr(value, depth),
                        INDENT.repeat(depth)
                    )
                }
            }
            ExprKind::Block(..) => {
                if self.pseudo() {
                    format!("do{}", self.block_body(expr, depth))
                } else {
                    self.block_body(expr, depth)
                }
            }
            ExprKind::Lambda(params, body) => {
                let params = params
                    .iter()
                    .map(|(name, ty)| match ty {
                        Some(ty) => format!("{name}: {ty}"),
                        None => name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let body = self.expr(body, depth);
                if self.pseudo() {
                    format!("(given {params} => {body})")
                } else {
                    format!("|{params}| {body}")
                }
            }
        }
    }
}

/// Source text of a pattern.
pub fn pattern_text(pattern: &Pattern) -> String {
    match &pattern.kind {
        PatternKind::Int(value) => value.to_string(),
        PatternKind::I64(value) => format!("{value}i64"),
        PatternKind::Wildcard => "_".to_owned(),
        PatternKind::Bind(name) => name.clone(),
        PatternKind::Ok(inner) => format!("Ok({})", pattern_text(inner)),
        PatternKind::Err(inner) => format!("Err({})", pattern_text(inner)),
        PatternKind::Some(inner) => format!("Some({})", pattern_text(inner)),
        PatternKind::None => "None".to_owned(),
        PatternKind::Variant(name, variant, inner) => match inner {
            Some(inner) => format!("{name}::{variant}({})", pattern_text(inner)),
            None => format!("{name}::{variant}"),
        },
        PatternKind::Bool(value) => value.to_string(),
        PatternKind::String(value) => string_literal(value),
    }
}

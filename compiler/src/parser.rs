use crate::ast::{
    EnumDecl, EnumVariant, Expr, ExprKind, Function, ImportDecl, Op, Pattern, PatternKind,
    PlaceStep, Program, Record, Span, Stmt, Type,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Kind, Token};
use std::collections::HashSet;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    type_params: Vec<String>,
    loop_depth: usize,
    import_aliases: HashSet<String>,
    compactible_types: Vec<(Span, &'static str)>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            type_params: Vec::new(),
            loop_depth: 0,
            import_aliases: HashSet::new(),
            compactible_types: Vec::new(),
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }
    pub fn compactible_types(&self) -> &[(Span, &'static str)] {
        &self.compactible_types
    }
    fn next_is(&self, kind: &Kind) -> bool {
        self.tokens.get(self.pos + 1).is_some_and(|token| {
            std::mem::discriminant(&token.kind) == std::mem::discriminant(kind)
        })
    }
    fn at_push_statement(&self) -> bool {
        matches!(&self.current().kind, Kind::Ident(_))
            && self.next_is(&Kind::Dot)
            && self
                .tokens
                .get(self.pos + 2)
                .is_some_and(|token| matches!(&token.kind, Kind::Ident(method) if method == "push"))
            && self
                .tokens
                .get(self.pos + 3)
                .is_some_and(|token| matches!(&token.kind, Kind::LParen))
    }
    /// Consume the `>` closing a type argument list. A following `=` may have
    /// been lexed together with it, as in `x:Option<I>=None`.
    fn close_angle(&mut self) -> Result<Span, Diagnostic> {
        let token = self.current().clone();
        if token.kind == Kind::Ge {
            let split = token.span.start + 1;
            self.tokens[self.pos].kind = Kind::Eq;
            self.tokens[self.pos].span.start = split;
            return Ok(Span::in_source(
                token.span.source_id,
                token.span.start,
                split,
            ));
        }
        self.expect(Kind::Gt).map(|token| token.span)
    }
    fn at(&self, kind: &Kind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }
    fn bump(&mut self) -> Token {
        let token = self.current().clone();
        if !self.at(&Kind::Eof) {
            self.pos += 1;
        }
        token
    }
    fn expect(&mut self, kind: Kind) -> Result<Token, Diagnostic> {
        if self.at(&kind) {
            Ok(self.bump())
        } else {
            Err(Diagnostic::new(
                "E002",
                self.current().span,
                format!("expected {:?}", kind),
            ))
        }
    }
    fn ident(&mut self) -> Result<(String, Span), Diagnostic> {
        let token = self.bump();
        if let Kind::Ident(name) = token.kind {
            Ok((name, token.span))
        } else {
            Err(Diagnostic::new("E002", token.span, "expected identifier"))
        }
    }
    fn negative_literal(&mut self, minus: Span) -> Result<(i32, Span), Diagnostic> {
        let number = self.expect(Kind::Int(String::new()))?;
        let span = minus.join(number.span);
        if minus.end != number.span.start {
            return Err(Diagnostic::new(
                "E002",
                span,
                "negative literal requires adjacent digits",
            ));
        }
        let Kind::Int(digits) = number.kind else {
            unreachable!()
        };
        let value = format!("-{digits}")
            .parse::<i32>()
            .map_err(|_| Diagnostic::new("E003", span, "i32 literal out of range"))?;
        Ok((value, span))
    }
    fn negative_i64_literal(&mut self, minus: Span) -> Result<(i64, Span), Diagnostic> {
        let number = self.expect(Kind::Int64(String::new()))?;
        let span = minus.join(number.span);
        if minus.end != number.span.start {
            return Err(Diagnostic::new(
                "E002",
                span,
                "negative literal requires adjacent digits",
            ));
        }
        let Kind::Int64(digits) = number.kind else {
            unreachable!()
        };
        let value = format!("-{digits}")
            .parse::<i64>()
            .map_err(|_| Diagnostic::new("E003", span, "i64 literal out of range"))?;
        Ok((value, span))
    }
    fn ty(&mut self) -> Result<Type, Diagnostic> {
        if self.at(&Kind::LParen) {
            self.bump();
            let mut params = Vec::new();
            while !self.at(&Kind::RParen) {
                params.push(self.ty()?);
                if !self.at(&Kind::RParen) {
                    self.expect(Kind::Comma)?;
                }
            }
            self.bump();
            self.expect(Kind::Arrow)?;
            let ret = self.ty()?;
            return Ok(Type::Fn(params, Box::new(ret)));
        }
        if self.at(&Kind::LBracket) {
            self.bump();
            let element = self.ty()?;
            self.expect(Kind::RBracket)?;
            return Ok(Type::Array(Box::new(element)));
        }
        let (mut name, span) = self.ident()?;
        if self.import_aliases.contains(&name) && self.at(&Kind::ColonColon) {
            self.bump();
            name = format!("{name}::{}", self.ident()?.0);
        }
        match name.as_str() {
            "i32" => self.compactible_types.push((span, "I")),
            "i64" => self.compactible_types.push((span, "L")),
            "f64" => self.compactible_types.push((span, "F")),
            _ => {}
        }
        match name.as_str() {
            "i32" | "I" => Ok(Type::I32),
            "i64" | "L" => Ok(Type::I64),
            "f64" | "F" => Ok(Type::F64),
            "bool" => Ok(Type::Bool),
            "String" => Ok(Type::String),
            "Bytes" => Ok(Type::Bytes),
            "Unit" => Ok(Type::Unit),
            "Result" => {
                self.expect(Kind::Lt)?;
                let ok = self.ty()?;
                self.expect(Kind::Comma)?;
                let err = self.ty()?;
                self.close_angle()?;
                Ok(Type::Result(Box::new(ok), Box::new(err)))
            }
            "Option" => {
                self.expect(Kind::Lt)?;
                let element = self.ty()?;
                self.close_angle()?;
                Ok(Type::Option(Box::new(element)))
            }
            "Task" => {
                self.expect(Kind::Lt)?;
                let result = self.ty()?;
                self.close_angle()?;
                Ok(Type::Task(Box::new(result)))
            }
            _ if self.type_params.contains(&name) => Ok(Type::Param(name)),
            _ if self.at(&Kind::Lt) => {
                self.bump();
                let mut args = Vec::new();
                loop {
                    args.push(self.ty()?);
                    if !self.at(&Kind::Comma) {
                        break;
                    }
                    self.bump();
                }
                self.close_angle()?;
                Ok(Type::Applied(name, args))
            }
            _ => Ok(Type::Named(name)),
        }
    }

    fn generic_params(&mut self) -> Result<Vec<String>, Diagnostic> {
        if !self.at(&Kind::Lt) {
            return Ok(Vec::new());
        }
        self.bump();
        let mut params = Vec::new();
        loop {
            params.push(self.ident()?.0);
            if !self.at(&Kind::Comma) {
                break;
            }
            self.bump();
        }
        self.close_angle()?;
        Ok(params)
    }

    pub fn program(&mut self) -> Result<Program, Diagnostic> {
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut records = Vec::new();
        let mut enums = Vec::new();
        let mut declarations_started = false;
        while !self.at(&Kind::Eof) {
            if self.at(&Kind::Import) {
                let start = self.bump().span;
                if declarations_started {
                    return Err(Diagnostic::new(
                        "E002",
                        start,
                        "imports must precede declarations",
                    ));
                }
                let (alias, alias_span) = self.ident()?;
                if !self.import_aliases.insert(alias.clone()) {
                    return Err(Diagnostic::new(
                        "E118",
                        alias_span,
                        "duplicate import alias",
                    ));
                }
                self.expect(Kind::Eq)?;
                let path = self.bump();
                let Kind::String(path_text) = path.kind else {
                    return Err(Diagnostic::new(
                        "E002",
                        path.span,
                        "expected import path string",
                    ));
                };
                let end = self.expect(Kind::Semicolon)?.span;
                imports.push(ImportDecl {
                    alias,
                    path: path_text,
                    span: start.join(end),
                });
            } else {
                declarations_started = true;
                let public = if self.at(&Kind::Pub) {
                    self.bump();
                    true
                } else {
                    false
                };
                if self.at(&Kind::Struct) {
                    let mut declaration = self.record()?;
                    declaration.public = public;
                    records.push(declaration);
                } else if self.at(&Kind::Enum) {
                    let mut declaration = self.enum_decl()?;
                    declaration.public = public;
                    enums.push(declaration);
                } else {
                    let mut declaration = self.function()?;
                    declaration.public = public;
                    functions.push(declaration);
                }
            }
        }
        if functions.is_empty() && records.is_empty() && enums.is_empty() && imports.is_empty() {
            return Err(Diagnostic::new(
                "E002",
                self.current().span,
                "expected a function, record, or enum",
            ));
        }
        Ok(Program {
            imports,
            records,
            enums,
            functions,
        })
    }

    fn enum_decl(&mut self) -> Result<EnumDecl, Diagnostic> {
        let start = self.expect(Kind::Enum)?.span;
        let (name, _) = self.ident()?;
        self.expect(Kind::LBrace)?;
        let mut variants = Vec::new();
        if !self.at(&Kind::RBrace) {
            loop {
                let (name, _) = self.ident()?;
                let payload = if self.at(&Kind::LParen) {
                    self.bump();
                    let ty = self.ty()?;
                    self.expect(Kind::RParen)?;
                    Some(ty)
                } else {
                    None
                };
                variants.push(EnumVariant { name, payload });
                if !self.at(&Kind::Comma) {
                    break;
                }
                self.bump();
            }
        }
        let end = self.expect(Kind::RBrace)?.span;
        Ok(EnumDecl {
            name,
            public: false,
            variants,
            span: start.join(end),
        })
    }

    fn record(&mut self) -> Result<Record, Diagnostic> {
        let start = self.expect(Kind::Struct)?.span;
        let (name, _) = self.ident()?;
        let type_params = self.generic_params()?;
        self.type_params = type_params.clone();
        self.expect(Kind::LBrace)?;
        let mut fields = Vec::new();
        if !self.at(&Kind::RBrace) {
            loop {
                let (field, _) = self.ident()?;
                self.expect(Kind::Colon)?;
                fields.push((field, self.ty()?));
                if !self.at(&Kind::Comma) {
                    break;
                }
                self.bump();
            }
        }
        let end = self.expect(Kind::RBrace)?.span;
        self.type_params.clear();
        Ok(Record {
            name,
            public: false,
            type_params,
            fields,
            span: start.join(end),
        })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = if self.at(&Kind::Fn) {
            self.bump().span
        } else {
            self.current().span
        };
        let (name, _) = self.ident()?;
        let type_params = self.generic_params()?;
        self.type_params = type_params.clone();
        self.expect(Kind::LParen)?;
        let mut params = Vec::new();
        if !self.at(&Kind::RParen) {
            loop {
                let (param, _) = self.ident()?;
                self.expect(Kind::Colon)?;
                let ty = self.ty()?;
                params.push((param, ty));
                if !self.at(&Kind::Comma) {
                    break;
                }
                self.bump();
            }
        }
        self.expect(Kind::RParen)?;
        self.expect(Kind::Arrow)?;
        let ret = self.ty()?;
        let body = self.block()?;
        self.type_params.clear();
        let span = start.join(body.span);
        Ok(Function {
            name,
            public: false,
            type_params,
            params,
            ret,
            body,
            span,
        })
    }

    fn block(&mut self) -> Result<Expr, Diagnostic> {
        let start = self.expect(Kind::LBrace)?.span;
        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.at(&Kind::RBrace) {
            if self.at(&Kind::Eof) {
                return Err(Diagnostic::new(
                    "E002",
                    self.current().span,
                    "unclosed block",
                ));
            }
            if self.at(&Kind::Let) || self.at(&Kind::Var) {
                let mutable = self.at(&Kind::Var);
                let first = self.bump().span;
                let (name, _) = self.ident()?;
                let ty = if self.at(&Kind::Colon) {
                    self.bump();
                    Some(self.ty()?)
                } else {
                    None
                };
                self.expect(Kind::Eq)?;
                let value = self.expr(0)?;
                let end = self.expect(Kind::Semicolon)?.span;
                stmts.push(Stmt::Let {
                    name,
                    ty,
                    value,
                    mutable,
                    span: first.join(end),
                });
            } else if self.at(&Kind::For) {
                let first = self.bump().span;
                let (name, _) = self.ident()?;
                self.expect(Kind::In)?;
                let iterable = self.expr(0)?;
                let body = self.loop_block()?;
                let span = first.join(body.span);
                stmts.push(Stmt::For {
                    name,
                    iterable,
                    body,
                    span,
                });
            } else if self.at(&Kind::While) {
                let first = self.bump().span;
                let condition = self.expr(0)?;
                let body = self.loop_block()?;
                let span = first.join(body.span);
                stmts.push(Stmt::While {
                    condition,
                    body,
                    span,
                });
            } else if matches!(self.current().kind, Kind::Ident(_)) && self.next_is(&Kind::Eq) {
                let (name, first) = self.ident()?;
                self.bump();
                let value = self.expr(0)?;
                let end = self.expect(Kind::Semicolon)?.span;
                stmts.push(Stmt::Assign {
                    name,
                    path: Vec::new(),
                    value,
                    span: first.join(end),
                });
            } else if self.at_push_statement() {
                let (name, first) = self.ident()?;
                self.expect(Kind::Dot)?;
                self.ident()?;
                self.expect(Kind::LParen)?;
                let value = self.expr(0)?;
                self.expect(Kind::RParen)?;
                let end = self.expect(Kind::Semicolon)?.span;
                stmts.push(Stmt::Push {
                    name,
                    value,
                    span: first.join(end),
                });
            } else if self.at(&Kind::Break) || self.at(&Kind::Continue) {
                let token = self.bump();
                if self.loop_depth == 0 {
                    return Err(Diagnostic::new(
                        "E002",
                        token.span,
                        "break and continue require a loop body",
                    ));
                }
                let end = self.expect(Kind::Semicolon)?.span;
                let span = token.span.join(end);
                stmts.push(if token.kind == Kind::Break {
                    Stmt::Break { span }
                } else {
                    Stmt::Continue { span }
                });
            } else if self.at(&Kind::Return) {
                let first = self.bump().span;
                let value = self.expr(0)?;
                let end = self.expect(Kind::Semicolon)?.span;
                stmts.push(Stmt::Return {
                    value,
                    span: first.join(end),
                });
            } else {
                let value = self.expr(0)?;
                if self.at(&Kind::Eq) {
                    let (name, path) = place(value)?;
                    self.bump();
                    let assigned = self.expr(0)?;
                    let end = self.expect(Kind::Semicolon)?.span;
                    let span = path
                        .first()
                        .map_or(end, |step| match step {
                            PlaceStep::Index(_, span) | PlaceStep::Field(_, span) => *span,
                        })
                        .join(end);
                    stmts.push(Stmt::Assign {
                        name,
                        path,
                        value: assigned,
                        span,
                    });
                } else if self.at(&Kind::Semicolon) {
                    self.bump();
                    stmts.push(Stmt::Expr(value));
                } else if !self.at(&Kind::RBrace) && value.is_block_like() {
                    // Like loops, block-like expression statements end at `}`.
                    stmts.push(Stmt::Expr(value));
                } else {
                    tail = Some(Box::new(value));
                    break;
                }
            }
        }
        let end = self.expect(Kind::RBrace)?.span;
        Ok(Expr {
            kind: ExprKind::Block(stmts, tail),
            span: start.join(end),
        })
    }

    fn loop_block(&mut self) -> Result<Expr, Diagnostic> {
        self.loop_depth += 1;
        let result = self.block();
        self.loop_depth -= 1;
        result
    }

    fn expr(&mut self, min_prec: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.atom()?;
        loop {
            if self.at(&Kind::Dot) {
                self.bump();
                let (field, end) = self.ident()?;
                let span = left.span.join(end);
                left = Expr {
                    kind: ExprKind::Field(Box::new(left), field),
                    span,
                };
                continue;
            }
            if self.at(&Kind::LBracket) {
                self.bump();
                let index = self.expr(0)?;
                let end = self.expect(Kind::RBracket)?.span;
                let span = left.span.join(end);
                left = Expr {
                    kind: ExprKind::Index(Box::new(left), Box::new(index)),
                    span,
                };
                continue;
            }
            if self.at(&Kind::LParen)
                && matches!(
                    left.kind,
                    ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::Apply(..)
                )
            {
                self.bump();
                let mut args = Vec::new();
                while !self.at(&Kind::RParen) {
                    args.push(self.expr(0)?);
                    if !self.at(&Kind::RParen) {
                        self.expect(Kind::Comma)?;
                    }
                }
                let end = self.bump().span;
                let span = left.span.join(end);
                left = Expr {
                    kind: ExprKind::Apply(Box::new(left), args),
                    span,
                };
                continue;
            }
            if self.at(&Kind::Question) {
                let end = self.bump().span;
                let span = left.span.join(end);
                left = Expr {
                    kind: ExprKind::Try(Box::new(left)),
                    span,
                };
                continue;
            }
            let (op, prec) = match self.current().kind {
                Kind::OrOr => (Op::Or, 0),
                Kind::AndAnd => (Op::And, 1),
                Kind::EqEq => (Op::Eq, 2),
                Kind::BangEq => (Op::Ne, 2),
                Kind::Lt => (Op::Lt, 3),
                Kind::Le => (Op::Le, 3),
                Kind::Gt => (Op::Gt, 3),
                Kind::Ge => (Op::Ge, 3),
                Kind::Plus => (Op::Add, 4),
                Kind::Minus => (Op::Sub, 4),
                Kind::Star => (Op::Mul, 5),
                Kind::Slash => (Op::Div, 5),
                Kind::Percent => (Op::Rem, 5),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let right = self.expr(prec + 1)?;
            let span = left.span.join(right.span);
            left = Expr {
                kind: ExprKind::Binary(Box::new(left), op, Box::new(right)),
                span,
            };
        }
        Ok(left)
    }

    fn call_args(&mut self) -> Result<(Vec<Expr>, Span), Diagnostic> {
        self.expect(Kind::LParen)?;
        let mut args = Vec::new();
        if !self.at(&Kind::RParen) {
            loop {
                args.push(self.expr(0)?);
                if !self.at(&Kind::Comma) {
                    break;
                }
                self.bump();
            }
        }
        let end = self.expect(Kind::RParen)?.span;
        Ok((args, end))
    }

    fn atom(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump();
        match token.kind {
            Kind::Pipe | Kind::OrOr => {
                let mut params = Vec::new();
                if token.kind == Kind::Pipe {
                    while !self.at(&Kind::Pipe) {
                        let (name, _) = self.ident()?;
                        let ty = if self.at(&Kind::Colon) {
                            self.bump();
                            Some(self.ty()?)
                        } else {
                            None
                        };
                        params.push((name, ty));
                        if !self.at(&Kind::Pipe) {
                            self.expect(Kind::Comma)?;
                        }
                    }
                    self.bump();
                }
                // `break` and `continue` cannot leave a lambda body.
                let loop_depth = std::mem::take(&mut self.loop_depth);
                let body = self.expr(0);
                self.loop_depth = loop_depth;
                let body = body?;
                Ok(Expr {
                    span: token.span.join(body.span),
                    kind: ExprKind::Lambda(params, Box::new(body)),
                })
            }
            Kind::Bang => {
                let value = self.expr(6)?;
                Ok(Expr {
                    span: token.span.join(value.span),
                    kind: ExprKind::Not(Box::new(value)),
                })
            }
            Kind::Minus => {
                if self.at(&Kind::Int(String::new())) {
                    let (number, span) = self.negative_literal(token.span)?;
                    Ok(Expr {
                        kind: ExprKind::Int(number),
                        span,
                    })
                } else if self.at(&Kind::Int64(String::new())) {
                    let (number, span) = self.negative_i64_literal(token.span)?;
                    Ok(Expr {
                        kind: ExprKind::I64(number),
                        span,
                    })
                } else if self.at(&Kind::Float64(String::new())) {
                    let number = self.bump();
                    let span = token.span.join(number.span);
                    if token.span.end != number.span.start {
                        return Err(Diagnostic::new(
                            "E002",
                            span,
                            "negative literal requires adjacent digits",
                        ));
                    }
                    let Kind::Float64(text) = number.kind else {
                        unreachable!()
                    };
                    let value = format!("-{text}")
                        .parse::<f64>()
                        .map_err(|_| Diagnostic::new("E003", span, "invalid f64 literal"))?;
                    if !value.is_finite() {
                        return Err(Diagnostic::new("E003", span, "f64 literal out of range"));
                    }
                    Ok(Expr {
                        kind: ExprKind::F64(value.to_bits()),
                        span,
                    })
                } else {
                    let value = self.expr(6)?;
                    Ok(Expr {
                        span: token.span.join(value.span),
                        kind: ExprKind::Neg(Box::new(value)),
                    })
                }
            }
            Kind::Int(value) => {
                let number = value
                    .parse::<i32>()
                    .map_err(|_| Diagnostic::new("E003", token.span, "i32 literal out of range"))?;
                Ok(Expr {
                    kind: ExprKind::Int(number),
                    span: token.span,
                })
            }
            Kind::Int64(value) => {
                let number = value
                    .parse::<i64>()
                    .map_err(|_| Diagnostic::new("E003", token.span, "i64 literal out of range"))?;
                Ok(Expr {
                    kind: ExprKind::I64(number),
                    span: token.span,
                })
            }
            Kind::Float64(value) => {
                let number = value
                    .parse::<f64>()
                    .map_err(|_| Diagnostic::new("E003", token.span, "invalid f64 literal"))?;
                if !number.is_finite() {
                    return Err(Diagnostic::new(
                        "E003",
                        token.span,
                        "f64 literal out of range",
                    ));
                }
                Ok(Expr {
                    kind: ExprKind::F64(number.to_bits()),
                    span: token.span,
                })
            }
            Kind::String(value) => Ok(Expr {
                kind: ExprKind::String(value),
                span: token.span,
            }),
            Kind::True | Kind::False => Ok(Expr {
                kind: ExprKind::Bool(token.kind == Kind::True),
                span: token.span,
            }),
            Kind::Ok | Kind::Err | Kind::Some => {
                self.expect(Kind::LParen)?;
                let inner = self.expr(0)?;
                let end = self.expect(Kind::RParen)?.span;
                let kind = match token.kind {
                    Kind::Ok => ExprKind::Ok(Box::new(inner)),
                    Kind::Err => ExprKind::Err(Box::new(inner)),
                    Kind::Some => ExprKind::Some(Box::new(inner)),
                    _ => unreachable!(),
                };
                Ok(Expr {
                    kind,
                    span: token.span.join(end),
                })
            }
            Kind::None => Ok(Expr {
                kind: ExprKind::None,
                span: token.span,
            }),
            Kind::Ident(name) => {
                if self.at(&Kind::ColonColon) {
                    self.bump();
                    let (segment, end) = self.ident()?;
                    if self.import_aliases.contains(&name) && !self.at(&Kind::ColonColon) {
                        let (args, end) = self.call_args()?;
                        Ok(Expr {
                            kind: ExprKind::Call(format!("{name}::{segment}"), args),
                            span: token.span.join(end),
                        })
                    } else {
                        let (enum_name, variant, end) = if self.import_aliases.contains(&name) {
                            self.bump();
                            let (variant, end) = self.ident()?;
                            (format!("{name}::{segment}"), variant, end)
                        } else {
                            (name, segment, end)
                        };
                        let (payload, end) = if self.at(&Kind::LParen) {
                            self.bump();
                            let payload = self.expr(0)?;
                            let end = self.expect(Kind::RParen)?.span;
                            (Some(Box::new(payload)), end)
                        } else {
                            (None, end)
                        };
                        Ok(Expr {
                            kind: ExprKind::Variant(enum_name, variant, payload),
                            span: token.span.join(end),
                        })
                    }
                } else if self.at(&Kind::LParen) {
                    let (args, end) = self.call_args()?;
                    Ok(Expr {
                        kind: ExprKind::Call(name, args),
                        span: token.span.join(end),
                    })
                } else {
                    Ok(Expr {
                        kind: ExprKind::Var(name),
                        span: token.span,
                    })
                }
            }
            Kind::LBracket => {
                let mut values = Vec::new();
                if !self.at(&Kind::RBracket) {
                    loop {
                        values.push(self.expr(0)?);
                        if !self.at(&Kind::Comma) {
                            break;
                        }
                        self.bump();
                    }
                }
                let end = self.expect(Kind::RBracket)?.span;
                Ok(Expr {
                    kind: ExprKind::Array(values),
                    span: token.span.join(end),
                })
            }
            Kind::LParen => {
                let mut value = self.expr(0)?;
                let end = self.expect(Kind::RParen)?.span;
                value.span = token.span.join(end);
                Ok(value)
            }
            Kind::LBrace => {
                self.pos -= 1;
                self.block()
            }
            Kind::If => {
                let condition = self.expr(0)?;
                let yes = self.block()?;
                let no = if !self.at(&Kind::Else) {
                    // A missing `else` is an empty block; the checker then
                    // requires the `yes` branch to have type `Unit`.
                    Expr {
                        kind: ExprKind::Block(Vec::new(), None),
                        span: Span::in_source(yes.span.source_id, yes.span.end, yes.span.end),
                    }
                } else if self.bump().kind == Kind::Else && self.at(&Kind::If) {
                    self.atom()?
                } else {
                    self.block()?
                };
                let span = token.span.join(no.span);
                Ok(Expr {
                    kind: ExprKind::If(Box::new(condition), Box::new(yes), Box::new(no)),
                    span,
                })
            }
            Kind::Match => {
                let value = self.expr(0)?;
                self.expect(Kind::LBrace)?;
                let mut arms = Vec::new();
                while !self.at(&Kind::RBrace) {
                    let pattern = self.pattern()?;
                    self.expect(Kind::FatArrow)?;
                    let body = self.expr(0)?;
                    arms.push((pattern, body));
                    if !self.at(&Kind::Comma) {
                        break;
                    }
                    self.bump();
                }
                let end = self.expect(Kind::RBrace)?.span;
                Ok(Expr {
                    kind: ExprKind::Match(Box::new(value), arms),
                    span: token.span.join(end),
                })
            }
            Kind::Spawn => {
                let call = self.atom()?;
                if !matches!(call.kind, ExprKind::Call(_, _)) {
                    return Err(Diagnostic::new(
                        "E002",
                        call.span,
                        "spawn requires a named function call",
                    ));
                }
                Ok(Expr {
                    span: token.span.join(call.span),
                    kind: ExprKind::Spawn(Box::new(call)),
                })
            }
            _ => Err(Diagnostic::new("E002", token.span, "expected expression")),
        }
    }

    fn pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.bump();
        let (kind, span) = match token.kind {
            Kind::Minus => {
                if self.at(&Kind::Int64(String::new())) {
                    let (value, span) = self.negative_i64_literal(token.span)?;
                    (PatternKind::I64(value), span)
                } else {
                    let (value, span) = self.negative_literal(token.span)?;
                    (PatternKind::Int(value), span)
                }
            }
            Kind::Int(value) => (
                PatternKind::Int(value.parse::<i32>().map_err(|_| {
                    Diagnostic::new("E003", token.span, "integer literal outside i32 range")
                })?),
                token.span,
            ),
            Kind::Int64(value) => (
                PatternKind::I64(value.parse::<i64>().map_err(|_| {
                    Diagnostic::new("E003", token.span, "integer literal outside i64 range")
                })?),
                token.span,
            ),
            Kind::String(value) => (PatternKind::String(value), token.span),
            Kind::True => (PatternKind::Bool(true), token.span),
            Kind::False => (PatternKind::Bool(false), token.span),
            Kind::Ok | Kind::Err | Kind::Some => {
                self.expect(Kind::LParen)?;
                let (name, _) = self.ident()?;
                let end = self.expect(Kind::RParen)?.span;
                let kind = match token.kind {
                    Kind::Ok => PatternKind::Ok(name),
                    Kind::Err => PatternKind::Err(name),
                    Kind::Some => PatternKind::Some(name),
                    _ => unreachable!(),
                };
                (kind, token.span.join(end))
            }
            Kind::None => (PatternKind::None, token.span),
            Kind::Ident(name) if name == "_" => (PatternKind::Wildcard, token.span),
            Kind::Ident(name) => {
                self.expect(Kind::ColonColon)?;
                let (segment, end) = self.ident()?;
                let (enum_name, variant, end) = if self.import_aliases.contains(&name) {
                    self.expect(Kind::ColonColon)?;
                    let (variant, end) = self.ident()?;
                    (format!("{name}::{segment}"), variant, end)
                } else {
                    (name, segment, end)
                };
                let (binding, end) = if self.at(&Kind::LParen) {
                    self.bump();
                    let (binding, _) = self.ident()?;
                    let end = self.expect(Kind::RParen)?.span;
                    (Some(binding), end)
                } else {
                    (None, end)
                };
                (
                    PatternKind::Variant(enum_name, variant, binding),
                    token.span.join(end),
                )
            }
            _ => {
                return Err(Diagnostic::new(
                    "E002",
                    token.span,
                    "expected match pattern",
                ));
            }
        };
        Ok(Pattern { kind, span })
    }
}

/// Convert a parsed `name[index].field...` expression into an assignment target.
fn place(expr: Expr) -> Result<(String, Vec<PlaceStep>), Diagnostic> {
    match expr.kind {
        ExprKind::Var(name) => Ok((name, Vec::new())),
        ExprKind::Index(base, index) => {
            let (name, mut path) = place(*base)?;
            path.push(PlaceStep::Index(*index, expr.span));
            Ok((name, path))
        }
        ExprKind::Field(base, field) => {
            let (name, mut path) = place(*base)?;
            path.push(PlaceStep::Field(field, expr.span));
            Ok((name, path))
        }
        _ => Err(Diagnostic::new(
            "E002",
            expr.span,
            "assignment target must be a local name, element, or field",
        )),
    }
}

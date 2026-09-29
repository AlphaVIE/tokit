use crate::ast::{Expr, ExprKind, Function, Op, Program, Span, Stmt, Type};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Kind, Token};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
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
    fn ty(&mut self) -> Result<Type, Diagnostic> {
        let (name, span) = self.ident()?;
        match name.as_str() {
            "i32" => Ok(Type::I32),
            "bool" => Ok(Type::Bool),
            "Unit" => Ok(Type::Unit),
            _ => Err(Diagnostic::new(
                "E002",
                span,
                format!("unknown type {name}"),
            )),
        }
    }

    pub fn program(&mut self) -> Result<Program, Diagnostic> {
        let mut functions = Vec::new();
        while !self.at(&Kind::Eof) {
            functions.push(self.function()?);
        }
        if functions.is_empty() {
            return Err(Diagnostic::new(
                "E002",
                self.current().span,
                "expected a function",
            ));
        }
        Ok(Program { functions })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = self.expect(Kind::Fn)?.span;
        let (name, _) = self.ident()?;
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
        let span = start.join(body.span);
        Ok(Function {
            name,
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
            if self.at(&Kind::Let) {
                let first = self.bump().span;
                let (name, _) = self.ident()?;
                self.expect(Kind::Colon)?;
                let ty = self.ty()?;
                self.expect(Kind::Eq)?;
                let value = self.expr(0)?;
                let end = self.expect(Kind::Semicolon)?.span;
                stmts.push(Stmt::Let {
                    name,
                    ty,
                    value,
                    span: first.join(end),
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
                if self.at(&Kind::Semicolon) {
                    self.bump();
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

    fn expr(&mut self, min_prec: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.atom()?;
        loop {
            let (op, prec) = match self.current().kind {
                Kind::EqEq => (Op::Eq, 1),
                Kind::BangEq => (Op::Ne, 1),
                Kind::Lt => (Op::Lt, 2),
                Kind::Le => (Op::Le, 2),
                Kind::Gt => (Op::Gt, 2),
                Kind::Ge => (Op::Ge, 2),
                Kind::Plus => (Op::Add, 3),
                Kind::Minus => (Op::Sub, 3),
                Kind::Star => (Op::Mul, 4),
                Kind::Slash => (Op::Div, 4),
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

    fn atom(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.bump();
        match token.kind {
            Kind::Int(value) => {
                let number = value
                    .parse::<i32>()
                    .map_err(|_| Diagnostic::new("E003", token.span, "i32 literal out of range"))?;
                Ok(Expr {
                    kind: ExprKind::Int(number),
                    span: token.span,
                })
            }
            Kind::True | Kind::False => Ok(Expr {
                kind: ExprKind::Bool(token.kind == Kind::True),
                span: token.span,
            }),
            Kind::Ident(name) => {
                if self.at(&Kind::LParen) {
                    self.bump();
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
                self.expect(Kind::Else)?;
                let no = if self.at(&Kind::If) {
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
            _ => Err(Diagnostic::new("E002", token.span, "expected expression")),
        }
    }
}

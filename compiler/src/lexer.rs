use crate::ast::{SourceId, Span};
use crate::diagnostic::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Ident(String),
    Int(String),
    String(String),
    Fn,
    Struct,
    Enum,
    Import,
    Pub,
    Let,
    Var,
    For,
    While,
    Break,
    Continue,
    In,
    If,
    Match,
    Spawn,
    Else,
    Return,
    True,
    False,
    Ok,
    Err,
    Some,
    None,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    ColonColon,
    Comma,
    Semicolon,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    EqEq,
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    Question,
    Dot,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    lex_in_source(source, SourceId::default())
}

pub fn lex_in_source(source: &str, source_id: SourceId) -> Result<Vec<Token>, Diagnostic> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        let kind = if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            match &source[start..i] {
                "fn" => Kind::Fn,
                "struct" => Kind::Struct,
                "enum" => Kind::Enum,
                "import" => Kind::Import,
                "pub" => Kind::Pub,
                "let" => Kind::Let,
                "var" => Kind::Var,
                "for" => Kind::For,
                "while" => Kind::While,
                "break" => Kind::Break,
                "continue" => Kind::Continue,
                "in" => Kind::In,
                "if" => Kind::If,
                "match" => Kind::Match,
                "spawn" => Kind::Spawn,
                "else" => Kind::Else,
                "return" => Kind::Return,
                "true" => Kind::True,
                "false" => Kind::False,
                "Ok" => Kind::Ok,
                "Err" => Kind::Err,
                "Some" => Kind::Some,
                "None" => Kind::None,
                name => Kind::Ident(name.to_owned()),
            }
        } else if bytes[i] == b'"' {
            i += 1;
            let mut value = String::new();
            loop {
                if i >= bytes.len() || bytes[i] == b'\n' || bytes[i] == b'\r' {
                    return Err(Diagnostic::new(
                        "E004",
                        Span::in_source(source_id, start, i),
                        "unclosed string literal",
                    ));
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                if bytes[i] == b'\\' {
                    i += 1;
                    let escaped = match bytes.get(i) {
                        Some(b'n') => '\n',
                        Some(b't') => '\t',
                        Some(b'r') => '\r',
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        _ => {
                            return Err(Diagnostic::new(
                                "E004",
                                Span::in_source(
                                    source_id,
                                    start,
                                    i.saturating_add(1).min(bytes.len()),
                                ),
                                "invalid string escape",
                            ));
                        }
                    };
                    value.push(escaped);
                    i += 1;
                } else {
                    let character = source[i..].chars().next().expect("valid UTF-8 character");
                    value.push(character);
                    i += character.len_utf8();
                }
            }
            Kind::String(value)
        } else if bytes[i].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            Kind::Int(source[start..i].to_owned())
        } else {
            i += 1;
            match bytes[start] {
                b'(' => Kind::LParen,
                b')' => Kind::RParen,
                b'{' => Kind::LBrace,
                b'}' => Kind::RBrace,
                b'[' => Kind::LBracket,
                b']' => Kind::RBracket,
                b':' if bytes.get(i) == Some(&b':') => {
                    i += 1;
                    Kind::ColonColon
                }
                b':' => Kind::Colon,
                b',' => Kind::Comma,
                b';' => Kind::Semicolon,
                b'+' => Kind::Plus,
                b'*' => Kind::Star,
                b'/' => Kind::Slash,
                b'-' if bytes.get(i) == Some(&b'>') => {
                    i += 1;
                    Kind::Arrow
                }
                b'-' => Kind::Minus,
                b'=' if bytes.get(i) == Some(&b'>') => {
                    i += 1;
                    Kind::FatArrow
                }
                b'=' if bytes.get(i) == Some(&b'=') => {
                    i += 1;
                    Kind::EqEq
                }
                b'=' => Kind::Eq,
                b'!' if bytes.get(i) == Some(&b'=') => {
                    i += 1;
                    Kind::BangEq
                }
                b'<' if bytes.get(i) == Some(&b'=') => {
                    i += 1;
                    Kind::Le
                }
                b'<' => Kind::Lt,
                b'>' if bytes.get(i) == Some(&b'=') => {
                    i += 1;
                    Kind::Ge
                }
                b'>' => Kind::Gt,
                b'?' => Kind::Question,
                b'.' => Kind::Dot,
                _ => {
                    return Err(Diagnostic::new(
                        "E001",
                        Span::in_source(source_id, start, i),
                        "invalid character",
                    ));
                }
            }
        };
        tokens.push(Token {
            kind,
            span: Span::in_source(source_id, start, i),
        });
    }
    tokens.push(Token {
        kind: Kind::Eof,
        span: Span::in_source(source_id, i, i),
    });
    Ok(tokens)
}

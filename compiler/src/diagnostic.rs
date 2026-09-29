use crate::ast::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            span,
            message: message.into(),
        }
    }

    pub fn display(&self, source: &str) -> String {
        let (line, column) = self.location(source);
        format!("{}@{}:{} {}", self.code, line, column, self.message)
    }

    fn location(&self, source: &str) -> (usize, usize) {
        let before = &source[..self.span.start.min(source.len())];
        let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        (line, column)
    }

    pub fn json(&self, source: &str) -> String {
        let (line, column) = self.location(source);
        format!(
            "{{\"code\":\"{}\",\"span\":{{\"start\":{},\"end\":{}}},\"line\":{},\"column\":{},\"message\":\"{}\"}}",
            self.code,
            self.span.start,
            self.span.end,
            line,
            column,
            escape_json(&self.message)
        )
    }
}

pub fn escape_json(input: &str) -> String {
    let mut output = String::new();
    for ch in input.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch <= '\u{001f}' => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output
}

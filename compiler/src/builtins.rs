use crate::ast::{EnumDecl, EnumVariant, Span, Type};

pub const READ_TEXT: &str = "read_text";
pub const WRITE_TEXT: &str = "write_text";
pub const LINES: &str = "lines";
pub const ARGS: &str = "args";
pub const LEN: &str = "len";
pub const PARSE_I32: &str = "parse_i32";
pub const UTF8_BYTES: &str = "utf8_bytes";
pub const UTF8_DECODE: &str = "utf8_decode";
pub const IO_ERROR: &str = "IoError";
pub const PARSE_ERROR: &str = "ParseError";
pub const TASK_ERROR: &str = "TaskError";
pub const JOIN: &str = "join";
pub const IO_ERROR_VARIANTS: [&str; 4] = ["Denied", "NotFound", "InvalidUtf8", "Other"];

pub fn io_error_decl() -> EnumDecl {
    EnumDecl {
        name: IO_ERROR.to_owned(),
        public: true,
        variants: IO_ERROR_VARIANTS
            .iter()
            .map(|name| EnumVariant {
                name: (*name).to_owned(),
                payload: None,
            })
            .collect(),
        span: Span::new(0, 0),
    }
}

pub fn read_text_result() -> Type {
    Type::Result(
        Box::new(Type::String),
        Box::new(Type::Named(IO_ERROR.to_owned())),
    )
}

pub fn write_text_result() -> Type {
    Type::Result(
        Box::new(Type::Unit),
        Box::new(Type::Named(IO_ERROR.to_owned())),
    )
}

pub fn task_error_decl() -> EnumDecl {
    EnumDecl {
        name: TASK_ERROR.to_owned(),
        public: true,
        variants: vec![EnumVariant {
            name: "Failed".to_owned(),
            payload: None,
        }],
        span: Span::new(0, 0),
    }
}

pub fn parse_error_decl() -> EnumDecl {
    EnumDecl {
        name: PARSE_ERROR.to_owned(),
        public: true,
        variants: ["Invalid", "OutOfRange"]
            .into_iter()
            .map(|name| EnumVariant {
                name: name.to_owned(),
                payload: None,
            })
            .collect(),
        span: Span::new(0, 0),
    }
}

pub fn parse_i32_result() -> Type {
    Type::Result(
        Box::new(Type::I32),
        Box::new(Type::Named(PARSE_ERROR.to_owned())),
    )
}

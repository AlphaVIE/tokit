use crate::ast::{EnumDecl, EnumVariant, Span, Type};

pub const READ_TEXT: &str = "read_text";
pub const READ_BYTES: &str = "read_bytes";
pub const WRITE_TEXT: &str = "write_text";
pub const WRITE_BYTES: &str = "write_bytes";
pub const LINES: &str = "lines";
pub const ARGS: &str = "args";
pub const PRINT: &str = "print";
pub const READ_LINE: &str = "read_line";
pub const READ_STDIN: &str = "read_stdin";
pub const EXIT: &str = "exit";
pub const LEN: &str = "len";
pub const PARSE_I32: &str = "parse_i32";
pub const PARSE_I64: &str = "parse_i64";
pub const WIDEN_I64: &str = "i64";
pub const NARROW_I32: &str = "i32";
pub const TO_F64: &str = "f64";
pub const PARSE_F64: &str = "parse_f64";
pub const UTF8_BYTES: &str = "utf8_bytes";
pub const UTF8_DECODE: &str = "utf8_decode";
pub const UTF8_ENCODE: &str = "utf8_encode";
pub const UTF8_DECODE_BYTES: &str = "utf8_decode_bytes";
pub const BYTES_FROM_I32: &str = "bytes_from_i32";
pub const BYTES_TO_I32: &str = "bytes_to_i32";
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

pub fn read_bytes_result() -> Type {
    Type::Result(
        Box::new(Type::Bytes),
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

pub fn parse_f64_result() -> Type {
    Type::Result(
        Box::new(Type::F64),
        Box::new(Type::Named(PARSE_ERROR.to_owned())),
    )
}

pub fn parse_i64_result() -> Type {
    Type::Result(
        Box::new(Type::I64),
        Box::new(Type::Named(PARSE_ERROR.to_owned())),
    )
}

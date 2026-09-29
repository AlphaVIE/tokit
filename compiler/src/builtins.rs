use crate::ast::{EnumDecl, EnumVariant, Span, Type};

pub const READ_TEXT: &str = "read_text";
pub const LINES: &str = "lines";
pub const IO_ERROR: &str = "IoError";
pub const IO_ERROR_VARIANTS: [&str; 4] = ["Denied", "NotFound", "InvalidUtf8", "Other"];

pub fn io_error_decl() -> EnumDecl {
    EnumDecl {
        name: IO_ERROR.to_owned(),
        variants: IO_ERROR_VARIANTS
            .iter()
            .map(|name| EnumVariant {
                name: (*name).to_owned(),
                payload: None,
            })
            .collect(),
        span: Span { start: 0, end: 0 },
    }
}

pub fn read_text_result() -> Type {
    Type::Result(
        Box::new(Type::String),
        Box::new(Type::Named(IO_ERROR.to_owned())),
    )
}

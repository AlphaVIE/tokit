use crate::ast::{EnumDecl, EnumVariant, Span, Type};

pub const READ_TEXT: &str = "read_text";
pub const READ_BYTES: &str = "read_bytes";
pub const WRITE_TEXT: &str = "write_text";
pub const WRITE_BYTES: &str = "write_bytes";
pub const LINES: &str = "lines";
pub const ARGS: &str = "args";
pub const PRINT: &str = "print";
pub const SERVE: &str = "serve";
pub const HTTP_REQUEST: &str = "http_request";
pub const REQUEST: &str = "Request";
pub const RESPONSE: &str = "Response";
pub const LIST_DIR: &str = "list_dir";
pub const EXISTS: &str = "exists";
pub const MAKE_DIR: &str = "make_dir";
pub const REMOVE_FILE: &str = "remove_file";
pub const ENV: &str = "env";
pub const NOW_MS: &str = "now_ms";
pub const CLOCK_NS: &str = "clock_ns";
pub const SLEEP_MS: &str = "sleep_ms";
pub const ABS: &str = "abs";
pub const MIN: &str = "min";
pub const MAX: &str = "max";
pub const POW: &str = "pow";
pub const SQRT: &str = "sqrt";
pub const FLOOR: &str = "floor";
pub const CEIL: &str = "ceil";
pub const ROUND: &str = "round";
pub const EXP: &str = "exp";
pub const LN: &str = "ln";
pub const SIN: &str = "sin";
pub const COS: &str = "cos";
pub const TAN: &str = "tan";
pub const ATAN2: &str = "atan2";
pub const PI: &str = "pi";
pub const MAP: &str = "Map";
pub const MAP_FN: &str = "map";
pub const FILTER: &str = "filter";
pub const ANY: &str = "any";
pub const ALL: &str = "all";
pub const FOLD: &str = "fold";
pub const SORT_BY: &str = "sort_by";
pub const RANGE: &str = "range";
pub const SORT: &str = "sort";
pub const REVERSE: &str = "reverse";
pub const SLICE: &str = "slice";
pub const GET: &str = "get";
pub const GET_OR: &str = "get_or";
pub const KEYS: &str = "keys";
pub const VALUES: &str = "values";
pub const REMOVE: &str = "remove";
pub const TO_STRING: &str = "String";
pub const CHARS: &str = "chars";
pub const SPLIT: &str = "split";
pub const TRIM: &str = "trim";
pub const CONTAINS: &str = "contains";
pub const STARTS_WITH: &str = "starts_with";
pub const ENDS_WITH: &str = "ends_with";
pub const REPLACE: &str = "replace";
pub const LOWER: &str = "lower";
pub const UPPER: &str = "upper";
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
        type_params: Vec::new(),
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
        type_params: Vec::new(),
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
        type_params: Vec::new(),
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

pub fn map_type(key: Type, value: Type) -> Type {
    Type::Applied(MAP.to_owned(), vec![key, value])
}

/// Map keys need a total order shared by both backends.
pub fn map_key(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I32 | Type::I64 | Type::String | Type::Bool | Type::Never
    )
}

/// Element types with a total order shared by both backends.
pub fn orderable(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I32 | Type::I64 | Type::F64 | Type::String | Type::Bool | Type::Never
    )
}

/// Element types whose `==` is defined.
pub fn equatable(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I32 | Type::I64 | Type::F64 | Type::Bool | Type::String | Type::Bytes | Type::Never
    )
}

/// Every builtin that is invoked with call syntax.
pub fn is_call(name: &str) -> bool {
    matches!(
        name,
        "serve"
            | "http_request"
            | "Request"
            | "Response"
            | "list_dir"
            | "exists"
            | "make_dir"
            | "remove_file"
            | "env"
            | "now_ms"
            | "clock_ns"
            | "sleep_ms"
            | "abs"
            | "min"
            | "max"
            | "pow"
            | "sqrt"
            | "floor"
            | "ceil"
            | "round"
            | "exp"
            | "ln"
            | "sin"
            | "cos"
            | "tan"
            | "atan2"
            | "pi"
            | "read_text"
            | "read_bytes"
            | "write_text"
            | "write_bytes"
            | "lines"
            | "args"
            | "print"
            | "map"
            | "filter"
            | "any"
            | "all"
            | "fold"
            | "sort_by"
            | "range"
            | "sort"
            | "reverse"
            | "slice"
            | "Map"
            | "get"
            | "get_or"
            | "keys"
            | "values"
            | "remove"
            | "String"
            | "chars"
            | "split"
            | "trim"
            | "contains"
            | "starts_with"
            | "ends_with"
            | "replace"
            | "lower"
            | "upper"
            | "read_line"
            | "read_stdin"
            | "exit"
            | "len"
            | "parse_i32"
            | "parse_i64"
            | "parse_f64"
            | "i32"
            | "i64"
            | "f64"
            | "utf8_bytes"
            | "utf8_decode"
            | "utf8_encode"
            | "utf8_decode_bytes"
            | "bytes_from_i32"
            | "bytes_to_i32"
            | "join"
    )
}

/// The built-in HTTP message records, in constructor field order.
pub fn http_records() -> Vec<crate::ast::Record> {
    let headers = map_type(Type::String, Type::String);
    [
        (
            REQUEST,
            vec![
                ("method", Type::String),
                ("path", Type::String),
                ("query", Type::String),
                ("headers", headers.clone()),
                ("body", Type::String),
            ],
        ),
        (
            RESPONSE,
            vec![
                ("status", Type::I32),
                ("headers", headers),
                ("body", Type::String),
            ],
        ),
    ]
    .into_iter()
    .map(|(name, fields)| crate::ast::Record {
        name: name.to_owned(),
        public: true,
        type_params: Vec::new(),
        fields: fields
            .into_iter()
            .map(|(field, ty)| (field.to_owned(), ty))
            .collect(),
        span: Span::new(0, 0),
    })
    .collect()
}

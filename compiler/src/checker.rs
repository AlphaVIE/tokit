use std::collections::{HashMap, HashSet};

use crate::ast::{
    EnumVariant, Expr, ExprKind, Op, Pattern, PatternKind, PlaceStep, Program, Span, Stmt, Type,
};
use crate::builtins;
use crate::diagnostic::Diagnostic;

#[derive(Clone)]
struct Signature {
    type_params: Vec<String>,
    params: Vec<Type>,
    ret: Type,
    fields: Option<Vec<(String, Type)>>,
    variants: Option<Vec<EnumVariant>>,
    spawn_safe: bool,
}
#[derive(Clone)]
struct Binding {
    ty: Type,
    mutable: bool,
}

pub fn check(program: &Program) -> Result<(), Diagnostic> {
    check_with_types(program).map(|_| ())
}

pub fn check_with_types(program: &Program) -> Result<HashMap<Span, Type>, Diagnostic> {
    if let Some(import) = program.imports.first() {
        return Err(Diagnostic::new(
            "E118",
            import.span,
            "imports require a file-backed program loader",
        ));
    }
    let mut signatures = HashMap::new();
    let mut types = HashMap::new();
    let mut record_names = HashSet::new();
    let mut arities = HashMap::new();
    for builtin in [
        builtins::io_error_decl(),
        builtins::task_error_decl(),
        builtins::parse_error_decl(),
    ] {
        record_names.insert(builtin.name.clone());
        arities.insert(builtin.name.clone(), 0);
        signatures.insert(
            builtin.name.clone(),
            Signature {
                type_params: Vec::new(),
                params: Vec::new(),
                ret: Type::Named(builtin.name),
                fields: None,
                variants: Some(builtin.variants),
                spawn_safe: false,
            },
        );
    }
    arities.insert(builtins::MAP.to_owned(), 2);
    record_names.insert(builtins::MAP.to_owned());
    arities.insert(builtins::CONN.to_owned(), 0);
    arities.insert(builtins::LISTENER.to_owned(), 0);
    record_names.insert(builtins::CONN.to_owned());
    record_names.insert(builtins::LISTENER.to_owned());
    for record in builtins::http_records() {
        record_names.insert(record.name.clone());
        arities.insert(record.name.clone(), 0);
        signatures.insert(
            record.name.clone(),
            Signature {
                type_params: Vec::new(),
                params: record.fields.iter().map(|(_, ty)| ty.clone()).collect(),
                ret: Type::Named(record.name.clone()),
                fields: Some(record.fields),
                variants: None,
                spawn_safe: false,
            },
        );
    }
    for (name, params, ret) in [
        (
            builtins::READ_TEXT,
            vec![Type::String],
            builtins::read_text_result(),
        ),
        (
            builtins::READ_BYTES,
            vec![Type::String],
            builtins::read_bytes_result(),
        ),
        (
            builtins::WRITE_TEXT,
            vec![Type::String, Type::String],
            builtins::write_text_result(),
        ),
        (
            builtins::WRITE_BYTES,
            vec![Type::String, Type::Bytes],
            builtins::write_text_result(),
        ),
        (
            builtins::LINES,
            vec![Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (
            builtins::ARGS,
            Vec::new(),
            Type::Array(Box::new(Type::String)),
        ),
        (builtins::PRINT, vec![Type::String], Type::Unit),
        (builtins::BIT_AND, vec![Type::I32, Type::I32], Type::I32),
        (builtins::BIT_OR, vec![Type::I32, Type::I32], Type::I32),
        (builtins::BIT_XOR, vec![Type::I32, Type::I32], Type::I32),
        (builtins::BIT_NOT, vec![Type::I32], Type::I32),
        (builtins::SHL, vec![Type::I32, Type::I32], Type::I32),
        (builtins::SHR, vec![Type::I32, Type::I32], Type::I32),
        (builtins::SHA256, vec![Type::Bytes], Type::Bytes),
        (builtins::MD5, vec![Type::Bytes], Type::Bytes),
        (builtins::SHA1, vec![Type::Bytes], Type::Bytes),
        (
            builtins::HMAC_SHA256,
            vec![Type::Bytes, Type::Bytes],
            Type::Bytes,
        ),
        (
            builtins::PBKDF2_SHA256,
            vec![Type::Bytes, Type::Bytes, Type::I32],
            Type::Bytes,
        ),
        (builtins::BASE64_ENCODE, vec![Type::Bytes], Type::String),
        (
            builtins::BASE64_DECODE,
            vec![Type::String],
            Type::Option(Box::new(Type::Bytes)),
        ),
        (builtins::HEX, vec![Type::Bytes], Type::String),
        (builtins::RANDOM_BYTES, vec![Type::I32], Type::Bytes),
        (
            builtins::TCP_CONNECT,
            vec![Type::String],
            Type::Result(
                Box::new(Type::Named(builtins::CONN.to_owned())),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::TCP_SEND,
            vec![Type::Named(builtins::CONN.to_owned()), Type::Bytes],
            builtins::write_text_result(),
        ),
        (
            builtins::TCP_RECV,
            vec![Type::Named(builtins::CONN.to_owned()), Type::I32],
            Type::Result(
                Box::new(Type::Bytes),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::TCP_CLOSE,
            vec![Type::Named(builtins::CONN.to_owned())],
            Type::Unit,
        ),
        (
            builtins::LISTEN,
            vec![Type::String],
            Type::Result(
                Box::new(Type::Named(builtins::LISTENER.to_owned())),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::ACCEPT,
            vec![Type::Named(builtins::LISTENER.to_owned())],
            Type::Result(
                Box::new(Type::Named(builtins::CONN.to_owned())),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::HTTP_READ,
            vec![Type::Named(builtins::CONN.to_owned())],
            Type::Result(
                Box::new(Type::Named(builtins::REQUEST.to_owned())),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::HTTP_WRITE,
            vec![
                Type::Named(builtins::CONN.to_owned()),
                Type::Named(builtins::RESPONSE.to_owned()),
            ],
            builtins::write_text_result(),
        ),
        (
            builtins::SERVE,
            vec![
                Type::String,
                Type::I32,
                Type::Fn(
                    vec![Type::Named(builtins::REQUEST.to_owned())],
                    Box::new(Type::Named(builtins::RESPONSE.to_owned())),
                ),
            ],
            builtins::write_text_result(),
        ),
        (
            builtins::HTTP_REQUEST,
            vec![
                Type::String,
                Type::String,
                builtins::map_type(Type::String, Type::String),
                Type::String,
            ],
            Type::Result(
                Box::new(Type::Named(builtins::RESPONSE.to_owned())),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (
            builtins::LIST_DIR,
            vec![Type::String],
            Type::Result(
                Box::new(Type::Array(Box::new(Type::String))),
                Box::new(Type::Named(builtins::IO_ERROR.to_owned())),
            ),
        ),
        (builtins::EXISTS, vec![Type::String], Type::Bool),
        (
            builtins::MAKE_DIR,
            vec![Type::String],
            builtins::write_text_result(),
        ),
        (
            builtins::REMOVE_FILE,
            vec![Type::String],
            builtins::write_text_result(),
        ),
        (
            builtins::ENV,
            vec![Type::String],
            Type::Option(Box::new(Type::String)),
        ),
        (builtins::NOW_MS, Vec::new(), Type::I64),
        (builtins::CLOCK_NS, Vec::new(), Type::I64),
        (builtins::SLEEP_MS, vec![Type::I64], Type::Unit),
        (builtins::ABS, vec![Type::I32], Type::I32),
        (builtins::MIN, vec![Type::I32, Type::I32], Type::I32),
        (builtins::MAX, vec![Type::I32, Type::I32], Type::I32),
        (builtins::POW, vec![Type::I32, Type::I32], Type::I32),
        (builtins::SQRT, vec![Type::F64], Type::F64),
        (builtins::FLOOR, vec![Type::F64], Type::F64),
        (builtins::CEIL, vec![Type::F64], Type::F64),
        (builtins::ROUND, vec![Type::F64], Type::F64),
        (builtins::EXP, vec![Type::F64], Type::F64),
        (builtins::LN, vec![Type::F64], Type::F64),
        (builtins::SIN, vec![Type::F64], Type::F64),
        (builtins::COS, vec![Type::F64], Type::F64),
        (builtins::TAN, vec![Type::F64], Type::F64),
        (builtins::ATAN2, vec![Type::F64, Type::F64], Type::F64),
        (builtins::PI, Vec::new(), Type::F64),
        (
            builtins::RANGE,
            vec![Type::I32, Type::I32],
            Type::Array(Box::new(Type::I32)),
        ),
        (builtins::TO_STRING, vec![Type::I32], Type::String),
        (
            builtins::CHARS,
            vec![Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (
            builtins::SPLIT,
            vec![Type::String, Type::String],
            Type::Array(Box::new(Type::String)),
        ),
        (builtins::TRIM, vec![Type::String], Type::String),
        (
            builtins::CONTAINS,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::STARTS_WITH,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::ENDS_WITH,
            vec![Type::String, Type::String],
            Type::Bool,
        ),
        (
            builtins::REPLACE,
            vec![Type::String, Type::String, Type::String],
            Type::String,
        ),
        (builtins::LOWER, vec![Type::String], Type::String),
        (builtins::UPPER, vec![Type::String], Type::String),
        (
            builtins::READ_LINE,
            Vec::new(),
            Type::Option(Box::new(Type::String)),
        ),
        (
            builtins::READ_STDIN,
            Vec::new(),
            builtins::read_text_result(),
        ),
        (builtins::EXIT, vec![Type::I32], Type::Never),
        (
            builtins::PARSE_I32,
            vec![Type::String],
            builtins::parse_i32_result(),
        ),
        (
            builtins::PARSE_I64,
            vec![Type::String],
            builtins::parse_i64_result(),
        ),
        (
            builtins::PARSE_F64,
            vec![Type::String],
            builtins::parse_f64_result(),
        ),
        (builtins::WIDEN_I64, vec![Type::I32], Type::I64),
        (builtins::TO_F64, vec![Type::I32], Type::F64),
        (
            builtins::NARROW_I32,
            vec![Type::I64],
            Type::Option(Box::new(Type::I32)),
        ),
        (
            builtins::UTF8_BYTES,
            vec![Type::String],
            Type::Array(Box::new(Type::I32)),
        ),
        (
            builtins::UTF8_DECODE,
            vec![Type::Array(Box::new(Type::I32))],
            Type::Option(Box::new(Type::String)),
        ),
        (builtins::UTF8_ENCODE, vec![Type::String], Type::Bytes),
        (
            builtins::UTF8_DECODE_BYTES,
            vec![Type::Bytes],
            Type::Option(Box::new(Type::String)),
        ),
        (
            builtins::BYTES_FROM_I32,
            vec![Type::Array(Box::new(Type::I32))],
            Type::Option(Box::new(Type::Bytes)),
        ),
        (
            builtins::BYTES_TO_I32,
            vec![Type::Bytes],
            Type::Array(Box::new(Type::I32)),
        ),
    ] {
        signatures.insert(
            name.to_owned(),
            Signature {
                type_params: Vec::new(),
                params,
                ret,
                fields: None,
                variants: None,
                spawn_safe: false,
            },
        );
    }
    signatures.insert(
        builtins::LEN.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Array(Box::new(Type::Param("T".to_owned())))],
            ret: Type::I32,
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::GET.to_owned(),
        Signature {
            type_params: vec!["K".to_owned(), "V".to_owned()],
            params: vec![
                builtins::map_type(Type::Param("K".to_owned()), Type::Param("V".to_owned())),
                Type::Param("K".to_owned()),
            ],
            ret: Type::Option(Box::new(Type::Param("V".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::GET_OR.to_owned(),
        Signature {
            type_params: vec!["K".to_owned(), "V".to_owned()],
            params: vec![
                builtins::map_type(Type::Param("K".to_owned()), Type::Param("V".to_owned())),
                Type::Param("K".to_owned()),
                Type::Param("V".to_owned()),
            ],
            ret: Type::Param("V".to_owned()),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::KEYS.to_owned(),
        Signature {
            type_params: vec!["K".to_owned(), "V".to_owned()],
            params: vec![builtins::map_type(
                Type::Param("K".to_owned()),
                Type::Param("V".to_owned()),
            )],
            ret: Type::Array(Box::new(Type::Param("K".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::VALUES.to_owned(),
        Signature {
            type_params: vec!["K".to_owned(), "V".to_owned()],
            params: vec![builtins::map_type(
                Type::Param("K".to_owned()),
                Type::Param("V".to_owned()),
            )],
            ret: Type::Array(Box::new(Type::Param("V".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::REMOVE.to_owned(),
        Signature {
            type_params: vec!["K".to_owned(), "V".to_owned()],
            params: vec![
                builtins::map_type(Type::Param("K".to_owned()), Type::Param("V".to_owned())),
                Type::Param("K".to_owned()),
            ],
            ret: builtins::map_type(Type::Param("K".to_owned()), Type::Param("V".to_owned())),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::SORT.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Array(Box::new(Type::Param("T".to_owned())))],
            ret: Type::Array(Box::new(Type::Param("T".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::REVERSE.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Array(Box::new(Type::Param("T".to_owned())))],
            ret: Type::Array(Box::new(Type::Param("T".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::SLICE.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::I32,
                Type::I32,
            ],
            ret: Type::Array(Box::new(Type::Param("T".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::MAP_FN.to_owned(),
        Signature {
            type_params: vec!["T".to_owned(), "U".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Fn(
                    vec![Type::Param("T".to_owned())],
                    Box::new(Type::Param("U".to_owned())),
                ),
            ],
            ret: Type::Array(Box::new(Type::Param("U".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::FILTER.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Fn(vec![Type::Param("T".to_owned())], Box::new(Type::Bool)),
            ],
            ret: Type::Array(Box::new(Type::Param("T".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::ANY.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Fn(vec![Type::Param("T".to_owned())], Box::new(Type::Bool)),
            ],
            ret: Type::Bool,
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::ALL.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Fn(vec![Type::Param("T".to_owned())], Box::new(Type::Bool)),
            ],
            ret: Type::Bool,
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::FOLD.to_owned(),
        Signature {
            type_params: vec!["T".to_owned(), "U".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Param("U".to_owned()),
                Type::Fn(
                    vec![Type::Param("U".to_owned()), Type::Param("T".to_owned())],
                    Box::new(Type::Param("U".to_owned())),
                ),
            ],
            ret: Type::Param("U".to_owned()),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::SORT_BY.to_owned(),
        Signature {
            type_params: vec!["T".to_owned(), "K".to_owned()],
            params: vec![
                Type::Array(Box::new(Type::Param("T".to_owned()))),
                Type::Fn(
                    vec![Type::Param("T".to_owned())],
                    Box::new(Type::Param("K".to_owned())),
                ),
            ],
            ret: Type::Array(Box::new(Type::Param("T".to_owned()))),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    signatures.insert(
        builtins::JOIN.to_owned(),
        Signature {
            type_params: vec!["T".to_owned()],
            params: vec![Type::Task(Box::new(Type::Param("T".to_owned())))],
            ret: Type::Result(
                Box::new(Type::Param("T".to_owned())),
                Box::new(Type::Named(builtins::TASK_ERROR.to_owned())),
            ),
            fields: None,
            variants: None,
            spawn_safe: false,
        },
    );
    for record in &program.records {
        if matches!(
            record.name.as_str(),
            "i32"
                | "i64"
                | "f64"
                | "bool"
                | "String"
                | "Bytes"
                | "Unit"
                | "Result"
                | "Option"
                | "Task"
                | "read_text"
                | "read_bytes"
                | "write_text"
                | "write_bytes"
                | "lines"
                | "args"
                | "print"
                | "bit_and"
                | "bit_or"
                | "bit_xor"
                | "bit_not"
                | "shl"
                | "shr"
                | "sha256"
                | "md5"
                | "sha1"
                | "hmac_sha256"
                | "pbkdf2_sha256"
                | "base64_encode"
                | "base64_decode"
                | "hex"
                | "random_bytes"
                | "Conn"
                | "tcp_connect"
                | "tcp_send"
                | "tcp_recv"
                | "tcp_close"
                | "Listener"
                | "listen"
                | "accept"
                | "http_read"
                | "http_write"
                | "serve"
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
                | "utf8_bytes"
                | "utf8_decode"
                | "utf8_encode"
                | "utf8_decode_bytes"
                | "bytes_from_i32"
                | "bytes_to_i32"
                | "join"
        ) || !record_names.insert(record.name.clone())
        {
            return Err(Diagnostic::new(
                "E106",
                record.span,
                format!("duplicate or reserved type {}", record.name),
            ));
        }
        arities.insert(record.name.clone(), record.type_params.len());
    }
    for enum_decl in &program.enums {
        if matches!(
            enum_decl.name.as_str(),
            "i32"
                | "i64"
                | "f64"
                | "bool"
                | "String"
                | "Bytes"
                | "Unit"
                | "Result"
                | "Option"
                | "Task"
                | "read_text"
                | "read_bytes"
                | "write_text"
                | "write_bytes"
                | "lines"
                | "args"
                | "print"
                | "bit_and"
                | "bit_or"
                | "bit_xor"
                | "bit_not"
                | "shl"
                | "shr"
                | "sha256"
                | "md5"
                | "sha1"
                | "hmac_sha256"
                | "pbkdf2_sha256"
                | "base64_encode"
                | "base64_decode"
                | "hex"
                | "random_bytes"
                | "Conn"
                | "tcp_connect"
                | "tcp_send"
                | "tcp_recv"
                | "tcp_close"
                | "Listener"
                | "listen"
                | "accept"
                | "http_read"
                | "http_write"
                | "serve"
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
                | "utf8_bytes"
                | "utf8_decode"
                | "utf8_encode"
                | "utf8_decode_bytes"
                | "bytes_from_i32"
                | "bytes_to_i32"
                | "join"
        ) || !record_names.insert(enum_decl.name.clone())
        {
            return Err(Diagnostic::new(
                "E106",
                enum_decl.span,
                format!("duplicate or reserved type {}", enum_decl.name),
            ));
        }
        arities.insert(enum_decl.name.clone(), enum_decl.type_params.len());
        let mut names = HashSet::new();
        for variant in &enum_decl.variants {
            if !names.insert(&variant.name) {
                return Err(Diagnostic::new(
                    "E106",
                    enum_decl.span,
                    format!("duplicate variant {}", variant.name),
                ));
            }
        }
        signatures.insert(
            enum_decl.name.clone(),
            Signature {
                type_params: enum_decl.type_params.clone(),
                params: Vec::new(),
                ret: if enum_decl.type_params.is_empty() {
                    Type::Named(enum_decl.name.clone())
                } else {
                    Type::Applied(
                        enum_decl.name.clone(),
                        enum_decl
                            .type_params
                            .iter()
                            .map(|param| Type::Param(param.clone()))
                            .collect(),
                    )
                },
                fields: None,
                variants: Some(enum_decl.variants.clone()),
                spawn_safe: false,
            },
        );
    }
    for enum_decl in &program.enums {
        validate_params(&enum_decl.type_params, &record_names, enum_decl.span)?;
        for param in &enum_decl.type_params {
            if !enum_decl
                .variants
                .iter()
                .filter_map(|variant| variant.payload.as_ref())
                .any(|payload| mentions_param(payload, param))
            {
                return Err(Diagnostic::new(
                    "E115",
                    enum_decl.span,
                    format!("unused enum type parameter {param}"),
                ));
            }
        }
        for variant in &enum_decl.variants {
            if let Some(payload) = &variant.payload {
                validate_type(payload, &arities, enum_decl.span)?;
            }
        }
    }
    for record in &program.records {
        validate_params(&record.type_params, &record_names, record.span)?;
        for param in &record.type_params {
            if !record
                .fields
                .iter()
                .any(|(_, ty)| mentions_param(ty, param))
            {
                return Err(Diagnostic::new(
                    "E115",
                    record.span,
                    format!("unused record type parameter {param}"),
                ));
            }
        }
        let mut names = HashSet::new();
        for (name, ty) in &record.fields {
            if !names.insert(name) {
                return Err(Diagnostic::new(
                    "E106",
                    record.span,
                    format!("duplicate field {name}"),
                ));
            }
            validate_type(ty, &arities, record.span)?;
        }
        let signature = Signature {
            type_params: record.type_params.clone(),
            params: record.fields.iter().map(|(_, ty)| ty.clone()).collect(),
            ret: if record.type_params.is_empty() {
                Type::Named(record.name.clone())
            } else {
                Type::Applied(
                    record.name.clone(),
                    record
                        .type_params
                        .iter()
                        .map(|name| Type::Param(name.clone()))
                        .collect(),
                )
            },
            fields: Some(record.fields.clone()),
            variants: None,
            spawn_safe: false,
        };
        signatures.insert(record.name.clone(), signature);
    }
    for record in &program.records {
        if has_record_cycle(&record.name, program, &mut HashSet::new()) {
            return Err(Diagnostic::new(
                "E112",
                record.span,
                "recursive record value layout",
            ));
        }
    }
    for enum_decl in &program.enums {
        if enum_has_cycle(&enum_decl.name, program, &mut HashSet::new()) {
            return Err(Diagnostic::new(
                "E112",
                enum_decl.span,
                "recursive enum value layout",
            ));
        }
    }
    for function in &program.functions {
        validate_params(&function.type_params, &record_names, function.span)?;
        for (_, ty) in &function.params {
            validate_type(ty, &arities, function.span)?;
        }
        validate_type(&function.ret, &arities, function.span)?;
        let signature = Signature {
            type_params: function.type_params.clone(),
            params: function.params.iter().map(|(_, ty)| ty.clone()).collect(),
            ret: function.ret.clone(),
            fields: None,
            variants: None,
            spawn_safe: function_is_spawn_safe(&function.name, program, &mut HashSet::new()),
        };
        if signatures
            .insert(function.name.clone(), signature)
            .is_some()
        {
            return Err(Diagnostic::new(
                "E106",
                function.span,
                format!("duplicate function {}", function.name),
            ));
        }
    }
    for function in &program.functions {
        let mut env = HashMap::new();
        for (name, ty) in &function.params {
            if env
                .insert(
                    name.clone(),
                    Binding {
                        ty: ty.clone(),
                        mutable: false,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new(
                    "E106",
                    function.span,
                    format!("duplicate parameter {name}"),
                ));
            }
        }
        let body_type = type_of(&function.body, &env, &signatures, &function.ret, &mut types)?;
        require(
            &function.ret,
            &body_type,
            function.body.span,
            "function result",
        )?;
    }
    Ok(types)
}

fn validate_params(
    params: &[String],
    names: &HashSet<String>,
    span: Span,
) -> Result<(), Diagnostic> {
    let mut seen = HashSet::new();
    for param in params {
        if !seen.insert(param)
            || names.contains(param)
            || matches!(
                param.as_str(),
                "i32" | "bool" | "String" | "Unit" | "Result" | "Option" | "Task"
            )
        {
            return Err(Diagnostic::new(
                "E106",
                span,
                format!("duplicate or reserved type parameter {param}"),
            ));
        }
    }
    Ok(())
}

fn function_is_spawn_safe(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.name == name)
    else {
        return false;
    };
    if !function.type_params.is_empty() {
        return false;
    }
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let safe = expression_is_spawn_safe(&function.body, program, visiting);
    visiting.remove(name);
    safe
}

fn expression_is_spawn_safe(
    expr: &Expr,
    program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::I64(_)
        | ExprKind::F64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_)
        | ExprKind::None => true,
        ExprKind::Variant(_, _, payload) => payload
            .as_ref()
            .is_none_or(|value| expression_is_spawn_safe(value, program, visiting)),
        ExprKind::Array(items) => items
            .iter()
            .all(|item| expression_is_spawn_safe(item, program, visiting)),
        ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
            expression_is_spawn_safe(left, program, visiting)
                && expression_is_spawn_safe(right, program, visiting)
        }
        ExprKind::Field(value, _)
        | ExprKind::Not(value)
        | ExprKind::Neg(value)
        | ExprKind::Ok(value)
        | ExprKind::Err(value)
        | ExprKind::Some(value)
        | ExprKind::Try(value) => expression_is_spawn_safe(value, program, visiting),
        ExprKind::Call(name, args) => {
            name != builtins::READ_TEXT
                && name != builtins::READ_BYTES
                && name != builtins::WRITE_TEXT
                && name != builtins::WRITE_BYTES
                && name != builtins::ARGS
                && name != builtins::PRINT
                && name != builtins::SERVE
                && name != builtins::RANDOM_BYTES
                && name != builtins::TCP_CONNECT
                && name != builtins::TCP_SEND
                && name != builtins::TCP_RECV
                && name != builtins::TCP_CLOSE
                && name != builtins::LISTEN
                && name != builtins::ACCEPT
                && name != builtins::HTTP_READ
                && name != builtins::HTTP_WRITE
                && name != builtins::HTTP_REQUEST
                && name != builtins::LIST_DIR
                && name != builtins::EXISTS
                && name != builtins::MAKE_DIR
                && name != builtins::REMOVE_FILE
                && name != builtins::ENV
                && name != builtins::NOW_MS
                && name != builtins::CLOCK_NS
                && name != builtins::SLEEP_MS
                && name != builtins::READ_LINE
                && name != builtins::READ_STDIN
                && name != builtins::EXIT
                && (name != builtins::JOIN || args.len() == 2)
                && (matches!(
                    name.as_str(),
                    builtins::LINES
                        | builtins::LEN
                        | builtins::PARSE_I32
                        | builtins::PARSE_I64
                        | builtins::PARSE_F64
                        | builtins::JOIN
                        | builtins::BIT_AND
                        | builtins::BIT_OR
                        | builtins::BIT_XOR
                        | builtins::BIT_NOT
                        | builtins::SHL
                        | builtins::SHR
                        | builtins::SHA256
                        | builtins::MD5
                        | builtins::SHA1
                        | builtins::HMAC_SHA256
                        | builtins::PBKDF2_SHA256
                        | builtins::BASE64_ENCODE
                        | builtins::BASE64_DECODE
                        | builtins::HEX
                        | builtins::REQUEST
                        | builtins::RESPONSE
                        | builtins::ABS
                        | builtins::MIN
                        | builtins::MAX
                        | builtins::POW
                        | builtins::SQRT
                        | builtins::FLOOR
                        | builtins::CEIL
                        | builtins::ROUND
                        | builtins::EXP
                        | builtins::LN
                        | builtins::SIN
                        | builtins::COS
                        | builtins::TAN
                        | builtins::ATAN2
                        | builtins::PI
                        | builtins::MAP_FN
                        | builtins::FILTER
                        | builtins::ANY
                        | builtins::ALL
                        | builtins::FOLD
                        | builtins::SORT_BY
                        | builtins::RANGE
                        | builtins::SORT
                        | builtins::REVERSE
                        | builtins::SLICE
                        | builtins::MAP
                        | builtins::GET
                        | builtins::GET_OR
                        | builtins::KEYS
                        | builtins::VALUES
                        | builtins::REMOVE
                        | builtins::TO_STRING
                        | builtins::CHARS
                        | builtins::SPLIT
                        | builtins::TRIM
                        | builtins::CONTAINS
                        | builtins::STARTS_WITH
                        | builtins::ENDS_WITH
                        | builtins::REPLACE
                        | builtins::LOWER
                        | builtins::UPPER
                        | builtins::WIDEN_I64
                        | builtins::TO_F64
                        | builtins::NARROW_I32
                        | builtins::UTF8_BYTES
                        | builtins::UTF8_DECODE
                        | builtins::UTF8_ENCODE
                        | builtins::UTF8_DECODE_BYTES
                        | builtins::BYTES_FROM_I32
                        | builtins::BYTES_TO_I32
                ) || program.records.iter().any(|record| record.name == *name)
                    || function_is_spawn_safe(name, program, visiting))
                && args
                    .iter()
                    .all(|arg| expression_is_spawn_safe(arg, program, visiting))
        }
        ExprKind::Spawn(_) => false,
        ExprKind::If(condition, yes, no) => [condition, yes, no]
            .iter()
            .all(|value| expression_is_spawn_safe(value, program, visiting)),
        ExprKind::Match(value, arms) => {
            expression_is_spawn_safe(value, program, visiting)
                && arms
                    .iter()
                    .all(|(_, body)| expression_is_spawn_safe(body, program, visiting))
        }
        ExprKind::Lambda(_, body) => expression_is_spawn_safe(body, program, visiting),
        ExprKind::Apply(..) => false,
        ExprKind::Block(stmts, tail) => {
            stmts.iter().all(|stmt| match stmt {
                Stmt::Assign { path, value, .. } => {
                    path.iter().all(|step| match step {
                        PlaceStep::Index(index, _) => {
                            expression_is_spawn_safe(index, program, visiting)
                        }
                        PlaceStep::Field(..) => true,
                    }) && expression_is_spawn_safe(value, program, visiting)
                }
                Stmt::Let { value, .. }
                | Stmt::Push { value, .. }
                | Stmt::Return { value, .. }
                | Stmt::Expr(value) => expression_is_spawn_safe(value, program, visiting),
                Stmt::Break { .. } | Stmt::Continue { .. } => true,
                Stmt::For { iterable, body, .. } => {
                    expression_is_spawn_safe(iterable, program, visiting)
                        && expression_is_spawn_safe(body, program, visiting)
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    expression_is_spawn_safe(condition, program, visiting)
                        && expression_is_spawn_safe(body, program, visiting)
                }
            }) && tail
                .as_ref()
                .is_none_or(|value| expression_is_spawn_safe(value, program, visiting))
        }
    }
}

fn mentions_param(ty: &Type, param: &str) -> bool {
    match ty {
        Type::Param(name) => name == param,
        Type::Applied(_, args) => args.iter().any(|arg| mentions_param(arg, param)),
        Type::Array(element) => mentions_param(element, param),
        Type::Option(element) => mentions_param(element, param),
        Type::Task(result) => mentions_param(result, param),
        Type::Result(ok, err) => mentions_param(ok, param) || mentions_param(err, param),
        _ => false,
    }
}

fn validate_type(
    ty: &Type,
    arities: &HashMap<String, usize>,
    span: Span,
) -> Result<(), Diagnostic> {
    match ty {
        Type::Named(name) if arities.get(name) != Some(&0) => Err(Diagnostic::new(
            "E103",
            span,
            format!("unknown or unapplied type {name}"),
        )),
        Type::Applied(name, args) => {
            if arities.get(name) != Some(&args.len()) {
                return Err(Diagnostic::new(
                    "E103",
                    span,
                    format!("invalid type arguments for {name}"),
                ));
            }
            if name == builtins::MAP && !builtins::map_key(&args[0]) {
                return Err(Diagnostic::new(
                    "E103",
                    span,
                    format!(
                        "map keys must be i32, i64, String, or bool, not {}",
                        args[0]
                    ),
                ));
            }
            for arg in args {
                validate_type(arg, arities, span)?;
            }
            Ok(())
        }
        Type::Array(element) => validate_type(element, arities, span),
        Type::Fn(params, ret) => {
            for param in params {
                validate_type(param, arities, span)?;
            }
            validate_type(ret, arities, span)
        }
        Type::Option(element) => validate_type(element, arities, span),
        Type::Task(result) => validate_type(result, arities, span),
        Type::Result(ok, err) => {
            validate_type(ok, arities, span)?;
            validate_type(err, arities, span)
        }
        _ => Ok(()),
    }
}

fn has_record_cycle(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(record) = program.records.iter().find(|record| record.name == name) else {
        return false;
    };
    let args = record
        .type_params
        .iter()
        .map(|param| Type::Param(param.clone()))
        .collect::<Vec<_>>();
    record_has_cycle(name, &args, program, visiting)
}

fn record_has_cycle(
    name: &str,
    args: &[Type],
    program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let Some(record) = program.records.iter().find(|record| record.name == name) else {
        return enum_has_cycle(name, program, visiting);
    };
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let inferred: HashMap<String, Type> = record
        .type_params
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect();
    let cycle = record
        .fields
        .iter()
        .any(|(_, ty)| type_has_cycle(&substitute(ty, &inferred), program, visiting));
    visiting.remove(name);
    cycle
}

fn enum_has_cycle(name: &str, program: &Program, visiting: &mut HashSet<String>) -> bool {
    let Some(decl) = program.enums.iter().find(|decl| decl.name == name) else {
        return false;
    };
    if !visiting.insert(name.to_owned()) {
        return true;
    }
    let cycle = decl
        .variants
        .iter()
        .filter_map(|variant| variant.payload.as_ref())
        .any(|payload| type_has_cycle(payload, program, visiting));
    visiting.remove(name);
    cycle
}

fn type_has_cycle(ty: &Type, program: &Program, visiting: &mut HashSet<String>) -> bool {
    match ty {
        Type::Named(name) => record_has_cycle(name, &[], program, visiting),
        Type::Applied(name, args) => record_has_cycle(name, args, program, visiting),
        Type::Array(_) => false,
        Type::Option(element) => type_has_cycle(element, program, visiting),
        Type::Task(_) => false,
        Type::Result(ok, err) => {
            type_has_cycle(ok, program, visiting) || type_has_cycle(err, program, visiting)
        }
        _ => false,
    }
}

fn compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual || *actual == Type::Never {
        return true;
    }
    match (expected, actual) {
        (Type::Array(_), Type::EmptyArray) => true,
        (Type::Array(expected), Type::Array(actual)) => compatible(expected, actual),
        (Type::Option(expected), Type::Option(actual)) => compatible(expected, actual),
        (Type::Task(expected), Type::Task(actual)) => compatible(expected, actual),
        (Type::Fn(expected_params, expected_ret), Type::Fn(actual_params, actual_ret)) => {
            expected_params == actual_params && compatible(expected_ret, actual_ret)
        }
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            a_args.iter().zip(b_args).all(|(a, b)| compatible(a, b))
        }
        (Type::Result(expected_ok, expected_err), Type::Result(actual_ok, actual_err)) => {
            compatible(expected_ok, actual_ok) && compatible(expected_err, actual_err)
        }
        _ => false,
    }
}

fn binding_type_is_known(ty: &Type) -> bool {
    match ty {
        Type::Never | Type::EmptyArray => false,
        Type::Array(element) | Type::Option(element) | Type::Task(element) => {
            binding_type_is_known(element)
        }
        Type::Fn(params, ret) => {
            params.iter().all(binding_type_is_known) && binding_type_is_known(ret)
        }
        Type::Result(ok, err) => binding_type_is_known(ok) && binding_type_is_known(err),
        Type::Applied(_, args) => args.iter().all(binding_type_is_known),
        _ => true,
    }
}

fn join(left: &Type, right: &Type) -> Option<Type> {
    if left == right {
        return Some(left.clone());
    }
    if *left == Type::Never {
        return Some(right.clone());
    }
    if *right == Type::Never {
        return Some(left.clone());
    }
    match (left, right) {
        (Type::Array(_), Type::EmptyArray) => Some(left.clone()),
        (Type::EmptyArray, Type::Array(_)) => Some(right.clone()),
        (Type::Array(left), Type::Array(right)) => Some(Type::Array(Box::new(join(left, right)?))),
        (Type::Option(left), Type::Option(right)) => {
            Some(Type::Option(Box::new(join(left, right)?)))
        }
        (Type::Task(left), Type::Task(right)) => Some(Type::Task(Box::new(join(left, right)?))),
        (Type::Fn(left_params, left_ret), Type::Fn(right_params, right_ret))
            if left_params == right_params =>
        {
            Some(Type::Fn(
                left_params.clone(),
                Box::new(join(left_ret, right_ret)?),
            ))
        }
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            Some(Type::Applied(
                a_name.clone(),
                a_args
                    .iter()
                    .zip(b_args)
                    .map(|(a, b)| join(a, b))
                    .collect::<Option<Vec<_>>>()?,
            ))
        }
        (Type::Result(left_ok, left_err), Type::Result(right_ok, right_err)) => Some(Type::Result(
            Box::new(join(left_ok, right_ok)?),
            Box::new(join(left_err, right_err)?),
        )),
        _ => None,
    }
}

fn require(expected: &Type, actual: &Type, span: Span, context: &str) -> Result<(), Diagnostic> {
    if compatible(expected, actual) {
        Ok(())
    } else {
        Err(Diagnostic::new(
            "E102",
            span,
            format!("{context}: expected {expected}, got {actual}"),
        ))
    }
}

fn substitute(ty: &Type, inferred: &HashMap<String, Type>) -> Type {
    match ty {
        Type::Param(name) => inferred.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Array(element) => Type::Array(Box::new(substitute(element, inferred))),
        Type::Option(element) => Type::Option(Box::new(substitute(element, inferred))),
        Type::Task(result) => Type::Task(Box::new(substitute(result, inferred))),
        Type::Fn(params, ret) => Type::Fn(
            params
                .iter()
                .map(|param| substitute(param, inferred))
                .collect(),
            Box::new(substitute(ret, inferred)),
        ),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute(ok, inferred)),
            Box::new(substitute(err, inferred)),
        ),
        Type::Applied(name, args) => Type::Applied(
            name.clone(),
            args.iter().map(|arg| substitute(arg, inferred)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn infer_params(
    pattern: &Type,
    actual: &Type,
    inferred: &mut HashMap<String, Type>,
    span: Span,
) -> Result<(), Diagnostic> {
    if matches!(actual, Type::Never | Type::EmptyArray) {
        return Ok(());
    }
    match (pattern, actual) {
        (Type::Param(name), actual) => {
            if let Some(previous) = inferred.get(name) {
                let common = join(previous, actual).ok_or_else(|| {
                    Diagnostic::new(
                        "E102",
                        span,
                        format!("incompatible type arguments {previous} and {actual}"),
                    )
                })?;
                inferred.insert(name.clone(), common);
                Ok(())
            } else {
                inferred.insert(name.clone(), actual.clone());
                Ok(())
            }
        }
        (Type::Array(a), Type::Array(b)) => infer_params(a, b, inferred, span),
        (Type::Option(a), Type::Option(b)) => infer_params(a, b, inferred, span),
        (Type::Task(a), Type::Task(b)) => infer_params(a, b, inferred, span),
        (Type::Fn(a_params, a_ret), Type::Fn(b_params, b_ret))
            if a_params.len() == b_params.len() =>
        {
            for (a, b) in a_params.iter().zip(b_params) {
                infer_params(a, b, inferred, span)?;
            }
            infer_params(a_ret, b_ret, inferred, span)
        }
        (Type::Result(a_ok, a_err), Type::Result(b_ok, b_err)) => {
            infer_params(a_ok, b_ok, inferred, span)?;
            infer_params(a_err, b_err, inferred, span)
        }
        (Type::Applied(a_name, a_args), Type::Applied(b_name, b_args))
            if a_name == b_name && a_args.len() == b_args.len() =>
        {
            for (a, b) in a_args.iter().zip(b_args) {
                infer_params(a, b, inferred, span)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn match_pattern(
    pattern: &Pattern,
    matched: &Type,
    signatures: &HashMap<String, Signature>,
) -> Result<(String, Option<(String, Type)>), Diagnostic> {
    let invalid = || {
        Diagnostic::new(
            "E116",
            pattern.span,
            format!("pattern does not match {matched}"),
        )
    };
    match (&pattern.kind, matched) {
        (PatternKind::Int(value), Type::I32) => Ok((value.to_string(), None)),
        (PatternKind::I64(value), Type::I64) => Ok((value.to_string(), None)),
        (PatternKind::Wildcard, _) => Ok(("_".to_owned(), None)),
        (PatternKind::Ok(name), Type::Result(ok, _)) => {
            Ok(("Ok".to_owned(), Some((name.clone(), *ok.clone()))))
        }
        (PatternKind::Err(name), Type::Result(_, err)) => {
            Ok(("Err".to_owned(), Some((name.clone(), *err.clone()))))
        }
        (PatternKind::Some(name), Type::Option(element)) => {
            Ok(("Some".to_owned(), Some((name.clone(), *element.clone()))))
        }
        (PatternKind::None, Type::Option(_)) => Ok(("None".to_owned(), None)),
        (PatternKind::Bool(value), Type::Bool) => Ok((value.to_string(), None)),
        (PatternKind::String(value), Type::String) => Ok((format!("{value:?}"), None)),
        (
            PatternKind::Variant(name, variant, binding),
            Type::Named(actual) | Type::Applied(actual, _),
        ) if name == actual => {
            let signature = signatures.get(name);
            let args = match matched {
                Type::Applied(_, args) => args.clone(),
                _ => Vec::new(),
            };
            let inferred: HashMap<String, Type> = signature
                .map(|signature| signature.type_params.iter().cloned().zip(args).collect())
                .unwrap_or_default();
            let declared = signature
                .and_then(|signature| signature.variants.as_ref())
                .and_then(|variants| variants.iter().find(|item| item.name == *variant));
            match (declared.and_then(|item| item.payload.as_ref()), binding) {
                (None, None) if declared.is_some() => Ok((variant.clone(), None)),
                (Some(ty), Some(name)) => Ok((
                    variant.clone(),
                    Some((name.clone(), substitute(ty, &inferred))),
                )),
                _ => Err(invalid()),
            }
        }
        _ => Err(invalid()),
    }
}

fn type_of(
    expr: &Expr,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    let inferred = infer(expr, env, signatures, return_type, types)?;
    types.insert(expr.span, inferred.clone());
    Ok(inferred)
}

fn infer(
    expr: &Expr,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    match &expr.kind {
        ExprKind::Lambda(..) => type_lambda(expr, None, env, signatures, types),
        ExprKind::Apply(callee, args) => {
            let callee_type = type_of(callee, env, signatures, return_type, types)?;
            let Type::Fn(params, ret) = callee_type else {
                return Err(Diagnostic::new(
                    "E102",
                    callee.span,
                    format!("only function values can be called, got {callee_type}"),
                ));
            };
            if params.len() != args.len() {
                return Err(Diagnostic::new(
                    "E105",
                    expr.span,
                    format!(
                        "function value expects {} arguments, got {}",
                        params.len(),
                        args.len()
                    ),
                ));
            }
            for (arg, expected) in args.iter().zip(&params) {
                let actual =
                    type_expected(arg, Some(expected), env, signatures, return_type, types)?;
                require(expected, &actual, arg.span, "argument")?;
            }
            Ok(*ret)
        }
        ExprKind::Int(_) => Ok(Type::I32),
        ExprKind::I64(_) => Ok(Type::I64),
        ExprKind::F64(_) => Ok(Type::F64),
        ExprKind::Neg(value) => {
            let actual = type_of(value, env, signatures, return_type, types)?;
            if matches!(actual, Type::I32 | Type::I64 | Type::F64 | Type::Never) {
                Ok(actual)
            } else {
                Err(Diagnostic::new(
                    "E104",
                    expr.span,
                    "negation requires i32 or i64",
                ))
            }
        }
        ExprKind::Bool(_) => Ok(Type::Bool),
        ExprKind::String(_) => Ok(Type::String),
        ExprKind::Variant(name, variant, payload) => {
            let signature = signatures.get(name).ok_or_else(|| {
                Diagnostic::new("E103", expr.span, format!("unknown enum {name}"))
            })?;
            let Some(variants) = &signature.variants else {
                return Err(Diagnostic::new(
                    "E114",
                    expr.span,
                    format!("{name} is not an enum"),
                ));
            };
            let declared = variants
                .iter()
                .find(|item| item.name == *variant)
                .ok_or_else(|| {
                    Diagnostic::new(
                        "E114",
                        expr.span,
                        format!("unknown variant {name}::{variant}"),
                    )
                })?;
            let mut inferred = HashMap::new();
            match (&declared.payload, payload) {
                (None, None) => {}
                (Some(expected), Some(value)) => {
                    let actual =
                        type_expected(value, Some(expected), env, signatures, return_type, types)?;
                    if actual == Type::Never {
                        return Ok(Type::Never);
                    }
                    infer_params(expected, &actual, &mut inferred, value.span)?;
                    require(
                        &substitute(expected, &inferred),
                        &actual,
                        value.span,
                        "enum payload",
                    )?;
                }
                _ => {
                    return Err(Diagnostic::new(
                        "E114",
                        expr.span,
                        format!("wrong payload shape for {name}::{variant}"),
                    ));
                }
            }
            if signature.type_params.is_empty() {
                Ok(Type::Named(name.clone()))
            } else {
                // Unconstrained arguments stay open, like `None`, until context fixes them.
                Ok(Type::Applied(
                    name.clone(),
                    signature
                        .type_params
                        .iter()
                        .map(|param| inferred.get(param).cloned().unwrap_or(Type::Never))
                        .collect(),
                ))
            }
        }
        ExprKind::Array(values) => {
            let Some(first) = values.first() else {
                return Ok(Type::EmptyArray);
            };
            let mut element = type_of(first, env, signatures, return_type, types)?;
            if element == Type::Never {
                return Ok(Type::Never);
            }
            for value in values.iter().skip(1) {
                let actual = type_of(value, env, signatures, return_type, types)?;
                element = join(&element, &actual).ok_or_else(|| {
                    Diagnostic::new(
                        "E102",
                        value.span,
                        format!("array elements have different types: {element} and {actual}"),
                    )
                })?;
            }
            Ok(Type::Array(Box::new(element)))
        }
        ExprKind::Index(array, index) => {
            let array_type = type_of(array, env, signatures, return_type, types)?;
            let index_type = type_of(index, env, signatures, return_type, types)?;
            if array_type == Type::Never || index_type == Type::Never {
                return Ok(Type::Never);
            }
            if matches!(&array_type, Type::Applied(map, _) if map == builtins::MAP) {
                return Err(Diagnostic::new(
                    "E110",
                    array.span,
                    "read map entries with get(map,key) or get_or(map,key,default)",
                ));
            }
            require(&Type::I32, &index_type, index.span, "array index")?;
            match array_type {
                Type::Array(element) => Ok(*element),
                Type::Bytes => Ok(Type::I32),
                _ => Err(Diagnostic::new(
                    "E110",
                    array.span,
                    "indexing requires an array or Bytes",
                )),
            }
        }
        ExprKind::Field(value, field) => {
            let actual = type_of(value, env, signatures, return_type, types)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            if !matches!(actual, Type::Named(_) | Type::Applied(..)) {
                return Err(Diagnostic::new(
                    "E113",
                    value.span,
                    "field access requires a record",
                ));
            }
            field_type(actual, field, signatures, expr.span)
        }
        ExprKind::Ok(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(inner), Box::new(Type::Never)))
        }
        ExprKind::Err(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Result(Box::new(Type::Never), Box::new(inner)))
        }
        ExprKind::Some(inner) => {
            let inner = type_of(inner, env, signatures, return_type, types)?;
            if inner == Type::Never {
                return Ok(Type::Never);
            }
            Ok(Type::Option(Box::new(inner)))
        }
        ExprKind::None => Ok(Type::Option(Box::new(Type::Never))),
        ExprKind::Try(inner) => {
            let actual = type_of(inner, env, signatures, return_type, types)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            if let Type::Option(value) = actual {
                if !matches!(return_type, Type::Option(_)) {
                    return Err(Diagnostic::new(
                        "E111",
                        expr.span,
                        "? on an Option requires an Option return type",
                    ));
                }
                return Ok(*value);
            }
            let Type::Result(ok, err) = actual else {
                return Err(Diagnostic::new(
                    "E111",
                    inner.span,
                    "? requires a Result or Option value",
                ));
            };
            let Type::Result(_, expected_err) = return_type else {
                return Err(Diagnostic::new(
                    "E111",
                    expr.span,
                    "? requires a Result return type",
                ));
            };
            require(expected_err, &err, expr.span, "propagated error")?;
            Ok(*ok)
        }
        ExprKind::Var(name) => env
            .get(name)
            .map(|binding| binding.ty.clone())
            .ok_or_else(|| Diagnostic::new("E101", expr.span, format!("unknown name {name}"))),
        ExprKind::Not(inner) => {
            let actual = type_of(inner, env, signatures, return_type, types)?;
            if actual == Type::Never {
                return Ok(Type::Never);
            }
            require(&Type::Bool, &actual, inner.span, "logical negation")?;
            Ok(Type::Bool)
        }
        ExprKind::Binary(left, op, right) => {
            let lhs = type_of(left, env, signatures, return_type, types)?;
            let rhs = type_of(right, env, signatures, return_type, types)?;
            if lhs == Type::Never {
                return Ok(Type::Never);
            }
            if matches!(op, Op::And | Op::Or) {
                return if lhs == Type::Bool && matches!(rhs, Type::Bool | Type::Never) {
                    Ok(Type::Bool)
                } else {
                    Err(Diagnostic::new(
                        "E104",
                        expr.span,
                        format!("invalid operands {lhs} and {rhs} for {op:?}"),
                    ))
                };
            }
            if rhs == Type::Never {
                return Ok(Type::Never);
            }
            match op {
                Op::Add
                | Op::Sub
                | Op::Mul
                | Op::Div
                | Op::Rem
                | Op::Lt
                | Op::Le
                | Op::Gt
                | Op::Ge
                    if lhs == rhs && matches!(lhs, Type::I32 | Type::I64 | Type::F64) =>
                {
                    if matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem) {
                        Ok(lhs)
                    } else {
                        Ok(Type::Bool)
                    }
                }
                Op::Add if lhs == Type::String && rhs == Type::String => Ok(Type::String),
                Op::Add
                    if matches!(lhs, Type::Array(_) | Type::EmptyArray)
                        && matches!(rhs, Type::Array(_) | Type::EmptyArray) =>
                {
                    join(&lhs, &rhs).ok_or_else(|| {
                        Diagnostic::new(
                            "E104",
                            expr.span,
                            format!("cannot concatenate {lhs} and {rhs}"),
                        )
                    })
                }
                Op::Lt | Op::Le | Op::Gt | Op::Ge if lhs == Type::String && rhs == Type::String => {
                    Ok(Type::Bool)
                }
                Op::Eq | Op::Ne
                    if lhs == rhs
                        && matches!(
                            lhs,
                            Type::I32
                                | Type::I64
                                | Type::F64
                                | Type::Bool
                                | Type::String
                                | Type::Bytes
                        ) =>
                {
                    Ok(Type::Bool)
                }
                _ => Err(Diagnostic::new(
                    "E104",
                    expr.span,
                    format!("invalid operands {lhs} and {rhs} for {op:?}"),
                )),
            }
        }
        ExprKind::Call(name, args) if name == builtins::MAP => {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "E105",
                    expr.span,
                    format!("Map expects 0 arguments, got {}", args.len()),
                ));
            }
            Ok(builtins::map_type(Type::Never, Type::Never))
        }
        ExprKind::Call(name, args) if name == builtins::JOIN && args.len() == 2 => {
            let parts = type_of(&args[0], env, signatures, return_type, types)?;
            let separator = type_of(&args[1], env, signatures, return_type, types)?;
            require(
                &Type::Array(Box::new(Type::String)),
                &parts,
                args[0].span,
                "argument",
            )?;
            require(&Type::String, &separator, args[1].span, "argument")?;
            Ok(Type::String)
        }
        // Functions, records, and builtins take precedence over local values.
        ExprKind::Call(name, args)
            if !signatures.contains_key(name)
                && name != builtins::MAP
                && !(name == builtins::JOIN && args.len() == 2)
                && env.contains_key(name) =>
        {
            let Type::Fn(params, ret) = env[name].ty.clone() else {
                return Err(Diagnostic::new(
                    "E101",
                    expr.span,
                    format!("{name} is not a function value"),
                ));
            };
            if params.len() != args.len() {
                return Err(Diagnostic::new(
                    "E105",
                    expr.span,
                    format!(
                        "{name} expects {} arguments, got {}",
                        params.len(),
                        args.len()
                    ),
                ));
            }
            for (arg, expected) in args.iter().zip(&params) {
                let actual =
                    type_expected(arg, Some(expected), env, signatures, return_type, types)?;
                require(expected, &actual, arg.span, "argument")?;
            }
            Ok(*ret)
        }
        ExprKind::Call(name, args) => {
            let signature = signatures.get(name).ok_or_else(|| {
                Diagnostic::new("E101", expr.span, format!("unknown function {name}"))
            })?;
            if signature.variants.is_some() {
                return Err(Diagnostic::new(
                    "E114",
                    expr.span,
                    "enum values require a qualified variant",
                ));
            }
            if signature.params.len() != args.len() {
                return Err(Diagnostic::new(
                    "E105",
                    expr.span,
                    format!(
                        "{name} expects {} arguments, got {}",
                        signature.params.len(),
                        args.len()
                    ),
                ));
            }
            let mut actuals = Vec::with_capacity(args.len());
            let mut deferred = Vec::new();
            for (index, arg) in args.iter().enumerate() {
                if needs_expected_type(arg) {
                    deferred.push(index);
                    actuals.push(Type::Never);
                } else {
                    actuals.push(type_of(arg, env, signatures, return_type, types)?);
                }
            }
            if !deferred.is_empty() {
                let mut known = HashMap::new();
                for (index, (expected, actual)) in signature.params.iter().zip(&actuals).enumerate()
                {
                    if !deferred.contains(&index) {
                        infer_params(expected, actual, &mut known, args[index].span)?;
                    }
                }
                for index in deferred {
                    let expected = substitute(&signature.params[index], &known);
                    actuals[index] = type_expected(
                        &args[index],
                        Some(&expected),
                        env,
                        signatures,
                        return_type,
                        types,
                    )?;
                    infer_params(
                        &signature.params[index],
                        &actuals[index],
                        &mut known,
                        args[index].span,
                    )?;
                }
            }
            if name == builtins::LEN && actuals == [Type::Bytes] {
                return Ok(Type::I32);
            }
            if name == builtins::CONTAINS
                && let Some(Type::Array(element)) = actuals.first()
            {
                if !builtins::equatable(element) {
                    return Err(Diagnostic::new(
                        "E104",
                        args[0].span,
                        format!("contains cannot compare {element} elements"),
                    ));
                }
                require(element, &actuals[1], args[1].span, "argument")?;
                return Ok(Type::Bool);
            }
            if name == builtins::SORT
                && let Some(Type::Array(element)) = actuals.first()
                && !builtins::orderable(element)
            {
                return Err(Diagnostic::new(
                    "E104",
                    args[0].span,
                    format!("sort cannot order {element} elements"),
                ));
            }
            if let Some(Type::Applied(map, entry)) = actuals.first()
                && map == builtins::MAP
            {
                if name == builtins::LEN {
                    return Ok(Type::I32);
                }
                if name == builtins::CONTAINS {
                    require(&entry[0], &actuals[1], args[1].span, "argument")?;
                    return Ok(Type::Bool);
                }
            }
            if actuals == [Type::F64] {
                if name == builtins::NARROW_I32 {
                    return Ok(Type::Option(Box::new(Type::I32)));
                }
                if name == builtins::WIDEN_I64 {
                    return Ok(Type::Option(Box::new(Type::I64)));
                }
            }
            if name == builtins::TO_F64 && actuals == [Type::I64] {
                return Ok(Type::F64);
            }
            match (name.as_str(), actuals.as_slice()) {
                (builtins::ABS, [ty @ (Type::I64 | Type::F64)]) => return Ok(ty.clone()),
                (builtins::MIN | builtins::MAX, [left @ (Type::I64 | Type::F64), right])
                    if left == right =>
                {
                    return Ok(left.clone());
                }
                (builtins::POW, [Type::I64, Type::I32]) => return Ok(Type::I64),
                (
                    builtins::BIT_AND | builtins::BIT_OR | builtins::BIT_XOR,
                    [Type::I64, Type::I64],
                )
                | (builtins::BIT_NOT, [Type::I64])
                | (builtins::SHL | builtins::SHR, [Type::I64, Type::I32]) => return Ok(Type::I64),
                (builtins::POW, [Type::F64, Type::F64]) => return Ok(Type::F64),
                _ => {}
            }
            if name == builtins::TO_STRING
                && matches!(actuals[..], [Type::I64 | Type::F64 | Type::Bool])
            {
                return Ok(Type::String);
            }
            let mut inferred = HashMap::new();
            for ((arg, expected), actual) in args.iter().zip(&signature.params).zip(&actuals) {
                infer_params(expected, actual, &mut inferred, arg.span)?;
            }
            for name in &signature.type_params {
                if !inferred.contains_key(name) {
                    return Err(Diagnostic::new(
                        "E115",
                        expr.span,
                        format!("cannot infer type argument {name}"),
                    ));
                }
            }
            for ((arg, expected), actual) in args.iter().zip(&signature.params).zip(&actuals) {
                require(
                    &substitute(expected, &inferred),
                    actual,
                    arg.span,
                    "argument",
                )?;
            }
            if name == builtins::SORT_BY
                && let Some(key) = inferred.get("K")
                && !builtins::orderable(key)
            {
                return Err(Diagnostic::new(
                    "E104",
                    args[1].span,
                    format!("sort_by cannot order {key} keys"),
                ));
            }
            Ok(substitute(&signature.ret, &inferred))
        }
        ExprKind::Spawn(call) => {
            let ExprKind::Call(name, _) = &call.kind else {
                return Err(Diagnostic::new(
                    "E117",
                    call.span,
                    "spawn requires a named function call",
                ));
            };
            let result = type_of(call, env, signatures, return_type, types)?;
            if !signatures.get(name).is_some_and(|signature| {
                signature.spawn_safe
                    && !signature
                        .params
                        .iter()
                        .any(|param| carries_function(param, signatures, &mut HashSet::new()))
            }) {
                return Err(Diagnostic::new(
                    "E117",
                    expr.span,
                    format!(
                        "{name} cannot be spawned: only non-generic pure functions are supported"
                    ),
                ));
            }
            Ok(Type::Task(Box::new(result)))
        }
        ExprKind::If(condition, yes, no) => {
            let cond_type = type_of(condition, env, signatures, return_type, types)?;
            require(&Type::Bool, &cond_type, condition.span, "condition")?;
            let yes_type = type_of(yes, env, signatures, return_type, types)?;
            let no_type = type_of(no, env, signatures, return_type, types)?;
            join(&yes_type, &no_type).ok_or_else(|| {
                let message = if no.span.start == no.span.end {
                    format!("if without else must have type Unit, found {yes_type}")
                } else {
                    format!("branches have different types: {yes_type} and {no_type}")
                };
                Diagnostic::new("E102", expr.span, message)
            })
        }
        ExprKind::Match(value, arms) => {
            let matched = type_of(value, env, signatures, return_type, types)?;
            if matched == Type::Never {
                return Ok(Type::Never);
            }
            if matched == Type::Option(Box::new(Type::Never)) {
                return Err(Diagnostic::new(
                    "E115",
                    value.span,
                    "cannot infer Option element type for match",
                ));
            }
            // Integer and string domains cannot be listed, so they need `_`.
            let integer_match = matches!(matched, Type::I32 | Type::I64 | Type::String);
            let expected: HashSet<String> = match &matched {
                Type::I32 | Type::I64 | Type::String => HashSet::new(),
                Type::Result(_, _) => ["Ok".to_owned(), "Err".to_owned()].into_iter().collect(),
                Type::Option(_) => ["Some".to_owned(), "None".to_owned()].into_iter().collect(),
                Type::Bool => ["true".to_owned(), "false".to_owned()]
                    .into_iter()
                    .collect(),
                Type::Named(name) | Type::Applied(name, _) => signatures
                    .get(name)
                    .and_then(|signature| signature.variants.as_ref())
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "E116",
                            value.span,
                            "match requires integer, string, result, enum, or bool",
                        )
                    })?
                    .iter()
                    .map(|variant| variant.name.clone())
                    .collect(),
                _ => {
                    return Err(Diagnostic::new(
                        "E116",
                        value.span,
                        "match requires integer, string, result, enum, or bool",
                    ));
                }
            };
            let mut seen = HashSet::new();
            let mut wildcard = false;
            let mut result = None;
            for (pattern, body) in arms {
                let (key, binding) = match_pattern(pattern, &matched, signatures)?;
                if wildcard {
                    return Err(Diagnostic::new(
                        "E116",
                        pattern.span,
                        "unreachable match arm after wildcard",
                    ));
                }
                if matches!(&pattern.kind, PatternKind::Wildcard) {
                    if !integer_match && seen == expected {
                        return Err(Diagnostic::new(
                            "E116",
                            pattern.span,
                            "wildcard arm is unreachable",
                        ));
                    }
                    wildcard = true;
                } else if !seen.insert(key.clone()) {
                    return Err(Diagnostic::new(
                        "E116",
                        pattern.span,
                        format!("duplicate match arm {key}"),
                    ));
                }
                let mut scope = env.clone();
                if let Some((name, ty)) = binding {
                    scope.insert(name, Binding { ty, mutable: false });
                }
                let branch = type_of(body, &scope, signatures, return_type, types)?;
                result = Some(match result {
                    Some(previous) => join(&previous, &branch).ok_or_else(|| {
                        Diagnostic::new(
                            "E102",
                            body.span,
                            format!("match arms have different types: {previous} and {branch}"),
                        )
                    })?,
                    None => branch,
                });
            }
            if !wildcard && (integer_match || seen != expected) {
                let mut missing: Vec<_> = expected.difference(&seen).cloned().collect();
                if integer_match {
                    missing.push("_".to_owned());
                }
                missing.sort();
                return Err(Diagnostic::new(
                    "E116",
                    expr.span,
                    format!("non-exhaustive match: missing {}", missing.join(",")),
                ));
            }
            Ok(result.unwrap_or(Type::Never))
        }
        ExprKind::Block(stmts, tail) => {
            let mut scope = env.clone();
            let mut declared = HashSet::new();
            for (index, stmt) in stmts.iter().enumerate() {
                let has_following = index + 1 < stmts.len() || tail.is_some();
                let (statement_type, span) = match stmt {
                    Stmt::Let {
                        name,
                        ty,
                        value,
                        mutable,
                        span,
                    } => {
                        if !declared.insert(name.clone()) {
                            return Err(Diagnostic::new(
                                "E106",
                                *span,
                                format!("duplicate binding {name}"),
                            ));
                        }
                        let actual = type_expected(
                            value,
                            ty.as_ref(),
                            &scope,
                            signatures,
                            return_type,
                            types,
                        )?;
                        let mut arities: HashMap<String, usize> = signatures
                            .iter()
                            .filter_map(|(name, signature)| {
                                (signature.fields.is_some() || signature.variants.is_some())
                                    .then_some((name.clone(), signature.type_params.len()))
                            })
                            .collect();
                        arities.insert(builtins::MAP.to_owned(), 2);
                        arities.insert(builtins::CONN.to_owned(), 0);
                        arities.insert(builtins::LISTENER.to_owned(), 0);
                        let binding_ty = if let Some(ty) = ty {
                            validate_type(ty, &arities, *span)?;
                            require(ty, &actual, value.span, "binding")?;
                            ty.clone()
                        } else if binding_type_is_known(&actual) {
                            actual.clone()
                        } else {
                            return Err(Diagnostic::new(
                                "E115",
                                value.span,
                                "binding needs a type annotation for this value",
                            ));
                        };
                        scope.insert(
                            name.clone(),
                            Binding {
                                ty: binding_ty,
                                mutable: *mutable,
                            },
                        );
                        (actual, *span)
                    }
                    Stmt::Assign {
                        name,
                        path,
                        value,
                        span,
                    } => {
                        let binding = scope.get(name).ok_or_else(|| {
                            Diagnostic::new("E101", *span, format!("unknown name {name}"))
                        })?;
                        if !binding.mutable {
                            return Err(Diagnostic::new(
                                "E109",
                                *span,
                                format!("cannot assign immutable binding {name}"),
                            ));
                        }
                        let mut target = binding.ty.clone();
                        for step in path {
                            // The backends read each step's container type.
                            let container = target;
                            target = match step {
                                PlaceStep::Index(index, step_span) => {
                                    let index_type =
                                        type_of(index, &scope, signatures, return_type, types)?;
                                    types.insert(*step_span, container.clone());
                                    if let Type::Applied(map, entry) = &container
                                        && map == builtins::MAP
                                    {
                                        if !std::ptr::eq(step, path.last().expect("non-empty path"))
                                        {
                                            return Err(Diagnostic::new(
                                                "E110",
                                                *step_span,
                                                "a map entry must be the last assignment step",
                                            ));
                                        }
                                        require(&entry[0], &index_type, index.span, "map key")?;
                                        entry[1].clone()
                                    } else {
                                        require(
                                            &Type::I32,
                                            &index_type,
                                            index.span,
                                            "array index",
                                        )?;
                                        match container {
                                            Type::Array(element) => *element,
                                            Type::Bytes => Type::I32,
                                            _ => {
                                                return Err(Diagnostic::new(
                                                    "E110",
                                                    *step_span,
                                                    "indexed assignment requires an array, Bytes, or Map",
                                                ));
                                            }
                                        }
                                    }
                                }
                                PlaceStep::Field(field, step_span) => {
                                    types.insert(*step_span, container.clone());
                                    field_type(container, field, signatures, *step_span)?
                                }
                            };
                        }
                        let actual = type_expected(
                            value,
                            Some(&target),
                            &scope,
                            signatures,
                            return_type,
                            types,
                        )?;
                        require(&target, &actual, value.span, "assignment")?;
                        (actual, *span)
                    }
                    Stmt::Push { name, value, span } => {
                        let binding = scope.get(name).ok_or_else(|| {
                            Diagnostic::new("E101", *span, format!("unknown name {name}"))
                        })?;
                        if !binding.mutable {
                            return Err(Diagnostic::new(
                                "E109",
                                *span,
                                format!("cannot push to immutable binding {name}"),
                            ));
                        }
                        let element = match &binding.ty {
                            Type::Array(element) => element.as_ref(),
                            Type::Bytes => &Type::I32,
                            Type::String => &Type::String,
                            _ => {
                                return Err(Diagnostic::new(
                                    "E110",
                                    *span,
                                    format!(
                                        "push requires an array, Bytes, or String, got {}",
                                        binding.ty
                                    ),
                                ));
                            }
                        };
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        require(element, &actual, value.span, "append value")?;
                        types.insert(*span, binding.ty.clone());
                        (Type::Unit, *span)
                    }
                    Stmt::For {
                        name,
                        iterable,
                        body,
                        span,
                    } => {
                        let iter_type = type_of(iterable, &scope, signatures, return_type, types)?;
                        let element = match iter_type {
                            Type::Array(element) => *element,
                            Type::Bytes => Type::I32,
                            _ => {
                                return Err(Diagnostic::new(
                                    "E110",
                                    iterable.span,
                                    format!("for requires an array or Bytes, got {iter_type}"),
                                ));
                            }
                        };
                        let mut loop_scope = scope.clone();
                        loop_scope.insert(
                            name.clone(),
                            Binding {
                                ty: element,
                                mutable: false,
                            },
                        );
                        type_of(body, &loop_scope, signatures, return_type, types)?;
                        (Type::Unit, *span)
                    }
                    Stmt::While {
                        condition,
                        body,
                        span,
                    } => {
                        let condition_type =
                            type_of(condition, &scope, signatures, return_type, types)?;
                        require(
                            &Type::Bool,
                            &condition_type,
                            condition.span,
                            "while condition",
                        )?;
                        type_of(body, &scope, signatures, return_type, types)?;
                        (Type::Unit, *span)
                    }
                    Stmt::Return { value, span } => {
                        let actual = type_of(value, &scope, signatures, return_type, types)?;
                        require(return_type, &actual, value.span, "return")?;
                        (Type::Never, *span)
                    }
                    Stmt::Break { span } | Stmt::Continue { span } => (Type::Never, *span),
                    Stmt::Expr(value) => (
                        type_of(value, &scope, signatures, return_type, types)?,
                        value.span,
                    ),
                };
                if statement_type == Type::Never {
                    if has_following {
                        return Err(Diagnostic::new("E107", span, "unreachable code"));
                    }
                    return Ok(Type::Never);
                }
            }
            if let Some(tail) = tail {
                type_of(tail, &scope, signatures, return_type, types)
            } else {
                Ok(Type::Unit)
            }
        }
    }
}

/// The type of `field` on a record value of type `record`.
fn field_type(
    record: Type,
    field: &str,
    signatures: &HashMap<String, Signature>,
    span: Span,
) -> Result<Type, Diagnostic> {
    let (name, args) = match record {
        Type::Named(name) => (name, Vec::new()),
        Type::Applied(name, args) => (name, args),
        _ => {
            return Err(Diagnostic::new(
                "E113",
                span,
                "field access requires a record",
            ));
        }
    };
    let signature = signatures
        .get(&name)
        .ok_or_else(|| Diagnostic::new("E113", span, "unknown record"))?;
    let inferred: HashMap<String, Type> = signature.type_params.iter().cloned().zip(args).collect();
    signature
        .fields
        .as_ref()
        .and_then(|fields| fields.iter().find(|(candidate, _)| candidate == field))
        .map(|(_, ty)| substitute(ty, &inferred))
        .ok_or_else(|| Diagnostic::new("E113", span, format!("unknown field {field} on {name}")))
}

/// A lambda whose parameter types come from its context.
fn needs_expected_type(expr: &Expr) -> bool {
    matches!(&expr.kind, ExprKind::Lambda(params, _) if params.iter().any(|(_, ty)| ty.is_none()))
}

/// Type an expression, letting a lambda take parameter types from `expected`.
fn type_expected(
    expr: &Expr,
    expected: Option<&Type>,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    return_type: &Type,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    if matches!(expr.kind, ExprKind::Lambda(..)) {
        type_lambda(expr, expected, env, signatures, types)
    } else {
        type_of(expr, env, signatures, return_type, types)
    }
}

fn type_lambda(
    expr: &Expr,
    expected: Option<&Type>,
    env: &HashMap<String, Binding>,
    signatures: &HashMap<String, Signature>,
    types: &mut HashMap<Span, Type>,
) -> Result<Type, Diagnostic> {
    let ExprKind::Lambda(params, body) = &expr.kind else {
        unreachable!("type_lambda requires a lambda")
    };
    let expected_params = match expected {
        Some(Type::Fn(expected_params, _)) if expected_params.len() == params.len() => {
            Some(expected_params)
        }
        _ => None,
    };
    if let Some(span) = escaping_control(body) {
        return Err(Diagnostic::new(
            "E111",
            span,
            "return and ? cannot leave a lambda; the lambda's value is its body",
        ));
    }
    let mut arities: HashMap<String, usize> = signatures
        .iter()
        .filter_map(|(name, signature)| {
            (signature.fields.is_some() || signature.variants.is_some())
                .then_some((name.clone(), signature.type_params.len()))
        })
        .collect();
    arities.insert(builtins::MAP.to_owned(), 2);
    arities.insert(builtins::CONN.to_owned(), 0);
    arities.insert(builtins::LISTENER.to_owned(), 0);
    // Captured bindings are copies and cannot be reassigned inside the lambda.
    let mut scope: HashMap<String, Binding> = env
        .iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                Binding {
                    ty: binding.ty.clone(),
                    mutable: false,
                },
            )
        })
        .collect();
    let mut param_types = Vec::with_capacity(params.len());
    for (index, (name, annotated)) in params.iter().enumerate() {
        let contextual = expected_params
            .map(|types| &types[index])
            .filter(|ty| binding_type_is_known(ty));
        let ty = match (annotated, contextual) {
            (Some(ty), contextual) => {
                validate_type(ty, &arities, expr.span)?;
                if let Some(contextual) = contextual {
                    require(contextual, ty, expr.span, "lambda parameter")?;
                }
                ty.clone()
            }
            (None, Some(ty)) => ty.clone(),
            (None, None) => {
                return Err(Diagnostic::new(
                    "E115",
                    expr.span,
                    format!("lambda parameter {name} needs a type annotation"),
                ));
            }
        };
        if params[..index].iter().any(|(other, _)| other == name) {
            return Err(Diagnostic::new(
                "E106",
                expr.span,
                format!("duplicate lambda parameter {name}"),
            ));
        }
        scope.insert(
            name.clone(),
            Binding {
                ty: ty.clone(),
                mutable: false,
            },
        );
        param_types.push(ty);
    }
    let ret = type_of(body, &scope, signatures, &Type::Never, types)?;
    let ty = Type::Fn(param_types, Box::new(ret));
    types.insert(expr.span, ty.clone());
    Ok(ty)
}

/// The span of a `return` or `?` that would leave a lambda body.
fn escaping_control(expr: &Expr) -> Option<Span> {
    let found = std::cell::Cell::new(None);
    visit_own_body(
        expr,
        &mut |node| {
            if found.get().is_none() && matches!(node.kind, ExprKind::Try(_)) {
                found.set(Some(node.span));
            }
        },
        &mut |stmt| {
            if found.get().is_none()
                && let Stmt::Return { span, .. } = stmt
            {
                found.set(Some(*span));
            }
        },
    );
    found.get()
}

/// Visit expressions and statements of a body, not entering nested lambdas.
fn visit_own_body(expr: &Expr, on_expr: &mut dyn FnMut(&Expr), on_stmt: &mut dyn FnMut(&Stmt)) {
    on_expr(expr);
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::I64(_)
        | ExprKind::F64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_)
        | ExprKind::None
        | ExprKind::Lambda(..) => {}
        ExprKind::Array(items) | ExprKind::Call(_, items) => {
            items
                .iter()
                .for_each(|item| visit_own_body(item, on_expr, on_stmt));
        }
        ExprKind::Apply(callee, args) => {
            visit_own_body(callee, on_expr, on_stmt);
            args.iter()
                .for_each(|arg| visit_own_body(arg, on_expr, on_stmt));
        }
        ExprKind::Index(left, right) | ExprKind::Binary(left, _, right) => {
            visit_own_body(left, on_expr, on_stmt);
            visit_own_body(right, on_expr, on_stmt);
        }
        ExprKind::Field(inner, _)
        | ExprKind::Ok(inner)
        | ExprKind::Err(inner)
        | ExprKind::Some(inner)
        | ExprKind::Try(inner)
        | ExprKind::Not(inner)
        | ExprKind::Neg(inner)
        | ExprKind::Spawn(inner) => visit_own_body(inner, on_expr, on_stmt),
        ExprKind::Variant(_, _, payload) => {
            if let Some(payload) = payload {
                visit_own_body(payload, on_expr, on_stmt);
            }
        }
        ExprKind::If(condition, yes, no) => {
            visit_own_body(condition, on_expr, on_stmt);
            visit_own_body(yes, on_expr, on_stmt);
            visit_own_body(no, on_expr, on_stmt);
        }
        ExprKind::Match(value, arms) => {
            visit_own_body(value, on_expr, on_stmt);
            arms.iter()
                .for_each(|(_, body)| visit_own_body(body, on_expr, on_stmt));
        }
        ExprKind::Block(stmts, tail) => {
            for stmt in stmts {
                on_stmt(stmt);
                match stmt {
                    Stmt::Let { value, .. }
                    | Stmt::Push { value, .. }
                    | Stmt::Return { value, .. }
                    | Stmt::Expr(value) => visit_own_body(value, on_expr, on_stmt),
                    Stmt::Assign { path, value, .. } => {
                        for step in path {
                            if let PlaceStep::Index(index, _) = step {
                                visit_own_body(index, on_expr, on_stmt);
                            }
                        }
                        visit_own_body(value, on_expr, on_stmt);
                    }
                    Stmt::For { iterable, body, .. } => {
                        visit_own_body(iterable, on_expr, on_stmt);
                        visit_own_body(body, on_expr, on_stmt);
                    }
                    Stmt::While {
                        condition, body, ..
                    } => {
                        visit_own_body(condition, on_expr, on_stmt);
                        visit_own_body(body, on_expr, on_stmt);
                    }
                    Stmt::Break { .. } | Stmt::Continue { .. } => {}
                }
            }
            if let Some(tail) = tail {
                visit_own_body(tail, on_expr, on_stmt);
            }
        }
    }
}

/// Whether a value of this type can carry a function, which a task must not receive.
fn carries_function(
    ty: &Type,
    signatures: &HashMap<String, Signature>,
    visiting: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Fn(..) => true,
        Type::Array(inner) | Type::Option(inner) | Type::Task(inner) => {
            carries_function(inner, signatures, visiting)
        }
        Type::Result(ok, err) => {
            carries_function(ok, signatures, visiting)
                || carries_function(err, signatures, visiting)
        }
        Type::Named(name) | Type::Applied(name, _) => {
            let args = match ty {
                Type::Applied(_, args) => args.as_slice(),
                _ => &[],
            };
            if args
                .iter()
                .any(|arg| carries_function(arg, signatures, visiting))
            {
                return true;
            }
            if !visiting.insert(name.clone()) {
                return false;
            }
            let Some(signature) = signatures.get(name) else {
                return false;
            };
            let fields = signature
                .fields
                .iter()
                .flatten()
                .any(|(_, field)| carries_function(field, signatures, visiting));
            let payloads = signature
                .variants
                .iter()
                .flatten()
                .filter_map(|variant| variant.payload.as_ref())
                .any(|payload| carries_function(payload, signatures, visiting));
            fields || payloads
        }
        _ => false,
    }
}

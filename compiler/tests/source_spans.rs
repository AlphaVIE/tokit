use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::ast::{SourceId, Span};
use tokit_compiler::sources::SourceMap;
use tokit_compiler::{checker, interpreter, lexer, native, parse_in_source};

#[test]
fn lexing_and_parsing_preserve_source_identity() {
    let first = SourceId(1);
    let second = SourceId(2);
    let left = parse_in_source("fn f()->i32{1}", first).unwrap();
    let right = parse_in_source("fn g()->i32{2}", second).unwrap();
    assert_eq!(left.functions[0].body.span.source_id, first);
    assert_eq!(right.functions[0].body.span.source_id, second);
    assert_eq!(
        left.functions[0].body.span.start,
        right.functions[0].body.span.start
    );
    assert_ne!(left.functions[0].body.span, right.functions[0].body.span);
    let diagnostic = lexer::lex_in_source("@", second).unwrap_err();
    assert_eq!(diagnostic.span.source_id, second);
}

#[test]
fn checked_expression_types_keep_equal_offsets_in_distinct_sources() {
    let mut first = parse_in_source("fn f()->i32{1}", SourceId(1)).unwrap();
    let second = parse_in_source("fn g()->i32{2}", SourceId(2)).unwrap();
    let offset = first.functions[0].body.span.start;
    first.functions.extend(second.functions);
    let types = checker::check_with_types(&first).unwrap();
    assert!(types.contains_key(&Span::in_source(SourceId(1), offset, offset + 3)));
    assert!(types.contains_key(&Span::in_source(SourceId(2), offset, offset + 3)));
}

#[test]
fn diagnostics_select_the_registered_source_text_and_path() {
    let mut sources = SourceMap::new();
    let root = sources.push(
        PathBuf::from("main.tok"),
        "fn main()->i32{helper()}".to_owned(),
    );
    let library = sources.push(
        PathBuf::from("lib.tok"),
        "\nfn helper()->i32{true}".to_owned(),
    );
    let mut program = parse_in_source(&sources.get(root).unwrap().text, root).unwrap();
    program.functions.extend(
        parse_in_source(&sources.get(library).unwrap().text, library)
            .unwrap()
            .functions,
    );
    let diagnostic = checker::check(&program).unwrap_err();
    assert_eq!(diagnostic.span.source_id, library);
    assert!(
        diagnostic
            .display_with_sources(&sources)
            .starts_with("lib.tok:E102@2:")
    );
    assert!(
        diagnostic
            .json_with_sources(&sources)
            .contains("\"source\":\"lib.tok\"")
    );
}

#[test]
fn native_error_location_uses_the_expression_source() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let mut sources = SourceMap::new();
    let root = sources.push(
        PathBuf::from("main.tok"),
        "fn main()->i32{helper()}".to_owned(),
    );
    let library = sources.push(
        PathBuf::from("lib.tok"),
        "\nfn helper()->i32{2147483647+1}".to_owned(),
    );
    let mut program = parse_in_source(&sources.get(root).unwrap().text, root).unwrap();
    program.functions.extend(
        parse_in_source(&sources.get(library).unwrap().text, library)
            .unwrap()
            .functions,
    );
    checker::check(&program).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "tokit-source-map-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let binary = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
    native::build_with_sources(&program, &sources, &binary).unwrap();
    let output = Command::new(binary).output().unwrap();
    let diagnostic = interpreter::run(&program).unwrap_err();
    assert_eq!(diagnostic.span.source_id, library);
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        diagnostic.display(&sources.get(library).unwrap().text)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

use tokit_compiler::ast::{SourceId, Span};
use tokit_compiler::{checker, lexer, parse_in_source};

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

use std::path::Path;
use std::process::Command;

use tokit_compiler::{check, stats};

#[test]
fn counts_types_statements_expressions_and_patterns() {
    let source = "fn main()->i32{let x:i32=1;x+2}";
    let measured = stats::measure(source, &check(source).unwrap());
    assert_eq!(measured.ast_nodes, 10);
    assert_eq!(measured.semantic_ops, 2);
    assert_eq!(measured.functions, 1);
    assert_eq!(measured.declarations, 1);
    assert_eq!(measured.dependencies, 0);
    assert_eq!(measured.bytes, source.len());
    assert_eq!(measured.chars, source.chars().count());

    let source = "fn main()->i32{match true{true=>1,false=>2}}";
    let measured = stats::measure(source, &check(source).unwrap());
    assert_eq!(measured.ast_nodes, 10);
    assert_eq!(measured.semantic_ops, 1);
}

#[test]
fn stats_cli_emits_machine_readable_counts() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source_path = workspace.join("examples/match_result.tok");
    let source = std::fs::read_to_string(&source_path).unwrap();
    let expected = stats::measure(&source, &check(&source).unwrap()).json();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("stats")
        .arg(source_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
}

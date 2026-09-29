use std::path::Path;
use std::process::Command;

use tokit_compiler::{ai_index, check};

#[test]
fn index_lists_checked_declarations_and_transitive_file_grants() {
    let source = "struct Box<T>{value:T} enum Flag{Off,On(i32)} fn load()->Result<String,IoError>{read_text(\"x\")} fn wrap()->Result<String,IoError>{load()} fn main()->Result<String,IoError>{wrap()}";
    let index = ai_index::index(&check(source).unwrap());
    assert_eq!(index, ai_index::index(&check(source).unwrap()));
    assert!(index.starts_with("{\"version\":1,\"records\":["));
    assert!(index.contains("\"fields\":[[\"value\",\"T\"]]"));
    assert!(index.contains("\"variants\":[[\"Off\",null],[\"On\",\"i32\"]]"));
    assert!(index.contains("\"builtins\":[\"read_text\"]"));
    assert!(index.contains("\"calls\":[\"load\"]"));
    assert!(index.contains("\"calls\":[\"wrap\"]"));
    assert_eq!(index.matches("\"effects\":[\"fs.read\"]").count(), 3);
    assert_eq!(index.matches("\"direct_effects\":[\"fs.read\"]").count(), 1);
}

#[test]
fn recursive_call_cycle_propagates_environment_effect() {
    let source = "fn a(n:i32)->i32{if n==0{0}else{b(n-1)}} fn b(n:i32)->i32{if n==0{len(args())}else{a(n-1)}} fn main()->i32{a(1)}";
    let index = ai_index::index(&check(source).unwrap());
    assert_eq!(index.matches("\"effects\":[\"env.args\"]").count(), 3);
    assert_eq!(
        index.matches("\"direct_effects\":[\"env.args\"]").count(),
        1
    );
    assert!(index.contains("\"calls\":[\"b\"]"));
    assert!(index.contains("\"builtins\":[\"args\",\"len\"]"));
}

#[test]
fn task_creation_and_join_have_distinct_effects() {
    let source =
        "fn square(x:i32)->i32{x*x} fn main()->Result<i32,TaskError>{join(spawn square(7))}";
    let index = ai_index::index(&check(source).unwrap());
    assert!(index.contains("\"calls\":[\"square\"]"));
    assert!(index.contains("\"builtins\":[\"join\"]"));
    assert!(index.contains("\"direct_effects\":[\"task.join\",\"task.spawn\"]"));
    assert!(index.contains("\"effects\":[\"task.join\",\"task.spawn\"]"));
}

#[test]
fn cli_emits_index_only_after_checking() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["ai-index", "examples/line_count.tok"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let index = String::from_utf8(output.stdout).unwrap();
    assert!(index.contains("\"calls\":[\"line_count\"]"));
    assert!(index.contains("\"effects\":[\"fs.read\"]"));

    let invalid = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["ai-index", "examples/sample_lines.txt"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
}

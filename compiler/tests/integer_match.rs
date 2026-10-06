mod common;

use std::path::Path;
use std::process::Command;

use tokit_compiler::{check, run};

#[test]
fn integer_literals_and_final_wildcard_select_one_arm() {
    for (source, expected) in [
        ("fn main()->i32{match 0{0=>7,1=>8,_=>9}}", "7"),
        ("fn main()->i32{match 1{0=>7,1=>8,_=>9}}", "8"),
        ("fn main()->i32{match 0-1{0=>7,1=>8,_=>9}}", "9"),
        ("fn main()->i32{match 500{_=>3}}", "3"),
        ("fn main()->i32{match true{true=>7,_=>9}}", "7"),
        ("fn main()->i32{match false{true=>7,_=>9}}", "9"),
        (
            "enum Mode{A,B,C} fn main()->i32{match Mode::C{Mode::A=>1,_=>2}}",
            "2",
        ),
        ("fn main()->i32{match Some(5){Some(x)=>x,_=>0}}", "5"),
        ("fn main()->i32{match Ok(5){Ok(x)=>x,_=>0}}", "5"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
    }
}

#[test]
fn integer_match_rejects_missing_duplicate_and_unreachable_arms() {
    for (source, code) in [
        ("fn main()->i32{match 1{1=>1}}", "E116"),
        ("fn main()->i32{match 1{1=>1,1=>2,_=>3}}", "E116"),
        ("fn main()->i32{match 1{_=>1,1=>2}}", "E116"),
        ("fn main()->i32{match true{true=>1,false=>2,_=>3}}", "E116"),
        ("fn main()->i32{match true{1=>1,_=>2}}", "E116"),
        ("fn main()->i32{match 1{0=>1,_=>false}}", "E102"),
        ("fn main()->i32{match 1{2147483648=>1,_=>2}}", "E003"),
        ("fn main()->i32{match 1{0-1=>1,_=>2}}", "E002"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn native_integer_match_agrees_with_reference() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source = std::fs::read_to_string(root.join("examples/integer_match.tok")).unwrap();
    let expected = run(&source).unwrap().to_string();
    let directory = std::env::temp_dir().join(format!(
        "tokit-integer-match-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir(&directory).unwrap();
    let binary = directory.join(format!("integer_match{}", std::env::consts::EXE_SUFFIX));
    let build = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "build",
            root.join("examples/integer_match.tok").to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let output = Command::new(binary).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    std::fs::remove_dir_all(directory).unwrap();
}

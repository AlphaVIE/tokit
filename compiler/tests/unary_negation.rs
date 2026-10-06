mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn unary_negation_precedes_binary_arithmetic_and_checks_types() {
    for (source, expected) in [
        ("fn main()->i32{let x:i32=3;-x*2}", "-6"),
        ("fn main()->i32{let x:i32=3;-(x+2)}", "-5"),
        ("fn main()->i32{let xs:[i32]=[2];-xs[0]}", "-2"),
        ("fn main()->i32{--1}", "1"),
        ("fn negate(x:i32)->i32{-x} fn main()->i32{negate(7)}", "-7"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
    }
    assert_eq!(check("fn main()->i32{-true}").unwrap_err().code, "E104");
    assert_eq!(
        run("fn main()->i32{-(-2147483648)}").unwrap_err().code,
        "E201"
    );
}

#[test]
fn native_negation_matches_reference_success_and_overflow() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "tokit-negation-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir(&directory).unwrap();
    for (name, source) in [
        ("value", "fn main()->i32{let x:i32=7;-x}"),
        ("overflow", "fn main()->i32{-(-2147483648)}"),
    ] {
        let binary = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &binary).unwrap();
        let output = Command::new(binary).output().unwrap();
        match run(source) {
            Ok(value) => {
                assert!(output.status.success());
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap().trim(),
                    value.to_string()
                );
            }
            Err(error) => {
                assert!(!output.status.success());
                assert_eq!(
                    String::from_utf8_lossy(&output.stderr).trim(),
                    error.display(source)
                );
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

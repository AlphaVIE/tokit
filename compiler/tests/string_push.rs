mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn string_push_preserves_value_copies_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for (source, expected) in [
        (
            "fn main()->String{var text=\"\";text.push(\"Grüße\");text.push(\" 😀\");text}",
            "\"Grüße 😀\"",
        ),
        (
            "fn main()->String{var text=\"A\";let previous=text;text.push(\"B\");previous+\"|\"+text}",
            "\"A|AB\"",
        ),
        (
            "fn main()->String{var text=\"ab\";text.push(text);text}",
            "\"abab\"",
        ),
        (
            "fn main()->String{var text=\"x\";text.push(if true{\"y\"}else{\"z\"});text}",
            "\"xy\"",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if !native_available {
            continue;
        }
        let nonce = common::nonce();
        let output = std::env::temp_dir().join(format!(
            "tokit-string-push-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::remove_file(output).unwrap();
    }
}

#[test]
fn string_push_requires_mutable_string_and_string_piece() {
    for (source, code) in [
        (
            "fn main()->String{let text=\"\";text.push(\"x\");text}",
            "E109",
        ),
        ("fn main()->String{var text=\"\";text.push(1);text}", "E102"),
        (
            "fn main()->i32{var value=1;value.push(\"x\");value}",
            "E110",
        ),
        ("fn main()->String{missing.push(\"x\");\"\"}", "E101"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

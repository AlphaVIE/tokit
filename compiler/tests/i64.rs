use std::process::Command;

use tokit_compiler::{check, format, native, run, run_with_runtime_args};

fn native_output(source: &str) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "tokit-i64-{}-{nonce}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    std::fs::remove_file(output).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

#[test]
fn wide_literals_arithmetic_match_and_parse_agree_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for (source, expected) in [
        (
            "fn main()->i64{9223372036854775807i64}",
            "9223372036854775807",
        ),
        (
            "fn main()->i64{-9223372036854775808i64}",
            "-9223372036854775808",
        ),
        ("fn main()->i64{3000000000i64+7i64}", "3000000007"),
        ("fn main()->i64{-(1i64+2i64)}", "-3"),
        ("fn main()->i64{if 7i64>6i64{9i64/3i64}else{0i64}}", "3"),
        (
            "fn double(x:i64)->i64{x*2i64} fn main()->i64{double(9i64)}",
            "18",
        ),
        (
            "struct Box<T>{value:T} fn main()->Box<i64>{Box(9i64)}",
            "Box(value:9)",
        ),
        (
            "fn answer()->i64{42i64} fn main()->Result<i64,TaskError>{join(spawn answer())}",
            "Ok(42)",
        ),
        (
            "fn main()->i64{let xs:[i64]=[8i64,13i64];xs[len(xs)-1]}",
            "13",
        ),
        (
            "fn main()->i64{let xs=[1,2,3];i64(len(xs))+3000000000i64}",
            "3000000003",
        ),
        (
            "fn main()->Option<i32>{i32(2147483647i64)}",
            "Some(2147483647)",
        ),
        ("fn main()->Option<i32>{i32(2147483648i64)}", "None"),
        ("fn main()->i64{match -1i64{-1i64=>7i64,_=>0i64}}", "7"),
        (
            "fn main()->Result<i64,ParseError>{parse_i64(\"+9223372036854775807\")}",
            "Ok(9223372036854775807)",
        ),
        (
            "fn main()->Result<i64,ParseError>{parse_i64(\"9223372036854775808\")}",
            "Err(ParseError::OutOfRange)",
        ),
        (
            "fn main()->Result<i64,ParseError>{parse_i64(\"12x\")}",
            "Err(ParseError::Invalid)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn wide_cli_arguments_and_formatter_preserve_the_literal_contract() {
    let source = include_str!("../../examples/i64_counter.tok");
    let args = vec!["3000000000".to_owned()];
    assert_eq!(
        run_with_runtime_args(source, None, &args)
            .unwrap()
            .to_string(),
        "Ok(3000000001)"
    );
    let formatted = format::format("fn main()->i64{-9223372036854775808i64}").unwrap();
    assert!(formatted.contains("-9223372036854775808i64"));
    assert_eq!(run(&formatted).unwrap().to_string(), "-9223372036854775808");
}

#[test]
fn wide_integers_reject_mixed_types_and_invalid_literals() {
    for source in [
        "fn main()->i64{1i64+1}",
        "fn main()->i64{i64(1i64)}",
        "fn main()->Option<i32>{i32(1)}",
        "fn main()->i32{1i64}",
        "fn main()->i64{match 1i64{1=>2i64,_=>3i64}}",
        "fn main()->i64{9223372036854775808i64}",
        "fn main()->i64{-9223372036854775809i64}",
        "fn main()->i64{- 1i64}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
    let source = "fn main()->i64{9223372036854775808i64}";
    assert_eq!(check(source).unwrap_err().code, "E003");
}

#[test]
fn wide_arithmetic_failures_keep_checked_e201_diagnostics() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for source in [
        "fn main()->i64{9223372036854775807i64+1i64}",
        "fn main()->i64{-(0i64-9223372036854775807i64-1i64)}",
        "fn main()->i64{9i64/0i64}",
        "fn main()->i64{-9223372036854775808i64/-1i64}",
    ] {
        let error = run(source).unwrap_err().display(source);
        assert!(error.starts_with("E201@"), "{error}");
        if native_available {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let output = std::env::temp_dir().join(format!(
                "tokit-i64-fail-{}-{nonce}{}",
                std::process::id(),
                std::env::consts::EXE_SUFFIX
            ));
            native::build(&check(source).unwrap(), source, &output).unwrap();
            let result = Command::new(&output).output().unwrap();
            std::fs::remove_file(output).unwrap();
            assert!(!result.status.success());
            assert_eq!(String::from_utf8_lossy(&result.stderr).trim(), error);
        }
    }
}

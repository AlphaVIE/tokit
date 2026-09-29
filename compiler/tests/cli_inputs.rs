use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, native, run, run_with_runtime_args};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("tokit-cli-input-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

#[test]
fn argument_parsing_agrees_across_interpreter_cli_and_native() {
    let source = include_str!("../../examples/parse_argument.tok");
    let cases = [
        (None, "Err(ParseError::Invalid)"),
        (Some(""), "Err(ParseError::Invalid)"),
        (Some("+42"), "Ok(42)"),
        (Some("-42"), "Ok(-42)"),
        (Some("0007"), "Ok(7)"),
        (Some("2147483647"), "Ok(2147483647)"),
        (Some("-2147483648"), "Ok(-2147483648)"),
        (Some("2147483648"), "Err(ParseError::OutOfRange)"),
        (Some("-2147483649"), "Err(ParseError::OutOfRange)"),
        (Some("12x"), "Err(ParseError::Invalid)"),
        (Some("999999999999x"), "Err(ParseError::Invalid)"),
        (Some(" 12"), "Err(ParseError::Invalid)"),
    ];
    let directory = temporary_directory();
    let source_path = directory.join("parse.tok");
    std::fs::write(&source_path, source).unwrap();
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    let executable = directory.join(format!("parse{}", std::env::consts::EXE_SUFFIX));
    if native_available {
        native::build(&check(source).unwrap(), source, &executable).unwrap();
    }
    for (arg, expected) in cases {
        let values = arg.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(source, None, &values)
                .unwrap()
                .to_string(),
            expected
        );
        let mut cli = Command::new(env!("CARGO_BIN_EXE_tok"));
        cli.arg("run").arg(&source_path);
        if let Some(value) = arg {
            cli.arg("--").arg(value);
        }
        let output = cli.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        if native_available {
            let mut command = Command::new(&executable);
            if let Some(value) = arg {
                command.arg("--").arg(value);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn array_length_and_parse_errors_are_typed() {
    assert_eq!(
        run("fn main()->i32{len([1,2,3])}").unwrap().to_string(),
        "3"
    );
    assert_eq!(
        run("fn count<T>(xs:[T])->i32{len(xs)} fn main()->i32{count([1,2])}")
            .unwrap()
            .to_string(),
        "2"
    );
    assert!(check("fn parse(x:String)->Result<i32,ParseError>{parse_i32(x)} fn main()->Task<Result<i32,ParseError>>{spawn parse(\"12\")}").is_ok());
    assert_eq!(
        run("fn main()->i32{match parse_i32(\"x\"){Ok(n)=>n,Err(error)=>match error{ParseError::Invalid=>1,ParseError::OutOfRange=>2}}}")
            .unwrap()
            .to_string(),
        "1"
    );
    for (source, code) in [
        ("fn main()->i32{len(1)}", "E115"),
        ("fn main()->Result<i32,ParseError>{parse_i32(1)}", "E102"),
        ("fn main()->i32{len([])}", "E115"),
        ("enum ParseError{Invalid} fn main()->i32{0}", "E106"),
        ("fn len(xs:[i32])->i32{0} fn main()->i32{0}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

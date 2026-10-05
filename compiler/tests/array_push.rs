use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, explain, native, run, run_with_runtime_args, stats};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("tokit-push-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

#[test]
fn push_builds_arrays_with_value_semantics() {
    for (source, expected) in [
        (
            "fn main()->[i32]{var xs:[i32]=[];xs.push(1);xs.push(xs[0]+1);xs}",
            "[1,2]",
        ),
        (
            "fn main()->[i32]{var xs:[i32]=[1];let copy:[i32]=xs;xs.push(2);copy}",
            "[1]",
        ),
        (
            "fn main()->[i32]{var xs:[i32]=[1,2];for x in xs{xs.push(x);}xs}",
            "[1,2,1,2]",
        ),
        (
            "fn make(x:i32)->[i32]{var xs:[i32]=[];xs.push(x);xs} fn main()->Result<[i32],TaskError>{join(spawn make(7))}",
            "Ok([7])",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected);
        if Command::new("rustc").arg("--version").output().is_err() {
            assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
            continue;
        }
        let directory = temporary_directory();
        let executable = directory.join(format!("push{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &executable).unwrap();
        let output = Command::new(&executable).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn indexed_reads_preserve_array_copy_and_element_semantics() {
    let source = "fn make()->[String]{[\"made\"]} fn main()->[String]{var xs:[String]=[\"first\"];let copy=xs;xs.push(\"second\");[copy[0],xs[1],make()[0]]}";
    let expected = "[\"first\",\"second\",\"made\"]";
    assert_eq!(run(source).unwrap().to_string(), expected);

    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let directory = temporary_directory();
    let executable = directory.join(format!("indexed-read{}", std::env::consts::EXE_SUFFIX));
    native::build(&check(source).unwrap(), source, &executable).unwrap();
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn parsed_arguments_can_fill_an_array() {
    let source = include_str!("../../examples/parse_numbers.tok");
    let directory = temporary_directory();
    let source_path = directory.join("numbers.tok");
    std::fs::write(&source_path, source).unwrap();
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    let executable = directory.join(format!("numbers{}", std::env::consts::EXE_SUFFIX));
    if native_available {
        native::build(&check(source).unwrap(), source, &executable).unwrap();
    }
    for (inputs, expected) in [
        (vec![], "Ok([])"),
        (vec!["2", "-3", "17"], "Ok([2,-3,17])"),
        (vec!["2", "bad"], "Err(ParseError::Invalid)"),
        (vec!["2147483648"], "Err(ParseError::OutOfRange)"),
    ] {
        let arguments = inputs
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            run_with_runtime_args(source, None, &arguments)
                .unwrap()
                .to_string(),
            expected
        );
        let mut cli = Command::new(env!("CARGO_BIN_EXE_tok"));
        cli.arg("run").arg(&source_path).arg("--").args(&inputs);
        let output = cli.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        if native_available {
            let output = Command::new(&executable)
                .arg("--")
                .args(&inputs)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        }
    }
    let program = check(source).unwrap();
    assert!(explain::explain(&program).contains("mutable append"));
    assert!(stats::measure(source, &program).semantic_ops >= 5);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn push_requires_mutable_typed_array_binding() {
    for (source, code) in [
        ("fn main()->[i32]{let xs:[i32]=[];xs.push(1);xs}", "E109"),
        ("fn main()->i32{var x:i32=1;x.push(2);x}", "E110"),
        (
            "fn main()->[i32]{var xs:[i32]=[];xs.push(\"x\");xs}",
            "E102",
        ),
        ("fn main()->i32{missing.push(1);0}", "E101"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

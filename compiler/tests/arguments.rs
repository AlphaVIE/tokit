use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, native, run, run_with_runtime_args};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("tokit-args-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

#[test]
fn program_arguments_have_one_typed_runtime_view() {
    let source = "fn main()->[String]{args()}";
    assert_eq!(run(source).unwrap().to_string(), "[]");
    let program_args = vec![
        "hello world".to_owned(),
        "ä😀".to_owned(),
        "--allow-read".to_owned(),
    ];
    let expected = "[\"hello world\",\"ä😀\",\"--allow-read\"]";
    assert_eq!(
        run_with_runtime_args(source, None, &program_args)
            .unwrap()
            .to_string(),
        expected
    );
    let directory = temporary_directory();
    let path = directory.join("args.tok");
    std::fs::write(&path, source).unwrap();
    let cli = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(&path)
        .arg("--")
        .args(&program_args)
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    assert_eq!(String::from_utf8(cli.stdout).unwrap().trim(), expected);
    let json = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg("--json")
        .arg(&path)
        .arg("--")
        .arg("quote\"")
        .output()
        .unwrap();
    assert!(json.status.success());
    assert_eq!(
        String::from_utf8(json.stdout).unwrap().trim(),
        "{\"ok\":true,\"result\":\"[\\\"quote\\\\\\\"\\\"]\"}"
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let executable = directory.join(format!("args{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &executable).unwrap();
        let output = Command::new(&executable)
            .arg("--")
            .args(&program_args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn read_grant_and_program_arguments_work_together() {
    let directory = temporary_directory();
    let file = directory.join("input.txt");
    std::fs::write(&file, "hello").unwrap();
    let source = "fn main()->Result<String,IoError>{read_text(args()[0])}";
    let path = directory.join("read.tok");
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg("--allow-read")
        .arg(&directory)
        .arg(&path)
        .arg("--")
        .arg(&file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "Ok(\"hello\")"
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let executable = directory.join(format!("reader{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &executable).unwrap();
        let output = Command::new(&executable)
            .arg("--allow-read")
            .arg(&directory)
            .arg("--")
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "Ok(\"hello\")"
        );
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn argument_access_is_reserved_and_not_spawn_safe() {
    for (source, code) in [
        ("fn args()->i32{0} fn main()->i32{0}", "E106"),
        ("fn main()->[String]{args(1)}", "E105"),
        (
            "fn grab()->[String]{args()} fn main()->Task<[String]>{spawn grab()}",
            "E117",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

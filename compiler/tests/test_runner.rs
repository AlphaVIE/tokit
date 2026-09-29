use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::test_runner::{Outcome, run};

fn temporary_directory() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("tokit-test-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

fn literal(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
    )
}

#[test]
fn discovered_tests_continue_after_false_error_and_typed_failure() {
    let source = "fn test_pass()->bool{true} fn test_false()->bool{false} fn test_error()->bool{2147483647+1==0} fn test_result()->Result<bool,i32>{Ok(true)} fn test_typed_failure()->Result<bool,i32>{Err(7)}";
    let report = run(source, None).unwrap();
    assert_eq!(report.cases.len(), 5);
    assert_eq!(report.failed(), 3);
    assert_eq!(report.cases[0].outcome, Outcome::Passed);
    assert_eq!(
        report.cases[1].outcome,
        Outcome::Failed("returned false".to_owned())
    );
    assert!(
        matches!(&report.cases[2].outcome, Outcome::Failed(message) if message.starts_with("E201@"))
    );
    assert_eq!(report.cases[3].outcome, Outcome::Passed);
    assert_eq!(
        report.cases[4].outcome,
        Outcome::Failed("returned Err(7)".to_owned())
    );
    assert!(report.display().ends_with("2 passed; 3 failed"));
}

#[test]
fn missing_tests_and_invalid_signatures_fail_before_execution() {
    assert_eq!(
        run("fn helper()->bool{true}", None).unwrap_err().code,
        "E203"
    );
    for source in [
        "fn test_arg(n:i32)->bool{true}",
        "fn test_number()->i32{1}",
        "fn test_generic<T>(x:T)->bool{true}",
    ] {
        assert_eq!(run(source, None).unwrap_err().code, "E203", "{source}");
    }
}

#[test]
fn cli_runs_mainless_tests_and_reports_failure_status() {
    let directory = temporary_directory();
    let path = directory.join("suite.tok");
    std::fs::write(&path, "fn test_yes()->bool{true} fn test_no()->bool{false}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["test", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "PASS test_yes\nFAIL test_no: returned false\n1 passed; 1 failed\n"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_read_test_needs_an_explicit_grant() {
    let directory = temporary_directory();
    let file = directory.join("input.txt");
    std::fs::write(&file, "alpha\nbeta\n").unwrap();
    let source = format!(
        "fn test_read()->bool{{match read_text({}){{Ok(text)=>len(lines(text))==2,Err(error)=>false}}}}",
        literal(&file)
    );
    assert_eq!(run(&source, None).unwrap().failed(), 1);
    assert_eq!(run(&source, Some(&directory)).unwrap().failed(), 0);
    let path = directory.join("suite.tok");
    std::fs::write(&path, &source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("test")
        .arg("--allow-read")
        .arg(&directory)
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("1 passed; 0 failed")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

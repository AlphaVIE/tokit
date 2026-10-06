//! examples/calculator.tok evaluates arguments and standard input the same way
//! in the interpreter and as a native executable.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CASES: [(&str, &str); 12] = [
    ("2*(3+4.5)", "15"),
    ("1+2*3", "7"),
    ("2^3^2", "512"),
    ("-(4-6)*2", "4"),
    ("7/2", "3.5"),
    ("10%4", "2"),
    ("0.1+0.2", "0.30000000000000004"),
    ("1/0", "error: division by zero"),
    ("2*(3", "error: missing )"),
    ("3 x", "error: unexpected x at column 3"),
    ("1..2", "error: bad number 1..2"),
    (")", "error: expected a number or ("),
];

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/calculator.tok")
}

fn run(program: &mut Command, input: Option<&str>) -> Output {
    let mut child = program
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    if let Some(input) = input {
        stdin.write_all(input.as_bytes()).unwrap();
    }
    drop(stdin);
    child.wait_with_output().unwrap()
}

fn exercise(command: impl Fn() -> Command) {
    for (expression, expected) in CASES {
        let output = run(command().args(["--", expression]), None);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
        assert_eq!(
            output.status.success(),
            !expected.starts_with("error"),
            "{expression}"
        );
    }
    let input: String = CASES
        .iter()
        .map(|(expression, _)| format!("{expression}\n"))
        .collect();
    let output = run(&mut command(), Some(&format!("{input}\n")));
    assert!(output.status.success());
    let expected: Vec<_> = CASES.iter().map(|(_, value)| *value).collect();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn calculator_agrees_in_both_backends() {
    exercise(|| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tok"));
        command.arg("run").arg(example());
        command
    });
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = std::env::temp_dir().join(format!(
            "tokit-calculator-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        let built = Command::new(env!("CARGO_BIN_EXE_tok"))
            .arg("build")
            .arg(example())
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        exercise(|| Command::new(&binary));
        std::fs::remove_file(binary).unwrap();
    }
}

//! `tok build --target wasm32-wasip1` produces WebAssembly modules that print
//! what the interpreter prints. Runs when `TOKIT_WASM_RUNNER` names a WASI
//! runtime (for example `wasmtime`); CI sets `TOKIT_REQUIRE_WASM=1`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn runner() -> Option<String> {
    let runner = std::env::var("TOKIT_WASM_RUNNER").ok();
    if runner.is_none() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_WASM").as_deref(),
            Ok("1"),
            "TOKIT_REQUIRE_WASM is set but TOKIT_WASM_RUNNER is not"
        );
    }
    runner
}

fn example(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples")
        .join(path)
}

fn tok(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn wasm_modules_match_the_interpreter() {
    let Some(runner) = runner() else {
        return;
    };
    let directory = std::env::temp_dir().join(format!("tokit-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    // Deep recursion, a pure task, closures, nested patterns, and arguments.
    let deep = directory.join("deep.tok");
    std::fs::write(
        &deep,
        "down(n:I)->I{if n==0{0}else{1+down(n-1)}}\nmain()->I{down(9000)}",
    )
    .unwrap();
    let cases: Vec<(PathBuf, Vec<&str>)> = vec![
        (example("answer.tok"), vec![]),
        (example("closures.tok"), vec![]),
        (example("task_square.tok"), vec![]),
        (example("generic_tree.tok"), vec![]),
        (example("calculator.tok"), vec!["2*(3+4.5)^2"]),
        (deep, vec![]),
    ];
    for (source, arguments) in cases {
        let module = directory.join(format!(
            "{}.wasm",
            source.file_stem().unwrap().to_string_lossy()
        ));
        let built = tok(&[
            "build".as_ref(),
            source.as_os_str(),
            "-o".as_ref(),
            module.as_os_str(),
            "--target".as_ref(),
            "wasm32-wasip1".as_ref(),
        ]);
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let mut interpreter = vec!["run".as_ref(), source.as_os_str()];
        if !arguments.is_empty() {
            interpreter.push("--".as_ref());
            interpreter.extend(arguments.iter().map(std::ffi::OsStr::new));
        }
        let expected = tok(&interpreter);
        let mut command = Command::new(&runner);
        // Programs accept arguments without `--`, which runners may consume.
        command.arg("run").arg(&module).args(&arguments);
        let actual = command.output().unwrap();
        assert!(
            actual.status.success(),
            "{}: {}",
            source.display(),
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&actual.stdout),
            String::from_utf8_lossy(&expected.stdout),
            "{}",
            source.display()
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

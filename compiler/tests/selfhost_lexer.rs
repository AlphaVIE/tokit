//! Bootstrap equivalence: the lexer written in Tokit must reproduce the Rust
//! lexer's token listing for every source in the repository and for errors.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tokit_compiler::{check, lexer, native};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "tok") {
            found.push(path);
        }
    }
}

fn piped(mut command: Command, input: &str) -> (Option<i32>, String) {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
    )
}

const INVALID: [&str; 8] = [
    "main()->I{1 & 2}",
    r#"main()->String{"a\q"}"#,
    r#"main()->String{"open"#,
    "main()->String{\"line\nbreak\"}",
    "main()->F{1e+}",
    "main()->I{é}",
    "main()->I{#}",
    r#""\"#,
];

#[test]
fn self_hosted_lexer_matches_the_rust_lexer() {
    let lexer_source = root().join("selfhost/lexer.tok");
    let mut files = Vec::new();
    sources(&root().join("examples"), &mut files);
    sources(&root().join("selfhost"), &mut files);
    files.sort();
    let mut inputs: Vec<(String, String)> = files
        .iter()
        .map(|path| {
            (
                path.display().to_string(),
                std::fs::read_to_string(path).unwrap(),
            )
        })
        .collect();
    inputs.extend(
        INVALID
            .iter()
            .map(|source| ((*source).to_owned(), (*source).to_owned())),
    );
    inputs.push((
        "tricky".to_owned(),
        "a.b 1.5 1.e3 2i64x 3i64 1e9 x//c\n\"ü\\n\" ->=>::|| |&& !=<=>=%".to_owned(),
    ));
    assert!(inputs.len() > 50, "too few sources: {}", inputs.len());

    let native_binary = if Command::new("rustc").arg("--version").output().is_ok() {
        let source = std::fs::read_to_string(&lexer_source).unwrap();
        let binary = std::env::temp_dir().join(format!(
            "tokit-selfhost-lexer-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(&source).unwrap(), &source, &binary).unwrap();
        Some(binary)
    } else {
        None
    };
    for (name, input) in &inputs {
        let expected = format!("{}\n", lexer::listing(input));
        let status = if expected.starts_with("error ") { 1 } else { 0 };
        let mut interpreter = Command::new(env!("CARGO_BIN_EXE_tok"));
        interpreter.arg("run").arg(&lexer_source);
        assert_eq!(
            piped(interpreter, input),
            (Some(status), expected.clone()),
            "{name}"
        );
        if let Some(binary) = &native_binary {
            assert_eq!(
                piped(Command::new(binary), input),
                (Some(status), expected),
                "{name}"
            );
        }
    }
    if let Some(binary) = native_binary {
        std::fs::remove_file(binary).unwrap();
    }
}

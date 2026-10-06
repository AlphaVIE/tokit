//! `tok expand` must round-trip: the expanded program checks and behaves the same.

use std::path::{Path, PathBuf};

fn sources(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "tok") {
            found.push(path);
        }
    }
}

#[test]
fn expanded_programs_check_and_run_the_same() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for directory in ["examples", "packages", "selfhost", "research/baselines"] {
        sources(&root.join(directory), &mut files);
    }
    let mut compared = 0;
    for path in files {
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(program) = tokit_compiler::parse(&source) else {
            continue;
        };
        if !program.imports.is_empty() || tokit_compiler::check(&source).is_err() {
            continue;
        }
        let expanded = tokit_compiler::expand::expand(&program);
        tokit_compiler::check(&expanded)
            .unwrap_or_else(|error| panic!("{}: {error:?}\n{expanded}", path.display()));
        let reexpanded = tokit_compiler::expand::expand(&tokit_compiler::parse(&expanded).unwrap());
        assert_eq!(reexpanded, expanded, "{} is not stable", path.display());
        // Interactive or networked programs are compared only structurally.
        let effects = [
            "read_line",
            "read_stdin",
            "serve(",
            "listen(",
            "tcp_",
            "http_request",
            "sleep_ms",
        ];
        let interactive = effects.iter().any(|effect| source.contains(effect));
        if !interactive
            && program
                .functions
                .iter()
                .any(|function| function.name == "main")
        {
            let before = tokit_compiler::run(&source).map(|value| value.to_string());
            let after = tokit_compiler::run(&expanded).map(|value| value.to_string());
            if let (Ok(before), Ok(after)) = (&before, &after) {
                assert_eq!(before, after, "{}", path.display());
                compared += 1;
            }
        }
        let pseudo = tokit_compiler::expand::pseudocode(&program);
        assert!(!pseudo.contains('{') || source.contains('{'));
    }
    assert!(compared >= 20, "only {compared} programs compared");
}

#[test]
fn pseudocode_reads_without_braces() {
    let program = tokit_compiler::parse(
        "struct P{x:I} f(p:P)->I{var t=0;for i in range(0,p.x){if i%2==0&&!false{t=t+i;}}t}",
    )
    .unwrap();
    assert_eq!(
        tokit_compiler::expand::pseudocode(&program),
        "record P\n    x: i32\n\nfunction f(p: P) returns i32:\n    variable t = 0\n    for i in range(0, p.x):\n        if i % 2 == 0 and not false:\n            t becomes t + i\n    => t\n"
    );
}

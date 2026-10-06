mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temporary(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "tokit-registry-{label}-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn tok(directory: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .current_dir(directory)
        .env("TOK_HOME", home)
        .output()
        .unwrap()
}

fn text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n")
}

#[test]
fn new_add_run_and_remove_registry_packages() {
    let base = temporary("flow");
    let home = base.join("home");
    assert!(tok(&base, &home, &["new", "demo"]).status.success());
    let project = base.join("demo");
    let run = tok(&project, &home, &["run", "main.tok"]);
    assert_eq!(text(&run), "hello from demo\n");

    let search = tok(&project, &home, &["search", "json"]);
    assert!(
        text(&search).starts_with("json@0.1.0  "),
        "{}",
        text(&search)
    );

    assert!(tok(&project, &home, &["add", "json"]).status.success());
    let manifest = std::fs::read_to_string(project.join("tok.toml")).unwrap();
    assert!(
        manifest.contains("json = { version = \"0.1.0\", sha256 = \""),
        "{manifest}"
    );
    let lock = std::fs::read_to_string(project.join("tok.lock")).unwrap();
    assert!(lock.contains("path = \"registry:json@0.1.0\""), "{lock}");
    assert!(
        !lock.contains(home.to_str().unwrap()),
        "lock must not contain store paths"
    );
    std::fs::write(
        project.join("main.tok"),
        "import json=\"pkg:json\";\nmain()->String{match json::parse(\"[1,{\\\"a\\\":null}]\"){Ok(v)=>match json::render(v){Ok(t)=>t,Err(e)=>\"render\"},Err(e)=>\"parse\"}}\n",
    )
    .unwrap();
    assert_eq!(
        text(&tok(&project, &home, &["run", "main.tok"])),
        "\"[1,{\\\"a\\\":null}]\"\n"
    );

    // A corrupted store entry is detected and rebuilt from the registry.
    let store = std::fs::read_dir(home.join("store"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(store.join("json.tok"), "main()->I{1}").unwrap();
    assert_eq!(
        text(&tok(&project, &home, &["run", "main.tok"])),
        "\"[1,{\\\"a\\\":null}]\"\n"
    );

    let again = tok(&project, &home, &["add", "json"]);
    assert!(!again.status.success());
    let missing = tok(&project, &home, &["add", "nope"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("available: "));
    let wrong_version = tok(&project, &home, &["add", "json@9.9.9"]);
    assert!(!wrong_version.status.success());

    assert!(tok(&project, &home, &["rm", "json"]).status.success());
    let manifest = std::fs::read_to_string(project.join("tok.toml")).unwrap();
    assert!(!manifest.contains("json ="), "{manifest}");
    assert!(!tok(&base, &home, &["new", "demo"]).status.success());
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn registry_pins_must_match_the_registry() {
    let base = temporary("pins");
    let home = base.join("home");
    assert!(tok(&base, &home, &["new", "app"]).status.success());
    let project = base.join("app");
    let manifest = project.join("tok.toml");
    let source = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        format!(
            "{source}json = {{ version = \"0.1.0\", sha256 = \"{}\" }}\n",
            "0".repeat(64)
        ),
    )
    .unwrap();
    let lock = tok(&project, &home, &["lock", "main.tok"]);
    assert!(!lock.status.success());
    assert!(String::from_utf8_lossy(&lock.stderr).contains("does not match registry package"));
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn registry_packages_check_and_are_canonical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packages");
    let packages = tokit_compiler::registry::packages();
    assert!(packages.len() >= 3);
    for package in packages {
        let source_path = root.join(&package.name).join(&package.entry);
        let source = std::fs::read_to_string(&source_path).unwrap();
        tokit_compiler::check(&source)
            .unwrap_or_else(|error| panic!("{}: {error:?}", package.name));
        let compact = tokit_compiler::format::compact_functions(&source)
            .and_then(|text| tokit_compiler::format::compact_integer_types(&text))
            .and_then(|text| tokit_compiler::format::compact_blocks(&text))
            .unwrap();
        assert_eq!(compact, source, "{} is not compact", package.name);
        assert_eq!(
            tokit_compiler::format::format(&source).unwrap(),
            source,
            "{} is not formatted",
            package.name
        );
        assert!(
            !package.description.is_empty(),
            "{} needs a description",
            package.name
        );
    }
}

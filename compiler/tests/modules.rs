use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{ai_index, check, interpreter, modules, native};

fn temporary_directory(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("tokit-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    path
}

fn write(directory: &Path, name: &str, source: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn module_loader_uses_unsaved_entry_and_imported_sources() {
    let directory = temporary_directory("module-overlays");
    let root = write(
        &directory,
        "main.tok",
        "import math=\"math.tok\";fn main()->i32{math::triple(7)}",
    );
    let math = write(&directory, "math.tok", "pub fn triple(n:i32)->i32{n*3}");
    let mut overrides = HashMap::new();
    overrides.insert(
        std::fs::canonicalize(&math).unwrap(),
        "pub fn triple(n:i32)->i32{missing}".to_owned(),
    );
    let error = modules::load_with_overrides(&root, &overrides)
        .err()
        .unwrap();
    assert_eq!(error.diagnostic.code, "E101");
    assert_eq!(
        error
            .sources
            .get(error.diagnostic.span.source_id)
            .unwrap()
            .path,
        std::fs::canonicalize(&math).unwrap()
    );

    overrides.insert(
        std::fs::canonicalize(&math).unwrap(),
        "pub fn triple(n:i32)->i32{n*4}".to_owned(),
    );
    overrides.insert(
        std::fs::canonicalize(&root).unwrap(),
        "import math=\"math.tok\";fn main()->i32{math::triple(8)}".to_owned(),
    );
    let loaded = modules::load_with_overrides(&root, &overrides)
        .unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "32");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn diamond_imports_load_once_and_run_in_both_backends() {
    let directory = temporary_directory("module-diamond");
    let root = write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; import b=\"b.tok\"; fn main()->i32{a::twice(3)+b::four()}",
    );
    write(
        &directory,
        "a.tok",
        "import shared=\"shared.tok\"; pub fn twice(n:i32)->i32{shared::double(n)}",
    );
    write(
        &directory,
        "b.tok",
        "import shared=\"shared.tok\"; pub fn four()->i32{shared::double(2)}",
    );
    write(&directory, "shared.tok", "pub fn double(n:i32)->i32{n*2}");
    let loaded = modules::load(&root).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(loaded.sources.len(), 4);
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "10");
    assert_eq!(loaded.program.functions.len(), 4);
    let index = ai_index::index_loaded(&loaded);
    assert!(index.starts_with(
        "{\"version\":3,\"sources\":[\"main.tok\",\"a.tok\",\"shared.tok\",\"b.tok\"]"
    ));
    assert!(
        index.contains(
            "\"modules\":[{\"source\":0,\"imports\":[[\"a\",1],[\"b\",3]],\"exports\":[]}"
        )
    );
    assert!(index.contains("{\"source\":1,\"imports\":[[\"shared\",2]],\"exports\":[\"twice\"]}"));
    assert!(index.contains("{\"source\":2,\"imports\":[],\"exports\":[\"double\"]}"));
    assert!(index.contains("{\"source\":3,\"imports\":[[\"shared\",2]],\"exports\":[\"four\"]}"));
    let generated = native::emit_with_sources(&loaded.program, &loaded.sources).unwrap();
    let canonical_parent = loaded
        .sources
        .get(tokit_compiler::ast::SourceId(0))
        .unwrap()
        .path
        .parent()
        .unwrap()
        .to_string_lossy();
    assert!(!generated.contains(canonical_parent.as_ref()));
    if Command::new("rustc").arg("--version").output().is_ok() {
        let output = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(output).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "10");
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cycles_duplicate_imports_and_path_escape_are_rejected() {
    let directory = temporary_directory("module-errors");
    let work = directory.join("work");
    std::fs::create_dir(&work).unwrap();
    write(&directory, "outside.tok", "fn outside()->i32{1}");
    let root = write(&work, "main.tok", "import a=\"a.tok\"; fn main()->i32{1}");
    write(&work, "a.tok", "import main=\"main.tok\"; fn a()->i32{1}");
    let error = modules::load(&root).err().expect("cycle rejected");
    assert_eq!(error.diagnostic.code, "E118");
    assert!(error.display().contains("a.tok:E118@1:"));

    write(
        &work,
        "main.tok",
        "import a=\"a.tok\"; import again=\"a.tok\"; fn main()->i32{1}",
    );
    write(&work, "a.tok", "fn a()->i32{1}");
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E118");

    write(
        &work,
        "main.tok",
        "import outside=\"../outside.tok\"; fn main()->i32{1}",
    );
    let error = modules::load(&root).err().expect("escape rejected");
    assert_eq!(error.diagnostic.code, "E118");
    assert!(error.diagnostic.message.contains("escapes"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn imported_type_errors_keep_their_source_and_unresolved_imports_fail() {
    let directory = temporary_directory("module-types");
    let root = write(
        &directory,
        "main.tok",
        "import lib=\"lib.tok\"; fn main()->i32{lib::f()}",
    );
    write(&directory, "lib.tok", "\npub fn f()->i32{true}");
    let error = modules::load(&root).err().expect("type error rejected");
    assert_eq!(error.diagnostic.code, "E102");
    assert!(error.display().contains("lib.tok:E102@2:"));
    let cli = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["check", "--json", root.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!cli.status.success());
    let json = String::from_utf8(cli.stdout).unwrap();
    assert!(json.contains("\"source\":"));
    assert!(json.contains("lib.tok"));
    assert_eq!(
        check("import lib=\"lib.tok\"; fn main()->i32{1}")
            .unwrap_err()
            .code,
        "E118"
    );
    assert_eq!(
        tokit_compiler::parse("fn main()->i32{1} import lib=\"lib.tok\";")
            .unwrap_err()
            .code,
        "E002"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_commands_load_imports_and_index_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let example = root.join("examples/modules/main.tok");
    let tok = env!("CARGO_BIN_EXE_tok");
    for (command, expected) in [
        ("check", "ok"),
        ("run", "21"),
        ("test", "1 passed; 0 failed"),
        ("explain", "triple"),
        ("stats", "\"dependencies\":1"),
        ("ai-index", "\"version\":3"),
    ] {
        let input = if command == "test" {
            root.join("examples/modules/tests.tok")
        } else {
            example.clone()
        };
        let result = Command::new(tok).arg(command).arg(input).output().unwrap();
        assert!(
            result.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = String::from_utf8(result.stdout).unwrap();
        assert!(output.contains(expected), "{command}: {output}");
        if command == "ai-index" {
            assert!(output.contains("\"sources\":"));
            assert!(output.contains("\"span\":[1,"));
            assert!(output.contains("\"main.tok\""));
            assert!(output.contains("\"name\":\"math::triple\""));
            assert!(output.contains("\"calls\":[\"math::triple\"]"));
            assert!(!output.contains(&root.to_string_lossy().to_string()));
        }
    }
    if Command::new("rustc").arg("--version").output().is_ok() {
        let directory = temporary_directory("module-cli-build");
        let output = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
        let build = Command::new(tok)
            .args([
                "build",
                example.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let result = Command::new(output).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "21");
        std::fs::remove_dir_all(directory).unwrap();
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
}

#[test]
fn modules_keep_private_names_separate_and_expose_qualified_types() {
    let directory = temporary_directory("module-scopes");
    let root = write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; import b=\"b.tok\"; fn main()->i32{let value:a::Box<i32> =a::make(7);match b::Event::Number(a::id(value.value)){b::Event::Ready=>0,b::Event::Number(n)=>n+a::answer()}}",
    );
    write(
        &directory,
        "a.tok",
        "struct Hidden{n:i32} fn helper()->i32{1} pub struct Box<T>{value:T} pub fn make(n:i32)->Box<i32>{Box(n)} pub fn id<T>(value:T)->T{value} pub fn answer()->i32{helper()}",
    );
    write(
        &directory,
        "b.tok",
        "struct Hidden{n:i32} fn helper()->i32{2} pub enum Event{Ready,Number(i32)}",
    );
    let loaded = modules::load(&root).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "8");
    assert!(
        loaded
            .program
            .records
            .iter()
            .any(|record| record.name == "a::Box")
    );
    assert!(
        loaded
            .program
            .enums
            .iter()
            .any(|declaration| declaration.name == "b::Event")
    );
    let index = ai_index::index_loaded(&loaded);
    assert!(index.contains("\"exports\":[\"Box\",\"answer\",\"id\",\"make\"]"));
    assert!(index.contains("\"name\":\"a::Hidden\""));
    assert!(index.contains("\"public\":false"));
    assert!(index.contains("\"name\":\"a::Box\""));
    assert!(index.contains("\"public\":true"));
    if Command::new("rustc").arg("--version").output().is_ok() {
        let output = directory.join(format!("program{}", std::env::consts::EXE_SUFFIX));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "8");
    } else {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn module_visibility_and_direct_imports_are_enforced() {
    let directory = temporary_directory("module-visibility");
    let root = write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn main()->i32{a::private()}",
    );
    write(
        &directory,
        "a.tok",
        "import b=\"b.tok\"; fn private()->i32{1} pub fn call()->i32{b::value()}",
    );
    write(&directory, "b.tok", "pub fn value()->i32{2}");
    let error = modules::load(&root).err().expect("private call rejected");
    assert_eq!(error.diagnostic.code, "E119");
    assert!(error.display().contains("main.tok:E119@1:"));

    write(
        &directory,
        "a.tok",
        "pub fn call()->i32{1} struct Hidden{value:i32}",
    );
    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn main()->i32{let hidden:a::Hidden =a::Hidden(1);hidden.value}",
    );
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E119");
    write(
        &directory,
        "a.tok",
        "import b=\"b.tok\"; fn private()->i32{1} pub fn call()->i32{b::value()}",
    );

    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn main()->i32{value()}",
    );
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E101");

    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn main()->i32{b::value()}",
    );
    assert!(
        modules::load(&root).is_err(),
        "transitive import must not leak"
    );

    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn main()->i32{a::call()}",
    );
    let loaded = modules::load(&root).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "2");

    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; import a=\"b.tok\"; fn main()->i32{1}",
    );
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E118");
    write(
        &directory,
        "main.tok",
        "import a=\"a.tok\"; fn a()->i32{1} fn main()->i32{1}",
    );
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E118");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn imported_test_functions_are_discovered_by_local_name() {
    let directory = temporary_directory("module-tests");
    let root = write(&directory, "main.tok", "import checks=\"checks.tok\";");
    write(&directory, "checks.tok", "fn test_truth()->bool{true}");
    let loaded = modules::load(&root).unwrap_or_else(|error| panic!("{}", error.display()));
    let report =
        tokit_compiler::test_runner::run_loaded(&loaded.program, &loaded.sources, None, None)
            .unwrap();
    assert_eq!(
        report.display(),
        "PASS checks::test_truth\n1 passed; 0 failed"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

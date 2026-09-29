use std::path::{Path, PathBuf};
use std::process::Command;

use tokit_compiler::{check, interpreter, modules, native};

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
fn diamond_imports_load_once_and_run_in_both_backends() {
    let directory = temporary_directory("module-diamond");
    let root = write(
        &directory,
        "main.tok",
        "import \"a.tok\"; import \"b.tok\"; fn main()->i32{twice(3)+four()}",
    );
    write(
        &directory,
        "a.tok",
        "import \"shared.tok\"; fn twice(n:i32)->i32{double(n)}",
    );
    write(
        &directory,
        "b.tok",
        "import \"shared.tok\"; fn four()->i32{double(2)}",
    );
    write(&directory, "shared.tok", "fn double(n:i32)->i32{n*2}");
    let loaded = modules::load(&root).unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(loaded.sources.len(), 4);
    assert_eq!(interpreter::run(&loaded.program).unwrap().to_string(), "10");
    assert_eq!(loaded.program.functions.len(), 4);
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
    let root = write(&work, "main.tok", "import \"a.tok\"; fn main()->i32{1}");
    write(&work, "a.tok", "import \"main.tok\"; fn a()->i32{1}");
    let error = modules::load(&root).err().expect("cycle rejected");
    assert_eq!(error.diagnostic.code, "E118");
    assert!(error.display().contains("a.tok:E118@1:"));

    write(
        &work,
        "main.tok",
        "import \"a.tok\"; import \"a.tok\"; fn main()->i32{1}",
    );
    write(&work, "a.tok", "fn a()->i32{1}");
    assert_eq!(modules::load(&root).err().unwrap().diagnostic.code, "E118");

    write(
        &work,
        "main.tok",
        "import \"../outside.tok\"; fn main()->i32{1}",
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
        "import \"lib.tok\"; fn main()->i32{f()}",
    );
    write(&directory, "lib.tok", "\nfn f()->i32{true}");
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
        check("import \"lib.tok\"; fn main()->i32{1}")
            .unwrap_err()
            .code,
        "E118"
    );
    assert_eq!(
        tokit_compiler::parse("fn main()->i32{1} import \"lib.tok\";")
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
        ("ai-index", "\"version\":2"),
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

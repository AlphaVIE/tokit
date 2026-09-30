use std::path::Path;
use std::process::Command;

use tokit_compiler::{interpreter, modules, native, test_runner};

fn example(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples/json")
        .join(name)
}

#[test]
fn json_module_tests_pass_and_invalid_number_is_typed() {
    let tests =
        modules::load(&example("tests.tok")).unwrap_or_else(|error| panic!("{}", error.display()));
    let report = test_runner::run_loaded(&tests.program, &tests.sources, None, None).unwrap();
    assert_eq!(report.failed(), 0, "{}", report.display());
    assert_eq!(report.cases.len(), 6);

    let invalid = modules::load(&example("invalid.tok"))
        .unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(
        interpreter::run(&invalid.program).unwrap().to_string(),
        "Err(json::RenderError::InvalidNumber)"
    );
}

#[test]
fn json_renderer_agrees_with_native_backend() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    for entry in ["main.tok", "invalid.tok"] {
        let loaded =
            modules::load(&example(entry)).unwrap_or_else(|error| panic!("{}", error.display()));
        let expected = interpreter::run(&loaded.program).unwrap().to_string();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "tokit-json-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            entry,
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::remove_file(output).unwrap();
    }
}

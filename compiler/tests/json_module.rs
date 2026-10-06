mod common;

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
fn json_benchmarks_reject_same_length_wrong_output() {
    for entry in ["bench_main.tok", "render_bench_main.tok"] {
        let path = example(entry);
        let output = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(["run", path.to_str().unwrap(), "--", "{}", "1", "[]"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{entry}");
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "None");
    }
}

#[test]
fn json_module_tests_pass_and_invalid_number_is_typed() {
    let tests =
        modules::load(&example("tests.tok")).unwrap_or_else(|error| panic!("{}", error.display()));
    let report = test_runner::run_loaded(&tests.program, &tests.sources, None, None).unwrap();
    assert_eq!(report.failed(), 0, "{}", report.display());
    assert_eq!(report.cases.len(), 6);

    let parse_tests = modules::load(&example("parse_tests.tok"))
        .unwrap_or_else(|error| panic!("{}", error.display()));
    let parse_report =
        test_runner::run_loaded(&parse_tests.program, &parse_tests.sources, None, None).unwrap();
    assert_eq!(parse_report.failed(), 0, "{}", parse_report.display());
    assert_eq!(parse_report.cases.len(), 6);

    let invalid = modules::load(&example("invalid.tok"))
        .unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(
        interpreter::run(&invalid.program).unwrap().to_string(),
        "Err(json::RenderError::InvalidNumber)"
    );
    let parse_invalid = modules::load(&example("parse_invalid.tok"))
        .unwrap_or_else(|error| panic!("{}", error.display()));
    assert_eq!(
        interpreter::run(&parse_invalid.program)
            .unwrap()
            .to_string(),
        "Err(json::JsonParseError(offset:3))"
    );
}

#[test]
fn json_renderer_agrees_with_native_backend() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    for entry in [
        "main.tok",
        "invalid.tok",
        "parse_main.tok",
        "parse_invalid.tok",
    ] {
        let loaded =
            modules::load(&example(entry)).unwrap_or_else(|error| panic!("{}", error.display()));
        let expected = interpreter::run(&loaded.program).unwrap().to_string();
        let nonce = common::nonce();
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

#[test]
fn json_cli_corpus_agrees_across_interpreter_and_native() {
    let entry = example("parse_cli.tok");
    let loaded = modules::load(&entry).unwrap_or_else(|error| panic!("{}", error.display()));
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    let nonce = common::nonce();
    let output = std::env::temp_dir().join(format!(
        "tokit-json-corpus-{}-{nonce}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    if native_available {
        native::build_with_sources(&loaded.program, &loaded.sources, &output).unwrap();
    }
    let fixtures = [
        ("null", Some("null")),
        ("{}", Some("{}")),
        ("-0.25E+08", Some("-0.25E+08")),
        (
            "  [1, true, {\"é\": \"\\uD83D\\uDE00\"}] \n",
            Some("[1,true,{\"é\":\"😀\"}]"),
        ),
        ("\"\\u0000\"", Some("\"\\u0000\"")),
        ("\"a\\/b\"", Some("\"a/b\"")),
        ("[1,]", None),
        ("{\"x\":}", None),
        ("+1", None),
        ("\"\\uDEAD\"", None),
    ];
    for (input, canonical) in fixtures {
        let expected =
            canonical.map_or_else(|| "None".to_owned(), |text| format!("Some({text:?})"));
        let interpreted = Command::new(env!("CARGO_BIN_EXE_tok"))
            .args(["run", entry.to_str().unwrap(), "--", input])
            .output()
            .unwrap();
        assert!(
            interpreted.status.success(),
            "{input}: {}",
            String::from_utf8_lossy(&interpreted.stderr)
        );
        assert_eq!(
            String::from_utf8(interpreted.stdout).unwrap().trim(),
            expected,
            "interpreter: {input}"
        );
        if native_available {
            let compiled = Command::new(&output).args(["--", input]).output().unwrap();
            assert!(
                compiled.status.success(),
                "{input}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            assert_eq!(
                String::from_utf8(compiled.stdout).unwrap().trim(),
                expected,
                "native: {input}"
            );
        }
    }
    if native_available {
        std::fs::remove_file(output).unwrap();
    }
}

use std::path::Path;
use std::process::Command;

use tokit_compiler::{format, lexer};

#[test]
fn canonical_format_is_idempotent_and_keeps_tokens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for entry in std::fs::read_dir(root.join("examples")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "tok") {
            let source = std::fs::read_to_string(&path).unwrap();
            let compacted = format::compact_functions(&source).unwrap();
            assert_eq!(
                format::compact_functions(&compacted).unwrap(),
                compacted,
                "{}",
                path.display()
            );
            assert_eq!(
                tokit_compiler::parse(&source).unwrap().functions.len(),
                tokit_compiler::parse(&compacted).unwrap().functions.len()
            );
            let formatted = format::format(&source).unwrap();
            assert_eq!(
                format::format(&formatted).unwrap(),
                formatted,
                "{}",
                path.display()
            );
            let before = lexer::lex(&source).unwrap();
            let after = lexer::lex(&formatted).unwrap();
            assert_eq!(
                before.iter().map(|token| &token.kind).collect::<Vec<_>>(),
                after.iter().map(|token| &token.kind).collect::<Vec<_>>(),
                "{}",
                path.display()
            );
        }
    }
    for name in ["sum-positive", "checked-division", "generic-pair"] {
        let source =
            std::fs::read_to_string(root.join("research/baselines").join(name).join("tokit.tok"))
                .unwrap();
        let formatted = format::format(&source).unwrap();
        assert_eq!(format::format(&formatted).unwrap(), formatted, "{name}");
        assert_eq!(
            lexer::lex(&source)
                .unwrap()
                .iter()
                .map(|token| &token.kind)
                .collect::<Vec<_>>(),
            lexer::lex(&formatted)
                .unwrap()
                .iter()
                .map(|token| &token.kind)
                .collect::<Vec<_>>(),
            "{name}"
        );
        assert!(formatted.contains("// BENCH_START\n"));
        assert!(formatted.contains("// BENCH_END\n"));
    }
}

#[test]
fn comments_and_ambiguous_token_boundaries_are_preserved() {
    let source = "// keep\npub fn sum<T>(x:T)->T{x}fn main()->i32{1}";
    let compacted = format::compact_functions(source).unwrap();
    assert_eq!(compacted, "// keep\npub sum<T>(x:T)->T{x}main()->i32{1}");
    assert_eq!(format::compact_functions(&compacted).unwrap(), compacted);
    assert_eq!(
        format::compact_functions("fn // retained\nmain()->i32{1}").unwrap(),
        " // retained\nmain()->i32{1}"
    );
    assert_eq!(
        tokit_compiler::run(source).unwrap(),
        tokit_compiler::run(&compacted).unwrap()
    );
    let compact = "add(a:i32,b:i32)->i32{a+b}main()->i32{add(20,22)}";
    let formatted = format::format(compact).unwrap();
    assert_eq!(
        formatted,
        "add(a:i32,b:i32)->i32{a+b}\nmain()->i32{add(20,22)}\n"
    );
    assert_eq!(format::format(&formatted).unwrap(), formatted);
    let source = "// header\nfn main ( ) -> i32 { let x : i32 = 7 ; // note\n x / 2 } // end";
    let formatted = format::format(source).unwrap();
    assert_eq!(
        formatted,
        "// header\nfn main()->i32{let x:i32=7;\n// note\nx/2}\n// end\n"
    );
    assert_eq!(format::format(&formatted).unwrap(), formatted);
    assert_eq!(
        format::format("fn main()->i32{1 // x\n +2}").unwrap(),
        "fn main()->i32{1\n// x\n+2}\n"
    );
}

#[test]
fn cli_check_and_write_use_the_same_canonical_form() {
    let path = std::env::temp_dir().join(format!(
        "tokit-fmt-{}-{}.tok",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "fn main ( ) -> i32 { 1 + 2 }").unwrap();
    let binary = env!("CARGO_BIN_EXE_tok");
    assert!(
        !Command::new(binary)
            .arg("fmt")
            .arg("--check")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(binary)
            .arg("fmt")
            .arg("--write")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new(binary)
            .arg("fmt")
            .arg("--check")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "fn main()->i32{1+2}\n"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn compact_cli_writes_keyword_free_declarations() {
    let path = std::env::temp_dir().join(format!(
        "tokit-compact-{}-{}.tok",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "fn main()->i32{42}").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["compact", "--write", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "main()->i32{42}");
    std::fs::remove_file(path).unwrap();
}

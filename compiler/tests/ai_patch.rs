use std::fs;
use std::process::Command;

use tokit_compiler::ai_patch::{self, FunctionEdit};

fn edit(function: &str, original: &str, replacement: &str) -> FunctionEdit {
    FunctionEdit {
        function: function.to_owned(),
        expected_sha256: ai_patch::digest(original),
        replacement: replacement.to_owned(),
    }
}

#[test]
fn name_and_hash_patch_multiple_functions_without_rewriting_neighbors() {
    let source = "// header\nfn first()->i32{1}\nfn second()->i32{2}\n";
    let edits = [
        edit("first", "fn first()->i32{1}", "fn first()->i32{3}"),
        edit("second", "fn second()->i32{2}", "fn second()->i32{4}"),
    ];
    let patched = ai_patch::apply(source, &edits).unwrap();
    assert_eq!(
        patched,
        "// header\nfn first()->i32{3}\nfn second()->i32{4}\n"
    );
    assert_eq!(tokit_compiler::run(&patched).unwrap_err().code, "E203");
    let shifted = format!("// unrelated offset\n{source}");
    assert!(ai_patch::apply(&shifted, &edits).is_ok());
    let stale = [edit("first", "fn first()->i32{0}", "fn first()->i32{3}")];
    assert_eq!(ai_patch::apply(source, &stale).unwrap_err().code, "P004");
    assert_eq!(
        ai_patch::apply(source, &[edits[0].to_owned(), edits[0].to_owned()])
            .unwrap_err()
            .code,
        "P002"
    );
}

#[test]
fn replacements_preserve_import_context_and_cannot_consume_neighbors() {
    let old = "fn answer()->i32{math::value()}";
    let source = format!("import math=\"math.tok\";{old}fn neighbor()->i32{{2}}");
    let patched = ai_patch::apply(
        &source,
        &[edit("answer", old, "fn answer()->i32{math::value()+1}")],
    )
    .unwrap();
    assert!(patched.ends_with("fn neighbor()->i32{2}"));
    for replacement in [
        "fn answer()->i32{7}//",
        "pub fn answer()->i32{7}",
        "fn renamed()->i32{7}",
    ] {
        assert_eq!(
            ai_patch::apply(&source, &[edit("answer", old, replacement)])
                .unwrap_err()
                .code,
            "P005"
        );
    }
}

#[test]
fn cli_validates_import_dependents_before_writing() {
    let root = std::env::temp_dir().join(format!(
        "tokit-ai-patch-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let entry = root.join("main.tok");
    let module = root.join("math.tok");
    let request = root.join("request.json");
    fs::write(
        &entry,
        "import math=\"math.tok\";fn main()->i32{math::answer()}",
    )
    .unwrap();
    let original = "pub fn answer()->i32{1}";
    fs::write(&module, original).unwrap();
    let index = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["ai-patch-index", module.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(index.status.success());
    let indexed: serde_json::Value = serde_json::from_slice(&index.stdout).unwrap();
    assert_eq!(
        indexed["functions"][0]["sha256"],
        ai_patch::digest("fn answer()->i32{1}")
    );

    let make_request = |replacement: &str| {
        serde_json::json!({
            "version":1,
            "target":"math.tok",
            "edits":[{
                "function":"answer",
                "expected_sha256": ai_patch::digest("fn answer()->i32{1}"),
                "replacement":replacement
            }]
        })
        .to_string()
    };
    fs::write(&request, make_request("fn answer()->bool{true}")).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "ai-patch",
            entry.to_str().unwrap(),
            request.to_str().unwrap(),
            "--write",
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert_eq!(fs::read_to_string(&module).unwrap(), original);
    let error: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert!(error["code"].is_string());

    fs::write(&request, make_request("fn answer()->i32{7}")).unwrap();
    let preview = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "ai-patch",
            entry.to_str().unwrap(),
            request.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(preview.status.success());
    assert_eq!(fs::read_to_string(&module).unwrap(), original);
    assert_eq!(
        String::from_utf8(preview.stdout).unwrap().trim(),
        "pub fn answer()->i32{7}"
    );
    let accepted = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "ai-patch",
            entry.to_str().unwrap(),
            request.to_str().unwrap(),
            "--write",
        ])
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert_eq!(
        fs::read_to_string(&module).unwrap(),
        "pub fn answer()->i32{7}"
    );
    fs::remove_dir_all(root).unwrap();
}

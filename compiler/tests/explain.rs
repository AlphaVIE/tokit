use std::path::Path;
use std::process::Command;

use tokit_compiler::{check, explain};

#[test]
fn explanation_is_deterministic_and_describes_nested_control_flow() {
    let source = "struct Box<T>{value:T} enum Choice{Yes,No} fn pick(c:Choice,x:i32)->Box<i32>{Box(match c{Choice::Yes=>x,Choice::No=>0})} fn main()->Box<i32>{pick(Choice::Yes,5)}";
    let program = check(source).unwrap();
    assert_eq!(
        explain::explain(&program),
        concat!(
            "Tokit program\n\nRecords\n",
            "  Box<T>\n    value: T\n",
            "\nEnums\n  Choice: Yes, No\n",
            "\nFunctions\n",
            "  pick(c: Choice, x: i32) -> Box<i32>\n",
            "    calls: none\n    constructs: Box\n    operations: exhaustive matching\n",
            "  main() -> Box<i32>\n",
            "    calls: pick\n    constructs: none\n    operations: none\n",
        )
    );
}

#[test]
fn cli_explain_checks_source_and_reports_operations() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["explain", "examples/match_result.tok"])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(output.status.success());
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.contains("divide(a: i32, b: i32) -> Result<i32,DivError>"));
    assert!(report.contains("calls: divide"));
    assert!(report.contains("operations: exhaustive matching"));
}

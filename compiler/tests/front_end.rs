use std::process::Command;
use tokit_compiler::{check, interpreter::Value, run};

#[test]
fn executes_functions_bindings_and_branches() {
    let source =
        "fn add(a:i32,b:i32)->i32{a+b} fn main()->i32{let x:i32=40;if x>0{add(x,2)}else{0}}";
    assert_eq!(run(source).unwrap(), Value::I32(42));
}

#[test]
fn executes_recursion_and_early_return() {
    let source =
        "fn fact(n:i32)->i32{if n<=1{return 1;}else{}; n*fact(n-1)} fn main()->i32{fact(5)}";
    assert_eq!(run(source).unwrap(), Value::I32(120));
}

#[test]
fn checks_before_running() {
    let cases = [
        ("fn main()->i32{true+1}", "E104"),
        ("fn main()->i32{missing}", "E101"),
        ("fn main()->i32{let x:bool=1;0}", "E102"),
        ("fn main()->i32{if 1{2}else{3}}", "E102"),
        ("fn f(x:i32)->i32{x} fn main()->i32{f()}", "E105"),
        ("fn main()->i32{0} fn main()->i32{1}", "E106"),
        ("fn main()->i32{2147483648}", "E003"),
        ("fn main()->i32{0", "E002"),
        ("fn main()->i32{@}", "E001"),
        ("fn main()->i32{return 1; 2}", "E107"),
    ];
    for (source, expected) in cases {
        assert_eq!(check(source).unwrap_err().code, expected, "{source}");
    }
}

#[test]
fn checked_arithmetic_reports_runtime_error() {
    assert_eq!(
        run("fn main()->i32{2147483647+1}").unwrap_err().code,
        "E201"
    );
    assert_eq!(run("fn main()->i32{1/0}").unwrap_err().code, "E201");
}

#[test]
fn run_requires_parameterless_main() {
    assert_eq!(run("fn other()->i32{1}").unwrap_err().code, "E203");
    assert_eq!(run("fn main(x:i32)->i32{x}").unwrap_err().code, "E203");
}

#[test]
fn machine_diagnostic_has_stable_span_and_escaping() {
    use tokit_compiler::{ast::Span, diagnostic::Diagnostic};
    let diagnostic = Diagnostic::new("E999", Span { start: 2, end: 4 }, "bad \"name\"\n");
    assert_eq!(
        diagnostic.json("a\né"),
        r#"{"code":"E999","span":{"start":2,"end":4},"line":2,"column":1,"message":"bad \"name\"\n"}"#
    );
}

#[test]
fn cli_emits_json_success() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["check", "--json", "examples/answer.tok"])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        r#"{"ok":true,"result":"ok"}"#
    );
}

#[test]
fn malformed_short_inputs_do_not_panic() {
    let alphabet = b"fn let if else return(){}:,+-*/=<>;0123abc \n";
    let mut state = 0x839bc62bu32;
    for _ in 0..2000 {
        let mut source = String::new();
        for _ in 0..48 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            source.push(alphabet[(state as usize) % alphabet.len()] as char);
        }
        assert!(
            std::panic::catch_unwind(|| check(&source)).is_ok(),
            "{source:?}"
        );
    }
}

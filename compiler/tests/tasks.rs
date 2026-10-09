mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn task_values_join_and_match_errors() {
    let cases = [
        (
            "fn square(x:i32)->i32{x*x} fn main()->Result<i32,TaskError>{var task:Task<i32> =spawn square(7);let a:i32=join(task)?;let b:i32=join(task)?;Ok(a+b)}",
            "Ok(98)",
        ),
        (
            "fn square(x:i32)->i32{x*x} fn main()->i32{match join(spawn square(7)){Ok(n)=>n,Err(e)=>match e{TaskError::Failed=>0}}}",
            "49",
        ),
        (
            "fn square(x:i32)->i32{x*x} fn main()->Task<i32>{spawn square(7)}",
            "<task>",
        ),
        (
            "fn sum(xs:[i32])->i32{var total:i32=0;for x in xs{total=total+x;}total} fn main()->Result<i32,TaskError>{join(spawn sum([1,2,3]))}",
            "Ok(6)",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(run(source).unwrap().to_string(), expected);
        if Command::new("rustc").arg("--version").output().is_err() {
            assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
            continue;
        }
        let nonce = common::nonce();
        let output = std::env::temp_dir().join(format!(
            "tokit-task-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::remove_file(output).unwrap();
    }
}

#[test]
fn only_typed_pure_named_functions_can_be_spawned() {
    for (source, code) in [
        ("fn main()->Task<i32>{spawn lines(\"x\")}", "E117"),
        ("fn main()->Task<i32>{spawn read_text(\"x\")}", "E117"),
        (
            "struct Box{value:i32} fn main()->Task<Box>{spawn Box(1)}",
            "E117",
        ),
        (
            "fn f()->Result<String,IoError>{read_text(\"x\")} fn main()->Task<Result<String,IoError>>{spawn f()}",
            "E117",
        ),
        (
            "fn f()->Result<String,IoError>{read_text(\"x\")} fn g()->Result<String,IoError>{f()} fn main()->Task<Result<String,IoError>>{spawn g()}",
            "E117",
        ),
        (
            "fn f<T>(x:T)->T{x} fn main()->Task<i32>{spawn f(1)}",
            "E117",
        ),
        (
            "fn square(x:i32)->i32{x*x} fn main()->Task<String>{spawn square(1)}",
            "E102",
        ),
        ("fn main()->Result<i32,TaskError>{join(1)}", "E115"),
        ("fn main()->Task<Unknown>{spawn foo()}", "E103"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn arithmetic_failure_inside_task_remains_a_fatal_diagnostic() {
    let source =
        "overflow()->i32{2147483647+1} main()->Result<i32,TaskError>{join(spawn overflow())}";
    assert_eq!(run(source).unwrap_err().code, "E201");
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let output = std::env::temp_dir().join(format!(
        "tokit-task-overflow-{}-{}{}",
        std::process::id(),
        common::nonce(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E201"));
    std::fs::remove_file(output).unwrap();
}

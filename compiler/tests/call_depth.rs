mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_result(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-depth-{}-{}{}",
        std::process::id(),
        common::nonce(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    let stream = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    (
        output.status.success(),
        String::from_utf8(stream).unwrap().trim().to_owned(),
    )
}

#[test]
fn deep_recursion_runs_up_to_the_shared_limit() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    // `main` occupies the first of 10,000 frames.
    let within = "count(n:I,acc:[I])->I{if n==0{len(acc)}else{var next=acc;if n%1000==0{next.push(n);}else{};count(n-1,next)}} main()->I{count(9998,[])}";
    let beyond = "count(n:I)->I{if n==0{0}else{count(n-1)+1}} main()->I{count(9999)}";
    assert_eq!(run(within).unwrap().to_string(), "9");
    let error = run(beyond).unwrap_err();
    assert_eq!(error.code, "E202");
    if native_available {
        assert_eq!(native_result(within), (true, "9".to_owned()));
        let (success, stderr) = native_result(beyond);
        assert!(!success && stderr.starts_with("E202@"), "{stderr}");
    }
}

#[test]
fn deep_recursion_inside_a_task_uses_its_own_stack() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    let source = "count(n:I)->I{if n==0{0}else{count(n-1)+1}} main()->Result<I,TaskError>{join(spawn count(9000))}";
    assert_eq!(run(source).unwrap().to_string(), "Ok(9000)");
    if native_available {
        assert_eq!(native_result(source), (true, "Ok(9000)".to_owned()));
    }
}

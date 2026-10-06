mod common;

use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, explain, native, run, stats};

fn temporary_directory() -> PathBuf {
    let nonce = common::nonce();
    let directory =
        std::env::temp_dir().join(format!("tokit-loop-control-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

#[test]
fn break_and_continue_target_the_nearest_loop() {
    for (source, expected) in [
        (
            "fn main()->i32{var i:i32=0;while true{i=i+1;if i==3{break;}else{};}i}",
            "3",
        ),
        (
            "fn main()->i32{var i:i32=0;var sum:i32=0;while i<5{i=i+1;if i==3{continue;}else{};sum=sum+i;}sum}",
            "12",
        ),
        (
            "fn main()->i32{var sum:i32=0;for x in [1,2,3]{if x==2{break;}else{};sum=sum+x;}sum}",
            "1",
        ),
        (
            "fn main()->i32{var sum:i32=0;for x in [1,2,3]{if x==2{continue;}else{};sum=sum+x;}sum}",
            "4",
        ),
        (
            "fn main()->i32{var outer:i32=0;var count:i32=0;while outer<3{outer=outer+1;for x in [1,2,3]{if x==2{break;}else{};count=count+1;}}count}",
            "3",
        ),
        (
            "fn main()->i32{var outer:i32=0;var count:i32=0;while outer<3{outer=outer+1;var inner:i32=0;while inner<3{inner=inner+1;if inner==2{continue;}else{};count=count+1;}count=count+10;}count}",
            "36",
        ),
        (
            "fn main()->i32{var x:i32=0;while true{match x==2{true=>{break;},false=>{x=x+1;}};}x}",
            "2",
        ),
        (
            "fn count()->i32{var sum:i32=0;for x in [1,2,3]{if x==2{continue;}else{};sum=sum+x;}sum} fn main()->Result<i32,TaskError>{join(spawn count())}",
            "Ok(4)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected);
        if Command::new("rustc").arg("--version").output().is_err() {
            assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
            continue;
        }
        let directory = temporary_directory();
        let executable = directory.join(format!("loop{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &executable).unwrap();
        let output = Command::new(&executable).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), expected);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn loop_control_is_scoped_and_counts_as_control_flow() {
    for (source, code) in [
        ("fn main()->i32{break;0}", "E002"),
        ("fn main()->i32{continue;0}", "E002"),
        ("fn main()->i32{if true{break;}else{};0}", "E002"),
        ("fn main()->i32{while true{break;0}0}", "E107"),
        ("fn main()->i32{for x in [1]{continue;0}0}", "E107"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
    let source = include_str!("../../examples/loop_control.tok");
    let program = check(source).unwrap();
    assert_eq!(run(source).unwrap().to_string(), "12");
    let description = explain::explain(&program);
    assert!(description.contains("loop break"));
    assert!(description.contains("loop continue"));
    assert!(stats::measure(source, &program).semantic_ops >= 8);
}

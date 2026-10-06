mod common;

use std::path::PathBuf;
use std::process::Command;

use tokit_compiler::{check, explain, native, run, stats};

fn temporary_directory() -> PathBuf {
    let nonce = common::nonce();
    let directory =
        std::env::temp_dir().join(format!("tokit-while-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    directory
}

#[test]
fn while_loops_recheck_conditions_and_propagate_control_flow() {
    for (source, expected) in [
        ("fn main()->i32{var x:i32=0;while x<5{x=x+1;}x}", "5"),
        ("fn main()->i32{var x:i32=7;while false{x=x+1;}x}", "7"),
        (
            "fn main()->[i32]{var xs:[i32]=[];while len(xs)<3{xs.push(len(xs));}xs}",
            "[0,1,2]",
        ),
        (
            "fn main()->i32{var x:i32=0;while x<10{if x==3{return x;}else{x=x+1;}}0}",
            "3",
        ),
        (
            "fn main()->Result<i32,i32>{var x:i32=0;while x<3{let value:Result<i32,i32> =Err(7);x=value?;}Ok(x)}",
            "Err(7)",
        ),
        (
            "fn count(n:i32)->i32{var x:i32=0;while x<n{x=x+1;}x} fn main()->Result<i32,TaskError>{join(spawn count(4))}",
            "Ok(4)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected);
        if Command::new("rustc").arg("--version").output().is_err() {
            assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
            continue;
        }
        let directory = temporary_directory();
        let executable = directory.join(format!("while{}", std::env::consts::EXE_SUFFIX));
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
fn while_conditions_and_body_scopes_are_checked() {
    for (source, code) in [
        ("fn main()->i32{while 1{}0}", "E102"),
        ("fn main()->i32{while false{missing=1;}0}", "E101"),
        (
            "fn main()->i32{while false{let hidden:i32=1;}hidden}",
            "E101",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
    let source = include_str!("../../examples/iterative_factorial.tok");
    let program = check(source).unwrap();
    assert_eq!(run(source).unwrap().to_string(), "120");
    assert!(explain::explain(&program).contains("conditional iteration"));
    assert!(stats::measure(source, &program).semantic_ops >= 5);
}

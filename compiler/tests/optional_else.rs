mod common;

use std::process::Command;

use tokit_compiler::{check, format, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-else-{}-{}{}",
        std::process::id(),
        common::nonce(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn if_without_else_and_block_statements_run_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        ("main()->I{var x=0;if true{x=1;}x}", "1"),
        ("main()->I{var x=0;if false{x=1;}x}", "0"),
        ("main()->I{var x=0;if x==1{x=5;}else if x==0{x=7;}x}", "7"),
        ("main()->I{var x=0;while x<10{if x==4{break;}x=x+1;}x}", "4"),
        ("main()->I{var x=0;match x{0=>{x=3;},_=>{}}{x=x+1;}x}", "4"),
        ("f()->I{if true{return 2;}3} main()->I{f()}", "2"),
        ("main()->I{var x=1;if x>0{x=2;};x}", "2"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn if_without_else_requires_a_unit_branch() {
    let error = check("main()->I{if true{1}}").unwrap_err();
    assert_eq!(error.code, "E102");
    assert!(
        error.message.contains("if without else"),
        "{}",
        error.message
    );
    assert_eq!(
        check("main()->I{let y=if true{1};y}").unwrap_err().code,
        "E102"
    );
    // Without `;` a non-block expression still has to be the block tail.
    assert_eq!(check("main()->I{var x=0;x=1 x}").unwrap_err().code, "E002");
}

#[test]
fn compact_blocks_removes_only_redundant_tokens() {
    for (source, expected) in [
        (
            "main()->I{var x=0;if x==0{x=1;}else{};x}",
            "main()->I{var x=0;if x==0{x=1;}x}",
        ),
        (
            "main()->I{var x=0;if x==0{x=1;}else if x==1{x=2;}else {  };return x;}",
            "main()->I{var x=0;if x==0{x=1;}else if x==1{x=2;}return x;}",
        ),
        // The `;` must stay where the next token could continue an expression.
        (
            "main()->I{var x=0;if x==0{x=1;}else{};-x}",
            "main()->I{var x=0;if x==0{x=1;};-x}",
        ),
        // Branches with values and comments are left alone.
        (
            "main()->I{let y=if true{1}else{2};y}",
            "main()->I{let y=if true{1}else{2};y}",
        ),
        (
            "main()->I{var x=0;if true{x=1;}else{ // keep\n};x}",
            "main()->I{var x=0;if true{x=1;}else{ // keep\n}x}",
        ),
        ("main()->I{var x=0;{x=1;};x}", "main()->I{var x=0;{x=1;}x}"),
    ] {
        let compacted = format::compact_blocks(source).unwrap();
        assert_eq!(compacted, expected, "{source}");
        assert_eq!(format::compact_blocks(&compacted).unwrap(), compacted);
        assert_eq!(
            run(source).unwrap().to_string(),
            run(&compacted).unwrap().to_string()
        );
        let formatted = format::format(&compacted).unwrap();
        assert_eq!(format::format(&formatted).unwrap(), formatted);
    }
}

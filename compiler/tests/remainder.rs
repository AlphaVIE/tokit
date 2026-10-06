use std::process::Command;

use tokit_compiler::ir::{self, BinaryOp, InstructionKind};
use tokit_compiler::{check, checker, format, native, parse, run};

fn native_output(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-rem-{}-{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(binary).unwrap();
    let text = if output.status.success() {
        String::from_utf8(output.stdout).unwrap()
    } else {
        String::from_utf8(output.stderr).unwrap()
    };
    (output.status.success(), text.trim().to_owned())
}

#[test]
fn remainder_agrees_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        ("main()->I{17%5}", "2"),
        ("main()->I{-17%5}", "-2"),
        ("main()->I{17%-5}", "2"),
        ("main()->I{-2147483648%-1}", "0"),
        ("main()->L{-9223372036854775808i64%-1i64}", "0"),
        ("main()->L{10000000007i64%10i64}", "7"),
        ("main()->I{2+7%4*3}", "11"),
        ("main()->F{7.5%2.0}", "1.5"),
        ("main()->F{-7.5%2.0}", "-1.5"),
        ("main()->F{1.0%0.0}", "NaN"),
        ("even(x:I)->bool{x%2==0} main()->bool{even(10)}", "true"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(
                native_output(source),
                (true, expected.to_owned()),
                "{source}"
            );
        }
    }
}

#[test]
fn integer_remainder_by_zero_fails_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for source in [
        "zero()->I{0} main()->I{5%zero()}",
        "zero()->L{0i64} main()->L{5i64%zero()}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E201", "{source}");
        if native_available {
            let (success, stderr) = native_output(source);
            assert!(!success && stderr.contains("E201"), "{source}: {stderr}");
        }
    }
}

#[test]
fn remainder_is_typed_formatted_and_lowered() {
    for source in [
        "main()->I{1%1i64}",
        "main()->F{1.0%1}",
        "main()->bool{true%true}",
        "main()->String{\"a\"%\"b\"}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E104", "{source}");
    }
    assert_eq!(
        format::format("main()->I{ 7 % 3 }").unwrap(),
        format::format("main()->I{7%3}").unwrap()
    );
    let source = "rem(a:I,b:I)->I{a%b} main()->I{rem(7,3)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &program, &types).unwrap();
    lowered.verify().unwrap();
    assert!(lowered.instructions.iter().any(|instruction| matches!(
        instruction.kind,
        InstructionKind::Binary(BinaryOp::Rem, _, _)
    )));
}

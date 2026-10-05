use std::process::Command;

use tokit_compiler::ir::{self, InstructionKind, ValueId};
use tokit_compiler::{check, checker, format, native, parse, run};

fn native_result(source: &str) -> (bool, String, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-boolean-{}-{}{}",
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
    (
        output.status.success(),
        String::from_utf8(output.stdout).unwrap().trim().to_owned(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn boolean_operators_have_explicit_precedence_and_lazy_execution() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for (source, expected) in [
        ("main()->bool{true||false&&false}", "true"),
        ("main()->bool{!false&&1<2}", "true"),
        ("main()->bool{false&&true==false}", "false"),
        ("main()->bool{!(1<2)||3<4}", "true"),
        ("main()->bool{false&&1/0==0}", "false"),
        ("main()->bool{true||1/0==0}", "true"),
        ("main()->I{var n=0;false&&{n=1;true};n}", "0"),
        ("main()->I{var n=0;true||{n=1;false};n}", "0"),
        ("main()->I{var n=0;true&&{n=1;true};n}", "1"),
        ("main()->bool{let x=false&&{return true;};x}", "false"),
        ("main()->bool{let x=true||{return false;};x}", "true"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            let (success, stdout, stderr) = native_result(source);
            assert!(success, "{source}: {stderr}");
            assert_eq!(stdout, expected, "{source}");
        }
    }
    for source in ["main()->bool{true&&1/0==0}", "main()->bool{false||1/0==0}"] {
        assert_eq!(run(source).unwrap_err().code, "E201");
        if native_available {
            let (success, _, stderr) = native_result(source);
            assert!(!success);
            assert!(stderr.contains("E201"));
        }
    }
}

#[test]
fn boolean_operators_reject_non_boolean_operands_and_format_idempotently() {
    for source in [
        "main()->bool{1&&true}",
        "main()->bool{true||1}",
        "main()->bool{!1}",
        "main()->bool{1&&{return true;}}",
    ] {
        assert!(matches!(check(source).unwrap_err().code, "E102" | "E104"));
    }
    for source in ["main()->bool{true&false}", "main()->bool{true|false}"] {
        assert_eq!(check(source).unwrap_err().code, "E001");
    }
    assert_eq!(
        check("main()->bool{false&&unknown}").unwrap_err().code,
        "E101"
    );
    let formatted = format::format("main ( ) -> bool { ! false && ( true || false ) }").unwrap();
    assert_eq!(format::format(&formatted).unwrap(), formatted);
    assert_eq!(run(&formatted).unwrap().to_string(), "true");
}

#[test]
fn scalar_ir_uses_lazy_regions_for_boolean_operators() {
    let source = "logic(a:bool,b:bool)->bool{!a||b&&true} main()->bool{logic(false,false)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &program, &types).unwrap();
    lowered.verify().unwrap();
    assert!(
        lowered
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::Not(_)))
    );
    let outer = lowered.instructions.last().unwrap();
    let InstructionKind::Conditional { yes, no, .. } = &outer.kind else {
        panic!("expected lazy disjunction");
    };
    assert!(yes.instructions.is_empty());
    assert!(!no.instructions.is_empty());
    let mut malformed = lowered.clone();
    malformed.instructions[1].kind = InstructionKind::Not(ValueId(1));
    assert_eq!(
        malformed.verify(),
        Err("logical negation operand type mismatch")
    );
    assert_eq!(run(source).unwrap().to_string(), "true");
}

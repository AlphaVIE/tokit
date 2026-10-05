use std::process::Command;

use tokit_compiler::ast::Type;
use tokit_compiler::ir::{self, BinaryOp, InstructionKind, ValueId};
use tokit_compiler::{checker, native, parse, run};

#[test]
fn scalar_ir_has_typed_ordered_values_and_native_parity() {
    let source = "fn greater(a:i64,b:i64)->bool{a+b>b} fn main()->bool{greater(2i64,3i64)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &types).unwrap();
    lowered.verify().unwrap();
    assert_eq!(lowered.instructions.len(), 5);
    assert_eq!(lowered.instructions[2].ty, Type::I64);
    assert_eq!(
        lowered.instructions[2].kind,
        InstructionKind::Binary(BinaryOp::Add, ValueId(0), ValueId(1))
    );
    assert_eq!(lowered.result, ValueId(4));
    assert!(ir::lower_function(&program.functions[1], &types).is_none());
    let mut malformed = lowered.clone();
    malformed.instructions[2].kind = InstructionKind::Binary(BinaryOp::Add, ValueId(3), ValueId(1));
    assert_eq!(malformed.verify(), Err("binary operand is not defined"));

    let generated = native::emit(&program, source).unwrap();
    assert!(generated.contains("let __tok_v2: i64 = __tok_add_i64("));
    assert!(generated.contains("let __tok_v4: bool = (__tok_v2 > __tok_v3)"));
    assert_eq!(run(source).unwrap().to_string(), "true");
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let binary = std::env::temp_dir().join(format!(
        "tokit-ir-{}-{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&program, source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "true");
    std::fs::remove_file(binary).unwrap();
}

#[test]
fn scalar_ir_preserves_checked_overflow_diagnostic() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    for source in [
        "fn add(a:i64,b:i64)->i64{a+b} fn main()->i64{add(9223372036854775807i64,1i64)}",
        "main()->I{2147483647+1;0}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E201");
        let binary = std::env::temp_dir().join(format!(
            "tokit-ir-overflow-{}-{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&tokit_compiler::check(source).unwrap(), source, &binary).unwrap();
        let output = Command::new(&binary).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr).unwrap().contains("E201"));
        std::fs::remove_file(binary).unwrap();
    }
}

#[test]
fn scalar_ir_lowers_scoped_immutable_bindings() {
    let source = "calc(a:I)->I{let x=a+1;{let x=x*2;x+3}} main()->I{calc(19)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &types).unwrap();
    lowered.verify().unwrap();
    assert_eq!(lowered.instructions.len(), 7);
    assert_eq!(lowered.result, ValueId(6));
    assert_eq!(
        lowered.instructions[4].kind,
        InstructionKind::Binary(BinaryOp::Mul, ValueId(2), ValueId(3))
    );
    assert!(ir::lower_function(&program.functions[1], &types).is_none());
    assert_eq!(run(source).unwrap().to_string(), "43");
    let generated = native::emit(&program, source).unwrap();
    assert!(generated.contains("let __tok_v4: i32 = __tok_mul("));
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let binary = std::env::temp_dir().join(format!(
        "tokit-ir-locals-{}-{}{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&program, source, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "43");
    std::fs::remove_file(binary).unwrap();

    let mutable = parse("f(a:I)->I{var x=a;x=2;x}").unwrap();
    let types = checker::check_with_types(&mutable).unwrap();
    assert!(ir::lower_function(&mutable.functions[0], &types).is_none());
}

#[test]
fn scalar_ir_branches_are_lazy_scoped_and_match_native_execution() {
    let source = "choose(flag:bool,a:I)->I{if flag{let x=a+1;x}else{let x=a+2;x}} main()->I{choose(false,40)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &types).unwrap();
    lowered.verify().unwrap();
    assert_eq!(lowered.instructions.len(), 2);
    let InstructionKind::Conditional { yes, no, .. } = &lowered.instructions[1].kind else {
        panic!("expected conditional instruction");
    };
    assert_eq!(yes.instructions.len(), 3);
    assert_eq!(no.instructions.len(), 3);
    let mut malformed = lowered.clone();
    let InstructionKind::Conditional { yes, no, .. } = &mut malformed.instructions[1].kind else {
        unreachable!();
    };
    no.result = yes.result;
    assert_eq!(malformed.verify(), Err("no branch result type mismatch"));
    assert_eq!(run(source).unwrap().to_string(), "42");
    let generated = native::emit(&program, source).unwrap();
    assert!(generated.contains("if __tok_v0"));
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = std::env::temp_dir().join(format!(
            "tokit-ir-branch-scope-{}-{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&program, source, &binary).unwrap();
        let output = Command::new(&binary).output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "42");
        std::fs::remove_file(binary).unwrap();
    }

    for (branch, expected) in [
        ("main()->I{if false{1/0}else{42}}", Some("42")),
        ("main()->I{if true{42}else{1/0}}", Some("42")),
        ("main()->I{(if false{1/0}else{41})+1}", Some("42")),
        ("main()->I{if true{1/0}else{42}}", None),
        ("main()->I{if true{if false{1/0}else{7}}else{0}}", Some("7")),
    ] {
        let program = parse(branch).unwrap();
        let types = checker::check_with_types(&program).unwrap();
        ir::lower_function(&program.functions[0], &types)
            .unwrap()
            .verify()
            .unwrap();
        match expected {
            Some(value) => assert_eq!(run(branch).unwrap().to_string(), value),
            None => assert_eq!(run(branch).unwrap_err().code, "E201"),
        }
        if Command::new("rustc").arg("--version").output().is_err() {
            assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
            continue;
        }
        let binary = std::env::temp_dir().join(format!(
            "tokit-ir-branch-{}-{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&program, branch, &binary).unwrap();
        let output = Command::new(&binary).output().unwrap();
        match expected {
            Some(value) => {
                assert!(output.status.success());
                assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), value);
            }
            None => {
                assert!(!output.status.success());
                assert!(String::from_utf8(output.stderr).unwrap().contains("E201"));
            }
        }
        std::fs::remove_file(binary).unwrap();
    }
}

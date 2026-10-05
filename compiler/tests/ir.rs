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
    let source = "fn add(a:i64,b:i64)->i64{a+b} fn main()->i64{add(9223372036854775807i64,1i64)}";
    assert_eq!(run(source).unwrap_err().code, "E201");
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
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

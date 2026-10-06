use std::process::Command;

use tokit_compiler::ir::{self, InstructionKind};
use tokit_compiler::{check, checker, format, native, parse, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-f64-{}-{}{}",
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn floats_agree_between_interpreter_and_native_backend() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        ("main()->f64{1.5+2.25}", "3.75"),
        ("main()->F{1e3/4.0}", "250.0"),
        ("main()->F{-0.0}", "-0.0"),
        ("main()->F{-(1.0+2.0)}", "-3.0"),
        ("main()->F{1.0/0.0}", "inf"),
        ("main()->F{0.0/0.0}", "NaN"),
        ("main()->bool{0.0/0.0==0.0/0.0}", "false"),
        ("main()->bool{0.0/0.0!=0.0/0.0}", "true"),
        ("main()->bool{0.0==-0.0}", "true"),
        ("main()->bool{1.0<2.0}", "true"),
        ("double(x:F)->F{x*2.0} main()->F{double(1.25)}", "2.5"),
        (
            "struct Box<T>{value:T} main()->Box<F>{Box(1.5)}",
            "Box(value:1.5)",
        ),
        ("main()->[F]{[1.0,-0.0,2.5e-1]}", "[1.0,-0.0,0.25]"),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn float_literals_and_mixed_arithmetic_are_checked() {
    for source in [
        "main()->F{1.0+1}",
        "main()->F{1.0+1i64}",
        "main()->I{1.0}",
        "main()->F{1e309}",
        "main()->F{- 1.0}",
        "main()->F{1e+}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
    assert_eq!(check("main()->F{1e309}").unwrap_err().code, "E003");
    let source = "main()->f64{-1.25e+2}";
    let formatted = format::format(source).unwrap();
    assert_eq!(format::format(&formatted).unwrap(), formatted);
    assert_eq!(run(&formatted).unwrap().to_string(), "-125.0");
}

#[test]
fn scalar_float_functions_lower_to_verified_ir() {
    let source = "negate(x:F)->F{-x} main()->F{negate(1.25)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    let lowered = ir::lower_function(&program.functions[0], &program, &types).unwrap();
    lowered.verify().unwrap();
    assert!(
        lowered
            .instructions
            .iter()
            .any(|instruction| matches!(instruction.kind, InstructionKind::FloatNeg(_)))
    );
}

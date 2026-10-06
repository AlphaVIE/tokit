mod common;

use std::process::Command;

use tokit_compiler::ir::{self, InstructionKind};
use tokit_compiler::{check, checker, format, native, parse, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-f64-{}-{}{}",
        std::process::id(),
        common::nonce(),
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

#[test]
fn numeric_conversions_and_float_parsing_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        ("main()->F{f64(7)/2.0}", "3.5"),
        ("main()->F{f64(9007199254740993i64)}", "9007199254740992.0"),
        ("main()->Option<I>{i32(-2.9)}", "Some(-2)"),
        ("main()->Option<I>{i32(2147483647.9)}", "Some(2147483647)"),
        ("main()->Option<I>{i32(2147483648.0)}", "None"),
        ("main()->Option<I>{i32(0.0/0.0)}", "None"),
        (
            "main()->Option<L>{i64(-9.2e18)}",
            "Some(-9200000000000000000)",
        ),
        ("main()->Option<L>{i64(9.3e18)}", "None"),
        ("main()->Option<L>{i64(1.0/0.0)}", "None"),
        ("main()->L{i64(3)}", "3"),
        (
            "main()->Result<F,ParseError>{parse_f64(\"-1.5e2\")}",
            "Ok(-150.0)",
        ),
        ("main()->Result<F,ParseError>{parse_f64(\"+7\")}", "Ok(7.0)"),
        (
            "main()->Result<F,ParseError>{parse_f64(\"1e400\")}",
            "Err(ParseError::OutOfRange)",
        ),
        (
            "main()->[Result<F,ParseError>]{[parse_f64(\"inf\"),parse_f64(\".5\"),parse_f64(\"5.\"),parse_f64(\" 1\"),parse_f64(\"1e\"),parse_f64(\"\")]}",
            "[Err(ParseError::Invalid),Err(ParseError::Invalid),Err(ParseError::Invalid),Err(ParseError::Invalid),Err(ParseError::Invalid),Err(ParseError::Invalid)]",
        ),
        (
            "mean(a:I,b:L)->F{(f64(a)+f64(b))/2.0} main()->F{mean(1,2i64)}",
            "1.5",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn numeric_conversions_reject_unsupported_operands() {
    for source in [
        "main()->F{f64(1.0)}",
        "main()->F{f64(true)}",
        "main()->Option<I>{i32(1)}",
        "main()->L{i64(1.0)}",
        "struct f64{x:I} main()->I{1}",
        "parse_f64(x:I)->I{x} main()->I{1}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
}

#[test]
fn infallible_conversions_lower_to_verified_ir() {
    let source =
        "mean(a:I,b:L)->F{(f64(a)+f64(b))/2.0} wide(x:I)->L{i64(x)} main()->F{mean(1,2i64)}";
    let program = parse(source).unwrap();
    let types = checker::check_with_types(&program).unwrap();
    for function in &program.functions[..2] {
        let lowered = ir::lower_function(function, &program, &types).unwrap();
        lowered.verify().unwrap();
        assert!(
            lowered
                .instructions
                .iter()
                .any(|instruction| matches!(instruction.kind, InstructionKind::Convert(_)))
        );
    }
}

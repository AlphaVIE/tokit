mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_result(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-math-{}-{}{}",
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
fn math_builtins_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "main()->[I]{[abs(-5),min(3,-2),max(3,-2),pow(2,10),pow(-3,3),pow(7,0)]}",
            "[5,-2,3,1024,-27,1]",
        ),
        (
            "main()->[L]{[abs(-5i64),min(1i64,2i64),max(1i64,2i64),pow(2i64,40)]}",
            "[5,1,2,1099511627776]",
        ),
        (
            "main()->[F]{[abs(-1.5),min(1.0,0.0/0.0),max(2.0,3.5),pow(2.0,0.5),sqrt(16.0),sqrt(-1.0)]}",
            "[1.5,1.0,3.5,1.4142135623730951,4.0,NaN]",
        ),
        (
            "main()->[F]{[floor(-1.5),ceil(-1.5),round(2.5),round(-2.5),exp(0.0),ln(1.0),sin(0.0),cos(0.0),tan(0.0),atan2(1.0,1.0)*4.0,pi()]}",
            "[-2.0,-1.0,3.0,-3.0,1.0,0.0,0.0,1.0,0.0,3.141592653589793,3.141592653589793]",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(
                native_result(source),
                (true, expected.to_owned()),
                "{source}"
            );
        }
    }
}

#[test]
fn integer_math_overflow_fails_in_both_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for source in [
        "main()->I{abs(-2147483647-1)}",
        "main()->I{pow(2,31)}",
        "main()->I{pow(2,-1)}",
        "main()->L{pow(10i64,19)}",
        "main()->L{abs(-9223372036854775808i64)}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E201", "{source}");
        if native_available {
            let (success, stderr) = native_result(source);
            assert!(!success && stderr.starts_with("E201"), "{source}: {stderr}");
        }
    }
    for source in [
        "main()->I{min(1,2i64)}",
        "main()->F{sqrt(4)}",
        "main()->L{pow(2i64,2i64)}",
        "main()->F{pow(2.0,2)}",
        "main()->bool{abs(true)}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E102", "{source}");
    }
}

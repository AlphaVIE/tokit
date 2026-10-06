mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_result(source: &str) -> (bool, String) {
    let binary = std::env::temp_dir().join(format!(
        "tokit-bitwise-{}-{}{}",
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
fn bit_operations_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "main()->[I]{[bit_and(12,10),bit_or(12,10),bit_xor(12,10),bit_not(0),shl(1,31),shr(-8,1),shr(255,4)]}",
            "[8,14,6,-1,-2147483648,-4,15]",
        ),
        (
            "main()->[L]{[bit_and(12i64,10i64),bit_xor(-1i64,5i64),shl(1i64,40),shr(-1i64,63),bit_not(5i64)]}",
            "[8,-6,1099511627776,-1,-6]",
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
    for source in [
        "main()->I{shl(1,32)}",
        "main()->I{shr(1,-1)}",
        "main()->L{shl(1i64,64)}",
    ] {
        assert_eq!(run(source).unwrap_err().code, "E201", "{source}");
        if native_available {
            let (success, stderr) = native_result(source);
            assert!(!success && stderr.starts_with("E201"), "{source}: {stderr}");
        }
    }
    for source in [
        "main()->I{bit_and(1,2i64)}",
        "main()->L{shl(1i64,1i64)}",
        "main()->I{bit_or(true,1)}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E102", "{source}");
    }
}

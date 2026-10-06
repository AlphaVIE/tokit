mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn utf8_conversion_matches_interpreter_and_native() {
    let cases = [
        ("fn main()->[i32]{utf8_bytes(\"\")}", "[]"),
        (
            "fn main()->[i32]{utf8_bytes(\"Aé😀\")}",
            "[65,195,169,240,159,152,128]",
        ),
        (
            "fn main()->Option<String>{utf8_decode([65,195,169,240,159,152,128])}",
            "Some(\"Aé😀\")",
        ),
        (
            "fn main()->Option<String>{utf8_decode(utf8_bytes(\"Grüße\"))}",
            "Some(\"Grüße\")",
        ),
        (
            "fn main()->Option<String>{let data:[i32]=[];utf8_decode(data)}",
            "Some(\"\")",
        ),
        ("fn main()->Option<String>{utf8_decode([255])}", "None"),
        ("fn main()->Option<String>{utf8_decode([-1])}", "None"),
        ("fn main()->Option<String>{utf8_decode([256])}", "None"),
        ("fn main()->Option<String>{utf8_decode([195,40])}", "None"),
        (
            "fn decode()->Option<String>{utf8_decode([65])} fn main()->Result<Option<String>,TaskError>{join(spawn decode())}",
            "Ok(Some(\"A\"))",
        ),
    ];
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for (source, expected) in cases {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if !native_available {
            continue;
        }
        let nonce = common::nonce();
        let output = std::env::temp_dir().join(format!(
            "tokit-utf8-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{}: {}",
            source,
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::remove_file(output).unwrap();
    }
}

#[test]
fn utf8_conversion_rejects_wrong_types_and_reserved_names() {
    for source in [
        "fn main()->[i32]{utf8_bytes(1)}",
        "fn main()->Option<String>{utf8_decode(\"A\")}",
        "fn main()->Option<String>{utf8_decode([\"A\"])}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
    for source in [
        "fn utf8_bytes(x:String)->[i32]{[1]}",
        "struct utf8_decode{value:i32}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E106", "{source}");
    }
}

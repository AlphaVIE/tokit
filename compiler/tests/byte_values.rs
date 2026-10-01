use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn byte_values_match_interpreter_and_native() {
    let cases = [
        (
            "fn main()->Bytes{utf8_encode(\"Aé😀\")}",
            "Bytes([65, 195, 169, 240, 159, 152, 128])",
        ),
        (
            "fn main()->Option<String>{utf8_decode_bytes(utf8_encode(\"Grüße\"))}",
            "Some(\"Grüße\")",
        ),
        (
            "fn main()->Option<String>{utf8_decode_bytes(match bytes_from_i32([255]){Some(data)=>data,None=>utf8_encode(\"\")})}",
            "None",
        ),
        ("fn main()->Option<Bytes>{bytes_from_i32([-1])}", "None"),
        ("fn main()->Option<Bytes>{bytes_from_i32([256])}", "None"),
        (
            "fn main()->Option<Bytes>{bytes_from_i32([])}",
            "Some(Bytes([]))",
        ),
        (
            "fn main()->[i32]{bytes_to_i32(utf8_encode(\"Aé\"))}",
            "[65,195,169]",
        ),
        (
            "fn main()->i32{let data:Bytes=utf8_encode(\"Aé\");data[1]+len(data)}",
            "198",
        ),
        (
            "fn main()->i32{var sum:i32=0;for byte in utf8_encode(\"Aé\"){sum=sum+byte;}sum}",
            "429",
        ),
        (
            "fn main()->bool{utf8_encode(\"A\")==utf8_encode(\"A\")}",
            "true",
        ),
        (
            "struct Box{data:Bytes} fn main()->Bytes{Box(utf8_encode(\"x\")).data}",
            "Bytes([120])",
        ),
        (
            "fn identity<T>(value:T)->T{value} fn main()->Bytes{identity(utf8_encode(\"x\"))}",
            "Bytes([120])",
        ),
        (
            "fn encode()->Bytes{utf8_encode(\"x\")} fn main()->Result<Bytes,TaskError>{join(spawn encode())}",
            "Ok(Bytes([120]))",
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
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "tokit-bytes-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            expected,
            "{source}"
        );
        std::fs::remove_file(output).unwrap();
    }
}

#[test]
fn byte_values_reject_wrong_types_and_reserved_names() {
    for source in [
        "fn main()->Bytes{utf8_encode(1)}",
        "fn main()->Option<String>{utf8_decode_bytes([1])}",
        "fn main()->Option<Bytes>{bytes_from_i32(\"A\")}",
        "fn main()->[i32]{bytes_to_i32([1])}",
        "fn main()->Bytes{utf8_encode(\"A\")[0]}",
    ] {
        assert!(check(source).is_err(), "{source}");
    }
    for source in [
        "struct Bytes{value:i32}",
        "fn utf8_encode(text:String)->Bytes{bytes_from_i32([1])}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E106", "{source}");
    }
}

#[test]
fn byte_index_bounds_match_native_diagnostic() {
    let source = "fn main()->i32{utf8_encode(\"A\")[1]}";
    let expected = run(source).unwrap_err().display(source);
    assert!(expected.starts_with("E205@"));
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "tokit-byte-bounds-{}-{nonce}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(source).unwrap(), source, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    assert!(!result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stderr).trim(), expected);
    std::fs::remove_file(output).unwrap();
}

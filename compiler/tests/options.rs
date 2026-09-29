use std::process::Command;

use tokit_compiler::{check, native, run};

#[test]
fn optional_values_match_in_interpreter_and_native() {
    let cases = [
        ("fn main()->Option<i32>{Some(7)}", "Some(7)"),
        ("fn main()->Option<i32>{None}", "None"),
        (
            "fn main()->i32{let x:Option<i32> =None;match x{Some(v)=>v,None=>0}}",
            "0",
        ),
        (
            "fn positive(x:i32)->Option<i32>{if x>0{Some(x)}else{None}} fn main()->i32{match positive(7){Some(v)=>v,None=>0}}",
            "7",
        ),
        (
            "struct Entry{value:Option<String>} fn main()->Entry{Entry(Some(\"hello\"))}",
            "Entry(value:Some(\"hello\"))",
        ),
        (
            "fn main()->[Option<i32>]{[Some(1),None,Some(3)]}",
            "[Some(1),None,Some(3)]",
        ),
        (
            "fn keep<T>(value:Option<T>)->Option<T>{value} fn main()->Option<i32>{keep(Some(9))}",
            "Some(9)",
        ),
        (
            "struct Entry{value:Option<String>} fn main()->Entry{Entry(None)}",
            "Entry(value:None)",
        ),
    ];
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    if !native_available {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
    }
    for (source, expected) in cases {
        assert_eq!(run(source).unwrap().to_string(), expected);
        if !native_available {
            continue;
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "tokit-option-{}-{nonce}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), expected);
        std::fs::remove_file(output).unwrap();
    }
}

#[test]
fn optional_values_require_valid_types_and_exhaustive_matches() {
    for (source, code) in [
        ("fn main()->Option<Unknown>{None}", "E103"),
        ("fn main()->Option<i32>{Some(\"x\")}", "E102"),
        (
            "fn main()->i32{let x:Option<i32> =Some(1);match x{Some(v)=>v}}",
            "E116",
        ),
        (
            "fn main()->i32{let x:Option<i32> =None;match x{Some(v)=>v,None=>0,None=>1}}",
            "E116",
        ),
        ("fn main()->i32{match None{Some(v)=>1,None=>0}}", "E115"),
        ("struct Option<T>{value:T} fn main()->i32{0}", "E106"),
        ("struct Node{next:Option<Node>} fn main()->i32{0}", "E112"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

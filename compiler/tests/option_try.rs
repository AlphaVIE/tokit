use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-option-try-{}-{}{}",
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
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn question_mark_propagates_none() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    let program = "first_word_len(text:String)->Option<I>{let words=split(text,\" \");let first=get(lookup(),words[0])?;Some(first+len(words))} lookup()->Map<String,I>{var m:Map<String,I>=Map();m[\"hi\"]=10;m}";
    for (main, expected) in [
        (
            "main()->[Option<I>]{[first_word_len(\"hi there\"),first_word_len(\"no match\")]}",
            "[Some(12),None]",
        ),
        (
            "main()->Option<I>{let x=i32(3000000000i64)?;Some(x)}",
            "None",
        ),
        (
            "main()->Option<I>{var t=0;for x in [1i64,2i64]{t=t+i32(x)?;}Some(t)}",
            "Some(3)",
        ),
    ] {
        let source = format!("{program} {main}");
        assert_eq!(run(&source).unwrap().to_string(), expected, "{main}");
        if native_available {
            assert_eq!(native_output(&source), expected, "{main}");
        }
    }
}

#[test]
fn question_mark_requires_matching_return_types() {
    for (source, code) in [
        ("f(x:Option<I>)->I{x?} main()->I{f(None)}", "E111"),
        (
            "f(x:Option<I>)->Result<I,String>{Ok(x?)} main()->I{1}",
            "E111",
        ),
        (
            "f(x:Result<I,String>)->Option<I>{Some(x?)} main()->I{1}",
            "E111",
        ),
        ("f(x:I)->Option<I>{Some(x?)} main()->I{1}", "E111"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

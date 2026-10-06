use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-strings-{}-{}{}",
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
fn string_builtins_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    for (source, expected) in [
        (
            "main()->[String]{[String(-7),String(3000000000i64),String(1.5),String(-0.0),String(1.0/0.0),String(false)]}",
            r#"["-7","3000000000","1.5","-0.0","inf","false"]"#,
        ),
        (
            r#"main()->[String]{split(trim("  a,b,,c  "),",")}"#,
            r#"["a","b","","c"]"#,
        ),
        (
            r#"main()->[String]{[join(["x","y","z"],", "),join([],"-"),join(chars("añ🙂"),"|")]}"#,
            r#"["x, y, z","","a|ñ|🙂"]"#,
        ),
        (
            r#"main()->[String]{[join(split("abc",""),"/"),replace("a.b.c",".","::"),replace("abc","","x"),lower("ÄBC"),upper("straße")]}"#,
            r#"["abc","a::b::c","abc","äbc","STRASSE"]"#,
        ),
        (
            r#"main()->[bool]{[contains("haystack","st"),contains("a",""),starts_with("tokit","tok"),ends_with("tokit","kit"),ends_with("a","ab")]}"#,
            "[true,true,true,true,false]",
        ),
        (
            r#"main()->[bool]{["apple"<"banana","b"<="a","Z"<"a","é">"z","same">="same"]}"#,
            "[true,false,true,true,true]",
        ),
        (
            r#"count(text:String)->I{len(split(text," "))} main()->Result<I,TaskError>{join(spawn count("a b c"))}"#,
            "Ok(3)",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn string_builtins_are_typed() {
    for (source, code) in [
        (r#"main()->String{String("x")}"#, "E102"),
        ("main()->String{String([1])}", "E102"),
        (r#"main()->[String]{split("a",1)}"#, "E102"),
        (r#"main()->String{join(["a"],1)}"#, "E102"),
        (r#"main()->String{join([1],",")}"#, "E102"),
        (r#"main()->bool{"a"<1}"#, "E104"),
        ("main()->bool{true<false}", "E104"),
        ("fn trim(x:I)->I{x} main()->I{1}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

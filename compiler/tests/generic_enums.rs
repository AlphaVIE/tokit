use std::process::Command;

use tokit_compiler::{check, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-generic-enums-{}-{}{}",
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
fn generic_enums_agree_between_backends() {
    let native_available = Command::new("rustc").arg("--version").output().is_ok();
    let tree = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/generic_tree.tok"),
    )
    .unwrap();
    for (source, expected) in [
        (tree.as_str(), r#"["3","root,a,b","num 1;text x"]"#),
        (
            "enum Maybe<T>{Nothing,Just(T)} or<T>(m:Maybe<T>,d:T)->T{match m{Maybe::Just(v)=>v,Maybe::Nothing=>d}} main()->[I]{let n:Maybe<I>=Maybe::Nothing;[or(Maybe::Just(4),0),or(n,7)]}",
            "[4,7]",
        ),
        (
            "enum Pair<A,B>{Both([A]),Other(B)} main()->Pair<I,String>{Pair::Both([1,2])}",
            "Pair::Both([1,2])",
        ),
        (
            "enum Box<T>{Full(T)} main()->[Box<(I)->I>]{[Box::Full(|x:I|x)]}",
            "[Box::Full(<fn>)]",
        ),
    ] {
        assert_eq!(run(source).unwrap().to_string(), expected, "{source}");
        if native_available {
            assert_eq!(native_output(source), expected, "{source}");
        }
    }
}

#[test]
fn generic_enums_are_checked() {
    for (source, code) in [
        ("enum E<T>{A,B} main()->I{1}", "E115"),
        (
            "enum E<T>{A(T)} main()->I{let x=E::A(1);let y:E<String>=x;1}",
            "E102",
        ),
        ("enum E<T>{A(T),B} main()->I{let x=E::B;1}", "E115"),
        (
            "enum E<T>{A(T),B} main()->I{let x:E<I>=E::B;match x{E::A(v)=>v+\"s\",E::B=>0}}",
            "E104",
        ),
        ("enum E<T>{A(T)} main()->I{let x:E=E::A(1);1}", "E103"),
        ("enum E<T>{A(T)} main()->I{let x:E<I,I>=E::A(1);1}", "E103"),
        ("enum E<T,T>{A(T)} main()->I{1}", "E106"),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

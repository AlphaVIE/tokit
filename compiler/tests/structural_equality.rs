//! `==` and `contains` on arrays, records, enums, options, and results compare
//! structurally, identically in both backends.

use std::process::Command;

use tokit_compiler::{check, native, run};

const PROGRAM: &str = r#"struct P{x:I,name:String}
enum Shape{Dot,Circle(F),Group([Shape])}
struct Box<T>{item:T}
struct Handler{run:(I)->I}
main()->[bool]{let nan=0.0/0.0;[P(1,"a")==P(1,"a"),P(1,"a")!=P(2,"a"),[1,2]==[1,2],[1,2]==[2,1],Shape::Circle(1.5)==Shape::Circle(1.5),Shape::Dot==Shape::Circle(0.0),Shape::Group([Shape::Dot])==Shape::Group([Shape::Dot]),Some(3)==Some(3),parse_i32("x")==Err(ParseError::Invalid),Box([P(1,"a")])==Box([P(1,"a")]),[nan]==[nan],Some(0.0)==Some(-0.0),contains([P(1,"a"),P(2,"b")],P(2,"b")),contains([Some(1)],None),Handler(|x|x).run(2)==2]}"#;

const EXPECTED: &str =
    "[true,true,true,false,true,false,true,true,true,true,false,true,true,false,true]";

#[test]
fn structural_equality_agrees_in_both_backends() {
    assert_eq!(run(PROGRAM).unwrap().to_string(), EXPECTED);
    if Command::new("rustc").arg("--version").output().is_err() {
        return;
    }
    let binary = std::env::temp_dir().join(format!(
        "tokit-equality-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(PROGRAM).unwrap(), PROGRAM, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(&binary).unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), EXPECTED);
}

#[test]
fn values_without_equality_are_rejected() {
    for source in [
        "struct H{run:(I)->I} main()->bool{H(|x|x)==H(|x|x)}",
        "main()->bool{let m:Map<I,I>=Map();m==m}",
        "f<T>(a:T,b:T)->bool{a==b} main()->bool{f(1,1)}",
        "main()->bool{[1]==[\"a\"]}",
        "main()->bool{Some(1)==Some(\"a\")}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E104", "{source}");
    }
}

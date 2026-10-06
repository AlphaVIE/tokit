//! Nested and binding patterns: typing, exhaustiveness, and identical results
//! in the interpreter and native executables.

use std::process::Command;

use tokit_compiler::{check, native, run};

const PROGRAM: &str = r#"enum Shape{Dot,Circle(I),Label(String)}
enum Tree<T>{Leaf,Node(T)}
describe(r:Result<Option<Shape>,IoError>)->String{match r{Ok(Some(Shape::Circle(0)))=>"point",Ok(Some(Shape::Circle(n)))=>"circle "+String(n),Ok(Some(Shape::Label("")))=>"blank",Ok(Some(Shape::Label(t)))=>"label "+t,Ok(Some(other))=>"dot",Ok(None)=>"nothing",Err(IoError::NotFound)=>"missing",Err(e)=>"error"}}
flags(x:Option<bool>)->I{match x{Some(true)=>1,Some(false)=>2,None=>3}}
depth(t:Tree<Option<I>>)->I{match t{Tree::Node(Some(v))=>v,Tree::Node(None)=>-1,Tree::Leaf=>0}}
word(s:String)->String{match s{"hi"=>"greeting",other=>"said "+other}}
code(n:I)->I{match n{0=>0,k=>k*2}}
main()->[String]{[describe(Ok(Some(Shape::Circle(0)))),describe(Ok(Some(Shape::Circle(3)))),describe(Ok(Some(Shape::Label("")))),describe(Ok(Some(Shape::Label("x")))),describe(Ok(Some(Shape::Dot))),describe(Ok(None)),describe(Err(IoError::NotFound)),describe(Err(IoError::Denied)),String(flags(Some(true))+flags(Some(false))*10+flags(None)*100),String(depth(Tree::Node(Some(7)))),String(depth(Tree::Node(None))),String(depth(Tree::Leaf)),word("hi"),word("yo"),String(code(0)+code(5))]}"#;

const EXPECTED: &str = r#"["point","circle 3","blank","label x","dot","nothing","missing","error","321","7","-1","0","greeting","said yo","10"]"#;

#[test]
fn nested_patterns_run_the_same_in_both_backends() {
    assert_eq!(run(PROGRAM).unwrap().to_string(), EXPECTED);
    if Command::new("rustc").arg("--version").output().is_err() {
        return;
    }
    let binary = std::env::temp_dir().join(format!(
        "tokit-nested-patterns-{}{}",
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
fn nested_exhaustiveness_names_the_missing_case() {
    for (source, message) in [
        (
            "f(x:Option<bool>)->I{match x{Some(true)=>1,None=>0}}",
            "non-exhaustive match: missing Some(false)",
        ),
        (
            "f(x:Result<I,IoError>)->I{match x{Ok(n)=>n,Err(IoError::Denied)=>0}}",
            "non-exhaustive match: missing Err(InvalidUtf8)",
        ),
        (
            "f(x:Option<Option<I>>)->I{match x{Some(Some(1))=>1,None=>0}}",
            "non-exhaustive match: missing Some(None)",
        ),
        (
            "f(x:Option<I>)->I{match x{Some(1)=>1,None=>0}}",
            "non-exhaustive match: missing Some(_)",
        ),
        (
            "f(x:Option<I>)->I{match x{Some(n)=>n}}",
            "non-exhaustive match: missing None",
        ),
        (
            "f(x:Option<bool>)->I{match x{Some(b)=>1,Some(true)=>2,None=>0}}",
            "duplicate match arm Some(true)",
        ),
        (
            "f(x:I)->I{match x{n=>n,0=>1}}",
            "unreachable match arm after wildcard",
        ),
        (
            "f(x:Option<bool>)->I{match x{Some(true)=>1,Some(false)=>2,None=>3,_=>4}}",
            "wildcard arm is unreachable",
        ),
        (
            "f(x:Option<I>)->I{match x{Some(true)=>1,_=>0}}",
            "pattern does not match i32",
        ),
    ] {
        let error = check(&format!("{source} main()->I{{0}}")).unwrap_err();
        assert_eq!(
            (error.code, error.message.as_str()),
            ("E116", message),
            "{source}"
        );
    }
}

#[test]
fn pattern_bindings_are_scoped_and_typed() {
    // A bare name binds the whole value with its type.
    assert_eq!(
        run("main()->I{match Some(4){Some(n)=>match n{1=>0,m=>m+1},None=>0}}")
            .unwrap()
            .to_string(),
        "5"
    );
    assert_eq!(
        check("main()->I{match 1{n=>0};n}").unwrap_err().code,
        "E101"
    );
    assert_eq!(
        check("main()->bool{match Some(1){Some(v)=>v,None=>false}}")
            .unwrap_err()
            .code,
        "E102"
    );
}

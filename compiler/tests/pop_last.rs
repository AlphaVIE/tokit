//! `last(xs)`, `pop(xs)`, and the statement `xs.pop();` agree in both
//! backends and steer method-style calls to the right spelling.

mod common;

use std::process::Command;

use tokit_compiler::{check, native, run};

const PROGRAM: &str = r#"balanced(s:String)->bool{var st:[String]=[];for c in chars(s){if c=="("{st.push(c);}else if c==")"{match last(st){Some(o)=>{st.pop();},None=>{return false;}}}}len(st)==0}
main()->[String]{let e:[I]=[];var xs=[1,2,3];xs.pop();let top=last(xs);[String(balanced("(())")),String(balanced("())")),String(balanced("((")),String(len(xs)),match top{Some(v)=>String(v),None=>"none"},String(pop(e)==e),String(last(e)==None),String(pop([4])==e)]}"#;

const EXPECTED: &str = r#"["true","false","false","2","2","true","true","true"]"#;

#[test]
fn pop_and_last_agree_in_both_backends() {
    assert_eq!(run(PROGRAM).unwrap().to_string(), EXPECTED);
    if Command::new("rustc").arg("--version").output().is_err() {
        return;
    }
    let binary = std::env::temp_dir().join(format!(
        "tokit-pop-last-{}{}",
        common::nonce(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(PROGRAM).unwrap(), PROGRAM, &binary).unwrap();
    let output = Command::new(&binary).output().unwrap();
    std::fs::remove_file(&binary).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), EXPECTED);
}

#[test]
fn pop_statement_and_hints() {
    // `xs.pop();` needs a `var`, like `xs.push(v);`.
    assert_eq!(
        check("main()->I{let xs=[1];xs.pop();0}").unwrap_err().code,
        "E109"
    );
    // Reading and removing at once is spelled with last and pop.
    let error = check("main()->I{var xs=[1];let t=xs.pop();0}").unwrap_err();
    assert_eq!(error.code, "E113");
    assert!(error.message.contains("last(xs)"), "{}", error.message);
    let error = check("main()->I{var xs=[1];let t=xs.peek();0}").unwrap_err();
    assert!(error.message.contains("call last(xs)"), "{}", error.message);
    // Both are pure, so spawned functions may use them.
    assert!(
        check(
            "f()->Option<I>{last(pop([1,2]))} main()->Result<Option<I>,TaskError>{join(spawn f())}"
        )
        .is_ok()
    );
    assert_eq!(
        check("pop(x:I)->I{x} main()->I{1}").unwrap_err().code,
        "E106"
    );
}

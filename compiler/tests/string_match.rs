use std::process::Command;

use tokit_compiler::{check, format, native, run};

fn native_output(source: &str) -> String {
    let binary = std::env::temp_dir().join(format!(
        "tokit-string-match-{}-{}{}",
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
fn string_patterns_agree_between_backends() {
    let source = r#"cmd(c:String)->String{match c{"add"=>"plus","rm"=>"minus","\"q\"\n"=>"quote",""=>"empty",_=>"other "+c}}
main()->[String]{map(["add","rm","x","","\"q\"\n","ünï"],|c|cmd(c))}"#;
    let expected = r#"["plus","minus","other x","empty","quote","other ünï"]"#;
    assert_eq!(run(source).unwrap().to_string(), expected);
    let formatted = format::format(source).unwrap();
    assert_eq!(run(&formatted).unwrap().to_string(), expected);
    if Command::new("rustc").arg("--version").output().is_ok() {
        assert_eq!(native_output(source), expected);
    }
}

#[test]
fn string_matches_need_a_wildcard_and_distinct_arms() {
    for source in [
        r#"main()->I{match "a"{"a"=>1}}"#,
        r#"main()->I{match "a"{"a"=>1,"a"=>2,_=>3}}"#,
        r#"main()->I{match 1{"a"=>1,_=>2}}"#,
        r#"main()->I{match "a"{1=>1,_=>2}}"#,
        r#"main()->I{match "a"{_=>1,"a"=>2}}"#,
    ] {
        assert_eq!(check(source).unwrap_err().code, "E116", "{source}");
    }
}

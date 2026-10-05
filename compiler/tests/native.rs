use std::path::Path;
use std::process::Command;

use tokit_compiler::{check, native, run};

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("tokit-{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn native_emits_only_used_runtime_sections() {
    let source = "fn main()->i32{42}";
    let generated = native::emit(&check(source).unwrap(), source).unwrap();
    assert!(generated.contains("fn __tok_configure_runtime"));
    for unused in [
        "struct __TokBytes",
        "enum __TokIoError",
        "enum __TokParseError",
        "fn __tok_utf8_bytes",
        "enum __TokTaskError",
        "fn __tok_read_bytes",
        "fn __tok_add_i64",
    ] {
        assert!(!generated.contains(unused), "unexpected {unused}");
    }
}

#[test]
fn native_borrows_byte_length_without_cloning_the_binding() {
    let source = "fn main()->i32{let data=utf8_encode(\"abc\");len(data)+len(data)}";
    let generated = native::emit(&check(source).unwrap(), source).unwrap();
    assert!(generated.contains("__tok_len(&u_64617461.0,"));
    assert!(!generated.contains("__tok_len(&(u_64617461.clone()).0,"));
}

#[test]
fn native_output_matches_reference_interpreter() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let directory = temporary_directory("native-test");
    for name in [
        "answer",
        "factorial",
        "sum_positive",
        "checked_division",
        "checked_division_enum",
        "generic_pair",
        "match_result",
        "integer_match",
        "signed_literals",
        "payload_enum",
        "record",
        "option_lookup",
        "parse_argument",
        "parse_numbers",
        "iterative_factorial",
        "loop_control",
        "task_square",
    ] {
        let source =
            std::fs::read_to_string(root.join("examples").join(format!("{name}.tok"))).unwrap();
        let expected = run(&source).unwrap().to_string();
        let program = check(&source).unwrap();
        let output = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        native::build(&program, &source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            expected,
            "{name}"
        );
    }
    for (name, source) in [
        (
            "match_result",
            r#"fn describe(x:Result<i32,String>)->String{match x{Ok(v)=>if v>0{"positive"}else{"zero"},Err(e)=>e}} fn main()->String{describe(Err("bad"))}"#,
        ),
        (
            "match_enum",
            "enum Mode{Add,Sub} fn apply(m:Mode,a:i32,b:i32)->i32{match m{Mode::Add=>a+b,Mode::Sub=>a-b}} fn main()->i32{apply(Mode::Sub,7,2)}",
        ),
        (
            "payload_enum_match",
            "enum Shape{Point,Square(i32),Text(String)} fn describe(s:Shape)->String{match s{Shape::Point=>\"point\",Shape::Square(n)=>if n>0{\"square\"}else{\"empty\"},Shape::Text(label)=>label}} fn main()->String{describe(Shape::Text(\"hello\"))}",
        ),
        (
            "payload_enum_render",
            "enum Shape{Point,Square(i32)} fn main()->Shape{Shape::Square(7)}",
        ),
        (
            "recursive_array_enum",
            "enum List{Nil,Cons([List])} fn main()->List{List::Cons([List::Nil])}",
        ),
        ("match_bool", "fn main()->i32{match true{true=>1,false=>2}}"),
        (
            "match_bool_wildcard",
            "fn main()->i32{match false{true=>1,_=>2}}",
        ),
        (
            "match_result_wildcard",
            "fn main()->i32{match Ok(5){Ok(v)=>v,_=>0}}",
        ),
        (
            "match_direct_result",
            "fn main()->i32{match Ok(1){Ok(v)=>v,Err(e)=>0}}",
        ),
        (
            "match_early_return",
            "fn f(x:Result<i32,i32>)->i32{match x{Ok(v)=>{return v;},Err(e)=>e}} fn main()->i32{f(Ok(7))}",
        ),
        (
            "generic_pair_i32",
            "struct Pair<T>{left:T,right:T} fn flip<T>(p:Pair<T>)->Pair<T>{Pair(p.right,p.left)} fn main()->Pair<i32>{flip(Pair(1,2))}",
        ),
        (
            "generic_pair_string",
            "struct Pair<T>{left:T,right:T} fn flip<T>(p:Pair<T>)->Pair<T>{Pair(p.right,p.left)} fn main()->Pair<String>{flip(Pair(\"a\",\"b\"))}",
        ),
        (
            "generic_recursive_array",
            "struct Node{child:Wrap<Node>} struct Wrap<T>{xs:[T]} fn empty<T>(xs:[T])->Wrap<T>{Wrap(xs)} fn main()->Node{let xs:[Node]=[];Node(empty(xs))}",
        ),
        (
            "generic_result_join",
            "struct Pair<T>{left:T,right:T} fn main()->Pair<Result<i32,i32>>{Pair(Ok(1),Err(2))}",
        ),
        (
            "generic_two_params",
            "struct Pair<A,B>{left:A,right:B} fn flip<A,B>(p:Pair<A,B>)->Pair<B,A>{Pair(p.right,p.left)} fn main()->Pair<String,i32>{flip(Pair(5,\"x\"))}",
        ),
        (
            "enum_error",
            "enum DivError{DivZero,Overflow} fn div(a:i32,b:i32)->Result<i32,DivError>{if b==0{Err(DivError::DivZero)}else{Ok(a/b)}} fn main()->Result<i32,DivError>{div(7,0)}",
        ),
        (
            "enum_array",
            "enum Flag{On,Off} fn main()->[Flag]{[Flag::On,Flag::Off]}",
        ),
        ("empty_enum", "enum Void{} fn main()->i32{1}"),
        (
            "record_project",
            r#"struct Point{x:i32,y:i32} struct Label{point:Point,text:String} fn make(n:i32)->Label{Label(Point(n,n+1),"✓")} fn main()->i32{make(4).point.y}"#,
        ),
        (
            "record_render",
            r#"struct Pair{left:i32,right:String} fn main()->Pair{Pair(3,"x")}"#,
        ),
        ("empty_record", "struct Empty{} fn main()->Empty{Empty()}"),
        (
            "recursive_array_record",
            "struct Node{children:[Node]} fn main()->Node{Node([Node([])])}",
        ),
        ("array_index", "fn main()->String{[\"a\",\"✓\"][1]}"),
        (
            "array_read_after_push",
            "fn main()->String{var xs:[String]=[\"a\"];let first:String=xs[0];xs.push(\"b\");first+xs[len(xs)-1]}",
        ),
        ("nested_array_index", "fn main()->i32{[[1,2],[3,4]][1][0]}"),
        (
            "byte_length_after_push",
            "fn main()->i32{var data=utf8_encode(\"abc\");let before=len(data);data.push(100);before+len(data)}",
        ),
        (
            "utf8_strings",
            r#"fn greet(x:String)->String{"Grüß, "+x+"\n"} fn main()->[String]{[greet("世界"),"✓"]}"#,
        ),
        ("discarded_result", "fn main()->i32{Ok(1);0}"),
        (
            "result_array",
            "fn main()->[Result<i32,i32>]{[Ok(1),Err(2)]}",
        ),
    ] {
        let expected = run(source).unwrap().to_string();
        let program = check(source).unwrap();
        let output = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        native::build(&program, source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            expected,
            "{name}"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_array_bounds_match_reference_diagnostic() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(), Ok("1"));
        return;
    }
    let directory = temporary_directory("bounds-test");
    for (name, source) in [
        ("positive", "fn main()->i32{[1][2]}"),
        ("negative", "fn main()->i32{[1][0-1]}"),
        ("empty", "fn main()->i32{let xs:[i32]=[];xs[0]}"),
    ] {
        let expected = run(source).unwrap_err().display(source);
        let output = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        assert!(!result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stderr).trim(), expected);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn native_checked_arithmetic_reports_e201() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
        return;
    }
    let source = "fn main()->i32{2147483647+1}";
    let program = check(source).unwrap();
    let directory = temporary_directory("overflow-test");
    let output = directory.join(format!("overflow{}", std::env::consts::EXE_SUFFIX));
    native::build(&program, source, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).starts_with("E201@"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn recursion_limit_matches_interpreter() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
        return;
    }
    let source = "fn recur()->i32{recur()} fn main()->i32{recur()}";
    let directory = temporary_directory("recursion-test");
    let input = directory.join("recursion.tok");
    std::fs::write(&input, source).unwrap();
    let interpreted = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!interpreted.status.success());
    assert!(String::from_utf8_lossy(&interpreted.stderr).starts_with("E202@"));
    let output = directory.join(format!("recursion{}", std::env::consts::EXE_SUFFIX));
    native::build(&check(source).unwrap(), source, &output).unwrap();
    let compiled = Command::new(&output).output().unwrap();
    assert!(!compiled.status.success());
    assert!(String::from_utf8_lossy(&compiled.stderr).starts_with("E202@"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn cli_build_command_produces_executable() {
    if Command::new("rustc").arg("--version").output().is_err() {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let directory = temporary_directory("cli-build-test");
    let input = root.join("examples/answer.tok");
    let output = directory.join(format!("answer{}", std::env::consts::EXE_SUFFIX));
    let build = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args([
            "build",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&output).output().unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8(run.stdout).unwrap().trim(), "42");
    std::fs::remove_dir_all(directory).unwrap();
}

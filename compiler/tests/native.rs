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
        "record",
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
        ("nested_array_index", "fn main()->i32{[[1,2],[3,4]][1][0]}"),
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

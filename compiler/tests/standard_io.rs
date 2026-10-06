use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tokit_compiler::{check, native};

fn temporary_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "tokit-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn piped(mut command: Command, input: &str) -> (Option<i32>, String, String) {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        String::from_utf8(output.stderr).unwrap(),
    )
}

fn both(source: &str, input: &str) -> Vec<(Option<i32>, String, String)> {
    let directory = temporary_directory("stdio");
    let path = directory.join("main.tok");
    std::fs::write(&path, source).unwrap();
    let mut interpreter = Command::new(env!("CARGO_BIN_EXE_tok"));
    interpreter.arg("run").arg(&path);
    let mut results = vec![piped(interpreter, input)];
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = directory.join(format!("main{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(source).unwrap(), source, &binary).unwrap();
        results.push(piped(Command::new(&binary), input));
    } else {
        assert_ne!(
            std::env::var("TOKIT_REQUIRE_NATIVE").as_deref(),
            Ok("1"),
            "rustc required by CI"
        );
    }
    std::fs::remove_dir_all(Path::new(&directory)).unwrap();
    results
}

const ECHO: &str = r#"main()->Unit{
print("name?");
let name=match read_line(){Some(v)=>v,None=>"nobody"};
print("hi "+name);
let rest=match read_stdin(){Ok(text)=>text,Err(e)=>"?"};
var count=0;
for line in lines(rest){count=count+1;}
if count>1{print("many");exit(3);}
print("few");
}"#;

#[test]
fn print_read_line_and_exit_agree_between_backends() {
    for (code, stdout, stderr) in both(ECHO, "Ada\r\nx\ny\n") {
        assert_eq!(
            (code, stdout.as_str(), stderr.as_str()),
            (Some(3), "name?\nhi Ada\nmany\n", "")
        );
    }
    for (code, stdout, _) in both(ECHO, "") {
        assert_eq!(
            (code, stdout.as_str()),
            (Some(0), "name?\nhi nobody\nfew\n")
        );
    }
}

#[test]
fn printed_output_precedes_results_and_runtime_failures() {
    let source = "main()->I{print(\"before\");let zero=0;print(\"after\");1/zero}";
    for (code, stdout, stderr) in both(source, "") {
        assert_eq!(code, Some(1));
        assert_eq!(stdout, "before\nafter\n");
        assert!(stderr.starts_with("E201@"), "{stderr}");
    }
    for (code, stdout, _) in both("main()->I{print(\"x\");7}", "") {
        assert_eq!((code, stdout.as_str()), (Some(0), "x\n7\n"));
    }
}

#[test]
fn invalid_utf8_input_is_reported() {
    let source = "main()->Result<String,IoError>{read_stdin()}";
    let directory = temporary_directory("stdio-utf8");
    let path = directory.join("main.tok");
    std::fs::write(&path, source).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&[0xff, 0xfe])
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "Err(IoError::InvalidUtf8)"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn input_and_output_are_effects_outside_tasks() {
    for source in [
        "f()->Unit{print(\"x\")} main()->Unit{let t=spawn f();}",
        "f()->Option<String>{read_line()} main()->Unit{let t=spawn f();}",
        "f()->I{exit(1)} main()->Unit{let t=spawn f();}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E117", "{source}");
    }
    assert_eq!(check("main()->Unit{exit(1)}").map(|_| ()), Ok(()));
    assert_eq!(
        check("struct print{x:I} main()->I{1}").unwrap_err().code,
        "E106"
    );
}

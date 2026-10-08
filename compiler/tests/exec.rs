//! `exec(program,args,stdin)` runs another program under `--allow-run`,
//! identically in the interpreter and native executables. The child process
//! is `tok` itself, so the test needs no other tools.

mod common;

use std::process::Command;

use tokit_compiler::check;

const PROGRAM: &str = r#"show(r:Result<Process,IoError>)->String{match r{Ok(p)=>String(p.status)+"|"+trim(p.stdout)+"|"+String(contains(p.stderr,"E101")),Err(IoError::Denied)=>"denied",Err(IoError::NotFound)=>"missing",Err(e)=>"other"}}
main()->[String]{let a=args();[show(exec(a[0],["run",a[1]],"tokit\n")),show(exec(a[0],["check",a[2]],"")),show(exec("tok-no-such-program",[],""))]}"#;

#[test]
fn exec_runs_granted_programs_in_both_backends() {
    let directory = std::env::temp_dir().join(format!("tokit-exec-{}", common::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let echo = directory.join("echo.tok");
    std::fs::write(
        &echo,
        "main()->Unit{match read_line(){Some(l)=>print(upper(l)),None=>print(\"none\")}}",
    )
    .unwrap();
    let bad = directory.join("bad.tok");
    std::fs::write(&bad, "main()->I{missing}").unwrap();
    let program = directory.join("main.tok");
    std::fs::write(&program, PROGRAM).unwrap();
    let tok = env!("CARGO_BIN_EXE_tok");
    let arguments = [tok, echo.to_str().unwrap(), bad.to_str().unwrap()];
    let expected = "[\"0|TOKIT|false\",\"1||true\",\"missing\"]";
    let interpreted = Command::new(tok)
        .args(["run", "--allow-run", "*"])
        .arg(&program)
        .arg("--")
        .args(arguments)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&interpreted.stdout).trim(),
        expected
    );
    // A grant for one program does not cover another.
    let narrow = Command::new(tok)
        .args(["run", "--allow-run", "git"])
        .arg(&program)
        .arg("--")
        .args(arguments)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&narrow.stdout).trim(),
        "[\"denied\",\"denied\",\"denied\"]"
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = directory.join(format!("main{}", std::env::consts::EXE_SUFFIX));
        tokit_compiler::native::build(&check(PROGRAM).unwrap(), PROGRAM, &binary).unwrap();
        let native = Command::new(&binary)
            .args(["--allow-run", "*", "--"])
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&native.stdout).trim(), expected);
        let denied = Command::new(&binary)
            .arg("--")
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&denied.stdout).trim(),
            "[\"denied\",\"denied\",\"denied\"]"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn exec_is_an_effect() {
    assert_eq!(
        check("f()->I{let r=exec(\"x\",[],\"\");1} main()->I{let t=spawn f();1}")
            .unwrap_err()
            .code,
        "E117"
    );
    assert_eq!(
        check("struct Process{x:I} main()->I{1}").unwrap_err().code,
        "E106"
    );
}

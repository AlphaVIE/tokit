//! `tok run --native` compiles once into `$TOK_HOME/cache/native` and runs the
//! cached executable with the same grants, arguments, input, and exit status.

mod common;

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn tok(home: &std::path::Path, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .env("TOK_HOME", home)
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
    child.wait_with_output().unwrap()
}

#[test]
fn native_runs_match_the_interpreter_and_reuse_the_cache() {
    if Command::new("rustc").arg("--version").output().is_err() {
        return;
    }
    let directory = std::env::temp_dir().join(format!("tokit-run-native-{}", common::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let home = directory.join("home");
    let data = directory.join("data.txt");
    std::fs::write(&data, "from a file").unwrap();
    let program = directory.join("main.tok");
    std::fs::write(
        &program,
        "main()->Unit{let a=args();let line=match read_line(){Some(l)=>l,None=>\"\"};let text=match read_text(a[0]){Ok(t)=>t,Err(e)=>\"denied\"};print(join(a,\"+\")+\"|\"+line+\"|\"+text);if len(a)>1{exit(3);}}",
    )
    .unwrap();
    let path = program.to_str().unwrap();
    let file = data.to_str().unwrap();
    for (grant, arguments, status) in [(true, vec![file], 0), (false, vec![file, "x"], 3)] {
        let mut args = vec!["run"];
        let mut native = vec!["run", "--native"];
        if grant {
            args.extend(["--allow-read", file]);
            native.extend(["--allow-read", file]);
        }
        for list in [&mut args, &mut native] {
            list.extend([path, "--"]);
            list.extend(arguments.iter().copied());
        }
        let expected = tok(&home, &args, "typed\n");
        let actual = tok(&home, &native, "typed\n");
        assert_eq!(
            String::from_utf8_lossy(&actual.stdout),
            String::from_utf8_lossy(&expected.stdout),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.status.code(), Some(status));
        assert_eq!(expected.status.code(), Some(status));
    }
    let cached: Vec<_> = std::fs::read_dir(home.join("cache/native"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(cached.len(), 1, "one program, one cached build: {cached:?}");
    let rejected = tok(&home, &["run", "--native", "--json", path], "");
    assert_eq!(rejected.status.code(), Some(2));
    std::fs::remove_dir_all(directory).unwrap();
}

//! `http_request` with `https://` URLs goes through the system curl, which the
//! test replaces with a recording fake (`TOKIT_CURL`) so it runs offline.
//! Set `TOKIT_NETWORK_TESTS=1` to also fetch https://example.com for real.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const PROGRAM: &str = r#"main()->String{let a=args();var h:Map<String,String>=Map();h["X-Token"]="abc";match http_request("POST",a[0],h,"payload"){Ok(r)=>String(r.status)+"|"+get_or(r.headers,"x-reply","none")+"|"+r.body,Err(e)=>match e{IoError::Denied=>"denied",_=>"failed"}}}"#;

/// A curl stand-in that records its arguments and stdin and prints a response.
fn fake_curl(directory: &Path) -> PathBuf {
    let log = directory.join("curl-args.txt");
    if cfg!(windows) {
        let script = directory.join("curl.cmd");
        std::fs::write(
            &script,
            format!(
                "@echo off\r\necho %* > \"{}\"\r\nmore > \"{}.stdin\"\r\necho HTTP/1.1 201 Created\r\necho X-Reply: yes\r\necho.\r\n<nul set /p =made\r\nexit /b 0\r\n",
                log.display(),
                log.display()
            ),
        )
        .unwrap();
        script
    } else {
        let script = directory.join("curl.sh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" > '{}'\ncat > '{}.stdin'\nprintf 'HTTP/1.1 201 Created\\r\\nX-Reply: yes\\r\\n\\r\\nmade'\n",
                log.display(),
                log.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        script
    }
}

fn run(command: &mut Command, curl: &Path) -> String {
    let output = command.env("TOKIT_CURL", curl).output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn https_requests_use_the_system_client_in_both_backends() {
    let directory = std::env::temp_dir().join(format!("tokit-https-{}", common::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let curl = fake_curl(&directory);
    let program = directory.join("main.tok");
    std::fs::write(&program, PROGRAM).unwrap();
    let url = "https://api.example.test:8443/v1/items?x=1";
    let tok = env!("CARGO_BIN_EXE_tok");
    let mut binaries = vec![];
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = directory.join(format!("main{}", std::env::consts::EXE_SUFFIX));
        let source = std::fs::read_to_string(&program).unwrap();
        tokit_compiler::native::build(&tokit_compiler::check(&source).unwrap(), &source, &binary)
            .unwrap();
        binaries.push(binary);
    }
    let interpreter = |grant: &str| {
        let mut command = Command::new(tok);
        command
            .args(["run", "--allow-net", grant])
            .arg(&program)
            .args(["--", url]);
        command
    };
    let expected = "\"201|yes|made\"";
    assert_eq!(
        run(&mut interpreter("api.example.test:8443"), &curl),
        expected
    );
    // Batch files see Windows-quoted arguments; compare without quotes.
    let args = std::fs::read_to_string(directory.join("curl-args.txt"))
        .unwrap()
        .replace('"', "");
    for part in [
        "--proto =https",
        "x-token: abc",
        "--data-binary @-",
        "--request POST",
        url,
    ] {
        assert!(args.contains(part), "{part} missing from {args}");
    }
    let sent = std::fs::read_to_string(directory.join("curl-args.txt.stdin")).unwrap();
    assert_eq!(sent.trim(), "payload");
    // The grant is checked against host:port before curl starts.
    std::fs::remove_file(directory.join("curl-args.txt")).unwrap();
    assert_eq!(
        run(&mut interpreter("api.example.test:443"), &curl),
        "\"denied\""
    );
    assert!(!directory.join("curl-args.txt").exists());
    for binary in binaries {
        let mut command = Command::new(&binary);
        command.args(["--allow-net", "api.example.test:8443", "--", url]);
        assert_eq!(run(&mut command, &curl), expected);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn https_reaches_a_real_server_when_enabled() {
    if std::env::var("TOKIT_NETWORK_TESTS").as_deref() != Ok("1") {
        return;
    }
    let source = r#"main()->String{let h:Map<String,String>=Map();match http_request("GET","https://example.com/",h,""){Ok(r)=>String(r.status)+" "+String(contains(r.body,"Example Domain")),Err(e)=>"failed"}}"#;
    let directory = std::env::temp_dir().join(format!("tokit-https-real-{}", common::nonce()));
    std::fs::create_dir_all(&directory).unwrap();
    let program = directory.join("main.tok");
    std::fs::write(&program, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", "example.com:443"])
        .arg(&program)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "\"200 true\""
    );
    std::fs::remove_dir_all(directory).unwrap();
}

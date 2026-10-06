//! `listen`/`accept`/`http_read`/`http_write`: a stateful server loop written
//! in Tokit (examples/crud_service.tok) answers a scripted client.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use tokit_compiler::{check, native};

fn free_address() -> String {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    probe.local_addr().unwrap().to_string()
}

/// Send one request, retrying the connection while the server starts.
fn request(addr: &str, method: &str, path: &str, body: &str) -> String {
    let mut stream = None;
    for _ in 0..200 {
        if let Ok(connected) = TcpStream::connect(addr) {
            stream = Some(connected);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let mut stream = stream.expect("server did not start");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nhost: test\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let status = response.split(' ').nth(1).unwrap_or_default().to_owned();
    let content = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    format!("{status} {content}")
}

const SCRIPT: [(&str, &str, &str, &str); 9] = [
    ("PUT", "/items/b", "two", "201 created"),
    ("PUT", "/items/a", "one", "201 created"),
    ("GET", "/items/a", "", "200 one"),
    ("PUT", "/items/a", "uno", "200 replaced"),
    ("GET", "/items", "", "200 a\nb"),
    ("DELETE", "/items/b", "", "204 "),
    ("GET", "/items/b", "", "404 not found"),
    ("PATCH", "/items/a", "", "405 method not allowed"),
    ("GET", "/items", "", "200 a"),
];

fn exercise(mut server: Child, addr: &str) {
    let mut answers = Vec::new();
    for (method, path, body, _) in SCRIPT {
        answers.push(request(addr, method, path, body));
    }
    // A malformed request is answered without reaching the routing code.
    let mut raw = TcpStream::connect(addr).unwrap();
    raw.write_all(b"nonsense\r\n\r\n").unwrap();
    let mut reply = String::new();
    raw.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 400"), "{reply}");
    let status = server.wait().unwrap();
    let mut stdout = String::new();
    server
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    assert!(status.success(), "{stdout}");
    assert_eq!(stdout.trim(), format!("listening on {addr}"));
    let expected: Vec<_> = SCRIPT.iter().map(|step| step.3.to_owned()).collect();
    assert_eq!(answers, expected);
}

fn example() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/crud_service.tok")
}

#[test]
fn crud_service_keeps_state_across_requests_in_both_backends() {
    let requests = (SCRIPT.len() + 1).to_string();
    let addr = free_address();
    let server = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr])
        .arg(example())
        .args(["--", &addr, &requests])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    exercise(server, &addr);
    if Command::new("rustc").arg("--version").output().is_ok() {
        let source = std::fs::read_to_string(example()).unwrap();
        let binary = std::env::temp_dir().join(format!(
            "tokit-crud-bin-{}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        native::build(&check(&source).unwrap(), &source, &binary).unwrap();
        let addr = free_address();
        let server = Command::new(&binary)
            .args(["--allow-net", &addr, "--", &addr, &requests])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        exercise(server, &addr);
        std::fs::remove_file(binary).unwrap();
    }
}

#[test]
fn listening_needs_a_grant_and_is_an_effect() {
    let source = r#"main()->String{match listen("127.0.0.1:0"){Ok(l)=>"listening",Err(e)=>match e{IoError::Denied=>"denied",_=>"other"}}}"#;
    assert_eq!(
        tokit_compiler::run(source).unwrap().to_string(),
        "\"denied\""
    );
    for call in [
        "listen(\"127.0.0.1:1\")",
        "match listen(\"x\"){Ok(l)=>accept(l),Err(e)=>Err(e)}",
    ] {
        let program = format!("f()->I{{let r={call};1}} main()->I{{let t=spawn f();1}}");
        assert_eq!(check(&program).unwrap_err().code, "E117", "{call}");
    }
    assert_eq!(
        check("struct Listener{x:I} main()->I{1}").unwrap_err().code,
        "E106"
    );
}

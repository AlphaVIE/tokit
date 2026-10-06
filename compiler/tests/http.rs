use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tokit_compiler::{check, native};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn native_available() -> bool {
    Command::new("rustc").arg("--version").output().is_ok()
}

fn build(source_path: &Path, label: &str) -> PathBuf {
    let source = std::fs::read_to_string(source_path).unwrap();
    let binary = std::env::temp_dir().join(format!(
        "tokit-http-{label}-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(&source).unwrap(), &source, &binary).unwrap();
    binary
}

/// Send one raw request, retrying until the server accepts connections.
fn exchange(addr: &str, request: &str) -> String {
    for _ in 0..200 {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream.write_all(request.as_bytes()).unwrap();
                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();
                return response;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    panic!("server at {addr} never accepted a connection");
}

fn check_server(server: Child, addr: &str) {
    let hello = exchange(addr, "GET /hello?name=ada HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(hello.starts_with("HTTP/1.1 200 OK\r\n"), "{hello}");
    assert!(hello.contains("content-type: text/plain\r\n"), "{hello}");
    assert!(hello.ends_with("\r\n\r\nhello name=ada"), "{hello}");
    let echo = exchange(
        addr,
        "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 4\r\n\r\nping",
    );
    assert!(
        echo.ends_with("content-length: 4\r\nconnection: close\r\n\r\nping"),
        "{echo}"
    );
    let missing = exchange(addr, "GET /nope HTTP/1.1\r\n\r\n");
    assert!(
        missing.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{missing}"
    );
    let output = server.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        format!("listening on {addr}\ndone\n")
    );
}

#[test]
fn http_server_answers_in_both_backends() {
    let example = root().join("examples/http_server.tok");
    let addr = format!("127.0.0.1:{}", free_port());
    let server = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr])
        .arg(&example)
        .args(["--", &addr, "3"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    check_server(server, &addr);
    if native_available() {
        let binary = build(&example, "server");
        let addr = format!("127.0.0.1:{}", free_port());
        let server = Command::new(&binary)
            .args(["--allow-net", &addr, "--", &addr, "3"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        check_server(server, &addr);
        std::fs::remove_file(binary).unwrap();
    }
}

const CLIENT: &str = r#"main()->[String]{var h:Map<String,String>=Map();h["x-token"]="t1";let a=args();match http_request("POST",a[0],h,"data"){Ok(r)=>[String(r.status),r.body,get_or(r.headers,"x-reply","?")],Err(e)=>["error"]}}"#;

/// Serve one canned chunked response and return what the client sent.
fn canned_server() -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        let mut buffer = [0u8; 1024];
        while !received.ends_with(b"data") {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            received.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 201 Created\r\nX-Reply: yes\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n")
            .unwrap();
        String::from_utf8(received).unwrap()
    });
    (addr, handle)
}

fn check_client(output: std::process::Output, sent: String) {
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        r#"["201","abcde","yes"]"#
    );
    assert!(sent.starts_with("POST /path?q=1 HTTP/1.1\r\n"), "{sent}");
    assert!(sent.contains("\r\nx-token: t1\r\n"), "{sent}");
    assert!(sent.ends_with("\r\n\r\ndata"), "{sent}");
}

#[test]
fn http_client_decodes_chunked_responses_in_both_backends() {
    let directory = std::env::temp_dir().join(format!("tokit-http-client-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let program = directory.join("client.tok");
    std::fs::write(&program, CLIENT).unwrap();

    let (addr, server) = canned_server();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr])
        .arg(&program)
        .args(["--", &format!("http://{addr}/path?q=1")])
        .output()
        .unwrap();
    check_client(output, server.join().unwrap());

    if native_available() {
        let binary = build(&program, "client");
        let (addr, server) = canned_server();
        let output = Command::new(&binary)
            .args(["--allow-net", "*", "--", &format!("http://{addr}/path?q=1")])
            .output()
            .unwrap();
        check_client(output, server.join().unwrap());
        std::fs::remove_file(binary).unwrap();
    }

    // Without a matching grant the request is refused before connecting.
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", "127.0.0.1:1"])
        .arg(&program)
        .args(["--", "http://127.0.0.1:2/"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        r#"["error"]"#
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn network_builtins_are_typed_effects() {
    for source in [
        "f()->Result<Response,IoError>{http_request(\"GET\",\"http://a/\",Map(),\"\")} main()->I{let t=spawn f();1}",
        "f()->Result<Unit,IoError>{serve(\"a:1\",1,|r|Response(200,Map(),\"\"))} main()->I{let t=spawn f();1}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E117", "{source}");
    }
    assert_eq!(
        check("main()->I{let r=Response(\"200\",Map(),\"\");1}")
            .unwrap_err()
            .code,
        "E102"
    );
    assert_eq!(
        check("struct Request{x:I} main()->I{1}").unwrap_err().code,
        "E106"
    );
    assert!(
        check("main()->String{let r=Request(\"GET\",\"/\",\"\",Map(),\"\");r.method+r.path}")
            .is_ok()
    );
}

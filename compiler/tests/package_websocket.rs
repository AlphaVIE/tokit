//! The websocket package: examples/websocket_echo serves an independent
//! RFC 6455 client written here and the package's own client, in both backends.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn example(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/websocket_echo")
        .join(file)
}

fn free_address() -> String {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    probe.local_addr().unwrap().to_string()
}

fn connect(addr: &str) -> TcpStream {
    for _ in 0..200 {
        if let Ok(stream) = TcpStream::connect(addr) {
            return stream;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("server did not start");
}

/// A client frame: always masked, as RFC 6455 requires.
fn frame(fin: bool, op: u8, payload: &[u8]) -> Vec<u8> {
    let mask = [0x37, 0xfa, 0x21, 0x3d];
    let mut out = vec![if fin { 0x80 } else { 0 } | op];
    match payload.len() {
        n if n < 126 => out.push(0x80 | n as u8),
        n if n < 65536 => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(0x80 | 127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    out
}

/// A server frame: never masked.
fn read_frame(stream: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut head = [0u8; 2];
    stream.read_exact(&mut head).unwrap();
    assert_eq!(head[1] & 0x80, 0, "server frames must not be masked");
    let length = match head[1] & 0x7f {
        126 => {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended).unwrap();
            u16::from_be_bytes(extended) as usize
        }
        127 => {
            let mut extended = [0u8; 8];
            stream.read_exact(&mut extended).unwrap();
            u64::from_be_bytes(extended) as usize
        }
        n => n as usize,
    };
    let mut payload = vec![0u8; length];
    stream.read_exact(&mut payload).unwrap();
    assert_eq!(head[0] & 0x80, 0x80, "server frames are not fragmented");
    (head[0] & 0x0f, payload)
}

fn rfc_client(addr: &str) {
    let mut stream = connect(addr);
    // The handshake example from RFC 6455 section 1.3.
    stream
        .write_all(b"GET /chat HTTP/1.1\r\nHost: test\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n")
        .unwrap();
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).unwrap();
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    assert!(
        head.contains("sec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"),
        "{head}"
    );
    stream.write_all(&frame(true, 1, b"hello")).unwrap();
    assert_eq!(read_frame(&mut stream), (1, b"echo: hello".to_vec()));
    // A fragmented message with a ping between its fragments.
    stream.write_all(&frame(false, 1, "ab".as_bytes())).unwrap();
    stream.write_all(&frame(true, 9, b"are you there")).unwrap();
    stream.write_all(&frame(true, 0, "cö".as_bytes())).unwrap();
    assert_eq!(read_frame(&mut stream), (10, b"are you there".to_vec()));
    assert_eq!(
        read_frame(&mut stream),
        (1, "echo: abcö".as_bytes().to_vec())
    );
    // Binary data with a 16-bit and a 64-bit length.
    let medium: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    stream.write_all(&frame(true, 2, &medium)).unwrap();
    assert_eq!(read_frame(&mut stream), (2, medium));
    let large = vec![7u8; 70_000];
    stream.write_all(&frame(true, 2, &large)).unwrap();
    assert_eq!(read_frame(&mut stream), (2, large));
    stream.write_all(&frame(true, 8, &[3, 232])).unwrap();
    assert_eq!(read_frame(&mut stream), (8, vec![3, 232]));
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).unwrap();
    assert!(rest.is_empty());
}

fn plain_request(addr: &str) {
    let mut stream = connect(addr);
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: test\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
    assert!(
        response.ends_with("expected a WebSocket upgrade"),
        "{response}"
    );
}

fn exercise(mut server: Child, addr: &str, client: Command) {
    rfc_client(addr);
    plain_request(addr);
    let mut client = client;
    let output = client.output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "echo: hi|2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = server.wait().unwrap();
    let mut stdout = String::new();
    server
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    assert!(status.success(), "{stdout}");
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        [
            format!("listening on {addr}").as_str(),
            "connection closed after 4 messages",
            "rejected: expected a WebSocket upgrade",
            "connection closed after 2 messages",
        ]
    );
}

#[test]
fn websocket_echo_serves_rfc_and_package_clients_in_both_backends() {
    let tok = env!("CARGO_BIN_EXE_tok");
    let addr = free_address();
    let server = Command::new(tok)
        .args(["run", "--allow-net", &addr])
        .arg(example("main.tok"))
        .args(["--", &addr, "3"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut client = Command::new(tok);
    client
        .args(["run", "--allow-net", &addr])
        .arg(example("client.tok"))
        .args(["--", &addr]);
    exercise(server, &addr, client);

    if Command::new("rustc").arg("--version").output().is_ok() {
        let directory = std::env::temp_dir().join(format!("tokit-ws-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut binaries = Vec::new();
        for name in ["main", "client"] {
            let binary = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            let built = Command::new(tok)
                .arg("build")
                .arg(example(&format!("{name}.tok")))
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            binaries.push(binary);
        }
        let addr = free_address();
        let server = Command::new(&binaries[0])
            .args(["--allow-net", &addr, "--", &addr, "3"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut client = Command::new(&binaries[1]);
        client.args(["--allow-net", &addr, "--", &addr]);
        exercise(server, &addr, client);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

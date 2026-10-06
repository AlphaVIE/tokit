//! `serve(addr,limit,workers,handler)`: native programs answer requests in
//! parallel; the reference interpreter answers the same requests in order.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tokit_compiler::{check, native};

fn free_address() -> String {
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    probe.local_addr().unwrap().to_string()
}

fn get(addr: &str, path: &str) -> String {
    let mut stream = None;
    for _ in 0..200 {
        if let Ok(connected) = TcpStream::connect(addr) {
            stream = Some(connected);
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let mut stream = stream.expect("server did not start");
    write!(stream, "GET {path} HTTP/1.1\r\nhost: test\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_owned()
}

fn example() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/concurrent_http.tok")
}

/// Send `count` slow requests at once and return the answers and elapsed time.
fn burst(addr: &str, count: usize) -> (Vec<String>, Duration) {
    // Wait until the server answers before timing anything.
    assert_eq!(get(addr, "/"), "ok");
    let started = Instant::now();
    let clients: Vec<_> = (0..count)
        .map(|_| {
            let addr = addr.to_owned();
            std::thread::spawn(move || get(&addr, "/work?ms=300"))
        })
        .collect();
    let answers = clients
        .into_iter()
        .map(|client| client.join().unwrap())
        .collect();
    (answers, started.elapsed())
}

fn finish(mut server: Child) -> String {
    let status = server.wait().unwrap();
    let mut stdout = String::new();
    server
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    assert!(status.success(), "{stdout}");
    stdout
}

#[test]
fn worker_pool_answers_requests_in_parallel() {
    if Command::new("rustc").arg("--version").output().is_err() {
        return;
    }
    let source = std::fs::read_to_string(example()).unwrap();
    let binary = std::env::temp_dir().join(format!(
        "tokit-concurrent-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    native::build(&check(&source).unwrap(), &source, &binary).unwrap();
    let addr = free_address();
    let server = Command::new(&binary)
        .args(["--allow-net", &addr, "--", &addr, "9", "8"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let (answers, elapsed) = burst(&addr, 8);
    assert!(
        answers.iter().all(|answer| answer == "worked 300 ms"),
        "{answers:?}"
    );
    // Eight sequential requests would take at least 2.4 s.
    assert!(elapsed < Duration::from_millis(1500), "took {elapsed:?}");
    assert_eq!(
        finish(server).lines().collect::<Vec<_>>(),
        [
            format!("listening on {addr} with 8 workers").as_str(),
            "done"
        ]
    );
    std::fs::remove_file(binary).unwrap();
}

#[test]
fn interpreter_answers_the_same_requests_in_order() {
    let addr = free_address();
    let server = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr])
        .arg(example())
        .args(["--", &addr, "3", "4"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    assert_eq!(get(&addr, "/"), "ok");
    assert_eq!(get(&addr, "/work?ms=5"), "worked 5 ms");
    assert_eq!(get(&addr, "/missing"), "not found");
    assert_eq!(
        finish(server).lines().collect::<Vec<_>>(),
        [
            format!("listening on {addr} with 4 workers").as_str(),
            "done"
        ]
    );
}

#[test]
fn worker_count_is_checked() {
    for (source, code) in [
        (
            r#"main()->Unit{let r=serve("x",1,"many",|r|Response(200,Map(),""));}"#,
            "E102",
        ),
        (
            r#"main()->Unit{let r=serve("x",1,2,3,|r|Response(200,Map(),""));}"#,
            "E105",
        ),
        (
            r#"f()->I{let r=serve("x",1,2,|r|Response(200,Map(),""));1} main()->I{let t=spawn f();1}"#,
            "E117",
        ),
    ] {
        assert_eq!(check(source).unwrap_err().code, code, "{source}");
    }
}

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

use tokit_compiler::{check, native};

const CLIENT: &str = r#"main()->[String]{let c=match tcp_connect(args()[0]){Ok(c)=>c,Err(e)=>{print("denied");exit(1)}};let copy=c;let sent=tcp_send(copy,utf8_encode("ping\n"));var got=utf8_encode("");while len(got)<5{match tcp_recv(c,3){Ok(part)=>{if len(part)==0{break;}for b in part{got.push(b);}},Err(e)=>{break;}}}tcp_close(c);let after=tcp_send(c,utf8_encode("x"));[match utf8_decode_bytes(got){Some(t)=>t,None=>"?"},match after{Ok(u)=>"open",Err(e)=>"closed"}]}"#;
const EXPECTED: &str = r#"["PING\n","closed"]"#;

/// Accept one connection and answer each byte uppercased.
fn echo_server() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0u8; 64];
        let mut total = 0;
        while total < 5 {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            total += read;
            stream
                .write_all(&buffer[..read].to_ascii_uppercase())
                .unwrap();
        }
    });
    (addr, handle)
}

#[test]
fn tcp_round_trip_in_both_backends() {
    let directory = std::env::temp_dir().join(format!("tokit-tcp-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let program = directory.join("client.tok");
    std::fs::write(&program, CLIENT).unwrap();
    let (addr, server) = echo_server();
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr])
        .arg(&program)
        .args(["--", &addr])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = directory.join(format!("client{}", std::env::consts::EXE_SUFFIX));
        native::build(&check(CLIENT).unwrap(), CLIENT, &binary).unwrap();
        let (addr, server) = echo_server();
        let output = Command::new(&binary)
            .args(["--allow-net", "*", "--", &addr])
            .output()
            .unwrap();
        server.join().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("run")
        .arg(&program)
        .args(["--", "127.0.0.1:9"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "denied");
    std::fs::remove_dir_all(directory).unwrap();
    for source in [
        "f(c:Conn)->I{tcp_close(c);1} main()->I{1} g(c:Conn)->Task<I>{spawn f(c)}",
        "f()->Result<Conn,IoError>{tcp_connect(\"a:1\")} main()->I{let t=spawn f();1}",
    ] {
        assert_eq!(check(source).unwrap_err().code, "E117", "{source}");
    }
    assert_eq!(
        check("main()->bool{let c:Option<Conn>=None;true}").map(|_| ()),
        Ok(())
    );
    assert_eq!(
        check("struct Conn{x:I} main()->I{1}").unwrap_err().code,
        "E106"
    );
}

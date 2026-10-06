//! The registry's redis package against an in-process RESP2 server.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tok(directory: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .current_dir(directory)
        .env("TOK_HOME", home)
        .output()
        .unwrap()
}

/// Serve one client: PING, AUTH, SET, GET, INCR, DEL, and LRANGE-like arrays.
fn fake_redis() -> (String, std::thread::JoinHandle<Vec<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut writer = stream.try_clone().unwrap();
        let mut reader = BufReader::new(stream);
        let mut data: HashMap<String, String> = HashMap::new();
        let mut seen = Vec::new();
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).unwrap() == 0 {
                break;
            }
            let count: usize = header.trim_end().trim_start_matches('*').parse().unwrap();
            let mut args = Vec::new();
            for _ in 0..count {
                let mut size = String::new();
                reader.read_line(&mut size).unwrap();
                let mut value = String::new();
                reader.read_line(&mut value).unwrap();
                args.push(value.trim_end_matches("\r\n").to_owned());
            }
            let reply = match args[0].as_str() {
                "PING" => "+PONG\r\n".to_owned(),
                "AUTH" if args[1] == "secret" => "+OK\r\n".to_owned(),
                "AUTH" => "-WRONGPASS invalid password\r\n".to_owned(),
                "SET" => {
                    data.insert(args[1].clone(), args[2].clone());
                    "+OK\r\n".to_owned()
                }
                "GET" => match data.get(&args[1]) {
                    Some(value) => format!("${}\r\n{value}\r\n", value.len()),
                    None => "$-1\r\n".to_owned(),
                },
                "INCR" => {
                    let next = data.get(&args[1]).map_or(0, |v| v.parse::<i64>().unwrap()) + 1;
                    data.insert(args[1].clone(), next.to_string());
                    format!(":{next}\r\n")
                }
                "DEL" => format!(":{}\r\n", i32::from(data.remove(&args[1]).is_some())),
                "PAIR" => "*2\r\n$1\r\na\r\n*1\r\n:7\r\n".to_owned(),
                _ => "-ERR unknown command\r\n".to_owned(),
            };
            // Split replies across writes to exercise incremental parsing.
            let (first, rest) = reply.as_bytes().split_at(reply.len() / 2);
            writer.write_all(first).unwrap();
            writer.flush().unwrap();
            writer.write_all(rest).unwrap();
            seen.push(args);
        }
        seen
    });
    (addr, handle)
}

const MAIN: &str = r#"import redis="pkg:redis";
show(r:redis::Reply)->String{match r{redis::Reply::Many(items)=>"["+join(map(items,|i|show(i)),",")+"]",redis::Reply::Bulk(s)=>s,redis::Reply::Int(n)=>String(n),redis::Reply::Text(s)=>s,redis::Reply::Error(e)=>"error "+e,redis::Reply::Nil=>"nil"}}
run(addr:String)->Result<[String],String>{let c=redis::connect(addr)?;let pong=redis::ping(c)?;let denied=match redis::auth(c,"nope"){Ok(u)=>"accepted",Err(e)=>e};redis::auth(c,"secret")?;redis::store(c,"name","tökit")?;let name=redis::fetch(c,"name")?;let missing=redis::fetch(c,"other")?;let n=redis::incr(c,"hits")?;let m=redis::incr(c,"hits")?;let removed=redis::delete(c,"name")?;let pair=show(redis::cmd(c,["PAIR"])?);let unknown=show(redis::cmd(c,["NOPE"])?);redis::close(c);Ok([pong,denied,match name{Some(v)=>v,None=>"none"},match missing{Some(v)=>v,None=>"none"},String(n+m),String(removed),pair,unknown])}
main()->String{match run(args()[0]){Ok(parts)=>join(parts,"|"),Err(e)=>"failed: "+e}}
"#;
const EXPECTED: &str =
    "\"PONG|WRONGPASS invalid password|tökit|none|3|1|[a,[7]]|error ERR unknown command\"";

fn project(label: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "tokit-redis-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    assert!(tok(&base, &home, &["new", "app"]).status.success());
    let project = base.join("app");
    let added = tok(&project, &home, &["add", "redis"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    std::fs::write(project.join("main.tok"), MAIN).unwrap();
    (project, home)
}

#[test]
fn redis_package_talks_resp_in_both_backends() {
    let (project, home) = project("run");
    let (addr, server) = fake_redis();
    let output = tok(
        &project,
        &home,
        &["run", "--allow-net", &addr, "main.tok", "--", &addr],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        EXPECTED,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let commands = server.join().unwrap();
    assert_eq!(commands[3], ["SET", "name", "tökit"]);

    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = format!("app{}", std::env::consts::EXE_SUFFIX);
        let built = tok(&project, &home, &["build", "main.tok", "-o", &binary]);
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let (addr, server) = fake_redis();
        let output = Command::new(project.join(&binary))
            .args(["--allow-net", &addr, "--", &addr])
            .output()
            .unwrap();
        server.join().unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
    }
    std::fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

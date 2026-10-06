//! The registry's postgres package against an in-process server that speaks
//! the PostgreSQL v3 protocol, including real SCRAM-SHA-256 verification.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tokit_compiler::crypto::{
    __tok_base64_decode as b64_decode, __tok_base64_encode as b64, __tok_hex as hex,
    __tok_hmac_sha256 as hmac, __tok_md5 as md5, __tok_pbkdf2_sha256 as pbkdf2,
    __tok_sha256 as sha256,
};

const PASSWORD: &str = "s3cr3t";

fn read_exact(stream: &mut TcpStream, count: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; count];
    stream.read_exact(&mut buffer).unwrap();
    buffer
}

/// One typed message: (kind, body).
fn read_message(stream: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut kind = [0u8; 1];
    stream.read_exact(&mut kind).ok()?;
    let size = u32::from_be_bytes(read_exact(stream, 4).try_into().unwrap()) as usize;
    Some((kind[0], read_exact(stream, size - 4)))
}

fn message(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
    out.extend_from_slice(body);
    out
}

fn auth(code: u32, extra: &[u8]) -> Vec<u8> {
    let mut body = code.to_be_bytes().to_vec();
    body.extend_from_slice(extra);
    message(b'R', &body)
}

fn error(code: &str, text: &str) -> Vec<u8> {
    let body = format!("SERROR\0C{code}\0M{text}\0\0");
    message(b'E', body.as_bytes())
}

fn row_description(columns: &[&str]) -> Vec<u8> {
    let mut body = (columns.len() as u16).to_be_bytes().to_vec();
    for column in columns {
        body.extend_from_slice(column.as_bytes());
        body.push(0);
        body.extend_from_slice(&[0; 18]);
    }
    message(b'T', &body)
}

fn data_row(values: &[Option<&[u8]>]) -> Vec<u8> {
    let mut body = (values.len() as u16).to_be_bytes().to_vec();
    for value in values {
        match value {
            Some(bytes) => {
                body.extend_from_slice(&(bytes.len() as i32).to_be_bytes());
                body.extend_from_slice(bytes);
            }
            None => body.extend_from_slice(&(-1i32).to_be_bytes()),
        }
    }
    message(b'D', &body)
}

fn ready() -> Vec<u8> {
    message(b'Z', b"I")
}

fn cstring(body: &[u8], at: usize) -> (String, usize) {
    let end = at + body[at..].iter().position(|byte| *byte == 0).unwrap();
    (String::from_utf8(body[at..end].to_vec()).unwrap(), end + 1)
}

#[derive(Clone, Copy)]
enum Auth {
    Scram,
    Md5,
}

/// Returns false when authentication failed and the session ended.
fn authenticate(stream: &mut TcpStream, mode: Auth, user: &str) -> bool {
    match mode {
        Auth::Scram => {
            stream.write_all(&auth(10, b"SCRAM-SHA-256\0\0")).unwrap();
            let (_, body) = read_message(stream).unwrap();
            let (mechanism, at) = cstring(&body, 0);
            assert_eq!(mechanism, "SCRAM-SHA-256");
            let first = String::from_utf8(body[at + 4..].to_vec()).unwrap();
            let bare = first.strip_prefix("n,,").unwrap();
            let client_nonce = bare.strip_prefix("n=,r=").unwrap();
            let salt = b"tokit-salt";
            let server_first = format!("r={client_nonce}SRV,s={},i=4096", b64(salt));
            stream
                .write_all(&auth(11, server_first.as_bytes()))
                .unwrap();
            let (_, body) = read_message(stream).unwrap();
            let final_message = String::from_utf8(body).unwrap();
            let (without_proof, proof) = final_message.rsplit_once(",p=").unwrap();
            let auth_message = format!("{bare},{server_first},{without_proof}");
            let salted = pbkdf2(PASSWORD.as_bytes(), salt, 4096);
            let stored = sha256(&hmac(&salted, b"Client Key"));
            let signature = hmac(&stored, auth_message.as_bytes());
            let client_key: Vec<u8> = b64_decode(proof)
                .unwrap()
                .iter()
                .zip(&signature)
                .map(|(a, b)| a ^ b)
                .collect();
            if sha256(&client_key) != stored {
                stream
                    .write_all(&error("28P01", "password authentication failed"))
                    .unwrap();
                return false;
            }
            let server_signature = hmac(&hmac(&salted, b"Server Key"), auth_message.as_bytes());
            let verifier = format!("v={}", b64(&server_signature));
            stream.write_all(&auth(12, verifier.as_bytes())).unwrap();
        }
        Auth::Md5 => {
            stream.write_all(&auth(5, b"salt")).unwrap();
            let (_, body) = read_message(stream).unwrap();
            let (answer, _) = cstring(&body, 0);
            let inner = hex(&md5(format!("{PASSWORD}{user}").as_bytes()));
            let mut outer = inner.into_bytes();
            outer.extend_from_slice(b"salt");
            if answer != format!("md5{}", hex(&md5(&outer))) {
                stream
                    .write_all(&error("28P01", "password authentication failed"))
                    .unwrap();
                return false;
            }
        }
    }
    let mut greeting = auth(0, b"");
    greeting.extend(message(b'S', b"server_version\x0016.4\0"));
    greeting.extend(message(b'K', &[0, 0, 0, 1, 0, 0, 0, 2]));
    greeting.extend(ready());
    stream.write_all(&greeting).unwrap();
    true
}

fn serve(mut stream: TcpStream, mode: Auth) {
    let size = u32::from_be_bytes(read_exact(&mut stream, 4).try_into().unwrap()) as usize;
    let startup = read_exact(&mut stream, size - 4);
    assert_eq!(&startup[..4], &196608u32.to_be_bytes());
    let (key, at) = cstring(&startup, 4);
    assert_eq!(key, "user");
    let (user, _) = cstring(&startup, at);
    if !authenticate(&mut stream, mode, &user) {
        return;
    }
    let mut params: Vec<Vec<u8>> = Vec::new();
    while let Some((kind, body)) = read_message(&mut stream) {
        let reply = match kind {
            b'Q' => {
                let (sql, _) = cstring(&body, 0);
                match sql.as_str() {
                    "SELECT id, name FROM users" => [
                        row_description(&["id", "name"]),
                        data_row(&[Some(b"1"), Some("adä".as_bytes())]),
                        data_row(&[Some(b"2"), None]),
                        message(b'C', b"SELECT 2\0"),
                        ready(),
                    ]
                    .concat(),
                    "BAD" => [error("42601", "syntax error at or near \"BAD\""), ready()].concat(),
                    _ => [message(b'C', b"INSERT 0 1\0"), ready()].concat(),
                }
            }
            b'P' => {
                let (_, at) = cstring(&body, 0);
                let (sql, _) = cstring(&body, at);
                assert_eq!(sql, "SELECT $1, $2");
                continue;
            }
            b'B' => {
                let (_, at) = cstring(&body, 0);
                let (_, mut at) = cstring(&body, at);
                at += 2;
                let count = u16::from_be_bytes([body[at], body[at + 1]]) as usize;
                at += 2;
                params.clear();
                for _ in 0..count {
                    let size = i32::from_be_bytes(body[at..at + 4].try_into().unwrap()) as usize;
                    params.push(body[at + 4..at + 4 + size].to_vec());
                    at += 4 + size;
                }
                continue;
            }
            b'D' | b'E' => continue,
            b'S' => {
                let values: Vec<Option<&[u8]>> =
                    params.iter().map(|p| Some(p.as_slice())).collect();
                [
                    message(b'1', b""),
                    message(b'2', b""),
                    row_description(&["p1", "p2"]),
                    data_row(&values),
                    message(b'C', b"SELECT 1\0"),
                    ready(),
                ]
                .concat()
            }
            b'X' => break,
            other => panic!("unexpected message {}", other as char),
        };
        // Split each reply to exercise buffered reads.
        let (first, rest) = reply.split_at(reply.len() / 2);
        stream.write_all(first).unwrap();
        stream.flush().unwrap();
        stream.write_all(rest).unwrap();
    }
}

fn server(mode: Auth) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve(stream, mode);
    });
    (addr, handle)
}

const MAIN: &str = r#"import pg="pkg:postgres";
show(r:pg::Rows)->String{r.tag+":"+join(r.columns,",")+":"+join(map(r.rows,|row|join(map(row,|v|match v{Some(s)=>s,None=>"NULL"}),",")),";")}
run(addr:String,user:String,pw:String)->Result<[String],String>{let db=pg::connect(pg::Config(addr,user,pw,"app"))?;let rows=show(pg::query(db,"SELECT id, name FROM users")?);let bad=match pg::query(db,"BAD"){Ok(r)=>"ok",Err(e)=>e};let tag=pg::execute(db,"INSERT INTO t VALUES (1)")?;let echoed=show(pg::query_with(db,"SELECT $1, $2",["tök","x'y"])?);pg::close(db);Ok([rows,bad,tag,echoed,pg::quote("it's")])}
main()->String{let a=args();match run(a[0],a[1],a[2]){Ok(p)=>join(p,"|"),Err(e)=>"failed: "+e}}
"#;
const EXPECTED: &str = "\"SELECT 2:id,name:1,adä;2,NULL|syntax error at or near \\\"BAD\\\" (42601)|INSERT 0 1|SELECT 1:p1,p2:tök,x'y|'it''s'\"";

fn tok(directory: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .current_dir(directory)
        .env("TOK_HOME", home)
        .output()
        .unwrap()
}

fn project() -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "tokit-postgres-{}-{}",
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
    assert!(tok(&project, &home, &["add", "postgres"]).status.success());
    std::fs::write(project.join("main.tok"), MAIN).unwrap();
    (project, home)
}

fn interpreted(project: &Path, home: &Path, mode: Auth, password: &str) -> String {
    let (addr, handle) = server(mode);
    let output = tok(
        project,
        home,
        &[
            "run",
            "--allow-net",
            &addr,
            "main.tok",
            "--",
            &addr,
            "ada",
            password,
        ],
    );
    handle.join().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn postgres_package_authenticates_and_queries_in_both_backends() {
    let (project, home) = project();
    assert_eq!(
        interpreted(&project, &home, Auth::Scram, PASSWORD),
        EXPECTED
    );
    assert_eq!(interpreted(&project, &home, Auth::Md5, PASSWORD), EXPECTED);
    assert_eq!(
        interpreted(&project, &home, Auth::Scram, "wrong"),
        "\"failed: password authentication failed (28P01)\""
    );
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = format!("app{}", std::env::consts::EXE_SUFFIX);
        let built = tok(&project, &home, &["build", "main.tok", "-o", &binary]);
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        for mode in [Auth::Scram, Auth::Md5] {
            let (addr, handle) = server(mode);
            let output = Command::new(project.join(&binary))
                .args(["--allow-net", &addr, "--", &addr, "ada", PASSWORD])
                .output()
                .unwrap();
            handle.join().unwrap();
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), EXPECTED);
        }
    }
    std::fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

#[test]
fn scram_matches_the_rfc_7677_example() {
    let (project, home) = project();
    std::fs::write(
        project.join("main.tok"),
        "import pg=\"pkg:postgres\";\nmain()->[String]{match pg::scram_final(\"user\",\"pencil\",\"rOprNGfwEbeRWgbNEkqO\",\"r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096\"){Ok(s)=>[s.message,s.server_signature],Err(e)=>[e]}}\n",
    )
    .unwrap();
    let output = tok(&project, &home, &["run", "main.tok"]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        r#"["c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=","6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="]"#
    );
    std::fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

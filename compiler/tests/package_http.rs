//! The registry's http package serving a small API in both backends.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

fn tok(directory: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .current_dir(directory)
        .env("TOK_HOME", home)
        .output()
        .unwrap()
}

const MAIN: &str = r#"import http="pkg:http";
user(r:Request,p:Map<String,String>)->Response{http::json(200,"{\"id\":\""+get_or(p,"id","")+"\"}")}
search(r:Request,p:Map<String,String>)->Response{let q=http::query(r);http::text(200,"q="+get_or(q,"q","")+" n="+get_or(q,"n","?"))}
create(r:Request,p:Map<String,String>)->Response{http::text(201,"created "+r.body+" by "+match http::header(r,"X-Client"){Some(v)=>v,None=>"?"})}
main()->Unit{let routes=[http::Route("GET","/users/:id",|r,p|user(r,p)),http::Route("GET","/search",|r,p|search(r,p)),http::Route("POST","/users",|r,p|create(r,p))];match serve(args()[0],5,|r|http::dispatch(routes,r)){Ok(u)=>{},Err(e)=>{exit(1);}}}
"#;

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn exchange(addr: &str, request: &str) -> String {
    for _ in 0..200 {
        if let Ok(mut stream) = TcpStream::connect(addr) {
            stream.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            return response;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("server never started");
}

fn check_api(server: Child, addr: &str) {
    let user = exchange(addr, "GET /users/42 HTTP/1.1\r\n\r\n");
    assert!(user.starts_with("HTTP/1.1 200 OK"), "{user}");
    assert!(user.contains("content-type: application/json"), "{user}");
    assert!(user.ends_with("{\"id\":\"42\"}"), "{user}");
    let search = exchange(
        addr,
        "GET /search?q=hello%20w%C3%B6rld+x&n=3 HTTP/1.1\r\n\r\n",
    );
    assert!(search.ends_with("q=hello wörld x n=3"), "{search}");
    let created = exchange(
        addr,
        "POST /users HTTP/1.1\r\nX-Client: cli\r\nContent-Length: 3\r\n\r\nada",
    );
    assert!(created.starts_with("HTTP/1.1 201 Created"), "{created}");
    assert!(created.ends_with("created ada by cli"), "{created}");
    let wrong_method = exchange(addr, "DELETE /users/1 HTTP/1.1\r\n\r\n");
    assert!(wrong_method.starts_with("HTTP/1.1 405"), "{wrong_method}");
    let missing = exchange(addr, "GET /nope HTTP/1.1\r\n\r\n");
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    assert!(server.wait_with_output().unwrap().status.success());
}

fn project() -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "tokit-http-package-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    assert!(tok(&base, &home, &["new", "api"]).status.success());
    let project = base.join("api");
    assert!(tok(&project, &home, &["add", "http"]).status.success());
    std::fs::write(project.join("main.tok"), MAIN).unwrap();
    (project, home)
}

#[test]
fn http_package_routes_requests_in_both_backends() {
    let (project, home) = project();
    let addr = format!("127.0.0.1:{}", free_port());
    let server = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["run", "--allow-net", &addr, "main.tok", "--", &addr])
        .current_dir(&project)
        .env("TOK_HOME", &home)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    check_api(server, &addr);
    if Command::new("rustc").arg("--version").output().is_ok() {
        let binary = format!("api{}", std::env::consts::EXE_SUFFIX);
        let built = tok(&project, &home, &["build", "main.tok", "-o", &binary]);
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let addr = format!("127.0.0.1:{}", free_port());
        let server = Command::new(project.join(&binary))
            .args(["--allow-net", &addr, "--", &addr])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        check_api(server, &addr);
    }
    std::fs::remove_dir_all(project.parent().unwrap()).unwrap();
}

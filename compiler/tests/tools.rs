use std::io::Write;
use std::process::{Command, Stdio};

fn temporary_file(name: &str, contents: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "tokit-tools-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

fn tok(args: &[&str], input: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .replace("\r\n", "\n")
}

#[test]
fn doc_lists_public_declarations_with_comments() {
    let path = temporary_file(
        "geo.tok",
        "// Geometry helpers.\npub struct Point{x:I,y:I}\n// Squared distance from the origin.\npub norm(p:Point)->I{p.x*p.x+p.y*p.y}\nhidden()->I{1}\n",
    );
    let doc = tok(&["doc", path.to_str().unwrap()], "");
    assert!(doc.starts_with("# geo\n\nGeometry helpers.\n"), "{doc}");
    assert!(
        doc.contains(
            "## `norm`\n\n```tokit\nnorm(p:Point)->I\n```\n\nSquared distance from the origin."
        ),
        "{doc}"
    );
    assert!(doc.contains("- `x: i32`"), "{doc}");
    assert!(!doc.contains("hidden"), "{doc}");
    let private = tok(&["doc", "--private", path.to_str().unwrap()], "");
    assert!(private.contains("## `hidden`"), "{private}");
}

#[test]
fn lint_reports_unused_and_unchanged_bindings() {
    let path = temporary_file(
        "lint.tok",
        "helper()->I{1}\nmain()->I{var unused=1;var same=2;let _ok=3;var changed=0;changed=same;var xs:[I]=[];xs.push(1);changed+len(xs)}\n",
    );
    let output = tok(&["lint", path.to_str().unwrap()], "");
    let codes: Vec<&str> = output.lines().map(|line| &line[..4]).collect();
    assert_eq!(codes, ["W003", "W001", "W002"], "{output}");
    assert!(output.contains("unused is never read"));
    assert!(output.contains("same is never changed"));
    let json = tok(&["lint", "--json", path.to_str().unwrap()], "");
    assert!(
        json.starts_with("{\"ok\":true,\"warnings\":[{\"code\":\"W003\""),
        "{json}"
    );
}

#[test]
fn repl_keeps_declarations_and_bindings() {
    let output = tok(
        &["repl"],
        "sq(x:I)->I{x*x}\nlet a=7\nsq(a)+1\nmap([1,2],|x|sq(x))\nnope(1)\nstruct P{x:I}\nP(a).x\n:reset\na\n:quit\n",
    );
    let results: Vec<&str> = output
        .split("> ")
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Tokit REPL"))
        .collect();
    assert_eq!(
        results,
        [
            "50",
            "[1,4]",
            "E101 unknown function nope",
            "7",
            "E101 unknown name a"
        ]
    );
}

#[test]
fn bench_times_each_bench_function() {
    let path = temporary_file(
        "b.tok",
        "bench_add()->I{1+2}\nbench_text()->String{join([\"a\",\"b\"],\",\")}\nmain()->I{0}\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["bench", "--iterations", "20", path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert!(lines[0].starts_with("bench_add ") && lines[0].ends_with(" ns/iter (20 iterations)"));
    assert!(lines[1].starts_with("bench_text "));
}

#[test]
fn help_lists_every_command() {
    let help = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for command in [
        "run",
        "build",
        "test",
        "bench",
        "repl",
        "check",
        "lint",
        "explain",
        "expand",
        "stats",
        "ai-index",
        "tokens",
        "doc",
        "fmt",
        "compact",
        "new",
        "add",
        "rm",
        "search",
        "lock",
        "pkg-hash",
        "lsp",
        "--allow-net",
    ] {
        assert!(
            text.contains(&format!(" {command}")) || text.contains(&format!("[{command}")),
            "{command} missing from help"
        );
    }
    let unknown = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("frobnicate")
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).starts_with("usage: tok <command>"));
}

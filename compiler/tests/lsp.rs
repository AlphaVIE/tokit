mod common;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn frame(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).unwrap();
    let mut output = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    output.extend(body);
    output
}

fn decode(mut bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    while !bytes.is_empty() {
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap();
        let header = std::str::from_utf8(&bytes[..header_end]).unwrap();
        let length = header
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        bytes = &bytes[header_end + 4..];
        messages.push(serde_json::from_slice(&bytes[..length]).unwrap());
        bytes = &bytes[length..];
    }
    messages
}

fn file_uri(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| {
        std::fs::canonicalize(path.parent().unwrap())
            .unwrap()
            .join(path.file_name().unwrap())
    });
    let raw = canonical.to_str().unwrap().replace('\\', "/");
    #[cfg(windows)]
    let raw = format!("/{}", raw.strip_prefix("//?/").unwrap_or(&raw));
    let mut encoded = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("file://{encoded}")
}

#[test]
fn lsp_tracks_unsaved_text_and_utf16_diagnostics() {
    let uri = "file:///virtual/unwritten.tok";
    let valid = "struct Pair{x:i32} enum Color{Red} fn main()->i32{0}";
    let invalid = "fn main()->i32{let s=\"😀\";missing}";
    let messages = [
        json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"tokit","version":1,"text":valid}}}),
        json!({"jsonrpc":"2.0","id":8,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":invalid}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":1},"contentChanges":[{"text":valid}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}}),
        json!({"jsonrpc":"2.0","id":9,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    assert_eq!(output.len(), 6);
    assert_eq!(output[0]["id"], 7);
    assert_eq!(
        output[0]["result"]["capabilities"]["positionEncoding"],
        "utf-16"
    );
    assert_eq!(
        output[0]["result"]["capabilities"]["textDocumentSync"]["change"],
        1
    );
    assert_eq!(output[1]["params"]["diagnostics"], json!([]));
    assert_eq!(output[1]["params"]["version"], 1);
    assert_eq!(output[2]["result"][0]["name"], "Pair");
    assert_eq!(output[2]["result"][0]["kind"], 23);
    assert_eq!(output[2]["result"][1]["name"], "Color");
    assert_eq!(output[2]["result"][1]["kind"], 10);
    assert_eq!(output[2]["result"][2]["name"], "main");
    assert_eq!(output[2]["result"][2]["kind"], 12);
    assert_eq!(output[3]["params"]["version"], 2);
    let error = &output[3]["params"]["diagnostics"][0];
    assert_eq!(error["code"], "E101");
    assert_eq!(error["range"]["start"]["line"], 0);
    let prefix = invalid.split("missing").next().unwrap();
    assert_eq!(
        error["range"]["start"]["character"],
        prefix.encode_utf16().count()
    );
    assert_eq!(output[4]["params"]["diagnostics"], json!([]));
    assert!(output[4]["params"].get("version").is_none());
    assert_eq!(output[5]["id"], 9);
    assert_eq!(output[5]["result"], Value::Null);
}

#[test]
fn tok_lsp_uses_clean_stdio_framing() {
    let input = [
        json!({"jsonrpc":"2.0","id":"start","method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ]
    .iter()
    .flat_map(frame)
    .collect::<Vec<_>>();
    let mut process = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(&input).unwrap();
    let result = process.wait_with_output().unwrap();
    assert!(result.status.success());
    let output = decode(&result.stdout);
    assert_eq!(output.len(), 2);
    assert_eq!(output[0]["id"], "start");
    assert_eq!(output[1]["id"], "stop");
}

#[test]
fn imported_buffers_get_syntax_diagnostics_without_false_name_errors() {
    let uri = "file:///virtual/main.tok";
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"version":1,"text":"import math=\"math.tok\";fn main()->i32{math::triple(7)}"}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"text":"import math=\"math.tok\";fn main()->i32{math::triple(7)@}"}]}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    assert_eq!(output[1]["params"]["diagnostics"], json!([]));
    assert_eq!(output[2]["params"]["diagnostics"][0]["severity"], 1);
}

#[test]
fn lsp_rechecks_open_import_graph_and_clears_stale_diagnostics() {
    let directory = std::env::temp_dir().join(format!(
        "tokit lsp graph {} {}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir(&directory).unwrap();
    let main_path = directory.join("main.tok");
    let math_path = directory.join("math.tok");
    let main_text = "import math=\"math.tok\";fn main()->i32{math::triple(7)}";
    std::fs::write(&main_path, main_text).unwrap();
    std::fs::write(&math_path, "pub fn triple(n:i32)->i32{n*3}").unwrap();
    let main_uri = file_uri(&main_path);
    let math_uri = file_uri(&math_path);
    let invalid_math = "pub fn triple(n:i32)->i32{let note=\"😀\";missing}";
    assert!(main_uri.contains("%20"));
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":main_uri,"version":1,"text":main_text}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":math_uri,"version":1,"text":invalid_math}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":math_uri,"version":2},"contentChanges":[{"text":"pub fn triple(n:i32)->String{\"x\"}"}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":math_uri,"version":3},"contentChanges":[{"text":"pub fn triple(n:i32)->i32{n*4}"}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":math_uri,"version":2},"contentChanges":[{"text":"pub fn triple(n:i32)->i32{missing}"}]}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":math_uri}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    let publications = output
        .iter()
        .filter(|message| message["method"] == "textDocument/publishDiagnostics")
        .collect::<Vec<_>>();
    assert!(publications.iter().any(|message| {
        message["params"]["uri"] == math_uri
            && message["params"]["version"] == 1
            && message["params"]["diagnostics"][0]["code"] == "E101"
            && message["params"]["diagnostics"][0]["range"]["start"]["character"]
                == invalid_math
                    .split("missing")
                    .next()
                    .unwrap()
                    .encode_utf16()
                    .count()
    }));
    assert!(publications.iter().any(|message| {
        message["params"]["uri"] == main_uri
            && message["params"]["diagnostics"][0]["code"] == "E102"
    }));
    assert!(publications.iter().any(|message| {
        message["params"]["uri"] == main_uri && message["params"]["diagnostics"] == json!([])
    }));
    let math_publications = publications
        .iter()
        .filter(|message| message["params"]["uri"] == math_uri)
        .collect::<Vec<_>>();
    assert_eq!(
        math_publications.last().unwrap()["params"]["diagnostics"],
        json!([])
    );
    assert!(
        math_publications.last().unwrap()["params"]
            .get("version")
            .is_none()
    );
    assert_eq!(math_publications.len(), 4);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lsp_uses_unsaved_pinned_package_source() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let entry = examples.join("package_json/main.tok");
    let package = examples.join("json/json.tok");
    let entry_uri = file_uri(&entry);
    let package_uri = file_uri(&package);
    let entry_text = std::fs::read_to_string(&entry).unwrap();
    let package_text = std::fs::read_to_string(&package).unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":entry_uri,"version":1,"text":entry_text}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":package_uri,"version":1,"text":"pub fn broken()->i32{@}"}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":package_uri,"version":2},"contentChanges":[{"text":package_text}]}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    let package_publications = output
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == package_uri
        })
        .collect::<Vec<_>>();
    assert_eq!(package_publications.len(), 2);
    assert_eq!(package_publications[0]["params"]["version"], 1);
    assert_eq!(
        package_publications[0]["params"]["diagnostics"][0]["severity"],
        1
    );
    assert_eq!(package_publications[1]["params"]["version"], 2);
    assert_eq!(package_publications[1]["params"]["diagnostics"], json!([]));
}

#[test]
fn lsp_resolves_new_unsaved_import_file() {
    let directory = std::env::temp_dir().join(format!(
        "tokit-lsp-new-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir(&directory).unwrap();
    let main_path = directory.join("main.tok");
    let new_path = directory.join("new.tok");
    let main_text = "import next=\"new.tok\";fn main()->i32{next::value()}";
    std::fs::write(&main_path, main_text).unwrap();
    assert!(!new_path.exists());
    let main_uri = file_uri(&main_path);
    let new_uri = file_uri(&new_path);
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":main_uri,"version":1,"text":main_text}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":new_uri,"version":1,"text":"pub fn value()->i32{42}"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    let main_publications = output
        .iter()
        .filter(|message| {
            message["method"] == "textDocument/publishDiagnostics"
                && message["params"]["uri"] == main_uri
        })
        .collect::<Vec<_>>();
    assert_eq!(main_publications.len(), 2);
    assert_eq!(
        main_publications[0]["params"]["diagnostics"][0]["code"],
        "E118"
    );
    assert_eq!(main_publications[1]["params"]["diagnostics"], json!([]));
    assert!(!new_path.exists());
    std::fs::remove_dir_all(directory).unwrap();
}

fn session(text: &str, requests: &[Value]) -> Vec<Value> {
    let uri = "file:///virtual/navigation.tok";
    let mut messages = vec![
        json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"tokit","version":1,"text":text}}}),
    ];
    messages.extend(requests.iter().cloned());
    messages.push(json!({"jsonrpc":"2.0","id":"stop","method":"shutdown"}));
    messages.push(json!({"jsonrpc":"2.0","method":"exit"}));
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut process = Command::new(env!("CARGO_BIN_EXE_tok"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    process.stdin.take().unwrap().write_all(&input).unwrap();
    let result = process.wait_with_output().unwrap();
    assert!(result.status.success());
    decode(&result.stdout)
        .into_iter()
        .filter(|message| message.get("id").is_some_and(|id| id.is_number()))
        .collect()
}

fn at(id: i64, method: &str, line: u64, character: u64) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":{"textDocument":{"uri":"file:///virtual/navigation.tok"},"position":{"line":line,"character":character}}})
}

#[test]
fn lsp_finds_definitions_and_hovers() {
    let text = "struct Point{x:I,y:I}\nnorm(p:Point)->I{p.x*p.x}\nmain()->I{let first=Point(3,4);let d=norm(first);map([1],|v|v+d)[0]}\n// é 😀 offsets";
    let responses = session(
        text,
        &[
            // `norm` call on line 2 → its declaration on line 1.
            at(1, "textDocument/definition", 2, 38),
            // `first` argument → the `let first` binding.
            at(2, "textDocument/definition", 2, 43),
            // `d` inside the lambda → the `let d` binding.
            at(3, "textDocument/definition", 2, 62),
            // `v` inside the lambda → the lambda parameter.
            at(4, "textDocument/definition", 2, 60),
            // Hover on the call shows the signature; on a local, its type.
            at(5, "textDocument/hover", 2, 38),
            at(6, "textDocument/hover", 2, 43),
            at(7, "textDocument/hover", 1, 7),
            // Nothing to show on punctuation.
            at(8, "textDocument/definition", 2, 6),
        ],
    );
    let by_id = |id: i64| {
        responses
            .iter()
            .find(|message| message["id"] == id)
            .unwrap()["result"]
            .clone()
    };
    assert_eq!(by_id(1)["range"]["start"], json!({"line":1,"character":0}));
    assert_eq!(by_id(2)["range"]["start"], json!({"line":2,"character":14}));
    assert_eq!(by_id(3)["range"]["start"], json!({"line":2,"character":35}));
    assert_eq!(by_id(4)["range"]["start"], json!({"line":2,"character":58}));
    assert_eq!(
        by_id(5)["contents"]["value"],
        "```tokit\nnorm(p:Point)->I\n```"
    );
    assert_eq!(by_id(6)["contents"]["value"], "```tokit\nfirst: Point\n```");
    // A type name has no expression type, so it shows its declaration.
    assert_eq!(
        by_id(7)["contents"]["value"],
        "```tokit\nstruct Point{x:I,y:I}\n```"
    );
    assert_eq!(by_id(8), Value::Null);
}

fn request(id: i64, method: &str, line: u64, character: u64, extra: Value) -> Value {
    let mut message = at(id, method, line, character);
    for (key, value) in extra.as_object().unwrap() {
        message["params"][key] = value.clone();
    }
    message
}

#[test]
fn lsp_finds_references_and_renames_bindings() {
    let text = "struct Point{x:I,y:I}\nnorm(p:Point)->I{p.x*p.x}\nmain()->I{let x=Point(3,4);let n=norm(x);let f=|a,b|a+b+n;f(x.x,n)}";
    let responses = session(
        text,
        &[
            // `x` local on line 2: binding, argument, and field receiver; not the field `.x`.
            request(
                1,
                "textDocument/references",
                2,
                14,
                json!({"context":{"includeDeclaration":true}}),
            ),
            request(
                2,
                "textDocument/references",
                2,
                14,
                json!({"context":{"includeDeclaration":false}}),
            ),
            // Lambda parameter `b` and its one use.
            request(
                3,
                "textDocument/references",
                2,
                50,
                json!({"context":{"includeDeclaration":true}}),
            ),
            request(4, "textDocument/rename", 1, 1, json!({"newName":"length"})),
            request(5, "textDocument/rename", 1, 1, json!({"newName":"let"})),
            request(6, "textDocument/rename", 1, 1, json!({"newName":"len"})),
            request(7, "textDocument/rename", 0, 13, json!({"newName":"z"})),
        ],
    );
    let by_id = |id: i64| {
        responses
            .iter()
            .find(|message| message["id"] == id)
            .unwrap()
            .clone()
    };
    let columns = |result: &Value| {
        result
            .as_array()
            .unwrap()
            .iter()
            .map(|location| {
                (
                    location["range"]["start"]["line"].as_u64().unwrap(),
                    location["range"]["start"]["character"].as_u64().unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(columns(&by_id(1)["result"]), [(2, 14), (2, 38), (2, 60)]);
    assert_eq!(columns(&by_id(2)["result"]), [(2, 38), (2, 60)]);
    assert_eq!(columns(&by_id(3)["result"]), [(2, 50), (2, 54)]);
    let edits = by_id(4)["result"]["changes"]["file:///virtual/navigation.tok"].clone();
    assert_eq!(columns(&edits), [(1, 0), (2, 33)]);
    assert!(
        edits
            .as_array()
            .unwrap()
            .iter()
            .all(|edit| edit["newText"] == "length")
    );
    assert_eq!(by_id(5)["error"]["code"], -32602);
    assert_eq!(by_id(6)["error"]["code"], -32602);
    // Field declarations are not bindings that can be renamed.
    assert_eq!(by_id(7)["error"]["code"], -32602);
}

#[test]
fn lsp_completes_locals_declarations_builtins_and_fields() {
    // The document does not parse: completion must still work while typing.
    let text = "struct Point{x:I,y:I}\nnorm(p:Point)->I{p.x}\nmain()->I{let total=1;let q=Point(1,2);q.\n  tot";
    let responses = session(
        text,
        &[
            at(1, "textDocument/completion", 2, 41),
            at(2, "textDocument/completion", 3, 5),
        ],
    );
    let labels = |id: i64| {
        responses
            .iter()
            .find(|message| message["id"] == id)
            .unwrap()["result"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["label"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(labels(1), ["x", "y"]);
    let general = labels(2);
    assert_eq!(&general[..2], ["q", "total"]);
    for expected in ["Point", "norm", "main", "len", "map", "while"] {
        assert!(general.iter().any(|label| label == expected), "{expected}");
    }
    assert!(
        !general.iter().any(|label| label == "p"),
        "other function's parameter"
    );
}

#[test]
fn lsp_resolves_nested_and_bare_pattern_bindings() {
    let text = "enum S{C(I)}\nf(x:Option<S>)->I{match x{Some(S::C(r))=>r,other=>match other{None=>0,_=>1}}}";
    let responses = session(
        text,
        &[
            at(1, "textDocument/definition", 1, 41),
            at(2, "textDocument/definition", 1, 56),
        ],
    );
    let start = |id: i64| {
        responses
            .iter()
            .find(|message| message["id"] == id)
            .unwrap()["result"]["range"]["start"]
            .clone()
    };
    assert_eq!(start(1), json!({"line":1,"character":36}));
    assert_eq!(start(2), json!({"line":1,"character":43}));
}

#[test]
fn lsp_navigates_into_imported_modules() {
    let directory = std::env::temp_dir().join(format!(
        "tokit-lsp-cross-{}-{}",
        std::process::id(),
        common::nonce()
    ));
    std::fs::create_dir(&directory).unwrap();
    let main_path = directory.join("main.tok");
    let shapes_path = directory.join("shapes.tok");
    // The local `area` must not capture the imported one.
    let main_text = "import s=\"shapes.tok\";\narea(x:I)->I{x}\nmain()->I{let c=s::Shape::Circle(2);s::area(c)+area(1)}";
    std::fs::write(&main_path, main_text).unwrap();
    // Saved text differs from the open buffer: navigation must use the buffer.
    std::fs::write(&shapes_path, "pub enum Shape{Circle(I)}").unwrap();
    let shapes_text = "pub enum Shape{Circle(I)}\n// area of a shape\npub area(s:Shape)->I{match s{Shape::Circle(r)=>3*r*r}}";
    let main_uri = file_uri(&main_path);
    let shapes_uri = file_uri(&shapes_path);
    let at = |id: i64, method: &str, character: u64| json!({"jsonrpc":"2.0","id":id,"method":method,"params":{"textDocument":{"uri":main_uri},"position":{"line":2,"character":character}}});
    let messages = [
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":main_uri,"version":1,"text":main_text}}}),
        json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":shapes_uri,"version":1,"text":shapes_text}}}),
        at(1, "textDocument/definition", 39),
        at(2, "textDocument/hover", 39),
        at(3, "textDocument/definition", 26),
        at(4, "textDocument/definition", 47),
        json!({"jsonrpc":"2.0","id":9,"method":"shutdown"}),
        json!({"jsonrpc":"2.0","method":"exit"}),
    ];
    let input = messages.iter().flat_map(frame).collect::<Vec<_>>();
    let mut output = Vec::new();
    assert!(tokit_compiler::lsp::serve(&mut input.as_slice(), &mut output).unwrap());
    let output = decode(&output);
    let result =
        |id: i64| output.iter().find(|message| message["id"] == id).unwrap()["result"].clone();
    // `s::area` → `area` in the open shapes buffer, line 2.
    assert_eq!(result(1)["uri"], shapes_uri);
    assert_eq!(result(1)["range"]["start"], json!({"line":2,"character":4}));
    assert_eq!(
        result(2)["contents"]["value"],
        "```tokit\narea(s:Shape)->I\n```"
    );
    // `s::Shape::Circle` → the enum declaration.
    assert_eq!(result(3)["uri"], shapes_uri);
    assert_eq!(result(3)["range"]["start"], json!({"line":0,"character":9}));
    // The unqualified local `area` stays in main.tok.
    assert_eq!(result(4)["uri"], main_uri);
    assert_eq!(result(4)["range"]["start"], json!({"line":1,"character":0}));
    std::fs::remove_dir_all(directory).unwrap();
}

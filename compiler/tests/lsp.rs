use std::io::Write;
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

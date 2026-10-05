//! Small stdio language server for live diagnostics and document symbols.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

struct Document {
    text: String,
    version: i64,
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?;
    let encoded = encoded.strip_prefix("localhost").unwrap_or(encoded);
    let encoded = encoded.split(['?', '#']).next()?;
    if !encoded.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut at = 0;
    while at < encoded.len() {
        if encoded.as_bytes()[at] == b'%' {
            let hex = std::str::from_utf8(encoded.as_bytes().get(at + 1..at + 3)?).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            bytes.push(encoded.as_bytes()[at]);
            at += 1;
        }
    }
    let decoded = String::from_utf8(bytes).ok()?;
    #[cfg(windows)]
    let decoded = {
        let bytes = decoded.as_bytes();
        if bytes.len() >= 3
            && bytes[0] == b'/'
            && bytes[1].is_ascii_alphabetic()
            && bytes[2] == b':'
        {
            &decoded[1..]
        } else {
            decoded.as_str()
        }
    };
    let path = PathBuf::from(decoded);
    if path.extension().is_none_or(|extension| extension != "tok") {
        return None;
    }
    fs::canonicalize(&path).ok().or_else(|| {
        let parent = fs::canonicalize(path.parent()?).ok()?;
        Some(parent.join(path.file_name()?))
    })
}

fn path_uri(path: &Path) -> Option<String> {
    let raw = path.to_str()?.replace('\\', "/");
    #[cfg(windows)]
    let raw = raw.strip_prefix("//?/").unwrap_or(&raw);
    #[cfg(windows)]
    let raw = format!("/{raw}");
    let mut encoded = String::new();
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b':' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(format!("file://{encoded}"))
}

fn read_message<R: BufRead>(input: &mut R) -> io::Result<Option<Result<Json, serde_json::Error>>> {
    let mut length = None;
    let mut saw_header = false;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return if saw_header {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated LSP header",
                ))
            } else {
                Ok(None)
            };
        }
        saw_header = true;
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing LSP Content-Length"))?;
    if length > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message exceeds 16 MiB",
        ));
    }
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)))
}

fn send<W: Write>(output: &mut W, message: &Json) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

fn position(text: &str, offset: usize) -> Json {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let character = before
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .encode_utf16()
        .count();
    json!({"line": line, "character": character})
}

fn range(text: &str, span: Span) -> Json {
    json!({
        "start": position(text, span.start),
        "end": position(text, span.end),
    })
}

fn diagnostic(text: &str, error: &Diagnostic) -> Json {
    json!({
        "range": range(text, error.span),
        "severity": 1,
        "code": error.code,
        "source": "tokit",
        "message": error.message,
    })
}

fn standalone_error(text: &str) -> Option<Diagnostic> {
    match crate::parse(text) {
        Err(error) => Some(error),
        Ok(program) if program.imports.is_empty() => crate::checker::check(&program).err(),
        Ok(_) => None,
    }
}

fn collect_diagnostics(documents: &HashMap<String, Document>) -> HashMap<String, Vec<Json>> {
    let mut overrides = HashMap::new();
    let mut uri_for_path = HashMap::new();
    for (uri, document) in documents {
        if let Some(path) = file_uri_path(uri) {
            overrides.insert(path.clone(), document.text.clone());
            uri_for_path.insert(path, uri.clone());
        }
    }
    let mut publications = documents
        .keys()
        .map(|uri| (uri.clone(), Vec::new()))
        .collect::<HashMap<_, _>>();
    let mut entries = documents.iter().collect::<Vec<_>>();
    entries.sort_by_key(|(uri, _)| *uri);
    for (uri, document) in entries {
        let finding = if let Some(path) = file_uri_path(uri) {
            crate::modules::load_with_overrides(&path, &overrides)
                .err()
                .and_then(|error| {
                    let source = error.sources.get(error.diagnostic.span.source_id)?;
                    let target_uri = uri_for_path
                        .get(&source.path)
                        .cloned()
                        .or_else(|| path_uri(&source.path))
                        .unwrap_or_else(|| uri.clone());
                    Some((target_uri, diagnostic(&source.text, &error.diagnostic)))
                })
        } else {
            standalone_error(&document.text)
                .map(|error| (uri.clone(), diagnostic(&document.text, &error)))
        };
        if let Some((target_uri, finding)) = finding {
            let list = publications.entry(target_uri).or_default();
            if !list.contains(&finding) {
                list.push(finding);
            }
        }
    }
    publications
}

fn publish<W: Write>(
    output: &mut W,
    uri: &str,
    version: Option<i64>,
    diagnostics: Vec<Json>,
) -> io::Result<()> {
    let mut params = json!({"uri": uri, "diagnostics": diagnostics});
    if let Some(version) = version {
        params["version"] = json!(version);
    }
    send(
        output,
        &json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": params}),
    )
}

fn publish_all<W: Write>(
    output: &mut W,
    documents: &HashMap<String, Document>,
    previously_published: &mut HashSet<String>,
) -> io::Result<()> {
    let mut current = collect_diagnostics(documents);
    let current_uris = current.keys().cloned().collect::<HashSet<_>>();
    let mut all_uris = current_uris
        .union(previously_published)
        .cloned()
        .collect::<Vec<_>>();
    all_uris.sort();
    for uri in all_uris {
        let version = documents.get(&uri).map(|document| document.version);
        publish(
            output,
            &uri,
            version,
            current.remove(&uri).unwrap_or_default(),
        )?;
    }
    *previously_published = current_uris;
    Ok(())
}

fn symbols(text: &str) -> Json {
    let Ok(program) = crate::parse(text) else {
        return json!([]);
    };
    let mut result = Vec::new();
    for function in &program.functions {
        let range = range(text, function.span);
        result.push((
            function.span.start,
            json!({
                "name": function.name,
                "kind": 12,
                "range": range,
                "selectionRange": range,
            }),
        ));
    }
    for record in &program.records {
        let range = range(text, record.span);
        result.push((
            record.span.start,
            json!({
                "name": record.name,
                "kind": 23,
                "range": range,
                "selectionRange": range,
            }),
        ));
    }
    for enumeration in &program.enums {
        let range = range(text, enumeration.span);
        result.push((
            enumeration.span.start,
            json!({
                "name": enumeration.name,
                "kind": 10,
                "range": range,
                "selectionRange": range,
            }),
        ));
    }
    result.sort_by_key(|(start, _)| *start);
    json!(
        result
            .into_iter()
            .map(|(_, symbol)| symbol)
            .collect::<Vec<_>>()
    )
}

fn response(id: &Json, result: Json) -> Json {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: &Json, code: i32, message: &str) -> Json {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Serve LSP messages until `exit` or EOF. Returns true only after a clean shutdown.
pub fn serve<R: BufRead, W: Write>(input: &mut R, output: &mut W) -> io::Result<bool> {
    let mut documents = HashMap::<String, Document>::new();
    let mut previously_published = HashSet::new();
    let mut initialized = false;
    let mut shutdown = false;
    while let Some(message) = read_message(input)? {
        let message = match message {
            Ok(message) => message,
            Err(_) => {
                send(output, &error_response(&Json::Null, -32700, "parse error"))?;
                continue;
            }
        };
        let id = message.get("id");
        let method = message.get("method").and_then(Json::as_str);
        let params = message.get("params").unwrap_or(&Json::Null);
        match method {
            Some("initialize") if !initialized && !shutdown => {
                initialized = true;
                if let Some(id) = id {
                    send(
                        output,
                        &response(
                            id,
                            json!({
                                "capabilities": {
                                    "positionEncoding": "utf-16",
                                    "textDocumentSync": {"openClose": true, "change": 1},
                                    "documentSymbolProvider": true,
                                },
                                "serverInfo": {"name": "tokit", "version": env!("CARGO_PKG_VERSION")},
                            }),
                        ),
                    )?;
                }
            }
            Some("shutdown") if initialized && !shutdown => {
                shutdown = true;
                if let Some(id) = id {
                    send(output, &response(id, Json::Null))?;
                }
            }
            Some("exit") => return Ok(shutdown),
            Some("textDocument/didOpen") if initialized && !shutdown && id.is_none() => {
                let text_document = &params["textDocument"];
                if let (Some(uri), Some(text), Some(version)) = (
                    text_document["uri"].as_str(),
                    text_document["text"].as_str(),
                    text_document["version"].as_i64(),
                ) {
                    documents.insert(
                        uri.to_owned(),
                        Document {
                            text: text.to_owned(),
                            version,
                        },
                    );
                    publish_all(output, &documents, &mut previously_published)?;
                }
            }
            Some("textDocument/didChange") if initialized && !shutdown && id.is_none() => {
                let text_document = &params["textDocument"];
                if let (Some(uri), Some(version), Some(changes)) = (
                    text_document["uri"].as_str(),
                    text_document["version"].as_i64(),
                    params["contentChanges"].as_array(),
                ) && let Some(document) = documents.get_mut(uri)
                    && version > document.version
                    && let Some(change) = changes.last()
                    && change.get("range").is_none()
                    && let Some(text) = change["text"].as_str()
                {
                    document.text = text.to_owned();
                    document.version = version;
                    publish_all(output, &documents, &mut previously_published)?;
                }
            }
            Some("textDocument/didClose") if initialized && !shutdown && id.is_none() => {
                if let Some(uri) = params["textDocument"]["uri"].as_str()
                    && documents.remove(uri).is_some()
                {
                    publish_all(output, &documents, &mut previously_published)?;
                }
            }
            Some("textDocument/documentSymbol") if initialized && !shutdown && id.is_some() => {
                let uri = params["textDocument"]["uri"].as_str();
                let result = uri
                    .and_then(|uri| documents.get(uri))
                    .map(|document| symbols(&document.text))
                    .unwrap_or_else(|| json!([]));
                send(output, &response(id.expect("checked id"), result))?;
            }
            Some(_) if id.is_some() => {
                send(
                    output,
                    &error_response(id.expect("checked id"), -32601, "method not found"),
                )?;
            }
            _ => {}
        }
    }
    Ok(shutdown)
}

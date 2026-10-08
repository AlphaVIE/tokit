//! Small stdio language server for diagnostics, symbols, definitions, hovers,
//! references, renames, and completion.

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

/// The byte offset of an LSP position (line and UTF-16 column), clamped to the text.
fn offset_at(text: &str, line: u64, character: u64) -> usize {
    let mut start = 0;
    for _ in 0..line {
        match text[start..].find('\n') {
            Some(newline) => start += newline + 1,
            None => return text.len(),
        }
    }
    let mut units = 0;
    for (index, character_value) in text[start..].char_indices() {
        if character_value == '\n' || units >= character {
            return start + index;
        }
        units += character_value.len_utf16() as u64;
    }
    text.len()
}

/// The identifier token touching `offset`, if any.
fn identifier_at(text: &str, offset: usize) -> Option<(String, Span)> {
    crate::lexer::lex(text)
        .ok()?
        .into_iter()
        .find_map(|token| match token.kind {
            crate::lexer::Kind::Ident(name)
                if token.span.start <= offset && offset <= token.span.end =>
            {
                Some((name, token.span))
            }
            _ => None,
        })
}

/// Where the identifier at `offset` is bound.
fn definition_span(text: &str, offset: usize) -> Option<Span> {
    let (name, usage) = identifier_at(text, offset)?;
    let program = crate::parse(text).ok()?;
    let tokens = crate::lexer::lex(text).ok()?;
    let tokens: Vec<_> = tokens.iter().collect();
    binding(&program, &tokens, &name, usage)
}

/// Where `name`, used at `usage`, is bound: the nearest local binding at or
/// before it in the enclosing function, else a top-level declaration of that name.
fn binding(
    program: &crate::ast::Program,
    tokens: &[&crate::lexer::Token],
    name: &str,
    usage: Span,
) -> Option<Span> {
    use crate::lexer::Kind;
    let is_name = |kind: &Kind| matches!(kind, Kind::Ident(candidate) if *candidate == name);
    if let Some(function) = program
        .functions
        .iter()
        .find(|function| function.span.start <= usage.start && usage.end <= function.span.end)
    {
        let inside: Vec<_> = tokens
            .iter()
            .copied()
            .filter(|token| {
                function.span.start <= token.span.start && token.span.end <= function.span.end
            })
            .collect();
        let opening = |at: usize| {
            inside[at].kind == Kind::Pipe
                && inside[..at]
                    .iter()
                    .filter(|token| token.kind == Kind::Pipe)
                    .count()
                    % 2
                    == 0
        };
        let binds = |at: usize| {
            let previous = at.checked_sub(1).map(|before| &inside[before].kind);
            let next = inside.get(at + 1).map(|token| &token.kind);
            matches!(previous, Some(Kind::Let | Kind::Var | Kind::For))
                || (matches!(previous, Some(Kind::LParen | Kind::Comma))
                    && next == Some(&Kind::Colon)
                    && inside[..at].iter().all(|token| token.kind != Kind::LBrace))
                || (at > 0 && opening(at - 1))
                || (matches!(previous, Some(Kind::Comma))
                    && lambda_parameter(&inside, at).is_some_and(opening))
                || (matches!(previous, Some(Kind::LParen))
                    && next == Some(&Kind::RParen)
                    && inside[at + 1..]
                        .iter()
                        .find(|token| token.kind != Kind::RParen)
                        .is_some_and(|token| token.kind == Kind::FatArrow))
                || (matches!(previous, Some(Kind::LBrace | Kind::Comma))
                    && next == Some(&Kind::FatArrow))
        };
        if let Some(at) = (0..inside.len())
            .rev()
            .filter(|at| inside[*at].span.start <= usage.start)
            .find(|at| is_name(&inside[*at].kind) && binds(*at))
        {
            return Some(inside[at].span);
        }
    }
    let declaration = program
        .functions
        .iter()
        .map(|function| function.span)
        .chain(program.records.iter().map(|record| record.span))
        .chain(program.enums.iter().map(|enumeration| enumeration.span))
        .find(|span| {
            tokens
                .iter()
                .find(|token| {
                    span.start <= token.span.start && matches!(token.kind, Kind::Ident(_))
                })
                .is_some_and(|token| is_name(&token.kind))
        })?;
    tokens
        .iter()
        .find(|token| declaration.start <= token.span.start && is_name(&token.kind))
        .map(|token| token.span)
}

/// The `|` that would open a lambda parameter list around the identifier at `at`.
fn lambda_parameter(tokens: &[&crate::lexer::Token], at: usize) -> Option<usize> {
    use crate::lexer::Kind;
    let mut index = at;
    while index > 0 {
        index -= 1;
        match tokens[index].kind {
            Kind::Pipe => return Some(index),
            Kind::Ident(_) | Kind::Comma | Kind::Colon => {}
            Kind::LBracket
            | Kind::RBracket
            | Kind::Lt
            | Kind::Gt
            | Kind::LParen
            | Kind::RParen
            | Kind::Arrow => {}
            _ => return None,
        }
    }
    None
}

/// Identifier tokens bound where the one at `offset` is, in source order.
/// Field and variant names after `.` or `::` are not bindings and never match.
fn references(text: &str, offset: usize) -> Option<(Span, Vec<Span>)> {
    use crate::lexer::Kind;
    let (name, usage) = identifier_at(text, offset)?;
    let program = crate::parse(text).ok()?;
    let tokens = crate::lexer::lex(text).ok()?;
    let tokens: Vec<_> = tokens.iter().collect();
    let target = binding(&program, &tokens, &name, usage)?;
    let spans = tokens
        .iter()
        .enumerate()
        .filter(|(at, token)| {
            matches!(&token.kind, Kind::Ident(candidate) if *candidate == name)
                && !(*at > 0 && matches!(tokens[at - 1].kind, Kind::Dot | Kind::ColonColon))
        })
        .filter(|(_, token)| binding(&program, &tokens, &name, token.span) == Some(target))
        .map(|(_, token)| token.span)
        .collect();
    Some((target, spans))
}

const KEYWORDS: &[&str] = &[
    "struct", "enum", "import", "pub", "let", "var", "for", "while", "break", "continue", "in",
    "if", "match", "spawn", "else", "return", "true", "false", "Ok", "Err", "Some", "None",
];

fn identifier(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|rest| rest.is_ascii_alphanumeric() || rest == '_')
        && !KEYWORDS.contains(&name)
        && name != "fn"
}

/// Completion items at `offset`: field names after `.`, otherwise visible
/// locals, top-level declarations, builtins, and keywords. Works on tokens so
/// that half-written programs still complete.
fn completions(text: &str, offset: usize) -> Json {
    use crate::lexer::Kind;
    let tokens = crate::lexer::lex(text).unwrap_or_default();
    let before: Vec<_> = tokens
        .iter()
        .filter(|token| {
            token.span.end < offset
                || (token.span.end == offset && !matches!(token.kind, Kind::Ident(_)))
        })
        .collect();
    let mut items: Vec<(String, u8)> = Vec::new();
    let mut fields = Vec::new();
    let mut declarations = Vec::new();
    let mut depth = 0usize;
    let mut struct_body = false;
    for (at, token) in tokens.iter().enumerate() {
        let previous = at.checked_sub(1).map(|before| &tokens[before].kind);
        let next = tokens.get(at + 1).map(|token| &token.kind);
        match &token.kind {
            Kind::LBrace => depth += 1,
            Kind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    struct_body = false;
                }
            }
            Kind::Ident(name) if depth == 0 => match previous {
                Some(Kind::Struct) => {
                    declarations.push((name.clone(), 22));
                    struct_body = true;
                }
                Some(Kind::Enum) => declarations.push((name.clone(), 13)),
                _ if matches!(next, Some(Kind::LParen | Kind::Lt))
                    && !matches!(
                        previous,
                        Some(Kind::LParen | Kind::Comma | Kind::Colon | Kind::Arrow | Kind::Lt)
                    ) =>
                {
                    declarations.push((name.clone(), 3));
                }
                _ => {}
            },
            Kind::Ident(name) if struct_body && depth == 1 && next == Some(&Kind::Colon) => {
                fields.push((name.clone(), 5));
            }
            _ => {}
        }
    }
    if before.last().is_some_and(|token| token.kind == Kind::Dot) {
        items = fields;
    } else {
        let mut depth = 0usize;
        let mut pipes = 0usize;
        let mut locals: Vec<(String, u8)> = Vec::new();
        for (at, token) in before.iter().enumerate() {
            let previous = at.checked_sub(1).map(|index| &before[index].kind);
            let next = before.get(at + 1).map(|token| &token.kind);
            match &token.kind {
                Kind::LBrace => depth += 1,
                Kind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        locals.clear();
                        pipes = 0;
                    }
                }
                Kind::Pipe => pipes += 1,
                Kind::Ident(name) => {
                    let binds = matches!(previous, Some(Kind::Let | Kind::Var | Kind::For))
                        || (matches!(previous, Some(Kind::Pipe)) && pipes % 2 == 1)
                        || (matches!(previous, Some(Kind::LParen | Kind::Comma))
                            && next == Some(&Kind::Colon))
                        || (matches!(previous, Some(Kind::Comma)) && pipes % 2 == 1);
                    if binds {
                        locals.push((name.clone(), 6));
                    }
                }
                _ => {}
            }
        }
        items.extend(locals.into_iter().rev());
        items.extend(declarations);
        items.extend(
            crate::builtins::CALLS
                .iter()
                .map(|name| ((*name).to_owned(), 3)),
        );
        items.extend(KEYWORDS.iter().map(|name| ((*name).to_owned(), 14)));
    }
    let mut seen = HashSet::new();
    json!(
        items
            .into_iter()
            .filter(|(label, _)| seen.insert(label.clone()))
            .enumerate()
            .map(|(rank, (label, kind))| json!({"label": label, "kind": kind, "sortText": format!("{rank:04}")}))
            .collect::<Vec<_>>()
    )
}

/// A declaration in another file, reached through an import alias.
struct ImportedDeclaration {
    uri: String,
    text: String,
    /// The declaration's name in `text`.
    name: Span,
    /// The declaration up to its body, for hover.
    signature: String,
    /// The qualified name at the cursor in the current document.
    usage: Span,
}

/// The declaration that `alias::name` (or `alias::Enum::Variant`) at `offset`
/// refers to, resolved through the module graph with unsaved editor text.
fn imported_declaration(
    documents: &HashMap<String, Document>,
    uri: &str,
    text: &str,
    offset: usize,
) -> Option<ImportedDeclaration> {
    use crate::lexer::Kind;
    let tokens = crate::lexer::lex(text).ok()?;
    let at = tokens.iter().position(|token| {
        token.span.start <= offset
            && offset <= token.span.end
            && matches!(token.kind, Kind::Ident(_))
    })?;
    // Walk back over `ident ::` pairs to the first segment of the path.
    let mut first = at;
    while first >= 2
        && tokens[first - 1].kind == Kind::ColonColon
        && matches!(tokens[first - 2].kind, Kind::Ident(_))
    {
        first -= 2;
    }
    let Kind::Ident(alias) = &tokens[first].kind else {
        return None;
    };
    let Kind::Ident(member) = &tokens.get(first + 2)?.kind else {
        return None;
    };
    if first == at || tokens.get(first + 1)?.kind != Kind::ColonColon {
        return None;
    }
    let path = file_uri_path(uri)?;
    let mut overrides = HashMap::new();
    let mut uri_for_path = HashMap::new();
    for (open_uri, document) in documents {
        if let Some(open_path) = file_uri_path(open_uri) {
            overrides.insert(open_path.clone(), document.text.clone());
            uri_for_path.insert(open_path, open_uri.clone());
        }
    }
    let loaded = crate::modules::load_with_overrides(&path, &overrides).ok()?;
    let canonical = std::fs::canonicalize(&path).unwrap_or(path);
    let module = loaded.modules.iter().find(|module| {
        loaded
            .sources
            .get(module.source_id)
            .is_some_and(|source| source.path == canonical)
    })?;
    let target_id = module
        .imports
        .iter()
        .find(|(name, _)| name == alias)
        .map(|(_, id)| *id)?;
    let target = loaded.sources.get(target_id)?;
    let program = crate::parse(&target.text).ok()?;
    let (span, body_start) = program
        .functions
        .iter()
        .find(|function| function.name == *member)
        .map(|function| (function.span, function.body.span.start))
        .or_else(|| {
            program
                .records
                .iter()
                .find(|record| record.name == *member)
                .map(|record| (record.span, record.span.end))
        })
        .or_else(|| {
            program
                .enums
                .iter()
                .find(|declaration| declaration.name == *member)
                .map(|declaration| (declaration.span, declaration.span.end))
        })?;
    let name = crate::lexer::lex(&target.text)
        .ok()?
        .into_iter()
        .find(|token| {
            span.start <= token.span.start
                && matches!(&token.kind, Kind::Ident(candidate) if candidate == member)
        })?
        .span;
    Some(ImportedDeclaration {
        uri: uri_for_path
            .get(&target.path)
            .cloned()
            .or_else(|| path_uri(&target.path))?,
        signature: target.text[span.start..body_start].trim().to_owned(),
        text: target.text.clone(),
        name,
        usage: tokens[first].span.join(tokens[at].span),
    })
}

/// One use of a binding somewhere in the project.
struct Location {
    uri: String,
    range: Json,
    declaration: bool,
}

/// The editor text for every open document by canonical path, and the URI
/// each path is open under.
fn open_buffers(
    documents: &HashMap<String, Document>,
) -> (HashMap<PathBuf, String>, HashMap<PathBuf, String>) {
    let mut overrides = HashMap::new();
    let mut uris = HashMap::new();
    for (uri, document) in documents {
        if let Some(path) = file_uri_path(uri) {
            let path = fs::canonicalize(&path).unwrap_or(path);
            overrides.insert(path.clone(), document.text.clone());
            uris.insert(path, uri.clone());
        }
    }
    (overrides, uris)
}

/// Every use of the binding at `offset`. Local bindings and private
/// declarations stay within the document; a public top-level declaration is
/// also found as `alias::name` in every module of the open documents' module
/// graphs that imports its file.
fn project_references(
    documents: &HashMap<String, Document>,
    uri: &str,
    text: &str,
    offset: usize,
) -> Option<(String, Vec<Location>)> {
    use crate::lexer::Kind;
    // Resolve to the declaring file and the declaration's name token.
    let (home_uri, home_text, declaration, name) =
        if let Some(target) = imported_declaration(documents, uri, text, offset) {
            let name = target.text[target.name.start..target.name.end].to_owned();
            (target.uri, target.text, target.name, name)
        } else {
            let (identifier, _) = identifier_at(text, offset)?;
            let (target, _) = references(text, offset)?;
            (uri.to_owned(), text.to_owned(), target, identifier)
        };
    let (_, home_spans) = references(&home_text, declaration.start)?;
    let mut found: Vec<Location> = home_spans
        .into_iter()
        .map(|span| Location {
            uri: home_uri.clone(),
            range: range(&home_text, span),
            declaration: span == declaration,
        })
        .collect();
    // Only public top-level declarations are reachable from other files.
    let program = crate::parse(&home_text).ok()?;
    let tokens = crate::lexer::lex(&home_text).ok()?;
    let is_public_declaration = |span: Span| {
        program
            .functions
            .iter()
            .map(|function| (function.public, function.span))
            .chain(
                program
                    .records
                    .iter()
                    .map(|record| (record.public, record.span)),
            )
            .chain(
                program
                    .enums
                    .iter()
                    .map(|declaration| (declaration.public, declaration.span)),
            )
            .any(|(public, declared)| {
                public
                    && tokens
                        .iter()
                        .find(|token| {
                            declared.start <= token.span.start
                                && matches!(token.kind, Kind::Ident(_))
                        })
                        .is_some_and(|token| token.span == span)
            })
    };
    if !is_public_declaration(declaration) {
        return Some((name, found));
    }
    let home_path = file_uri_path(&home_uri)?;
    let home_path = fs::canonicalize(&home_path).unwrap_or(home_path);
    let (overrides, uris) = open_buffers(documents);
    let mut seen = HashSet::new();
    for entry in documents.keys().filter_map(|open| file_uri_path(open)) {
        let Ok(loaded) = crate::modules::load_with_overrides(&entry, &overrides) else {
            continue;
        };
        let Some(home_id) = (0..loaded.sources.len())
            .map(crate::ast::SourceId)
            .find(|id| {
                loaded
                    .sources
                    .get(*id)
                    .is_some_and(|source| source.path == home_path)
            })
        else {
            continue;
        };
        for module in &loaded.modules {
            let Some(source) = loaded.sources.get(module.source_id) else {
                continue;
            };
            if !seen.insert(source.path.clone()) {
                continue;
            }
            let aliases: Vec<&str> = module
                .imports
                .iter()
                .filter(|(_, id)| *id == home_id)
                .map(|(alias, _)| alias.as_str())
                .collect();
            if aliases.is_empty() {
                continue;
            }
            let Ok(module_tokens) = crate::lexer::lex(&source.text) else {
                continue;
            };
            let Some(module_uri) = uris
                .get(&source.path)
                .cloned()
                .or_else(|| path_uri(&source.path))
            else {
                continue;
            };
            for window in module_tokens.windows(3) {
                if let (Kind::Ident(alias), Kind::ColonColon, Kind::Ident(member)) =
                    (&window[0].kind, &window[1].kind, &window[2].kind)
                    && aliases.contains(&alias.as_str())
                    && *member == name
                {
                    found.push(Location {
                        uri: module_uri.clone(),
                        range: range(&source.text, window[2].span),
                        declaration: false,
                    });
                }
            }
        }
    }
    Some((name, found))
}

/// Markdown for the declaration or checked type at `offset`.
fn hover_text(text: &str, offset: usize) -> Option<(String, Span)> {
    let (name, usage) = identifier_at(text, offset)?;
    let program = crate::parse(text).ok()?;
    let excerpt = |start: usize, end: usize| text[start..end].trim().to_owned();
    let declaration = program
        .functions
        .iter()
        .find(|function| function.name == name)
        .map(|function| excerpt(function.span.start, function.body.span.start))
        .or_else(|| {
            program
                .records
                .iter()
                .find(|record| record.name == name)
                .map(|record| excerpt(record.span.start, record.span.end))
        })
        .or_else(|| {
            program
                .enums
                .iter()
                .find(|enumeration| enumeration.name == name)
                .map(|enumeration| excerpt(enumeration.span.start, enumeration.span.end))
        });
    let local_type = crate::checker::check_with_types(&program)
        .ok()
        .and_then(|types| types.get(&usage).map(|ty| format!("{name}: {ty}")));
    let shown = match (local_type, declaration) {
        (Some(local), _) => local,
        (None, Some(declaration)) => declaration,
        (None, None) => return None,
    };
    Some((format!("```tokit\n{shown}\n```"), usage))
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
                                    "definitionProvider": true,
                                    "hoverProvider": true,
                                    "referencesProvider": true,
                                    "renameProvider": true,
                                    "completionProvider": {"triggerCharacters": ["."]},
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
            Some(method @ ("textDocument/definition" | "textDocument/hover"))
                if initialized && !shutdown && id.is_some() =>
            {
                let uri = params["textDocument"]["uri"].as_str();
                let result = uri
                    .and_then(|uri| documents.get(uri).map(|document| (uri, document)))
                    .and_then(|(uri, document)| {
                        let offset = offset_at(
                            &document.text,
                            params["position"]["line"].as_u64()?,
                            params["position"]["character"].as_u64()?,
                        );
                        if let Some(target) = imported_declaration(&documents, uri, &document.text, offset) {
                            return Some(if method == "textDocument/definition" {
                                json!({"uri": target.uri, "range": range(&target.text, target.name)})
                            } else {
                                json!({
                                    "contents": {"kind": "markdown", "value": format!("```tokit
{}
```", target.signature)},
                                    "range": range(&document.text, target.usage),
                                })
                            });
                        }
                        if method == "textDocument/definition" {
                            let span = definition_span(&document.text, offset)?;
                            Some(json!({"uri": uri, "range": range(&document.text, span)}))
                        } else {
                            let (value, span) = hover_text(&document.text, offset)?;
                            Some(json!({
                                "contents": {"kind": "markdown", "value": value},
                                "range": range(&document.text, span),
                            }))
                        }
                    })
                    .unwrap_or(Json::Null);
                send(output, &response(id.expect("checked id"), result))?;
            }
            Some(method @ ("textDocument/references" | "textDocument/rename"))
                if initialized && !shutdown && id.is_some() =>
            {
                let id = id.expect("checked id");
                let uri = params["textDocument"]["uri"].as_str();
                let found = uri
                    .and_then(|uri| documents.get(uri).map(|document| (uri, document)))
                    .and_then(|(uri, document)| {
                        let offset = offset_at(
                            &document.text,
                            params["position"]["line"].as_u64()?,
                            params["position"]["character"].as_u64()?,
                        );
                        project_references(&documents, uri, &document.text, offset)
                    });
                let message = if method == "textDocument/references" {
                    let include_declaration = params["context"]["includeDeclaration"]
                        .as_bool()
                        .unwrap_or(true);
                    let result = found
                        .map(|(_, locations)| {
                            json!(
                                locations
                                    .into_iter()
                                    .filter(|location| include_declaration || !location.declaration)
                                    .map(|location| json!({"uri": location.uri, "range": location.range}))
                                    .collect::<Vec<_>>()
                            )
                        })
                        .unwrap_or(Json::Null);
                    response(id, result)
                } else {
                    let new_name = params["newName"].as_str().unwrap_or_default();
                    match found {
                        _ if !identifier(new_name) || crate::builtins::is_call(new_name) => {
                            error_response(id, -32602, "new name must be a non-reserved identifier")
                        }
                        None => error_response(id, -32602, "nothing to rename here"),
                        Some((_, locations)) => {
                            let mut changes = serde_json::Map::new();
                            for location in locations {
                                let edits =
                                    changes.entry(location.uri).or_insert_with(|| json!([]));
                                if let Some(list) = edits.as_array_mut() {
                                    list.push(
                                        json!({"range": location.range, "newText": new_name}),
                                    );
                                }
                            }
                            response(id, json!({"changes": changes}))
                        }
                    }
                };
                send(output, &message)?;
            }
            Some("textDocument/completion") if initialized && !shutdown && id.is_some() => {
                let uri = params["textDocument"]["uri"].as_str();
                let result = uri
                    .and_then(|uri| documents.get(uri))
                    .and_then(|document| {
                        let offset = offset_at(
                            &document.text,
                            params["position"]["line"].as_u64()?,
                            params["position"]["character"].as_u64()?,
                        );
                        Some(completions(&document.text, offset))
                    })
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

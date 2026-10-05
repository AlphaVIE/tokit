//! Checked, optimistic function patches against a source snapshot.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::ast::Span;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u32,
    pub target: String,
    pub edits: Vec<FunctionEdit>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionEdit {
    pub function: String,
    pub expected_sha256: String,
    pub replacement: String,
}

#[derive(Debug)]
pub struct PatchError {
    pub code: &'static str,
    pub message: String,
}

impl PatchError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn json(&self) -> String {
        json!({"ok":false,"code":self.code,"message":self.message}).to_string()
    }
}

pub fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn declaration(source: &str, span: Span) -> &str {
    &source[span.start..span.end]
}

/// List name-based identities, byte spans, and precondition hashes.
pub fn index(source: &str) -> Result<String, PatchError> {
    index_function(source, None)
}

/// Return the same schema restricted to a unique named function, when supplied.
pub fn index_function(source: &str, name: Option<&str>) -> Result<String, PatchError> {
    let program =
        crate::parse(source).map_err(|error| PatchError::new(error.code, error.display(source)))?;
    let functions = program
        .functions
        .iter()
        .filter(|function| name.is_none_or(|name| function.name == name))
        .map(|function| {
            json!({
                "name": function.name,
                "span": [function.span.start, function.span.end],
                "sha256": digest(declaration(source, function.span)),
            })
        })
        .collect::<Vec<Value>>();
    if name.is_some() && functions.len() != 1 {
        return Err(PatchError::new(
            "P003",
            "function target is missing or ambiguous",
        ));
    }
    Ok(json!({"version":1,"functions":functions}).to_string())
}

/// Apply every edit to a copy. The caller validates the whole module graph
/// before persisting this text, so a failed edit never changes the source.
pub fn apply(source: &str, edits: &[FunctionEdit]) -> Result<String, PatchError> {
    if edits.is_empty() {
        return Err(PatchError::new("P001", "patch has no edits"));
    }
    let program =
        crate::parse(source).map_err(|error| PatchError::new(error.code, error.display(source)))?;
    let mut seen = HashSet::new();
    let imports = program
        .imports
        .iter()
        .map(|import| declaration(source, import.span))
        .collect::<Vec<_>>()
        .join("\n");
    let mut replacements = Vec::with_capacity(edits.len());
    for edit in edits {
        if !seen.insert(&edit.function) {
            return Err(PatchError::new("P002", "duplicate function target"));
        }
        let function = program
            .functions
            .iter()
            .find(|function| function.name == edit.function)
            .ok_or_else(|| {
                PatchError::new("P003", format!("unknown function {}", edit.function))
            })?;
        let actual_hash = digest(declaration(source, function.span));
        if program
            .functions
            .iter()
            .filter(|item| item.name == edit.function)
            .count()
            != 1
        {
            return Err(PatchError::new("P003", "ambiguous function target"));
        }
        if actual_hash != edit.expected_sha256 {
            return Err(PatchError::new(
                "P004",
                format!("stale function {}", edit.function),
            ));
        }
        let replacement_source = format!("{imports}\n{}", edit.replacement);
        let replacement = crate::parse(&replacement_source)
            .map_err(|error| PatchError::new(error.code, error.display(&replacement_source)))?;
        if replacement.imports.len() != program.imports.len()
            || !replacement.records.is_empty()
            || !replacement.enums.is_empty()
            || replacement.functions.len() != 1
            || replacement.functions[0].name != edit.function
            || replacement.functions[0].public
            || declaration(&replacement_source, replacement.functions[0].span)
                != edit.replacement.trim()
        {
            return Err(PatchError::new(
                "P005",
                format!(
                    "replacement must contain only fn {} without pub",
                    edit.function
                ),
            ));
        }
        replacements.push((
            function.span.start,
            function.span.end,
            edit.replacement.trim().to_owned(),
        ));
    }
    replacements.sort_unstable_by_key(|item| std::cmp::Reverse(item.0));
    let mut result = source.to_owned();
    for (start, end, replacement) in replacements {
        result.replace_range(start..end, &replacement);
    }
    Ok(result)
}

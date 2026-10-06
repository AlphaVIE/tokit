//! `tok doc`: Markdown reference for a module's declarations. The comment
//! block at the top of the file describes the module; the `//` lines directly
//! above a later declaration describe that declaration.

use crate::ast::{Program, Span};

/// Comment lines ending on the line before `span`, unless they form the file header.
fn leading_comment(source: &str, span: Span) -> String {
    let before = &source[..span.start.min(source.len())];
    // A `pub` keyword on the same line belongs to the declaration.
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    if !matches!(before[line_start..].trim(), "" | "pub") {
        return String::new();
    }
    let before = &before[..line_start];
    let lines: Vec<&str> = before.lines().collect();
    let mut first = lines.len();
    while first > 0 && lines[first - 1].trim_start().starts_with("//") {
        first -= 1;
    }
    if first == 0 {
        return String::new();
    }
    comment_text(&lines[first..])
}

fn comment_text(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|line| line.trim_start().trim_start_matches("//").trim())
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe(comment: &str) -> String {
    if comment.is_empty() {
        String::new()
    } else {
        format!("{comment}\n\n")
    }
}

/// Markdown documenting public declarations (or all with `include_private`).
pub fn document(program: &Program, source: &str, title: &str, include_private: bool) -> String {
    let mut out = format!("# {title}\n");
    let header: Vec<&str> = source
        .lines()
        .take_while(|line| line.trim_start().starts_with("//"))
        .collect();
    if !header.is_empty() {
        out.push_str(&format!("\n{}\n", comment_text(&header)));
    }
    let shown = |public: bool| public || include_private;
    let mut entries: Vec<(usize, String)> = Vec::new();
    for record in program.records.iter().filter(|record| shown(record.public)) {
        let fields = record
            .fields
            .iter()
            .map(|(name, ty)| format!("- `{name}: {ty}`"))
            .collect::<Vec<_>>()
            .join("\n");
        entries.push((
            record.span.start,
            format!(
                "## struct `{}`\n\n```tokit\n{}\n```\n\n{}{fields}",
                record.name,
                source[record.span.start..record.span.end].trim(),
                describe(&leading_comment(source, record.span)),
            ),
        ));
    }
    for decl in program.enums.iter().filter(|decl| shown(decl.public)) {
        entries.push((
            decl.span.start,
            format!(
                "## enum `{}`\n\n```tokit\n{}\n```\n\n{}",
                decl.name,
                source[decl.span.start..decl.span.end].trim(),
                describe(&leading_comment(source, decl.span)),
            ),
        ));
    }
    for function in program
        .functions
        .iter()
        .filter(|function| shown(function.public))
    {
        entries.push((
            function.span.start,
            format!(
                "## `{}`\n\n```tokit\n{}\n```\n\n{}",
                function.name,
                source[function.span.start..function.body.span.start].trim(),
                describe(&leading_comment(source, function.span)),
            ),
        ));
    }
    entries.sort_by_key(|(start, _)| *start);
    for (_, entry) in entries {
        out.push('\n');
        out.push_str(entry.trim_end());
        out.push('\n');
    }
    out
}

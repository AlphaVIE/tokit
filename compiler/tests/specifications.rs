//! The specification documents must stay in step with the compiler.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Codes such as `E116` or `W001` that appear in `text` between quotes or backticks.
fn codes(text: &str, quote: char) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();
    for (at, character) in text.char_indices() {
        if character != quote || at + 5 >= bytes.len() {
            continue;
        }
        let candidate = &text[at + 1..at + 5];
        let mut chars = candidate.chars();
        if matches!(chars.next(), Some('E' | 'W'))
            && chars.all(|digit| digit.is_ascii_digit())
            && matches!(bytes[at + 5], b'"' | b'`' | b':')
        {
            found.insert(candidate.to_owned());
        }
    }
    found
}

#[test]
fn diagnostics_reference_lists_exactly_the_emitted_codes() {
    let mut emitted = BTreeSet::new();
    for entry in std::fs::read_dir(root().join("compiler/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            emitted.extend(codes(&std::fs::read_to_string(path).unwrap(), '"'));
        }
    }
    let documented: BTreeSet<String> = std::fs::read_to_string(root().join("spec/DIAGNOSTICS.md"))
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| line[3..7].to_owned())
        .collect();
    assert_eq!(documented, emitted);
}

#[test]
fn grammar_lists_every_keyword() {
    let grammar = std::fs::read_to_string(root().join("spec/GRAMMAR.md")).unwrap();
    let line = grammar
        .lines()
        .skip_while(|line| !line.starts_with("Keywords:"))
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let listed: BTreeSet<&str> = line.split('`').nth(1).unwrap().split_whitespace().collect();
    for word in &listed {
        let tokens = tokit_compiler::lexer::lex(word).unwrap();
        assert!(
            !matches!(tokens[0].kind, tokit_compiler::lexer::Kind::Ident(_)),
            "{word} is listed as a keyword but lexes as a name"
        );
    }
    assert_eq!(listed.len(), 23, "the lexer has 23 keywords");
}

#[test]
fn specification_index_links_resolve() {
    let index = std::fs::read_to_string(root().join("spec/README.md")).unwrap();
    let rows = index
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| #"));
    let mut topics = 0;
    for row in rows.filter(|row| !row.starts_with("| ---")) {
        topics += 1;
        for link in row.split("](").skip(1) {
            let target = link.split(')').next().unwrap();
            assert!(
                root().join("spec").join(target).is_file(),
                "broken link {target}"
            );
        }
    }
    assert_eq!(topics, 32);
}

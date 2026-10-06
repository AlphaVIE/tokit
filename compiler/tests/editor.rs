//! The bundled VS Code extension must stay consistent with the language.

use std::path::Path;

use serde_json::Value;
use tokit_compiler::lexer::{Kind, lex};

fn read_json(relative: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../editors/vscode")
        .join(relative);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// Words of a `\b(a|b|c)\b` pattern.
fn alternatives(pattern: &str) -> Vec<String> {
    pattern
        .trim_start_matches("\\b(")
        .trim_end_matches(")\\b")
        .split('|')
        .map(str::to_owned)
        .collect()
}

#[test]
fn manifest_points_at_existing_files() {
    let manifest = read_json("package.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../editors/vscode");
    let contributes = &manifest["contributes"];
    for path in [
        manifest["main"].as_str().unwrap(),
        contributes["languages"][0]["configuration"]
            .as_str()
            .unwrap(),
        contributes["grammars"][0]["path"].as_str().unwrap(),
    ] {
        assert!(root.join(path).is_file(), "{path}");
    }
    assert_eq!(contributes["languages"][0]["extensions"][0], ".tok");
    read_json("language-configuration.json");
}

#[test]
fn grammar_keywords_are_lexer_keywords() {
    let grammar = read_json("syntaxes/tokit.tmLanguage.json");
    let mut highlighted = Vec::new();
    for pattern in grammar["repository"]["keywords"]["patterns"]
        .as_array()
        .unwrap()
    {
        highlighted.extend(alternatives(pattern["match"].as_str().unwrap()));
    }
    for word in &highlighted {
        let tokens = lex(word).unwrap();
        assert!(
            !matches!(tokens[0].kind, Kind::Ident(_)),
            "{word} is highlighted as a keyword but lexes as a name"
        );
    }
    // Every reserved word the lexer knows must be highlighted.
    let source = "fn struct enum import pub let var for while break continue in if match spawn else return true false Ok Err Some None";
    for word in source.split(' ') {
        assert!(
            highlighted.iter().any(|known| known == word),
            "{word} missing from grammar"
        );
    }
}

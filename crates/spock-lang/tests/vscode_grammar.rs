use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use spock_lang::lexer::{lex, ACTIVE_KEYWORDS, CONTEXTUAL_KEYWORDS, RESERVED_KEYWORDS};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn read_json(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    serde_json::from_str(&fs::read_to_string(path).expect("read JSON"))
        .unwrap_or_else(|error| panic!("{} is valid JSON: {error}", path.display()))
}

fn words_in_match(pattern: &str) -> BTreeSet<String> {
    let body = pattern
        .strip_prefix(r"\b(?:")
        .and_then(|value| value.strip_suffix(r")\b"))
        .unwrap_or_else(|| panic!("expected an exact word-set pattern, got {pattern:?}"));
    body.split('|').map(str::to_owned).collect()
}

#[test]
fn vscode_manifest_registers_spock_files_and_grammar() {
    let root = repo_root().join("editors/vscode-spock");
    let manifest = read_json(root.join("package.json"));
    let language = &manifest["contributes"]["languages"][0];
    let grammar = &manifest["contributes"]["grammars"][0];

    assert_eq!(language["id"], "spock");
    assert_eq!(language["extensions"][0], ".spock");
    assert!(root.join("language-configuration.json").is_file());
    assert_eq!(grammar["language"], "spock");
    assert_eq!(grammar["scopeName"], "source.spock");
    assert!(root.join("syntaxes/spock.tmLanguage.json").is_file());
}

#[test]
fn textmate_keyword_sets_match_the_lexer_vocabulary() {
    let grammar =
        read_json(repo_root().join("editors/vscode-spock/syntaxes/spock.tmLanguage.json"));
    let repository = grammar["repository"].as_object().expect("repository");

    let active = repository["active-keywords"]["patterns"]
        .as_array()
        .expect("active keyword patterns")
        .iter()
        .flat_map(|entry| words_in_match(entry["match"].as_str().expect("active keyword match")))
        .collect::<BTreeSet<_>>();
    let expected_active = ACTIVE_KEYWORDS
        .iter()
        .map(|(word, _)| (*word).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(active, expected_active);

    let reserved_pattern = repository["reserved-keywords"]["patterns"][0]["match"]
        .as_str()
        .expect("reserved keyword match");
    let reserved = words_in_match(reserved_pattern);
    let expected_reserved = RESERVED_KEYWORDS
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(reserved, expected_reserved);
}

#[test]
fn canonical_vocabulary_has_the_expected_lexical_behavior() {
    for (word, expected) in ACTIVE_KEYWORDS {
        let tokens = lex(word).unwrap_or_else(|error| panic!("{word}: {error:?}"));
        assert_eq!(&tokens[0].kind, expected, "active keyword {word}");
    }

    for word in RESERVED_KEYWORDS {
        let error = lex(word).expect_err("reserved keyword must be rejected");
        assert_eq!(error.code, "L005", "reserved keyword {word}");
    }

    for word in CONTEXTUAL_KEYWORDS {
        let tokens = lex(word).unwrap_or_else(|error| panic!("{word}: {error:?}"));
        assert!(
            matches!(&tokens[0].kind, spock_lang::lexer::TokenKind::Ident(name) if name == word),
            "contextual keyword {word} must remain an identifier to the lexer"
        );
    }
}

#[test]
fn textmate_grammar_covers_contextual_forms_and_lexical_edges() {
    let grammar =
        fs::read_to_string(repo_root().join("editors/vscode-spock/syntaxes/spock.tmLanguage.json"))
            .expect("read grammar");

    for required in [
        "unchecked",
        "sql",
        "file",
        "comment.line.documentation.inner.spock",
        "comment.line.documentation.outer.spock",
        "string.quoted.triple.spock",
        "invalid.illegal.reserved.spock",
        "invalid.illegal.identifier.spock",
    ] {
        assert!(grammar.contains(required), "grammar must cover {required}");
    }
}

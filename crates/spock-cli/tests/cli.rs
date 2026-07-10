//! CLI behavior: exit codes, rendered diagnostics, contract output.

use assert_cmd::Command;
use predicates::prelude::*;

fn spock() -> Command {
    Command::cargo_bin("spock").expect("binary builds")
}

fn write_program(dir: &std::path::Path, source: &str) -> std::path::PathBuf {
    let path = dir.join("app.spock");
    std::fs::write(&path, source).expect("write temp program");
    path
}

#[test]
fn check_accepts_a_valid_program() {
    let dir = std::env::temp_dir().join("spock-cli-test-ok");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_program(
        &dir,
        "table user { key id: uuid = auto\n username: text unique }\n\
         fn find(username: text) -> user? { unchecked sql(\"SELECT * FROM user WHERE username = :username\") }\n\
         seed { user { username: \"maya\" } }",
    );
    spock()
        .args(["check", path.to_str().unwrap()])
        .assert()
        .success()
        // the unchecked-body count is the ledger (RFD 0011 §4)
        .stdout(predicate::str::contains(
            "ok: 1 table(s), 0 record(s), 1 fn(s) (1 unchecked escapes), 1 seed row(s)",
        ));
}

#[test]
fn check_renders_diagnostics_and_fails() {
    let dir = std::env::temp_dir().join("spock-cli-test-bad");
    std::fs::create_dir_all(&dir).unwrap();
    // two independent errors: no key (E005) and an unknown type (E003)
    let path = write_program(&dir, "table a { x: nope }");
    spock()
        .args(["check", path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("error[E003]"))
        .stderr(predicate::str::contains("error[E005]"))
        .stderr(predicate::str::contains("app.spock:1:"));
}

#[test]
fn build_emits_the_contract() {
    let dir = std::env::temp_dir().join("spock-cli-test-build");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_program(
        &dir,
        "table user { key id: uuid = auto\n username: text unique }",
    );
    let assert = spock()
        .args(["build", path.to_str().unwrap()])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let contract: serde_json::Value = serde_json::from_str(&out).expect("stdout is the contract");
    assert_eq!(contract["spock"], "v0");
    assert_eq!(contract["tables"][0]["name"], "user");
    // derived errors are in the artifact
    let codes: Vec<&str> = contract["tables"][0]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"user_username_taken"));
}

#[test]
fn missing_file_is_a_clean_error() {
    spock()
        .args(["check", "/definitely/not/a/file.spock"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("could not read"));
}

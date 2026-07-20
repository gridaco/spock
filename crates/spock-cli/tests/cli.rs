//! CLI behavior: exit codes, rendered diagnostics, contract output.

use assert_cmd::Command;
use predicates::prelude::*;

#[cfg(unix)]
use assert_cmd::prelude::CommandCargoExt;

fn spock() -> Command {
    Command::cargo_bin("spock").expect("binary builds")
}

fn write_program(dir: &std::path::Path, source: &str) -> std::path::PathBuf {
    let path = dir.join("app.spock");
    std::fs::write(&path, source).expect("write temp program");
    path
}

#[cfg(unix)]
fn assert_sigterm_shutdown(cwd: &std::path::Path, command_name: &str, arguments: &[&str]) {
    use std::net::{TcpListener, TcpStream};
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};

    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let mut command = std::process::Command::cargo_bin("spock").unwrap();
    let mut child = command
        .current_dir(cwd)
        .arg(command_name)
        .args(arguments)
        .args(["--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let ready_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("server exited before readiness: {status}");
        }
        if Instant::now() >= ready_deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("server did not become ready");
        }
        thread::sleep(Duration::from_millis(25));
    }

    let signal = std::process::Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(signal.success());

    let shutdown_deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= shutdown_deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("server did not terminate after SIGTERM");
        }
        thread::sleep(Duration::from_millis(25));
    };
    assert!(status.success(), "server exited with {status}");

    let rebound = TcpListener::bind(("127.0.0.1", port)).unwrap();
    drop(rebound);
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
fn relative_standalone_diagnostics_keep_the_caller_spelling() {
    let temporary = tempfile::tempdir().unwrap();
    write_program(temporary.path(), "table a { x: nope }");

    spock()
        .current_dir(temporary.path())
        .args(["check", "app.spock"])
        .assert()
        .failure()
        .stderr(predicate::str::starts_with("app.spock:1:"))
        .stderr(predicate::str::contains(temporary.path().to_string_lossy().as_ref()).not());
}

#[cfg(unix)]
#[test]
fn standalone_check_resolves_seed_assets_beside_a_symlink_spelling() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().unwrap();
    let source_root = temporary.path().join("source");
    let invocation_root = temporary.path().join("invocation");
    std::fs::create_dir(&source_root).unwrap();
    std::fs::create_dir(&invocation_root).unwrap();
    std::fs::write(
        source_root.join("app.spock"),
        "auth table user { key id: uuid = auto\n \
         username: text unique\n avatar: storage_object? }\n\
         seed { u = user { username: \"u\", avatar: file(\"./pic.png\") } }\n",
    )
    .unwrap();
    std::fs::write(
        invocation_root.join("pic.png"),
        b"\x89PNG\r\n\x1a\nseed-bytes",
    )
    .unwrap();
    symlink("../source/app.spock", invocation_root.join("app.spock")).unwrap();

    spock()
        .current_dir(invocation_root)
        .args(["check", "app.spock"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 seed row(s)"));
}

#[test]
fn check_is_the_full_load_proof() {
    // `check` now materializes in memory, so a body that compiles but
    // cannot load (an unknown table) fails at check time (RFD 0013) —
    // no server ever starts.
    let dir = std::env::temp_dir().join("spock-cli-test-loadproof");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_program(
        &dir,
        "table t { key id: uuid = auto }\n\
         fn broken() -> [t] { unchecked sql(\"SELECT * FROM ghost\") }",
    );
    spock()
        .args(["check", path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no such table: ghost"));
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
        .stderr(predicate::str::starts_with("error: could not read"))
        .stderr(predicate::str::contains("error: error:").not());
}

#[test]
fn help_exposes_framework_commands_and_retained_file_tools() {
    spock()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("new"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("dev"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("gen"));
}

#[test]
fn new_defaults_to_a_checkable_full_stack_project() {
    let temporary = tempfile::tempdir().unwrap();

    spock()
        .current_dir(temporary.path())
        .args(["new", "demo"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "created full-stack project `demo`",
        ));

    let project = temporary.path().join("demo");
    assert!(project.join("spock.toml").is_file());
    assert!(project.join("backend/app.spock").is_file());
    assert!(project.join("client/uhura.toml").is_file());
    spock()
        .current_dir(&project)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: project `demo`"));
}

#[test]
fn backend_only_new_omits_the_client_tree() {
    let temporary = tempfile::tempdir().unwrap();

    spock()
        .current_dir(temporary.path())
        .args(["new", "authority", "--backend-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "created backend-only project `authority`",
        ));

    let project = temporary.path().join("authority");
    assert!(!project.join("client").exists());
    spock()
        .current_dir(project)
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("backend only"));
}

#[test]
fn next_step_text_is_not_an_executable_command() {
    let temporary = tempfile::tempdir().unwrap();

    spock()
        .current_dir(temporary.path())
        .args(["new", "demo;touch PWN", "--backend-only"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "next: run `spock dev` from the project directory above",
        ))
        .stdout(predicate::str::contains("next: cd").not());

    assert!(!temporary.path().join("PWN").exists());
}

#[test]
fn project_check_deduplicates_shared_editor_and_play_diagnostics() {
    let temporary = tempfile::tempdir().unwrap();
    spock()
        .current_dir(temporary.path())
        .args(["new", "demo"])
        .assert()
        .success();
    std::fs::write(
        temporary.path().join("demo/client/machine.uhura"),
        "not valid uhura\n",
    )
    .unwrap();

    let assertion = spock()
        .current_dir(temporary.path().join("demo"))
        .arg("check")
        .assert()
        .failure();
    let stderr = String::from_utf8(assertion.get_output().stderr.clone()).unwrap();
    let repeated = "[R1001 uhura/parse]";
    assert_eq!(stderr.matches(repeated).count(), 1, "{stderr}");
    assert!(stderr.contains("machine.uhura:"), "{stderr}");
}

#[test]
fn framework_serve_commands_reject_explicit_file_mode() {
    let temporary = tempfile::tempdir().unwrap();
    let source = write_program(temporary.path(), "");

    for command in ["start", "dev"] {
        spock()
            .current_dir(temporary.path())
            .args([command, source.to_str().unwrap()])
            .assert()
            .failure()
            .stderr(predicate::str::contains("spock run"));
    }
}

#[cfg(unix)]
#[test]
fn standalone_sigterm_shutdown_is_clean_and_releases_the_port() {
    let temporary = tempfile::tempdir().unwrap();
    write_program(temporary.path(), "");

    assert_sigterm_shutdown(temporary.path(), "run", &["app.spock"]);
}

#[cfg(unix)]
#[test]
fn framework_sigterm_shutdown_is_clean_and_releases_the_port() {
    let temporary = tempfile::tempdir().unwrap();
    spock()
        .current_dir(temporary.path())
        .args(["new", "authority", "--backend-only"])
        .assert()
        .success();

    assert_sigterm_shutdown(&temporary.path().join("authority"), "start", &[]);
}

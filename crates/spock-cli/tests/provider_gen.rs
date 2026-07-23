//! Provider-generation spike goldens (uhura#29): every generated artifact is
//! char-equal with the hand-written instagram provider it replaces. Fixtures
//! are mechanically extracted from gridaco/uhura examples (MIT © Grida).

use spock_cli::provider_gen as pg;

const CONTRACT: &str = include_str!("provider_fixtures/contract.json");
const WIRE: &str = include_str!("provider_fixtures/instagram.wire");
const PROVIDER_TS: &str = include_str!("provider_fixtures/spock-provider.ts");
const MANIFEST: &str = include_str!("provider_fixtures/manifest.toml");

fn schema() -> pg::SpockSchema {
    pg::extract_contract(CONTRACT).expect("contract parses")
}

#[test]
fn contract_exposes_the_storage_object_system_table() {
    let schema = schema();
    assert_eq!(schema.tables.len(), 11);
    assert!(schema.has_table("storage_object"));
}

#[test]
fn declaration_validates_clean_against_the_contract() {
    let file = pg::parse(WIRE).expect("parse");
    let problems = pg::validate_against(&file, &schema());
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn machine_contract_types_match_the_handwritten_machine_uhura() {
    let file = pg::parse(WIRE).expect("parse");
    let generated = pg::generate_machine_types(&file);
    let golden = include_str!("provider_fixtures/machine-types.uhura");
    assert_eq!(generated.trim(), golden.trim());
}

#[test]
fn view_types_match_the_handwritten_machine_uhura() {
    let file = pg::parse(WIRE).expect("parse");
    let generated = pg::generate_view_types(&file, &schema()).expect("views validate");
    let golden = include_str!("provider_fixtures/view-types.uhura");
    assert_eq!(generated.trim(), golden.trim());
}

#[test]
fn snapshot_query_matches_the_handwritten_adapter() {
    let file = pg::parse(WIRE).expect("parse");
    let generated = pg::generate_snapshot_query(&file, &schema()).expect("generates");
    let golden = include_str!("provider_fixtures/snapshot-query.graphql");
    assert_eq!(generated, golden);
}

#[test]
fn dispatch_switch_matches_the_handwritten_adapter() {
    let file = pg::parse(WIRE).expect("parse");
    let generated = pg::generate_dispatch(&file).expect("generates");
    let golden = include_str!("provider_fixtures/dispatch-switch.ts");
    assert_eq!(generated, golden);
}

#[test]
fn refusal_whitelist_semantically_matches_the_handwritten_table() {
    let file = pg::parse(WIRE).expect("parse");
    let mut generated = pg::generate_refusals(&file).expect("generates");
    for (_, list) in generated.iter_mut() {
        list.sort();
    }
    generated.sort();

    let start = PROVIDER_TS
        .find("const COMMAND_REFUSALS")
        .expect("table present");
    let end = PROVIDER_TS[start..].find("};").expect("table end") + start;
    let block = &PROVIDER_TS[start..end];
    let mut handwritten: Vec<(String, Vec<String>)> = Vec::new();
    for cap in block.split('"').collect::<Vec<_>>().chunks(2) {
        if cap.len() < 2 {
            continue;
        }
        let token = cap[1];
        if token.contains('/') {
            handwritten.push((token.to_string(), Vec::new()));
        } else if let Some(last) = handwritten.last_mut() {
            last.1.push(token.to_string());
        }
    }
    for (_, list) in handwritten.iter_mut() {
        list.sort();
    }
    handwritten.sort();
    assert_eq!(handwritten.len(), 9, "fixture parse sanity");
    assert_eq!(generated, handwritten);
}

#[test]
fn play_assets_match_the_handwritten_adapter() {
    let entries = pg::parse_manifest(MANIFEST).expect("manifest parses");
    let file = pg::parse(WIRE).expect("parse");
    let generated = pg::generate_play_assets(&entries, &file.videos).expect("no collisions");
    let golden = include_str!("provider_fixtures/play-assets.ts");
    assert_eq!(generated, golden.trim_end());
}

#[test]
fn schema_lies_are_refused_with_precise_problems() {
    let schema = schema();
    let bad_fn = pg::parse("mutation X { post: post.id } -> call likee_post(post);").unwrap();
    let problems = pg::validate_against(&bad_fn, &schema);
    assert!(
        problems[0].contains("unknown fn `likee_post`"),
        "{problems:?}"
    );

    let bad_allow = pg::parse(
        "mutation X { post: post.id } -> call like_post(post) route feed/x allow cannot_follow_self;",
    )
    .unwrap();
    let problems = pg::validate_against(&bad_allow, &schema);
    assert!(
        problems[0].contains("allows `cannot_follow_self`"),
        "{problems:?}"
    );

    let bad_table = pg::parse("snapshot app { cap 200 per table; read ghosts; }").unwrap();
    let err = pg::generate_snapshot_query(&bad_table, &schema).unwrap_err();
    assert!(err[0].contains("unknown table `ghosts`"), "{err:?}");
}

/// Node 기반 실행 하니스 — 명시 실행 전용 (CI는 node를 요구하지 않는다):
/// `cargo test -p spock-cli --test provider_gen -- --ignored`
#[test]
#[ignore = "requires node"]
fn generated_module_drives_the_shared_runtime_under_node() {
    let file = pg::parse(WIRE).expect("parse");
    let module = pg::generate_provider_module(&file, &schema()).expect("generates");
    let dir = std::env::temp_dir().join("spock-provider-spike");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let module_path = dir.join("provider-tables.mjs");
    std::fs::write(&module_path, module).expect("write module");

    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/provider_runtime");
    let output = std::process::Command::new("node")
        .arg(format!("{base}/runtime-unit.mjs"))
        .arg(&module_path)
        .arg(format!("{base}/runtime.mjs"))
        .output()
        .expect("node must be available for this opt-in check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("runtime ok"), "{stdout}");
}

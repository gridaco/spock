//! Provider-generation spike goldens (uhura#29): every generated artifact is
//! char-equal with the hand-written instagram provider it replaces. Fixtures
//! are mechanically extracted from gridaco/uhura examples (MIT © Grida).

use spock_cli::provider_gen as pg;

const CONTRACT: &str = include_str!("provider_fixtures/contract.json");
const DECLARATION: &str = include_str!("provider_fixtures/instagram.provider");
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
    let file = pg::parse(DECLARATION).expect("parse");
    let problems = pg::validate_against(&file, &schema());
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn machine_contract_types_match_the_handwritten_machine_uhura() {
    let file = pg::parse(DECLARATION).expect("parse");
    let generated = pg::generate_machine_types(&file);
    let golden = include_str!("provider_fixtures/machine-types.uhura");
    assert_eq!(generated.trim(), golden.trim());
}

#[test]
fn view_types_match_the_handwritten_machine_uhura() {
    let file = pg::parse(DECLARATION).expect("parse");
    let generated = pg::generate_view_types(&file, &schema()).expect("views validate");
    let golden = include_str!("provider_fixtures/view-types.uhura");
    assert_eq!(generated.trim(), golden.trim());
}

#[test]
fn snapshot_query_matches_the_handwritten_adapter() {
    let file = pg::parse(DECLARATION).expect("parse");
    let generated = pg::generate_snapshot_query(&file, &schema()).expect("generates");
    let golden = include_str!("provider_fixtures/snapshot-query.graphql");
    assert_eq!(generated, golden);
}

#[test]
fn dispatch_switch_matches_the_handwritten_adapter() {
    let file = pg::parse(DECLARATION).expect("parse");
    let generated = pg::generate_dispatch(&file).expect("generates");
    let golden = include_str!("provider_fixtures/dispatch-switch.ts");
    assert_eq!(generated, golden);
}

#[test]
fn refusal_whitelist_semantically_matches_the_handwritten_table() {
    let file = pg::parse(DECLARATION).expect("parse");
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
    let file = pg::parse(DECLARATION).expect("parse");
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

const MODULE: &str = "app.instagram@1";

/// The key tags the dispatch switch reads are app data: they must be generated
/// module constants, and the runtime contract must stay the four generic helpers.
#[test]
fn key_tags_are_generated_data_and_helpers_stay_app_independent() {
    let file = pg::parse(DECLARATION).expect("parse");
    let module = pg::generate_provider_module(&file, &schema(), MODULE).expect("generates");

    assert!(
        module.contains(&format!("export const MODULE = \"{MODULE}\";")),
        "the module publishes the identity it was generated for"
    );
    for (constant, ty) in [
        ("POST_ID_TYPE", "PostId"),
        ("USER_ID_TYPE", "UserId"),
        ("STORY_ID_TYPE", "StoryId"),
    ] {
        assert!(
            module.contains(&format!("const {constant} = `${{MODULE}}::{ty}`;")),
            "{constant} must be generated, not supplied by the runtime"
        );
    }
    assert!(
        module.contains("const { keyText, requiredField, boolValue, textValue } = helpers;"),
        "helpers carry no app-specific names:\n{module}"
    );
}

/// Uhura declares these id types; the provider tags keys with the same names.
/// One generator keeps them from drifting apart.
#[test]
fn generated_key_tags_agree_with_the_generated_machine_types() {
    let file = pg::parse(DECLARATION).expect("parse");
    let machine = pg::generate_machine_types(&file);
    let module = pg::generate_provider_module(&file, &schema(), MODULE).expect("generates");
    for ty in ["PostId", "UserId", "StoryId"] {
        assert!(machine.contains(ty), "machine types declare {ty}");
        assert!(module.contains(&format!("::{ty}`")), "provider tags {ty}");
    }
}

fn write_module(name: &str, contents: String) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("spock-provider-spike");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write module");
    path
}

fn run_node(script: &str, args: &[&std::path::Path]) -> String {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/provider_runtime");
    let mut command = std::process::Command::new("node");
    command.arg(format!("{base}/{script}"));
    for arg in args {
        command.arg(arg);
    }
    command.arg(format!("{base}/runtime.mjs"));
    let output = command
        .output()
        .expect("node must be available for this opt-in check");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    stdout
}

/// Node-based execution harness. Node is already required to reach a green
/// workspace: the canonical Play artifacts these crates test against are built by
/// `pnpm -C uhura/web build:provider`, so nothing here is optional.
#[test]
fn generated_module_drives_the_shared_runtime_under_node() {
    let file = pg::parse(DECLARATION).expect("parse");
    let module = pg::generate_provider_module(&file, &schema(), MODULE).expect("generates");
    let path = write_module("runtime-tables.mjs", module);
    let stdout = run_node("runtime-unit.mjs", &[&path]);
    assert!(stdout.contains("runtime ok"), "{stdout}");
}

/// The runtime may not name any app, and it must refuse keys that do not carry
/// the Uhura type the generated tables declare.
#[test]
fn the_runtime_is_app_independent_and_checks_key_types() {
    let file = pg::parse(DECLARATION).expect("parse");
    let module = pg::generate_provider_module(&file, &schema(), MODULE).expect("generates");
    let path = write_module("genericity-tables.mjs", module);
    let source = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/provider_runtime/runtime.mjs"
    ));
    let stdout = run_node("genericity.mjs", &[&path, &source]);
    assert!(stdout.contains("genericity ok"), "{stdout}");
}

/// A second app, with its own tables and its own package identity, drives the very
/// same runtime file. This is what makes the runtime shared rather than Instagram's.
#[test]
fn a_second_app_drives_the_same_runtime() {
    const BLOG_CONTRACT: &str = r#"{
      "tables": [
        { "name": "article", "key": ["id"],
          "fields": [
            { "name": "id", "type": { "kind": "uuid" } },
            { "name": "title", "type": { "kind": "text" } }
          ] }
      ],
      "errors": [{ "code": "not_authorized" }],
      "fns": [
        { "name": "publish_article", "errors": ["not_authorized"] },
        { "name": "retract_article", "errors": ["not_authorized"] }
      ]
    }"#;
    const BLOG_DECLARATION: &str = r#"
      app "Blog";
      snapshot app { cap 50 per table; read article; }
      mutation SetPublished { article: article.id, published: bool }
        -> if published call publish_article(article) route desk/publish allow not_authorized
           else call retract_article(article) route desk/retract allow not_authorized;
    "#;

    let schema = pg::extract_contract(BLOG_CONTRACT).expect("contract parses");
    let file = pg::parse(BLOG_DECLARATION).expect("parse");
    assert!(pg::validate_against(&file, &schema).is_empty());
    let module = pg::generate_provider_module(&file, &schema, "app.blog@2").expect("generates");
    assert!(module.contains("const ARTICLE_ID_TYPE = `${MODULE}::ArticleId`;"));

    let path = write_module("blog-tables.mjs", module);
    let stdout = run_node("second-app.mjs", &[&path]);
    assert!(stdout.contains("second app ok"), "{stdout}");
}

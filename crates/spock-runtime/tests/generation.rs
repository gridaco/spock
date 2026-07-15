//! Backend-generation composition boundary: immutable ownership, captured
//! inputs, and authority routes that do not install application-wide policy.

use spock_runtime::generation::{BackendGeneration, CapturedBackend};

async fn serve(router: axum::Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral listener");
    let address = listener.local_addr().expect("listener address");
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve router");
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn authority_router_has_no_global_fallback_or_cors_policy() {
    let generation = BackendGeneration::from_captured(
        CapturedBackend::without_assets("table note { key id: uuid = auto }"),
        None,
    )
    .expect("generation");

    let (authority_base, authority_task) = serve(
        generation
            .authority_router()
            .expect("listenerless authority router"),
    )
    .await;
    let authority_missing = reqwest::Client::new()
        .get(format!("{authority_base}/belongs-to-the-framework"))
        .header("Origin", "http://127.0.0.1:8787")
        .send()
        .await
        .expect("authority request");
    assert_eq!(authority_missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert!(
        authority_missing
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "the embedding host owns CORS"
    );
    assert_eq!(authority_missing.text().await.unwrap(), "");
    assert_eq!(
        reqwest::get(format!("{authority_base}/~health"))
            .await
            .expect("authority health request")
            .status(),
        reqwest::StatusCode::NOT_FOUND,
        "aggregate health belongs to the embedding host"
    );
    authority_task.abort();

    // The compatibility router retains standalone Spock's JSON fallback and
    // permissive CORS; splitting authority routes did not change public v0.
    let (standalone_base, standalone_task) =
        serve(spock_runtime::http::router(generation.app()).expect("standalone router")).await;
    let standalone_missing = reqwest::Client::new()
        .get(format!("{standalone_base}/belongs-to-the-framework"))
        .header("Origin", "http://127.0.0.1:8787")
        .send()
        .await
        .expect("standalone request");
    assert_eq!(standalone_missing.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        standalone_missing.headers()["access-control-allow-origin"],
        "*"
    );
    assert!(standalone_missing
        .text()
        .await
        .expect("fallback body")
        .contains("no such path"));
    assert_eq!(
        reqwest::get(format!("{standalone_base}/~health"))
            .await
            .expect("standalone health")
            .status(),
        reqwest::StatusCode::OK
    );
    standalone_task.abort();
}

#[test]
fn empty_captured_backend_constructs_an_authority_generation() {
    let generation = BackendGeneration::from_captured(CapturedBackend::without_assets(""), None)
        .expect("empty backend is a valid authority generation");

    assert!(generation.contract().tables.is_empty());
    assert!(generation.contract().records.is_empty());
    assert!(generation.contract().fns.is_empty());
}

#[tokio::test]
async fn comment_only_authority_exposes_contract_without_graphql() {
    let generation = BackendGeneration::from_captured(
        CapturedBackend::without_assets("// backend intentionally empty\n"),
        None,
    )
    .expect("comment-only backend is a valid authority generation");
    let (base, task) = serve(
        generation
            .authority_router()
            .expect("empty authority router"),
    )
    .await;

    let graphql = reqwest::get(format!("{base}/graphql/v1"))
        .await
        .expect("GraphQL path request");
    assert_eq!(graphql.status(), reqwest::StatusCode::NOT_FOUND);
    assert_eq!(
        graphql.text().await.expect("unclaimed response body"),
        "",
        "the embedding host, not Spock, owns structured fallback"
    );

    let contract = reqwest::get(format!("{base}/~contract"))
        .await
        .expect("contract request");
    assert_eq!(contract.status(), reqwest::StatusCode::OK);
    let contract: serde_json::Value = contract.json().await.expect("contract JSON");
    assert_eq!(contract["spock"], "v0");
    assert_eq!(contract["tables"], serde_json::json!([]));
    assert_eq!(contract["fns"], serde_json::json!([]));

    task.abort();
}

#[test]
fn generation_owns_a_stable_contract_identity() {
    let generation = BackendGeneration::from_captured(
        CapturedBackend::without_assets("table note { key id: uuid = auto }"),
        None,
    )
    .expect("generation");

    assert_eq!(generation.contract().tables[0].name, "note");
    assert_eq!(generation.contract_fingerprint().as_str().len(), 64);
    assert_eq!(generation.input_fingerprint().unwrap().as_str().len(), 64);

    // Consumers share the exact owned App rather than reconstructing runtime
    // state for each router or request.
    let first = generation.app();
    let second = generation.app();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

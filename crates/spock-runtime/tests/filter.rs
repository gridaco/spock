//! The filter sub-language (RFD 0021), exercised end to end against the
//! technical fixture `examples/filter-lab/schema.spock`. Where the graphql/http
//! suites prove the surface exists, this suite probes its *edges* — ordering
//! ties, NULL placement, `%`/`_` escaping, ASCII-only case folding, closed-set
//! membership, the NULL law, and the keyset-skip the RFD deliberately does not
//! paper over. Findings are written up in examples/filter-lab/FEEDBACK.md.

use std::sync::Arc;

use serde_json::{json, Value};
use spock_runtime::{engine, http, App};

const SCHEMA: &str = include_str!("../../../examples/filter-lab/schema.spock");

async fn start() -> String {
    let contract = spock_lang::compile(SCHEMA).expect("filter-lab compiles");
    let conn = engine::open(&contract, None, None).expect("engine opens and seeds");
    let app = Arc::new(App::new(contract, conn));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { http::serve(app, listener).await.expect("serve") });
    format!("http://{addr}")
}

/// REST GET → (status, body).
async fn get(base: &str, path: &str) -> (u16, Value) {
    let resp = reqwest::get(format!("{base}{path}")).await.expect("GET");
    (
        resp.status().as_u16(),
        resp.json().await.unwrap_or(Value::Null),
    )
}

/// The `rows` of a REST list response.
fn rest_rows(body: &Value) -> Vec<Value> {
    body["rows"].as_array().cloned().unwrap_or_default()
}

/// The labels of a REST list response, in order.
fn rest_labels(body: &Value) -> Vec<String> {
    rest_rows(body)
        .iter()
        .map(|r| r["label"].as_str().unwrap().to_string())
        .collect()
}

async fn gql(base: &str, query: &str) -> Value {
    reqwest::Client::new()
        .post(format!("{base}/graphql/v1"))
        .json(&json!({ "query": query }))
        .send()
        .await
        .expect("POST graphql")
        .json()
        .await
        .expect("json")
}

fn gql_labels(resp: &Value, field: &str) -> Vec<String> {
    resp["data"][field]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["label"].as_str().unwrap().to_string())
        .collect()
}

/// Every scalar shape filters through both frontends to the same result.
#[tokio::test]
async fn scalar_operators() {
    let base = start().await;

    // int: gt / lte / in
    let (_, b) = get(&base, "/rest/v1/widget?rank=gt.1").await;
    assert_eq!(rest_rows(&b).len(), 2); // d, e
    let (_, b) = get(&base, "/rest/v1/widget?rank=lte.1").await;
    assert_eq!(rest_rows(&b).len(), 3); // a, b, c
    let (_, b) = get(&base, "/rest/v1/widget?rank=in.(2,3)").await;
    assert_eq!(rest_rows(&b).len(), 2); // d, e

    // float: gt
    let (_, b) = get(&base, "/rest/v1/widget?score=gt.2.0").await;
    assert_eq!(rest_rows(&b).len(), 3); // 2.5, 3.5, 10.0

    // bool: is.true (REST) and _eq: true (GraphQL) agree
    let (_, b) = get(&base, "/rest/v1/widget?active=is.true").await;
    assert_eq!(rest_rows(&b).len(), 3); // a, c, e
    let r = gql(&base, "{ widget(where: {active: {_eq: true}}) { label } }").await;
    assert_eq!(gql_labels(&r, "widget").len(), 3);

    // is.null / not.is.null (b has no note)
    let (_, b) = get(&base, "/rest/v1/widget?note=is.null").await;
    assert_eq!(rest_labels(&b), vec!["Alpha Two"]);
    let (_, b) = get(&base, "/rest/v1/widget?note=not.is.null").await;
    assert_eq!(rest_rows(&b).len(), 4);

    // timestamp range over canonical (lexically sortable) storage
    let (_, b) = get(&base, "/rest/v1/widget?made_at=gte.2026-01-03T00:00:00Z").await;
    assert_eq!(rest_rows(&b).len(), 3); // c, d, e
}

/// Closed sets filter as strings; an off-member operand fails loudly.
#[tokio::test]
async fn closed_set_membership() {
    let base = start().await;

    let (_, b) = get(&base, "/rest/v1/widget?kind=eq.alpha").await;
    assert_eq!(rest_rows(&b).len(), 2); // a, b
    let (_, b) = get(&base, "/rest/v1/widget?kind=in.(beta,gamma)").await;
    assert_eq!(rest_rows(&b).len(), 3); // c, d, e

    // an off-set value is a type_mismatch (422), not a silent empty result
    let (status, b) = get(&base, "/rest/v1/widget?kind=eq.delta").await;
    assert_eq!(
        (status, b["error"]["code"].as_str().unwrap()),
        (422, "type_mismatch")
    );
    let r = gql(
        &base,
        r#"{ widget(where: {kind: {_eq: "delta"}}) { label } }"#,
    )
    .await;
    assert_eq!(r["errors"][0]["extensions"]["code"], "type_mismatch");
}

/// FINDING territory: `ilike` is ASCII-case-insensitive and treats `%`/`_` as
/// wildcards with no client-reachable escape. These assertions pin the exact
/// behavior recorded in FEEDBACK.md.
#[tokio::test]
async fn ilike_edges() {
    let base = start().await;

    // case-insensitive across ASCII: matches "alpha one" and "Alpha Two"
    let (_, b) = get(&base, "/rest/v1/widget?label=ilike.*alpha*").await;
    assert_eq!(rest_rows(&b).len(), 2);

    // `*` aliases `%`; "50% off" matched by a suffix pattern
    let (_, b) = get(&base, "/rest/v1/widget?label=ilike.*off").await;
    assert_eq!(rest_labels(&b), vec!["50% off"]);

    // ASCII-only case folding: "café" matches lowercase-exact, but NOT the
    // uppercased non-ASCII form — É is not folded to é (FEEDBACK F3).
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_ilike: "café"}}) { label } }"#,
    )
    .await;
    assert_eq!(gql_labels(&r, "widget"), vec!["café"]);
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_ilike: "CAFÉ"}}) { label } }"#,
    )
    .await;
    assert_eq!(r["data"]["widget"].as_array().unwrap().len(), 0);
    // the ASCII head still folds: CAF% matches café
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_ilike: "CAF%"}}) { label } }"#,
    )
    .await;
    assert_eq!(gql_labels(&r, "widget"), vec!["café"]);
}

/// The forced stable total order (RFD 0021 §7): three rank-1 rows tie, yet
/// paging never skips or duplicates because the pk is appended as a tiebreak.
#[tokio::test]
async fn ordering_is_stable_across_ties() {
    let base = start().await;

    // the full order by rank asc — the tie (a,b,c) resolves by pk
    let (_, all) = get(&base, "/rest/v1/widget?order=rank.asc").await;
    let full: Vec<String> = rest_rows(&all)
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(full.len(), 5);

    // paging in windows of 2 reconstructs the exact same order — no gaps, no
    // repeats, even though rank alone is ambiguous across the first three rows
    let mut paged = Vec::new();
    for offset in [0, 2, 4] {
        let (_, page) = get(
            &base,
            &format!("/rest/v1/widget?order=rank.asc&limit=2&offset={offset}"),
        )
        .await;
        for r in rest_rows(&page) {
            paged.push(r["id"].as_str().unwrap().to_string());
        }
    }
    assert_eq!(paged, full, "paged order diverged from the full order");
}

/// NULLS placement is explicit and Postgres-shaped: last on asc, first on desc
/// (SQLite's implicit default is the opposite and is never inherited).
#[tokio::test]
async fn nulls_sort_explicitly() {
    let base = start().await;

    // b has a null note; asc → NULLS LAST
    let (_, b) = get(&base, "/rest/v1/widget?order=note.asc").await;
    assert_eq!(rest_labels(&b).last().unwrap(), "Alpha Two");
    // desc → NULLS FIRST
    let (_, b) = get(&base, "/rest/v1/widget?order=note.desc").await;
    assert_eq!(rest_labels(&b).first().unwrap(), "Alpha Two");
}

/// FINDING: three-valued logic. `_neq` excludes NULL rows (NULL <> x is NULL,
/// not true), so a row with a null note is absent from `note != "first"` —
/// SQL-faithful, and a genuine footgun worth stating (FEEDBACK F2).
#[tokio::test]
async fn neq_excludes_nulls() {
    let base = start().await;
    let (_, b) = get(&base, "/rest/v1/widget?note=neq.first").await;
    let labels = rest_labels(&b);
    assert_eq!(labels.len(), 3); // c, d, e — NOT a ("first") and NOT b (null)
    assert!(
        !labels.contains(&"Alpha Two".to_string()),
        "null-note row leaked in"
    );
}

/// References filter by their key (folded to the FK column, no EXISTS), and
/// the reverse collection carries its own filter surface.
#[tokio::test]
async fn references_and_reverse() {
    let base = start().await;

    // a's id, then widgets whose parent is a
    let (_, all) = get(&base, "/rest/v1/widget?label=eq.alpha%20one").await;
    let a_id = rest_rows(&all)[0]["id"].as_str().unwrap().to_string();
    let (_, b) = get(&base, &format!("/rest/v1/widget?parent=eq.{a_id}")).await;
    assert_eq!(rest_rows(&b).len(), 2); // d, e

    // the same, GraphQL, via the reverse collection with its own order_by
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_eq: "alpha one"}}) { widget_by_parent(order_by: {rank: asc}) { label } } }"#,
    )
    .await;
    let kids: Vec<&str> = r["data"]["widget"][0]["widget_by_parent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["label"].as_str().unwrap())
        .collect();
    assert_eq!(kids, vec!["50% off", "café"]); // rank 2, then 3
}

/// A composite-key table filters and orders through the same composer; the
/// forced total order falls back to the whole compound key.
#[tokio::test]
async fn composite_key_table() {
    let base = start().await;
    let (_, b) = get(&base, "/rest/v1/edge?weight=gt.4").await;
    assert_eq!(rest_rows(&b).len(), 2); // (a,b)=5, (b,c)=8
                                        // listable and stably ordered even with no single key
    let (status, b) = get(&base, "/rest/v1/edge?order=weight.desc").await;
    assert_eq!(status, 200);
    let weights: Vec<i64> = rest_rows(&b)
        .iter()
        .map(|e| e["weight"].as_i64().unwrap())
        .collect();
    assert_eq!(weights, vec![8, 5, 3]);
}

/// The refusals: the NULL law, `like`, cross-table traversal, the depth
/// ceiling — each a caller-shaped error, never a 500.
#[tokio::test]
async fn refusals() {
    let base = start().await;

    // the NULL law: `_eq: null` is refused (use `_is_null`)
    let r = gql(&base, "{ widget(where: {note: {_eq: null}}) { label } }").await;
    assert_eq!(r["errors"][0]["extensions"]["code"], "bad_request");

    // `like` (case-sensitive) is refused with a hint toward `ilike`
    let (status, b) = get(&base, "/rest/v1/widget?label=like.alpha").await;
    assert_eq!(
        (status, b["error"]["code"].as_str().unwrap()),
        (400, "bad_request")
    );

    // reserved cross-table traversal (§5): a non-key sub-field on a ref
    let r = gql(
        &base,
        r#"{ widget(where: {parent: {label: {_eq: "alpha one"}}}) { label } }"#,
    )
    .await;
    assert_eq!(r["errors"][0]["extensions"]["code"], "bad_request");

    // the offset depth ceiling (§7)
    let (status, b) = get(&base, "/rest/v1/widget?offset=99999").await;
    assert_eq!(
        (status, b["error"]["code"].as_str().unwrap()),
        (400, "bad_request")
    );
    let r = gql(&base, "{ widget(offset: 99999) { label } }").await;
    assert_eq!(r["errors"][0]["extensions"]["code"], "bad_request");

    // `ilike` on a non-text column is refused (mirrors the GraphQL side, where
    // `_ilike` exists only on `String`) — SQLite would otherwise coerce silently
    let (status, b) = get(&base, "/rest/v1/widget?rank=ilike.5*").await;
    assert_eq!(
        (status, b["error"]["code"].as_str().unwrap()),
        (422, "type_mismatch")
    );

    // deeply nested logical groups are refused before they can overflow the
    // stack (§8.7) — 40 levels is past the depth ceiling of 32
    let mut nested = "rank.gt.0".to_string();
    for _ in 0..40 {
        nested = format!("and({nested})");
    }
    let (status, b) = get(&base, &format!("/rest/v1/widget?and=({nested})")).await;
    assert_eq!(
        (status, b["error"]["code"].as_str().unwrap()),
        (400, "bad_request")
    );

    // a multi-key `order_by` object is refused: `serde_json::Map` would sort the
    // keys and silently reorder terms — the list form carries multiple terms
    let r = gql(
        &base,
        "{ widget(order_by: {rank: asc, score: desc}) { label } }",
    )
    .await;
    assert_eq!(r["errors"][0]["extensions"]["code"], "bad_request");
}

/// FINDING (the honest one): a bare `_gt` keyset over a non-unique sort column
/// silently skips tie rows — exactly the G16 defect the RFD refuses to hide by
/// NOT productizing keyset in v0 (FEEDBACK F1). This test *pins the footgun* so
/// nobody mistakes it for a supported cursor.
#[tokio::test]
async fn naive_keyset_skips_ties() {
    let base = start().await;

    // page 1: order by rank asc, take 2 → the first two rank-1 rows
    let (_, p1) = get(&base, "/rest/v1/widget?order=rank.asc&limit=2").await;
    let page1 = rest_rows(&p1);
    assert_eq!(page1.len(), 2);
    let last_rank = page1[1]["rank"].as_i64().unwrap(); // == 1

    // the "obvious" keyset for page 2: rank > last_rank. It DROPS the third
    // rank-1 row entirely — the row is never returned by any such page.
    let (_, p2) = get(
        &base,
        &format!("/rest/v1/widget?rank=gt.{last_rank}&order=rank.asc"),
    )
    .await;
    let page2_ranks: Vec<i64> = rest_rows(&p2)
        .iter()
        .map(|r| r["rank"].as_i64().unwrap())
        .collect();
    assert!(
        !page2_ranks.contains(&1),
        "naive keyset unexpectedly kept a tie row: {page2_ranks:?}"
    );
    // total rows a client would see across the two "pages" is 4, not 5 — one
    // rank-1 row is lost. This is why v0 ships offset + a forced order and
    // defers a real keyset cursor.
    assert_eq!(page1.len() + rest_rows(&p2).len(), 4);
}

/// Verifies the factual claims made in FEEDBACK.md so the writeup can't drift
/// from behavior: ref-nullness folds to `IS NULL` (F5), the `\` escape reaches
/// a literal `%` (F4), and a closed-set column tolerates (meaningless) ordered
/// operators (F6).
#[tokio::test]
async fn feedback_claims_hold() {
    let base = start().await;

    // F5: nullness of a reference, both terse (REST) and nested (GraphQL)
    let (_, b) = get(&base, "/rest/v1/widget?parent=is.null").await;
    assert_eq!(rest_rows(&b).len(), 3); // a, b, c have no parent
    let r = gql(
        &base,
        "{ widget(where: {parent: {id: {_is_null: true}}}) { label } }",
    )
    .await;
    assert_no_gql_errors(&r);
    assert_eq!(r["data"]["widget"].as_array().unwrap().len(), 3);

    // F4: the `\` escape reaches a literal `%` — "50% off" matched exactly
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_ilike: "50\\% off"}}) { label } }"#,
    )
    .await;
    assert_no_gql_errors(&r);
    assert_eq!(gql_labels(&r, "widget"), vec!["50% off"]);
    // ...while an unescaped `%` is a wildcard: "50%" matches the same row via prefix
    let r = gql(
        &base,
        r#"{ widget(where: {label: {_ilike: "50%"}}) { label } }"#,
    )
    .await;
    assert_eq!(gql_labels(&r, "widget"), vec!["50% off"]);

    // F6: a closed-set column accepts ordered ops (lexical, meaningless, but
    // defined and non-erroring)
    let (status, _) = get(&base, "/rest/v1/widget?kind=gt.alpha").await;
    assert_eq!(status, 200);
}

fn assert_no_gql_errors(resp: &Value) {
    assert!(
        resp["errors"].is_null(),
        "unexpected errors: {}",
        resp["errors"]
    );
}

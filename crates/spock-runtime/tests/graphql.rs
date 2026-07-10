//! End-to-end GraphQL verification (docs/spec/v0.md §8.2): compile a real
//! program, serve it, and exercise the derived schema over actual HTTP —
//! introspection, list/by-key roots, forward and reverse nesting, limits,
//! null-for-miss, and GraphiQL.

use std::sync::Arc;

use serde_json::{json, Value};
use spock_runtime::{engine, http, App};

const PROGRAM: &str = r#"
table user {
  key id: uuid = auto
  username: text unique
  bio: text?
  invited_by: user?
  joined_at: timestamp = now
}

table post {
  key id: uuid = auto
  author: user
  caption: text?
  published_at: timestamp = now
}

table follow {
  key (follower, target)
  follower: user
  target: user
  since: timestamp = now
}

table comment {
  key id: uuid = auto
  post: post on delete cascade
  author: user
  body: text
  at: timestamp = now
}

seed {
  maya = user { username: "maya", bio: "photographer" }
  luis = user { username: "luis", invited_by: maya }

  p1 = post { author: maya, caption: "first light" }
  p2 = post { author: maya, caption: "golden hour" }

  follow { follower: luis, target: maya }
  comment { post: p1, author: luis, body: "great shot" }
}
"#;

async fn start() -> String {
    let contract = spock_lang::compile(PROGRAM).expect("program compiles");
    let conn = engine::open(&contract, None).expect("engine opens and seeds");
    let app = Arc::new(App::new(contract, conn));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        http::serve(app, listener).await.expect("serve");
    });
    format!("http://{addr}")
}

/// POST a GraphQL query (with optional variables); return the response JSON.
async fn gql(base: &str, query: &str, variables: Value) -> Value {
    let mut body = json!({ "query": query });
    if !variables.is_null() {
        body["variables"] = variables;
    }
    reqwest::Client::new()
        .post(format!("{base}/graphql/v1"))
        .json(&body)
        .send()
        .await
        .expect("POST /graphql/v1")
        .json()
        .await
        .expect("json response")
}

fn assert_no_errors(response: &Value) {
    assert!(
        response["errors"].is_null(),
        "unexpected errors: {}",
        response["errors"]
    );
}

#[tokio::test]
async fn the_graphql_surface() {
    let base = start().await;

    // -- introspection: derived types visible, no mutations ---------------
    let resp = gql(
        &base,
        "{ __schema { queryType { name } mutationType { name } types { name } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["__schema"]["queryType"]["name"], "Query");
    assert!(resp["data"]["__schema"]["mutationType"].is_null());
    let types: Vec<&str> = resp["data"]["__schema"]["types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in ["User", "Post", "Follow", "Comment", "UUID", "Timestamp"] {
        assert!(types.contains(&expected), "missing type {expected}");
    }

    // -- root list ---------------------------------------------------------
    let resp = gql(&base, "{ user_list { username bio } }", Value::Null).await;
    assert_no_errors(&resp);
    let users = resp["data"]["user_list"].as_array().unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users.iter().filter(|u| u["username"] == "maya").count(), 1);

    // -- forward ref nesting ------------------------------------------------
    let resp = gql(
        &base,
        "{ post_list { caption author { username } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let posts = resp["data"]["post_list"].as_array().unwrap();
    assert_eq!(posts.len(), 2);
    assert!(posts.iter().all(|p| p["author"]["username"] == "maya"));

    // -- reverse collection nesting (the canonical pitch query) -------------
    let resp = gql(
        &base,
        "{ user_list { username post_author_list { caption comment_post_list { body } } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user_list"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let posts = maya["post_author_list"].as_array().unwrap();
    assert_eq!(posts.len(), 2);
    let first_light = posts
        .iter()
        .find(|p| p["caption"] == "first light")
        .unwrap();
    assert_eq!(first_light["comment_post_list"][0]["body"], "great shot");
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert_eq!(luis["post_author_list"].as_array().unwrap().len(), 0);

    // -- both follow directions exist and are distinct ----------------------
    let resp = gql(
        &base,
        "{ user_list { username follow_follower_list { since } follow_target_list { since } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user_list"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert_eq!(maya["follow_target_list"].as_array().unwrap().len(), 1); // luis follows maya
    assert_eq!(maya["follow_follower_list"].as_array().unwrap().len(), 0);
    assert_eq!(luis["follow_follower_list"].as_array().unwrap().len(), 1);
    assert_eq!(luis["follow_target_list"].as_array().unwrap().len(), 0);

    // -- optional self-ref: object or null -----------------------------------
    let resp = gql(
        &base,
        "{ user_list { username invited_by { username } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user_list"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert!(maya["invited_by"].is_null());
    assert_eq!(luis["invited_by"]["username"], "maya");

    // -- by-key root: hit via variables, misses are null --------------------
    let resp = gql(&base, "{ user_list { id username } }", Value::Null).await;
    let users = resp["data"]["user_list"].as_array().unwrap();
    let maya_id = users.iter().find(|u| u["username"] == "maya").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let query = "query($id: UUID!) { user(id: $id) { username } }";
    let resp = gql(&base, query, json!({ "id": maya_id })).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["user"]["username"], "maya");

    let resp = gql(
        &base,
        query,
        json!({ "id": uuid::Uuid::now_v7().to_string() }),
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["user"].is_null());

    let resp = gql(&base, query, json!({ "id": "not-a-uuid" })).await;
    assert_no_errors(&resp);
    assert!(resp["data"]["user"].is_null()); // malformed key matches no row

    // -- limits: default is applied, cap clamps, negative errors ------------
    let resp = gql(&base, "{ post_list(limit: 1) { caption } }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["post_list"].as_array().unwrap().len(), 1);

    let resp = gql(
        &base,
        "{ post_list(limit: 99999) { caption } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp); // clamped, not an error
    assert_eq!(resp["data"]["post_list"].as_array().unwrap().len(), 2);

    let resp = gql(&base, "{ post_list(limit: -1) { caption } }", Value::Null).await;
    assert!(!resp["errors"].is_null());

    // -- composite-key table: listable, no by-key root ------------------------
    let resp = gql(&base, "{ follow_list { since } }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["follow_list"].as_array().unwrap().len(), 1);

    let resp = gql(&base, "{ follow { since } }", Value::Null).await;
    assert!(!resp["errors"].is_null()); // validation error, HTTP 200

    // -- GraphiQL on GET -------------------------------------------------------
    let resp = reqwest::get(format!("{base}/graphql/v1")).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(content_type.starts_with("text/html"));
    let body = resp.text().await.unwrap();
    assert!(body.to_lowercase().contains("graphiql"));
}

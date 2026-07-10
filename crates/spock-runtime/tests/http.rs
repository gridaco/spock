//! End-to-end protocol verification (docs/spec/v0.md §8): compile a real
//! program, materialize it, serve it on a real TCP port, and exercise every
//! endpoint and every derived-error class over actual HTTP.

use std::sync::Arc;

use serde_json::{json, Value};
use spock_runtime::{engine, http, App};

const PROGRAM: &str = r#"
// the v0-expressible instagram slice
table user {
  key id: uuid = auto
  username: text unique
  bio: text?
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
  luis = user { username: "luis" }

  p1 = post { author: maya, caption: "first light" }

  follow { follower: luis, target: maya }
  comment { post: p1, author: luis, body: "great shot" }
}
"#;

/// Compile, materialize, serve on an ephemeral port; return the base URL.
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

async fn get(base: &str, path: &str) -> (u16, Value) {
    let resp = reqwest::get(format!("{base}{path}")).await.expect("GET");
    let status = resp.status().as_u16();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

async fn post(base: &str, path: &str, body: Value) -> (u16, Value) {
    let resp = reqwest::Client::new()
        .post(format!("{base}{path}"))
        .json(&body)
        .send()
        .await
        .expect("POST");
    let status = resp.status().as_u16();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

async fn delete(base: &str, path: &str) -> u16 {
    reqwest::Client::new()
        .delete(format!("{base}{path}"))
        .send()
        .await
        .expect("DELETE")
        .status()
        .as_u16()
}

fn error_code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or("<no code>")
}

#[tokio::test]
async fn the_whole_protocol() {
    let base = start().await;

    // -- meta surface ---------------------------------------------------
    let (status, health) = get(&base, "/~health").await;
    assert_eq!((status, health["ok"].as_bool()), (200, Some(true)));

    let (status, contract) = get(&base, "/~contract").await;
    assert_eq!(status, 200);
    assert_eq!(contract["spock"], "v0");
    assert_eq!(contract["tables"].as_array().unwrap().len(), 4);
    // derived errors are visible in the contract before any request (§6.1)
    let user_errors: Vec<&str> = contract["tables"][0]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap())
        .collect();
    assert!(user_errors.contains(&"user_username_taken"));
    assert!(user_errors.contains(&"user_username_required"));
    assert!(user_errors.contains(&"user_restricted"));

    // -- open reads: the seed is visible --------------------------------
    let (status, users) = get(&base, "/rest/v1/user").await;
    assert_eq!(status, 200);
    let rows = users["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let maya = rows
        .iter()
        .find(|r| r["username"] == "maya")
        .expect("maya seeded");
    assert_eq!(maya["bio"], "photographer");
    let maya_id = maya["id"].as_str().unwrap().to_string();

    // seed rows went through the write path: defaults were applied
    assert!(maya["joined_at"].as_str().unwrap().contains('T'));

    // ref integrity across the seed: the post's author is maya's uuid
    let (_, posts) = get(&base, "/rest/v1/post").await;
    assert_eq!(posts["rows"][0]["author"], json!(maya_id));

    // by-key read
    let (status, one) = get(&base, &format!("/rest/v1/user/{maya_id}")).await;
    assert_eq!(status, 200);
    assert_eq!(one["username"], "maya");

    // -- 404s ------------------------------------------------------------
    let (status, body) = get(&base, "/rest/v1/nonexistent").await;
    assert_eq!((status, error_code(&body)), (404, "not_found"));
    // tables are NOT served at root: the root namespace is protocol-owned
    let (status, _) = get(&base, "/user").await;
    assert_eq!(status, 404);
    let missing = uuid::Uuid::now_v7();
    let (status, _) = get(&base, &format!("/rest/v1/user/{missing}")).await;
    assert_eq!(status, 404);
    let (status, _) = get(&base, "/rest/v1/user/not-even-a-uuid").await;
    assert_eq!(status, 404);

    // composite-key table is listable but not key-addressable (§8)
    let (status, follows) = get(&base, "/rest/v1/follow").await;
    assert_eq!(status, 200);
    assert_eq!(follows["rows"].as_array().unwrap().len(), 1);
    let (status, body) = get(&base, "/rest/v1/follow/whatever").await;
    assert_eq!((status, error_code(&body)), (400, "bad_request"));

    // -- limit cap (§8): protocol default, not per-table syntax ----------
    let (status, body) = get(&base, "/rest/v1/user?limit=1").await;
    assert_eq!(status, 200);
    assert_eq!(body["rows"].as_array().unwrap().len(), 1);
    let (status, _) = get(&base, "/rest/v1/user?limit=99999").await; // clamped, not an error
    assert_eq!(status, 200);
    let (status, body) = get(&base, "/rest/v1/user?limit=abc").await;
    assert_eq!((status, error_code(&body)), (400, "bad_request"));

    // -- dev-surface writes: the happy path ------------------------------
    let (status, vera) = post(&base, "/~dev/user", json!({ "username": "vera" })).await;
    assert_eq!(status, 201);
    assert_eq!(vera["username"], "vera");
    assert_eq!(vera["bio"], Value::Null); // optional → null
    let vera_id = vera["id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&vera_id).is_ok()); // auto → uuidv7

    // -- derived error: unique (409, code from the contract) -------------
    let (status, body) = post(&base, "/~dev/user", json!({ "username": "maya" })).await;
    assert_eq!((status, error_code(&body)), (409, "user_username_taken"));
    assert_eq!(body["error"]["kind"], "unique");
    assert_eq!(body["error"]["fields"], json!(["username"]));

    // -- derived error: required (422) ------------------------------------
    let (status, body) = post(&base, "/~dev/user", json!({ "bio": "no name" })).await;
    assert_eq!((status, error_code(&body)), (422, "user_username_required"));

    // null is absence (§5.1): a null required field is the same error
    let (status, body) = post(&base, "/~dev/user", json!({ "username": null })).await;
    assert_eq!((status, error_code(&body)), (422, "user_username_required"));

    // -- derived error: ref_not_found (422) -------------------------------
    let (status, body) = post(
        &base,
        "/~dev/post",
        json!({ "author": missing.to_string(), "caption": "ghost" }),
    )
    .await;
    assert_eq!((status, error_code(&body)), (422, "post_author_not_found"));

    // -- derived error: composite key conflict (409) ----------------------
    let (_, users) = get(&base, "/rest/v1/user").await;
    let luis_id = users["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["username"] == "luis")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let follow_body = json!({ "follower": luis_id, "target": maya_id });
    let (status, body) = post(&base, "/~dev/follow", follow_body.clone()).await;
    assert_eq!((status, error_code(&body)), (409, "follow_already_exists"));
    assert_eq!(body["error"]["kind"], "key");

    // -- validation: unknown field / type mismatch / malformed body -------
    let (status, body) = post(
        &base,
        "/~dev/user",
        json!({ "username": "x", "admin": true }),
    )
    .await;
    assert_eq!((status, error_code(&body)), (422, "unknown_field"));
    let (status, body) = post(&base, "/~dev/user", json!({ "username": 42 })).await;
    assert_eq!((status, error_code(&body)), (422, "type_mismatch"));
    let resp = reqwest::Client::new()
        .post(format!("{base}/~dev/user"))
        .body("not json")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400);

    // -- delete: restrict blocks, then unblocks ---------------------------
    // vera has nothing pointing at her: deletable
    assert_eq!(delete(&base, &format!("/~dev/user/{vera_id}")).await, 204);
    let (status, _) = get(&base, &format!("/rest/v1/user/{vera_id}")).await;
    assert_eq!(status, 404);

    // maya is restrict-referenced (post.author, follow.target): 409
    let resp = reqwest::Client::new()
        .delete(format!("{base}/~dev/user/{maya_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 409);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(error_code(&body), "user_restricted");

    // -- delete: cascade flows -------------------------------------------
    // deleting the post cascades its comment (comment.post is cascade)
    let post_id = posts["rows"][0]["id"].as_str().unwrap().to_string();
    let (_, comments) = get(&base, "/rest/v1/comment").await;
    assert_eq!(comments["rows"].as_array().unwrap().len(), 1);
    assert_eq!(delete(&base, &format!("/~dev/post/{post_id}")).await, 204);
    let (_, comments) = get(&base, "/rest/v1/comment").await;
    assert_eq!(comments["rows"].as_array().unwrap().len(), 0);

    // idempotence of the check: deleting again is a 404
    assert_eq!(delete(&base, &format!("/~dev/post/{post_id}")).await, 404);
}

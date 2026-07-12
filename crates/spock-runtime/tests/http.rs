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

record stats { posts: int }

fn find_user(username: text) -> user? {
  unchecked sql("""SELECT * FROM user WHERE username = :username""")
}

mut fn rename_user(user: user, username: text) -> user ! user_username_taken {
  unchecked sql("""UPDATE user SET username = :username WHERE id = :user RETURNING *""")
}

fn author_stats(author: user) -> stats {
  unchecked sql("""SELECT count(*) AS posts FROM post WHERE author = :author""")
}

fn recent_posts(n: int) -> [post] {
  unchecked sql("""SELECT * FROM post ORDER BY published_at DESC LIMIT :n""")
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
    start_program(PROGRAM).await
}

async fn get(base: &str, path: &str) -> (u16, Value) {
    let resp = reqwest::get(format!("{base}{path}")).await.expect("GET");
    let status = resp.status().as_u16();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
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

    // -- the dev surface is retired: REST tables are read-only (§8, §9) --
    let resp = reqwest::Client::new()
        .post(format!("{base}/~dev/user"))
        .json(&json!({ "username": "vera" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 404); // table writes live on /graphql/v1
}

async fn rpc(base: &str, name: &str, body: Option<Value>) -> (u16, Value) {
    let mut req = reqwest::Client::new().post(format!("{base}/rest/v1/rpc/{name}"));
    if let Some(body) = body {
        req = req.json(&body);
    }
    let resp = req.send().await.expect("POST rpc");
    let status = resp.status().as_u16();
    let body = resp.json::<Value>().await.unwrap_or(Value::Null);
    (status, body)
}

// --- the actor seam (RFD 0014): a separate program with an `auth` anchor ---

const ACTOR_PROGRAM: &str = r#"
auth table account {
  key handle: text
  name: text?
}
table note {
  key id: uuid = auto
  owner: account = me
  body: text
}
seed {
  account { handle: "maya", name: "Maya Chen" }
  account { handle: "luis" }
}
"#;

// a uuid-keyed anchor with a unique text label — the instagram shape, where
// the picker's label (username) is a *different* value from the header key (id)
const PERSONA_PROGRAM: &str = r#"
auth table user {
  key id: uuid = auto
  username: text unique
  bio: text?
}
seed {
  maya = user { username: "maya", bio: "photographer" }
  luis = user { username: "luis" }
  noor = user { username: "noor" }
}
"#;

async fn start_program(program: &str) -> String {
    let contract = spock_lang::compile(program).expect("program compiles");
    let conn = engine::open(&contract, None, None).expect("engine opens and seeds");
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

async fn gql_as(base: &str, actor: Option<&str>, query: &str) -> Value {
    let mut req = reqwest::Client::new()
        .post(format!("{base}/graphql/v1"))
        .json(&json!({ "query": query }));
    if let Some(a) = actor {
        req = req.header("X-Spock-Actor", a);
    }
    let resp = req.send().await.expect("POST graphql");
    resp.json::<Value>().await.unwrap_or(Value::Null)
}

#[tokio::test]
async fn me_default_stamps_the_actor_on_the_floor() {
    let base = start_program(ACTOR_PROGRAM).await;
    // owner is a reference, exposed as the `account` object (graphql.md D5)
    let insert = r#"mutation { insert_note_one(object: {body: "hi"}) { owner { handle } body } }"#;

    // impersonated → owner auto-stamped from the header, absent from the input
    let body = gql_as(&base, Some("maya"), insert).await;
    assert_eq!(
        body["data"]["insert_note_one"]["owner"]["handle"], "maya",
        "{body}"
    );
    assert_eq!(body["data"]["insert_note_one"]["body"], "hi");

    // anonymous → the derived `required` error, not a raw 500 (§14.4)
    let body = gql_as(&base, None, insert).await;
    assert_eq!(
        body["errors"][0]["extensions"]["code"], "note_owner_required",
        "{body}"
    );

    // the `owner` field is off the client insert surface — a client cannot
    // forge it (removed from note_insert_input; async-graphql rejects it)
    let forge = r#"mutation { insert_note_one(object: {owner: "luis", body: "x"}) { id } }"#;
    let body = gql_as(&base, Some("maya"), forge).await;
    let msg = body["errors"][0]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("owner"),
        "expected owner rejected as unknown input, got {body}"
    );
}

#[tokio::test]
async fn the_rpc_surface() {
    let base = start().await;

    let (_, users) = get(&base, "/rest/v1/user").await;
    let rows = users["rows"].as_array().unwrap();
    let id_of = |name: &str| {
        rows.iter().find(|r| r["username"] == name).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (maya_id, luis_id) = (id_of("maya"), id_of("luis"));

    // -- maybe arity: hit is the row, miss is null ------------------------
    let (status, body) = rpc(&base, "find_user", Some(json!({"username": "maya"}))).await;
    assert_eq!(status, 200);
    assert_eq!(body["bio"], "photographer");
    let (status, body) = rpc(&base, "find_user", Some(json!({"username": "ghost"}))).await;
    assert_eq!(status, 200);
    assert!(body.is_null());

    // -- record return ------------------------------------------------------
    let (status, body) = rpc(&base, "author_stats", Some(json!({"author": maya_id}))).await;
    assert_eq!(status, 200);
    assert_eq!(body["posts"], 1);

    // -- many arity: the REST list envelope ----------------------------------
    let (status, body) = rpc(&base, "recent_posts", Some(json!({"n": 5}))).await;
    assert_eq!(status, 200);
    assert_eq!(body["rows"].as_array().unwrap().len(), 1);

    // -- one arity write + derived error in the §8.1 envelope ----------------
    let (status, body) = rpc(
        &base,
        "rename_user",
        Some(json!({"user": luis_id, "username": "luis_x"})),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["username"], "luis_x");
    let (status, body) = rpc(
        &base,
        "rename_user",
        Some(json!({"user": luis_id, "username": "maya"})),
    )
    .await;
    assert_eq!((status, error_code(&body)), (409, "user_username_taken"));
    assert_eq!(body["error"]["table"], "user");

    // -- argument failures, envelope-shaped -----------------------------------
    let (status, body) = rpc(&base, "find_user", None).await; // missing required
    assert_eq!((status, error_code(&body)), (400, "bad_request"));
    let (status, body) = rpc(&base, "find_user", Some(json!({"username": 42}))).await;
    assert_eq!((status, error_code(&body)), (422, "type_mismatch"));
    let (status, body) = rpc(&base, "find_user", Some(json!({"ghost": "x"}))).await;
    assert_eq!((status, error_code(&body)), (422, "unknown_field"));
    let (status, body) = rpc(&base, "find_user", Some(json!(["not", "an", "object"]))).await;
    assert_eq!((status, error_code(&body)), (400, "bad_request"));

    // -- unknown fn and write-miss ---------------------------------------------
    let (status, body) = rpc(&base, "nope", None).await;
    assert_eq!((status, error_code(&body)), (404, "not_found"));
    let ghost = uuid::Uuid::now_v7().to_string();
    let (status, body) = rpc(
        &base,
        "rename_user",
        Some(json!({"user": ghost, "username": "x"})),
    )
    .await;
    assert_eq!((status, error_code(&body)), (404, "not_found"));

    // -- GET rpc: read fns answer safe methods (§7.4, RFD 0012) ----------------
    // query-string values parse by the declared parameter type
    let (status, body) = get(&base, "/rest/v1/rpc/find_user?username=maya").await;
    assert_eq!(status, 200);
    assert_eq!(body["bio"], "photographer");
    let (status, body) = get(&base, "/rest/v1/rpc/recent_posts?n=1").await;
    assert_eq!(status, 200);
    assert_eq!(body["rows"].as_array().unwrap().len(), 1);
    // an unparseable value keeps the canonical type_mismatch envelope
    let (status, body) = get(&base, "/rest/v1/rpc/recent_posts?n=abc").await;
    assert_eq!((status, error_code(&body)), (422, "type_mismatch"));
    // unknown keys keep the canonical unknown_field envelope
    let (status, body) = get(&base, "/rest/v1/rpc/find_user?username=maya&ghost=1").await;
    assert_eq!((status, error_code(&body)), (422, "unknown_field"));
    // a mut fn refuses the safe method: 405, never a write
    let (status, body) = get(&base, "/rest/v1/rpc/rename_user?user=x&username=y").await;
    assert_eq!((status, error_code(&body)), (405, "bad_request"));
}

async fn whoami(base: &str, actor: Option<&str>) -> Value {
    let mut req = reqwest::Client::new().get(format!("{base}/~whoami"));
    if let Some(a) = actor {
        req = req.header("X-Spock-Actor", a);
    }
    let resp = req.send().await.expect("GET whoami");
    resp.json::<Value>().await.unwrap_or(Value::Null)
}

#[tokio::test]
async fn personas_and_whoami() {
    // -- uuid-keyed anchor: label is the unique text field, actor is the key --
    let base = start_program(PERSONA_PROGRAM).await;

    let (status, personas) = get(&base, "/~personas").await;
    assert_eq!(status, 200);
    let arr = personas.as_array().expect("personas is an array");
    assert_eq!(arr.len(), 3);
    let maya = arr
        .iter()
        .find(|p| p["label"] == "maya")
        .expect("maya persona");
    let maya_id = maya["actor"]
        .as_str()
        .expect("uuid actor string")
        .to_string();
    // the actor is the uuid key, not the username label
    assert!(
        uuid::Uuid::parse_str(&maya_id).is_ok(),
        "actor is the uuid key, got {maya_id}"
    );

    // no header → anonymous
    let (status, who) = get(&base, "/~whoami").await;
    assert_eq!(status, 200);
    assert_eq!(
        who,
        json!({ "actor": null, "anonymous": true, "known": false })
    );

    // a valid, seeded actor key → known
    assert_eq!(
        whoami(&base, Some(&maya_id)).await,
        json!({ "actor": maya_id, "anonymous": false, "known": true })
    );

    // the classic mistake: sending the username (the picker's *label*) when the
    // key is a uuid — a present-but-unparseable value, NOT anonymous, NOT known
    assert_eq!(
        whoami(&base, Some("maya")).await,
        json!({ "actor": null, "anonymous": false, "known": false })
    );

    // a well-formed but unseeded uuid → not anonymous, not known
    let ghost = uuid::Uuid::now_v7().to_string();
    assert_eq!(
        whoami(&base, Some(&ghost)).await,
        json!({ "actor": ghost, "anonymous": false, "known": false })
    );

    // -- text-keyed anchor: no unique text field → label falls back to the key --
    let base = start_program(ACTOR_PROGRAM).await;
    let (_, personas) = get(&base, "/~personas").await;
    let arr = personas.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    // `account` has key handle:text and name:text? (not unique) → label = handle
    let maya = arr.iter().find(|p| p["actor"] == "maya").expect("maya");
    assert_eq!(maya["label"], "maya");
    assert_eq!(
        whoami(&base, Some("maya")).await,
        json!({ "actor": "maya", "anonymous": false, "known": true })
    );
    assert_eq!(
        whoami(&base, Some("ghost")).await,
        json!({ "actor": "ghost", "anonymous": false, "known": false })
    );

    // -- no anchor at all → empty picker, always-anonymous whoami -------------
    let base = start().await; // the instagram PROGRAM: `user` is NOT an auth table
    let (status, personas) = get(&base, "/~personas").await;
    assert_eq!((status, personas), (200, json!([])));
    assert_eq!(
        whoami(&base, Some("maya")).await,
        json!({ "actor": null, "anonymous": true, "known": false })
    );
}

/// True when the studio bundle hasn't been built — `dist/` is gitignored, so a
/// fresh checkout embeds nothing and `/~studio` 404s until `pnpm build` runs.
fn studio_unbuilt(resp: &reqwest::Response) -> bool {
    if resp.status().as_u16() == 404 {
        eprintln!(
            "skipping studio test: bundle not built — run `pnpm build` in crates/spock-runtime/studio"
        );
        true
    } else {
        false
    }
}

#[tokio::test]
async fn studio_is_served() {
    // the console (a Vite/React SPA, studio/dist) is embedded in the binary via
    // rust-embed and served same-origin at /~studio
    let base = start().await;
    let resp = reqwest::get(format!("{base}/~studio"))
        .await
        .expect("GET /~studio");
    // dist/ is gitignored; on a fresh checkout the bundle isn't built yet
    if studio_unbuilt(&resp) {
        return;
    }
    assert_eq!(resp.status().as_u16(), 200);
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("text/html"), "served as html, got `{ct}`");
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("<title>spock studio</title>"),
        "the studio page is served"
    );
    // the SPA loads its bundle same-origin under the /~studio/ base (no CDN)
    assert!(
        body.contains("/~studio/assets/"),
        "the page references its embedded bundle"
    );
}

#[tokio::test]
async fn studio_assets_are_served() {
    // the hashed bundle (JS/CSS/fonts) is embedded too, so the console is fully
    // offline; an unknown asset path is a genuine 404.
    let base = start().await;
    let resp = reqwest::get(format!("{base}/~studio"))
        .await
        .expect("GET /~studio");
    if studio_unbuilt(&resp) {
        return;
    }
    let index = resp.text().await.unwrap();
    // pull the built JS bundle url out of the served index.html
    let i = index.find("src=\"/~studio/").expect("script src present");
    let after = &index[i + 5..];
    let url = &after[..after.find('"').expect("closing quote")];
    assert!(url.ends_with(".js"), "found a js bundle, got `{url}`");

    let asset = reqwest::get(format!("{base}{url}"))
        .await
        .expect("GET bundle");
    assert_eq!(asset.status().as_u16(), 200);
    let ct = asset
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(ct.contains("javascript"), "js mimetype, got `{ct}`");

    let missing = reqwest::get(format!("{base}/~studio/assets/does-not-exist.js"))
        .await
        .expect("GET missing");
    assert_eq!(missing.status().as_u16(), 404);
}

#[tokio::test]
async fn a_table_named_rpc_fails_startup() {
    let contract =
        spock_lang::compile("table rpc { key id: uuid = auto\n a: int }").expect("compiles");
    let conn = engine::open(&contract, None, None).expect("engine opens");
    let app = Arc::new(App::new(contract, conn));
    assert!(http::router(app).is_err());
}

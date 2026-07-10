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
    assert_eq!(resp["data"]["__schema"]["mutationType"]["name"], "Mutation");
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

    // the by-key root now exists for composite tables, but its key args are
    // required — omitting them is still a validation error over HTTP 200
    let resp = gql(&base, "{ follow { since } }", Value::Null).await;
    assert!(!resp["errors"].is_null());

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

fn extensions(response: &Value) -> &Value {
    &response["errors"][0]["extensions"]
}

#[tokio::test]
async fn the_graphql_mutations() {
    let base = start().await;

    // -- create: defaults applied, row returned -----------------------------
    let resp = gql(
        &base,
        r#"mutation { create_user(username: "vera") { id username bio joined_at } }"#,
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let vera = &resp["data"]["create_user"];
    assert_eq!(vera["username"], "vera");
    assert!(vera["bio"].is_null());
    let vera_id = vera["id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&vera_id).is_ok()); // auto -> uuidv7
    assert!(vera["joined_at"].as_str().unwrap().contains('T')); // now -> rfc3339

    // -- create: derived unique error in extensions -------------------------
    let resp = gql(
        &base,
        r#"mutation { create_user(username: "maya") { id } }"#,
        Value::Null,
    )
    .await;
    assert!(resp["data"].is_null());
    let ext = extensions(&resp);
    assert_eq!(ext["code"], "user_username_taken");
    assert_eq!(ext["kind"], "unique");
    assert_eq!(ext["table"], "user");
    assert_eq!(ext["fields"], json!(["username"]));

    // -- create: omitted required arg is a *validation* error ---------------
    // (required-no-default fields are non-null args; the type system
    // supersedes the derived `required` error on create — spec 8.2)
    let resp = gql(
        &base,
        "mutation { create_user(bio: \"x\") { id } }",
        Value::Null,
    )
    .await;
    assert!(!resp["errors"].is_null());

    // -- create: ref errors --------------------------------------------------
    let ghost = uuid::Uuid::now_v7().to_string();
    let resp = gql(
        &base,
        &format!(r#"mutation {{ create_post(author: "{ghost}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "post_author_not_found");
    assert_eq!(extensions(&resp)["kind"], "ref_not_found");

    let resp = gql(
        &base,
        r#"mutation { create_post(author: "not-a-uuid") { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "type_mismatch");

    // -- create: composite key conflict --------------------------------------
    let (maya_id, luis_id) = {
        let resp = gql(&base, "{ user_list { id username } }", Value::Null).await;
        let users = resp["data"]["user_list"].as_array().unwrap().clone();
        let id_of = |name: &str| {
            users.iter().find(|u| u["username"] == name).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_string()
        };
        (id_of("maya"), id_of("luis"))
    };
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ create_follow(follower: "{luis_id}", target: "{maya_id}") {{ since }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "follow_already_exists");
    assert_eq!(extensions(&resp)["kind"], "key");

    // -- update: change, keep, clear ------------------------------------------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user(id: "{vera_id}", bio: "climber") {{ username bio }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user"]["bio"], "climber");
    assert_eq!(resp["data"]["update_user"]["username"], "vera"); // untouched

    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{vera_id}", bio: null) {{ bio }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["update_user"]["bio"].is_null()); // explicit null clears

    // -- update: null on required fields -> derived required errors ----------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{vera_id}", username: null) {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_required");

    // required-with-default non-key field: clearable, so derived too
    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{vera_id}", joined_at: null) {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_joined_at_required");

    // -- update: unique conflict (pins UPDATE-vs-INSERT message parsing) ------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{vera_id}", username: "maya") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_taken");

    // -- update: write-miss and malformed key are errors, not null -----------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{ghost}", bio: "x") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    let resp = gql(
        &base,
        r#"mutation { update_user(id: "not-a-uuid", bio: "x") { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    // -- update: empty change set is a validated no-op ------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ update_user(id: "{vera_id}") {{ username }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user"]["username"], "vera");

    // -- update via variables: null clears, UNPROVIDED means omitted ----------
    // (pins the workaround for async-graphql coercing unprovided nullable
    // variables to null — spec 8.2 says unprovided variable = omitted arg)
    let query = r#"mutation($id: UUID!, $bio: String) { update_user(id: $id, bio: $bio) { bio } }"#;
    let resp = gql(&base, query, json!({ "id": vera_id, "bio": "with-vars" })).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user"]["bio"], "with-vars");

    let resp = gql(&base, query, json!({ "id": vera_id })).await; // $bio unprovided
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user"]["bio"], "with-vars"); // untouched

    let resp = gql(&base, query, json!({ "id": vera_id, "bio": null })).await;
    assert_no_errors(&resp);
    assert!(resp["data"]["update_user"]["bio"].is_null()); // explicit null clears

    // -- serial mutations: first commits, second errors, first survives -------
    let resp = gql(
        &base,
        r#"mutation {
             a: create_user(username: "rex") { id }
             b: create_user(username: "rex") { id }
           }"#,
        Value::Null,
    )
    .await;
    assert!(!resp["errors"].is_null()); // b conflicts
    let resp = gql(
        &base,
        r#"{ user_list(limit: 200) { username } }"#,
        Value::Null,
    )
    .await;
    let names: Vec<&str> = resp["data"]["user_list"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["username"].as_str().unwrap())
        .collect();
    assert_eq!(names.iter().filter(|n| **n == "rex").count(), 1); // a committed

    // -- delete: restrict blocks ----------------------------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_user(id: "{maya_id}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_restricted");
    assert_eq!(extensions(&resp)["kind"], "restricted");

    // -- delete: returns the row, then the read is null ------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_user(id: "{vera_id}") {{ username }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["delete_user"]["username"], "vera");
    let resp = gql(
        &base,
        &format!(r#"{{ user(id: "{vera_id}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert!(resp["data"]["user"].is_null());

    // -- delete: cascade (post -> its comments) --------------------------------
    let resp = gql(&base, r#"{ post_list { id caption } }"#, Value::Null).await;
    let p1 = resp["data"]["post_list"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["caption"] == "first light")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = gql(&base, "{ comment_list { id } }", Value::Null).await;
    assert_eq!(resp["data"]["comment_list"].as_array().unwrap().len(), 1);
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_post(id: "{p1}") {{ caption }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let resp = gql(&base, "{ comment_list { id } }", Value::Null).await;
    assert_eq!(resp["data"]["comment_list"].as_array().unwrap().len(), 0);

    // -- delete: double delete is not_found -------------------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_post(id: "{p1}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    // -- composite by-key query, then unfollow (the composite motivator) --------
    let resp = gql(
        &base,
        &format!(r#"{{ follow(follower: "{luis_id}", target: "{maya_id}") {{ since }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["follow"]["since"].is_string());
    let resp = gql(
        &base,
        &format!(r#"{{ follow(follower: "{maya_id}", target: "{luis_id}") {{ since }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["follow"].is_null()); // read-miss stays null

    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ delete_follow(follower: "{luis_id}", target: "{maya_id}") {{ since }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let resp = gql(&base, "{ follow_list { since } }", Value::Null).await;
    assert_eq!(resp["data"]["follow_list"].as_array().unwrap().len(), 0);
}

//! End-to-end GraphQL verification (docs/spec/graphql.md, Tier 1): compile
//! a real program, serve it, and exercise the derived schema over actual
//! HTTP — introspection, list/by-pk roots, forward and reverse nesting,
//! limits, null-for-miss, mutations with derived errors, and GraphiQL.

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

record stats { posts: int }

fn rename_user(user: user, username: text) -> user ! user_username_taken {
  unchecked sql("""
    UPDATE user SET username = :username
    WHERE id = :user
    RETURNING *
  """)
}

fn find_user(username: text) -> user? {
  unchecked sql("""SELECT * FROM user WHERE username = :username""")
}

fn author_stats(author: user) -> stats {
  unchecked sql("""SELECT count(*) AS posts FROM post WHERE author = :author""")
}

fn recent_posts(n: int) -> [post] {
  unchecked sql("""SELECT * FROM post ORDER BY published_at DESC LIMIT :n""")
}

fn post_count(author: user) -> int {
  unchecked sql("""SELECT count(*) FROM post WHERE author = :author""")
}

fn captions() -> [text] {
  unchecked sql("""SELECT caption FROM post ORDER BY caption""")
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

    // -- introspection: verbatim type names, lowercase scalars --------------
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
    for expected in [
        "user",
        "post",
        "follow",
        "comment",
        "uuid",
        "timestamp",
        "user_insert_input",
        "user_set_input",
        "user_pk_columns_input",
    ] {
        assert!(types.contains(&expected), "missing type {expected}");
    }

    // -- root list: the bare table name -------------------------------------
    let resp = gql(&base, "{ user { username bio } }", Value::Null).await;
    assert_no_errors(&resp);
    let users = resp["data"]["user"].as_array().unwrap();
    assert_eq!(users.len(), 2);
    assert_eq!(users.iter().filter(|u| u["username"] == "maya").count(), 1);

    // -- forward ref nesting ------------------------------------------------
    let resp = gql(
        &base,
        "{ post { caption author { username } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let posts = resp["data"]["post"].as_array().unwrap();
    assert_eq!(posts.len(), 2);
    assert!(posts.iter().all(|p| p["author"]["username"] == "maya"));

    // -- reverse collection nesting (the canonical pitch query) -------------
    let resp = gql(
        &base,
        "{ user { username post_by_author { caption comment_by_post { body } } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let posts = maya["post_by_author"].as_array().unwrap();
    assert_eq!(posts.len(), 2);
    let first_light = posts
        .iter()
        .find(|p| p["caption"] == "first light")
        .unwrap();
    assert_eq!(first_light["comment_by_post"][0]["body"], "great shot");
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert_eq!(luis["post_by_author"].as_array().unwrap().len(), 0);

    // -- both follow directions exist and are distinct ----------------------
    let resp = gql(
        &base,
        "{ user { username follow_by_follower { since } follow_by_target { since } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert_eq!(maya["follow_by_target"].as_array().unwrap().len(), 1); // luis follows maya
    assert_eq!(maya["follow_by_follower"].as_array().unwrap().len(), 0);
    assert_eq!(luis["follow_by_follower"].as_array().unwrap().len(), 1);
    assert_eq!(luis["follow_by_target"].as_array().unwrap().len(), 0);

    // -- optional self-ref: object or null -----------------------------------
    let resp = gql(
        &base,
        "{ user { username invited_by { username } } }",
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let users = resp["data"]["user"].as_array().unwrap();
    let maya = users.iter().find(|u| u["username"] == "maya").unwrap();
    let luis = users.iter().find(|u| u["username"] == "luis").unwrap();
    assert!(maya["invited_by"].is_null());
    assert_eq!(luis["invited_by"]["username"], "maya");

    // -- by-pk root: hit via variables, misses are null ----------------------
    let resp = gql(&base, "{ user { id username } }", Value::Null).await;
    let users = resp["data"]["user"].as_array().unwrap();
    let maya_id = users.iter().find(|u| u["username"] == "maya").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let query = "query($id: uuid!) { user_by_pk(id: $id) { username } }";
    let resp = gql(&base, query, json!({ "id": maya_id })).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["user_by_pk"]["username"], "maya");

    let resp = gql(
        &base,
        query,
        json!({ "id": uuid::Uuid::now_v7().to_string() }),
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["user_by_pk"].is_null());

    let resp = gql(&base, query, json!({ "id": "not-a-uuid" })).await;
    assert_no_errors(&resp);
    assert!(resp["data"]["user_by_pk"].is_null()); // malformed key matches no row

    // -- limits: default is applied, cap clamps, negative errors ------------
    let resp = gql(&base, "{ post(limit: 1) { caption } }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["post"].as_array().unwrap().len(), 1);

    let resp = gql(&base, "{ post(limit: 99999) { caption } }", Value::Null).await;
    assert_no_errors(&resp); // clamped, not an error
    assert_eq!(resp["data"]["post"].as_array().unwrap().len(), 2);

    let resp = gql(&base, "{ post(limit: -1) { caption } }", Value::Null).await;
    assert!(!resp["errors"].is_null());

    // -- composite-key table: bare name lists; by-pk needs its key args ------
    let resp = gql(&base, "{ follow { since } }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["follow"].as_array().unwrap().len(), 1);

    let resp = gql(&base, "{ follow_by_pk { since } }", Value::Null).await;
    assert!(!resp["errors"].is_null()); // key args are required

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
async fn the_graphql_fns() {
    let base = start().await;

    // ids for the write cases
    let resp = gql(&base, "{ user { id username } }", Value::Null).await;
    let users = resp["data"]["user"].as_array().unwrap().clone();
    let id_of = |name: &str| {
        users.iter().find(|u| u["username"] == name).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (maya_id, luis_id) = (id_of("maya"), id_of("luis"));

    // -- introspection: declared codes reach the description ----------------
    let resp = gql(
        &base,
        "{ __schema { mutationType { fields { name description } } } }",
        Value::Null,
    )
    .await;
    let fields = resp["data"]["__schema"]["mutationType"]["fields"]
        .as_array()
        .unwrap();
    let rename = fields.iter().find(|f| f["name"] == "rename_user").unwrap();
    assert!(rename["description"]
        .as_str()
        .unwrap()
        .contains("user_username_taken"));

    // -- maybe arity via variables: hit and null miss ------------------------
    let q = "mutation($u: String!) { find_user(username: $u) { username bio } }";
    let resp = gql(&base, q, json!({"u": "maya"})).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["find_user"]["bio"], "photographer");
    let resp = gql(&base, q, json!({"u": "ghost"})).await;
    assert_no_errors(&resp);
    assert!(resp["data"]["find_user"].is_null());

    // -- record return: an aggregate as a declared shape ---------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ author_stats(author: "{maya_id}") {{ posts }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["author_stats"]["posts"], 2);

    // -- many arity: the author owns LIMIT ------------------------------------
    let resp = gql(&base, "mutation { recent_posts(n: 1) { caption } }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["recent_posts"].as_array().unwrap().len(), 1);

    // -- scalar returns: GraphQL leaves, no selection set ----------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ post_count(author: "{maya_id}") }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["post_count"], 2);
    let resp = gql(&base, "mutation { captions }", Value::Null).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["captions"], json!(["first light", "golden hour"]));

    // -- one arity: the write returns the row ---------------------------------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ rename_user(user: "{luis_id}", username: "luis_x") {{ username }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["rename_user"]["username"], "luis_x");

    // -- a constraint tripped inside the escape routes to the derived code ----
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ rename_user(user: "{luis_id}", username: "maya") {{ username }} }}"#
        ),
        Value::Null,
    )
    .await;
    let ext = extensions(&resp);
    assert_eq!(ext["code"], "user_username_taken");
    assert_eq!(ext["kind"], "unique");
    assert_eq!(ext["table"], "user");
    assert_eq!(ext["fields"], json!(["username"]));

    // -- -> t with no matching row shouts (write-miss, D1) --------------------
    let ghost = uuid::Uuid::now_v7().to_string();
    let resp = gql(
        &base,
        &format!(r#"mutation {{ rename_user(user: "{ghost}", username: "x") {{ username }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    // -- malformed ref arg is a type mismatch ----------------------------------
    let resp = gql(
        &base,
        r#"mutation { rename_user(user: "not-a-uuid", username: "x") { username } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "type_mismatch");
}

#[tokio::test]
async fn the_graphql_mutations() {
    let base = start().await;

    // -- insert: defaults applied, row returned -----------------------------
    let resp = gql(
        &base,
        r#"mutation { insert_user_one(object: {username: "vera"}) { id username bio joined_at } }"#,
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let vera = &resp["data"]["insert_user_one"];
    assert_eq!(vera["username"], "vera");
    assert!(vera["bio"].is_null());
    let vera_id = vera["id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&vera_id).is_ok()); // auto -> uuidv7
    assert!(vera["joined_at"].as_str().unwrap().contains('T')); // now -> rfc3339

    // -- insert: derived unique error in extensions -------------------------
    let resp = gql(
        &base,
        r#"mutation { insert_user_one(object: {username: "maya"}) { id } }"#,
        Value::Null,
    )
    .await;
    assert!(resp["data"].is_null());
    let ext = extensions(&resp);
    assert_eq!(ext["code"], "user_username_taken");
    assert_eq!(ext["kind"], "unique");
    assert_eq!(ext["table"], "user");
    assert_eq!(ext["fields"], json!(["username"]));

    // -- insert: omitted required field is the *derived* error --------------
    // (insert_input is all-nullable, graphql.md §5 — required-ness is the
    // contract's to enforce, un-shadowed by GraphQL validation)
    let resp = gql(
        &base,
        r#"mutation { insert_user_one(object: {bio: "x"}) { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_required");
    assert_eq!(extensions(&resp)["kind"], "required");

    // on insert, explicit null is absence (v0 §5.1) — same derived error
    let resp = gql(
        &base,
        r#"mutation { insert_user_one(object: {username: null, bio: "x"}) { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_required");

    // -- insert: ref errors ---------------------------------------------------
    let ghost = uuid::Uuid::now_v7().to_string();
    let resp = gql(
        &base,
        &format!(r#"mutation {{ insert_post_one(object: {{author: "{ghost}"}}) {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "post_author_not_found");
    assert_eq!(extensions(&resp)["kind"], "ref_not_found");

    let resp = gql(
        &base,
        r#"mutation { insert_post_one(object: {author: "not-a-uuid"}) { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "type_mismatch");

    // -- insert: composite key conflict ---------------------------------------
    let (maya_id, luis_id) = {
        let resp = gql(&base, "{ user { id username } }", Value::Null).await;
        let users = resp["data"]["user"].as_array().unwrap().clone();
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
            r#"mutation {{ insert_follow_one(object: {{follower: "{luis_id}", target: "{maya_id}"}}) {{ since }} }}"#
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
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{bio: "climber"}}) {{ username bio }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user_by_pk"]["bio"], "climber");
    assert_eq!(resp["data"]["update_user_by_pk"]["username"], "vera"); // untouched

    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{bio: null}}) {{ bio }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["update_user_by_pk"]["bio"].is_null()); // explicit null clears

    // -- update: null on required fields -> derived required errors ----------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{username: null}}) {{ id }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_required");

    // required-with-default non-key field: clearable, so derived too
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{joined_at: null}}) {{ id }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_joined_at_required");

    // -- update: unique conflict (pins UPDATE-vs-INSERT message parsing) ------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{username: "maya"}}) {{ id }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_username_taken");

    // -- update: write-miss and malformed key are errors, not null -----------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{ghost}"}}, _set: {{bio: "x"}}) {{ id }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    let resp = gql(
        &base,
        r#"mutation { update_user_by_pk(pk_columns: {id: "not-a-uuid"}, _set: {bio: "x"}) { id } }"#,
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    // -- update: empty change set is a validated no-op ------------------------
    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{vera_id}"}}, _set: {{}}) {{ username }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user_by_pk"]["username"], "vera");

    // -- update via variables inside `_set`: null clears, UNPROVIDED omits ----
    // (pins the workaround for async-graphql coercing unprovided nullable
    // variables to null inside input objects — graphql.md §5, normative)
    let query = r#"mutation($id: uuid!, $bio: String) { update_user_by_pk(pk_columns: {id: $id}, _set: {bio: $bio}) { bio } }"#;
    let resp = gql(&base, query, json!({ "id": vera_id, "bio": "with-vars" })).await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user_by_pk"]["bio"], "with-vars");

    let resp = gql(&base, query, json!({ "id": vera_id })).await; // $bio unprovided
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["update_user_by_pk"]["bio"], "with-vars"); // untouched

    let resp = gql(&base, query, json!({ "id": vera_id, "bio": null })).await;
    assert_no_errors(&resp);
    assert!(resp["data"]["update_user_by_pk"]["bio"].is_null()); // explicit null clears

    // -- serial mutations: first commits, second errors, first survives -------
    let resp = gql(
        &base,
        r#"mutation {
             a: insert_user_one(object: {username: "rex"}) { id }
             b: insert_user_one(object: {username: "rex"}) { id }
           }"#,
        Value::Null,
    )
    .await;
    assert!(!resp["errors"].is_null()); // b conflicts
    let resp = gql(&base, r#"{ user(limit: 200) { username } }"#, Value::Null).await;
    let names: Vec<&str> = resp["data"]["user"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["username"].as_str().unwrap())
        .collect();
    assert_eq!(names.iter().filter(|n| **n == "rex").count(), 1); // a committed

    // -- delete: restrict blocks ----------------------------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_user_by_pk(id: "{maya_id}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "user_restricted");
    assert_eq!(extensions(&resp)["kind"], "restricted");

    // -- delete: returns the row, then the read is null ------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_user_by_pk(id: "{vera_id}") {{ username }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert_eq!(resp["data"]["delete_user_by_pk"]["username"], "vera");
    let resp = gql(
        &base,
        &format!(r#"{{ user_by_pk(id: "{vera_id}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert!(resp["data"]["user_by_pk"].is_null());

    // -- delete: cascade (post -> its comments) --------------------------------
    let resp = gql(&base, r#"{ post { id caption } }"#, Value::Null).await;
    let p1 = resp["data"]["post"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["caption"] == "first light")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = gql(&base, "{ comment { id } }", Value::Null).await;
    assert_eq!(resp["data"]["comment"].as_array().unwrap().len(), 1);
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_post_by_pk(id: "{p1}") {{ caption }} }}"#),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let resp = gql(&base, "{ comment { id } }", Value::Null).await;
    assert_eq!(resp["data"]["comment"].as_array().unwrap().len(), 0);

    // -- delete: double delete is not_found -------------------------------------
    let resp = gql(
        &base,
        &format!(r#"mutation {{ delete_post_by_pk(id: "{p1}") {{ id }} }}"#),
        Value::Null,
    )
    .await;
    assert_eq!(extensions(&resp)["code"], "not_found");

    // -- composite by-pk query, then unfollow (the composite motivator) --------
    let resp = gql(
        &base,
        &format!(
            r#"{{ follow_by_pk(follower: "{luis_id}", target: "{maya_id}") {{ since }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["follow_by_pk"]["since"].is_string());
    let resp = gql(
        &base,
        &format!(
            r#"{{ follow_by_pk(follower: "{maya_id}", target: "{luis_id}") {{ since }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    assert!(resp["data"]["follow_by_pk"].is_null()); // read-miss stays null

    let resp = gql(
        &base,
        &format!(
            r#"mutation {{ delete_follow_by_pk(follower: "{luis_id}", target: "{maya_id}") {{ since }} }}"#
        ),
        Value::Null,
    )
    .await;
    assert_no_errors(&resp);
    let resp = gql(&base, "{ follow { since } }", Value::Null).await;
    assert_eq!(resp["data"]["follow"].as_array().unwrap().len(), 0);
}

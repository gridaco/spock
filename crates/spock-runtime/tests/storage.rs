//! Storage byte plane (RFD 0018) — the G16 avatar-upload gauntlet end to end:
//! signed URL → upload → attach → serve, plus the orphan sweep and the
//! read-only floor. The harness keeps the `Arc<App>` so the sweep (which has no
//! HTTP endpoint — it is an in-process concern) can be driven directly.

use std::sync::Arc;

use serde_json::{json, Value};
use spock_runtime::{engine, http, storage, App};

const PROGRAM: &str = r#"
auth table user {
  key id: uuid = auto
  username: text unique
  avatar: storage_object?
}
seed {
  user { username: "maya" }
}
"#;

/// A far-future cutoff (canonical timestamp text) makes every object older than
/// it — forcing the sweep to consider all of them, no 30-minute wait.
const FUTURE: &str = "2999-01-01T00:00:00.000000Z";

struct Harness {
    base: String,
    app: Arc<App>,
    client: reqwest::Client,
}

async fn start() -> Harness {
    let contract = spock_lang::compile(PROGRAM).expect("program compiles");
    let conn = engine::open(&contract, None, None).expect("engine opens and seeds");
    let app = Arc::new(App::new(contract, conn));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let served = app.clone();
    tokio::spawn(async move {
        http::serve(served, listener).await.expect("serve");
    });
    Harness {
        base: format!("http://{addr}"),
        app,
        client: reqwest::Client::new(),
    }
}

impl Harness {
    async fn maya(&self) -> String {
        let rows = self
            .client
            .get(format!("{}/rest/v1/user", self.base))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        rows["rows"][0]["id"].as_str().unwrap().to_string()
    }

    /// Mint a pending object as `actor` (or anonymous), returning `(id, put_url)`.
    async fn mint(&self, actor: Option<&str>) -> (String, String) {
        let mut req = self
            .client
            .post(format!("{}/storage/v1/object/upload/sign", self.base));
        if let Some(a) = actor {
            req = req.header("X-Spock-Actor", a);
        }
        let body = req.send().await.unwrap().json::<Value>().await.unwrap();
        (
            body["id"].as_str().unwrap().to_string(),
            body["url"].as_str().unwrap().to_string(),
        )
    }

    async fn put(&self, url: &str, bytes: &[u8], content_type: &str) -> reqwest::StatusCode {
        self.client
            .put(format!("{}{url}", self.base))
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(bytes.to_vec())
            .send()
            .await
            .unwrap()
            .status()
    }

    async fn attach(&self, user: &str, object: &str) -> Value {
        let q = format!(
            r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{user}"}}, _set: {{avatar: "{object}"}}) {{ avatar {{ id state content_type size }} }} }}"#
        );
        self.client
            .post(format!("{}/graphql/v1", self.base))
            .json(&json!({ "query": q }))
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    }

    async fn object_exists(&self, id: &str) -> bool {
        self.client
            .get(format!("{}/rest/v1/storage_object/{id}", self.base))
            .send()
            .await
            .unwrap()
            .status()
            == 200
    }
}

#[tokio::test]
async fn g16_avatar_upload_end_to_end() {
    let h = start().await;
    let maya = h.maya().await;
    let bytes = b"\x89PNG\r\n\x1a\n-avatar-payload-";

    // 1. mint a signed upload URL (owner stamped from the actor header)
    let (id, put_url) = h.mint(Some(&maya)).await;

    // 2. upload the bytes → committed
    assert_eq!(h.put(&put_url, bytes, "image/png").await, 204);

    // 3. attach: the ref expands to the nested storage_object, now committed
    let attached = h.attach(&maya, &id).await;
    let obj = &attached["data"]["update_user_by_pk"]["avatar"];
    assert_eq!(obj["id"], id, "{attached}");
    assert_eq!(obj["state"], "committed");
    assert_eq!(obj["content_type"], "image/png");
    assert_eq!(obj["size"], bytes.len());

    // 4. sign a download URL and fetch the bytes back, byte-identical
    let signed = h
        .client
        .post(format!("{}/storage/v1/object/sign/{id}", h.base))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let get_url = signed["url"].as_str().unwrap();
    let resp = h
        .client
        .get(format!("{}{get_url}", h.base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers()[reqwest::header::CONTENT_TYPE], "image/png");
    assert_eq!(resp.bytes().await.unwrap().as_ref(), bytes);
}

#[tokio::test]
async fn signature_is_bound_and_state_guarded() {
    let h = start().await;
    let maya = h.maya().await;
    let (id, put_url) = h.mint(Some(&maya)).await;

    // a tampered signature never verifies (401 before any state is touched)
    let bad = h
        .client
        .get(format!(
            "{}/storage/v1/object/{id}?exp=9999999999&sig=deadbeef",
            h.base
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);

    // the first upload commits; a second, validly-signed PUT is a 409
    assert_eq!(h.put(&put_url, b"one", "text/plain").await, 204);
    assert_eq!(h.put(&put_url, b"two", "text/plain").await, 409);
}

#[tokio::test]
async fn storage_object_is_read_only_on_the_floor() {
    let h = start().await;
    let body = h
        .client
        .post(format!("{}/graphql/v1", h.base))
        .json(&json!({ "query": "mutation { insert_storage_object_one(object: {}) { id } }" }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let msg = body["errors"][0]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("insert_storage_object_one"),
        "the builtin table must not be floor-writable, got {body}"
    );
}

#[tokio::test]
async fn sweep_reclaims_orphans_but_spares_attached() {
    let h = start().await;
    let maya = h.maya().await;

    // A: minted, never uploaded → unattached pending orphan
    let (a, _) = h.mint(Some(&maya)).await;
    // B: uploaded, never attached → unreferenced committed orphan
    let (b, b_url) = h.mint(Some(&maya)).await;
    assert_eq!(h.put(&b_url, b"bee", "text/plain").await, 204);
    // C: uploaded and attached → must survive any sweep
    let (c, c_url) = h.mint(Some(&maya)).await;
    assert_eq!(h.put(&c_url, b"see", "text/plain").await, 204);
    h.attach(&maya, &c).await;

    // the real-TTL sweep spares everything fresh
    assert_eq!(storage::sweep(&h.app).unwrap(), 0);
    assert!(h.object_exists(&a).await && h.object_exists(&b).await && h.object_exists(&c).await);

    // with an exhaustive cutoff, the two orphans go and the attached one stays
    let collected = storage::sweep_before(&h.app, FUTURE, FUTURE).unwrap();
    assert_eq!(
        collected, 2,
        "the pending and the unreferenced-committed orphan"
    );
    assert!(!h.object_exists(&a).await, "unattached pending swept");
    assert!(!h.object_exists(&b).await, "unreferenced committed swept");
    assert!(h.object_exists(&c).await, "attached object spared");

    // detach C (null the avatar), then it too becomes reclaimable
    let detach = format!(
        r#"mutation {{ update_user_by_pk(pk_columns: {{id: "{maya}"}}, _set: {{avatar: null}}) {{ username }} }}"#
    );
    h.client
        .post(format!("{}/graphql/v1", h.base))
        .json(&json!({ "query": detach }))
        .send()
        .await
        .unwrap();
    assert_eq!(storage::sweep_before(&h.app, FUTURE, FUTURE).unwrap(), 1);
    assert!(!h.object_exists(&c).await, "detached object now swept");
}

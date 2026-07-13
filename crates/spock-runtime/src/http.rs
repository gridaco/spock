//! The HTTP protocol (docs/spec/v0.md §8). Plain HTTP + JSON. The root
//! namespace is protocol-owned: `~` is the meta surface, `/rest/v1` carries
//! the open reads (identity views, v0 degenerate form), `/graphql/v1` the
//! GraphQL reads and writes (§8.2). REST tables stay read-only in v0;
//! `POST /rest/v1/rpc/{fn}` is the deliberate write surface, and read fns
//! also answer `GET /rest/v1/rpc/{fn}` with query-string arguments — the
//! PostgREST stable-function symmetry (§7.4, RFD 0012).

use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dynamic::Schema;
use async_graphql::http::GraphiQLSource;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rust_embed::RustEmbed;
use serde_json::{json, Map, Value as JsonValue};
use spock_lang::ir::{FnArity, Table, Type};

use crate::error::ApiError;
use crate::filter;
use crate::graphql::{self, SchemaBuildError};
use crate::App;
use crate::{func, write};

/// The dev persona picker caps its projection so a large dev database cannot
/// blow up the dropdown (RFD 0015 §6.1).
const PERSONA_CAP: u32 = 100;

/// Startup failures: schema-derivation collisions (§8.2 naming laws), a table
/// claiming a protocol-owned REST segment, or a column whose name collides
/// with a reserved PostgREST control key (RFD 0021 §6). All abort load — never
/// a request-time surprise.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(transparent)]
    Schema(#[from] SchemaBuildError),
    #[error("table `rpc` collides with the protocol-owned /rest/v1/rpc segment")]
    ReservedRestSegment,
    #[error(
        "table `{table}` column `{column}` collides with a reserved filter control key \
         (order, limit, offset, select, and, or, not)"
    )]
    ReservedFilterColumn { table: String, column: String },
}

pub fn router(app: Arc<App>) -> Result<Router, StartupError> {
    // `/rest/v1/rpc/{fn}` shadows `GET /rest/v1/rpc/{id}` for a table
    // literally named `rpc` — collisions fail startup, never requests
    if app.contract.table("rpc").is_some() {
        return Err(StartupError::ReservedRestSegment);
    }
    // A column named like a PostgREST control key (`order`, `and`, …) would be
    // unaddressable / ambiguous in a REST filter, so it fails at load, not per
    // request — the exact set the parser treats specially (RFD 0021 §6).
    for table in &app.contract.tables {
        for field in &table.fields {
            if filter::REST_RESERVED_KEYS.contains(&field.name.as_str()) {
                return Err(StartupError::ReservedFilterColumn {
                    table: table.name.clone(),
                    column: field.name.clone(),
                });
            }
        }
    }
    let schema = graphql::schema(app.clone())?;
    let gql = Router::new()
        .route("/graphql/v1", get(graphiql).post(graphql_post))
        .with_state(GqlState {
            schema,
            app: app.clone(),
        });
    let mut base = Router::new()
        .route("/~contract", get(contract))
        .route("/~health", get(health))
        .route("/~studio", get(studio_index))
        .route("/~studio/", get(studio_index))
        .route("/~studio/{*path}", get(studio_asset))
        .route("/~personas", get(personas))
        .route("/~whoami", get(whoami))
        .route("/rest/v1/rpc/{name}", post(rpc_call).get(rpc_get))
        .route("/rest/v1/{table}", get(list_rows))
        .route("/rest/v1/{table}/{id}", get(get_row));

    // The storage byte plane (RFD 0018), mounted only when the contract carries
    // the `storage_object` builtin — sibling to `/rest` and `/graphql`. Its
    // paths are static and top-level, so no user table can shadow them (and the
    // builtin name is reserved at compile time, E048).
    if crate::storage::storage_active(&app.contract) {
        use crate::storage;
        use axum::extract::DefaultBodyLimit;
        base = base
            .route(
                "/storage/v1/object/upload/sign",
                post(storage::post_upload_sign),
            )
            .route(
                "/storage/v1/object/sign/{id}",
                post(storage::post_download_sign),
            )
            .route(
                "/storage/v1/object/{id}",
                get(storage::get_object)
                    .put(storage::put_object)
                    .layer(DefaultBodyLimit::max(storage::MAX_UPLOAD_BYTES)),
            );
    }

    Ok(base.fallback(not_found).with_state(app).merge(gql))
}

/// Serve the app on an already-bound listener until the task is stopped.
/// A GraphQL schema-derivation failure (§8.2 naming laws) aborts startup.
pub async fn serve(app: Arc<App>, listener: tokio::net::TcpListener) -> std::io::Result<()> {
    // The runtime owns background reconciliation: a storage contract sweeps its
    // orphaned objects for the life of the server (RFD 0018 §1.6), so every
    // embedder gets it — not just the CLI. The task is aborted when the server's
    // runtime is dropped.
    if crate::storage::storage_active(&app.contract) {
        tokio::spawn(crate::storage::sweep_loop(app.clone()));
    }
    let router = router(app).map_err(std::io::Error::other)?;
    axum::serve(listener, router).await
}

/// The GraphQL route's state: the derived schema plus the app, so
/// `graphql_post` can resolve the per-request actor from the header before
/// executing (RFD 0014 §4.3). The schema carries the app in its own global
/// data for resolvers; the *actor* rides `Request::data` per request, never
/// the schema-global channel (§14.3 — that would bleed across requests).
#[derive(Clone)]
struct GqlState {
    schema: Schema,
    app: Arc<App>,
}

// POST /graphql/v1 — execute a query (§8.2); errors render as GraphQL's own
// `errors[]`, not the §8.1 envelope
async fn graphql_post(
    State(state): State<GqlState>,
    headers: HeaderMap,
    Json(request): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    let actor = resolve_actor(&state.app, &headers);
    Json(
        state
            .schema
            .execute(request.data(graphql::CurrentActor(actor)))
            .await,
    )
}

// GET /graphql/v1 — GraphiQL. Its JS/CSS load from a CDN: blank offline,
// fine for a prototype.
async fn graphiql() -> Html<String> {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql/v1")
            .title("spock")
            .finish(),
    )
}

async fn contract(State(app): State<Arc<App>>) -> Json<spock_lang::ir::Contract> {
    Json(app.contract.clone())
}

async fn health() -> Json<JsonValue> {
    Json(json!({ "ok": true }))
}

// GET /~studio — the human-developer console (RFD 0015). A Vite/React SPA
// (crates/spock-runtime/studio) built to studio/dist, committed and embedded in
// the binary via rust-embed, served same-origin so the console is fully offline
// (no CDN): every request it makes to /~contract, /rest, /rpc, /~personas,
// /~whoami rides `X-Spock-Actor` with no CORS. A pure consumer of the contract —
// it never defines or edits schema.
#[derive(RustEmbed)]
#[folder = "studio/dist"]
struct StudioAssets;

async fn studio_index() -> Response {
    serve_studio_asset("index.html")
}

// GET /~studio/{*path} — the built assets (hashed JS/CSS, bundled fonts). The
// SPA has no client-side routes, so an unknown path is a genuine 404.
async fn studio_asset(Path(path): Path<String>) -> Response {
    serve_studio_asset(&path)
}

fn serve_studio_asset(path: &str) -> Response {
    match StudioAssets::get(path) {
        Some(file) => (
            [(axum::http::header::CONTENT_TYPE, file.metadata.mimetype())],
            file.data.into_owned(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// GET /~personas — the dev actor picker (RFD 0014 §4.3, RFD 0015 §6.1): the
// anchor table's rows projected to `[{ actor, label }]`. `actor` is the row's
// scalar key (E-ACT02) — verbatim what goes in `X-Spock-Actor`; `label` is the
// first unique text field, else the key itself. No `auth table` → `[]` (the
// caller shows "impersonation unavailable", not a broken dropdown).
async fn personas(State(app): State<Arc<App>>) -> Result<Json<JsonValue>, ApiError> {
    let Some(anchor) = app.contract.anchor() else {
        return Ok(Json(JsonValue::Array(Vec::new())));
    };
    let key = &anchor.key[0];
    let key_field = anchor.field(key).expect("checked: anchor key field exists");
    // the display label: the first unique text column, distinct from the key
    let label_field = anchor.fields.iter().find(|f| {
        f.name != *key && f.unique && matches!(app.contract.value_type(&f.ty), Type::Text)
    });

    let projection = match label_field {
        Some(l) => format!("\"{key}\", \"{}\"", l.name),
        None => format!("\"{key}\""),
    };
    let sql = format!(
        "SELECT {projection} FROM \"{}\" ORDER BY \"{key}\" ASC LIMIT {PERSONA_CAP}",
        anchor.name
    );

    let db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?
    {
        let key_ref = row
            .get_ref(0)
            .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
        let actor = crate::value::sql_to_json(&app.contract, key_field, key_ref);
        let label = match label_field {
            Some(l) => {
                let l_ref = row
                    .get_ref(1)
                    .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
                match crate::value::sql_to_json(&app.contract, l, l_ref) {
                    JsonValue::Null => label_string(&actor),
                    text => text,
                }
            }
            None => label_string(&actor),
        };
        out.push(json!({ "actor": actor, "label": label }));
    }
    Ok(Json(JsonValue::Array(out)))
}

// GET /~whoami — the dev-tier actor echo (RFD 0014 §4.3, RFD 0015 §6.2), the
// mirror of GoTrue's `GET /user`. Never rejects. `anonymous` keys on header
// *presence*, not on `resolve_actor` (which collapses absent / no-anchor /
// present-but-unparseable into `None`): a wrong-type value — the username sent
// where the key is a uuid — must read `anonymous: false, known: false`, the
// debugging signal whoami exists to give. `known` = the key exists as a row.
async fn whoami(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
    let raw = headers.get("x-spock-actor").and_then(|v| v.to_str().ok());
    let (Some(anchor), Some(raw)) = (app.contract.anchor(), raw) else {
        // no anchor, or no header → anonymous
        return Ok(Json(
            json!({ "actor": null, "anonymous": true, "known": false }),
        ));
    };
    // header present but not the key type → invalid, not anonymous
    let Ok(actor) = path_key_value(&app, anchor, raw) else {
        return Ok(Json(
            json!({ "actor": null, "anonymous": false, "known": false }),
        ));
    };

    let known = {
        let db = app
            .db
            .lock()
            .map_err(|_| ApiError::internal("db lock poisoned"))?;
        let sql = format!(
            "SELECT 1 FROM \"{}\" WHERE \"{}\" = ?1 LIMIT 1",
            anchor.name, anchor.key[0]
        );
        let mut stmt = db
            .prepare(&sql)
            .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
        stmt.exists([&actor])
            .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?
    };

    let key_field = anchor
        .field(&anchor.key[0])
        .expect("checked: anchor key field exists");
    let actor = crate::value::sql_to_json(&app.contract, key_field, ValueRef::from(&actor));
    Ok(Json(
        json!({ "actor": actor, "anonymous": false, "known": known }),
    ))
}

/// A picker label as a string: a text column renders verbatim; a non-text key
/// (a uuid/int) renders its JSON scalar as text, so a label is never null.
fn label_string(actor: &JsonValue) -> JsonValue {
    match actor {
        JsonValue::String(s) => JsonValue::String(s.clone()),
        other => JsonValue::String(other.to_string()),
    }
}

async fn not_found() -> ApiError {
    ApiError::not_found("no such path")
}

// POST /rest/v1/rpc/{fn} — call a declared fn (§7.4): REST's one
// deliberate write surface. The body is a JSON object of arguments;
// absent or empty means `{}` (zero-param fns are curl-friendly). The
// body parses by hand so even malformed JSON gets the §8.1 envelope.
async fn rpc_call(
    State(app): State<Arc<App>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<JsonValue>, ApiError> {
    let f = resolve_fn(&app, &name)?;
    let actor = resolve_actor(&app, &headers);
    let args: Map<String, JsonValue> = if body.trim().is_empty() {
        Map::new()
    } else {
        match serde_json::from_str::<JsonValue>(&body) {
            Ok(JsonValue::Object(map)) => map,
            Ok(JsonValue::Null) => Map::new(),
            Ok(_) => {
                return Err(ApiError::bad_request("fn arguments must be a JSON object"));
            }
            Err(e) => return Err(ApiError::bad_request(format!("malformed JSON body: {e}"))),
        }
    };
    run_rpc(&app, f, &args, actor)
}

// GET /rest/v1/rpc/{fn} — call a *read* fn with query-string arguments
// (§7.4, RFD 0012): the PostgREST stable-function symmetry. Values
// arrive as strings and parse by the declared parameter type; a value
// that does not parse passes through verbatim so the shared call path
// names the canonical `type_mismatch`. A `mut` fn refuses GET with 405 —
// a safe method must not write.
async fn rpc_get(
    State(app): State<Arc<App>>,
    Path(name): Path<String>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<JsonValue>, ApiError> {
    let f = resolve_fn(&app, &name)?;
    let actor = resolve_actor(&app, &headers);
    if !f.readonly {
        return Err(ApiError::method_not_allowed(format!(
            "fn `{name}` is `mut`; call it with POST"
        )));
    }
    let mut args: Map<String, JsonValue> = Map::new();
    for (key, raw) in params {
        // unknown keys carry no type and pass through as strings — the
        // shared call path rejects them by name
        let value = match f.params.iter().find(|p| p.name == key) {
            Some(p) => crate::value::text_to_json_scalar(app.contract.value_type(&p.ty), raw),
            None => JsonValue::String(raw),
        };
        args.insert(key, value);
    }
    run_rpc(&app, f, &args, actor)
}

fn resolve_fn<'a>(app: &'a App, name: &str) -> Result<&'a spock_lang::ir::FnDef, ApiError> {
    app.contract
        .fn_def(name)
        .ok_or_else(|| ApiError::not_found(format!("no fn `{name}` in this contract")))
}

/// The current actor from the `X-Spock-Actor` header (RFD 0014 §4.3), the
/// dev seam a verified JWT later fills. `None` (anonymous) when there is no
/// anchor, no header, or a value that does not parse as the anchor's key
/// type — canonicalized exactly like a by-key path segment so an uppercased
/// or braced uuid still matches the stored lowercase-canonical key. Never a
/// 404: a malformed header is anonymous, not an error.
pub(crate) fn resolve_actor(app: &App, headers: &HeaderMap) -> Option<SqlValue> {
    let anchor = app.contract.anchor()?;
    let raw = headers.get("x-spock-actor")?.to_str().ok()?;
    path_key_value(app, anchor, raw).ok()
}

/// The shared rpc execution tail: one locked call, arity-shaped envelope.
fn run_rpc(
    app: &App,
    f: &spock_lang::ir::FnDef,
    args: &Map<String, JsonValue>,
    actor: Option<SqlValue>,
) -> Result<Json<JsonValue>, ApiError> {
    let result = {
        let mut db = app
            .db
            .lock()
            .map_err(|_| ApiError::internal("db lock poisoned"))?;
        func::call(&app.contract, f, &mut db, args, actor)?
    };
    Ok(Json(match f.returns.arity {
        // list results share the REST list envelope
        FnArity::Many => json!({ "rows": result }),
        _ => result,
    }))
}

fn resolve_table<'a>(app: &'a App, name: &str) -> Result<&'a Table, ApiError> {
    app.contract
        .table(name)
        .ok_or_else(|| ApiError::not_found(format!("no table `{name}` in this contract")))
}

// GET /{table}?col=op.val&order=…&limit=…&offset=… — filtered, ordered,
// paged reads (RFD 0021 §6). The PostgREST operator grammar parses into the
// one predicate IR and runs through the same composer as the GraphQL floor.
// Query params are an ordered `Vec` (not a map): repeated keys — multiple
// column filters, multiple `order` terms — are honored in order.
async fn list_rows(
    State(app): State<Arc<App>>,
    Path(table): Path<String>,
    Query(params): Query<Vec<(String, String)>>,
) -> Result<Json<JsonValue>, ApiError> {
    let table = resolve_table(&app, &table)?;
    let q = filter::parse_rest(&app.contract, table, &params)?;

    let mut sql_params: Vec<SqlValue> = Vec::new();
    let where_sql = filter::lower_where(&q.predicate, &mut sql_params);
    filter::check_params(&sql_params)?;
    let order_sql = filter::lower_order(&q.order, &table.key);
    let sql = format!(
        "SELECT * FROM \"{}\" WHERE {where_sql} ORDER BY {order_sql} LIMIT {} OFFSET {}",
        table.name, q.limit, q.offset
    );

    let db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(sql_params.iter()))
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;

    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?
    {
        out.push(write::row_to_json(&app.contract, table, row)?);
    }
    Ok(Json(json!({ "rows": out })))
}

// GET /{table}/{id} — one row by key; single-key tables only (§8)
async fn get_row(
    State(app): State<Arc<App>>,
    Path((table, id)): Path<(String, String)>,
) -> Result<Json<JsonValue>, ApiError> {
    let table = resolve_table(&app, &table)?;
    let key = path_key_value(&app, table, &id)?;

    let db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    match write::select_by_key(&app.contract, table, &db, &[key])? {
        Some(row) => Ok(Json(row)),
        None => Err(ApiError::not_found(format!(
            "no {} row with this key",
            table.name
        ))),
    }
}

/// Interpret a path segment as the table's key value. Composite keys are not
/// addressable by path (§8 → 400); a malformed key value matches no row (404).
fn path_key_value(app: &App, table: &Table, raw: &str) -> Result<SqlValue, ApiError> {
    if table.key.len() != 1 {
        return Err(ApiError::bad_request(format!(
            "`{}` has a composite key; by-key access is unavailable in v0",
            table.name
        )));
    }
    let key_field = table
        .field(&table.key[0])
        .expect("checked: key field exists");
    let not_found = || ApiError::not_found(format!("no {} row with this key", table.name));
    match app.contract.value_type(&key_field.ty) {
        Type::Text => Ok(SqlValue::Text(raw.to_string())),
        Type::Uuid => uuid::Uuid::parse_str(raw)
            .map(|u| SqlValue::Text(u.to_string()))
            .map_err(|_| not_found()),
        Type::Int => raw
            .parse::<i64>()
            .map(SqlValue::Integer)
            .map_err(|_| not_found()),
        Type::Float => raw
            .parse::<f64>()
            .map(SqlValue::Real)
            .map_err(|_| not_found()),
        Type::Bool => match raw {
            "true" => Ok(SqlValue::Integer(1)),
            "false" => Ok(SqlValue::Integer(0)),
            _ => Err(not_found()),
        },
        Type::Timestamp => {
            use time::format_description::well_known::Rfc3339;
            time::OffsetDateTime::parse(raw, &Rfc3339)
                .map(|t| SqlValue::Text(crate::value::canon_timestamp(t)))
                .map_err(|_| not_found())
        }
        // A set type is never a key (checker E043), so a key never
        // bottoms out at one.
        Type::Set { .. } => unreachable!("a set type is never a key"),
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

//! The HTTP protocol (docs/spec/v0.md §8). Plain HTTP + JSON. The root
//! namespace is protocol-owned: `~` is the meta surface, `/rest/v1` carries
//! the open reads (identity views, v0 degenerate form), `/graphql/v1` the
//! GraphQL reads and writes (§8.2). REST tables stay read-only in v0;
//! `POST /rest/v1/rpc/{fn}` is the deliberate write surface (§7.4).

use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dynamic::Schema;
use async_graphql::http::GraphiQLSource;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::types::Value as SqlValue;
use serde_json::{json, Map, Value as JsonValue};
use spock_lang::ir::{FnArity, Table, Type};

use crate::error::ApiError;
use crate::graphql::{self, SchemaBuildError};
use crate::{func, write};
use crate::App;

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

/// Startup failures: schema-derivation collisions (§8.2 naming laws) or a
/// table claiming a protocol-owned REST segment. Both abort load — never
/// a request-time surprise.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(transparent)]
    Schema(#[from] SchemaBuildError),
    #[error("table `rpc` collides with the protocol-owned /rest/v1/rpc segment")]
    ReservedRestSegment,
}

pub fn router(app: Arc<App>) -> Result<Router, StartupError> {
    // `/rest/v1/rpc/{fn}` shadows `GET /rest/v1/rpc/{id}` for a table
    // literally named `rpc` — collisions fail startup, never requests
    if app.contract.table("rpc").is_some() {
        return Err(StartupError::ReservedRestSegment);
    }
    let schema = graphql::schema(app.clone())?;
    let gql = Router::new()
        .route("/graphql/v1", get(graphiql).post(graphql_post))
        .with_state(schema);
    Ok(Router::new()
        .route("/~contract", get(contract))
        .route("/~health", get(health))
        .route("/rest/v1/rpc/{name}", post(rpc_call))
        .route("/rest/v1/{table}", get(list_rows))
        .route("/rest/v1/{table}/{id}", get(get_row))
        .fallback(not_found)
        .with_state(app)
        .merge(gql))
}

/// Serve the app on an already-bound listener until the task is stopped.
/// A GraphQL schema-derivation failure (§8.2 naming laws) aborts startup.
pub async fn serve(app: Arc<App>, listener: tokio::net::TcpListener) -> std::io::Result<()> {
    let router = router(app).map_err(std::io::Error::other)?;
    axum::serve(listener, router).await
}

// POST /graphql/v1 — execute a query (§8.2); errors render as GraphQL's own
// `errors[]`, not the §8.1 envelope
async fn graphql_post(
    State(schema): State<Schema>,
    Json(request): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    Json(schema.execute(request).await)
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
    body: String,
) -> Result<Json<JsonValue>, ApiError> {
    let f = app
        .contract
        .fn_def(&name)
        .ok_or_else(|| ApiError::not_found(format!("no fn `{name}` in this contract")))?;
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
    let result = {
        let mut db = app
            .db
            .lock()
            .map_err(|_| ApiError::internal("db lock poisoned"))?;
        func::call(&app.contract, f, &mut db, &args)?
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

// GET /{table}?limit=N — list rows, key order, capped (§8)
async fn list_rows(
    State(app): State<Arc<App>>,
    Path(table): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<JsonValue>, ApiError> {
    let table = resolve_table(&app, &table)?;

    let limit = match params.get("limit") {
        None => DEFAULT_LIMIT,
        Some(raw) => raw
            .parse::<u32>()
            .map_err(|_| ApiError::bad_request("`limit` must be a non-negative integer"))?
            .min(MAX_LIMIT),
    };

    let order = table
        .key
        .iter()
        .map(|k| format!("\"{k}\" ASC"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT * FROM \"{}\" ORDER BY {order} LIMIT {limit}",
        table.name
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
        Type::Bool => match raw {
            "true" => Ok(SqlValue::Integer(1)),
            "false" => Ok(SqlValue::Integer(0)),
            _ => Err(not_found()),
        },
        Type::Timestamp => {
            use time::format_description::well_known::Rfc3339;
            time::OffsetDateTime::parse(raw, &Rfc3339)
                .map(|t| SqlValue::Text(t.format(&Rfc3339).expect("rfc3339 roundtrip")))
                .map_err(|_| not_found())
        }
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

//! The HTTP protocol (docs/spec/v0.md §8). Plain HTTP + JSON; `~` is the
//! meta surface; reads are the open surface in v0 degenerate form; writes
//! exist only on the dev surface until `fn` lands.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::types::Value as SqlValue;
use serde_json::{json, Value as JsonValue};
use spock_lang::ir::{Table, Type};

use crate::error::ApiError;
use crate::write;
use crate::App;

const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/~contract", get(contract))
        .route("/~health", get(health))
        .route("/~dev/{table}", post(dev_insert))
        .route("/~dev/{table}/{id}", delete(dev_delete))
        .route("/{table}", get(list_rows))
        .route("/{table}/{id}", get(get_row))
        .fallback(not_found)
        .with_state(app)
}

/// Serve the app on an already-bound listener until the task is stopped.
pub async fn serve(app: Arc<App>, listener: tokio::net::TcpListener) -> std::io::Result<()> {
    axum::serve(listener, router(app)).await
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

// POST /~dev/{table} — dev-surface insert (§7.2, §8)
async fn dev_insert(
    State(app): State<Arc<App>>,
    Path(table): Path<String>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let table = resolve_table(&app, &table)?;

    let parsed: JsonValue = serde_json::from_slice(&body)
        .map_err(|_| ApiError::bad_request("body is not valid JSON"))?;
    let JsonValue::Object(object) = parsed else {
        return Err(ApiError::bad_request("body must be a JSON object"));
    };

    let mut db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let row = write::insert_row(&app.contract, table, &mut db, &object)?;
    Ok((StatusCode::CREATED, Json(row)).into_response())
}

// DELETE /~dev/{table}/{id} — dev-surface delete (§7.2, §8)
async fn dev_delete(
    State(app): State<Arc<App>>,
    Path((table, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let table = resolve_table(&app, &table)?;
    let key = path_key_value(&app, table, &id)?;

    let mut db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    write::delete_row(&app.contract, table, &mut db, &key)?;
    Ok(StatusCode::NO_CONTENT)
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

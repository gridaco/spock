//! The storage byte plane (RFD 0018): a Supabase-shaped HTTP protocol for
//! minting signed upload/download URLs and transferring object bytes, on the
//! open floor beside `/rest` and `/graphql`. Active only when the contract
//! carries the injected `storage_object` table.
//!
//! - `POST /storage/v1/object/upload/sign` — mint a pending object + a signed
//!   PUT URL (Supabase `createSignedUploadUrl`).
//! - `PUT  /storage/v1/object/{id}?exp&sig` — receive bytes, flip
//!   `pending → committed` atomically with the blob write (`uploadToSignedUrl`).
//! - `POST /storage/v1/object/sign/{id}` — mint a signed GET URL
//!   (`createSignedUrl`).
//! - `GET  /storage/v1/object/{id}?exp&sig` — serve bytes (signed download).
//!
//! The metadata row is authoritative; the bytes are reconciled to it by the
//! sweep (§ `sweep`).

pub mod blob;
pub mod sign;

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use spock_lang::ir::{Contract, STORAGE_OBJECT_TABLE};

use crate::error::ApiError;
use crate::write::sqlite_internal;
use crate::{http, write, App};

/// How long a minted upload URL is valid.
const UPLOAD_TTL_SECS: i64 = 600;
/// How long a minted download URL is valid.
const DOWNLOAD_TTL_SECS: i64 = 600;
/// The largest object body accepted on a signed PUT (25 MiB). Enforced at the
/// router boundary (413 before any byte is hashed).
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Whether the storage surface is active — the contract carries the builtin
/// `storage_object` table (RFD 0018). Gates the protocol routes and the blob
/// store, exactly as `contract.anchor()` gates `spock_actor()`.
pub fn storage_active(contract: &Contract) -> bool {
    contract.table(STORAGE_OBJECT_TABLE).is_some()
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

/// The sha256 of `bytes`, hex-encoded — an object's explicit checksum. Shared by
/// the signed-PUT path and the `file()` seed loader (RFD 0018).
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// `POST /storage/v1/object/upload/sign` — mint a pending object and a signed
/// PUT URL. The object is created through the shared write path, so its
/// defaults (`id = auto`, `state = pending`, `created_at = now`, `owner = me`)
/// apply exactly as a floor insert would; the client body is ignored — content
/// type and size are recorded when the bytes arrive.
pub async fn post_upload_sign(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
    let table = app
        .contract
        .table(STORAGE_OBJECT_TABLE)
        .ok_or_else(|| ApiError::internal("storage is not active for this contract"))?;
    let actor = http::resolve_actor(&app, &headers);
    let row = {
        let mut db = app
            .db
            .lock()
            .map_err(|_| ApiError::internal("db lock poisoned"))?;
        write::insert_row(&app.contract, table, &mut db, &Map::new(), actor)?
    };
    let id = row
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::internal("mint produced no id"))?
        .to_string();
    let url = app.signer.url("PUT", &id, now_unix(), UPLOAD_TTL_SECS);
    Ok(Json(json!({ "id": id, "url": url })))
}

/// `PUT /storage/v1/object/{id}?exp&sig` — receive bytes for a pending object.
/// The blob write and the `pending → committed` flip commit in one SQLite
/// transaction, so a committed object always has its bytes (no S3-style
/// "200 with an embedded error" window).
pub async fn put_object(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    verify(&app, "PUT", &id, &params)?;

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let size = body.len() as i64;
    let checksum = sha256_hex(&body);

    let mut db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_internal)?;
    // Commit the metadata only for an object still awaiting its upload. Any
    // other state (already committed, or gone) matches zero rows → 409.
    let flipped = tx
        .execute(
            &format!(
                "UPDATE \"{STORAGE_OBJECT_TABLE}\" \
                 SET \"state\" = 'committed', \"size\" = ?1, \"checksum\" = ?2, \
                     \"content_type\" = ?3 \
                 WHERE \"id\" = ?4 AND \"state\" = 'pending'"
            ),
            rusqlite::params![size, checksum, content_type, id],
        )
        .map_err(sqlite_internal)?;
    if flipped != 1 {
        return Err(ApiError::conflict(format!(
            "object `{id}` is not awaiting an upload"
        )));
    }
    app.blobs
        .put(&tx, &id, &body)
        .map_err(|e| ApiError::internal(format!("blob write: {e}")))?;
    tx.commit().map_err(sqlite_internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /storage/v1/object/sign/{id}` — mint a signed GET URL for a committed
/// object (Supabase `createSignedUrl`). Where a caller is allowed to reach this
/// is where read authorization will live (v1 `policy`).
pub async fn post_download_sign(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Json<JsonValue>, ApiError> {
    let meta = {
        let db = app
            .db
            .lock()
            .map_err(|_| ApiError::internal("db lock poisoned"))?;
        object_meta(&db, &id).map_err(sqlite_internal)?
    };
    let Some((state, _)) = meta else {
        return Err(ApiError::not_found(format!("no storage object `{id}`")));
    };
    if state != "committed" {
        return Err(ApiError::conflict(format!(
            "object `{id}` has no committed bytes to serve"
        )));
    }
    let url = app.signer.url("GET", &id, now_unix(), DOWNLOAD_TTL_SECS);
    Ok(Json(json!({ "url": url })))
}

/// `GET /storage/v1/object/{id}?exp&sig` — serve the bytes of a committed
/// object with its recorded content type. A pending or absent object is a 404.
pub async fn get_object(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    verify(&app, "GET", &id, &params)?;

    let db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let (state, content_type) = object_meta(&db, &id)
        .map_err(sqlite_internal)?
        .ok_or_else(|| ApiError::not_found(format!("no storage object `{id}`")))?;
    if state != "committed" {
        return Err(ApiError::not_found(format!(
            "object `{id}` has no bytes yet"
        )));
    }
    let bytes = app
        .blobs
        .get(&db, &id)
        .map_err(|e| ApiError::internal(format!("blob read: {e}")))?
        .ok_or_else(|| ApiError::not_found(format!("object `{id}` has no bytes")))?;
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    Ok(([(header::CONTENT_TYPE, content_type)], bytes).into_response())
}

/// Verify a signed request: the HMAC must match (method, id, exp) and the
/// expiry must be in the future. Fail-safe — any problem is 401.
fn verify(
    app: &App,
    method: &str,
    id: &str,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    let exp = params
        .get("exp")
        .and_then(|s| s.parse::<i64>().ok())
        .ok_or_else(|| ApiError::unauthorized("missing or malformed `exp`"))?;
    let sig = params
        .get("sig")
        .ok_or_else(|| ApiError::unauthorized("missing signature"))?;
    if exp < now_unix() {
        return Err(ApiError::unauthorized("signed URL has expired"));
    }
    if !app.signer.verify(method, id, exp, sig) {
        return Err(ApiError::unauthorized("invalid signature"));
    }
    Ok(())
}

/// The `(state, content_type)` of an object, or `None` if it does not exist.
fn object_meta(conn: &Connection, id: &str) -> rusqlite::Result<Option<(String, Option<String>)>> {
    conn.query_row(
        &format!(
            "SELECT \"state\", \"content_type\" FROM \"{STORAGE_OBJECT_TABLE}\" WHERE \"id\" = ?1"
        ),
        [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
    )
    .optional()
}

/// A pending object older than this is treated as abandoned (never uploaded).
const PENDING_TTL_MINS: i64 = 30;
/// A committed object unreferenced for longer than this is reclaimed — the
/// grace window covers the gap between a client's commit and its attach.
const COMMITTED_GRACE_MINS: i64 = 30;

/// Reclaim orphaned objects (RFD 0018 §1.6) at the configured TTLs. Runs on the
/// in-process ticker; a subprocess or startup sweep would be incoherent, since
/// the database is disposable and recreated per run.
pub fn sweep(app: &App) -> Result<usize, ApiError> {
    let now = time::OffsetDateTime::now_utc();
    let pending_cutoff =
        crate::value::canon_timestamp(now - time::Duration::minutes(PENDING_TTL_MINS));
    let committed_cutoff =
        crate::value::canon_timestamp(now - time::Duration::minutes(COMMITTED_GRACE_MINS));
    sweep_before(app, &pending_cutoff, &committed_cutoff)
}

/// The sweep with explicit cutoffs (canonical timestamps) — the testable core.
/// The two orphan classes: *unattached pending* (`pending` older than
/// `pending_cutoff`) and *unreferenced committed* (`committed` older than
/// `committed_cutoff`). Only objects with **no** inbound `storage_object`
/// reference are eligible, so an attached object is never collected, whatever
/// its age. The metadata row is authoritative; the blob is deleted alongside it.
pub fn sweep_before(
    app: &App,
    pending_cutoff: &str,
    committed_cutoff: &str,
) -> Result<usize, ApiError> {
    let unref = unreferenced_predicate(&app.contract);
    let mut db = app
        .db
        .lock()
        .map_err(|_| ApiError::internal("db lock poisoned"))?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_internal)?;
    let victims: Vec<String> = {
        let sql = format!(
            "SELECT \"id\" FROM \"{STORAGE_OBJECT_TABLE}\" \
             WHERE ({unref}) AND ( \
               (\"state\" = 'pending'   AND \"created_at\" < ?1) OR \
               (\"state\" = 'committed' AND \"created_at\" < ?2) )"
        );
        let mut stmt = tx.prepare(&sql).map_err(sqlite_internal)?;
        let rows = stmt
            .query_map(rusqlite::params![pending_cutoff, committed_cutoff], |r| {
                r.get::<_, String>(0)
            })
            .map_err(sqlite_internal)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_internal)?
    };
    for id in &victims {
        // Delete the bytes explicitly (keeping the sweep substrate-agnostic —
        // an object store would delete the remote object), then the row. The
        // blob table's `ON DELETE CASCADE` is a backstop for the SQLite store.
        app.blobs
            .delete(&tx, id)
            .map_err(|e| ApiError::internal(format!("blob delete: {e}")))?;
        tx.execute(
            &format!("DELETE FROM \"{STORAGE_OBJECT_TABLE}\" WHERE \"id\" = ?1"),
            [id],
        )
        .map_err(sqlite_internal)?;
    }
    tx.commit().map_err(sqlite_internal)?;
    Ok(victims.len())
}

/// The "no inbound `storage_object` reference points at this row" SQL predicate,
/// built from the contract's inbound refs. Vacuously true when nothing
/// references storage objects (then only the TTLs protect fresh uploads).
fn unreferenced_predicate(contract: &Contract) -> String {
    let refs = contract.inbound_refs(STORAGE_OBJECT_TABLE);
    if refs.is_empty() {
        return "1".to_string();
    }
    refs.iter()
        .map(|(child, field)| {
            format!(
                "NOT EXISTS (SELECT 1 FROM \"{}\" WHERE \"{}\" = \"{STORAGE_OBJECT_TABLE}\".\"id\")",
                child.name, field.name
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// How often the running server reclaims orphans.
const SWEEP_INTERVAL_SECS: u64 = 60;

/// Reclaim orphaned objects on a fixed interval for the life of the server
/// (RFD 0018 §1.6). `serve` spawns this for a storage contract; it never returns
/// and is aborted when the server's runtime is dropped. In-process because
/// orphans exist only inside a running server and the database is disposable.
pub(crate) async fn sweep_loop(app: Arc<App>) {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(SWEEP_INTERVAL_SECS));
    loop {
        tick.tick().await;
        if let Err(e) = sweep(&app) {
            eprintln!("storage sweep: {e}");
        }
    }
}

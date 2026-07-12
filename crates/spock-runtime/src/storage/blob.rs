//! The byte backend (RFD 0018 §1.5). A pluggable `BlobStore`; the v0 impl keeps
//! bytes in a SQLite BLOB so a byte write and the `pending → committed` flip
//! commit in one transaction (the byte/metadata atomicity S3 and Supabase both
//! leave open), and the store is disposable with the `.db`. A sidecar-dir or
//! object-store impl drops in behind the same trait — the deferred D3 swap.

use rusqlite::{Connection, OptionalExtension};
use spock_lang::ir::STORAGE_OBJECT_TABLE;

/// The physical table holding object bytes. **Not** a contract table, so it is
/// invisible to REST/GraphQL/codegen; its `object_id` cascades from
/// `storage_object`, so deleting an object row reclaims its bytes.
const STORAGE_BLOB_TABLE: &str = "storage_blob";

pub trait BlobStore: Send + Sync {
    /// Create the backing store. Called once at load.
    fn init(&self, conn: &Connection) -> rusqlite::Result<()>;
    /// Persist `bytes` for object `id`. The `conn` handle lets a substrate that
    /// can enlist in the SQLite transaction (this impl) be atomic with the
    /// metadata flip; an object-store impl would ignore it and lean on the
    /// sweep for byte orphans.
    fn put(&self, conn: &Connection, id: &str, bytes: &[u8]) -> rusqlite::Result<()>;
    /// Fetch the bytes for `id`, or `None` if absent.
    fn get(&self, conn: &Connection, id: &str) -> rusqlite::Result<Option<Vec<u8>>>;
    /// Remove the bytes for `id` (idempotent).
    fn delete(&self, conn: &Connection, id: &str) -> rusqlite::Result<()>;
}

/// The byte backend for this build — the single D3 swap point (RFD 0018 §1.5).
/// Load-time (blob-table init, `file()` seed) and serve-time (`App`) both take
/// their store from here, so they provably share one backend; swapping the
/// substrate is this one line.
pub fn default_blob_store() -> Box<dyn BlobStore> {
    Box::new(SqliteBlobStore)
}

/// v0: bytes in a SQLite BLOB, keyed by object id. Stateless.
pub struct SqliteBlobStore;

impl BlobStore for SqliteBlobStore {
    fn init(&self, conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS \"{STORAGE_BLOB_TABLE}\" (\
               \"object_id\" TEXT PRIMARY KEY \
                 REFERENCES \"{STORAGE_OBJECT_TABLE}\" (\"id\") ON DELETE CASCADE, \
               \"bytes\" BLOB NOT NULL\
             )"
        ))
    }

    fn put(&self, conn: &Connection, id: &str, bytes: &[u8]) -> rusqlite::Result<()> {
        conn.execute(
            &format!(
                "INSERT INTO \"{STORAGE_BLOB_TABLE}\" (\"object_id\", \"bytes\") VALUES (?1, ?2) \
                 ON CONFLICT(\"object_id\") DO UPDATE SET \"bytes\" = excluded.\"bytes\""
            ),
            rusqlite::params![id, bytes],
        )?;
        Ok(())
    }

    fn get(&self, conn: &Connection, id: &str) -> rusqlite::Result<Option<Vec<u8>>> {
        conn.query_row(
            &format!("SELECT \"bytes\" FROM \"{STORAGE_BLOB_TABLE}\" WHERE \"object_id\" = ?1"),
            [id],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
    }

    fn delete(&self, conn: &Connection, id: &str) -> rusqlite::Result<()> {
        conn.execute(
            &format!("DELETE FROM \"{STORAGE_BLOB_TABLE}\" WHERE \"object_id\" = ?1"),
            [id],
        )?;
        Ok(())
    }
}

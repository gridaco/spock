//! Engine bootstrap (docs/spec/v0.md §7.1, §7.3): open SQLite, materialize
//! the schema fresh, replay the seed through the write path. There are no
//! migrations in v0 — state is disposable by doctrine (RFD 0008 §3).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Map, Value as Json};
use spock_lang::ddl::ddl;
use spock_lang::ir::{Contract, SeedValue};

use crate::error::ApiError;
use crate::write::insert_row;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not reset database file: {0}")]
    Io(#[from] std::io::Error),
    #[error("seed row {index} ({table}): {source}")]
    Seed {
        index: usize,
        table: String,
        source: Box<ApiError>,
    },
}

/// Open the database (in-memory when `path` is `None`), materialize the
/// schema, and replay the seed. A file database is recreated from scratch:
/// no migrations in v0.
pub fn open(contract: &Contract, path: Option<&Path>) -> Result<Connection, EngineError> {
    let mut conn = match path {
        None => Connection::open_in_memory()?,
        Some(p) => {
            for suffix in ["", "-wal", "-shm"] {
                let candidate = if suffix.is_empty() {
                    p.to_path_buf()
                } else {
                    p.with_file_name(format!(
                        "{}{suffix}",
                        p.file_name().and_then(|n| n.to_str()).unwrap_or("spock.db")
                    ))
                };
                if candidate.exists() {
                    std::fs::remove_file(&candidate)?;
                }
            }
            Connection::open(p)?
        }
    };

    conn.pragma_update(None, "foreign_keys", true)?;
    for statement in ddl(contract) {
        conn.execute_batch(&statement)?;
    }
    seed(contract, &mut conn)?;
    Ok(conn)
}

/// Replay the seed (§7.3) through the same write path as the dev surface.
fn seed(contract: &Contract, conn: &mut Connection) -> Result<(), EngineError> {
    // binding name -> the bound row's key value (as JSON)
    let mut bindings: HashMap<String, Json> = HashMap::new();

    for (index, row) in contract.seed.iter().enumerate() {
        let table = contract
            .table(&row.table)
            .expect("checked: seed tables exist");

        let mut body = Map::new();
        for (name, value) in &row.fields {
            let json = match value {
                SeedValue::Str(s) => Json::String(s.clone()),
                SeedValue::Int(n) => Json::Number((*n).into()),
                SeedValue::Bool(b) => Json::Bool(*b),
                SeedValue::Ref { binding } => bindings
                    .get(binding)
                    .cloned()
                    .expect("checked: bindings resolve"),
            };
            body.insert(name.clone(), json);
        }

        let stored =
            insert_row(contract, table, conn, &body).map_err(|source| EngineError::Seed {
                index,
                table: table.name.clone(),
                source: Box::new(source),
            })?;

        if let Some(binding) = &row.binding {
            if let Some(key_field) = table.key.first() {
                if table.key.len() == 1 {
                    let key_value = stored
                        .get(key_field)
                        .cloned()
                        .expect("stored row has its key");
                    bindings.insert(binding.clone(), key_value);
                }
            }
        }
    }
    Ok(())
}

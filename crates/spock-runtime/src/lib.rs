//! The Spock v0 runtime (docs/spec/v0.md §7–§8).
//!
//! Loads a compiled contract, materializes it into embedded SQLite, replays
//! the seed through the contract's own write path, and serves the HTTP
//! protocol. Database access is serialized through one connection — a
//! documented v0 prototype property (§7.2).

pub mod engine;
pub mod error;
pub mod func;
pub mod graphql;
pub mod http;
pub mod storage;
pub mod value;
pub mod write;

use std::sync::Mutex;

use rusqlite::Connection;
use spock_lang::ir::Contract;

use crate::storage::blob::{default_blob_store, BlobStore};
use crate::storage::sign::Signer;

/// Shared server state: the contract, the (serialized) database, and — for a
/// contract that uses storage (RFD 0018) — the per-run URL signer and the byte
/// backend. The signer and store are always present but only ever touched when
/// the storage surface is active, so `App::new` keeps its two-argument shape.
pub struct App {
    pub contract: Contract,
    pub db: Mutex<Connection>,
    pub signer: Signer,
    pub blobs: Box<dyn BlobStore>,
}

impl App {
    pub fn new(contract: Contract, db: Connection) -> Self {
        App {
            contract,
            db: Mutex::new(db),
            signer: Signer::random(),
            blobs: default_blob_store(),
        }
    }
}

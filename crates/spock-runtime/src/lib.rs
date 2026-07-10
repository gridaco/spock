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
pub mod value;
pub mod write;

use std::sync::Mutex;

use rusqlite::Connection;
use spock_lang::ir::Contract;

/// Shared server state: the contract and the (serialized) database.
pub struct App {
    pub contract: Contract,
    pub db: Mutex<Connection>,
}

impl App {
    pub fn new(contract: Contract, db: Connection) -> Self {
        App {
            contract,
            db: Mutex::new(db),
        }
    }
}

//! One immutable Spock authority generation.
//!
//! A generation binds a checked contract to the database, signer, blob store,
//! authority router, and background-task lifecycle that serve it. Project
//! observation and replacement policy deliberately live above this crate: the
//! runtime can construct a generation from already captured bytes, but it does
//! not watch or reread those inputs.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use axum::Router;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use spock_lang::diag::Diagnostic;
use spock_lang::ir::Contract;

use crate::engine::{self, EngineError};
use crate::http::{self, StartupError};
use crate::App;

const LIFECYCLE_DORMANT: u8 = 0;
const LIFECYCLE_RUNNING: u8 = 1;
const LIFECYCLE_STOPPED: u8 = 2;

/// Exact source and seed-asset bytes captured by the project layer.
///
/// Paths are the checked `file("...")` spellings carried by the contract.
/// A `BTreeMap` makes the input fingerprint independent of insertion order.
/// Construction performs no filesystem access; callers are responsible for
/// coherently capturing the complete bundle before handing it to the runtime.
#[derive(Clone, Debug)]
pub struct CapturedBackend {
    source: Arc<[u8]>,
    seed_assets: BTreeMap<String, Arc<[u8]>>,
    input_fingerprint: CapturedInputFingerprint,
}

impl CapturedBackend {
    pub fn new(source: impl AsRef<[u8]>, seed_assets: BTreeMap<String, Vec<u8>>) -> Self {
        let source: Arc<[u8]> = Arc::from(source.as_ref());
        let seed_assets: BTreeMap<String, Arc<[u8]>> = seed_assets
            .into_iter()
            .map(|(path, bytes)| (path, Arc::from(bytes)))
            .collect();
        let input_fingerprint = captured_input_fingerprint(&source, &seed_assets);
        Self {
            source,
            seed_assets,
            input_fingerprint,
        }
    }

    pub fn without_assets(source: impl AsRef<[u8]>) -> Self {
        Self::new(source, BTreeMap::new())
    }

    pub fn source(&self) -> &[u8] {
        &self.source
    }

    pub fn seed_asset(&self, path: &str) -> Option<&[u8]> {
        self.seed_assets.get(path).map(AsRef::as_ref)
    }

    pub fn input_fingerprint(&self) -> &CapturedInputFingerprint {
        &self.input_fingerprint
    }
}

/// SHA-256 identity of the exact captured source and seed-asset bundle.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CapturedInputFingerprint(String);

impl CapturedInputFingerprint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapturedInputFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// SHA-256 identity of the canonical serialized checked contract.
///
/// This intentionally excludes materialized rows and seed-file bytes. Those
/// belong to the captured-input/world identity, while this value describes the
/// authority contract exposed to linkers and protocol consumers.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContractFingerprint(String);

impl ContractFingerprint {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContractFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An immutable checked authority generation.
///
/// The `App` and both fingerprints never change. Lifecycle state is a one-shot
/// ownership latch: at most one background-task guard can exist for a
/// generation, preventing duplicate storage sweepers.
pub struct BackendGeneration {
    app: Arc<App>,
    contract_fingerprint: ContractFingerprint,
    input_fingerprint: Option<CapturedInputFingerprint>,
    lifecycle: Arc<AtomicU8>,
}

impl BackendGeneration {
    /// Compile and materialize a generation solely from already captured
    /// source and seed-asset bytes.
    pub fn from_captured(
        captured: CapturedBackend,
        database_path: Option<&Path>,
    ) -> Result<Self, BackendGenerationError> {
        let source =
            std::str::from_utf8(captured.source()).map_err(BackendGenerationError::SourceUtf8)?;
        let contract = spock_lang::compile(source).map_err(BackendGenerationError::Compile)?;
        let conn =
            engine::open_with_captured_assets(&contract, database_path, &captured.seed_assets)?;
        Ok(Self::from_parts(
            contract,
            conn,
            Some(captured.input_fingerprint),
        ))
    }

    /// Wrap an already constructed app. This compatibility seam lets existing
    /// embedders adopt generation-owned routing/lifecycle independently of the
    /// captured-input constructor.
    pub fn from_app(app: Arc<App>) -> Self {
        let contract_fingerprint = contract_fingerprint(&app.contract);
        Self {
            app,
            contract_fingerprint,
            input_fingerprint: None,
            lifecycle: Arc::new(AtomicU8::new(LIFECYCLE_DORMANT)),
        }
    }

    fn from_parts(
        contract: Contract,
        conn: Connection,
        input_fingerprint: Option<CapturedInputFingerprint>,
    ) -> Self {
        let app = Arc::new(App::new(contract, conn));
        let contract_fingerprint = contract_fingerprint(&app.contract);
        Self {
            app,
            contract_fingerprint,
            input_fingerprint,
            lifecycle: Arc::new(AtomicU8::new(LIFECYCLE_DORMANT)),
        }
    }

    pub fn app(&self) -> Arc<App> {
        Arc::clone(&self.app)
    }

    pub fn contract(&self) -> &Contract {
        &self.app.contract
    }

    pub fn contract_fingerprint(&self) -> &ContractFingerprint {
        &self.contract_fingerprint
    }

    pub fn input_fingerprint(&self) -> Option<&CapturedInputFingerprint> {
        self.input_fingerprint.as_ref()
    }

    /// Routes owned by the authority runtime only. There is deliberately no
    /// fallback or CORS layer: the framework host owns application-wide route
    /// partition, fallback, and cross-origin policy.
    pub fn authority_router(&self) -> Result<Router, StartupError> {
        http::authority_router(self.app())
    }

    /// Start generation-owned background work exactly once.
    ///
    /// Dropping or explicitly shutting down the returned guard aborts every
    /// task. A stopped generation cannot be restarted; construct a new
    /// generation instead.
    pub fn start_background_tasks(&self) -> Result<BackendLifecycle, BackendLifecycleError> {
        let runtime = if crate::storage::storage_active(&self.app.contract) {
            Some(
                tokio::runtime::Handle::try_current()
                    .map_err(|_| BackendLifecycleError::NoRuntime)?,
            )
        } else {
            None
        };
        self.lifecycle
            .compare_exchange(
                LIFECYCLE_DORMANT,
                LIFECYCLE_RUNNING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| BackendLifecycleError::AlreadyStarted)?;

        let mut tasks = Vec::new();
        if let Some(runtime) = runtime {
            tasks.push(runtime.spawn(crate::storage::sweep_loop(self.app())));
        }
        Ok(BackendLifecycle {
            state: Arc::clone(&self.lifecycle),
            tasks,
            released: false,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackendLifecycleError {
    #[error("backend generation lifecycle has already been started")]
    AlreadyStarted,
    #[error("a Tokio runtime is required to start storage background tasks")]
    NoRuntime,
}

/// Explicit ownership of a generation's background tasks.
pub struct BackendLifecycle {
    state: Arc<AtomicU8>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    released: bool,
}

impl BackendLifecycle {
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub async fn shutdown(mut self) {
        self.abort_tasks();
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        self.release();
    }

    fn abort_tasks(&self) {
        for task in &self.tasks {
            task.abort();
        }
    }

    fn release(&mut self) {
        if !self.released {
            self.state.store(LIFECYCLE_STOPPED, Ordering::Release);
            self.released = true;
        }
    }
}

impl Drop for BackendLifecycle {
    fn drop(&mut self) {
        self.abort_tasks();
        self.release();
    }
}

#[derive(Debug)]
pub enum BackendGenerationError {
    SourceUtf8(std::str::Utf8Error),
    Compile(Vec<Diagnostic>),
    Engine(EngineError),
}

impl BackendGenerationError {
    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        match self {
            Self::Compile(diagnostics) => Some(diagnostics),
            _ => None,
        }
    }
}

impl fmt::Display for BackendGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceUtf8(error) => write!(f, "backend source is not UTF-8: {error}"),
            Self::Compile(diagnostics) => write!(
                f,
                "backend source has {} diagnostic(s); generation not constructed",
                diagnostics.len()
            ),
            Self::Engine(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BackendGenerationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceUtf8(error) => Some(error),
            Self::Compile(_) => None,
            Self::Engine(error) => Some(error),
        }
    }
}

impl From<EngineError> for BackendGenerationError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

fn contract_fingerprint(contract: &Contract) -> ContractFingerprint {
    let bytes = serde_json::to_vec(contract).expect("checked contracts always serialize");
    let mut hasher = Sha256::new();
    hasher.update(b"spock-contract/0\0");
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    ContractFingerprint(hex::encode(hasher.finalize()))
}

fn captured_input_fingerprint(
    source: &[u8],
    assets: &BTreeMap<String, Arc<[u8]>>,
) -> CapturedInputFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(b"spock-backend-input/0\0");
    hash_part(&mut hasher, source);
    hasher.update((assets.len() as u64).to_be_bytes());
    for (path, bytes) in assets {
        hash_part(&mut hasher, path.as_bytes());
        hash_part(&mut hasher, bytes);
    }
    CapturedInputFingerprint(hex::encode(hasher.finalize()))
}

fn hash_part(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORAGE_SOURCE: &str = "auth table user { key id: uuid = auto\n \
        username: text unique\n avatar: storage_object? }\n\
        seed { user { username: \"u\", avatar: file(\"./pic.png\") } }\n";

    #[test]
    fn captured_inputs_have_order_independent_fingerprints() {
        let mut first = BTreeMap::new();
        first.insert("./b.txt".to_string(), b"b".to_vec());
        first.insert("./a.txt".to_string(), b"a".to_vec());
        let mut second = BTreeMap::new();
        second.insert("./a.txt".to_string(), b"a".to_vec());
        second.insert("./b.txt".to_string(), b"b".to_vec());

        let first = CapturedBackend::new("// source", first);
        let second = CapturedBackend::new("// source", second);
        assert_eq!(first.input_fingerprint(), second.input_fingerprint());
    }

    #[test]
    fn captured_assets_are_the_only_seed_bytes_materialized() {
        let payload = b"captured-payload".to_vec();
        let captured = CapturedBackend::new(
            STORAGE_SOURCE,
            BTreeMap::from([("./pic.png".to_string(), payload.clone())]),
        );
        let expected_input = captured.input_fingerprint().clone();
        let generation = BackendGeneration::from_captured(captured, None).expect("generation");

        assert_eq!(generation.input_fingerprint(), Some(&expected_input));
        let db = generation.app.db.lock().expect("db lock");
        let stored: Vec<u8> = db
            .query_row("SELECT bytes FROM storage_blob", [], |row| row.get(0))
            .expect("captured blob");
        assert_eq!(stored, payload);
    }

    #[test]
    fn missing_captured_asset_never_falls_back_to_the_filesystem() {
        let error = match BackendGeneration::from_captured(
            CapturedBackend::without_assets(STORAGE_SOURCE),
            None,
        ) {
            Ok(_) => panic!("generation unexpectedly materialized a missing captured asset"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("captured seed asset `./pic.png` is missing"),
            "{error}"
        );
    }

    #[test]
    fn contract_and_input_fingerprints_have_distinct_meanings() {
        let first = BackendGeneration::from_captured(
            CapturedBackend::without_assets("table t { key id: uuid = auto }"),
            None,
        )
        .expect("first generation");
        let second = BackendGeneration::from_captured(
            CapturedBackend::without_assets(
                "// input-only comment\ntable t { key id: uuid = auto }",
            ),
            None,
        )
        .expect("second generation");

        assert_eq!(first.contract_fingerprint(), second.contract_fingerprint());
        assert_ne!(first.input_fingerprint(), second.input_fingerprint());
    }

    #[tokio::test]
    async fn lifecycle_is_one_shot_and_explicitly_stoppable() {
        let generation = BackendGeneration::from_captured(
            CapturedBackend::new(
                STORAGE_SOURCE,
                BTreeMap::from([("./pic.png".to_string(), b"bytes".to_vec())]),
            ),
            None,
        )
        .expect("generation");

        let lifecycle = generation.start_background_tasks().expect("starts once");
        assert_eq!(lifecycle.task_count(), 1);
        assert!(generation.start_background_tasks().is_err());
        lifecycle.shutdown().await;
        assert!(generation.start_background_tasks().is_err());
    }

    #[test]
    fn storage_lifecycle_without_a_runtime_is_a_clean_error() {
        let generation = BackendGeneration::from_captured(
            CapturedBackend::new(
                STORAGE_SOURCE,
                BTreeMap::from([("./pic.png".to_string(), b"bytes".to_vec())]),
            ),
            None,
        )
        .expect("generation");

        assert!(matches!(
            generation.start_background_tasks(),
            Err(BackendLifecycleError::NoRuntime)
        ));
    }
}

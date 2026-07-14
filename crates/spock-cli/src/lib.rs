//! Reusable implementation of Spock's file-oriented commands.
//!
//! This crate deliberately keeps argument parsing and terminal presentation in
//! the `spock` binary. Consumers can load one `.spock` file, perform the same
//! full-load proof as `spock check`, derive build/codegen artifacts, or prepare
//! the historical standalone server without going through Clap or spawning a
//! child process.

mod project_commands;
mod write_plan;

pub use project_commands::{
    check_target, create_project, init_project, resolve_project_for_serve, CheckTargetError,
    CheckTargetSummary, NewProjectNameError, ProjectWriteError, ProjectWriteOperation,
    ProjectWriteSummary, ResolveProjectForServeError,
};

pub use write_plan::{
    apply_write_plan, ApplyError, ApplyStage, ApplySummary, CreatedPathKind, RollbackReport,
    RollbackResidual, RootPolicy,
};

use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use spock_lang::diag::Diagnostic;
use spock_lang::ir::Contract;

/// A source file and the checked contract derived from its exact contents.
#[derive(Debug)]
pub struct FileProgram {
    /// The caller's spelling, retained for byte-compatible diagnostics.
    path: PathBuf,
    /// The path used for I/O and relative `file(...)` seed assets.
    read_path: PathBuf,
    source: String,
    contract: Contract,
}

impl FileProgram {
    /// Read and compile one `.spock` source file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProgramLoadError> {
        let path = path.as_ref();
        Self::load_from(path, path)
    }

    /// Load a caller-spelled path as though the process were running in `cwd`.
    ///
    /// Project target resolution canonicalizes directories, but standalone
    /// file mode must not canonicalize the final `.spock` component: doing so
    /// changes both rendered diagnostics and the directory used by seed
    /// `file(...)` references when the source itself is a symlink.
    pub(crate) fn load_from_cwd(path: &Path, cwd: &Path) -> Result<Self, ProgramLoadError> {
        let read_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };
        Self::load_from(path, &read_path)
    }

    fn load_from(display_path: &Path, read_path: &Path) -> Result<Self, ProgramLoadError> {
        let path = display_path.to_path_buf();
        let read_path = read_path.to_path_buf();
        let source =
            std::fs::read_to_string(&read_path).map_err(|error| ProgramLoadError::Read {
                path: path.clone(),
                error,
            })?;
        let contract =
            spock_lang::compile(&source).map_err(|diagnostics| ProgramLoadError::Diagnostics {
                path: path.clone(),
                source: source.clone(),
                diagnostics,
            })?;
        Ok(Self {
            path,
            read_path,
            source,
            contract,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_dir(&self) -> PathBuf {
        source_dir(&self.read_path)
    }

    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    pub fn into_contract(self) -> Contract {
        self.contract
    }
}

/// Failure to read or compile a file program.
#[derive(Debug)]
pub enum ProgramLoadError {
    Read {
        path: PathBuf,
        error: std::io::Error,
    },
    Diagnostics {
        path: PathBuf,
        source: String,
        diagnostics: Vec<Diagnostic>,
    },
}

impl ProgramLoadError {
    pub fn diagnostics(&self) -> Option<&[Diagnostic]> {
        match self {
            Self::Diagnostics { diagnostics, .. } => Some(diagnostics),
            Self::Read { .. } => None,
        }
    }
}

impl fmt::Display for ProgramLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, error } => {
                write!(f, "error: could not read {}: {error}", path.display())
            }
            Self::Diagnostics {
                path,
                source,
                diagnostics,
            } => {
                let display_path = path.display().to_string();
                for diagnostic in diagnostics {
                    writeln!(f, "{}", diagnostic.render(source, &display_path))?;
                }
                write!(
                    f,
                    "error: {} diagnostic(s), contract not produced",
                    diagnostics.len()
                )
            }
        }
    }
}

impl std::error::Error for ProgramLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { error, .. } => Some(error),
            Self::Diagnostics { .. } => None,
        }
    }
}

/// The directory a `.spock` file lives in: the root for seed `file("...")`
/// assets. A path with no parent retains the historical empty, cwd-relative
/// base directory.
pub fn source_dir(file: impl AsRef<Path>) -> PathBuf {
    file.as_ref()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// Successful result of the full `check` load proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckSummary {
    pub tables: usize,
    pub records: usize,
    pub functions: usize,
    pub unchecked_escapes: usize,
    pub seed_rows: usize,
}

impl fmt::Display for CheckSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let functions = if self.functions == 0 {
            "0 fn(s)".to_string()
        } else {
            format!(
                "{} fn(s) ({} unchecked escapes)",
                self.functions, self.unchecked_escapes
            )
        };
        write!(
            f,
            "ok: {} table(s), {} record(s), {functions}, {} seed row(s)",
            self.tables, self.records, self.seed_rows
        )
    }
}

/// Materialize in memory, validate function bodies and checks, prove defaults,
/// and replay seed data: everything `spock run` would reject before serving.
pub fn full_load_check(
    contract: &Contract,
    base_dir: impl AsRef<Path>,
) -> anyhow::Result<CheckSummary> {
    spock_runtime::engine::open(contract, None, Some(base_dir.as_ref()))?;
    Ok(CheckSummary {
        tables: contract.tables.len(),
        records: contract.records.len(),
        functions: contract.fns.len(),
        unchecked_escapes: contract.fns.iter().map(|function| function.sql.len()).sum(),
        seed_rows: contract.seed.len(),
    })
}

/// Pretty JSON emitted by `spock build`.
pub fn build_artifact(contract: &Contract) -> String {
    serde_json::to_string_pretty(contract).expect("contract serializes")
}

/// A derived `spock gen` artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationTarget {
    Types,
    GraphqlSchema,
}

pub fn generate_artifact(contract: &Contract, target: GenerationTarget) -> anyhow::Result<String> {
    match target {
        GenerationTarget::Types => {
            spock_lang::typescript::typescript(contract).map_err(anyhow::Error::from)
        }
        GenerationTarget::GraphqlSchema => graphql_sdl(contract),
    }
}

/// Derive the runtime's GraphQL SDL without letting seed-data failures gate a
/// data-independent artifact.
fn graphql_sdl(contract: &Contract) -> anyhow::Result<String> {
    let mut contract = contract.clone();
    contract.seed.clear();
    let conn = spock_runtime::engine::open(&contract, None, None)?;
    let app = Arc::new(spock_runtime::App::new(contract, conn));
    Ok(spock_runtime::graphql::schema(app)?.sdl())
}

/// Counts and capabilities reported when a standalone run has materialized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandaloneRunSummary {
    pub tables: usize,
    pub functions: usize,
    pub seed_rows: usize,
    pub storage: bool,
}

impl fmt::Display for StandaloneRunSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "spock v0 — contract loaded: {} table(s), {} fn(s), {} seed row(s) replayed",
            self.tables, self.functions, self.seed_rows
        )
    }
}

/// A fully materialized historical standalone Spock server.
///
/// Construction is independent of Clap and terminal output. The app accessor
/// also lets an embedding host reuse the exact runtime state without binding a
/// second listener.
pub struct StandaloneRun {
    app: Arc<spock_runtime::App>,
    summary: StandaloneRunSummary,
    // Declared last so the database-backed app is dropped before its
    // process-lifetime advisory lock is released.
    _named_state_lock: Option<spock_host::NamedStateLock>,
}

impl StandaloneRun {
    pub fn construct(
        contract: Contract,
        database_path: Option<&Path>,
        base_dir: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        // `engine::open` deliberately deletes an existing database, WAL, and
        // SHM before reconstructing from seed. Acquire the shared framework
        // lock first so a second standalone/framework process can never race
        // that destructive boundary.
        let named_state_lock = database_path
            .map(spock_host::NamedStateLock::acquire)
            .transpose()?;
        let conn = spock_runtime::engine::open(&contract, database_path, Some(base_dir.as_ref()))?;
        let summary = StandaloneRunSummary {
            tables: contract.tables.len(),
            functions: contract.fns.len(),
            seed_rows: contract.seed.len(),
            storage: spock_runtime::storage::storage_active(&contract),
        };
        Ok(Self {
            app: Arc::new(spock_runtime::App::new(contract, conn)),
            summary,
            _named_state_lock: named_state_lock,
        })
    }

    pub fn app(&self) -> Arc<spock_runtime::App> {
        Arc::clone(&self.app)
    }

    pub fn summary(&self) -> StandaloneRunSummary {
        self.summary
    }

    /// Bind the historical loopback listener and serve until an interactive
    /// process-shutdown signal or a server failure. `on_listening` runs after
    /// a successful bind so the binary can retain its exact presentation
    /// without coupling it here.
    pub fn serve_until_ctrl_c(self, port: u16, on_listening: impl FnOnce()) -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        let shutdown = {
            let _runtime_guard = runtime.enter();
            install_standalone_shutdown_signal()?
        };
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
            on_listening();
            tokio::select! {
                result = spock_runtime::http::serve(self.app, listener) => result?,
                result = shutdown => result?,
            }
            Ok::<(), anyhow::Error>(())
        })
    }
}

type StandaloneShutdownSignal = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

#[cfg(unix)]
fn install_standalone_shutdown_signal() -> io::Result<StandaloneShutdownSignal> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(Box::pin(async move {
        tokio::select! {
            received = interrupt.recv() => require_standalone_signal(received, "SIGINT"),
            received = terminate.recv() => require_standalone_signal(received, "SIGTERM"),
        }
    }))
}

#[cfg(windows)]
fn install_standalone_shutdown_signal() -> io::Result<StandaloneShutdownSignal> {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};

    let mut interrupt = ctrl_c()?;
    let mut break_signal = ctrl_break()?;
    let mut close = ctrl_close()?;
    Ok(Box::pin(async move {
        tokio::select! {
            received = interrupt.recv() => require_standalone_signal(received, "Ctrl-C"),
            received = break_signal.recv() => require_standalone_signal(received, "Ctrl-Break"),
            received = close.recv() => require_standalone_signal(received, "console close"),
        }
    }))
}

#[cfg(not(any(unix, windows)))]
fn install_standalone_shutdown_signal() -> io::Result<StandaloneShutdownSignal> {
    Ok(Box::pin(tokio::signal::ctrl_c()))
}

#[cfg(any(unix, windows))]
fn require_standalone_signal(received: Option<()>, name: &str) -> io::Result<()> {
    received.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!("{name} signal stream closed before delivering a signal"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_error_owns_stably_rendered_diagnostics() {
        let path = std::env::temp_dir().join(format!(
            "spock-cli-library-diagnostic-{}.spock",
            std::process::id()
        ));
        std::fs::write(&path, "table a { x: nope }").expect("write source");

        let error = FileProgram::load(&path).expect_err("source must fail checking");
        let rendered = error.to_string();
        assert!(rendered.contains("error[E003]"), "{rendered}");
        assert!(rendered.contains("error[E005]"), "{rendered}");
        assert!(rendered.ends_with("error: 2 diagnostic(s), contract not produced"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn full_load_summary_and_build_artifact_are_reusable() {
        let contract = spock_lang::compile(
            "table user { key id: uuid = auto\n username: text unique }\n\
             seed { user { username: \"maya\" } }",
        )
        .expect("contract");

        let summary = full_load_check(&contract, "").expect("full load proof");
        assert_eq!(
            summary.to_string(),
            "ok: 1 table(s), 0 record(s), 0 fn(s), 1 seed row(s)"
        );
        let artifact: serde_json::Value =
            serde_json::from_str(&build_artifact(&contract)).expect("contract JSON");
        assert_eq!(artifact["tables"][0]["name"], "user");
    }

    #[test]
    fn generation_and_standalone_construction_are_library_operations() {
        let contract =
            spock_lang::compile("table user { key id: uuid = auto\n username: text unique }")
                .expect("contract");

        let types = generate_artifact(&contract, GenerationTarget::Types).expect("types");
        assert!(types.contains("export interface user {"), "{types}");
        let schema = generate_artifact(&contract, GenerationTarget::GraphqlSchema).expect("SDL");
        assert!(schema.contains("type Query"), "{schema}");
        assert!(schema.contains("user("), "{schema}");

        let run = StandaloneRun::construct(contract, None, "").expect("standalone run");
        assert_eq!(
            run.summary(),
            StandaloneRunSummary {
                tables: 1,
                functions: 0,
                seed_rows: 0,
                storage: false,
            }
        );
        assert_eq!(run.app().contract.tables[0].name, "user");
    }

    #[test]
    fn standalone_named_database_is_locked_before_reset_for_the_run_lifetime() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("shared.sqlite");
        let sentinel = b"another process owns these bytes";
        std::fs::write(&database, sentinel).unwrap();
        let external_lock = spock_host::NamedStateLock::acquire(&database).unwrap();
        let contract =
            spock_lang::compile("table user { key id: uuid = auto\n username: text unique }")
                .unwrap();

        let blocked = StandaloneRun::construct(contract.clone(), Some(&database), "")
            .err()
            .expect("a live framework lock must block standalone reset");
        assert!(blocked.to_string().contains("already owned"), "{blocked}");
        assert_eq!(std::fs::read(&database).unwrap(), sentinel);

        drop(external_lock);
        let run = StandaloneRun::construct(contract, Some(&database), "").unwrap();
        let contended = spock_host::NamedStateLock::acquire(&database)
            .expect_err("standalone run must retain the lock");
        assert!(
            contended.to_string().contains("already owned"),
            "{contended}"
        );

        drop(run);
        let released = spock_host::NamedStateLock::acquire(&database).unwrap();
        drop(released);
    }
}

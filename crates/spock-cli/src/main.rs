//! The public `spock` command: framework projects plus the retained standalone
//! language-file tools.

use std::collections::BTreeSet;
use std::future::Future;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use clap::{Parser, Subcommand};
use spock_cli::{
    CheckTargetError, CheckTargetSummary, FileProgram, GenerationTarget, StandaloneRun,
};
use spock_host::{HostMode, HostNotice, HostNoticeSink, ProjectCheckFailure, ServeOptions};

#[derive(Parser)]
#[command(
    name = "spock",
    version,
    about = "The Spock framework and language toolchain"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check a framework project, or an explicit standalone .spock file.
    Check {
        #[arg(value_name = "PATH")]
        target: Option<PathBuf>,
    },
    /// Create a new framework project.
    New {
        #[arg(value_name = "NAME")]
        name: String,
        /// Create the required Spock backend without an Uhura client.
        #[arg(long)]
        backend_only: bool,
    },
    /// Adopt an existing directory as a framework project.
    Init {
        #[arg(value_name = "PATH")]
        target: Option<PathBuf>,
    },
    /// Check once and serve one fixed framework generation.
    Start {
        #[arg(value_name = "PATH")]
        target: Option<PathBuf>,
        #[arg(long, default_value_t = 4000)]
        port: u16,
        /// Disposable database file, reconstructed from seed on process start.
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Serve a framework project with live client publication.
    Dev {
        #[arg(value_name = "PATH")]
        target: Option<PathBuf>,
        #[arg(long, default_value_t = 4000)]
        port: u16,
        /// Disposable database file, reconstructed from seed on process start.
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Compile a standalone program to its contract (JSON on stdout, or -o FILE).
    Build {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Compile, materialize, seed, and serve one standalone .spock program.
    Run {
        file: PathBuf,
        #[arg(long, default_value_t = 4000)]
        port: u16,
        /// Database file (recreated on every run). Default: in-memory.
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Generate derived artifacts from a standalone program (RFD 0010).
    Gen {
        #[command(subcommand)]
        target: GenTarget,
    },
}

#[derive(Subcommand)]
enum GenTarget {
    /// TypeScript types: rows, insert/update shapes, error-code unions.
    Types {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// The GraphQL SDL the runtime serves — for offline schema tooling.
    GraphqlSchema {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    execute(Cli::parse().command)
}

fn execute(command: Command) -> ExitCode {
    match command {
        Command::Check { target } => {
            let Some(cwd) = current_dir_or_report() else {
                return ExitCode::FAILURE;
            };
            match spock_cli::check_target(target.as_deref(), &cwd) {
                Ok(summary) => {
                    println!("{summary}");
                    if let CheckTargetSummary::Project { report, .. } = summary {
                        print_project_warnings(report.warnings);
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => report_check_error(error),
            }
        }
        Command::New { name, backend_only } => {
            let Some(cwd) = current_dir_or_report() else {
                return ExitCode::FAILURE;
            };
            match spock_cli::create_project(&cwd, &name, backend_only) {
                Ok(summary) => {
                    println!("{summary}");
                    println!("next: run `spock dev` from the project directory above");
                    ExitCode::SUCCESS
                }
                Err(error) => report_error(error),
            }
        }
        Command::Init { target } => {
            let Some(cwd) = current_dir_or_report() else {
                return ExitCode::FAILURE;
            };
            match spock_cli::init_project(target.as_deref(), &cwd) {
                Ok(summary) => {
                    println!("{summary}");
                    println!("next: run `spock dev` from the project directory above");
                    ExitCode::SUCCESS
                }
                Err(error) => report_error(error),
            }
        }
        Command::Start { target, port, db } => {
            serve_framework(target.as_deref(), port, db, HostMode::Start)
        }
        Command::Dev { target, port, db } => {
            serve_framework(target.as_deref(), port, db, HostMode::Dev)
        }
        Command::Build { file, out } => {
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            let json = spock_cli::build_artifact(program.contract());
            match out {
                None => println!("{json}"),
                Some(path) => {
                    if let Err(error) = std::fs::write(&path, json) {
                        eprintln!("error: could not write {}: {error}", path.display());
                        return ExitCode::FAILURE;
                    }
                    println!("wrote {}", path.display());
                }
            }
            ExitCode::SUCCESS
        }
        Command::Run { file, port, db } => {
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            let base_dir = program.source_dir();
            let run = StandaloneRun::construct(program.into_contract(), db.as_deref(), base_dir);
            match run.and_then(|run| {
                let summary = run.summary();
                println!("{summary}");
                run.serve_until_ctrl_c(port, move || {
                    print_standalone_listening(port, summary.storage)
                })
            }) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => report_error(error),
            }
        }
        Command::Gen { target } => {
            let (file, out) = match &target {
                GenTarget::Types { file, out } | GenTarget::GraphqlSchema { file, out } => {
                    (file.clone(), out.clone())
                }
            };
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            let artifact = match target {
                GenTarget::Types { .. } => {
                    spock_cli::generate_artifact(program.contract(), GenerationTarget::Types)
                }
                GenTarget::GraphqlSchema { .. } => spock_cli::generate_artifact(
                    program.contract(),
                    GenerationTarget::GraphqlSchema,
                ),
            };
            match artifact {
                Ok(content) => emit(out, content),
                Err(error) => report_error(error),
            }
        }
    }
}

fn serve_framework(
    target: Option<&Path>,
    port: u16,
    database_path: Option<PathBuf>,
    mode: HostMode,
) -> ExitCode {
    let Some(cwd) = current_dir_or_report() else {
        return ExitCode::FAILURE;
    };
    let layout = match spock_cli::resolve_project_for_serve(target, &cwd) {
        Ok(layout) => layout,
        Err(error) => return report_error(error),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return report_error(error),
    };
    let options = ServeOptions {
        bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
        database_path,
        ..ServeOptions::default()
    };
    let notices = HostNoticeSink::new(print_framework_notice);
    let shutdown_signal = {
        let _runtime_guard = runtime.enter();
        match install_framework_shutdown_signal() {
            Ok(signal) => signal,
            Err(error) => {
                return report_error(format!(
                    "could not install shutdown signal handler: {error}"
                ))
            }
        }
    };
    let shutdown_error = Arc::new(Mutex::new(None));
    let shutdown_error_for_signal = Arc::clone(&shutdown_error);
    let shutdown = async move {
        if let Err(error) = shutdown_signal.await {
            *shutdown_error_for_signal
                .lock()
                .expect("shutdown error lock") = Some(error);
        }
    };
    let result = runtime.block_on(spock_host::serve_project(
        layout, mode, options, notices, shutdown,
    ));
    if let Some(error) = shutdown_error.lock().expect("shutdown error lock").take() {
        return report_error(format!("shutdown signal handling failed: {error}"));
    }
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => report_error(error),
    }
}

type ShutdownSignal = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

#[cfg(unix)]
fn install_framework_shutdown_signal() -> io::Result<ShutdownSignal> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(Box::pin(async move {
        tokio::select! {
            received = interrupt.recv() => require_signal(received, "SIGINT"),
            received = terminate.recv() => require_signal(received, "SIGTERM"),
        }
    }))
}

#[cfg(windows)]
fn install_framework_shutdown_signal() -> io::Result<ShutdownSignal> {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};

    let mut interrupt = ctrl_c()?;
    let mut break_signal = ctrl_break()?;
    let mut close = ctrl_close()?;
    Ok(Box::pin(async move {
        tokio::select! {
            received = interrupt.recv() => require_signal(received, "Ctrl-C"),
            received = break_signal.recv() => require_signal(received, "Ctrl-Break"),
            received = close.recv() => require_signal(received, "console close"),
        }
    }))
}

#[cfg(not(any(unix, windows)))]
fn install_framework_shutdown_signal() -> io::Result<ShutdownSignal> {
    Ok(Box::pin(tokio::signal::ctrl_c()))
}

#[cfg(any(unix, windows))]
fn require_signal(received: Option<()>, name: &str) -> io::Result<()> {
    received.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            format!("{name} signal stream closed before delivering a signal"),
        )
    })
}

fn print_framework_notice(notice: HostNotice) {
    match notice {
        HostNotice::DevelopmentPolicy => eprintln!(
            "warning: backend and spock.toml changes are observed but not applied; restart `spock dev` to reconstruct backend state from seed"
        ),
        HostNotice::Listening {
            address,
            client_configured,
        } => {
            println!("listening on http://{address}");
            if client_configured {
                println!("  GET  /                     Uhura Editor");
                println!("  GET  /play                 Uhura Play");
            }
            println!("  GET  /~studio              Spock Studio");
            println!("  GET  /~contract            active Spock contract");
            println!("  GET  /~project/status      framework generation status");
            println!("  GET  /~health              aggregate readiness");
            println!("  *    /rest/v1/*            authority REST and RPC");
            println!("  POST /graphql/v1           GraphQL when the contract is non-empty");
        }
        HostNotice::ClientBuilding { observed_revision } => {
            println!("client: building revision {observed_revision}");
        }
        HostNotice::ClientPublished {
            observed_revision,
            source_revision,
            play_generation,
        } => println!(
            "client: published revision {observed_revision} (source {source_revision}, Play generation {play_generation})"
        ),
        HostNotice::ClientRejected {
            observed_revision,
            diagnostics,
            serving_last_good,
        } => {
            eprintln!(
                "client: rejected revision {observed_revision}; serving_last_good={serving_last_good}"
            );
            for diagnostic in diagnostics {
                eprintln!("  {diagnostic}");
            }
        }
        HostNotice::BackendRestartRequired {
            changed_inputs,
            diagnostics,
        } => {
            eprintln!(
                "backend: restart required; active state remains pinned (changed: {})",
                changed_inputs.join(", ")
            );
            for diagnostic in diagnostics {
                eprintln!("  {diagnostic}");
            }
        }
        HostNotice::BackendReverted => {
            println!("backend: inputs match the active generation again");
        }
        HostNotice::ObserverError { message } => {
            eprintln!("warning: development observer: {message}");
        }
    }
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
}

/// Print to stdout, or write to `-o FILE`.
fn emit(out: Option<PathBuf>, content: String) -> ExitCode {
    match out {
        None => {
            print!("{content}");
            ExitCode::SUCCESS
        }
        Some(path) => {
            if let Err(error) = std::fs::write(&path, content) {
                eprintln!("error: could not write {}: {error}", path.display());
                return ExitCode::FAILURE;
            }
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
    }
}

fn current_dir_or_report() -> Option<PathBuf> {
    match std::env::current_dir() {
        Ok(path) => Some(path),
        Err(error) => {
            eprintln!("error: could not resolve the working directory: {error}");
            None
        }
    }
}

/// Binary-only presentation adapter for the library's structured load error.
fn load_or_report(path: &Path) -> Option<FileProgram> {
    match FileProgram::load(path) {
        Ok(program) => Some(program),
        Err(error) => {
            eprintln!("{error}");
            None
        }
    }
}

fn report_error(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::FAILURE
}

fn report_check_error(error: CheckTargetError) -> ExitCode {
    match error {
        // These two preserve the historical standalone rendering, which
        // already owns its `error:` prefix and source diagnostics.
        CheckTargetError::FileLoad(error) => eprintln!("{error}"),
        CheckTargetError::FileLoadProof { source, .. } => eprintln!("error: {source}"),
        CheckTargetError::ProjectCheck(error) => report_project_check_failure(&error),
        error => eprintln!("error: {error}"),
    }
    ExitCode::FAILURE
}

fn print_project_warnings(warnings: Vec<spock_host::ProjectCheckDiagnostic>) {
    let mut seen = BTreeSet::new();
    for warning in warnings {
        if seen.insert(warning.clone()) {
            eprintln!("warning: {warning}");
        }
    }
}

fn report_project_check_failure(error: &ProjectCheckFailure) {
    let mut seen = BTreeSet::new();
    let mut first = true;
    for diagnostic in error.diagnostics() {
        if !seen.insert(diagnostic.clone()) {
            continue;
        }
        let rendered = diagnostic.to_string();
        if first {
            eprintln!("error: {rendered}");
            first = false;
        } else {
            eprintln!("{rendered}");
        }
    }
    if first {
        eprintln!("error: project check failed without diagnostics");
    }
}

fn print_standalone_listening(port: u16, storage: bool) {
    println!("listening on http://127.0.0.1:{port}");
    println!("  GET  /~studio             the developer console — browse, impersonate, run");
    println!("  GET  /~contract           the contract, as data");
    println!("  GET  /rest/v1/{{table}}     open reads (identity view)");
    println!("  POST /rest/v1/rpc/{{fn}}    call a declared fn");
    println!("  POST /graphql/v1          GraphQL reads + writes (GraphiQL in the browser)");
    if storage {
        println!("  *    /storage/v1/object    upload + serve files (signed URLs)");
    }
}

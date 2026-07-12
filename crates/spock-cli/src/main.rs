//! The `spock` command: `check` (diagnostics), `build` (emit the contract),
//! `gen` (derived artifacts — TypeScript types, GraphQL SDL; RFD 0010),
//! `run` (materialize + serve the HTTP protocol). docs/spec/v0.md.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use spock_lang::ir::Contract;

#[derive(Parser)]
#[command(name = "spock", version, about = "The Spock v0 toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse and check a program; print diagnostics.
    Check { file: PathBuf },
    /// Compile a program to its contract (JSON on stdout, or -o FILE).
    Build {
        file: PathBuf,
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Compile, materialize the database, replay the seed, and serve HTTP.
    Run {
        file: PathBuf,
        #[arg(long, default_value_t = 4000)]
        port: u16,
        /// Database file (recreated on every run). Default: in-memory.
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Generate derived artifacts from a program (RFD 0010).
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
    match Cli::parse().command {
        Command::Check { file } => {
            let Some(contract) = load(&file) else {
                return ExitCode::FAILURE;
            };
            // The full load proof (RFD 0013): materialize the schema in
            // memory, validate every fn body and inlined check, prove
            // defaults against their checks, and replay the seed — so
            // anything `spock run` would reject at load surfaces here,
            // without starting a server.
            if let Err(e) = spock_runtime::engine::open(&contract, None) {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
            // every v0 fn statement is an SQL escape — the unchecked count
            // is the ledger (RFD 0011 §4), trending to zero as native
            // bodies arrive
            let fns = if contract.fns.is_empty() {
                "0 fn(s)".to_string()
            } else {
                format!(
                    "{} fn(s) ({} unchecked escapes)",
                    contract.fns.len(),
                    contract.fns.iter().map(|f| f.sql.len()).sum::<usize>()
                )
            };
            println!(
                "ok: {} table(s), {} record(s), {fns}, {} seed row(s)",
                contract.tables.len(),
                contract.records.len(),
                contract.seed.len()
            );
            ExitCode::SUCCESS
        }
        Command::Build { file, out } => {
            let Some(contract) = load(&file) else {
                return ExitCode::FAILURE;
            };
            let json = serde_json::to_string_pretty(&contract).expect("contract serializes");
            match out {
                None => println!("{json}"),
                Some(path) => {
                    if let Err(e) = std::fs::write(&path, json) {
                        eprintln!("error: could not write {}: {e}", path.display());
                        return ExitCode::FAILURE;
                    }
                    println!("wrote {}", path.display());
                }
            }
            ExitCode::SUCCESS
        }
        Command::Run { file, port, db } => {
            let Some(contract) = load(&file) else {
                return ExitCode::FAILURE;
            };
            match run(contract, port, db) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Gen { target } => {
            let (file, out) = match &target {
                GenTarget::Types { file, out } | GenTarget::GraphqlSchema { file, out } => {
                    (file.clone(), out.clone())
                }
            };
            let Some(contract) = load(&file) else {
                return ExitCode::FAILURE;
            };
            let artifact = match target {
                GenTarget::Types { .. } => {
                    spock_lang::typescript::typescript(&contract).map_err(anyhow::Error::from)
                }
                GenTarget::GraphqlSchema { .. } => graphql_sdl(contract),
            };
            match artifact {
                Ok(content) => emit(out, content),
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

/// The SDL of the schema the runtime would serve — derived through the
/// same builder as `run`, so it cannot drift. The in-memory engine exists
/// only because the builder wants a full `App`; no resolver ever runs,
/// and the seed is dropped first: the SDL is a pure function of the
/// tables, so a data problem (say, a seed unique conflict) must not gate
/// a data-independent artifact.
fn graphql_sdl(mut contract: Contract) -> anyhow::Result<String> {
    contract.seed.clear();
    let conn = spock_runtime::engine::open(&contract, None)?;
    let app = Arc::new(spock_runtime::App::new(contract, conn));
    Ok(spock_runtime::graphql::schema(app)?.sdl())
}

/// Print to stdout, or write to `-o FILE`.
fn emit(out: Option<PathBuf>, content: String) -> ExitCode {
    match out {
        None => {
            print!("{content}");
            ExitCode::SUCCESS
        }
        Some(path) => {
            if let Err(e) = std::fs::write(&path, content) {
                eprintln!("error: could not write {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
    }
}

/// Read, compile, and (on failure) render every diagnostic.
fn load(path: &PathBuf) -> Option<Contract> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: could not read {}: {e}", path.display());
            return None;
        }
    };
    match spock_lang::compile(&source) {
        Ok(contract) => Some(contract),
        Err(diags) => {
            for diag in &diags {
                eprintln!("{}", diag.render(&source, &path.display().to_string()));
            }
            eprintln!(
                "error: {} diagnostic(s), contract not produced",
                diags.len()
            );
            None
        }
    }
}

fn run(contract: Contract, port: u16, db: Option<PathBuf>) -> anyhow::Result<()> {
    let conn = spock_runtime::engine::open(&contract, db.as_deref())?;
    println!(
        "spock v0 — contract loaded: {} table(s), {} fn(s), {} seed row(s) replayed",
        contract.tables.len(),
        contract.fns.len(),
        contract.seed.len()
    );

    let app = Arc::new(spock_runtime::App::new(contract, conn));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        println!("listening on http://127.0.0.1:{port}");
        println!("  GET  /~studio             the developer console — browse, impersonate, run");
        println!("  GET  /~contract           the contract, as data");
        println!("  GET  /rest/v1/{{table}}     open reads (identity view)");
        println!("  POST /rest/v1/rpc/{{fn}}    call a declared fn");
        println!("  POST /graphql/v1          GraphQL reads + writes (GraphiQL in the browser)");
        tokio::select! {
            result = spock_runtime::http::serve(app, listener) => result?,
            _ = tokio::signal::ctrl_c() => {}
        }
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

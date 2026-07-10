//! The `spock` command: `check` (diagnostics), `build` (emit the contract),
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
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { file } => {
            let Some(contract) = load(&file) else {
                return ExitCode::FAILURE;
            };
            println!(
                "ok: {} table(s), {} seed row(s)",
                contract.tables.len(),
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
        "spock v0 — contract loaded: {} table(s), {} seed row(s) replayed",
        contract.tables.len(),
        contract.seed.len()
    );

    let app = Arc::new(spock_runtime::App::new(contract, conn));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        println!("listening on http://127.0.0.1:{port}");
        println!("  GET  /~contract           the contract, as data");
        println!("  GET  /rest/v1/{{table}}     open reads (identity view)");
        println!("  POST /graphql/v1          GraphQL reads + writes (GraphiQL in the browser)");
        tokio::select! {
            result = spock_runtime::http::serve(app, listener) => result?,
            _ = tokio::signal::ctrl_c() => {}
        }
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

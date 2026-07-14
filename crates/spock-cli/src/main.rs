//! The `spock` command: `check` (diagnostics), `build` (emit the contract),
//! `gen` (derived artifacts — TypeScript types, GraphQL SDL; RFD 0010),
//! `run` (materialize + serve the HTTP protocol). docs/spec/v0.md.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use spock_cli::{FileProgram, GenerationTarget, StandaloneRun};

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
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            match spock_cli::full_load_check(program.contract(), program.source_dir()) {
                Ok(summary) => {
                    println!("{summary}");
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::Build { file, out } => {
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            let json = spock_cli::build_artifact(program.contract());
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
            let Some(program) = load_or_report(&file) else {
                return ExitCode::FAILURE;
            };
            let base_dir = program.source_dir();
            let run = StandaloneRun::construct(program.into_contract(), db.as_deref(), base_dir);
            match run.and_then(|run| {
                let summary = run.summary();
                println!("{summary}");
                run.serve_until_ctrl_c(port, move || print_listening(port, summary.storage))
            }) {
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
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
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

fn print_listening(port: u16, storage: bool) {
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

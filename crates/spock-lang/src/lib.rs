//! The Spock v0 language: lexer, parser, checker, contract IR, emissions.
//!
//! Pipeline: source → [`lexer`] → [`parser`] (AST) → [`check`] (lower +
//! validate) → [`ir::Contract`] → emissions ([`ddl`] SQLite schema,
//! [`typescript`] client types — RFD 0010).
//!
//! The [`ir::Contract`] is the interchange artifact (docs/spec/v0.md §6);
//! everything before it is front-end, everything after it is back-end.

pub mod ast;
pub mod check;
pub mod ddl;
pub mod diag;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod typescript;

#[cfg(test)]
mod editor_tests;

use diag::Diagnostic;
use ir::Contract;

/// Compile Spock source text to a contract, or the full list of diagnostics.
pub fn compile(source: &str) -> Result<Contract, Vec<Diagnostic>> {
    let tokens = lexer::lex(source).map_err(|d| vec![d])?;
    let file = parser::parse(tokens).map_err(|d| vec![d])?;
    check::check(&file)
}

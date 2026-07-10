//! TypeScript emission (docs/rfd/0010-client-codegen-architecture.md §4).
//!
//! The contract as types: per table, the row shape, what insert and update
//! accept, and the derived error vocabulary as literal unions — plus one
//! `contract` map tying them together (the generic parameter a client
//! takes). A sibling of the SQLite DDL emission: another conformance of
//! the IR, versioned with the language.
//!
//! The emission is total in the language's sense: names it cannot assign
//! (JavaScript reserved words, TypeScript predeclared types, its own
//! top-level names) and cross-table collisions on derived names fail
//! generation with a stated reason — it never emits code that does not
//! compile. Output is deterministic: contract order, no timestamps, so
//! regeneration diffs are schema diffs.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::ir::{Contract, FnArity, FnDef, Record, Table, Type};

/// Names the emission may never assign to a table: JavaScript reserved
/// words (invalid as type names), TypeScript's predeclared type names,
/// and the emission's own top-level names. Only lowercase entries are
/// needed — the language admits only lowercase identifiers.
const RESERVED_TS_NAMES: &[&str] = &[
    // JavaScript reserved words (including strict-mode and module-level)
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    // TypeScript predeclared type names
    "any",
    "bigint",
    "boolean",
    "never",
    "number",
    "object",
    "string",
    "symbol",
    "undefined",
    "unknown",
    // TypeScript type-position keywords (invalid as type names)
    "infer",
    "keyof",
    "readonly",
    // the emission's own top-level names ("fns" is the contract map's
    // property, claimed so a table or record named `fns` fails stated)
    "contract",
    "error_code",
    "fns",
    "reserved_error",
    "timestamp",
    "uuid",
];

/// Emission failures. Both are program-shaped, not bugs: a contract the
/// checker accepts can still break the emission's naming laws.
#[derive(Debug, PartialEq)]
pub enum TsGenError {
    /// The table's name — or one of its four derived names — is a
    /// reserved word, a predeclared type, or one of the emission's own
    /// names (e.g. a table named `reserved` derives `reserved_error`).
    ReservedName { table: String, name: String },
    /// Two tables derive the same type name (e.g. `user` and a table
    /// literally named `user_insert`).
    DuplicateTypeName { a: String, b: String, name: String },
}

impl std::fmt::Display for TsGenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TsGenError::ReservedName { table, name } => write!(
                f,
                "`{table}` collides with the reserved TypeScript name `{name}`"
            ),
            TsGenError::DuplicateTypeName { a, b, name } => write!(
                f,
                "`{a}` and `{b}` both derive a TypeScript type named `{name}`"
            ),
        }
    }
}

impl std::error::Error for TsGenError {}

/// Emit the contract as TypeScript (RFD 0010 §4).
pub fn typescript(contract: &Contract) -> Result<String, TsGenError> {
    // Pass 1 — the name space: each table claims its four derived names.
    // A table's own four are pairwise distinct, so any duplicate is a
    // cross-table collision.
    let mut claimed: HashMap<String, String> = HashMap::new(); // name -> table
    for table in &contract.tables {
        for name in [
            table.name.clone(),
            format!("{}_insert", table.name),
            format!("{}_update", table.name),
            format!("{}_error", table.name),
        ] {
            // every derived name is checked, not just the bare one — a
            // table named `reserved` would otherwise derive the
            // emission's own `reserved_error`
            if RESERVED_TS_NAMES.contains(&name.as_str()) {
                return Err(TsGenError::ReservedName {
                    table: table.name.clone(),
                    name,
                });
            }
            if let Some(other) = claimed.insert(name.clone(), table.name.clone()) {
                return Err(TsGenError::DuplicateTypeName {
                    a: other,
                    b: table.name.clone(),
                    name,
                });
            }
        }
    }
    // records claim their bare name; fns claim `<fn>_args` (the fn name
    // itself is a map key, not a type — a fn named `contract` is fine)
    for record in &contract.records {
        let name = record.name.clone();
        if RESERVED_TS_NAMES.contains(&name.as_str()) {
            return Err(TsGenError::ReservedName {
                table: record.name.clone(),
                name,
            });
        }
        if let Some(other) = claimed.insert(name.clone(), record.name.clone()) {
            return Err(TsGenError::DuplicateTypeName {
                a: other,
                b: record.name.clone(),
                name,
            });
        }
    }
    for f in &contract.fns {
        let name = format!("{}_args", f.name);
        if RESERVED_TS_NAMES.contains(&name.as_str()) {
            return Err(TsGenError::ReservedName {
                table: f.name.clone(),
                name,
            });
        }
        if let Some(other) = claimed.insert(name.clone(), f.name.clone()) {
            return Err(TsGenError::DuplicateTypeName {
                a: other,
                b: f.name.clone(),
                name,
            });
        }
    }

    // Pass 2 — emit.
    let mut out = String::new();
    out.push_str("// Generated by `spock gen types` — do not edit.\n");
    let _ = writeln!(out, "// Contract: spock {}.", contract.spock);
    out.push_str("\n/** A UUID in canonical hyphenated form. */\n");
    out.push_str("export type uuid = string;\n");
    out.push_str("\n/** An RFC 3339 UTC timestamp. */\n");
    out.push_str("export type timestamp = string;\n");

    for table in &contract.tables {
        emit_row(&mut out, contract, table);
        emit_insert(&mut out, contract, table);
        if has_settable_fields(table) {
            emit_update(&mut out, contract, table);
        }
        emit_error_union(&mut out, table);
    }

    for record in &contract.records {
        emit_record(&mut out, contract, record);
    }
    for f in &contract.fns {
        emit_fn_args(&mut out, contract, f);
    }

    out.push_str("\n/** Reserved non-derived codes (docs/spec/v0.md §6.1). */\n");
    out.push_str("export type reserved_error =\n");
    out.push_str("  | \"not_found\"\n");
    out.push_str("  | \"type_mismatch\"\n");
    out.push_str("  | \"unknown_field\"\n");
    out.push_str("  | \"bad_request\"\n");
    out.push_str("  | \"internal\";\n");

    out.push_str("\n/** Every error code this contract can produce. */\n");
    out.push_str("export type error_code =\n");
    for table in &contract.tables {
        let _ = writeln!(out, "  | {}_error", table.name);
    }
    out.push_str("  | reserved_error;\n");

    out.push_str("\n/** The compiled surface, one entry per table — the generic parameter a client takes. */\n");
    out.push_str("export interface contract {\n");
    for table in &contract.tables {
        let t = &table.name;
        let update = if has_settable_fields(table) {
            format!("{t}_update")
        } else {
            // nothing is settable on a pure-key table (its update surface
            // does not exist — graphql.md §5 has the same rule)
            "never".to_string()
        };
        let _ = writeln!(
            out,
            "  {t}: {{ row: {t}; insert: {t}_insert; update: {update}; error: {t}_error }};"
        );
    }
    if contract.fns.is_empty() {
        out.push_str("  fns: {};\n");
    } else {
        out.push_str("  fns: {\n");
        for f in &contract.fns {
            // scalars render as their TS value type, shapes as interfaces
            let target = match f.returns.scalar_type() {
                Some(ty) => value_ts(contract, &ty),
                None => f.returns.of.clone(),
            };
            let returns = match f.returns.arity {
                FnArity::One => target,
                FnArity::Maybe => format!("{target} | null"),
                FnArity::Many => format!("{target}[]"),
            };
            let error = if f.errors.is_empty() {
                "never".to_string()
            } else {
                f.errors
                    .iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            let _ = writeln!(
                out,
                "    {}: {{ args: {}_args; returns: {returns}; error: {error} }};",
                f.name, f.name
            );
        }
        out.push_str("  };\n");
    }
    out.push_str("}\n");

    Ok(out)
}

/// Whether the table has any non-key field — the precondition for an
/// update shape (keys are immutable, spec §7.2).
fn has_settable_fields(table: &Table) -> bool {
    table.fields.iter().any(|f| !table.key.contains(&f.name))
}

/// The TypeScript type of a field's value. A reference renders as the
/// target key's type via indexed access (`user["id"]`) — structurally a
/// scalar, semantically a pointer the reader can follow.
/// `interface <record>` — a named wire shape (fn return).
fn emit_record(out: &mut String, contract: &Contract, record: &Record) {
    let _ = writeln!(
        out,
        "\n/** The `{}` record — a fn return shape. */",
        record.name
    );
    let _ = writeln!(out, "export interface {} {{", record.name);
    for f in &record.fields {
        let null = if f.optional { " | null" } else { "" };
        let _ = writeln!(out, "  {}: {}{null};", f.name, value_ts(contract, &f.ty));
    }
    out.push_str("}\n");
}

/// `interface <fn>_args` — what a fn call takes. Optional params admit
/// `null`: for fn arguments null and absence mean the same thing.
fn emit_fn_args(out: &mut String, contract: &Contract, f: &FnDef) {
    let _ = writeln!(out, "\n/** Arguments of fn `{}`. */", f.name);
    let _ = writeln!(out, "export interface {}_args {{", f.name);
    for p in &f.params {
        let ty = value_ts(contract, &p.ty);
        if p.optional {
            let _ = writeln!(out, "  {}?: {ty} | null;", p.name);
        } else {
            let _ = writeln!(out, "  {}: {ty};", p.name);
        }
    }
    out.push_str("}\n");
}

fn value_ts(contract: &Contract, ty: &Type) -> String {
    match ty {
        Type::Text => "string".into(),
        Type::Int => "number".into(),
        Type::Float => "number".into(),
        Type::Bool => "boolean".into(),
        Type::Uuid => "uuid".into(),
        Type::Timestamp => "timestamp".into(),
        Type::Ref { table, .. } => {
            let target = contract.table(table).expect("checked: ref target exists");
            // references always target single-key tables (checker E010)
            format!("{}[\"{}\"]", target.name, target.key[0])
        }
    }
}

/// `interface <t>` — the row, as reads return it.
fn emit_row(out: &mut String, contract: &Contract, table: &Table) {
    let _ = writeln!(out, "\n/** One `{}` row, as reads return it. */", table.name);
    let _ = writeln!(out, "export interface {} {{", table.name);
    for f in &table.fields {
        let null = if f.optional { " | null" } else { "" };
        let _ = writeln!(out, "  {}: {}{null};", f.name, value_ts(contract, &f.ty));
    }
    out.push_str("}\n");
}

/// `interface <t>_insert` — what insert accepts (spec §7.2): required iff
/// required with no default; on insert `null` is absence (§5.1), so only
/// optional fields admit it.
fn emit_insert(out: &mut String, contract: &Contract, table: &Table) {
    let _ = writeln!(
        out,
        "\n/** What insert accepts for `{}` (omission applies defaults; on insert `null` is absence). */",
        table.name
    );
    let _ = writeln!(out, "export interface {}_insert {{", table.name);
    for f in &table.fields {
        let ty = value_ts(contract, &f.ty);
        if !f.optional && f.default.is_none() {
            let _ = writeln!(out, "  {}: {ty};", f.name);
        } else {
            let null = if f.optional { " | null" } else { "" };
            let _ = writeln!(out, "  {}?: {ty}{null};", f.name);
        }
    }
    out.push_str("}\n");
}

/// `interface <t>_update` — non-key fields only (keys are immutable):
/// absent = keep; `null` clears an optional field, and is excluded from
/// required fields because there it is the derived `required` error.
fn emit_update(out: &mut String, contract: &Contract, table: &Table) {
    let _ = writeln!(
        out,
        "\n/** What update accepts for `{}` (absent = keep; `null` clears an optional field). Keys are immutable. */",
        table.name
    );
    let _ = writeln!(out, "export interface {}_update {{", table.name);
    for f in &table.fields {
        if table.key.contains(&f.name) {
            continue;
        }
        let null = if f.optional { " | null" } else { "" };
        let _ = writeln!(out, "  {}?: {}{null};", f.name, value_ts(contract, &f.ty));
    }
    out.push_str("}\n");
}

/// `type <t>_error` — the table's derived codes (spec §6.1, frozen
/// vocabulary), contract order.
fn emit_error_union(out: &mut String, table: &Table) {
    let _ = writeln!(
        out,
        "\n/** Error codes `{}` writes can produce (docs/spec/v0.md §6.1). */",
        table.name
    );
    if table.errors.is_empty() {
        let _ = writeln!(out, "export type {}_error = never;", table.name);
        return;
    }
    let _ = writeln!(out, "export type {}_error =", table.name);
    for (i, e) in table.errors.iter().enumerate() {
        let end = if i + 1 == table.errors.len() { ";" } else { "" };
        let _ = writeln!(out, "  | \"{}\"{end}", e.code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(source: &str) -> Result<String, TsGenError> {
        typescript(&crate::compile(source).expect("program compiles"))
    }

    const PROGRAM: &str = "table user { key id: uuid = auto\n username: text unique\n bio: text?\n invited_by: user?\n joined_at: timestamp = now }\n\
                           table follow { key (follower, target)\n follower: user\n target: user\n since: timestamp = now }";

    #[test]
    fn emits_the_row_shape() {
        let ts = emit(PROGRAM).unwrap();
        assert!(ts.contains("export interface user {"), "{ts}");
        assert!(ts.contains("  id: uuid;"), "{ts}");
        assert!(ts.contains("  bio: string | null;"), "{ts}");
        assert!(ts.contains("  invited_by: user[\"id\"] | null;"), "{ts}");
        assert!(ts.contains("  joined_at: timestamp;"), "{ts}");
    }

    #[test]
    fn insert_requires_exactly_the_defaultless_required_fields() {
        let ts = emit(PROGRAM).unwrap();
        let block: Vec<&str> = ts
            .lines()
            .skip_while(|l| !l.starts_with("export interface user_insert"))
            .take_while(|l| !l.starts_with('}'))
            .collect();
        assert!(block.contains(&"  username: string;"), "{block:?}");
        assert!(block.contains(&"  id?: uuid;"), "{block:?}"); // auto default
        assert!(block.contains(&"  bio?: string | null;"), "{block:?}");
        assert!(block.contains(&"  joined_at?: timestamp;"), "{block:?}");
        // composite ref keys: required (no default), typed as the target key
        assert!(
            ts.contains("export interface follow_insert {\n  follower: user[\"id\"];"),
            "{ts}"
        );
    }

    #[test]
    fn update_excludes_keys_and_null_on_required() {
        let ts = emit(PROGRAM).unwrap();
        let block: Vec<&str> = ts
            .lines()
            .skip_while(|l| !l.starts_with("export interface user_update"))
            .take_while(|l| !l.starts_with('}'))
            .collect();
        assert!(!block.iter().any(|l| l.trim_start().starts_with("id")), "{block:?}");
        assert!(block.contains(&"  username?: string;"), "{block:?}"); // no `| null`
        assert!(block.contains(&"  bio?: string | null;"), "{block:?}");
    }

    #[test]
    fn error_unions_carry_the_derived_vocabulary() {
        let ts = emit(PROGRAM).unwrap();
        assert!(ts.contains("\"user_already_exists\""), "{ts}");
        assert!(ts.contains("\"user_username_taken\""), "{ts}");
        assert!(ts.contains("\"user_username_required\""), "{ts}");
        assert!(ts.contains("\"follow_already_exists\""), "{ts}");
        assert!(ts.contains("| reserved_error;"), "{ts}");
        assert!(
            ts.contains("user: { row: user; insert: user_insert; update: user_update; error: user_error };"),
            "{ts}"
        );
    }

    #[test]
    fn pure_key_table_has_no_update_shape() {
        let ts = emit(
            "table user { key id: uuid = auto\n a: int }\n\
             table follow { key (follower, target)\n follower: user\n target: user }",
        )
        .unwrap();
        assert!(!ts.contains("follow_update"), "{ts}");
        assert!(
            ts.contains("follow: { row: follow; insert: follow_insert; update: never; error: follow_error };"),
            "{ts}"
        );
    }

    #[test]
    fn reserved_table_name_fails_generation() {
        // type-position keywords (`keyof` etc.) would emit non-compiling
        // TS; `reserved` collides through its DERIVED name `reserved_error`
        for table in ["function", "string", "contract", "keyof", "readonly", "infer", "reserved"] {
            let err = emit(&format!("table {table} {{ key id: uuid = auto\n a: int }}")).unwrap_err();
            assert!(matches!(err, TsGenError::ReservedName { .. }), "{table}: {err}");
        }
    }

    /// The full emission for one small program, verbatim — the golden the
    /// RFD 0010 verification story rests on. Any formatting or rule
    /// change must update this deliberately.
    #[test]
    fn golden() {
        let ts = emit("table user { key id: uuid = auto\n username: text unique }").unwrap();
        let golden = r#"// Generated by `spock gen types` — do not edit.
// Contract: spock v0.

/** A UUID in canonical hyphenated form. */
export type uuid = string;

/** An RFC 3339 UTC timestamp. */
export type timestamp = string;

/** One `user` row, as reads return it. */
export interface user {
  id: uuid;
  username: string;
}

/** What insert accepts for `user` (omission applies defaults; on insert `null` is absence). */
export interface user_insert {
  id?: uuid;
  username: string;
}

/** What update accepts for `user` (absent = keep; `null` clears an optional field). Keys are immutable. */
export interface user_update {
  username?: string;
}

/** Error codes `user` writes can produce (docs/spec/v0.md §6.1). */
export type user_error =
  | "user_already_exists"
  | "user_username_taken"
  | "user_username_required";

/** Reserved non-derived codes (docs/spec/v0.md §6.1). */
export type reserved_error =
  | "not_found"
  | "type_mismatch"
  | "unknown_field"
  | "bad_request"
  | "internal";

/** Every error code this contract can produce. */
export type error_code =
  | user_error
  | reserved_error;

/** The compiled surface, one entry per table — the generic parameter a client takes. */
export interface contract {
  user: { row: user; insert: user_insert; update: user_update; error: user_error };
  fns: {};
}
"#;
        assert_eq!(ts, golden);
    }

    #[test]
    fn emits_records_and_fns() {
        let ts = emit(
            "table user { key id: uuid = auto\n username: text unique }\n\
             record stats { posts: int\n latest: timestamp? }\n\
             fn rename_user(user: user, name: text, note: text?) -> user ! user_username_taken { unchecked sql(\"S\") }\n\
             fn find_user(name: text) -> user? { unchecked sql(\"S\") }\n\
             fn tally() -> [stats] { unchecked sql(\"S\") }\n\
             fn user_count() -> int { unchecked sql(\"S\") }\n\
             fn last_seen() -> timestamp? { unchecked sql(\"S\") }",
        )
        .unwrap();
        // record interface
        assert!(ts.contains("export interface stats {\n  posts: number;\n  latest: timestamp | null;\n}"), "{ts}");
        // args: required, ref-as-target-key, optional-with-null
        assert!(ts.contains("export interface rename_user_args {\n  user: user[\"id\"];\n  name: string;\n  note?: string | null;\n}"), "{ts}");
        // zero-param fn still gets an (empty) args interface
        assert!(ts.contains("export interface tally_args {\n}"), "{ts}");
        // the fns map: arity rendering and error unions
        assert!(
            ts.contains("    rename_user: { args: rename_user_args; returns: user; error: \"user_username_taken\" };"),
            "{ts}"
        );
        assert!(
            ts.contains("    find_user: { args: find_user_args; returns: user | null; error: never };"),
            "{ts}"
        );
        // scalar returns render as TS value types, not interface names
        assert!(
            ts.contains("    user_count: { args: user_count_args; returns: number; error: never };"),
            "{ts}"
        );
        assert!(
            ts.contains("    last_seen: { args: last_seen_args; returns: timestamp | null; error: never };"),
            "{ts}"
        );
        assert!(
            ts.contains("    tally: { args: tally_args; returns: stats[]; error: never };"),
            "{ts}"
        );
    }

    #[test]
    fn record_and_fn_name_collisions_fail_generation() {
        // a table named `fns` collides with the contract map property
        let err = emit("table fns { key id: uuid = auto\n a: int }").unwrap_err();
        assert!(matches!(err, TsGenError::ReservedName { .. }), "{err}");
        // a record named like a table's derived name
        let err = emit(
            "table user { key id: uuid = auto\n a: int }\n\
             record user_insert { x: int }",
        )
        .unwrap_err();
        assert!(matches!(err, TsGenError::DuplicateTypeName { .. }), "{err}");
        // a table named like a fn's args interface
        let err = emit(
            "table hello_args { key id: uuid = auto\n a: int }\n\
             fn hello() -> hello_args { unchecked sql(\"S\") }",
        )
        .unwrap_err();
        assert!(matches!(err, TsGenError::DuplicateTypeName { .. }), "{err}");
    }

    #[test]
    fn derived_name_collision_fails_generation() {
        let err = emit(
            "table user { key id: uuid = auto\n a: int }\n\
             table user_insert { key id: uuid = auto\n b: int }",
        )
        .unwrap_err();
        assert!(matches!(err, TsGenError::DuplicateTypeName { .. }), "{err}");
    }

    #[test]
    fn deterministic() {
        assert_eq!(emit(PROGRAM).unwrap(), emit(PROGRAM).unwrap());
    }
}

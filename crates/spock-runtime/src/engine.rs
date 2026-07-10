//! Engine bootstrap (docs/spec/v0.md §7.1, §7.3): open SQLite, materialize
//! the schema fresh, replay the seed through the write path. There are no
//! migrations in v0 — state is disposable by doctrine (RFD 0008 §3).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Map, Value as Json};
use spock_lang::ddl::ddl;
use spock_lang::ir::{Contract, SeedValue};

use crate::error::ApiError;
use crate::write::insert_row;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not reset database file: {0}")]
    Io(#[from] std::io::Error),
    #[error("seed row {index} ({table}): {source}")]
    Seed {
        index: usize,
        table: String,
        source: Box<ApiError>,
    },
    #[error("fn `{name}`: {message}")]
    Fn { name: String, message: String },
}

/// Open the database (in-memory when `path` is `None`), materialize the
/// schema, and replay the seed. A file database is recreated from scratch:
/// no migrations in v0.
pub fn open(contract: &Contract, path: Option<&Path>) -> Result<Connection, EngineError> {
    let mut conn = match path {
        None => Connection::open_in_memory()?,
        Some(p) => {
            for suffix in ["", "-wal", "-shm"] {
                let candidate = if suffix.is_empty() {
                    p.to_path_buf()
                } else {
                    p.with_file_name(format!(
                        "{}{suffix}",
                        p.file_name().and_then(|n| n.to_str()).unwrap_or("spock.db")
                    ))
                };
                if candidate.exists() {
                    std::fs::remove_file(&candidate)?;
                }
            }
            Connection::open(p)?
        }
    };

    conn.pragma_update(None, "foreign_keys", true)?;
    register_builtins(&conn)?;
    for statement in ddl(contract) {
        conn.execute_batch(&statement)?;
    }
    validate_fns(contract, &conn)?;
    seed(contract, &mut conn)?;
    Ok(conn)
}

/// The engine builtins (§7.1): `spock_uuid()` and `spock_now()` — the
/// same mints the write path uses for `= auto` and `= now`, so a value
/// generated inside an escape body is indistinguishable from one the
/// engine generated. Registered before DDL: the emitted DEFAULT clauses
/// reference them.
fn register_builtins(conn: &Connection) -> Result<(), EngineError> {
    use rusqlite::functions::FunctionFlags;
    // non-deterministic by nature: neither DETERMINISTIC (they change per
    // call) nor DIRECTONLY (DEFAULT clauses must be able to call them)
    conn.create_scalar_function("spock_uuid", 0, FunctionFlags::SQLITE_UTF8, |_| {
        Ok(crate::value::new_uuid())
    })?;
    conn.create_scalar_function("spock_now", 0, FunctionFlags::SQLITE_UTF8, |_| {
        Ok(crate::value::now_utc())
    })?;
    Ok(())
}

/// Validate every fn's SQL escapes at load (§7.4), after DDL (tables must
/// exist) and before seed replay (a broken fn fails before any data
/// moves). `prepare` compiles without executing, so this checks syntax
/// and table/column resolution; each statement's own metadata checks the
/// placeholders, and the **last** statement's result columns are checked
/// against the declared return shape (earlier statements are guards and
/// effects — their results are discarded at execution). Parameters are
/// checked across the whole body: every `:param` in any statement must be
/// declared, every declared param must appear in at least one statement.
/// Total derivation, extended into the escape.
fn validate_fns(contract: &Contract, conn: &Connection) -> Result<(), EngineError> {
    for f in &contract.fns {
        let fail = |message: String| EngineError::Fn {
            name: f.name.clone(),
            message,
        };
        if f.sql.is_empty() {
            return Err(fail("body contains no SQL statement".into()));
        }

        let mut used: Vec<String> = Vec::new();
        let last_index = f.sql.len() - 1;
        for (index, sql) in f.sql.iter().enumerate() {
            let at = |message: String| {
                if last_index == 0 {
                    fail(message)
                } else {
                    fail(format!("statement {}: {message}", index + 1))
                }
            };

            // exactly one statement per escape: Batch skips comment- and
            // whitespace-only segments, so trailing `;` and comments are
            // tolerated
            let mut batch = rusqlite::Batch::new(conn, sql);
            let stmt = match batch.next() {
                Err(e) => return Err(at(format!("does not compile: {e}"))),
                Ok(None) => return Err(at("the escape contains no SQL statement".into())),
                Ok(Some(stmt)) => stmt,
            };

            // placeholders: every SQL parameter is a declared param
            for i in 1..=stmt.parameter_count() {
                let Some(name) = stmt.parameter_name(i) else {
                    return Err(at(
                        "positional parameters are not allowed; use `:param`".into(),
                    ));
                };
                let Some(bare) = name.strip_prefix(':') else {
                    return Err(at(format!("parameter `{name}` must use the `:param` form")));
                };
                if f.params.iter().all(|p| p.name != bare) {
                    return Err(at(format!(
                        "the SQL names `{name}`, which is not a parameter of this fn"
                    )));
                }
                used.push(bare.to_string());
            }

            // only the final statement answers; its columns are the
            // contract's
            if index == last_index {
                validate_return_columns(contract, f, &stmt, &at)?;
            }
            drop(stmt);

            // a second statement inside one escape is a body error
            match batch.next() {
                Ok(None) => {}
                Ok(Some(_)) => {
                    return Err(at(
                        "an escape holds exactly one SQL statement (add another `unchecked sql(...)`)"
                            .into(),
                    ));
                }
                Err(e) => return Err(at(format!("does not compile: {e}"))),
            }
        }

        // every declared param appears somewhere in the body
        for p in &f.params {
            if !used.contains(&p.name) {
                return Err(fail(format!(
                    "parameter `{}` is declared but never used in the SQL",
                    p.name
                )));
            }
        }
    }
    Ok(())
}

/// The final statement's result columns must equal the declared return
/// shape's fields, by name — known at prepare time for SELECT and
/// RETURNING alike. An empty column set also rejects DML without
/// RETURNING: every fn answers with rows.
fn validate_return_columns(
    contract: &Contract,
    f: &spock_lang::ir::FnDef,
    stmt: &rusqlite::Statement<'_>,
    at: &dyn Fn(String) -> EngineError,
) -> Result<(), EngineError> {
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    if columns.is_empty() {
        return Err(at(format!(
            "the SQL returns no columns, but the fn returns `{}` (DML needs RETURNING)",
            f.returns.of
        )));
    }
    // a scalar return is one column, any name — there is no shape to
    // match against
    if f.returns.scalar {
        if columns.len() != 1 {
            return Err(at(format!(
                "the SQL returns {} columns, but the fn returns the scalar `{}` (exactly one column)",
                columns.len(),
                f.returns.of
            )));
        }
        return Ok(());
    }
    let mut dedup = columns.clone();
    dedup.sort();
    dedup.dedup();
    if dedup.len() != columns.len() {
        return Err(at(
            "the SQL returns duplicate column names; row mapping is by name".into(),
        ));
    }
    let declared = contract
        .output_fields(&f.returns.of)
        .expect("checked: fn return shape exists");
    let missing: Vec<&str> = declared
        .iter()
        .map(|(n, _, _)| *n)
        .filter(|n| !columns.iter().any(|c| c == n))
        .collect();
    let extra: Vec<&String> = columns
        .iter()
        .filter(|c| !declared.iter().any(|(n, _, _)| n == &c.as_str()))
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(at(format!(
            "the SQL's columns do not match `{}` (missing: [{}], extra: [{}])",
            f.returns.of,
            missing.join(", "),
            extra
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        )));
    }
    Ok(())
}

/// Replay the seed (§7.3) through the same write path as the dev surface.
fn seed(contract: &Contract, conn: &mut Connection) -> Result<(), EngineError> {
    // binding name -> the bound row's key value (as JSON)
    let mut bindings: HashMap<String, Json> = HashMap::new();

    for (index, row) in contract.seed.iter().enumerate() {
        let table = contract
            .table(&row.table)
            .expect("checked: seed tables exist");

        let mut body = Map::new();
        for (name, value) in &row.fields {
            let json = match value {
                SeedValue::Str(s) => Json::String(s.clone()),
                SeedValue::Int(n) => Json::Number((*n).into()),
                SeedValue::Float(f) => serde_json::Number::from_f64(*f)
                    .map(Json::Number)
                    .expect("checked: seed floats are finite"),
                SeedValue::Bool(b) => Json::Bool(*b),
                SeedValue::Ref { binding } => bindings
                    .get(binding)
                    .cloned()
                    .expect("checked: bindings resolve"),
            };
            body.insert(name.clone(), json);
        }

        let stored =
            insert_row(contract, table, conn, &body).map_err(|source| EngineError::Seed {
                index,
                table: table.name.clone(),
                source: Box::new(source),
            })?;

        if let Some(binding) = &row.binding {
            if let Some(key_field) = table.key.first() {
                if table.key.len() == 1 {
                    let key_value = stored
                        .get(key_field)
                        .cloned()
                        .expect("stored row has its key");
                    bindings.insert(binding.clone(), key_value);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "table user { key id: uuid = auto\n username: text unique }\n";

    fn open_err(fn_src: &str) -> String {
        let contract = spock_lang::compile(&format!("{BASE}{fn_src}")).expect("compiles");
        match open(&contract, None) {
            Ok(_) => panic!("expected a load failure for: {fn_src}"),
            Err(e) => e.to_string(),
        }
    }

    fn open_ok(fn_src: &str) {
        let contract = spock_lang::compile(&format!("{BASE}{fn_src}")).expect("compiles");
        open(&contract, None).expect("loads");
    }

    #[test]
    fn valid_fn_bodies_load() {
        open_ok("fn find(username: text) -> user? { unchecked sql(\"SELECT * FROM user WHERE username = :username\") }");
        // trailing semicolon and comment are tolerated (Batch skips them)
        open_ok("fn find(username: text) -> user? { unchecked sql(\"SELECT * FROM user WHERE username = :username; -- done\") }");
        // DML with RETURNING *
        open_ok("fn rename(user: user, username: text) -> user ! user_username_taken { unchecked sql(\"UPDATE user SET username = :username WHERE id = :user RETURNING *\") }");
        // a CTE is one statement
        open_ok("fn all() -> [user] { unchecked sql(\"WITH x AS (SELECT * FROM user) SELECT * FROM x\") }");
    }

    #[test]
    fn escape_inserts_borrow_the_declared_defaults() {
        // the load-bearing G4 claim, proven: a fn INSERT that omits
        // `= auto` / `= now` / literal-default columns gets the engine's
        // own values via the emitted DEFAULT clauses — including the
        // application-defined spock_uuid()/spock_now() builtins, which
        // SQLite happily calls from a column DEFAULT
        let contract = spock_lang::compile(
            "table note {\n\
               key id: uuid = auto\n\
               body: text\n\
               pinned: bool = false\n\
               at: timestamp = now\n\
             }\n\
             fn add(body: text) -> note {\n\
               unchecked sql(\"INSERT INTO note (body) VALUES (:body) RETURNING *\")\n\
             }\n\
             fn stamp() -> timestamp {\n\
               unchecked sql(\"SELECT spock_now()\")\n\
             }",
        )
        .expect("compiles");
        let mut conn = open(&contract, None).expect("loads");

        let f = contract.fn_def("add").expect("declared");
        let row = crate::func::call(
            &contract,
            f,
            &mut conn,
            &serde_json::Map::from_iter([("body".to_string(), "hi".into())]),
        )
        .expect("insert with omitted defaults");
        // a v7 uuid from the engine's own mint, not a hand-rolled v4
        let id = uuid::Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
        assert_eq!(id.get_version_num(), 7);
        assert_eq!(row["pinned"], serde_json::Value::Bool(false));
        // the stamp parses as the engine's own format
        use time::format_description::well_known::Rfc3339;
        time::OffsetDateTime::parse(row["at"].as_str().unwrap(), &Rfc3339)
            .expect("DEFAULT (spock_now()) is RFC 3339");

        // and the builtins are directly callable from a body
        let f = contract.fn_def("stamp").expect("declared");
        let now = crate::func::call(&contract, f, &mut conn, &serde_json::Map::new())
            .expect("spock_now() resolves in an escape body");
        time::OffsetDateTime::parse(now.as_str().unwrap(), &Rfc3339).expect("rfc 3339");
    }

    #[test]
    fn set_null_nulls_the_child_field_on_delete() {
        let contract = spock_lang::compile(
            "table user { key id: uuid = auto\n username: text unique }\n\
             table comment {\n\
               key id: uuid = auto\n\
               author: user\n\
               body: text\n\
               parent: comment? on delete set null\n\
             }\n\
             seed {\n\
               maya = user { username: \"maya\" }\n\
               top = comment { author: maya, body: \"parent\" }\n\
               comment { author: maya, body: \"reply\", parent: top }\n\
             }",
        )
        .expect("compiles");
        let mut conn = open(&contract, None).expect("loads");
        let table = contract.table("comment").expect("declared");

        let parent_id: String = conn
            .query_row(
                "SELECT id FROM comment WHERE body = 'parent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        crate::write::delete_row(
            &contract,
            table,
            &mut conn,
            &[rusqlite::types::Value::Text(parent_id)],
        )
        .expect("set null does not restrict the delete");

        // the reply survives, orphaned by design
        let (count, orphaned): (i64, i64) = conn
            .query_row(
                "SELECT count(*), count(*) FILTER (WHERE parent IS NULL) FROM comment",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((count, orphaned), (1, 1));
    }

    #[test]
    fn fn_sql_failures_gate_the_load() {
        // syntax error surfaces SQLite's own message
        assert!(open_err("fn f() -> user { unchecked sql(\"SELEC *\") }").contains("does not compile"));
        // unknown column resolves at prepare
        assert!(open_err("fn f() -> user { unchecked sql(\"SELECT * FROM user WHERE ghost = 1\") }")
            .contains("does not compile"));
        // exactly one statement per escape
        assert!(
            open_err("fn f() -> user { unchecked sql(\"SELECT * FROM user; SELECT * FROM user\") }")
                .contains("exactly one SQL statement")
        );
        // empty and comment-only bodies
        assert!(open_err("fn f() -> user { unchecked sql(\"\") }").contains("no SQL statement"));
        assert!(open_err("fn f() -> user { unchecked sql(\"-- nothing\") }").contains("no SQL statement"));
        // placeholder spellings: bare `?` is nameless (→ positional error);
        // `?1` and `@a` carry their spelling as the name (→ :param error)
        assert!(
            open_err("fn f(a: int) -> user { unchecked sql(\"SELECT * FROM user LIMIT ?\") }")
                .contains("positional")
        );
        assert!(
            open_err("fn f(a: int) -> user { unchecked sql(\"SELECT * FROM user LIMIT ?1\") }")
                .contains(":param")
        );
        assert!(open_err("fn f(a: int) -> user { unchecked sql(\"SELECT * FROM user LIMIT @a\") }")
            .contains(":param"));
        // both directions of the placeholder check
        assert!(
            open_err("fn f() -> user { unchecked sql(\"SELECT * FROM user WHERE id = :ghost\") }")
                .contains("not a parameter")
        );
        assert!(open_err("fn f(a: int) -> user { unchecked sql(\"SELECT * FROM user\") }")
            .contains("never used"));
        // a scalar return is exactly one column, any name
        assert!(open_err("fn f() -> int { unchecked sql(\"SELECT 1, 2\") }")
            .contains("exactly one column"));
        open_ok("fn f() -> int { unchecked sql(\"SELECT count(*) FROM user\") }");
        // column set-equality, duplicates, and DML-without-RETURNING
        assert!(open_err("fn f() -> user { unchecked sql(\"SELECT id FROM user\") }").contains("do not match"));
        assert!(
            open_err("fn f() -> user { unchecked sql(\"SELECT id AS username, username FROM user\") }")
                .contains("duplicate column")
        );
        assert!(open_err(
            "fn f(username: text) -> user { unchecked sql(\"UPDATE user SET username = :username\") }"
        )
        .contains("RETURNING"));
    }

    #[test]
    fn multi_statement_bodies_validate() {
        // a param may live in any statement; non-final DML needs no
        // RETURNING; only the last statement's columns are the contract's
        open_ok(
            "fn f(u: uuid, name: text) -> user {\n\
               unchecked sql(\"UPDATE user SET username = :name WHERE id = :u\")\n\
               unchecked sql(\"SELECT * FROM user WHERE id = :u\")\n\
             }",
        );
        // failures name their statement
        assert!(open_err(
            "fn f(u: uuid) -> user {\n\
               unchecked sql(\"UPDATE user SET username = 'x' WHERE id = :u\")\n\
               unchecked sql(\"SELECT * FROM user WHERE id = :ghost\")\n\
             }"
        )
        .contains("statement 2:"));
        // a declared param used in NO statement still fails
        assert!(open_err(
            "fn f(u: uuid, unused: int) -> user {\n\
               unchecked sql(\"UPDATE user SET username = 'x' WHERE id = :u\")\n\
               unchecked sql(\"SELECT * FROM user WHERE id = :u\")\n\
             }"
        )
        .contains("never used"));
        // the final statement still answers: shape mismatch fails there
        assert!(open_err(
            "fn f(u: uuid) -> user {\n\
               unchecked sql(\"SELECT id FROM user WHERE id = :u\")\n\
               unchecked sql(\"UPDATE user SET username = 'x' WHERE id = :u\")\n\
             }"
        )
        .contains("statement 2: the SQL returns no columns"));
    }
}

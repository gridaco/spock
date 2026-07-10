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
    for statement in ddl(contract) {
        conn.execute_batch(&statement)?;
    }
    validate_fns(contract, &conn)?;
    seed(contract, &mut conn)?;
    Ok(conn)
}

/// Validate every fn's SQL escape at load (§7.4), after DDL (tables must
/// exist) and before seed replay (a broken fn fails before any data
/// moves). `prepare` compiles without executing, so this checks syntax
/// and table/column resolution; the statement's own metadata checks the
/// placeholders (both directions) and the result columns against the
/// declared return shape. Total derivation, extended into the escape.
fn validate_fns(contract: &Contract, conn: &Connection) -> Result<(), EngineError> {
    for f in &contract.fns {
        let fail = |message: String| EngineError::Fn {
            name: f.name.clone(),
            message,
        };

        // exactly one statement: Batch skips comment/whitespace-only
        // segments, so trailing `;` and comments are tolerated
        let mut batch = rusqlite::Batch::new(conn, &f.sql);
        let stmt = match batch.next() {
            Err(e) => return Err(fail(format!("does not compile: {e}"))),
            Ok(None) => return Err(fail("body contains no SQL statement".into())),
            Ok(Some(stmt)) => stmt,
        };

        // placeholders, both directions: every SQL parameter is a declared
        // param, every declared param appears in the SQL
        let mut seen: Vec<String> = Vec::new();
        for i in 1..=stmt.parameter_count() {
            let Some(name) = stmt.parameter_name(i) else {
                return Err(fail(
                    "positional parameters are not allowed; use `:param`".into(),
                ));
            };
            let Some(bare) = name.strip_prefix(':') else {
                return Err(fail(format!(
                    "parameter `{name}` must use the `:param` form"
                )));
            };
            if f.params.iter().all(|p| p.name != bare) {
                return Err(fail(format!(
                    "the SQL names `{name}`, which is not a parameter of this fn"
                )));
            }
            seen.push(bare.to_string());
        }
        for p in &f.params {
            if !seen.contains(&p.name) {
                return Err(fail(format!(
                    "parameter `{}` is declared but never used in the SQL",
                    p.name
                )));
            }
        }

        // result columns must equal the declared shape's fields, by name —
        // known at prepare time for SELECT and RETURNING alike. An empty
        // column set also rejects DML without RETURNING: every fn returns
        // rows.
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        if columns.is_empty() {
            return Err(fail(format!(
                "the SQL returns no columns, but the fn returns `{}` (DML needs RETURNING)",
                f.returns.of
            )));
        }
        // a scalar return is one column, any name — there is no shape to
        // match against
        if f.returns.scalar {
            if columns.len() != 1 {
                return Err(fail(format!(
                    "the SQL returns {} columns, but the fn returns the scalar `{}` (exactly one column)",
                    columns.len(),
                    f.returns.of
                )));
            }
            drop(stmt);
            match batch.next() {
                Ok(None) => continue,
                Ok(Some(_)) => {
                    return Err(fail("body must be a single SQL statement".into()));
                }
                Err(e) => return Err(fail(format!("does not compile: {e}"))),
            }
        }
        let mut dedup = columns.clone();
        dedup.sort();
        dedup.dedup();
        if dedup.len() != columns.len() {
            return Err(fail(
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
            return Err(fail(format!(
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
        drop(stmt);

        // a second statement is a body error
        match batch.next() {
            Ok(None) => {}
            Ok(Some(_)) => {
                return Err(fail("body must be a single SQL statement".into()));
            }
            Err(e) => return Err(fail(format!("does not compile: {e}"))),
        }
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
        // exactly one statement
        assert!(
            open_err("fn f() -> user { unchecked sql(\"SELECT * FROM user; SELECT * FROM user\") }")
                .contains("single SQL statement")
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
}

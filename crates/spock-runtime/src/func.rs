//! Fn execution (docs/spec/v0.md §7.4). One call = one IMMEDIATE
//! transaction (the serializable-by-default stance, RFD 0005); arguments
//! bind by name (`:param`); rows map back **by column name** against the
//! declared return shape (validated total at load, engine.rs); arity
//! decides the result. Engine failures route cross-table to derived
//! errors — [`crate::write::map_fn_engine_error`].

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, ToSql, TransactionBehavior};
use serde_json::{Map, Value as Json};
use spock_lang::ir::{Contract, FnArity, FnDef};

use crate::error::ApiError;
use crate::value::{json_to_sql_scalar, sql_to_json_scalar};
use crate::write::map_fn_engine_error;

/// Call a declared fn with JSON arguments; returns the row / row-or-null /
/// rows its signature declares. For fn arguments `null` and absence mean
/// the same thing — there is no update carve-out here.
pub fn call(
    contract: &Contract,
    f: &FnDef,
    conn: &mut Connection,
    args: &Map<String, Json>,
) -> Result<Json, ApiError> {
    // unknown arguments are rejected; missing required ones refuse early
    for name in args.keys() {
        if f.params.iter().all(|p| &p.name != name) {
            return Err(ApiError::fn_unknown_arg(&f.name, name));
        }
    }
    let mut binds: Vec<(String, SqlValue)> = Vec::with_capacity(f.params.len());
    for p in &f.params {
        let provided = args.get(&p.name).filter(|v| !v.is_null());
        let value = match provided {
            Some(v) => json_to_sql_scalar(contract.value_type(&p.ty), v)
                .map_err(|expected| ApiError::fn_arg_mismatch(&f.name, &p.name, expected))?,
            None if p.optional => SqlValue::Null,
            None => {
                return Err(ApiError::bad_request(format!(
                    "fn `{}` requires `{}`",
                    f.name, p.name
                )));
            }
        };
        binds.push((format!(":{}", p.name), value));
    }

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;

    let rows: Vec<Json> = {
        let mut stmt = tx
            .prepare(&f.sql)
            .map_err(|e| map_fn_engine_error(contract, e))?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let declared = contract
            .output_fields(&f.returns.of)
            .ok_or_else(|| ApiError::internal("contract drift: fn return shape"))?;
        let params: Vec<(&str, &dyn ToSql)> = binds
            .iter()
            .map(|(n, v)| (n.as_str(), v as &dyn ToSql))
            .collect();
        let mut rows = stmt
            .query(params.as_slice())
            .map_err(|e| map_fn_engine_error(contract, e))?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| map_fn_engine_error(contract, e))? {
            let mut obj = Map::new();
            for (i, col) in columns.iter().enumerate() {
                let (_, ty, _) = declared
                    .iter()
                    .find(|(n, _, _)| *n == col.as_str())
                    .ok_or_else(|| ApiError::internal("contract drift: unvalidated column"))?;
                let value = row
                    .get_ref(i)
                    .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
                obj.insert(col.clone(), sql_to_json_scalar(contract.value_type(ty), value));
            }
            out.push(Json::Object(obj));
        }
        out
    };

    // arity is checked BEFORE commit: a violated `-> t` must roll its
    // writes back, not report an error over committed effects
    let result = match f.returns.arity {
        FnArity::One => match rows.len() {
            1 => rows.into_iter().next().expect("len checked"),
            // a write that did not happen must shout (graphql.md D1)
            0 => {
                return Err(ApiError::not_found(format!(
                    "fn `{}`: the SQL matched no row",
                    f.name
                )));
            }
            n => {
                return Err(ApiError::internal(format!(
                    "fn `{}` declares -> {} but the SQL returned {n} rows",
                    f.name, f.returns.of
                )));
            }
        },
        FnArity::Maybe => match rows.len() {
            0 => Json::Null,
            1 => rows.into_iter().next().expect("len checked"),
            n => {
                return Err(ApiError::internal(format!(
                    "fn `{}` declares -> {}? but the SQL returned {n} rows",
                    f.name, f.returns.of
                )));
            }
        },
        FnArity::Many => Json::Array(rows),
    };

    tx.commit()
        .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;

    const PROGRAM: &str = r#"
table user {
  key id: uuid = auto
  username: text unique
  bio: text?
}

table post {
  key id: uuid = auto
  author: user
  caption: text?
}

record stats { posts: int }

fn rename_user(user: user, username: text) -> user ! user_username_taken {
  sql("UPDATE user SET username = :username WHERE id = :user RETURNING *")
}

fn find_user(username: text) -> user? {
  sql("SELECT * FROM user WHERE username = :username")
}

fn user_stats(user: user) -> stats {
  sql("SELECT count(*) AS posts FROM post WHERE author = :user")
}

fn all_users() -> [user] {
  sql("SELECT * FROM user ORDER BY username")
}

fn force_post(author: uuid, caption: text?) -> post {
  sql("INSERT INTO post (id, author, caption) VALUES (:author, :author, :caption) RETURNING *")
}

fn clear_username(user: user) -> user {
  sql("UPDATE user SET username = NULL WHERE id = :user RETURNING *")
}

seed {
  maya = user { username: "maya", bio: "photographer" }
  luis = user { username: "luis" }
  post { author: maya, caption: "first light" }
}
"#;

    fn setup() -> (Contract, Connection) {
        let contract = spock_lang::compile(PROGRAM).expect("compiles");
        let conn = engine::open(&contract, None).expect("loads");
        (contract, conn)
    }

    fn args(pairs: &[(&str, Json)]) -> Map<String, Json> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn user_id(contract: &Contract, conn: &mut Connection, username: &str) -> String {
        let f = contract.fn_def("find_user").unwrap();
        let row = call(contract, f, conn, &args(&[("username", username.into())])).unwrap();
        row["id"].as_str().unwrap().to_string()
    }

    #[test]
    fn arities_and_reads() {
        let (contract, mut conn) = setup();
        // maybe: hit and null miss
        let f = contract.fn_def("find_user").unwrap();
        let row = call(&contract, f, &mut conn, &args(&[("username", "maya".into())])).unwrap();
        assert_eq!(row["bio"], "photographer");
        let miss = call(&contract, f, &mut conn, &args(&[("username", "ghost".into())])).unwrap();
        assert!(miss.is_null());
        // many
        let f = contract.fn_def("all_users").unwrap();
        let rows = call(&contract, f, &mut conn, &Map::new()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        // record return: an aggregate as a shape
        let maya = user_id(&contract, &mut conn, "maya");
        let f = contract.fn_def("user_stats").unwrap();
        let stats = call(&contract, f, &mut conn, &args(&[("user", maya.clone().into())])).unwrap();
        assert_eq!(stats["posts"], 1);
    }

    #[test]
    fn one_arity_write_and_misses() {
        let (contract, mut conn) = setup();
        let maya = user_id(&contract, &mut conn, "maya");
        let f = contract.fn_def("rename_user").unwrap();
        // hit: the row comes back renamed
        let row = call(
            &contract,
            f,
            &mut conn,
            &args(&[("user", maya.clone().into()), ("username", "maya_x".into())]),
        )
        .unwrap();
        assert_eq!(row["username"], "maya_x");
        // miss: -> t with zero rows shouts
        let ghost = uuid::Uuid::now_v7().to_string();
        let err = call(
            &contract,
            f,
            &mut conn,
            &args(&[("user", ghost.into()), ("username", "x".into())]),
        )
        .unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn argument_validation() {
        let (contract, mut conn) = setup();
        let f = contract.fn_def("find_user").unwrap();
        // unknown arg
        let err = call(&contract, f, &mut conn, &args(&[("ghost", "x".into())])).unwrap_err();
        assert_eq!(err.code, "unknown_field");
        // missing required
        let err = call(&contract, f, &mut conn, &Map::new()).unwrap_err();
        assert_eq!(err.code, "bad_request");
        // type mismatch (int for text)
        let err = call(&contract, f, &mut conn, &args(&[("username", 42.into())])).unwrap_err();
        assert_eq!(err.code, "type_mismatch");
        // malformed uuid for a ref param
        let f = contract.fn_def("rename_user").unwrap();
        let err = call(
            &contract,
            f,
            &mut conn,
            &args(&[("user", "not-a-uuid".into()), ("username", "x".into())]),
        )
        .unwrap_err();
        assert_eq!(err.code, "type_mismatch");
        // optional arg: absent and explicit null are the same (NULL bind).
        // (force_post reuses :author as the post id, so each call needs a
        // fresh author.)
        let maya = user_id(&contract, &mut conn, "maya");
        let luis = user_id(&contract, &mut conn, "luis");
        let f = contract.fn_def("force_post").unwrap();
        let row = call(&contract, f, &mut conn, &args(&[("author", maya.into())])).unwrap();
        assert!(row["caption"].is_null());
        let row = call(
            &contract,
            f,
            &mut conn,
            &args(&[("author", luis.into()), ("caption", Json::Null)]),
        )
        .unwrap();
        assert!(row["caption"].is_null());
    }

    #[test]
    fn cross_table_error_routing() {
        let (contract, mut conn) = setup();
        let luis = user_id(&contract, &mut conn, "luis");
        // unique violation inside fn SQL → user's own derived code
        let f = contract.fn_def("rename_user").unwrap();
        let err = call(
            &contract,
            f,
            &mut conn,
            &args(&[("user", luis.clone().into()), ("username", "maya".into())]),
        )
        .unwrap_err();
        assert_eq!(err.code, "user_username_taken");
        assert_eq!(err.table.as_deref(), Some("user"));
        assert_eq!(err.fields, vec!["username"]);
        // NOT NULL violation → derived required
        let f = contract.fn_def("clear_username").unwrap();
        let err = call(&contract, f, &mut conn, &args(&[("user", luis.into())])).unwrap_err();
        assert_eq!(err.code, "user_username_required");
        // FK violation → reserved bad_request (sqlite names no table)
        let ghost = uuid::Uuid::now_v7().to_string();
        let f = contract.fn_def("force_post").unwrap();
        let err = call(&contract, f, &mut conn, &args(&[("author", ghost.into())])).unwrap_err();
        assert_eq!(err.code, "bad_request");
    }
}

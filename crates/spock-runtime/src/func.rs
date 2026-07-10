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
        // scalar returns map column 0 as a bare value; shapes map by name
        let scalar_ty = f.returns.scalar_type();
        let declared = match &scalar_ty {
            Some(_) => Vec::new(),
            None => contract
                .output_fields(&f.returns.of)
                .ok_or_else(|| ApiError::internal("contract drift: fn return shape"))?,
        };
        let mut out = Vec::new();
        let last_index = f.sql.len().saturating_sub(1);
        for (index, sql) in f.sql.iter().enumerate() {
            let mut stmt = tx
                .prepare(sql)
                .map_err(|e| map_fn_engine_error(contract, e))?;
            let columns: Vec<String> =
                stmt.column_names().iter().map(|c| c.to_string()).collect();
            // each statement binds only the params it names (validated
            // total across the body at load)
            let params: Vec<(&str, &dyn ToSql)> = binds
                .iter()
                .filter(|(n, _)| matches!(stmt.parameter_index(n), Ok(Some(_))))
                .map(|(n, v)| (n.as_str(), v as &dyn ToSql))
                .collect();
            let mut rows = stmt
                .query(params.as_slice())
                .map_err(|e| map_fn_engine_error(contract, e))?;
            if index < last_index {
                // a guard or an effect: run it to completion (a guard's
                // refusal fires while stepping), discard its rows
                while rows
                    .next()
                    .map_err(|e| map_fn_engine_error(contract, e))?
                    .is_some()
                {}
                continue;
            }
            // the last statement answers
            while let Some(row) = rows.next().map_err(|e| map_fn_engine_error(contract, e))? {
            if let Some(ty) = &scalar_ty {
                let value = row
                    .get_ref(0)
                    .map_err(|e| ApiError::internal(format!("sqlite: {e}")))?;
                let json = sql_to_json_scalar(ty, value);
                // a NULL under a non-optional scalar is the body breaking
                // its contract — surface it (GraphQL would otherwise serve
                // null under a non-null type); `-> t?` keeps null
                if json.is_null() && f.returns.arity != FnArity::Maybe {
                    return Err(ApiError::internal(format!(
                        "fn `{}`: the SQL returned NULL for the non-optional scalar `{of}` (declare -> {of}? if null is possible)",
                        f.name,
                        of = f.returns.of
                    )));
                }
                out.push(json);
                continue;
            }
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
record ratio { value: float }

fn rename_user(user: user, username: text) -> user ! user_username_taken {
  unchecked sql("UPDATE user SET username = :username WHERE id = :user RETURNING *")
}

fn find_user(username: text) -> user? {
  unchecked sql("SELECT * FROM user WHERE username = :username")
}

fn user_stats(user: user) -> stats {
  unchecked sql("SELECT count(*) AS posts FROM post WHERE author = :user")
}

fn all_users() -> [user] {
  unchecked sql("SELECT * FROM user ORDER BY username")
}

fn force_post(author: uuid, caption: text?) -> post {
  unchecked sql("INSERT INTO post (id, author, caption) VALUES (:author, :author, :caption) RETURNING *")
}

fn clear_username(user: user) -> user {
  unchecked sql("UPDATE user SET username = NULL WHERE id = :user RETURNING *")
}

fn double(f: float) -> ratio {
  unchecked sql("SELECT :f * 2.0 AS value")
}

fn user_count() -> int {
  unchecked sql("SELECT count(*) FROM user")
}

fn bio_of(username: text) -> text? {
  unchecked sql("SELECT bio FROM user WHERE username = :username")
}

fn usernames() -> [text] {
  unchecked sql("SELECT username FROM user ORDER BY username")
}

fn lying_count() -> int {
  unchecked sql("SELECT NULL")
}

fn bios() -> [text] {
  unchecked sql("SELECT bio FROM user")
}

fn post_and_count(author: user, caption: text) -> int {
  unchecked sql("INSERT INTO post (author, caption) VALUES (:author, :caption)")
  unchecked sql("SELECT count(*) FROM post WHERE author = :author")
}

fn insert_then_miss(author: user) -> post {
  unchecked sql("INSERT INTO post (author, caption) VALUES (:author, 'doomed')")
  unchecked sql("SELECT * FROM post WHERE caption = 'no such caption'")
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
        // float param binds Real; the result maps back as a JSON number
        let f = contract.fn_def("double").unwrap();
        let row = call(&contract, f, &mut conn, &args(&[("f", 1.5.into())])).unwrap();
        assert_eq!(row["value"], 3.0);
        // a whole JSON number is a legal float argument
        let row = call(&contract, f, &mut conn, &args(&[("f", 2.into())])).unwrap();
        assert_eq!(row["value"], 4.0);
    }

    #[test]
    fn scalar_returns() {
        let (contract, mut conn) = setup();
        // -> int: a bare JSON number, no wrapper shape
        let f = contract.fn_def("user_count").unwrap();
        assert_eq!(call(&contract, f, &mut conn, &Map::new()).unwrap(), 2);
        // -> text?: a value... (bio is a nullable column: a stored NULL and
        // a missing row both surface as null)
        let f = contract.fn_def("bio_of").unwrap();
        let hit = call(&contract, f, &mut conn, &args(&[("username", "maya".into())])).unwrap();
        assert_eq!(hit, "photographer");
        let none = call(&contract, f, &mut conn, &args(&[("username", "luis".into())])).unwrap();
        assert!(none.is_null());
        let miss = call(&contract, f, &mut conn, &args(&[("username", "ghost".into())])).unwrap();
        assert!(miss.is_null());
        // -> [text]: bare values in order
        let f = contract.fn_def("usernames").unwrap();
        let rows = call(&contract, f, &mut conn, &Map::new()).unwrap();
        assert_eq!(rows, serde_json::json!(["luis", "maya"]));
        // a NULL under a non-optional scalar is the body's contract
        // violation — internal, never a null under a non-null type
        let f = contract.fn_def("lying_count").unwrap();
        let err = call(&contract, f, &mut conn, &Map::new()).unwrap_err();
        assert_eq!(err.code, "internal");
        // same rule per element for lists (luis has no bio)
        let f = contract.fn_def("bios").unwrap();
        let err = call(&contract, f, &mut conn, &Map::new()).unwrap_err();
        assert_eq!(err.code, "internal");
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
    fn multi_statement_bodies() {
        let (contract, mut conn) = setup();
        let maya = user_id(&contract, &mut conn, "maya");
        // earlier statements are effects, the last statement answers:
        // the count the fn returns already sees its own INSERT
        let f = contract.fn_def("post_and_count").unwrap();
        let n = call(
            &contract,
            f,
            &mut conn,
            &args(&[("author", maya.clone().into()), ("caption", "second".into())]),
        )
        .unwrap();
        assert_eq!(n, 2); // the seed post + this one
        // one transaction: a violated `-> t` arity rolls EVERY statement
        // back, including earlier effects
        let f = contract.fn_def("insert_then_miss").unwrap();
        let err = call(&contract, f, &mut conn, &args(&[("author", maya.clone().into())])).unwrap_err();
        assert_eq!(err.code, "not_found");
        let f = contract.fn_def("post_and_count").unwrap();
        let n = call(
            &contract,
            f,
            &mut conn,
            &args(&[("author", maya.into()), ("caption", "third".into())]),
        )
        .unwrap();
        assert_eq!(n, 3); // 'doomed' never landed
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

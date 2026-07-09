//! The write path (docs/spec/v0.md §7.2). One code path for every write:
//! the dev surface and the seed both come through here, so a seed row and a
//! POST are validated identically.

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde_json::{Map, Value as Json};
use spock_lang::ir::{Contract, DefaultValue, ErrorKind, OnDelete, Table, Type};
use time::format_description::well_known::Rfc3339;

use crate::error::ApiError;
use crate::value::{json_to_sql, sql_to_json};

/// Insert one row (§7.2), returning the stored row as JSON.
pub fn insert_row(
    contract: &Contract,
    table: &Table,
    conn: &mut Connection,
    body: &Map<String, Json>,
) -> Result<Json, ApiError> {
    // 1. unknown fields are rejected
    for name in body.keys() {
        if table.field(name).is_none() {
            return Err(ApiError::unknown_field(&table.name, name));
        }
    }

    // 2–3. type-check provided values, apply defaults, enforce required.
    // `null` is absence (§5.1): identical to omitting the field.
    let mut values: Vec<SqlValue> = Vec::with_capacity(table.fields.len());
    for field in &table.fields {
        let provided = body.get(&field.name).filter(|v| !v.is_null());
        let value = match provided {
            Some(v) => json_to_sql(contract, table, field, v)?,
            None => match &field.default {
                Some(DefaultValue::Auto) => SqlValue::Text(uuid::Uuid::now_v7().to_string()),
                Some(DefaultValue::Now) => SqlValue::Text(
                    time::OffsetDateTime::now_utc()
                        .format(&Rfc3339)
                        .expect("rfc3339 format"),
                ),
                Some(DefaultValue::Str { value }) => SqlValue::Text(value.clone()),
                Some(DefaultValue::Int { value }) => SqlValue::Integer(*value),
                Some(DefaultValue::Bool { value }) => SqlValue::Integer(*value as i64),
                None if field.optional => SqlValue::Null,
                None => {
                    let err = table
                        .error_for(ErrorKind::Required, &[&field.name])
                        .ok_or_else(|| ApiError::internal("missing derived required error"))?;
                    return Err(ApiError::derived(
                        &table.name,
                        err,
                        format!("{}.{} is required", table.name, field.name),
                    ));
                }
            },
        };
        values.push(value);
    }

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_internal)?;

    // 4. provided references must exist
    for (field, value) in table.fields.iter().zip(&values) {
        let Type::Ref { table: target, .. } = &field.ty else {
            continue;
        };
        if matches!(value, SqlValue::Null) {
            continue;
        }
        let target_table = contract
            .table(target)
            .ok_or_else(|| ApiError::internal("checked: ref target exists"))?;
        let sql = format!(
            "SELECT 1 FROM \"{}\" WHERE \"{}\" = ?1 LIMIT 1",
            target_table.name, target_table.key[0]
        );
        let exists: Option<i64> = tx
            .query_row(&sql, [value], |row| row.get(0))
            .optional()
            .map_err(sqlite_internal)?;
        if exists.is_none() {
            let err = table
                .error_for(ErrorKind::RefNotFound, &[&field.name])
                .ok_or_else(|| ApiError::internal("missing derived ref error"))?;
            return Err(ApiError::derived(
                &table.name,
                err,
                format!(
                    "{}.{} references a {} that does not exist",
                    table.name, field.name, target
                ),
            ));
        }
    }

    // 5. the engine insert; unique/key violations map to derived codes
    let columns = table
        .fields
        .iter()
        .map(|f| format!("\"{}\"", f.name))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=table.fields.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO \"{}\" ({columns}) VALUES ({placeholders})",
        table.name
    );
    tx.execute(&sql, rusqlite::params_from_iter(values.iter()))
        .map_err(|e| map_insert_error(table, e))?;

    // read the stored row back by key
    let row = select_by_key(contract, table, &tx, &key_values(table, &values))?
        .ok_or_else(|| ApiError::internal("inserted row not found"))?;

    tx.commit().map_err(sqlite_internal)?;
    Ok(row)
}

/// Delete one row by key (§7.2): inbound `restrict` references are checked
/// first; `cascade` references delegate to the engine.
pub fn delete_row(
    contract: &Contract,
    table: &Table,
    conn: &mut Connection,
    key: &SqlValue,
) -> Result<(), ApiError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_internal)?;

    let key_field = &table.key[0];
    let exists: Option<i64> = tx
        .query_row(
            &format!(
                "SELECT 1 FROM \"{}\" WHERE \"{}\" = ?1 LIMIT 1",
                table.name, key_field
            ),
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite_internal)?;
    if exists.is_none() {
        return Err(ApiError::not_found(format!(
            "no {} row with this key",
            table.name
        )));
    }

    for (child, field) in contract.inbound_refs(&table.name) {
        let Type::Ref { on_delete, .. } = &field.ty else {
            continue;
        };
        if *on_delete != OnDelete::Restrict {
            continue;
        }
        let referenced: Option<i64> = tx
            .query_row(
                &format!(
                    "SELECT 1 FROM \"{}\" WHERE \"{}\" = ?1 LIMIT 1",
                    child.name, field.name
                ),
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_internal)?;
        if referenced.is_some() {
            let err = table
                .error_for(ErrorKind::Restricted, &[])
                .ok_or_else(|| ApiError::internal("missing derived restricted error"))?;
            return Err(ApiError::derived(
                &table.name,
                err,
                format!("cannot delete: referenced by {}.{}", child.name, field.name),
            ));
        }
    }

    tx.execute(
        &format!(
            "DELETE FROM \"{}\" WHERE \"{}\" = ?1",
            table.name, key_field
        ),
        [key],
    )
    .map_err(|e| map_delete_error(table, e))?;

    tx.commit().map_err(sqlite_internal)?;
    Ok(())
}

/// Read one row by its (possibly composite) key values.
pub fn select_by_key(
    contract: &Contract,
    table: &Table,
    conn: &Connection,
    key: &[SqlValue],
) -> Result<Option<Json>, ApiError> {
    let predicate = table
        .key
        .iter()
        .enumerate()
        .map(|(i, name)| format!("\"{name}\" = ?{}", i + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    let sql = format!("SELECT * FROM \"{}\" WHERE {predicate}", table.name);
    let mut stmt = conn.prepare(&sql).map_err(sqlite_internal)?;
    let mut rows = stmt
        .query(rusqlite::params_from_iter(key.iter()))
        .map_err(sqlite_internal)?;
    match rows.next().map_err(sqlite_internal)? {
        Some(row) => Ok(Some(row_to_json(contract, table, row)?)),
        None => Ok(None),
    }
}

/// Render a `SELECT *` row as JSON (column order = field order, §7.1).
pub fn row_to_json(
    contract: &Contract,
    table: &Table,
    row: &rusqlite::Row<'_>,
) -> Result<Json, ApiError> {
    let mut out = Map::new();
    for (i, field) in table.fields.iter().enumerate() {
        let value = row.get_ref(i).map_err(sqlite_internal)?;
        out.insert(field.name.clone(), sql_to_json(contract, field, value));
    }
    Ok(Json::Object(out))
}

fn key_values(table: &Table, values: &[SqlValue]) -> Vec<SqlValue> {
    table
        .key
        .iter()
        .map(|name| {
            let idx = table
                .fields
                .iter()
                .position(|f| &f.name == name)
                .expect("checked: key fields exist");
            values[idx].clone()
        })
        .collect()
}

fn sqlite_internal(e: rusqlite::Error) -> ApiError {
    ApiError::internal(format!("sqlite: {e}"))
}

/// Map an engine insert error to a derived error (§6.1). Unique and
/// primary-key violations carry `table.column` lists in their message.
fn map_insert_error(table: &Table, e: rusqlite::Error) -> ApiError {
    use rusqlite::ffi;
    if let rusqlite::Error::SqliteFailure(err, Some(msg)) = &e {
        let is_pk = err.extended_code == ffi::SQLITE_CONSTRAINT_PRIMARYKEY;
        let is_unique = err.extended_code == ffi::SQLITE_CONSTRAINT_UNIQUE;
        if is_pk || is_unique {
            let fields = violated_columns(msg);
            let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
            // primary key first, then unique field/groups
            if let Some(derr) = table
                .error_for(ErrorKind::Key, &field_refs)
                .or_else(|| table.error_for(ErrorKind::Unique, &field_refs))
            {
                let what = fields.join(", ");
                return ApiError::derived(
                    &table.name,
                    derr,
                    match derr.kind {
                        ErrorKind::Key => format!("a {} with this key already exists", table.name),
                        _ => format!("{}.{what} is already taken", table.name),
                    },
                );
            }
        }
    }
    sqlite_internal(e)
}

/// Map an engine delete error. Direct restricts are pre-checked; a cascade
/// chain can still hit a deeper `restrict`, which surfaces here.
fn map_delete_error(table: &Table, e: rusqlite::Error) -> ApiError {
    use rusqlite::ffi;
    if let rusqlite::Error::SqliteFailure(err, _) = &e {
        if err.extended_code == ffi::SQLITE_CONSTRAINT_FOREIGNKEY
            || err.extended_code == ffi::SQLITE_CONSTRAINT_TRIGGER
        {
            if let Some(derr) = table.error_for(ErrorKind::Restricted, &[]) {
                return ApiError::derived(
                    &table.name,
                    derr,
                    "cannot delete: a cascading delete is restricted downstream",
                );
            }
        }
    }
    sqlite_internal(e)
}

/// Parse `"UNIQUE constraint failed: user.a, user.b"` → `["a", "b"]`.
fn violated_columns(msg: &str) -> Vec<String> {
    msg.split(':')
        .nth(1)
        .unwrap_or("")
        .split(',')
        .filter_map(|part| part.trim().split('.').nth(1))
        .map(str::to_string)
        .collect()
}

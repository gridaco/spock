//! JSON ↔ SQL value conversion, governed by the contract's types
//! (docs/spec/v0.md §5.1). Validation happens here, before the engine.

use rusqlite::types::{Value as SqlValue, ValueRef};
use serde_json::Value as Json;
use spock_lang::ir::{Contract, Field, Table, Type};
use time::format_description::well_known::Rfc3339;

use crate::error::ApiError;

/// Validate a provided JSON value against a field's *value* type and convert
/// it to a SQL value. `Null` is handled by the caller (absence, §5.1).
pub fn json_to_sql(
    contract: &Contract,
    table: &Table,
    field: &Field,
    value: &Json,
) -> Result<SqlValue, ApiError> {
    let value_type = contract.value_type(&field.ty);
    let mismatch = |expected: &str| ApiError::type_mismatch(&table.name, &field.name, expected);
    match value_type {
        Type::Text => match value {
            Json::String(s) => Ok(SqlValue::Text(s.clone())),
            _ => Err(mismatch("a string")),
        },
        Type::Int => match value.as_i64() {
            Some(n) if !value.is_boolean() => Ok(SqlValue::Integer(n)),
            _ => Err(mismatch("an integer")),
        },
        Type::Bool => match value {
            Json::Bool(b) => Ok(SqlValue::Integer(*b as i64)),
            _ => Err(mismatch("a boolean")),
        },
        Type::Uuid => match value {
            Json::String(s) => match uuid::Uuid::parse_str(s) {
                Ok(u) => Ok(SqlValue::Text(u.to_string())),
                Err(_) => Err(mismatch("a uuid string")),
            },
            _ => Err(mismatch("a uuid string")),
        },
        Type::Timestamp => match value {
            Json::String(s) => match time::OffsetDateTime::parse(s, &Rfc3339) {
                Ok(t) => Ok(SqlValue::Text(
                    t.format(&Rfc3339).expect("rfc3339 roundtrip"),
                )),
                Err(_) => Err(mismatch("an RFC 3339 timestamp string")),
            },
            _ => Err(mismatch("an RFC 3339 timestamp string")),
        },
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

/// Render one SQLite column value as JSON, governed by the field type.
pub fn sql_to_json(contract: &Contract, field: &Field, value: ValueRef<'_>) -> Json {
    let value_type = contract.value_type(&field.ty);
    match value {
        ValueRef::Null => Json::Null,
        ValueRef::Integer(n) => match value_type {
            Type::Bool => Json::Bool(n != 0),
            _ => Json::Number(n.into()),
        },
        ValueRef::Text(bytes) => Json::String(String::from_utf8_lossy(bytes).into_owned()),
        // v0 never stores reals or blobs; render defensively.
        ValueRef::Real(f) => serde_json::Number::from_f64(f)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        ValueRef::Blob(_) => Json::Null,
    }
}

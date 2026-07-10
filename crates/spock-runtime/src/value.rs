//! JSON ↔ SQL value conversion, governed by the contract's types
//! (docs/spec/v0.md §5.1). Validation happens here, before the engine.

use rusqlite::types::{Value as SqlValue, ValueRef};
use serde_json::Value as Json;
use spock_lang::ir::{Contract, Field, Table, Type};
use time::format_description::well_known::Rfc3339;

use crate::error::ApiError;

/// The engine's uuid mint — UUIDv7, the same generator `= auto` uses.
/// Exposed to escape bodies as the SQL builtin `spock_uuid()`.
pub fn new_uuid() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// The canonical stored timestamp: UTC, fixed six-digit fractional
/// seconds. Fixed width matters — RFC 3339 permits trimmed fractions,
/// and `00:00:00.5Z` sorts lexicographically *after* `00:00:00.51Z`;
/// canonical width makes text order chronological order, which the
/// example's cursors and ORDER BYs rely on. Every timestamp the runtime
/// stores — defaults, seed literals, wire inputs — passes through here.
pub fn canon_timestamp(t: time::OffsetDateTime) -> String {
    let canon = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    );
    t.to_offset(time::UtcOffset::UTC)
        .format(&canon)
        .expect("canonical timestamp formats")
}

/// The engine's clock — the same stamp `= now` uses. Exposed to escape
/// bodies as the SQL builtin `spock_now()`.
pub fn now_utc() -> String {
    canon_timestamp(time::OffsetDateTime::now_utc())
}

/// A wire string (query-string value, path segment) as the JSON a *value*
/// type expects (refs already chased). Values that do not parse pass
/// through as strings — the shared call path then names the canonical
/// `type_mismatch` — and the text-shaped types (text, uuid, timestamp)
/// pass through untouched by construction.
pub fn text_to_json_scalar(value_type: &Type, raw: String) -> Json {
    match value_type {
        Type::Int => match raw.parse::<i64>() {
            Ok(n) => Json::from(n),
            Err(_) => Json::String(raw),
        },
        Type::Float => match raw.parse::<f64>() {
            Ok(v) if v.is_finite() => serde_json::json!(v),
            _ => Json::String(raw),
        },
        Type::Bool => match raw.as_str() {
            "true" => Json::Bool(true),
            "false" => Json::Bool(false),
            _ => Json::String(raw),
        },
        _ => Json::String(raw),
    }
}

/// Validate a JSON value against a *value* type (refs already chased) and
/// convert it. `Err` is the "expected …" description — callers supply
/// their own context (a table field, a fn argument).
pub fn json_to_sql_scalar(value_type: &Type, value: &Json) -> Result<SqlValue, &'static str> {
    match value_type {
        Type::Text => match value {
            Json::String(s) => Ok(SqlValue::Text(s.clone())),
            _ => Err("a string"),
        },
        Type::Int => match value.as_i64() {
            Some(n) if !value.is_boolean() => Ok(SqlValue::Integer(n)),
            _ => Err("an integer"),
        },
        // a whole JSON number is a legal float value; booleans are not
        Type::Float => match value.as_f64() {
            Some(f) if !value.is_boolean() => Ok(SqlValue::Real(f)),
            _ => Err("a number"),
        },
        Type::Bool => match value {
            Json::Bool(b) => Ok(SqlValue::Integer(*b as i64)),
            _ => Err("a boolean"),
        },
        Type::Uuid => match value {
            Json::String(s) => match uuid::Uuid::parse_str(s) {
                Ok(u) => Ok(SqlValue::Text(u.to_string())),
                Err(_) => Err("a uuid string"),
            },
            _ => Err("a uuid string"),
        },
        Type::Timestamp => match value {
            // inputs are RFC 3339 in any offset; storage is canonical
            Json::String(s) => match time::OffsetDateTime::parse(s, &Rfc3339) {
                Ok(t) => Ok(SqlValue::Text(canon_timestamp(t))),
                Err(_) => Err("an RFC 3339 timestamp string"),
            },
            _ => Err("an RFC 3339 timestamp string"),
        },
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

/// Validate a provided JSON value against a field's *value* type and convert
/// it to a SQL value. `Null` is handled by the caller (absence, §5.1).
pub fn json_to_sql(
    contract: &Contract,
    table: &Table,
    field: &Field,
    value: &Json,
) -> Result<SqlValue, ApiError> {
    json_to_sql_scalar(contract.value_type(&field.ty), value)
        .map_err(|expected| ApiError::type_mismatch(&table.name, &field.name, expected))
}

/// Render one SQLite column value as JSON, governed by a *value* type.
pub fn sql_to_json_scalar(value_type: &Type, value: ValueRef<'_>) -> Json {
    match value {
        ValueRef::Null => Json::Null,
        ValueRef::Integer(n) => match value_type {
            Type::Bool => Json::Bool(n != 0),
            _ => Json::Number(n.into()),
        },
        ValueRef::Text(bytes) => Json::String(String::from_utf8_lossy(bytes).into_owned()),
        // float columns; NaN/infinity have no JSON spelling → null
        ValueRef::Real(f) => serde_json::Number::from_f64(f)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        // v0 never stores blobs; render defensively.
        ValueRef::Blob(_) => Json::Null,
    }
}

/// Render one SQLite column value as JSON, governed by the field type.
pub fn sql_to_json(contract: &Contract, field: &Field, value: ValueRef<'_>) -> Json {
    sql_to_json_scalar(contract.value_type(&field.ty), value)
}

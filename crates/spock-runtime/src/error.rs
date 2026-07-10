//! The wire error envelope (docs/spec/v0.md §8.1).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use spock_lang::ir::{DerivedError, ErrorKind};

/// An error as it leaves the runtime: derived (schema-owned) or reserved
/// (protocol-owned). Renders as the §8.1 envelope.
#[derive(Clone, Debug)]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub kind: &'static str,
    pub table: Option<String>,
    pub fields: Vec<String>,
    pub message: String,
}

impl ApiError {
    pub fn derived(table: &str, err: &DerivedError, message: impl Into<String>) -> Self {
        ApiError {
            status: err.status,
            code: err.code.clone(),
            kind: kind_str(err.kind),
            table: Some(table.to_string()),
            fields: err.fields.clone(),
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError {
            status: 400,
            code: "bad_request".into(),
            kind: "bad_request",
            table: None,
            fields: vec![],
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        ApiError {
            status: 404,
            code: "not_found".into(),
            kind: "not_found",
            table: None,
            fields: vec![],
            message: message.into(),
        }
    }

    pub fn unknown_field(table: &str, field: &str) -> Self {
        ApiError {
            status: 422,
            code: "unknown_field".into(),
            kind: "unknown_field",
            table: Some(table.to_string()),
            fields: vec![field.to_string()],
            message: format!("`{table}` has no field `{field}`"),
        }
    }

    pub fn type_mismatch(table: &str, field: &str, expected: &str) -> Self {
        ApiError {
            status: 422,
            code: "type_mismatch".into(),
            kind: "type_mismatch",
            table: Some(table.to_string()),
            fields: vec![field.to_string()],
            message: format!("`{table}.{field}` expects {expected}"),
        }
    }

    /// A fn argument that failed its declared type. Same code as a
    /// field mismatch (`type_mismatch`); the fn has no table.
    pub fn fn_arg_mismatch(fn_name: &str, param: &str, expected: &str) -> Self {
        ApiError {
            status: 422,
            code: "type_mismatch".into(),
            kind: "type_mismatch",
            table: None,
            fields: vec![param.to_string()],
            message: format!("fn `{fn_name}` argument `{param}` expects {expected}"),
        }
    }

    /// An argument the fn does not declare.
    pub fn fn_unknown_arg(fn_name: &str, arg: &str) -> Self {
        ApiError {
            status: 422,
            code: "unknown_field".into(),
            kind: "unknown_field",
            table: None,
            fields: vec![arg.to_string()],
            message: format!("fn `{fn_name}` has no parameter `{arg}`"),
        }
    }

    /// A refusal the fn minted in its `!` clause and raised from the
    /// body via `spock_refuse` (§7.4, RFD 0012). The conflict family —
    /// a product rule said no — beside the derived constraint
    /// violations.
    pub fn refused(code: &str, fn_name: &str) -> Self {
        ApiError {
            status: 409,
            code: code.into(),
            kind: "refused",
            table: None,
            fields: vec![],
            message: format!("fn `{fn_name}` refused: {code}"),
        }
    }

    /// A `mut` fn called through a safe method (GET rpc, §7.4). Same
    /// reserved code as other caller errors; the status says the rest.
    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        ApiError {
            status: 405,
            code: "bad_request".into(),
            kind: "bad_request",
            table: None,
            fields: vec![],
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        ApiError {
            status: 500,
            code: "internal".into(),
            kind: "internal",
            table: None,
            fields: vec![],
            message: message.into(),
        }
    }
}

pub fn kind_str(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::Key => "key",
        ErrorKind::Unique => "unique",
        ErrorKind::Required => "required",
        ErrorKind::RefNotFound => "ref_not_found",
        ErrorKind::Restricted => "restricted",
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = json!({
            "error": {
                "code": self.code,
                "kind": self.kind,
                "table": self.table,
                "fields": self.fields,
                "message": self.message,
            }
        });
        (status, Json(body)).into_response()
    }
}

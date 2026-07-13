//! The filter sub-language (RFD 0021): one owned predicate IR that both
//! borrowed frontends — Hasura `bool_exp` (GraphQL) and PostgREST operators
//! (REST) — lower into, then to one SQLite `WHERE`. This module owns the IR,
//! the injection-proof lowering, ordering (with a forced stable total order),
//! and paging (page cap + offset-depth ceiling). The GraphQL frontend parses
//! into [`Predicate`] in `graphql.rs`; the REST frontend is [`parse_rest`]
//! here. Nothing outside this module emits SQL for a filter.
//!
//! Safety is structural, not escaping-based (RFD 0021 §8): operators come
//! from a closed enum mapped through a fixed match arm; columns are resolved
//! against the declared field set by the frontend, then double-quoted; and
//! every *value* is a bound `?` parameter — the only length-variable emission
//! is the count of `?` in an `IN` list.

use rusqlite::types::Value as SqlValue;
use serde_json::Value as Json;
use spock_lang::ir::{Contract, Field, Table, Type};

use crate::error::ApiError;
use crate::value::{json_to_sql_scalar, text_to_json_scalar};

/// Page *size* (deviation D2): default 50, ceiling 200.
pub const DEFAULT_LIMIT: i64 = 50;
pub const MAX_LIMIT: i64 = 200;
/// Page *depth* (RFD 0021 §7, §14.1): offset is O(n) and holds the single
/// connection lock, so a window ceiling bounds it. Deep paging is the
/// deferred keyset cursor's job, not a deep offset's.
pub const MAX_OFFSET: i64 = 10_000;
/// Tree-global bound-parameter budget (§8.7). Below SQLite's default
/// `SQLITE_LIMIT_VARIABLE_NUMBER` (32766): a predicate whose *total* `?`
/// count — across every leaf, not any one `IN` list — would exceed it is a
/// `bad_request`, never a `prepare()`-time 500.
pub const MAX_FILTER_PARAMS: usize = 30_000;

/// The single-value comparison operators. `_in`/`_nin` (list) and `_is_null`
/// have their own [`Predicate`] variants; this is the closed set that maps
/// through a fixed match arm to a SQL token — no client operator string ever
/// reaches SQL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    /// Case-insensitive (ASCII) `LIKE`. `_like`/`like` (case-sensitive) is
    /// refused: SQLite `LIKE` is ASCII-case-insensitive and the case-sensitive
    /// form needs the banned `PRAGMA case_sensitive_like` (RFD 0021 §4).
    Ilike,
}

impl CmpOp {
    fn token(self) -> &'static str {
        match self {
            CmpOp::Eq => "=",
            CmpOp::Neq => "<>",
            CmpOp::Gt => ">",
            CmpOp::Gte => ">=",
            CmpOp::Lt => "<",
            CmpOp::Lte => "<=",
            CmpOp::Ilike => "LIKE",
        }
    }
}

/// The owned predicate tree (RFD 0021 §3), mirroring Hasura `bool_exp` and the
/// PostgREST group grammar — the same shape. The comparison value is a
/// `SqlValue` (v0 `Operand::Lit`); the v1 policy claims binding is a reserved
/// second operand variant — "the same tree with one more binding" — and the
/// cross-table `Exists` node is reserved but never constructed in v0 (§11).
pub enum Predicate {
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    Cmp {
        col: String,
        op: CmpOp,
        value: SqlValue,
    },
    In {
        col: String,
        negated: bool,
        values: Vec<SqlValue>,
    },
    IsNull {
        col: String,
        negated: bool,
    },
    /// A constant truth value — the no-filter case (`Const(true)`) and the
    /// empty-set canonicalizations of §8.6.
    Const(bool),
}

/// A sort direction. The lowering always emits explicit `NULLS` placement:
/// SQLite's implicit default is inverted from Postgres/Hasura, so it is never
/// inherited (RFD 0021 §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Asc,
    Desc,
}

impl Dir {
    fn sql(self) -> &'static str {
        match self {
            Dir::Asc => "ASC NULLS LAST",
            Dir::Desc => "DESC NULLS FIRST",
        }
    }
}

/// Double-quote a SQL identifier, doubling any embedded `"` (§8.3). Columns
/// are validated lowercase identifiers by the time they arrive, so this is
/// belt-and-suspenders — a filter identifier can never degrade into a literal.
fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn bool_lit(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}

/// Lower a predicate to a SQL boolean expression, pushing bound values onto
/// `params` in emission order (§8). The caller inlines the already-validated
/// integer `LIMIT`/`OFFSET`, so `params` carries only filter operands.
pub fn lower_where(pred: &Predicate, params: &mut Vec<SqlValue>) -> String {
    match pred {
        Predicate::Const(b) => bool_lit(*b).to_string(),
        Predicate::And(ps) => join(ps, "AND", true, params),
        Predicate::Or(ps) => join(ps, "OR", false, params),
        Predicate::Not(p) => format!("NOT ({})", lower_where(p, params)),
        Predicate::Cmp { col, op, value } => {
            params.push(value.clone());
            match op {
                // ESCAPE makes a literal % or _ in the pattern inert; the
                // client's own %/_ stay wildcards (§4).
                CmpOp::Ilike => format!("{} LIKE ? ESCAPE '\\'", quote(col)),
                _ => format!("{} {} ?", quote(col), op.token()),
            }
        }
        Predicate::In {
            col,
            negated,
            values,
        } => {
            if values.is_empty() {
                // `_in []` matches nothing; `_nin []` matches everything (§8.6)
                return bool_lit(*negated).to_string();
            }
            let marks = vec!["?"; values.len()].join(", ");
            params.extend(values.iter().cloned());
            let not = if *negated { "NOT " } else { "" };
            format!("{} {not}IN ({marks})", quote(col))
        }
        Predicate::IsNull { col, negated } => {
            let not = if *negated { "NOT " } else { "" };
            format!("{} IS {not}NULL", quote(col))
        }
    }
}

fn join(ps: &[Predicate], sep: &str, empty: bool, params: &mut Vec<SqlValue>) -> String {
    if ps.is_empty() {
        return bool_lit(empty).to_string();
    }
    let parts: Vec<String> = ps.iter().map(|p| lower_where(p, params)).collect();
    format!("({})", parts.join(&format!(" {sep} ")))
}

/// Lower `ORDER BY` with the forced stable total order (§7): the user's terms,
/// then every key column not already named, appended in the direction of the
/// last user term (ASC when the client gave no order). Single-direction by
/// construction, so rows can never silently skip or duplicate across pages.
pub fn lower_order(terms: &[(String, Dir)], key: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    for (col, dir) in terms {
        out.push(format!("{} {}", quote(col), dir.sql()));
    }
    let tiebreak = terms.last().map(|(_, d)| *d).unwrap_or(Dir::Asc);
    for k in key {
        if !terms.iter().any(|(c, _)| c == k) {
            out.push(format!("{} {}", quote(k), tiebreak.sql()));
        }
    }
    out.join(", ")
}

/// Coerce a JSON operand to a SQL value under a field's *value* type (refs
/// chased to the target-key scalar). A malformed uuid/timestamp or wrong JSON
/// shape is `type_mismatch` (§9, §10) — fail-loud, not a silently-empty match.
/// When `check_set_membership`, an off-set value on a closed-set column is
/// `type_mismatch` too (a typo'd enum value fails loudly instead of matching
/// nothing).
pub fn coerce_operand(
    contract: &Contract,
    table: &Table,
    field: &Field,
    value: &Json,
    check_set_membership: bool,
) -> Result<SqlValue, ApiError> {
    let vt = contract.value_type(&field.ty);
    let sql = json_to_sql_scalar(vt, value)
        .map_err(|expected| ApiError::type_mismatch(&table.name, &field.name, expected))?;
    if check_set_membership {
        if let (Type::Set { values }, SqlValue::Text(s)) = (vt, &sql) {
            if !values.iter().any(|m| m == s) {
                return Err(ApiError::type_mismatch(
                    &table.name,
                    &field.name,
                    &format!("one of: {}", values.join(", ")),
                ));
            }
        }
    }
    Ok(sql)
}

/// Clamp a requested limit to `[0, MAX_LIMIT]`; a negative value is a caller
/// error. Absence (`None`) is the default page — the frontends supply it.
pub fn clamp_limit(n: i64) -> Result<i64, ApiError> {
    if n < 0 {
        return Err(ApiError::bad_request("`limit` must be non-negative"));
    }
    Ok(n.min(MAX_LIMIT))
}

/// Validate an offset: non-negative and within the depth ceiling (§7, §14.1).
pub fn check_offset(n: i64) -> Result<i64, ApiError> {
    if n < 0 {
        return Err(ApiError::bad_request("`offset` must be non-negative"));
    }
    if n > MAX_OFFSET {
        return Err(ApiError::bad_request(format!(
            "`offset` exceeds the {MAX_OFFSET}-row depth ceiling; deep paging \
             is the job of a keyset cursor, not a deep offset (RFD 0021 §7)"
        )));
    }
    Ok(n)
}

/// Refuse a predicate whose total bound-parameter count would overrun SQLite's
/// variable limit (§8.7) — tree-global, not per-`IN`-list.
pub fn check_params(params: &[SqlValue]) -> Result<(), ApiError> {
    if params.len() > MAX_FILTER_PARAMS {
        return Err(ApiError::bad_request(format!(
            "filter binds {} values, over the {MAX_FILTER_PARAMS} limit",
            params.len()
        )));
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────
// The PostgREST frontend (RFD 0021 §6): `?col=op.val`, `and=(…)`/`or=(…)`,
// `not.` prefixes, `is.{null,true,false,unknown}`, `order`/`limit`/`offset`.
// Parses into the same `Predicate` the GraphQL frontend builds.
// ─────────────────────────────────────────────────────────────────────────

/// The reserved REST control keys — the exact set the parser treats specially.
/// A table with a column in this set fails startup (`http::router`), so at
/// request time a key here is unambiguously a control key, never a column
/// (§6).
pub const REST_RESERVED_KEYS: &[&str] = &["order", "limit", "offset", "select", "and", "or", "not"];

/// A parsed REST list query: the predicate, ordering, and paging.
pub struct RestQuery {
    pub predicate: Predicate,
    pub order: Vec<(String, Dir)>,
    pub limit: i64,
    pub offset: i64,
}

/// Parse an ordered list of query params into a [`RestQuery`]. Repeated keys
/// (multiple column filters, multiple `order`) are honored in order — hence
/// the ordered `&[(String, String)]`, not a map. Every top-level filter is
/// implicitly AND-ed (PostgREST convention).
pub fn parse_rest(
    contract: &Contract,
    table: &Table,
    params: &[(String, String)],
) -> Result<RestQuery, ApiError> {
    let mut conds: Vec<Predicate> = Vec::new();
    let mut order: Vec<(String, Dir)> = Vec::new();
    let mut limit = DEFAULT_LIMIT;
    let mut offset = 0i64;

    for (key, val) in params {
        match key.as_str() {
            "limit" => {
                let n = val
                    .parse::<i64>()
                    .map_err(|_| ApiError::bad_request("`limit` must be a non-negative integer"))?;
                limit = clamp_limit(n)?;
            }
            "offset" => {
                let n = val.parse::<i64>().map_err(|_| {
                    ApiError::bad_request("`offset` must be a non-negative integer")
                })?;
                offset = check_offset(n)?;
            }
            "order" => order.extend(parse_order(table, val)?),
            "select" => {
                return Err(ApiError::bad_request(
                    "column projection (`select`) is not supported in v0",
                ));
            }
            "and" => conds.push(Predicate::And(parse_group(contract, table, val)?)),
            "or" => conds.push(Predicate::Or(parse_group(contract, table, val)?)),
            "not.and" => conds.push(Predicate::Not(Box::new(Predicate::And(parse_group(
                contract, table, val,
            )?)))),
            "not.or" => conds.push(Predicate::Not(Box::new(Predicate::Or(parse_group(
                contract, table, val,
            )?)))),
            // a plain column filter: `?col=op.val`
            col => conds.push(parse_column_op(contract, table, col, val)?),
        }
    }

    let predicate = match conds.len() {
        0 => Predicate::Const(true),
        1 => conds.pop().expect("len checked"),
        _ => Predicate::And(conds),
    };
    Ok(RestQuery {
        predicate,
        order,
        limit,
        offset,
    })
}

/// `?order=col.asc,col2.desc` — a comma-separated list of `col[.dir]`. A
/// third `.nullsfirst/.nullslast` segment is refused (the nulls-order variants
/// are deferred, §7) rather than silently ignored.
fn parse_order(table: &Table, raw: &str) -> Result<Vec<(String, Dir)>, ApiError> {
    let mut out = Vec::new();
    for term in raw.split(',').filter(|s| !s.is_empty()) {
        let mut parts = term.split('.');
        let col = parts.next().unwrap_or("");
        let dir = match parts.next() {
            None | Some("asc") => Dir::Asc,
            Some("desc") => Dir::Desc,
            Some(other) => {
                return Err(ApiError::bad_request(format!(
                    "unsupported order direction `{other}`; v0 offers `asc` and `desc` \
                     (explicit NULLS ordering is deferred, RFD 0021 §7)"
                )));
            }
        };
        if parts.next().is_some() {
            return Err(ApiError::bad_request(
                "explicit NULLS ordering (`.nullsfirst`/`.nullslast`) is deferred in v0",
            ));
        }
        resolve_field(table, col)?;
        out.push((col.to_string(), dir));
    }
    Ok(out)
}

/// Resolve a column name against the table's declared fields, or `unknown_field`.
fn resolve_field<'a>(table: &'a Table, col: &str) -> Result<&'a Field, ApiError> {
    table
        .field(col)
        .ok_or_else(|| ApiError::unknown_field(&table.name, col))
}

/// Split the content of a logical group `(a,b,c)` at top-level commas —
/// respecting nested parentheses and double-quoted spans.
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_quote = false;
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.push(c);
            }
            '\\' if in_quote => {
                cur.push(c);
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            '(' if !in_quote => {
                depth += 1;
                cur.push(c);
            }
            ')' if !in_quote => {
                depth -= 1;
                cur.push(c);
            }
            ',' if !in_quote && depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parse a logical group body: `(m1,m2,…)`. Each member is either a nested
/// group (`and(…)`, `or(…)`, `not.and(…)`, `not.or(…)`) or a column condition
/// `col.op.val`.
fn parse_group(contract: &Contract, table: &Table, raw: &str) -> Result<Vec<Predicate>, ApiError> {
    let inner = raw
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| {
            ApiError::bad_request(format!("logical group must be parenthesised: `{raw}`"))
        })?;
    let mut out = Vec::new();
    for member in split_top_level(inner) {
        out.push(parse_group_member(contract, table, &member)?);
    }
    Ok(out)
}

/// One member inside a logical group. Distinguishes a nested group from a
/// column condition by the `and(`/`or(`/`not.and(`/`not.or(` prefix.
fn parse_group_member(
    contract: &Contract,
    table: &Table,
    member: &str,
) -> Result<Predicate, ApiError> {
    for (prefix, negated, is_or) in [
        ("not.and(", true, false),
        ("not.or(", true, true),
        ("and(", false, false),
        ("or(", false, true),
    ] {
        if let Some(rest) = member.strip_prefix(prefix) {
            let body = format!("({rest}"); // restore the paren the prefix ate
            let members = parse_group(contract, table, &body)?;
            let group = if is_or {
                Predicate::Or(members)
            } else {
                Predicate::And(members)
            };
            return Ok(if negated {
                Predicate::Not(Box::new(group))
            } else {
                group
            });
        }
    }
    // a column condition: `col.<opexpr>` (dot-joined, unlike the top-level
    // `?col=<opexpr>` where the column is the query key)
    let (col, opexpr) = member
        .split_once('.')
        .ok_or_else(|| ApiError::bad_request(format!("malformed filter `{member}`")))?;
    parse_column_op(contract, table, col, opexpr)
}

/// Parse one column operator expression: `[not.]op.value`, applied to `col`.
fn parse_column_op(
    contract: &Contract,
    table: &Table,
    col: &str,
    opexpr: &str,
) -> Result<Predicate, ApiError> {
    // a leading `not.` negates the whole condition (covers `not.is.null`, the
    // faithful PostgREST spelling for IS NOT NULL — there is no `is.not_null`)
    if let Some(rest) = opexpr.strip_prefix("not.") {
        return Ok(Predicate::Not(Box::new(parse_column_op(
            contract, table, col, rest,
        )?)));
    }
    let field = resolve_field(table, col)?;
    let (op, value) = opexpr
        .split_once('.')
        .ok_or_else(|| ApiError::bad_request(format!("malformed operator `{opexpr}`")))?;

    let cmp = |op: CmpOp, membership: bool| -> Result<Predicate, ApiError> {
        let json = text_to_json_scalar(contract.value_type(&field.ty), value.to_string());
        Ok(Predicate::Cmp {
            col: col.to_string(),
            op,
            value: coerce_operand(contract, table, field, &json, membership)?,
        })
    };

    match op {
        "eq" => cmp(CmpOp::Eq, true),
        "neq" => cmp(CmpOp::Neq, true),
        "gt" => cmp(CmpOp::Gt, false),
        "gte" => cmp(CmpOp::Gte, false),
        "lt" => cmp(CmpOp::Lt, false),
        "lte" => cmp(CmpOp::Lte, false),
        "ilike" => {
            // PostgREST aliases `*` → `%` to dodge URL-encoding (§6 deviation)
            let pattern = value.replace('*', "%");
            Ok(Predicate::Cmp {
                col: col.to_string(),
                op: CmpOp::Ilike,
                value: SqlValue::Text(pattern),
            })
        }
        "like" => Err(ApiError::bad_request(format!(
            "`like` (case-sensitive) is not supported on the SQLite floor; use `ilike` \
             for case-insensitive matching (`{col}=ilike.{value}`)"
        ))),
        "in" | "nin" => {
            let items = parse_in_list(value)?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let json = text_to_json_scalar(contract.value_type(&field.ty), item);
                values.push(coerce_operand(contract, table, field, &json, true)?);
            }
            Ok(Predicate::In {
                col: col.to_string(),
                negated: op == "nin",
                values,
            })
        }
        "is" => match value {
            "null" => Ok(Predicate::IsNull {
                col: col.to_string(),
                negated: false,
            }),
            "unknown" => Ok(Predicate::IsNull {
                col: col.to_string(),
                negated: false,
            }),
            "true" | "false" => {
                if !matches!(contract.value_type(&field.ty), Type::Bool) {
                    return Err(ApiError::type_mismatch(&table.name, col, "a boolean column"));
                }
                Ok(Predicate::Cmp {
                    col: col.to_string(),
                    op: CmpOp::Eq,
                    value: SqlValue::Integer((value == "true") as i64),
                })
            }
            other => Err(ApiError::bad_request(format!(
                "`is.{other}` is not a valid identity check; use null, not_null (as `not.is.null`), true, or false"
            ))),
        },
        other => Err(ApiError::bad_request(format!(
            "unsupported operator `{other}` on `{}.{col}`",
            table.name
        ))),
    }
}

/// Parse an `in.(…)` value list: `(a,b,"c,d")`, respecting quotes.
fn parse_in_list(raw: &str) -> Result<Vec<String>, ApiError> {
    let inner = raw
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| {
            ApiError::bad_request(format!("`in` list must be parenthesised: `{raw}`"))
        })?;
    Ok(split_top_level(inner)
        .into_iter()
        .map(|item| unquote(&item))
        .collect())
}

/// Strip a surrounding pair of double quotes and unescape `\"`/`\\` inside —
/// PostgREST's quoting for values containing reserved characters.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let inner = &s[1..s.len() - 1];
        inner.replace("\\\"", "\"").replace("\\\\", "\\")
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower(pred: &Predicate) -> (String, Vec<SqlValue>) {
        let mut params = Vec::new();
        let sql = lower_where(pred, &mut params);
        (sql, params)
    }

    #[test]
    fn cmp_lowers_to_bound_param() {
        let (sql, params) = lower(&Predicate::Cmp {
            col: "age".into(),
            op: CmpOp::Gte,
            value: SqlValue::Integer(18),
        });
        assert_eq!(sql, "\"age\" >= ?");
        assert_eq!(params, vec![SqlValue::Integer(18)]);
    }

    #[test]
    fn ilike_carries_escape() {
        let (sql, _) = lower(&Predicate::Cmp {
            col: "name".into(),
            op: CmpOp::Ilike,
            value: SqlValue::Text("%a%".into()),
        });
        assert_eq!(sql, "\"name\" LIKE ? ESCAPE '\\'");
    }

    #[test]
    fn empty_in_is_false_empty_nin_is_true() {
        let (sql, params) = lower(&Predicate::In {
            col: "id".into(),
            negated: false,
            values: vec![],
        });
        assert_eq!(sql, "0");
        assert!(params.is_empty());
        let (sql, _) = lower(&Predicate::In {
            col: "id".into(),
            negated: true,
            values: vec![],
        });
        assert_eq!(sql, "1");
    }

    #[test]
    fn in_binds_each_value() {
        let (sql, params) = lower(&Predicate::In {
            col: "k".into(),
            negated: false,
            values: vec![
                SqlValue::Integer(1),
                SqlValue::Integer(2),
                SqlValue::Integer(3),
            ],
        });
        assert_eq!(sql, "\"k\" IN (?, ?, ?)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn is_null_has_no_param() {
        let (sql, params) = lower(&Predicate::IsNull {
            col: "deleted_at".into(),
            negated: true,
        });
        assert_eq!(sql, "\"deleted_at\" IS NOT NULL");
        assert!(params.is_empty());
    }

    #[test]
    fn and_or_not_nest_and_bind_in_order() {
        let (sql, params) = lower(&Predicate::And(vec![
            Predicate::Cmp {
                col: "a".into(),
                op: CmpOp::Eq,
                value: SqlValue::Integer(1),
            },
            Predicate::Or(vec![
                Predicate::Cmp {
                    col: "b".into(),
                    op: CmpOp::Lt,
                    value: SqlValue::Integer(2),
                },
                Predicate::Not(Box::new(Predicate::IsNull {
                    col: "c".into(),
                    negated: false,
                })),
            ]),
        ]));
        assert_eq!(sql, "(\"a\" = ? AND (\"b\" < ? OR NOT (\"c\" IS NULL)))");
        assert_eq!(params, vec![SqlValue::Integer(1), SqlValue::Integer(2)]);
    }

    #[test]
    fn empty_combinators_canonicalize() {
        assert_eq!(lower(&Predicate::And(vec![])).0, "1");
        assert_eq!(lower(&Predicate::Or(vec![])).0, "0");
        assert_eq!(lower(&Predicate::Const(true)).0, "1");
    }

    #[test]
    fn identifier_quotes_are_doubled() {
        // a filter identifier can never break out into a literal
        let (sql, _) = lower(&Predicate::IsNull {
            col: "we\"ird".into(),
            negated: false,
        });
        assert_eq!(sql, "\"we\"\"ird\" IS NULL");
    }

    #[test]
    fn order_forces_pk_tiebreak_in_last_direction() {
        // user sorts desc; the pk tiebreak follows the last term's direction
        let sql = lower_order(&[("created".into(), Dir::Desc)], &["id".into()]);
        assert_eq!(sql, "\"created\" DESC NULLS FIRST, \"id\" DESC NULLS FIRST");
        // no user order → key ascending
        let sql = lower_order(&[], &["id".into()]);
        assert_eq!(sql, "\"id\" ASC NULLS LAST");
        // a user term already on the key is not duplicated
        let sql = lower_order(&[("id".into(), Dir::Asc)], &["id".into()]);
        assert_eq!(sql, "\"id\" ASC NULLS LAST");
    }

    #[test]
    fn top_level_split_respects_parens_and_quotes() {
        assert_eq!(split_top_level("a.eq.1,b.gt.2"), vec!["a.eq.1", "b.gt.2"]);
        assert_eq!(
            split_top_level("a.eq.1,or(b.eq.2,c.eq.3)"),
            vec!["a.eq.1", "or(b.eq.2,c.eq.3)"]
        );
        assert_eq!(
            split_top_level("name.eq.\"a,b\",x.gt.1"),
            vec!["name.eq.\"a,b\"", "x.gt.1"]
        );
    }

    #[test]
    fn unquote_strips_and_unescapes() {
        assert_eq!(unquote("plain"), "plain");
        assert_eq!(unquote("\"a,b\""), "a,b");
        assert_eq!(unquote("\"a\\\"b\""), "a\"b");
    }
}

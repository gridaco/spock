//! SQLite DDL emission (docs/spec/v0.md §7.1). One `CREATE TABLE` per
//! contract table, declaration order preserved, all identifiers quoted.
//! Declared defaults are emitted as DEFAULT clauses — `auto`/`now` call
//! the engine builtins (`spock_uuid()`/`spock_now()`), so escape-body
//! INSERTs may omit defaulted columns and get the same values the write
//! path would mint.

use std::collections::HashMap;

use crate::ir::{Contract, DefaultValue, Type};

/// Emit `CREATE TABLE` statements for every table, in declaration order.
pub fn ddl(contract: &Contract) -> Vec<String> {
    contract
        .tables
        .iter()
        .map(|table| {
            let mut lines: Vec<String> = Vec::new();

            for field in &table.fields {
                let storage = contract.storage_type(&field.ty).sql();
                let null = if field.optional { "" } else { " NOT NULL" };
                // `= me` (RFD 0014) has no DDL DEFAULT — `default_sql` returns
                // None: spock_actor() is DIRECTONLY, so the runtime stamps the
                // actor on the write path and an escape names spock_actor().
                let default = field
                    .default
                    .as_ref()
                    .and_then(default_sql)
                    .map(|expr| format!(" DEFAULT {expr}"))
                    .unwrap_or_default();
                lines.push(format!("  \"{}\" {storage}{null}{default}", field.name));
            }

            let key_list = quote_list(&table.key);
            lines.push(format!("  PRIMARY KEY ({key_list})"));

            for field in &table.fields {
                if field.unique {
                    lines.push(format!("  UNIQUE (\"{}\")", field.name));
                }
            }
            for group in &table.uniques {
                lines.push(format!("  UNIQUE ({})", quote_list(group)));
            }

            // Value constraints → named CHECKs whose names are the derived
            // `<table>_<fields>_invalid` codes (RFD 0013): the name is the
            // whole runtime routing channel. Emitted after the UNIQUE block,
            // before the foreign keys; field order, then row checks.
            //
            // Closed-set membership: members escaped by doubling `'`.
            for field in &table.fields {
                if let Type::Set { values } = &field.ty {
                    let list = values
                        .iter()
                        .map(|v| format!("'{}'", v.replace('\'', "''")))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!(
                        "  CONSTRAINT \"{}_{}_invalid\" CHECK (\"{}\" IN ({list}))",
                        table.name, field.name, field.name
                    ));
                }
            }
            // Field validators: inline-expand the fn body against the column.
            for field in &table.fields {
                if let Some(fn_name) = &field.check {
                    if let Some(clause) = inline_validator(contract, fn_name, &[field.name.as_str()])
                    {
                        lines.push(format!(
                            "  CONSTRAINT \"{}_{}_invalid\" CHECK ({clause})",
                            table.name, field.name
                        ));
                    }
                }
            }
            // Row validators: inline-expand against the named columns.
            for check in &table.checks {
                let cols: Vec<&str> = check.fields.iter().map(String::as_str).collect();
                if let Some(clause) = inline_validator(contract, &check.fn_name, &cols) {
                    lines.push(format!(
                        "  CONSTRAINT \"{}_{}_invalid\" CHECK ({clause})",
                        table.name,
                        check.fields.join("_")
                    ));
                }
            }

            for field in &table.fields {
                if let Type::Ref { table: target, on_delete } = &field.ty {
                    let target_key = &contract
                        .table(target)
                        .expect("checked: ref target exists")
                        .key[0];
                    let action = match on_delete {
                        crate::ir::OnDelete::Restrict => "RESTRICT",
                        crate::ir::OnDelete::Cascade => "CASCADE",
                        crate::ir::OnDelete::SetNull => "SET NULL",
                    };
                    lines.push(format!(
                        "  FOREIGN KEY (\"{}\") REFERENCES \"{target}\" (\"{target_key}\") ON DELETE {action}",
                        field.name
                    ));
                }
            }

            format!(
                "CREATE TABLE \"{}\" (\n{}\n);",
                table.name,
                lines.join(",\n")
            )
        })
        .collect()
}

/// A default value as a SQLite DEFAULT expression, or `None` for `= me`
/// (RFD 0014): `spock_actor()` is DIRECTONLY, so `me` has no DDL DEFAULT —
/// the runtime materializes it on the write path instead.
fn default_sql(default: &DefaultValue) -> Option<String> {
    Some(match default {
        DefaultValue::Auto => "(spock_uuid())".into(),
        DefaultValue::Now => "(spock_now())".into(),
        DefaultValue::Actor => return None,
        DefaultValue::Str { value } => format!("'{}'", value.replace('\'', "''")),
        DefaultValue::Int { value } => value.to_string(),
        // {:?} keeps the decimal point (`2.0`, not `2`) — literals verbatim
        DefaultValue::Float { value } => format!("{value:?}"),
        DefaultValue::Bool { value } => if *value { "1" } else { "0" }.into(),
    })
}

fn quote_list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| format!("\"{n}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Inline-expand a validator fn body into a CHECK expression (RFD 0013):
/// strip the leading `SELECT`, then substitute each `:param` for its
/// positionally-bound column. The checker guarantees the fn is a single
/// `SELECT <boolean expression>` with one param per column, so this is a
/// pure textual rewrite. `None` only on contract drift (missing fn).
fn inline_validator(contract: &Contract, fn_name: &str, columns: &[&str]) -> Option<String> {
    let f = contract.fn_def(fn_name)?;
    let body = f.sql.first()?;
    let map: HashMap<&str, String> = f
        .params
        .iter()
        .map(|p| p.name.as_str())
        .zip(columns.iter().map(|c| format!("\"{c}\"")))
        .collect();
    let clause = substitute_params(strip_select(body), &map);
    Some(trim_check_expr(clause.trim()))
}

/// Drop trailing statement terminators (`;`) and trailing comments from an
/// inlined CHECK expression, so it splices cleanly inside `CHECK (…)` (a
/// trailing `;` is a syntax error there, and a trailing `-- comment` would
/// eat the closing paren). String- and comment-aware: a `;`, `--`, or `/*`
/// inside a string literal is preserved. Returns the slice up to the last
/// meaningful character.
fn trim_check_expr(expr: &str) -> String {
    let bytes = expr.as_bytes();
    let mut i = 0;
    let mut last_meaningful = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\'' {
                        if bytes.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                last_meaningful = i;
            }
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
            }
            b';' => i += 1,
            c if c.is_ascii_whitespace() => i += 1,
            _ => {
                i += 1;
                last_meaningful = i;
            }
        }
    }
    expr[..last_meaningful].to_string()
}

/// The `SELECT <expr>` that evaluates a field validator with its parameter
/// bound to the field's *literal default* — the load's default-vs-check
/// proof (RFD 0013 L-G). `None` when the field has no check, no default, or
/// an engine-minted (`auto`/`now`) default (a `check` on which is E042).
pub fn field_check_default_probe(
    contract: &Contract,
    table_name: &str,
    field_name: &str,
) -> Option<String> {
    let field = contract.table(table_name)?.field(field_name)?;
    let fn_name = field.check.as_ref()?;
    let d = field.default.as_ref()?;
    // engine-minted defaults have no literal to probe against a check
    if d.is_engine_minted() {
        return None;
    }
    let value = default_sql(d)?;
    let f = contract.fn_def(fn_name)?;
    let param = f.params.first()?;
    let map: HashMap<&str, String> = std::iter::once((param.name.as_str(), value)).collect();
    let expr = substitute_params(strip_select(f.sql.first()?), &map)
        .trim()
        .to_string();
    Some(format!("SELECT {expr}"))
}

/// Everything after the leading `SELECT` (skipping leading whitespace and
/// comments). Falls back to the whole string if no `SELECT` leads.
fn strip_select(sql: &str) -> &str {
    let bytes = sql.as_bytes();
    let mut i = 0;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes[i..].starts_with(b"--") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        break;
    }
    // compare on bytes, not a &str slice — the first non-ws/comment byte may
    // begin a multibyte char that straddles offset i+6 (a &str slice there
    // panics; a &[u8] slice does not)
    if i + 6 <= bytes.len() && bytes[i..i + 6].eq_ignore_ascii_case(b"SELECT") {
        &sql[i + 6..]
    } else {
        &sql[i..]
    }
}

/// Substitute `:param` tokens for quoted column names, token-aware
/// (longest-match identifier run) and never inside a single-quoted string
/// literal (RFD 0013 L-Q). Slices are copied verbatim, so UTF-8 members
/// survive.
fn substitute_params(expr: &str, map: &HashMap<&str, String>) -> String {
    let bytes = expr.as_bytes();
    let mut out = String::with_capacity(expr.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\'' {
                        if bytes.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                out.push_str(&expr[start..i]);
            }
            b':' => {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                match map.get(&expr[start..j]) {
                    Some(replacement) => out.push_str(replacement),
                    None => out.push_str(&expr[i..j]),
                }
                i = j;
            }
            _ => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'\'' && bytes[i] != b':' {
                    i += 1;
                }
                out.push_str(&expr[start..i]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    #[test]
    fn emits_expected_schema() {
        let contract = compile(
            "table user {\n\
               key id: uuid = auto\n\
               username: text unique\n\
               bio: text?\n\
               joined_at: timestamp = now\n\
             }\n\
             table post {\n\
               key id: uuid = auto\n\
               author: user\n\
               caption: text?\n\
               pinned: bool = false\n\
             }\n\
             table follow {\n\
               key (follower, target)\n\
               follower: user\n\
               target: user on delete cascade\n\
             }",
        )
        .unwrap();

        let statements = ddl(&contract);
        assert_eq!(statements.len(), 3);

        assert_eq!(
            statements[0],
            "CREATE TABLE \"user\" (\n\
             \x20 \"id\" TEXT NOT NULL DEFAULT (spock_uuid()),\n\
             \x20 \"username\" TEXT NOT NULL,\n\
             \x20 \"bio\" TEXT,\n\
             \x20 \"joined_at\" TEXT NOT NULL DEFAULT (spock_now()),\n\
             \x20 PRIMARY KEY (\"id\"),\n\
             \x20 UNIQUE (\"username\")\n\
             );"
        );

        // bool → INTEGER; the ref column takes the target key's storage type
        assert!(statements[1].contains("\"pinned\" INTEGER NOT NULL DEFAULT 0"));
        assert!(statements[1].contains("\"author\" TEXT NOT NULL"));
        assert!(statements[1]
            .contains("FOREIGN KEY (\"author\") REFERENCES \"user\" (\"id\") ON DELETE RESTRICT"));

        assert!(statements[2].contains("PRIMARY KEY (\"follower\", \"target\")"));
        assert!(statements[2]
            .contains("FOREIGN KEY (\"target\") REFERENCES \"user\" (\"id\") ON DELETE CASCADE"));
    }

    #[test]
    fn emits_literal_defaults() {
        let contract = compile(
            "table t {\n\
               key id: uuid = auto\n\
               status: text = \"it's fine\"\n\
               tries: int = 3\n\
               weight: float = 0.5\n\
             }",
        )
        .unwrap();
        let sql = &ddl(&contract)[0];
        // single quotes doubled — the classic injection foothold
        assert!(sql.contains("\"status\" TEXT NOT NULL DEFAULT 'it''s fine'"));
        assert!(sql.contains("\"tries\" INTEGER NOT NULL DEFAULT 3"));
        assert!(sql.contains("\"weight\" REAL NOT NULL DEFAULT 0.5"));
    }

    #[test]
    fn emits_set_check_constraint() {
        // members with an apostrophe must be single-quote-doubled (the
        // DEFAULT-clause law), or the generated DDL fails to prepare.
        let contract = compile(
            "table media {\n\
               key id: uuid = auto\n\
               kind: \"image\" | \"it's video\"\n\
             }",
        )
        .unwrap();
        let sql = &ddl(&contract)[0];
        // TEXT storage, and a named CHECK whose name is the derived code
        assert!(sql.contains("\"kind\" TEXT NOT NULL"));
        assert!(
            sql.contains(
                "CONSTRAINT \"media_kind_invalid\" CHECK (\"kind\" IN ('image', 'it''s video'))"
            ),
            "{sql}"
        );
    }

    #[test]
    fn emits_validator_check_by_inline_expansion() {
        let contract = compile(
            "fn valid_username(name: text) -> bool { unchecked sql(\"SELECT :name NOT GLOB '*[^a-z0-9._]*' AND length(:name) BETWEEN 1 AND 30\") }\n\
             fn distinct_pair(a: uuid, b: uuid) -> bool { unchecked sql(\"SELECT :a <> :b\") }\n\
             table user { key id: uuid = auto\n username: text check valid_username }\n\
             table follow { key (follower, target)\n follower: user\n target: user\n\
               check (follower, target) distinct_pair }",
        )
        .unwrap();
        let statements = ddl(&contract);
        // the field validator inlines its body with :name → \"username\"
        let user = statements.iter().find(|s| s.contains("\"user\"")).unwrap();
        assert!(
            user.contains(
                "CONSTRAINT \"user_username_invalid\" CHECK (\"username\" NOT GLOB '*[^a-z0-9._]*' AND length(\"username\") BETWEEN 1 AND 30)"
            ),
            "{user}"
        );
        // the row validator inlines both params positionally
        let follow = statements
            .iter()
            .find(|s| s.contains("\"follow\""))
            .unwrap();
        assert!(
            follow.contains(
                "CONSTRAINT \"follow_follower_target_invalid\" CHECK (\"follower\" <> \"target\")"
            ),
            "{follow}"
        );
    }

    #[test]
    fn inline_expansion_trims_trailing_terminator_and_comment() {
        // a trailing `;` or `-- comment` in a validator body must not corrupt
        // the spliced CHECK (the `;` is a syntax error there, the comment
        // would eat the closing paren)
        for body in [
            "SELECT :n > 0;",
            "SELECT :n > 0 -- ok",
            "SELECT :n > 0 ; -- ok",
        ] {
            let contract = compile(&format!(
                "fn v(n: int) -> bool {{ unchecked sql(\"{body}\") }}\n\
                 table t {{ key id: uuid = auto\n n: int check v }}"
            ))
            .unwrap();
            let sql = &ddl(&contract)[0];
            assert!(
                sql.contains("CONSTRAINT \"t_n_invalid\" CHECK (\"n\" > 0)"),
                "body `{body}` produced: {sql}"
            );
        }
    }

    #[test]
    fn strip_select_handles_a_multibyte_leading_byte() {
        // a non-SELECT-leading body whose first run straddles byte offset 6
        // with a multibyte char must not panic strip_select — it falls
        // through to the fallback expression, verbatim (a raw triple-quoted
        // string so the `€` reaches the body unescaped)
        let contract = compile(
            "fn v(s: text) -> bool { unchecked sql(\"\"\"ABCDE\u{20ac}\"\"\") }\n\
             table t { key id: uuid = auto\n s: text check v }",
        )
        .unwrap();
        // does not panic; the (invalid) body is emitted for the engine to reject
        let sql = &ddl(&contract)[0];
        assert!(sql.contains("CHECK (ABCDE\u{20ac})"), "{sql}");
    }

    #[test]
    fn emits_real_for_float() {
        let contract = compile("table tag { key id: uuid = auto\n x: float\n y: float? }").unwrap();
        let statements = ddl(&contract);
        assert!(statements[0].contains("\"x\" REAL NOT NULL"));
        assert!(statements[0].contains("\"y\" REAL,"));
    }

    #[test]
    fn emits_set_null() {
        let contract = compile(
            "table user { key id: uuid = auto }\n\
             table comment {\n\
               key id: uuid = auto\n\
               author: user\n\
               parent: comment? on delete set null\n\
             }",
        )
        .unwrap();
        let statements = ddl(&contract);
        assert!(statements[1].contains(
            "FOREIGN KEY (\"parent\") REFERENCES \"comment\" (\"id\") ON DELETE SET NULL"
        ));
    }
}

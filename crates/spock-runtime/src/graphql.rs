//! GraphQL reads at `/graphql/v1` (docs/spec/v0.md §8.2).
//!
//! The schema is built dynamically at startup from the loaded contract —
//! types are unknown at compile time, so this is `async_graphql::dynamic`
//! end to end. Read-only by design: the query root is derived from tables;
//! mutations arrive with `fn`. Naming laws (normative, §8.2):
//!
//! - object type = PascalCase of the table name (`post_media` → `PostMedia`);
//!   collisions or reserved names fail startup, not requests;
//! - field names are the contract's, verbatim;
//! - `Query.<table>_list(limit)` for every table; `Query.<table>(<key>)`
//!   for single-key tables (missing row → `null`);
//! - forward reference fields resolve to the referenced object;
//! - reverse collections are `<child>_<field>_list` on the referenced type.

use std::collections::HashMap;
use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, ResolverContext, Scalar, Schema,
    SchemaError, TypeRef,
};
use async_graphql::Value as GqlValue;
use rusqlite::types::Value as SqlValue;
use serde_json::Value as Json;
use spock_lang::ir::{Contract, Table, Type};
use time::format_description::well_known::Rfc3339;

use crate::value::json_to_sql;
use crate::write;
use crate::App;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

/// Type names the derivation may never produce (builtins + our scalars).
const RESERVED_TYPE_NAMES: &[&str] = &[
    "Query",
    "String",
    "Int",
    "Float",
    "Boolean",
    "ID",
    "UUID",
    "Timestamp",
];

/// Schema derivation failures. All are startup-fatal: a program the checker
/// accepted can still fail the GraphQL naming laws (§8.2), and that must
/// surface at load, not per-request.
#[derive(Debug, thiserror::Error)]
pub enum SchemaBuildError {
    #[error("tables `{a}` and `{b}` both map to GraphQL type `{name}`")]
    DuplicateTypeName { a: String, b: String, name: String },
    #[error("table `{table}` maps to reserved GraphQL type name `{name}`")]
    ReservedTypeName { table: String, name: String },
    #[error("query fields for tables `{a}` and `{b}` collide on `{name}`")]
    DuplicateQueryField { a: String, b: String, name: String },
    #[error("on type `{type_name}`: generated field `{name}` collides with `{other}`")]
    DuplicateObjectField {
        type_name: String,
        name: String,
        other: String,
    },
    #[error(transparent)]
    Schema(#[from] SchemaError),
}

/// Build the GraphQL schema for a loaded contract. The `App` travels as
/// schema data; resolvers reach the database through it.
pub fn schema(app: Arc<App>) -> Result<Schema, SchemaBuildError> {
    let contract = &app.contract;

    // Pass 1a — object type names + collision guard.
    let mut type_names: HashMap<String, String> = HashMap::new(); // table -> Type
    let mut claimed_types: HashMap<String, String> = HashMap::new(); // Type -> table
    for table in &contract.tables {
        let name = pascal_type_name(&table.name);
        if RESERVED_TYPE_NAMES.contains(&name.as_str()) {
            return Err(SchemaBuildError::ReservedTypeName {
                table: table.name.clone(),
                name,
            });
        }
        if let Some(other) = claimed_types.insert(name.clone(), table.name.clone()) {
            return Err(SchemaBuildError::DuplicateTypeName {
                a: other,
                b: table.name.clone(),
                name,
            });
        }
        type_names.insert(table.name.clone(), name);
    }

    // Pass 1b — query-root field names.
    let mut claimed_roots: HashMap<String, String> = HashMap::new(); // field -> table
    for table in &contract.tables {
        let mut claim = |name: String| -> Result<(), SchemaBuildError> {
            if let Some(other) = claimed_roots.insert(name.clone(), table.name.clone()) {
                return Err(SchemaBuildError::DuplicateQueryField {
                    a: other,
                    b: table.name.clone(),
                    name,
                });
            }
            Ok(())
        };
        claim(format!("{}_list", table.name))?;
        if table.key.len() == 1 {
            claim(table.name.clone())?;
        }
    }

    // Pass 2 — object types (declared fields, forward refs, reverse
    // collections), with per-type field-name collision guard.
    let mut objects: Vec<Object> = Vec::new();
    for table in &contract.tables {
        let type_name = &type_names[&table.name];
        let mut obj = Object::new(type_name.clone());
        let mut claimed_fields: HashMap<String, String> = HashMap::new();

        for field in &table.fields {
            claimed_fields.insert(field.name.clone(), "a declared field".into());
            obj = obj.field(match &field.ty {
                Type::Ref { table: target, .. } => forward_ref_field(
                    table.name.clone(),
                    field.name.clone(),
                    type_names[target].clone(),
                    field.optional,
                ),
                _ => scalar_field(
                    field.name.clone(),
                    scalar_type_ref(contract, &field.ty, field.optional),
                ),
            });
        }

        for (child, ref_field) in contract.inbound_refs(&table.name) {
            let name = format!("{}_{}_list", child.name, ref_field.name);
            if let Some(other) = claimed_fields.insert(name.clone(), "a reverse collection".into())
            {
                return Err(SchemaBuildError::DuplicateObjectField {
                    type_name: type_name.clone(),
                    name,
                    other,
                });
            }
            obj = obj.field(reverse_list_field(
                name,
                table.name.clone(),
                child.name.clone(),
                ref_field.name.clone(),
                type_names[&child.name].clone(),
            ));
        }

        objects.push(obj);
    }

    // Pass 3 — the Query root.
    let mut query = Object::new("Query");
    for table in &contract.tables {
        let type_name = &type_names[&table.name];
        query = query.field(root_list_field(table, type_name));
        if table.key.len() == 1 {
            query = query.field(root_by_key_field(contract, table, type_name));
        }
    }

    // Pass 4 — register and finish. Introspection stays enabled (§8.2).
    let mut builder = Schema::build("Query", None, None)
        .register(Scalar::new("UUID").description("A UUID in canonical hyphenated form"))
        .register(Scalar::new("Timestamp").description("An RFC 3339 UTC timestamp"))
        .data(app.clone())
        // self-references permit unbounded nesting; stay above GraphiQL's
        // introspection depth while bounding pathological queries
        .limit_depth(32);
    for obj in objects {
        builder = builder.register(obj);
    }
    builder = builder.register(query);
    Ok(builder.finish()?)
}

/// `post_media` → `PostMedia`. Total on §2.1 identifiers; distinct inputs
/// may collide (`user_2` vs `user2`) — the caller guards.
fn pascal_type_name(snake: &str) -> String {
    snake
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// GraphQL type for a scalar (non-ref) field, nullable iff optional.
fn scalar_type_ref(contract: &Contract, ty: &Type, optional: bool) -> TypeRef {
    let name = scalar_name(contract, ty);
    if optional {
        TypeRef::named(name)
    } else {
        TypeRef::named_nn(name)
    }
}

fn scalar_name(contract: &Contract, ty: &Type) -> &'static str {
    match contract.value_type(ty) {
        Type::Text => TypeRef::STRING,
        Type::Int => TypeRef::INT,
        Type::Bool => TypeRef::BOOLEAN,
        Type::Uuid => "UUID",
        Type::Timestamp => "Timestamp",
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

fn gql<E: std::fmt::Display>(e: E) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}

fn app_of<'a>(ctx: &ResolverContext<'a>) -> Result<&'a Arc<App>, async_graphql::Error> {
    ctx.data::<Arc<App>>()
}

fn parent_row<'a>(ctx: &ResolverContext<'a>) -> Result<&'a Json, async_graphql::Error> {
    ctx.parent_value.try_downcast_ref::<Json>()
}

/// `limit` argument: default 50, ceiling 200 (shared with §8), negative is
/// an error — mirroring the REST surface.
fn read_limit(ctx: &ResolverContext<'_>) -> Result<i64, async_graphql::Error> {
    match ctx.args.get("limit") {
        None => Ok(DEFAULT_LIMIT),
        Some(v) => {
            let n = v.i64()?;
            if n < 0 {
                return Err(gql("`limit` must be non-negative"));
            }
            Ok(n.min(MAX_LIMIT))
        }
    }
}

/// Run `SELECT *` over a table (optionally filtered by one column), key
/// order, limited. One lock scope; no await while held (§7.2 discipline).
fn query_rows(
    app: &App,
    table: &Table,
    filter: Option<(&str, &SqlValue)>,
    limit: i64,
) -> Result<Vec<Json>, async_graphql::Error> {
    let order = table
        .key
        .iter()
        .map(|k| format!("\"{k}\" ASC"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = match filter {
        Some((column, _)) => format!(
            "SELECT * FROM \"{}\" WHERE \"{column}\" = ?1 ORDER BY {order} LIMIT {limit}",
            table.name
        ),
        None => format!(
            "SELECT * FROM \"{}\" ORDER BY {order} LIMIT {limit}",
            table.name
        ),
    };

    let db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
    let mut stmt = db.prepare(&sql).map_err(gql)?;
    let mut rows = match filter {
        Some((_, value)) => stmt.query([value]).map_err(gql)?,
        None => stmt.query([]).map_err(gql)?,
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().map_err(gql)? {
        out.push(write::row_to_json(&app.contract, table, row).map_err(gql)?);
    }
    Ok(out)
}

/// A scalar field: read the cell from the parent JSON row.
fn scalar_field(name: String, ty: TypeRef) -> Field {
    let field_name = name.clone();
    Field::new(name, ty, move |ctx| {
        let field_name = field_name.clone();
        FieldFuture::new(async move {
            let row = parent_row(&ctx)?;
            match row.get(&field_name) {
                None | Some(Json::Null) => Ok(None),
                Some(v) => Ok(Some(FieldValue::value(
                    GqlValue::from_json(v.clone()).map_err(gql)?,
                ))),
            }
        })
    })
}

/// A forward reference: the parent row's FK cell → one point lookup of the
/// referenced row. N+1 accepted (prototype).
fn forward_ref_field(
    table_name: String,
    field_name: String,
    target_type: String,
    optional: bool,
) -> Field {
    let ty = if optional {
        TypeRef::named(target_type)
    } else {
        TypeRef::named_nn(target_type)
    };
    Field::new(field_name.clone(), ty, move |ctx| {
        let table_name = table_name.clone();
        let field_name = field_name.clone();
        FieldFuture::new(async move {
            let app = app_of(&ctx)?;
            let contract = &app.contract;
            let table = contract
                .table(&table_name)
                .ok_or_else(|| gql("schema/contract drift: table"))?;
            let field = table
                .field(&field_name)
                .ok_or_else(|| gql("schema/contract drift: field"))?;
            let Type::Ref { table: target, .. } = &field.ty else {
                return Err(gql("schema/contract drift: not a reference"));
            };
            let row = parent_row(&ctx)?;
            match row.get(&field_name) {
                None | Some(Json::Null) => Ok(None),
                Some(cell) => {
                    let key = json_to_sql(contract, table, field, cell).map_err(gql)?;
                    let target_table = contract
                        .table(target)
                        .ok_or_else(|| gql("schema/contract drift: target"))?;
                    let child = {
                        let db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                        write::select_by_key(contract, target_table, &db, &[key]).map_err(gql)?
                    };
                    Ok(child.map(FieldValue::owned_any))
                }
            }
        })
    })
}

/// A reverse collection on the referenced type: `<child>_<field>_list`,
/// children whose reference column equals the parent's key.
fn reverse_list_field(
    name: String,
    parent_table: String,
    child_table: String,
    ref_field: String,
    child_type: String,
) -> Field {
    Field::new(name, TypeRef::named_nn_list_nn(child_type), move |ctx| {
        let parent_table = parent_table.clone();
        let child_table = child_table.clone();
        let ref_field = ref_field.clone();
        FieldFuture::new(async move {
            let app = app_of(&ctx)?;
            let contract = &app.contract;
            let parent = contract
                .table(&parent_table)
                .ok_or_else(|| gql("schema/contract drift: parent"))?;
            let child = contract
                .table(&child_table)
                .ok_or_else(|| gql("schema/contract drift: child"))?;
            // inbound refs always target single-key tables (checker E010)
            let key_field = parent
                .single_key()
                .ok_or_else(|| gql("schema/contract drift: composite parent key"))?;
            let row = parent_row(&ctx)?;
            let cell = row
                .get(&key_field.name)
                .ok_or_else(|| gql("schema/contract drift: parent key cell"))?;
            let key = json_to_sql(contract, parent, key_field, cell).map_err(gql)?;
            let limit = read_limit(&ctx)?;
            let rows = query_rows(app, child, Some((ref_field.as_str(), &key)), limit)?;
            Ok(Some(FieldValue::list(
                rows.into_iter().map(FieldValue::owned_any),
            )))
        })
    })
    .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
}

/// `Query.<table>_list(limit): [T!]!`
fn root_list_field(table: &Table, type_name: &str) -> Field {
    let table_name = table.name.clone();
    Field::new(
        format!("{}_list", table.name),
        TypeRef::named_nn_list_nn(type_name),
        move |ctx| {
            let table_name = table_name.clone();
            FieldFuture::new(async move {
                let app = app_of(&ctx)?;
                let table = app
                    .contract
                    .table(&table_name)
                    .ok_or_else(|| gql("schema/contract drift: table"))?;
                let limit = read_limit(&ctx)?;
                let rows = query_rows(app, table, None, limit)?;
                Ok(Some(FieldValue::list(
                    rows.into_iter().map(FieldValue::owned_any),
                )))
            })
        },
    )
    .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
}

/// `Query.<table>(<key>): T` — single-key tables; a miss (including a
/// malformed uuid/timestamp key, §8's "malformed key matches no row") is
/// `null`, never an error.
fn root_by_key_field(contract: &Contract, table: &Table, type_name: &str) -> Field {
    let table_name = table.name.clone();
    let key_name = table.key[0].clone();
    let key_field = table.field(&key_name).expect("checked: key field exists");
    let arg_type = TypeRef::named_nn(scalar_name(contract, &key_field.ty));

    Field::new(table.name.clone(), TypeRef::named(type_name), move |ctx| {
        let table_name = table_name.clone();
        let key_name = key_name.clone();
        FieldFuture::new(async move {
            let app = app_of(&ctx)?;
            let contract = &app.contract;
            let table = contract
                .table(&table_name)
                .ok_or_else(|| gql("schema/contract drift: table"))?;
            let key_field = table
                .field(&key_name)
                .ok_or_else(|| gql("schema/contract drift: key"))?;
            let arg = ctx.args.try_get(&key_name)?;

            let key = match contract.value_type(&key_field.ty) {
                Type::Text => Some(SqlValue::Text(arg.string()?.to_string())),
                Type::Uuid => uuid::Uuid::parse_str(arg.string()?)
                    .ok()
                    .map(|u| SqlValue::Text(u.to_string())),
                Type::Int => Some(SqlValue::Integer(arg.i64()?)),
                Type::Bool => Some(SqlValue::Integer(arg.boolean()? as i64)),
                Type::Timestamp => time::OffsetDateTime::parse(arg.string()?, &Rfc3339)
                    .ok()
                    .map(|t| SqlValue::Text(t.format(&Rfc3339).expect("rfc3339 roundtrip"))),
                Type::Ref { .. } => unreachable!("value_type never returns a ref"),
            };
            let Some(key) = key else {
                return Ok(None); // malformed key value matches no row
            };

            let row = {
                let db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                write::select_by_key(contract, table, &db, &[key]).map_err(gql)?
            };
            Ok(row.map(FieldValue::owned_any))
        })
    })
    .argument(InputValue::new(table.key[0].clone(), arg_type))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;

    fn build(source: &str) -> Result<Schema, SchemaBuildError> {
        let contract = spock_lang::compile(source).expect("program compiles");
        let conn = engine::open(&contract, None).expect("engine opens");
        schema(Arc::new(App::new(contract, conn)))
    }

    #[test]
    fn pascal_casing() {
        assert_eq!(pascal_type_name("user"), "User");
        assert_eq!(pascal_type_name("post_media"), "PostMedia");
        assert_eq!(pascal_type_name("a_b_c"), "ABC");
        assert_eq!(pascal_type_name("_user"), "User");
        assert_eq!(pascal_type_name("a__b"), "AB");
        assert_eq!(pascal_type_name("user2"), "User2");
        assert_eq!(pascal_type_name("user_2"), "User2");
    }

    #[test]
    fn builds_for_a_normal_program() {
        let schema = build(
            "table user { key id: uuid = auto\n username: text unique }\n\
             table post { key id: uuid = auto\n author: user\n caption: text? }",
        )
        .unwrap();
        let sdl = schema.sdl();
        assert!(sdl.contains("type User"));
        assert!(sdl.contains("type Post"));
        assert!(sdl.contains("post_author_list"));
        assert!(!sdl.contains("type Mutation"));
    }

    #[test]
    fn duplicate_type_name_fails_startup() {
        let err = build(
            "table user_2 { key id: uuid = auto\n a: int }\n\
             table user2 { key id: uuid = auto\n b: int }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateTypeName { .. }),
            "{err}"
        );
    }

    #[test]
    fn reserved_type_name_fails_startup() {
        let err = build("table query { key id: uuid = auto\n a: int }").unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::ReservedTypeName { .. }),
            "{err}"
        );
    }

    #[test]
    fn root_field_collision_fails_startup() {
        // table `user`'s list field is `user_list`; a table named `user_list`
        // claims the same root name for its by-key field
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             table user_list { key id: uuid = auto\n b: int }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateQueryField { .. }),
            "{err}"
        );
    }

    #[test]
    fn reverse_field_collision_fails_startup() {
        // the reverse collection on user would be `post_author_list`, which
        // user also declares as a column
        let err = build(
            "table user { key id: uuid = auto\n post_author_list: int }\n\
             table post { key id: uuid = auto\n author: user }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateObjectField { .. }),
            "{err}"
        );
    }
}

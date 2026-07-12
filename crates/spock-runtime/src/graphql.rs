//! GraphQL reads and writes at `/graphql/v1` — Tier 1 of the dialect
//! specified in docs/spec/graphql.md (Hasura-mirrored).
//!
//! The schema is built dynamically at startup from the loaded contract —
//! types are unknown at compile time, so this is `async_graphql::dynamic`
//! end to end. The naming laws (graphql.md §3) are total:
//!
//! - object type = table name, verbatim (injective by construction — the
//!   language guarantees unique, lowercase table names);
//! - support types take Hasura's suffixes: `<t>_insert_input`,
//!   `<t>_set_input`, `<t>_pk_columns_input`;
//! - scalars are lowercase: `uuid`, `timestamp`;
//! - field names are the contract's, verbatim; forward references resolve
//!   to the referenced object, reverse collections are `<child>_by_<field>`;
//! - reserved table names and cross-table name collisions fail startup,
//!   never requests.
//!
//! Surface per table (graphql.md §4–§5): `Query.<t>(limit)` and
//! `Query.<t>_by_pk(<key args>)` (miss → `null`);
//! `Mutation.insert_<t>_one(object:)`, `update_<t>_by_pk(pk_columns:,
//! _set:)`, `delete_<t>_by_pk(<key args>)`, each returning the row. A
//! write that did not happen is an error (`extensions.code = "not_found"`),
//! never a silent `null` (deviation D1). `_set` semantics: absent = keep,
//! explicit `null` = clear (v0 §5.1 carve-out). All write errors carry the
//! §8.1 payload in `errors[].extensions`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputObject, InputValue, Object, ObjectAccessor,
    ResolverContext, Scalar, Schema, SchemaError, TypeRef,
};
use async_graphql::ErrorExtensions;
use async_graphql::Value as GqlValue;
use async_graphql_value::Value as AstValue;
use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value as Json};
use spock_lang::ir::{
    Contract, DerivedError, ErrorKind, Field as IrField, FnArity, FnDef, Table, Type,
};
use time::format_description::well_known::Rfc3339;

use crate::error::ApiError;
use crate::func;
use crate::value::json_to_sql;
use crate::write;
use crate::App;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

/// Table names the derivation reserves (graphql.md §3): they collide with
/// the operation roots or the derived scalars. The language only admits
/// lowercase identifiers, so the GraphQL builtins (`String`, `Query`, …)
/// are unrepresentable and need no entry here; `uuid`/`timestamp` are
/// doubly protected (type keywords the parser rejects as table names).
const RESERVED_TABLE_NAMES: &[&str] = &["query", "mutation", "subscription", "uuid", "timestamp"];

fn insert_input_name(table: &str) -> String {
    format!("{table}_insert_input")
}

fn set_input_name(table: &str) -> String {
    format!("{table}_set_input")
}

fn pk_columns_name(table: &str) -> String {
    format!("{table}_pk_columns_input")
}

/// Schema derivation failures. All are startup-fatal: a program the checker
/// accepted can still break the GraphQL naming laws (graphql.md §3), and
/// that must surface at load, not per-request.
#[derive(Debug, thiserror::Error)]
pub enum SchemaBuildError {
    #[error(
        "table `{table}` uses a reserved GraphQL name \
         (query, mutation, subscription, uuid, timestamp)"
    )]
    ReservedTableName { table: String },
    #[error("tables `{a}` and `{b}` both derive a GraphQL type named `{name}`")]
    DuplicateTypeName { a: String, b: String, name: String },
    #[error("query fields for {a} and {b} collide on `{name}`")]
    DuplicateQueryField { a: String, b: String, name: String },
    #[error("mutation fields for {a} and {b} collide on `{name}`")]
    DuplicateMutationField { a: String, b: String, name: String },
    #[error("on type `{type_name}`: generated field `{name}` collides with {other}")]
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

    // Pass 1a — the type name space. Object type = table name, verbatim
    // (injective by construction); each table also claims its Tier-1
    // support-type names, so a table literally named `user_insert_input`
    // cannot shadow the input type derived for `user`. A table's own four
    // names are pairwise distinct, so any duplicate is cross-table.
    let mut claimed_types: HashMap<String, String> = HashMap::new(); // type -> table
    for table in &contract.tables {
        if RESERVED_TABLE_NAMES.contains(&table.name.as_str()) {
            return Err(SchemaBuildError::ReservedTableName {
                table: table.name.clone(),
            });
        }
        for name in [
            table.name.clone(),
            insert_input_name(&table.name),
            set_input_name(&table.name),
            pk_columns_name(&table.name),
        ] {
            if let Some(other) = claimed_types.insert(name.clone(), table.name.clone()) {
                return Err(SchemaBuildError::DuplicateTypeName {
                    a: other,
                    b: table.name.clone(),
                    name,
                });
            }
        }
    }

    // Pass 1a′ — records join the type name space: the bare name only
    // (records derive no support types), against the same reserved list.
    for record in &contract.records {
        if RESERVED_TABLE_NAMES.contains(&record.name.as_str()) {
            return Err(SchemaBuildError::ReservedTableName {
                table: record.name.clone(),
            });
        }
        if let Some(other) = claimed_types.insert(record.name.clone(), record.name.clone()) {
            return Err(SchemaBuildError::DuplicateTypeName {
                a: other,
                b: record.name.clone(),
                name: record.name.clone(),
            });
        }
    }

    // Pass 1b — query-root field names: the bare list root, `_by_pk`,
    // and read fns (polarity puts them here — §7.4, RFD 0012). A read fn
    // named exactly like a table's root field fails startup like every
    // other claim collision.
    let mut claimed_roots: HashMap<String, String> = HashMap::new(); // field -> owner
    let dup_query: Collide = |a, b, name| SchemaBuildError::DuplicateQueryField { a, b, name };
    for table in &contract.tables {
        let owner = format!("table `{}`", table.name);
        claim(
            &mut claimed_roots,
            table.name.clone(),
            owner.clone(),
            dup_query,
        )?;
        claim(
            &mut claimed_roots,
            format!("{}_by_pk", table.name),
            owner,
            dup_query,
        )?;
    }
    for f in contract.fns.iter().filter(|f| f.readonly) {
        claim(
            &mut claimed_roots,
            f.name.clone(),
            format!("fn `{}`", f.name),
            dup_query,
        )?;
    }

    // Pass 1c — mutation-root field names. Derived CRUD names could once
    // not collide by construction; fn names break the construction, so
    // everything registered on the root is claimed — exactly what is
    // registered (a pure-key table claims no update).
    let mut claimed_mutations: HashMap<String, String> = HashMap::new(); // field -> owner
    let dup_mutation: Collide =
        |a, b, name| SchemaBuildError::DuplicateMutationField { a, b, name };
    for table in &contract.tables {
        // builtin tables (storage_object) register no floor mutations (Pass 3b),
        // so they claim no mutation-root names here — else a user `mut fn` could
        // collide with a name that is never registered.
        if table.builtin {
            continue;
        }
        let owner = format!("table `{}`", table.name);
        claim(
            &mut claimed_mutations,
            format!("insert_{}_one", table.name),
            owner.clone(),
            dup_mutation,
        )?;
        if has_settable_fields(table) {
            claim(
                &mut claimed_mutations,
                format!("update_{}_by_pk", table.name),
                owner.clone(),
                dup_mutation,
            )?;
        }
        claim(
            &mut claimed_mutations,
            format!("delete_{}_by_pk", table.name),
            owner,
            dup_mutation,
        )?;
    }
    for f in contract.fns.iter().filter(|f| !f.readonly) {
        claim(
            &mut claimed_mutations,
            f.name.clone(),
            format!("fn `{}`", f.name),
            dup_mutation,
        )?;
    }

    // Pass 2 — object types (declared fields, forward refs, reverse
    // collections), with per-type field-name collision guard.
    let mut objects: Vec<Object> = Vec::new();
    for table in &contract.tables {
        let mut obj = Object::new(table.name.clone());
        // the author's `///` doc becomes the type's GraphQL description (RFD 0016)
        if let Some(doc) = &table.doc {
            obj = obj.description(doc.clone());
        }
        let mut claimed_fields: HashMap<String, String> = HashMap::new();

        for field in &table.fields {
            claimed_fields.insert(field.name.clone(), "a declared field".into());
            let mut fld = match &field.ty {
                Type::Ref { table: target, .. } => forward_ref_field(
                    table.name.clone(),
                    field.name.clone(),
                    target.clone(),
                    field.optional,
                ),
                _ => scalar_field(
                    field.name.clone(),
                    scalar_type_ref(contract, &field.ty, field.optional),
                ),
            };
            if let Some(doc) = &field.doc {
                fld = fld.description(doc.clone());
            }
            obj = obj.field(fld);
        }

        for (child, ref_field) in contract.inbound_refs(&table.name) {
            let name = format!("{}_by_{}", child.name, ref_field.name);
            if let Some(other) = claimed_fields.insert(name.clone(), "a reverse collection".into())
            {
                return Err(SchemaBuildError::DuplicateObjectField {
                    type_name: table.name.clone(),
                    name,
                    other,
                });
            }
            obj = obj.field(reverse_list_field(
                name,
                table.name.clone(),
                child.name.clone(),
                ref_field.name.clone(),
            ));
        }

        objects.push(obj);
    }

    // Pass 2b — input types: `insert_input` always; `set_input` and
    // `pk_columns_input` only for tables with settable (non-key) fields —
    // GraphQL forbids empty input objects, and a pure-key table derives no
    // update anyway (nothing is settable).
    let mut inputs: Vec<InputObject> = Vec::new();
    for table in &contract.tables {
        // no floor mutations for a builtin table → no insert/set/pk input types
        // (they would be dead, unreferenced types in the schema).
        if table.builtin {
            continue;
        }
        inputs.push(insert_input_type(contract, table));
        if has_settable_fields(table) {
            inputs.push(set_input_type(contract, table));
            inputs.push(pk_columns_type(contract, table));
        }
    }

    // Pass 2c — record object types: scalar fields, no relations.
    for record in &contract.records {
        let mut obj = Object::new(record.name.clone());
        if let Some(doc) = &record.doc {
            obj = obj.description(doc.clone());
        }
        for f in &record.fields {
            let mut fld =
                scalar_field(f.name.clone(), scalar_type_ref(contract, &f.ty, f.optional));
            if let Some(doc) = &f.doc {
                fld = fld.description(doc.clone());
            }
            obj = obj.field(fld);
        }
        objects.push(obj);
    }

    // Pass 3 — the Query root. By-pk fields take the key fields as args,
    // so composite-key tables are addressable too.
    let mut query = Object::new("Query");
    for table in &contract.tables {
        query = query.field(root_list_field(table));
        query = query.field(root_by_pk_field(contract, table));
    }

    // Pass 3b — the Mutation root: insert/update/delete per table (update
    // only where something is settable).
    let mut mutation = Object::new("Mutation");
    for table in &contract.tables {
        // A builtin table (RFD 0018: `storage_object`) is read-only on the
        // open floor — the storage protocol is its only write path, so
        // metadata cannot desync from bytes. Reads (Pass 3) keep it queryable
        // and joinable.
        if table.builtin {
            continue;
        }
        mutation = mutation.field(insert_one_field(table));
        if has_settable_fields(table) {
            mutation = mutation.field(update_by_pk_field(table));
        }
        mutation = mutation.field(delete_by_pk_field(contract, table));
    }

    // Pass 3c — fn fields: the deliberate surface, on the root its
    // polarity names (§7.4, RFD 0012) — read fns next to the borrowed
    // floor's query fields, `mut` fns on Mutation (the Hasura-Actions
    // analogue, graphql.md §1).
    for f in &contract.fns {
        if f.readonly {
            query = query.field(fn_field(contract, f));
        } else {
            mutation = mutation.field(fn_field(contract, f));
        }
    }

    // Pass 4 — register and finish. Introspection stays enabled.
    let mut builder = Schema::build("Query", Some("Mutation"), None)
        .register(Scalar::new("uuid").description("A UUID in canonical hyphenated form"))
        .register(Scalar::new("timestamp").description("An RFC 3339 UTC timestamp"))
        .data(app.clone())
        // self-references permit unbounded nesting; stay above GraphiQL's
        // introspection depth while bounding pathological queries (D6)
        .limit_depth(32);
    for input in inputs {
        builder = builder.register(input);
    }
    for obj in objects {
        builder = builder.register(obj);
    }
    builder = builder.register(query).register(mutation);
    Ok(builder.finish()?)
}

/// Whether the table has any client-settable field — the precondition for
/// deriving `update_<t>_by_pk` and its input types. Excludes keys (immutable)
/// and `= me` fields (RFD 0014: server-stamped, off the client surface). If
/// none remain, no `_set` input is emitted — an empty GraphQL input object is
/// invalid and would fail schema build (§14.4).
fn has_settable_fields(table: &Table) -> bool {
    table
        .fields
        .iter()
        .any(|f| !table.key.contains(&f.name) && !f.is_actor_default())
}

/// A root's collision error, built from (prior owner, new owner, field).
type Collide = fn(String, String, String) -> SchemaBuildError;

/// Claim `name` for `owner` in a root's field namespace; a prior owner is
/// a startup-fatal collision. One mechanism for every root (§8.2 naming
/// laws) — the roots differ only in their error variant.
fn claim(
    map: &mut HashMap<String, String>,
    name: String,
    owner: String,
    collide: Collide,
) -> Result<(), SchemaBuildError> {
    if let Some(other) = map.insert(name.clone(), owner.clone()) {
        return Err(collide(other, owner, name));
    }
    Ok(())
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
        Type::Float => TypeRef::FLOAT,
        Type::Bool => TypeRef::BOOLEAN,
        Type::Uuid => "uuid",
        Type::Timestamp => "timestamp",
        // A closed set stays a GraphQL String (Tier-1 fidelity, no minted
        // enum); membership is enforced by the write path and the CHECK.
        Type::Set { .. } => TypeRef::STRING,
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    }
}

/// An input field carrying its declared `///` doc as a GraphQL description
/// (RFD 0016), or none when undocumented.
fn described_input(name: String, ty: TypeRef, doc: &Option<String>) -> InputValue {
    let iv = InputValue::new(name, ty);
    match doc {
        Some(d) => iv.description(d.clone()),
        None => iv,
    }
}

/// `<t>_insert_input`: every field, **all nullable** (graphql.md §5).
/// Required-ness is enforced by the contract at runtime, so the derived
/// `required` error stays reachable instead of being shadowed by GraphQL
/// validation. Reference fields take the target's key scalar.
fn insert_input_type(contract: &Contract, table: &Table) -> InputObject {
    let mut input = InputObject::new(insert_input_name(&table.name));
    for f in &table.fields {
        // `= me` fields (RFD 0014) leave the client insert surface: the
        // runtime stamps the actor, so a client cannot forge them.
        if f.is_actor_default() {
            continue;
        }
        input = input.field(described_input(
            f.name.clone(),
            TypeRef::named(scalar_name(contract, &f.ty)),
            &f.doc,
        ));
    }
    input
}

/// `<t>_set_input`: every non-key field, all nullable, with update
/// semantics — absent = keep, explicit `null` = clear (v0 §5.1). Key
/// fields never appear: keys are immutable, the key is the row's identity.
fn set_input_type(contract: &Contract, table: &Table) -> InputObject {
    let mut input = InputObject::new(set_input_name(&table.name));
    for f in &table.fields {
        // keys are immutable; `= me` fields are stamped once and never
        // reassigned by a client (RFD 0014) — both leave the update surface.
        if table.key.contains(&f.name) || f.is_actor_default() {
            continue;
        }
        input = input.field(described_input(
            f.name.clone(),
            TypeRef::named(scalar_name(contract, &f.ty)),
            &f.doc,
        ));
    }
    input
}

/// `<t>_pk_columns_input`: the key fields, non-null.
fn pk_columns_type(contract: &Contract, table: &Table) -> InputObject {
    let mut input = InputObject::new(pk_columns_name(&table.name));
    for name in &table.key {
        let field = table.field(name).expect("checked: key fields exist");
        input = input.field(described_input(
            name.clone(),
            TypeRef::named_nn(scalar_name(contract, &field.ty)),
            &field.doc,
        ));
    }
    input
}

fn gql<E: std::fmt::Display>(e: E) -> async_graphql::Error {
    async_graphql::Error::new(e.to_string())
}

/// The per-request actor (RFD 0014), injected into each GraphQL request via
/// `Request::data` by the HTTP layer (never the schema-global `.data`, which
/// would bleed across requests — §14.3). Resolvers read it with
/// `ctx.data_opt::<CurrentActor>()`.
pub struct CurrentActor(pub Option<SqlValue>);

/// The current actor from the resolver context, or `None` when anonymous /
/// unset. Cloned because `SqlValue` is not `Copy`.
fn actor_of(ctx: &ResolverContext<'_>) -> Option<SqlValue> {
    ctx.data_opt::<CurrentActor>().and_then(|a| a.0.clone())
}

fn app_of<'a>(ctx: &ResolverContext<'a>) -> Result<&'a Arc<App>, async_graphql::Error> {
    ctx.data::<Arc<App>>()
}

fn parent_row<'a>(ctx: &ResolverContext<'a>) -> Result<&'a Json, async_graphql::Error> {
    ctx.parent_value.try_downcast_ref::<Json>()
}

/// `limit` argument: default 50, ceiling 200 (deviation D2), negative is
/// an error — mirroring the REST surface. An explicit `null` (or a
/// null-coerced unprovided variable) is absence (v0 §5.1) → the default.
fn read_limit(ctx: &ResolverContext<'_>) -> Result<i64, async_graphql::Error> {
    match ctx.args.get("limit") {
        None => Ok(DEFAULT_LIMIT),
        Some(v) if v.is_null() => Ok(DEFAULT_LIMIT),
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

/// A reverse collection on the referenced type: `<child>_by_<field>`,
/// children whose reference column equals the parent's key.
fn reverse_list_field(
    name: String,
    parent_table: String,
    child_table: String,
    ref_field: String,
) -> Field {
    let child_type = child_table.clone();
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

/// `Query.<t>(limit): [t!]!` — the bare table name is the list root
/// (Hasura convention).
fn root_list_field(table: &Table) -> Field {
    let table_name = table.name.clone();
    Field::new(
        table.name.clone(),
        TypeRef::named_nn_list_nn(table.name.clone()),
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

/// Convert one argument value per the field's *value* type. `Ok(None)`
/// means a well-typed but unparseable uuid/timestamp — a key that can
/// match no row.
fn arg_to_sql(
    contract: &Contract,
    field: &IrField,
    arg: async_graphql::dynamic::ValueAccessor<'_>,
) -> Result<Option<SqlValue>, async_graphql::Error> {
    Ok(match contract.value_type(&field.ty) {
        Type::Text => Some(SqlValue::Text(arg.string()?.to_string())),
        Type::Uuid => uuid::Uuid::parse_str(arg.string()?)
            .ok()
            .map(|u| SqlValue::Text(u.to_string())),
        Type::Int => Some(SqlValue::Integer(arg.i64()?)),
        Type::Float => Some(SqlValue::Real(arg.f64()?)),
        Type::Bool => Some(SqlValue::Integer(arg.boolean()? as i64)),
        Type::Timestamp => time::OffsetDateTime::parse(arg.string()?, &Rfc3339)
            .ok()
            .map(|t| SqlValue::Text(crate::value::canon_timestamp(t))),
        // A set arg is textual; a set is never a key, so this is reached
        // only if a set field ever becomes an argument — coerce like text.
        Type::Set { .. } => Some(SqlValue::Text(arg.string()?.to_string())),
        Type::Ref { .. } => unreachable!("value_type never returns a ref"),
    })
}

/// One required argument per key field — shared by `Query.<t>_by_pk` and
/// `delete_<t>_by_pk`.
fn key_args(contract: &Contract, table: &Table) -> Vec<InputValue> {
    table
        .key
        .iter()
        .map(|name| {
            let field = table.field(name).expect("checked: key fields exist");
            InputValue::new(
                name.clone(),
                TypeRef::named_nn(scalar_name(contract, &field.ty)),
            )
        })
        .collect()
}

/// How a key that can match no row (malformed uuid/timestamp) lands:
/// reads render `null`; writes are errors — a write that did not happen
/// must shout (deviation D1).
#[derive(Clone, Copy)]
enum KeyMiss {
    ReadNull,
    WriteNotFound,
}

/// Parse inline key args in key order. `Ok(None)` only under `ReadNull`.
fn key_from_args(
    contract: &Contract,
    table: &Table,
    ctx: &ResolverContext<'_>,
    miss: KeyMiss,
) -> Result<Option<Vec<SqlValue>>, async_graphql::Error> {
    let mut key = Vec::with_capacity(table.key.len());
    for name in &table.key {
        let field = table
            .field(name)
            .ok_or_else(|| gql("schema/contract drift: key"))?;
        let arg = ctx.args.try_get(name)?;
        match arg_to_sql(contract, field, arg)? {
            Some(v) => key.push(v),
            None => {
                return match miss {
                    KeyMiss::ReadNull => Ok(None),
                    KeyMiss::WriteNotFound => Err(api_error_to_gql(ApiError::not_found(format!(
                        "no {} row with this key",
                        table.name
                    )))),
                };
            }
        }
    }
    Ok(Some(key))
}

/// Parse the key values out of a `pk_columns` object, in key order. Its
/// input fields are non-null, so presence is validation-guaranteed; a
/// malformed uuid/timestamp value can match no row, which for a write is
/// `not_found` (deviation D1).
fn key_from_pk_columns(
    contract: &Contract,
    table: &Table,
    pk: &ObjectAccessor<'_>,
) -> Result<Vec<SqlValue>, async_graphql::Error> {
    let mut key = Vec::with_capacity(table.key.len());
    for name in &table.key {
        let field = table
            .field(name)
            .ok_or_else(|| gql("schema/contract drift: key"))?;
        let arg = pk.try_get(name)?;
        match arg_to_sql(contract, field, arg)? {
            Some(v) => key.push(v),
            None => {
                return Err(api_error_to_gql(ApiError::not_found(format!(
                    "no {} row with this key",
                    table.name
                ))));
            }
        }
    }
    Ok(key)
}

/// Port an [`ApiError`] to a GraphQL error carrying the §8.1 envelope
/// payload in `extensions` (`status` is dropped — GraphQL is HTTP 200).
fn api_error_to_gql(e: ApiError) -> async_graphql::Error {
    async_graphql::Error::new(e.message.clone()).extend_with(|_, ext| {
        ext.set("code", e.code.clone());
        ext.set("kind", e.kind);
        match &e.table {
            Some(t) => ext.set("table", t.clone()),
            None => ext.set("table", GqlValue::Null),
        }
        ext.set(
            "fields",
            GqlValue::List(e.fields.iter().cloned().map(GqlValue::String).collect()),
        );
    })
}

/// Field names inside the object literal bound to `arg` whose values are
/// nullable variables the client did not provide (and whose definitions
/// carry no explicit default). async-graphql coerces such variables to
/// `null` (parser `VariableDefinition::default_value`) even inside input
/// objects; the GraphQL spec says such a field is *omitted* — we restore
/// the spec (graphql.md §5, normative), because `_set` semantics hang on
/// the distinction (v0 §5.1 carve-out). When the whole object is
/// variable-bound, JSON absence is real absence and nothing needs
/// restoring.
fn unprovided_variable_fields(ctx: &ResolverContext<'_>, arg: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(value) = ctx.ctx.item.node.get_argument(arg) else {
        return out;
    };
    let AstValue::Object(fields) = &value.node else {
        return out;
    };
    let env = &ctx.ctx.query_env;
    for (name, value) in fields {
        let AstValue::Variable(var_name) = value else {
            continue;
        };
        if env.variables.contains_key(var_name) {
            continue; // provided — possibly as an explicit null
        }
        let defaulted = env
            .operation
            .node
            .variable_definitions
            .iter()
            .any(|def| def.node.name.node == *var_name && def.node.default_value.is_some());
        if !defaulted {
            out.insert(name.to_string());
        }
    }
    out
}

/// `Query.<t>_by_pk(<key args>): t` — every table; a miss (including a
/// malformed uuid/timestamp key value) is `null`, never an error.
fn root_by_pk_field(contract: &Contract, table: &Table) -> Field {
    let table_name = table.name.clone();
    let mut field = Field::new(
        format!("{}_by_pk", table.name),
        TypeRef::named(table.name.clone()),
        move |ctx| {
            let table_name = table_name.clone();
            FieldFuture::new(async move {
                let app = app_of(&ctx)?;
                let contract = &app.contract;
                let table = contract
                    .table(&table_name)
                    .ok_or_else(|| gql("schema/contract drift: table"))?;
                let Some(key) = key_from_args(contract, table, &ctx, KeyMiss::ReadNull)? else {
                    return Ok(None); // malformed key value matches no row
                };
                let row = {
                    let db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                    write::select_by_key(contract, table, &db, &key).map_err(gql)?
                };
                Ok(row.map(FieldValue::owned_any))
            })
        },
    );
    for arg in key_args(contract, table) {
        field = field.argument(arg);
    }
    field
}

/// Codes from the table's derived errors matching `pred`, plus `extra` —
/// for mutation descriptions.
fn error_codes(table: &Table, pred: impl Fn(&DerivedError) -> bool, extra: &[&str]) -> Vec<String> {
    let mut codes: Vec<String> = table
        .errors
        .iter()
        .filter(|e| pred(e))
        .map(|e| e.code.clone())
        .collect();
    codes.extend(extra.iter().map(|s| s.to_string()));
    codes
}

/// Description enumerating the derived-error codes a mutation can produce —
/// the contract's failure surface, visible through introspection
/// (graphql.md §1).
fn mutation_description(action: &str, table: &Table, codes: &[String]) -> String {
    format!(
        "{action} one {} row. Errors: {}.",
        table.name,
        codes.join(", ")
    )
}

/// `Mutation.insert_<t>_one(object: <t>_insert_input!): t!` — defaults
/// apply on omission; on insert `null` is absence (v0 §5.1), so a
/// null-coerced unprovided variable inside the object already means
/// "omitted" and needs no correction.
fn insert_one_field(table: &Table) -> Field {
    let table_name = table.name.clone();
    let codes = error_codes(
        table,
        |e| match e.kind {
            ErrorKind::Key | ErrorKind::Unique | ErrorKind::RefNotFound => true,
            // required is insert-reachable only where no default steps in
            ErrorKind::Required => e
                .fields
                .first()
                .and_then(|f| table.field(f))
                .is_some_and(|f| f.default.is_none()),
            // a value-constraint (set or check) always fires on insert
            ErrorKind::Invalid => true,
            _ => false,
        },
        &[],
    );
    Field::new(
        format!("insert_{}_one", table.name),
        TypeRef::named_nn(table.name.clone()),
        move |ctx| {
            let table_name = table_name.clone();
            FieldFuture::new(async move {
                let app = app_of(&ctx)?;
                let contract = &app.contract;
                let table = contract
                    .table(&table_name)
                    .ok_or_else(|| gql("schema/contract drift: table"))?;
                let object = ctx.args.try_get("object")?.object()?;
                let mut body = Map::new();
                for f in &table.fields {
                    if let Some(v) = object.get(&f.name) {
                        body.insert(f.name.clone(), v.deserialize::<Json>()?);
                    }
                }
                // `= me` fields are absent from the input by construction
                // (removed from insert_input_type); the runtime stamps the
                // request actor for them (RFD 0014 §14.3).
                let actor = actor_of(&ctx);
                let row = {
                    let mut db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                    write::insert_row(contract, table, &mut db, &body, actor)
                        .map_err(api_error_to_gql)?
                };
                Ok(Some(FieldValue::owned_any(row)))
            })
        },
    )
    .argument(InputValue::new(
        "object",
        TypeRef::named_nn(insert_input_name(&table.name)),
    ))
    .description(mutation_description("Insert", table, &codes))
}

/// `Mutation.update_<t>_by_pk(pk_columns: <t>_pk_columns_input!, _set:
/// <t>_set_input!): t!` — keys select and are immutable; `_set` uses
/// update semantics: absent = keep, explicit `null` = clear or the derived
/// `required` error (v0 §5.1 carve-out, §7.2 Updates). Only derived for
/// tables with settable fields.
fn update_by_pk_field(table: &Table) -> Field {
    let table_name = table.name.clone();
    let codes = error_codes(
        table,
        |e| match e.kind {
            ErrorKind::Unique | ErrorKind::RefNotFound => true,
            // required is update-reachable by clearing any non-key field
            ErrorKind::Required => e.fields.first().is_some_and(|f| !table.key.contains(f)),
            // a value-constraint is update-reachable iff any of its fields
            // is settable (non-key) — an all-key check cannot fire (L-J)
            ErrorKind::Invalid => e.fields.iter().any(|f| !table.key.contains(f)),
            _ => false,
        },
        &["not_found"],
    );
    Field::new(
        format!("update_{}_by_pk", table.name),
        TypeRef::named_nn(table.name.clone()),
        move |ctx| {
            let table_name = table_name.clone();
            FieldFuture::new(async move {
                let app = app_of(&ctx)?;
                let contract = &app.contract;
                let table = contract
                    .table(&table_name)
                    .ok_or_else(|| gql("schema/contract drift: table"))?;
                let pk = ctx.args.try_get("pk_columns")?.object()?;
                let key = key_from_pk_columns(contract, table, &pk)?;
                let set = ctx.args.try_get("_set")?.object()?;
                let unprovided = unprovided_variable_fields(&ctx, "_set");
                let mut changes = Map::new();
                for f in &table.fields {
                    if table.key.contains(&f.name) {
                        continue;
                    }
                    let Some(v) = set.get(&f.name) else {
                        continue; // absent = keep
                    };
                    if v.is_null() && unprovided.contains(&f.name) {
                        continue; // unprovided variable = omitted (spec-faithful)
                    }
                    changes.insert(f.name.clone(), v.deserialize::<Json>()?);
                }
                let row = {
                    let mut db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                    write::update_row(contract, table, &mut db, &key, &changes)
                        .map_err(api_error_to_gql)?
                };
                Ok(Some(FieldValue::owned_any(row)))
            })
        },
    )
    .argument(InputValue::new(
        "pk_columns",
        TypeRef::named_nn(pk_columns_name(&table.name)),
    ))
    .argument(InputValue::new(
        "_set",
        TypeRef::named_nn(set_input_name(&table.name)),
    ))
    .description(mutation_description("Update", table, &codes))
}

/// `Mutation.delete_<t>_by_pk(<key args>): t!` — returns the row as it
/// read before deletion; inbound `restrict` references block (§7.2
/// Deletes).
fn delete_by_pk_field(contract: &Contract, table: &Table) -> Field {
    let table_name = table.name.clone();
    let codes = error_codes(table, |e| e.kind == ErrorKind::Restricted, &["not_found"]);
    let mut field = Field::new(
        format!("delete_{}_by_pk", table.name),
        TypeRef::named_nn(table.name.clone()),
        move |ctx| {
            let table_name = table_name.clone();
            FieldFuture::new(async move {
                let app = app_of(&ctx)?;
                let contract = &app.contract;
                let table = contract
                    .table(&table_name)
                    .ok_or_else(|| gql("schema/contract drift: table"))?;
                let key = key_from_args(contract, table, &ctx, KeyMiss::WriteNotFound)?
                    .expect("WriteNotFound never yields None");
                let row = {
                    let mut db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                    write::delete_row(contract, table, &mut db, &key).map_err(api_error_to_gql)?
                };
                Ok(Some(FieldValue::owned_any(row)))
            })
        },
    )
    .description(mutation_description("Delete", table, &codes));
    for arg in key_args(contract, table) {
        field = field.argument(arg);
    }
    field
}

/// `<Root>.<fn>(<params>): <ret>` — a declared fn on the root its
/// polarity names (`Query` for reads, `Mutation` for `mut` — §7.4), next
/// to the derived CRUD (graphql.md §1: the deliberate surface beside the
/// borrowed floor). One argument per param, nullable iff optional; for fn
/// arguments `null` means absent, so the `_set`-style unprovided-variable
/// machinery is unnecessary here. The description carries the declared
/// error codes.
fn fn_field(contract: &Contract, f: &FnDef) -> Field {
    let fn_name = f.name.clone();
    let target = match f.returns.scalar_type() {
        Some(ty) => scalar_name(contract, &ty).to_string(),
        None => f.returns.of.clone(),
    };
    let ret = match f.returns.arity {
        FnArity::One => TypeRef::named_nn(target),
        FnArity::Maybe => TypeRef::named(target),
        FnArity::Many => TypeRef::named_nn_list_nn(target),
    };
    // a scalar result is a GraphQL leaf: a value, not a resolvable row
    let leaf = |v: Json| -> async_graphql::Result<FieldValue<'static>> {
        Ok(FieldValue::value(GqlValue::from_json(v).map_err(gql)?))
    };
    let mut field = Field::new(f.name.clone(), ret, move |ctx| {
        let fn_name = fn_name.clone();
        FieldFuture::new(async move {
            let app = app_of(&ctx)?;
            let contract = &app.contract;
            let f = contract
                .fn_def(&fn_name)
                .ok_or_else(|| gql("schema/contract drift: fn"))?;
            let mut args = Map::new();
            for p in &f.params {
                let Some(v) = ctx.args.get(&p.name) else {
                    continue;
                };
                if v.is_null() {
                    continue; // null == absent for fn args
                }
                args.insert(p.name.clone(), v.deserialize::<Json>()?);
            }
            let actor = actor_of(&ctx);
            let result = {
                let mut db = app.db.lock().map_err(|_| gql("db lock poisoned"))?;
                func::call(contract, f, &mut db, &args, actor).map_err(api_error_to_gql)?
            };
            let scalar = f.returns.scalar;
            let wrap = |v: Json| -> async_graphql::Result<FieldValue<'static>> {
                if scalar {
                    leaf(v)
                } else {
                    Ok(FieldValue::owned_any(v))
                }
            };
            Ok(match f.returns.arity {
                FnArity::One => Some(wrap(result)?),
                FnArity::Maybe => {
                    if result.is_null() {
                        None
                    } else {
                        Some(wrap(result)?)
                    }
                }
                FnArity::Many => {
                    let Json::Array(rows) = result else {
                        return Err(gql("fn execution drift: expected rows"));
                    };
                    Some(FieldValue::list(
                        rows.into_iter()
                            .map(wrap)
                            .collect::<async_graphql::Result<Vec<_>>>()?,
                    ))
                }
            })
        })
    });
    for p in &f.params {
        let scalar = scalar_name(contract, &p.ty);
        let ty = if p.optional {
            TypeRef::named(scalar)
        } else {
            TypeRef::named_nn(scalar)
        };
        field = field.argument(described_input(p.name.clone(), ty, &p.doc));
    }
    let generated = if f.errors.is_empty() {
        format!("Call fn `{}`.", f.name)
    } else {
        format!("Call fn `{}`. Errors: {}.", f.name, f.errors.join(", "))
    };
    // the author's `///` doc leads; the generated call/errors line follows,
    // so the "Errors:" metadata is never lost (RFD 0016).
    let description = match &f.doc {
        Some(doc) => format!("{doc}\n\n{generated}"),
        None => generated,
    };
    field.description(description)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;

    fn build(source: &str) -> Result<Schema, SchemaBuildError> {
        let contract = spock_lang::compile(source).expect("program compiles");
        let conn = engine::open(&contract, None, None).expect("engine opens");
        schema(Arc::new(App::new(contract, conn)))
    }

    #[test]
    fn builds_for_a_normal_program() {
        let schema = build(
            "table user { key id: uuid = auto\n username: text unique }\n\
             table post { key id: uuid = auto\n author: user\n caption: text? }",
        )
        .unwrap();
        let sdl = schema.sdl();
        assert!(sdl.contains("type user"), "{sdl}");
        assert!(sdl.contains("type post"), "{sdl}");
        assert!(sdl.contains("post_by_author"), "{sdl}");
        assert!(sdl.contains("type Mutation"), "{sdl}");
        for mutation in [
            "insert_user_one(",
            "update_user_by_pk(",
            "delete_user_by_pk(",
            "insert_post_one(",
        ] {
            assert!(sdl.contains(mutation), "missing {mutation} in:\n{sdl}");
        }
    }

    #[test]
    fn docs_become_sdl_descriptions() {
        let schema = build(
            "/// a person on the network\n\
             table user {\n\
               key id: uuid = auto\n\
               /// the handle\n\
               username: text unique\n\
             }\n\
             /// look someone up\n\
             fn find(name: text) -> user? { unchecked sql(\"SELECT * FROM user WHERE username = :name\") }",
        )
        .unwrap();
        let sdl = schema.sdl();
        // table, field, and fn docs render as SDL descriptions (triple-quoted)
        assert!(sdl.contains("\"\"\""), "no descriptions in:\n{sdl}");
        assert!(sdl.contains("a person on the network"), "{sdl}");
        assert!(sdl.contains("the handle"), "{sdl}");
        assert!(sdl.contains("look someone up"), "{sdl}");
    }

    #[test]
    fn fn_doc_keeps_the_errors_metadata() {
        let schema = build(
            "table user { key id: uuid = auto\n username: text unique }\n\
             /// rename a user\n\
             mut fn rename(user: user, name: text) -> user ! user_username_taken {\n\
               unchecked sql(\"UPDATE user SET username = :name WHERE id = :user RETURNING *\")\n\
             }",
        )
        .unwrap();
        let sdl = schema.sdl();
        // the author's doc leads, but the generated Errors: line is not lost
        assert!(sdl.contains("rename a user"), "{sdl}");
        assert!(sdl.contains("Errors: user_username_taken"), "{sdl}");
    }

    /// One line of the SDL, located by prefix.
    fn sdl_line(sdl: &str, prefix: &str) -> String {
        sdl.lines()
            .find(|l| l.trim_start().starts_with(prefix))
            .unwrap_or_else(|| panic!("no `{prefix}` line in:\n{sdl}"))
            .to_string()
    }

    /// The trimmed body lines of one type/input block, located by header
    /// prefix (e.g. `input user_set_input`).
    fn sdl_block(sdl: &str, header_prefix: &str) -> Vec<String> {
        let mut lines = sdl.lines().skip_while(|l| !l.starts_with(header_prefix));
        assert!(
            lines.next().is_some(),
            "no `{header_prefix}` block in:\n{sdl}"
        );
        lines
            .take_while(|l| !l.starts_with('}'))
            .map(|l| l.trim().to_string())
            .collect()
    }

    #[test]
    fn derivation_laws() {
        let schema = build(
            "table user { key id: uuid = auto\n username: text unique\n bio: text?\n joined_at: timestamp = now }\n\
             table follow { key (follower, target)\n follower: user\n target: user\n since: timestamp = now }",
        )
        .unwrap();
        let sdl = schema.sdl();

        // insert_input: every field, all nullable — required-ness is the
        // runtime's (the derived error stays un-shadowed)
        let insert_input = sdl_block(&sdl, "input user_insert_input");
        for field in [
            "id: uuid",
            "username: String",
            "bio: String",
            "joined_at: timestamp",
        ] {
            assert!(
                insert_input.contains(&field.to_string()),
                "{insert_input:?}"
            );
        }

        // set_input: non-key fields only, all nullable
        let set_input = sdl_block(&sdl, "input user_set_input");
        assert!(
            !set_input.iter().any(|l| l.starts_with("id:")),
            "{set_input:?}"
        );
        for field in ["username: String", "bio: String", "joined_at: timestamp"] {
            assert!(set_input.contains(&field.to_string()), "{set_input:?}");
        }

        // pk_columns: key fields, non-null
        let pk = sdl_block(&sdl, "input user_pk_columns_input");
        assert!(pk.contains(&"id: uuid!".to_string()), "{pk:?}");

        // mutation shapes (graphql.md §5)
        let insert = sdl_line(&sdl, "insert_user_one(");
        assert!(
            insert.contains("(object: user_insert_input!): user!"),
            "{insert}"
        );
        let update = sdl_line(&sdl, "update_user_by_pk(");
        assert!(
            update.contains("(pk_columns: user_pk_columns_input!, _set: user_set_input!): user!"),
            "{update}"
        );

        // composite keys: full key args on delete and on the by-pk query
        let delete = sdl_line(&sdl, "delete_follow_by_pk(");
        assert!(
            delete.contains("follower: uuid!, target: uuid!"),
            "{delete}"
        );
        let by_pk = sdl_line(&sdl, "follow_by_pk(");
        assert!(
            by_pk.contains("follower: uuid!, target: uuid!): follow"),
            "{by_pk}"
        );
    }

    #[test]
    fn pure_key_table_derives_no_update() {
        // nothing is settable on a table whose every field is a key —
        // no update mutation, no set/pk input types (graphql.md §5)
        let schema = build(
            "table user { key id: uuid = auto\n a: int }\n\
             table follow { key (follower, target)\n follower: user\n target: user }",
        )
        .unwrap();
        let sdl = schema.sdl();
        assert!(!sdl.contains("update_follow_by_pk"), "{sdl}");
        assert!(!sdl.contains("follow_set_input"), "{sdl}");
        assert!(sdl.contains("insert_follow_one("), "{sdl}");
        assert!(sdl.contains("delete_follow_by_pk("), "{sdl}");
        assert!(sdl.contains("update_user_by_pk("), "{sdl}");
    }

    #[test]
    fn snake_case_neighbors_no_longer_collide() {
        // `user_2` vs `user2` collided under PascalCase derivation; verbatim
        // names are injective, so both build
        let schema = build(
            "table user_2 { key id: uuid = auto\n a: int }\n\
             table user2 { key id: uuid = auto\n b: int }",
        )
        .unwrap();
        let sdl = schema.sdl();
        assert!(sdl.contains("type user_2"), "{sdl}");
        assert!(sdl.contains("type user2"), "{sdl}");
    }

    #[test]
    fn reserved_table_names_fail_startup() {
        // `uuid`/`timestamp` never get this far — they are type keywords
        // the parser already rejects as table names (L010)
        for table in ["query", "mutation", "subscription"] {
            let err =
                build(&format!("table {table} {{ key id: uuid = auto\n a: int }}")).unwrap_err();
            assert!(
                matches!(err, SchemaBuildError::ReservedTableName { .. }),
                "{table}: {err}"
            );
        }
    }

    #[test]
    fn support_type_collision_fails_startup() {
        // `user` derives input type `user_insert_input`; a table by that
        // name claims the same type name
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             table user_insert_input { key id: uuid = auto\n b: int }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateTypeName { .. }),
            "{err}"
        );
    }

    #[test]
    fn root_field_collision_fails_startup() {
        // table `user`'s by-pk root is `user_by_pk`; a table named
        // `user_by_pk` claims the same root name for its list field
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             table user_by_pk { key id: uuid = auto\n b: int }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateQueryField { .. }),
            "{err}"
        );
    }

    #[test]
    fn fn_and_record_derivation_laws() {
        let schema = build(
            "table user { key id: uuid = auto\n username: text unique\n bio: text? }\n\
             record stats { posts: int\n latest: timestamp? }\n\
             mut fn rename_user(user: user, username: text, note: text?) -> user ! user_username_taken { unchecked sql(\"UPDATE user SET username = :username, bio = :note WHERE id = :user RETURNING *\") }\n\
             fn find_user(username: text) -> user? { unchecked sql(\"SELECT * FROM user WHERE username = :username\") }\n\
             fn tally(n: int) -> [stats] { unchecked sql(\"SELECT count(*) AS posts, NULL AS latest FROM user LIMIT :n\") }",
        )
        .unwrap();
        let sdl = schema.sdl();
        // polarity decides the root (§7.4): reads on Query, mut on Mutation
        let root_block = |root: &str| -> String {
            sdl.split(&format!("type {root} {{"))
                .nth(1)
                .expect("root type exists")
                .split('}')
                .next()
                .expect("root type closes")
                .to_string()
        };
        assert!(root_block("Query").contains("find_user("), "{sdl}");
        assert!(root_block("Query").contains("tally("), "{sdl}");
        assert!(!root_block("Query").contains("rename_user("), "{sdl}");
        assert!(root_block("Mutation").contains("rename_user("), "{sdl}");
        assert!(!root_block("Mutation").contains("find_user("), "{sdl}");
        // record object type, scalar fields, nullability from `?`
        assert!(sdl.contains("type stats"), "{sdl}");
        assert!(sdl.contains("latest: timestamp"), "{sdl}");
        // fn fields: args (ref param = target key scalar; optional nullable),
        // return per arity
        let rename = sdl_line(&sdl, "rename_user(");
        assert!(rename.contains("user: uuid!"), "{rename}");
        assert!(rename.contains("username: String!"), "{rename}");
        assert!(rename.contains("note: String)"), "{rename}");
        assert!(rename.contains(": user!"), "{rename}");
        let find = sdl_line(&sdl, "find_user(");
        assert!(find.contains("): user"), "{find}");
        assert!(!find.contains("): user!"), "{find}");
        let tally = sdl_line(&sdl, "tally(");
        assert!(tally.contains("): [stats!]!"), "{tally}");
        // description lists declared codes
        assert!(sdl.contains("Errors: user_username_taken."), "{sdl}");
    }

    #[test]
    fn fn_mutation_collision_fails_startup() {
        // a mut fn named like a derived CRUD mutation dies in the claim
        // pass (an unmarked fn of the same name would live on Query)
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             mut fn insert_user_one() -> user { unchecked sql(\"SELECT * FROM user\") }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateMutationField { .. }),
            "{err}"
        );
        // polarity moves the collision surface: a READ fn named like a
        // table's query field dies in the Query claim pass instead
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             fn user() -> [user] { unchecked sql(\"SELECT * FROM user\") }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateQueryField { .. }),
            "{err}"
        );
        // ...and frees the other root: a READ fn named `insert_user_one`
        // no longer collides with the derived mutation
        let schema = build(
            "table user { key id: uuid = auto\n a: int }\n\
             fn insert_user_one() -> [user] { unchecked sql(\"SELECT * FROM user\") }",
        );
        assert!(schema.is_ok(), "{:?}", schema.err());
        // a fn named `update_follow_by_pk` on a PURE-KEY follow builds:
        // the claim pass claims exactly what is registered
        let schema = build(
            "table user { key id: uuid = auto\n a: int }\n\
             table follow { key (follower, target)\n follower: user\n target: user }\n\
             fn update_follow_by_pk() -> [follow] { unchecked sql(\"SELECT * FROM follow\") }",
        );
        assert!(schema.is_ok(), "{:?}", schema.err());
    }

    #[test]
    fn record_collisions_fail_startup() {
        // record named like a reserved root
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             record query { x: int }\n\
             fn q() -> query { unchecked sql(\"SELECT 1 AS x\") }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::ReservedTableName { .. }),
            "{err}"
        );
        // record named like a table's derived input type
        let err = build(
            "table user { key id: uuid = auto\n a: int }\n\
             record user_insert_input { x: int }\n\
             fn q() -> user_insert_input { unchecked sql(\"SELECT 1 AS x\") }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateTypeName { .. }),
            "{err}"
        );
    }

    #[test]
    fn reverse_field_collision_fails_startup() {
        // the reverse collection on user would be `post_by_author`, which
        // user also declares as a column
        let err = build(
            "table user { key id: uuid = auto\n post_by_author: int }\n\
             table post { key id: uuid = auto\n author: user }",
        )
        .unwrap_err();
        assert!(
            matches!(err, SchemaBuildError::DuplicateObjectField { .. }),
            "{err}"
        );
    }
}

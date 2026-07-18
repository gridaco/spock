---
description: The complete Spock error vocabulary — derived, reserved, and product runtime codes, both wire envelopes, and every compile-time diagnostic.
order: 3
---

# Error codes

Spock has two error planes, and they never mix. **Runtime errors** are codes a
server emits on the wire — clients match on them, so they are API, frozen
additively for v0.x. **Compile-time diagnostics** are codes `spock check`
emits at the author — each one names a rule the program broke, with a source
span. This page is the lookup table for both.

## The runtime vocabulary

Every runtime code belongs to exactly one of three populations:

- **Derived errors** are minted from schema constraints — `team_name_taken`
  from a `unique` field — and are never hand-written.
- **Reserved codes** are protocol-owned — `not_found`, `bad_request`, and
  their peers — and mean the same thing in every contract.
- **Product errors** are top-level `error` declarations a program authors,
  raised from fn bodies as [refusals](../language/functions.md).

> **Experimental — RFD 0024 implementation preview.** Top-level `error`
> declarations ship in toolchain 0.5.2 and later as an implementation
> preview. [RFD 0024](../rfd/0024-error-declarations.md) remains a draft;
> this syntax is not yet part of the normative v0 specification.

The 0.5.2 preview replaced implicit refusal-code minting: a fn's `!` clause
no longer creates a code by naming it. A program migrating from an earlier
toolchain adds one top-level `error` declaration per authored refusal, and
renames any refusal called `unauthorized` or `conflict` — those codes are
storage-protocol-owned. The envelopes on the wire, the 409 refusal status,
and rollback behavior are unchanged.

### Derived errors

Schema-derived errors are part of the contract — visible at `GET /~contract`
before any request is made. The v0 specification, §6.1, defines six derived
kinds and one product-rule kind, `refused`:

| Kind | Derived from | Code | REST status |
| --- | --- | --- | --- |
| `key` | the key | `<table>_already_exists` | 409 |
| `unique` | each unique field or group | `<table>_<fields joined by _>_taken` | 409 |
| `required` | each required field that can be absent on insert (no default) or cleared on update (non-key) | `<table>_<field>_required` | 422 |
| `ref_not_found` | each reference field | `<table>_<field>_not_found` | 422 |
| `restricted` | any inbound `restrict` reference | `<table>_restricted` | 409 |
| `invalid` | each closed-set field or `check`-validated field/group | `<table>_<fields joined by _>_invalid` | 422 |
| `refused` | a fn raising a declared product error | the declared code | 409 |

The status split is principled. A 409 (`key`, `unique`, `restricted`,
`refused`) is a conflict with existing state — the same payload could succeed
against a different database. A 422 (`required`, `ref_not_found`, `invalid`)
is intrinsic to the request itself, next to `type_mismatch`.

Watch the derivation on a concrete program:

```spock
table team {
  key id: uuid = auto
  name: text unique
}

table member {
  key id: uuid = auto
  email: text unique
  team: team
  access: "admin" | "editor" | "viewer"
}
```

`spock build` enumerates `member`'s codes in the contract — one per
constraint, statuses included:

```json
"errors": [
  { "code": "member_already_exists",  "kind": "key",           "fields": ["id"],     "status": 409 },
  { "code": "member_email_taken",     "kind": "unique",        "fields": ["email"],  "status": 409 },
  { "code": "member_email_required",  "kind": "required",      "fields": ["email"],  "status": 422 },
  { "code": "member_team_required",   "kind": "required",      "fields": ["team"],   "status": 422 },
  { "code": "member_access_required", "kind": "required",      "fields": ["access"], "status": 422 },
  { "code": "member_team_not_found",  "kind": "ref_not_found", "fields": ["team"],   "status": 422 },
  { "code": "member_access_invalid",  "kind": "invalid",       "fields": ["access"], "status": 422 }
]
```

`team` additionally carries `team_restricted`, because `member.team` points
at it with the default `on delete restrict`; `member` has no inbound
references, so it gets no `restricted` code. `id` never derives a `required`
error — it has a default.

### Reserved codes

Reserved codes are not derived from any declaration and keep one spelling in
every contract:

| Code | Status | Where it appears |
| --- | --- | --- |
| `not_found` | 404 | a by-key read, update, or delete addressing a missing row; a fn declared `-> t` whose SQL matched no row |
| `type_mismatch` | 422 | a value that does not parse as the declared type — in a write body, a filter operand, or an rpc argument |
| `unknown_field` | 422 | a body or filter names a field the table does not declare |
| `bad_request` | 400 | malformed JSON, filter and paging violations, GET on a `mut` fn (as a 405), a foreign-key failure inside escape SQL |
| `internal` | 500 | the fn body broke its declared contract — wrong row arity, a NULL under a non-optional shape — or an unexpected engine failure |
| `unauthorized` | 401 | storage plane only: a missing, malformed, or expired signed-URL signature |
| `conflict` | 409 | storage plane only: a PUT to an object that is not `pending` |

The first five are function-applicable: a fn may list them in its `!`
clause. The storage-only pair may not — those codes belong to the byte plane.

### One collision-free vocabulary

Derived, reserved, and product codes share a single namespace, and the
checker enforces it: two constraints deriving the same code is **E044**, two
declarations of one product error is **E051**, and a product error claiming
a derived or reserved spelling is **E053**. The collision below is real —
the unique field `region_code` and the unique group `(region, code)` both
derive `account_region_code_taken`, because the underscore-join is not
injective:

```spock check=fail:E044
table account {
  key id: uuid = auto
  region: text
  code: text
  region_code: text unique
  unique (region, code)
}
```

The checker refuses the program rather than shipping an ambiguous code —
codes are API, and an ambiguous one cannot be matched on.

## Envelopes

REST carries every failure in one envelope, with the fields of the v0
specification, §8.1 — `code`, `kind`, `table`, `fields`, `message`:

```json
{
  "error": {
    "code": "team_name_taken",
    "kind": "unique",
    "table": "team",
    "fields": ["name"],
    "message": "team.name is already taken"
  }
}
```

GraphQL returns HTTP 200 and carries the same payload — minus the status,
which its transport does not use — in `errors[].extensions`:

```json
{
  "data": null,
  "errors": [
    {
      "message": "team.name is already taken",
      "locations": [{ "line": 1, "column": 12 }],
      "extensions": {
        "code": "team_name_taken",
        "kind": "unique",
        "table": "team",
        "fields": ["name"]
      }
    }
  ]
}
```

A refusal's envelope has `kind: "refused"`, `table: null`, and empty
`fields` — the code itself is the payload. Statuses and the endpoint-level
behavior live on the [HTTP API page](http.md).

## Compile-time diagnostics

Every diagnostic carries a stable code and a source span. E-codes are
program errors from the checker; L-codes are lexical and parse errors. The
v0 specification, §4, is normative; this table is a convenience mirror.

| Code | Condition |
| --- | --- |
| E001 | duplicate table name |
| E002 | duplicate field name within a table |
| E003 | unknown type (not builtin, not a declared table) |
| E005 | table declares no key |
| E006 | table declares more than one key |
| E007 | composite key names an unknown field |
| E008 | key field (inline or composite member) is optional |
| E009 | default incompatible with the field type |
| E010 | reference target table has a composite key (v0 restriction) |
| E011 | unique group names an unknown field |
| E012 | table has no fields |
| E014 | duplicate field within a composite key or unique group |
| E015 | `on delete` on a non-reference field |
| E016 | default on a reference field (`= me` is the one carve-out, E047) |
| E017 | a table's key type resolves through a reference cycle |
| E020 | seed row names an unknown table |
| E021 | seed row names an unknown field |
| E022 | seed row omits a required field that has no default |
| E023 | seed value incompatible with the field type |
| E024 | seed value references an unknown or not-yet-defined binding |
| E025 | seed binding's table does not match the reference target |
| E026 | seed binding used as the value of a non-reference field |
| E027 | duplicate seed binding name |
| E028 | seed row sets the same field twice |
| E030 | duplicate record name |
| E031 | record name collides with a table (one type namespace) |
| E032 | record has no fields |
| E033 | table-only syntax in a record |
| E034 | record field is not a builtin scalar |
| E035 | duplicate fn name |
| E036 | fn parameter type unknown, a record, or a composite-key table |
| E037 | fn return shape is not a declared table or record |
| E038 | duplicate fn parameter name |
| E039 | duplicate error code in one fn `!` clause |
| E040 | `on delete set null` on a required reference |
| E041 | `check` names a fn that does not exist |
| E042 | `check` validator law violated (not a read fn, not `bool`, not a single clauseless `SELECT`, non-deterministic, wrong arity, or attached to a closed-set/`auto`/`now` field) |
| E043 | closed-set type is degenerate (fewer than two values, a duplicate or empty value, or used as a key field) |
| E044 | two derived error codes collide |
| E045 | more than one `auth table` |
| E046 | the `auth table`'s key is not a single scalar column |
| E047 | `= me` is not on a reference to the `auth` table, or no anchor is declared |
| E048 | a user table is named `storage_object` (reserved for the storage builtin) |
| E049 | `file("...")` on a field that does not reference `storage_object` |
| E050 | a `file("...")` seed asset path is absolute or escapes the source directory |
| E051 | two top-level `error` declarations use the same identifier (RFD 0024 preview) |
| E052 | a code in a fn `!` clause is not a declared product error, a schema-derived error, or a function-applicable reserved error (RFD 0024 preview) |
| E053 | a product-error declaration collides with a derived or reserved code (RFD 0024 preview) |

The gaps in the numbering (E004, E013, E018, E019, E029) carry no diagnostic
in v0; the shipped checker emits none of them.

| Code | Condition |
| --- | --- |
| L001 | unexpected character |
| L002 | identifier contains an uppercase letter (identifiers are lowercase `snake_case`) |
| L003 | unterminated string |
| L004 | integer literal does not fit in a signed 64-bit integer |
| L005 | a keyword reserved for a future version used as an identifier |
| L006 | unterminated raw string |
| L010 | parse error — `expected <X>, found <Y>` |
| L011 | dangling doc comment: a `///` that documents nothing |
| L012 | misplaced inner doc: a `//!` after the first declaration |

L001 and L010 are the toolchain's stray-character and unexpected-token
codes; the specification defines the remaining lexical rules in §2 and the
doc-comment placement rules in §4.

# The GraphQL surface — dialect specification

Status: **normative**. This document specifies the GraphQL dialect Spock
derives, independent of what any given toolchain version has implemented.
The v0 runtime implements **Tier 1** (§7); higher tiers are the target.
`docs/spec/v0.md` §8.2 records the v0 protocol binding (mount, page
discipline, error envelope); §9 here records the executed migration from
the pre-dialect surface.

> **0.5.2 implementation preview — experimental, unstable, and
> non-normative.** References below to top-level product-error declarations
> describe the RFD 0024 prototype included in the `0.5.2` toolchain. RFD 0024
> remains `decision: draft` and
> `implementation: unplanned`; the GraphQL shape and envelope are unchanged,
> and this wording has no normative force unless that RFD is accepted and
> implemented.

---

## 1. Mental model

**The schema is a pure function of the contract.** Nobody authors GraphQL
in Spock — not types, not resolvers, not naming. Tables become types,
references become relationships, constraints become error codes, and the
whole thing regenerates on every load. There is no tracking step, no
metadata file, no per-table configuration. This forces one property that
shapes everything below: **every derivation rule must be total** — defined
for all legal contracts, with collisions failing at startup, never at
request time.

**Two write layers, shared roots.** The auto-CRUD mutations specified here
are the *borrowed floor*: generic per-table writes for prototyping
velocity. The *deliberate surface* — named `fn`s carrying product meaning
(`publish`, `follow`, `claim_username`) — lands on the root its polarity
names (§5.1): `mut` fns on the same `Mutation` root (the analogue of
Hasura Actions alongside Hasura's generated mutations), read fns on
`Query` beside the derived list roots. Auto-CRUD is the floor, not the
ceiling; nothing in this document is the reason Spock exists.

**Introspection is the contract's metadata.** The schema is
machine-readable by design — for GraphiQL, for codegen, for LLM agents.
Every generated mutation's description enumerates the derived-error codes
it can produce, so an agent can learn the failure surface without ever
triggering it.

**Governance arrives later and changes visibility, not shape.** v0 derives
one open schema (ungoverned tier, by decision). When `policy` lands, the
same derivation runs per role — fields, rows, and mutations a role cannot
touch simply do not appear in that role's schema (the moral equivalent of
Hasura's permission-filtered schemas). The dialect itself is unchanged by
governance; that is why it can be specified now.

## 2. Dialect commitment

Spock mirrors **Hasura's** GraphQL dialect. Rationale, recorded once: it
is the most widely known auto-CRUD dialect (humans and LLMs have seen more
Hasura-shaped schemas than any other), it is snake_case like Spock, and
the doctrine says to borrow convention for the borrowed layer rather than
invent a bespoke one.

The mirroring rule has three clauses:

1. **Mirror** wherever Hasura specifies a convention (names, arg shapes,
   input-type suffixes, response envelopes).
2. **Define totally** wherever Hasura defers to per-table configuration
   (relationship naming) — Spock has no configuration, so this spec picks
   the rule.
3. **Deviate only with a stated reason**, recorded in the register (§8).

## 3. Naming laws

- **Object type = table name, verbatim.** Table `user` is type `user`;
  `post_media` is type `post_media`. (No case transformation: this is
  Hasura's convention, and it makes type naming injective by construction —
  the language already guarantees unique table names.)
- **Derived input/support types** take Hasura's suffixes:
  `<table>_insert_input`, `<table>_set_input`, `<table>_pk_columns_input`,
  `<table>_mutation_response`, `<table>_bool_exp` (Tier 2),
  `<table>_order_by` (Tier 2).
- **Scalars** are lowercase, Hasura-style: `uuid`, `timestamp` (plus the
  GraphQL builtins `String`, `Int`, `Boolean`, `Float`, `ID`).
- **Open integer conformance gap (Studio feedback S5).** Spock `int` is signed
  64-bit, while GraphQL's built-in `Int` is signed 32-bit and JavaScript
  numbers are exactly integral only through `2^53 - 1`. The current `Int`
  mapping is therefore not a truthful full-width wire. A custom scalar/string
  encoding or a deliberately narrower language type requires a follow-up
  decision; see `crates/spock-runtime/studio/FEEDBACK.md` S5.
- **Field names** are the contract's, verbatim.
- **Reserved table names** (collide with roots or scalars): `query`,
  `mutation`, `subscription`, `uuid`, `timestamp`. A table with one of
  these names — or whose derived support-type names collide with another
  table's — fails at startup.
- **Relationships** (clause-2 territory; Hasura leaves these to config):
  - *forward*: the reference field's own name, resolving to the referenced
    object (`post.author: user!`). Spock has no separate raw-FK scalar
    field — the key is reachable through the object (`author { id }`).
  - *reverse*: `<child>_by_<field>` on the referenced type, resolving to a
    collection (`user.post_by_author: [post!]!`). Total by construction:
    two references from one child to the same table stay distinct
    (`follow_by_follower`, `follow_by_target`). No pluralization —
    English inflection is not a total function.

## 4. Reads

For every table `t`:

| Field | Shape | Semantics |
|---|---|---|
| `Query.<t>` | `(limit: Int, offset: Int°, where°, order_by°): [t!]!` | the list; default page 50, ceiling 200 (§8 deviation D2) |
| `Query.<t>_by_pk` | one non-null arg per key field: `(id: uuid!)`, composite `(follower: uuid!, target: uuid!)` | one row; **miss — including a malformed `uuid`/`timestamp` key value — is `null`** |
| relationships | §3 | forward: object (nullable iff the field is optional); reverse: `[child!]!` with the same list args |

° = Tier 2 (§7). Until then, lists take `limit` only and order by key
ascending; with `order_by` the default ordering remains key-ascending.

## 5. Writes

For every table `t` (single-row tier; bulk in §7):

| Field | Shape | Returns |
|---|---|---|
| `Mutation.insert_<t>_one` | `(object: <t>_insert_input!)` | `t!` |
| `Mutation.update_<t>_by_pk` | `(pk_columns: <t>_pk_columns_input!, _set: <t>_set_input!)` | `t!` |
| `Mutation.delete_<t>_by_pk` | key fields as inline non-null args | `t!` (the row as read before deletion) |

Input types:

- `<t>_insert_input` — every field, **all nullable** (Hasura convention).
  Required-ness is *not* encoded in the input type: it is enforced by the
  contract at runtime, so a missing required field produces the derived
  `<t>_<field>_required` error with its code in `extensions` — the error
  surface stays uniform instead of splitting between GraphQL validation
  and the contract. Defaults apply on omission; on insert, `null` is
  absence (spec v0 §5.1). Consequently, an optional field with a default cannot
  receive SQL `NULL` in the same insert: both omission and `null` apply the
  default. This deliberate v0 limitation is tracked as Studio feedback S4.
- `<t>_set_input` — every **non-key** field, all nullable, with update
  semantics: **absent = keep, explicit `null` = clear** (or the derived
  `required` error on a required field). Key fields never appear in
  `_set`: keys are immutable, the key is the row's identity.
- `<t>_pk_columns_input` — the key fields, non-null.

A table whose every field is a key field derives **no update mutation**:
nothing is settable, and GraphQL forbids empty input objects. Its
`_set_input` / `_pk_columns_input` names stay reserved (§3) regardless.
(Hasura behaves the same way: no updatable columns, no update mutation.)

Write semantics (mirroring spec v0 §7.2):

- a write that did not happen — missing row, malformed key value — is an
  **error** with `extensions.code = "not_found"`, and the by-pk mutations
  return non-null `t!` accordingly (§8 deviation D1: Hasura returns
  nullable);
- an argument bound to a variable the client did not provide is treated
  as **omitted**, per the GraphQL spec, even where a library coerces it to
  `null` (normative);
- top-level mutation fields execute serially; each write is its own
  transaction — an earlier field's committed write survives a later
  field's error;
- deletes check inbound `restrict` references (→ `<t>_restricted`);
  `cascade` and `set null` delegate to the engine (children deleted or
  their reference field nulled, respectively).

### 5.1 Functions — the deliberate surface

Declared `fn`s (spec v0 §7.4) land on the root their **polarity** names
(RFD 0012): unmarked (read) fns are `Query` fields next to the derived
list/by-pk roots; `mut` fns land on the `Mutation` root beside the
derived CRUD — the Hasura-Actions analogue promised in §1. The purity
marker §1 waited for is the *absence* of `mut`, and it is
engine-enforced, not asserted:

- `<Root>.<fn>` — the fn's name, verbatim; one argument per declared
  parameter (the parameter's value type: a table ref binds the target
  key's scalar), nullable iff the parameter is optional. For fn arguments
  `null` means *absent* — there is no `_set` carve-out, so the
  unprovided-variable correction is unnecessary here by construction.
- The return type follows the declared arity: `-> t` is `t!` (a miss is
  an **error**, `not_found` — deviation D1 applies to fns too), `-> t?`
  is `t` (miss = `null`), `-> [t]` is `[t!]!` — **uncapped**: the author
  owns `LIMIT` (a companion to D2, not a deviation from Hasura — the
  page cap governs derived lists, not authored SQL).
- A **scalar return** maps to the builtin's GraphQL scalar under the
  same arity scheme: `-> int` is `Int!`, `-> timestamp?` is `timestamp`,
  `-> [text]` is `[String!]!`. The field is a leaf — no selection set.
- `record` shapes register as object types under the §3 naming laws
  (bare name only; records derive no support types).
- The field description carries the declared error codes (`! a | b`) —
  the failure surface, introspectable before any call. Codes the SQL can
  produce but the signature does not declare still surface truthfully at
  runtime, routed cross-table to the owning table's derived error.
- In the proposed RFD 0024 delta, declared codes include **nominal product
  errors**: a top-level `error ident` owns the product-rule code, and each fn
  that can raise it lists it in `!`. The body raises it through the unchanged
  RFD 0012 refusal path, carried in `errors[].extensions` with
  `kind: "refused"` and `table: null` — distinct names for distinct rules,
  where v1 collapsed every guard into `not_found`.
- **Root names are claimed, per root**: on `Mutation`, derived CRUD
  names and `mut` fn names live in one namespace, and exactly what is
  registered is claimed (a pure-key table claims no update) — a `mut`
  fn named `insert_user_one` fails at startup, never at request time.
  On `Query`, read fn names are claimed next to the derived `<t>` and
  `<t>_by_pk` fields — a read fn named exactly like a table fails the
  same way. Polarity moves the collision surface with the fn.

## 6. Errors

Errors render as GraphQL's own `errors[]` over HTTP 200. The mechanism
mirrors Hasura (machine data in `errors[].extensions`); the **vocabulary
is Spock's** — every contract-derived error carries:

```json
{ "extensions": { "code": "user_username_taken", "kind": "unique",
                  "table": "user", "fields": ["username"] } }
```

Where Hasura says `constraint-violation`, Spock says which constraint, on
which table, over which fields — the code was in the contract (and in the
mutation's description) before any request was made. Reserved
non-derived codes: `not_found`, `type_mismatch`, `unknown_field`,
`bad_request`, `internal`. The program-wide reserved vocabulary proposed by
RFD 0024 also includes storage-plane `unauthorized` and `conflict`; GraphQL does
not emit those two, and a product declaration cannot claim them.

The proposed RFD 0024 product-error registry changes name ownership and
introspection metadata only. A product error raised by a function keeps the
same `kind: "refused"` and extensions shape. The underlying REST/`ApiError`
status remains 409 while GraphQL remains HTTP 200 and omits status; its optional
declaration doc remains contract metadata, not a runtime message.

A value-constraint violation (RFD 0013) carries `kind: "invalid"`, code
`<table>_<field>_invalid`. A **closed-set** field stays a GraphQL
`String` — Spock mints no enum type; the allowed values live in the
generated TypeScript literal union and in the mutation description, and
an off-set value is the `invalid` error above.

## 7. The conformance ladder

- **Tier 1 — single-row core** (the v0 target): everything in §3–§6.
  `_by_pk` / `_one` operations, relationships both directions, `limit`,
  derived errors in extensions. This is exactly the subset of Hasura that
  requires no filter language.
- **Tier 2 — filtered and bulk**. The *read* half **shipped** (RFD 0021):
  `where: <t>_bool_exp` (per-field comparison expressions `_eq _neq _gt _gte
  _lt _lte _in _nin _is_null`, text `_ilike`, combinators `_and _or _not`),
  `order_by: [<t>_order_by!]` (`asc | desc`), and `offset` land on every list
  root and reverse collection. `_like` (case-sensitive) is **refused, not
  offered** — the SQLite floor's `LIKE` is ASCII-case-insensitive and the
  case-sensitive form needs the banned `PRAGMA case_sensitive_like`, so
  advertising `_like` would lie about the floor (RFD 0021 §4). Reference
  fields carry the target's `<target>_bool_exp`, of which v0 accepts the
  key sub-field (folded to an FK comparison); cross-table traversal is a
  reserved node, refused for now (§5). The *write* half — the bulk mutations
  `insert_<t>(objects:)`, `update_<t>(where:, _set:)`, `delete_<t>(where:)`
  returning `<t>_mutation_response { affected_rows: Int!, returning: [t!]! }`
  — is still pending; it builds on the same predicate IR and is gated to the
  REST-writes milestone (keeping Hasura's non-null-`where` anti-footgun wall).
- **Tier 3 — conveniences**: `on_conflict` upsert (blocked on the
  language-level upsert semantics, v1-FEEDBACK L2 — the dialect must not
  back into semantics the language has not decided), `_inc` and update
  operators, aggregates (`<t>_aggregate`).
- **Out of scope** until the doctrine asks for them: subscriptions /
  streaming, Relay connections, `distinct_on`, multi-batch
  `update_<t>_many`.

## 8. Deviation register

Deviations from Hasura, each deliberate:

| # | Deviation | Reason |
|---|---|---|
| D1 | by-pk write-miss is an **error** (`not_found`), return types non-null (Hasura: nullable, `null` on miss) | a write that did not happen must shout; silent-null writes are a known footgun — Spock is a truth-telling prototype tool |
| D2 | lists carry a default page (50) and ceiling (200) (Hasura: unbounded by default) | protocol-level cap is doctrine (v1-FEEDBACK L6) |
| D3 | error codes are derived, specific, and pre-declared (Hasura: generic `constraint-violation`) | derived errors are the product |
| D4 | no configuration layer; relationship names are spec-fixed (§3) (Hasura: console/metadata tracking) | derivation must be total; a prototype language has no metadata step |
| D5 | no raw FK scalar sibling for reference fields (Hasura: `author_id` column + `author` relationship) | Spock's reference field *is* the semantic name; the key is one hop away |
| D6 | query depth bounded (32) (Hasura: unlimited unless configured) | self-references allow unbounded nesting; a prototype runtime ships safe |
| D7 | the builtin `storage_object` table (RFD 0018) is readable/joinable but emits **no** insert/update/delete mutations | files are governed rows, but the storage protocol (v0.md §8.3) is their only write path — floor CRUD would let metadata desync from bytes |

## 9. Migration from the v0.0 surface (executed)

The first v0 runtime shipped a Tier-1-equivalent surface with pre-dialect
naming; it has since been renamed to this specification. The mapping is
recorded (breaking; pre-release):

| v0.0 (as-is) | Tier 1 (to-be) |
|---|---|
| type `User` (PascalCase + collision guards) | type `user` (verbatim; guards removed — injective by construction) |
| scalars `UUID`, `Timestamp` | `uuid`, `timestamp` |
| `Query.user_list(limit:)` | `Query.user(limit:)` |
| `Query.user(id:)` | `Query.user_by_pk(id:)` |
| reverse `post_author_list` | reverse `post_by_author` |
| `create_user(<inline args>)` | `insert_user_one(object: user_insert_input!)` |
| `update_user(id:, <inline args>)` | `update_user_by_pk(pk_columns: {id:}, _set: {…})` |
| `delete_user(id:)` | `delete_user_by_pk(id:)` |
| create's required fields = non-null args (validation-shadowed `required` error) | insert_input all-nullable; `required` is a runtime derived error again |
| reserved: `Mutation` via PascalCase check | reserved: `query`, `mutation`, `subscription`, `uuid`, `timestamp` |

Unchanged by migration: update's absent/null semantics, write-miss-shouts
(D1), the extensions payload, limit caps (D2), serial mutations, depth
bound, GraphiQL on GET, introspection on.

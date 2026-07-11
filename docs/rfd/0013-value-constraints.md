# RFD 0013 — value constraints: validator fns and closed-set types

Status: ACCEPTED (July 2026), implementing now. Resolves the `format`
question RFD 0009 §4 deferred. Two constructs land together: a **closed-set
type** (`status: "pending" | "ready" | "failed"`) for the enum case, and a
**validator fn** referenced by `check` (field-level and cross-column) for
everything else — length, charset, range, non-empty, distinct-pair, ordering.
This revises RFD 0009 §3's ordering — the value tier lands before the filter
RFD — by the roadmap's own rule: **language work is the differentiator**.

## 1. The evidence

The instagram dogfood (`examples/instagram/v0-FEEDBACK.md`) has no way to say
what a value must look like, and the cost is paid three ways:

- **G1 — closed text sets.** Six columns hold a closed vocabulary as open
  `text`: `media.kind`, `media.status`, `follow_request.status`,
  `comment.status`, `report.status`, `notification.kind`. The legal values
  live in comments; the borrowed floor accepts `status: "pnding"`; the
  generated TS says `string`.
- **Floor-leaked guards.** `username` charset/length, `comment.body`
  non-empty and length, `report.reason` non-empty — enforced only inside a
  fn refusal guard, which the auto-CRUD floor bypasses entirely (G13: every
  fn guard is a promise the floor cannot keep). `invalid_username`,
  `empty_body`, `body_too_long`, `empty_reason` are all this.
- **Homeless rules.** `media_tag.x/y` want 0.0–1.0; `media.position ≥ 0`;
  `follower ≠ target` (three self-pair guards: `self_follow`, `self_block`,
  `self_restrict`). None can be said at the table tier at all.

## 2. How we got here — three designs, all rejected

The question was researched with a judged design panel: three independent
designs, three adversarial judges (a doctrine purist, an implementation
pragmatist, a PRD author). The three candidates were:

- **format-first** — a curated vocabulary of named formats (`format(email)`,
  `format(handle, 1..30)`). Rejected: a named format hides its actual rule in
  compiler lore, invisible on the page and absent from the contract; its
  lowering cannot evolve without silently changing accept/reject behavior; and
  arbitrary charsets are inexpressible.
- **check-first** — a raw SQL boolean expression inline on the field
  (`check "length(body) BETWEEN 1 AND 2200"`). Rejected: raw SQL blinds the
  checker (no compile-time seed rejection, no TS unions — both explicit G1
  asks) and the PRD line stops being readable (it relocates a fn's opaque
  guard onto the schema).
- **domain-first** — a named reusable scalar type (`domain handle: text {...}`,
  a rule mini-grammar). Rejected: it buys the whole type-namespace and
  GraphQL-reserved-name collision surface for nominal reuse the dogfood never
  exercises (every closed set appears on exactly one column), and it invents a
  bespoke rule grammar.

The governing principle that killed all three is a new doctrine input, worth
stating as a law:

> **LLM-writability.** spock is written by language models as much as by
> people. A surface must be either **SQL-exact** (a construct the model has
> seen ten thousand times) or **radically simple** (so small there is nothing
> to get wrong). A bespoke rule mini-grammar — `format(handle, 1..30)`,
> `{ length 1..30, charset "a-z0-9._" }` — is out-of-distribution by
> construction: novel syntax the model must be taught, and will spell wrong.

Both winning constructs obey it. A closed-set type is TypeScript's own union
syntax, verbatim. A validator is an ordinary `fn` — a construct the language
already has, already teaches, already tests — whose body is SQL the model
writes fluently. Neither introduces a grammar the model has not already
internalized.

## 3. The shape

```spock
// rules — validators are ordinary read fns, gathered in one section
fn valid_username(name: text) -> bool {
  unchecked sql("SELECT :name NOT GLOB '*[^a-z0-9._]*' AND length(:name) BETWEEN 1 AND 30")
}
fn nonempty(s: text)   -> bool { unchecked sql("SELECT length(:s) > 0") }
fn distinct_pair(a: uuid, b: uuid) -> bool { unchecked sql("SELECT :a <> :b") }

table user {
  key id: uuid = auto
  username: text check valid_username unique
}
table media {
  key id: uuid = auto
  kind: "image" | "video"                              // closed-set type
  status: "pending" | "ready" | "failed" = "pending"
}
table follow {
  key (follower, target)
  follower: user
  target: user on delete cascade
  check (follower, target) distinct_pair               // row check, mirrors unique (…)
}
```

### 3.1 Closed-set types

A closed set is a **text refinement written as a literal union**: `"a" | "b"`.
It is the enum case and only the enum case, because the enum case is the one
where the checker can own the whole story — the values are language-visible, so
a bad seed literal is a compile error (E023, extended), a bad default is a
compile error (E009 — the set *is* the type), and the generated TS is the
literal union, not `string` (G1's exact ask). Storage is TEXT; a
`CHECK (col IN (...))` is derived.

Laws: ≥2 distinct non-empty values (singletons parse and are rejected by the
checker, E043, so the production stays the single syntactic truth); a set type
may not be a key member, a fn param, or a record field (E043/E036/E034) —
forbidding key position is what keeps a set from ever reaching a validator's
param type through a ref. Set members are arbitrary strings and are escaped at
every emission site (SQL single-quote doubling, TS JS-escape); no member
alphabet is imposed.

### 3.2 Validator fns and `check`

Everything that is not a closed set — length, charset, range, non-empty,
cross-column distinctness and ordering — is a **boolean-returning read fn**
referenced by name:

- `field: type check fn_name` — a field check; the fn takes one param, matched
  positionally to the field's value type.
- `check (a, b) fn_name` — a row check (table item, mirrors `unique (a, b)`);
  the fn takes one param per named field.

`check` becomes a full keyword, parallel to `key`/`unique`/`mut`. This is a
source break — `check` is a legal identifier today, in every position (table,
record, fn, param, field, and seed-field names), absent from the §2.3 reserved
list. Recorded in §2.3's break parenthetical alongside `set`/`null`/`float`/
`mut`. **Untaken alternative:** a contextual keyword on the `sql`/`unchecked`
tier, fully decidable with the existing `unique (` two-token lookahead
(`check (` = row check, `check :` = a field literally named `check`,
post-type `check ident` = field check). Rejected for house consistency: the
structural modifiers `key`/`unique`/`mut` are all full keywords, and contextual
status is reserved for the escape markers.

Why a *named* fn and not inline SQL: a name is the deliberate surface (every
rule the program enforces has a name that shows up in the contract, the error,
and the TS), and reusing `fn` means zero new expression positions in the
grammar — no second place raw SQL can appear, no new `unchecked`/ledger ruling.
The cost is a three-line fn per one-liner rule; the payoff is that
`distinct_pair`, `nonempty`, `unit_interval` are each written once and reused
across tables — the nominal-domain reuse the panel's domain-first design wanted,
with no new declaration form.

### 3.3 Validator laws

A referenced fn must exist (E041) and be a validator: an unmarked (read) fn, a
single `SELECT` statement, a bare `bool` return, param count matching the check
arity, param types matching the field value types positionally.

- **Expression discipline.** A validator body is `SELECT <one boolean
  expression over its params>` — no `FROM`/`WHERE`/`JOIN`/`GROUP BY`/`ORDER
  BY`/`LIMIT`, no subquery. This is not a stylistic preference: the body is
  **inline-expanded into a CHECK constraint** (§4), and a clause cannot inline.
  The dogfood's own dominant guard idiom is `SELECT <val> WHERE <cond>` — an
  LLM trained on this repo will write `SELECT 1 WHERE :a <> :b` first — so the
  checker rejects it (E042) and names the rewrite: `SELECT :a <> :b`. The
  discipline is sound to enforce lexically because subqueries are prohibited in
  CHECK anyway; any such token would fail at load regardless. It also
  guarantees exactly one result row, so a validator called as an ordinary read
  fn returns `true`/`false`, never `not_found`.
- **Determinism discipline.** A validator may not reference a non-deterministic
  function. The checker owns this entirely — SQLite gives no reliable backstop:
  `CURRENT_TIMESTAMP`/`CURRENT_DATE`/`CURRENT_TIME` are accepted in a CHECK at
  *both* prepare and insert (a silently wall-clock-dependent constraint), and
  `datetime('now')`/`unixepoch()` fail only at insert as a bare `SQLITE_ERROR`
  the runtime cannot route as `invalid` (it would 500). So a string- and
  comment-aware scan rejects: the bare non-deterministic keywords and functions
  (`CURRENT_TIMESTAMP`/`CURRENT_DATE`/`CURRENT_TIME`, `random`, `randomblob`,
  `spock_now`, `spock_uuid`); a `'now'` literal used *inside* a date/time-family
  call (`date`/`time`/`datetime`/`julianday`/`strftime`/`unixepoch`/`timediff`);
  and a zero-argument date/time call (which defaults to the current time). A
  `'now'` used as ordinary data (`:s <> 'now'`, `IN ('now', …)`) is deterministic
  and left alone. The compile error names the fix.
- **Optional-field binding is allowed.** A param bound to an optional field may
  be NULL; the inlined expression follows SQL tri-valued logic, so a NULL value
  short-circuits the whole CHECK to *pass* — the desired semantics for
  `responded_at >= requested_at` when there is no response yet. A validator
  that must distinguish absence uses `IS [NOT] NULL` and binds a nullable
  param — permitted, not banned. (An earlier draft banned optional params
  outright; that contradicted the conditional-presence coverage this RFD
  claims. The ban is dropped.)
- **GLOB, not LIKE.** `LIKE` is case-insensitive by default and mutable by
  `PRAGMA case_sensitive_like`; `GLOB` is case-sensitive and stable. Validators
  use GLOB. (The same prepare-time PRAGMA hazard that RFD 0012's statement
  allow-list closed lives here too — a validator inheriting connection LIKE
  state would be non-local. GLOB has no such state.)
- A `check` may not attach to a set-typed field (the set self-validates) nor to
  a field defaulted `= auto`/`= now` (E042): engine-minted values vary per
  insert, so no static default-vs-check proof (§4) can exist, and there is no
  demand.
- Resolution is **file-global**: a `check` may reference a validator declared
  anywhere in the file. The dogfood convention gathers validators in one
  labeled `// rules` section; the checker validates the reference after the fn
  list and the ref-key types are both resolved.

## 4. Lowering, routing, and the corrected engine premise

Both constructs lower to a **named** SQLite CHECK constraint whose name **is**
the derived error code:

```sql
CONSTRAINT "media_status_invalid"  CHECK ("status" IN ('pending','ready','failed'))
CONSTRAINT "user_username_invalid" CHECK ("username" NOT GLOB '*[^a-z0-9._]*' AND length("username") BETWEEN 1 AND 30)
CONSTRAINT "follow_follower_target_invalid" CHECK ("follower" <> "target")
```

A validator lowers by **inline expansion**: strip the body's leading `SELECT`,
substitute each `:param` for its quoted column name (token-aware, longest-match,
never inside a quoted span), wrap in `CONSTRAINT <code> CHECK (...)`. SQLite has
no `CREATE FUNCTION` in SQL, so inlining is the only lowering that keeps the
`.db` pure SQL — raw third-party access (Python `sqlite3`, `datasette`) keeps
working, which registering a per-validator UDF would break.

**The routing channel is the constraint name.** A named CHECK fails with
exactly `CHECK constraint failed: <name>` (probed: identical for INSERT and
UPDATE, extended code SQLITE_CONSTRAINT_CHECK); the message carries no table or
column, so the name is the whole channel. New `SQLITE_CONSTRAINT_CHECK` arms
strip the prefix and match the code against `table.errors` (the table-in-hand
write path) or `contract.tables` (the fn-escape path) — no prebuilt map, no
new signatures. A violation inside any escape body auto-routes with zero fn
declarations: **the un-collapse.**

**Corrected premise (this is where the plan's first draft was wrong).** The
inline-expansion approach was pitched on "the engine polices validator purity
at load." Probed empirically, that is only partly true:

- Subqueries and aggregates in a CHECK are rejected at **DDL-prepare** (load).
  Good — the expression discipline is belt-and-suspenders here.
- An **unknown function** is rejected at DDL-prepare. Good.
- A **double-quoted identifier that is not a column** is a hard prepare error
  (no DQS leniency inside CHECK). Good — a mistyped column in a validator fails
  at load, not silently.
- A **non-deterministic** function is **not** rejected at DDL-prepare, and the
  engine gives no usable backstop: `random()` and `CURRENT_TIMESTAMP` are
  silently accepted at both prepare and insert (a wall-clock-dependent
  constraint ships), while `datetime('now')`/`unixepoch()` are rejected only at
  the first INSERT and only as a bare `SQLITE_ERROR` — not `SQLITE_CONSTRAINT_CHECK`
  — so the runtime cannot route it as `invalid` and would surface a 500. So
  determinism cannot be left to the engine at all — hence the checker's
  determinism scan (§3.3) owns it wholly.

Two further load-time proofs the engine does not give for free, added to
`spock check` and `spock run` alike (both open the engine):

- **Duplicate constraint names.** SQLite silently accepts two constraints with
  the same name in one table. The derived template `<table>_<fields>_invalid`
  is a non-injective underscore-join (a field named `a_b` and a row check on
  `(a, b)` collide). A checker claim-pass — the `graphql.rs` claimed-names
  pattern, moved to compile time — rejects any collision across **all** derived
  codes (`_taken`/`_required`/`_not_found`/`_invalid`), naming both sites. This
  retroactively hardens the pre-existing `_taken` join ambiguity too.
- **Default vs check.** SQLite does not evaluate a DEFAULT against a CHECK until
  a row exercises it. A literal default that violates its own field's validator
  would ship silently and 422 every insert that omits the field (the generated
  insert type marks it optional). The load runs `SELECT <inlined expr with the
  default substituted>` once per validator-checked literal-default field and
  fails the load, naming field + validator + default.

## 5. Errors, and the specificity cost

A violation is kind **`invalid`**, status **422** — not 409. The distinction
is real: a 409 (key/unique/restricted) is a conflict with *existing state* —
the same payload could succeed against a different database. A value-constraint
violation is intrinsic to the payload: no state makes `""` a legal body. That
is the `required`/`type_mismatch` family. The envelope and GraphQL extensions
keep the frozen §8.1 shape `{code, kind: "invalid", table, fields}`; the
**message** names what failed — a set lists its values, a validator names its
fn: `media.status must be one of: pending, ready, failed`; `comment.body failed
check valid_body`; `follow (follower, target) failed check distinct_pair`. A
client that wants the validator programmatically reads `Field.check` /
`Table.checks` from the contract (keyed by table + fields) — so the envelope
gains no field, and its shape stays frozen.

**The cost, recorded honestly.** One `check` per field plus name-only routing
means a validator that bundles two rules (`valid_username` is charset AND
length; a `body` validator would be non-empty AND ≤2200) produces one opaque
code — the dogfood's `empty_body` and `body_too_long`, two client-
distinguishable refusals today, collapse into one `comment_body_invalid`. A UI
can no longer render "too long" vs "empty" from the code alone. This is the
price of engine-level enforcement across the floor, and it is not forced: an
author who needs per-rule client-distinguishable errors keeps the fn refusals
(the mechanism is not retired — the sweep merely stops using refusals where a
check now compensates). The message names the validator and the contract's
`Field.check` lets a client find it; distinguishing *which conjunct* failed is
what refusals are still for.

## 6. What this deliberately does not do

- **Curated named formats (`email`, `url`, `slug`).** They stay out until a
  vocabulary-versioning story exists (tightening `email`'s pattern silently
  changes accept/reject behavior across compiler versions). If they ever land,
  they land as **visible stdlib validator fns** shipped in the language —
  never as compiler-owned lowerings. The mechanism is already here: `fn
  valid_email(s: text) -> bool { ... }` needs no new surface.
- **Cross-row rules — partial/conditional uniqueness (G6).** That is the unique
  tier (v1-FEEDBACK L1), not a value constraint.
- **Cross-table / state invariants (G14, `blocked`, `account_private`).** A
  CHECK is row-local (subqueries prohibited). These stay fn refusals now,
  `expect`/`policy` in v1.
- **Nominal `domain` declarations.** Deferred: the dogfood reuses validators by
  name already (a fn *is* the reusable unit); a named scalar *type* buys the
  type-namespace collision surface for no exercised demand. If demand appears,
  it is additive — a `domain` and a validator fn lower to the same CHECK.
- **Sub-rule error specificity within one check** (§5) — recorded cost, not a
  goal.

## 7. Contract mechanics (§6 freeze discipline)

All additive — old contract JSON loads in new consumers:

- A field's `type` gains a `{"kind":"set","values":[...]}` variant. Consumers
  reject unknown `kind`s rather than guess (the existing §6 posture), so this
  is additive under the same rule that admits new error kinds.
- `Field.check: Option<String>` (validator fn name), `#[serde(default)]`.
- `Table.checks: [{fields, fn}]`, `#[serde(default)]`, parallel to `uniques`.
- `errors[]` gains kind-`invalid` entries; `ErrorKind` gains `Invalid`.

TS emission (`spock gen types`): set types become literal unions in the row and
insert/update input types (members JS-escaped); checked fields gain a JSDoc
line; the per-table error union grows generically. GraphQL: scalars stay
`String` (Tier-1 fidelity — no minted enums, no reserved-name surface);
`invalid` joins the insert/update mutation-description predicates
reachability-exactly (an all-key row check cannot fire on update, so it is not
advertised there).

## 8. The doctrine line

fn v2 proved the contract could reach into the escape and stay derived. The
value tier proves the escape can reach back *out*: a validator's opaque SQL,
inline-expanded, becomes a named constraint the whole floor obeys — and its
violation names itself with a derived code the language never improvised. The
enum case, where the language *can* see the values, is owned end to end by the
checker; the open cases, where it cannot, are enforced by the engine and routed
by a name the checker mints. Neither construct adds a grammar a model has not
already learned. The escape may replace the body, never the contract — and now
the contract can borrow the escape's reach without borrowing its opacity.

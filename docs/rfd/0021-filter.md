# RFD 0021 — the filter sub-language: one predicate IR, two borrowed frontends

Status: **ACCEPTED — read half implemented in v0.** The predicate IR
(`crates/spock-runtime/src/filter.rs`), both frontends (Hasura `bool_exp` on
`/graphql/v1`, PostgREST operators on `/rest/v1`), ordering with the forced
stable total order, and paging all shipped and are exercised end to end by the
technical fixture `examples/filter-lab/` (see its FEEDBACK.md) plus the
graphql/http suites. `docs/spec/graphql.md` §7 is reconciled (`_like` struck).
The open decisions of §14 were resolved as recommended (offset window ceiling
10 000; `STRICT` tables deferred; shallow key-traversal ref filter shipped;
unknown-operator folded into `bad_request`). Still deferred, on this exact IR:
filtered/bulk **writes** (the REST-writes milestone) and v1 `policy` (the
reserved `Operand::Actor` / `Exists` seams, §11). This ratifies the
filter-dialect recommendation RFD 0009 §4 recorded "until ratified"; it
discharges the pagination/cursor debt fn v2 deferred here (RFD 0012 §3), and it
deliberately shapes its predicate IR as the structural dry-run for v1
`policy`/RLS.

The filter sub-language is the fifth thing the roadmap has called "next" and the
first to actually land the query layer. It has one job, seen from three angles:
give every row-returning surface **a predicate, a page, and a stable order** —
and give the author, for all three, nothing to hand-write.

## 1. The evidence

Two dogfood findings (`examples/instagram/v0-FEEDBACK.md`) are this RFD's brief,
and they draw its two halves.

- **G13 — the floor leaks what the fns hide.** `hashtag_page` excludes archived
  posts and private authors; `GET /rest/v1/post` serves them all. "Every
  fn-level guard in this file is a promise the borrowed floor cannot keep,
  because the floor cannot filter." The finding's own directive: it "ratifies
  the filter sub-language as the roadmap's next language work — and says its
  scope must include *policy* filtering (row visibility), not just query
  predicates." RFD 0013 closed the *value* half of that leak (bad username,
  empty body, self-follow → engine constraints). The *visibility/state* half —
  archived-excluded, blocked-excluded, private-only — is cross-row and "stays
  the filter/policy layer's job (a row-local CHECK cannot see it)." So the
  predicate this RFD designs is, by the dogfood's explicit instruction, the same
  predicate v1 `policy` will AND into every read. Design it once.

- **G16 — every fn read re-solves solved problems, and gets them wrong.**
  Adversarial review of the dogfood's first draft found three read-path defects
  in one pass: a cursored read ordered by a non-unique timestamp with no
  tiebreaker (boundary ties silently skipped); two discovery pages with a `LIMIT`
  but no cursor at all (rows past the newest 30 unreachable); and a search that
  concatenated user input into `LIKE` unescaped (`%`/`_` as a user-controlled
  wildcard language). "All three are the same finding: pagination discipline,
  stable ordering, and pattern escaping are *protocol* problems being
  hand-solved per fn in an unverifiable body." The directive: "whatever read
  sub-language emerges should own page shape (cursor, ordering, ceiling) the way
  §8 already owns it for the floor, leaving the fn author only the predicate."

Both findings say the same thing. The floor is limit-only (`Query.<t>(limit:)`,
key-ascending, ceiling 200 — graphql.md §4); the deliberate surface hand-rolls
the rest and gets it wrong. The instagram v0 program has a dozen read fns (G11),
most returning lists — `home_feed`, `notifications`, `hashtag_page`, and the
discovery/search pages — every one of them uncapped, unfilterable from outside,
and re-deriving the same three protocol concerns in opaque SQL. This RFD makes those three
concerns — **the predicate, the page, the order** — protocol, and leaves the
predicate the only thing anyone writes.

## 2. The doctrine that shapes it

Three laws already on the books decide most of the design before it starts.

**Borrow before build (RFD 0009).** The read surface mirrors an existing,
widely-known dialect rather than inventing one. There are two frontends because
there are two floors — GraphQL borrows **Hasura's `bool_exp`**, REST borrows
**PostgREST's operators** — and exactly **one owned IR** they both lower into.
The IR is Spock's; the two surfaces are not. There is no third grammar, and in
particular there is **no filter syntax in the `.spock` language** — a filter is
a per-request protocol artifact, never author source. The only filter-shaped
thing that will ever be authored is a v1 `policy` predicate (§11), and it is the
one place the next law binds.

**LLM-writability (RFD 0013 §2).** "A surface must be either SQL-exact ... or
radically simple." Every v0 operator name is a token an LLM has already seen ten
thousand times in Hasura or PostgREST; nothing is coined. This law is also why
the Postgres-only operator tail is **refused, not faked** (§4): advertising an
operator the SQLite floor cannot honor is a lie about production, and a lie the
model would learn to write. Where the borrow cannot be honored, the refusal is
specific and names the substitute — a refusal the model can self-correct from is
part of writability, a bare "unknown operator" is not.

**Honest about production, and the page cap (D2, v1-FEEDBACK L6).** Derived
lists carry a default page (50) and a ceiling (200); this RFD extends the same
cap to every filtered surface and to `order`/`offset`. And it states plainly
where offset lies (§7): a prototype must not sell a stable cursor it does not
have.

## 3. The predicate IR — the owned core

**Placement.** The predicate is per-request protocol state; it lives in the
runtime (`crates/spock-runtime/src/filter.rs`, new), **not** the contract IR.
The contract IR is a serde-normative interchange artifact — "Everything a runtime
or tool needs is in `Contract`; nothing refers back to source text"
(`crates/spock-lang/src/ir.rs:1-4`). Request-shaped state has no place in it. In
v0 the filter touches **no** `spock-lang` type and owes **no** legacy-JSON
regression; the first and only `spock-lang` change comes in v1, when `policy`
adds an authored predicate as a new additive `Table` field (§11, §13).

**The tree.** One recursive node, mirroring `bool_exp` and PostgREST's group
grammar, which are the same shape:

```rust
// crates/spock-runtime/src/filter.rs
enum Predicate {
    And(Vec<Predicate>),          // empty ⇒ TRUE   (§8)
    Or(Vec<Predicate>),           // empty ⇒ FALSE  (§8)
    Not(Box<Predicate>),
    Cmp { col: ColRef, op: CmpOp, value: Operand },
    IsNull { col: ColRef, negated: bool },
    Exists { rel: RelRef, inner: Box<Predicate>, negated: bool },  // RESERVED — §5, §11
}

enum Operand {
    Lit(SqlValue),                // v0: always a bound parameter
    // Actor,  ─┐  RESERVED, not constructed in v0 — the claims binding a v1
    //          │  policy leaf resolves per-request (spock_actor(), RFD 0014).
    //          └─ "the same tree with one more binding."
}
```

Two reservations carry the whole v1 story at zero v0 cost, and they are the
reason this shape earns its keep over a flatter one:

- **`Operand` is leaf-parametric.** v0 populates only `Lit`. Naming `Actor`
  without constructing it is the literal encoding of "the same tree with one
  more binding" — a policy is a `Predicate` whose leaves may reference the
  actor's claims, resolved per request into `Lit`s, yielding a tree that lowers
  through the identical path.
- **`Exists` is reserved, un-emitted.** Cross-table reach ("this row belongs to
  my org") is the one leaf that "cannot be bolted onto the leaf machinery cheaply
  later" — Hasura keeps `_exists` permission-only for exactly this reason. v0
  decides its *shape* now and defers its *lowering* (§5): the frontends refuse
  any nested-relationship key with `bad_request`, so the node exists in the IR
  but never in a v0 query.

**The single composer.** One function lowers a `Predicate` (+ order + page) to
`WHERE ... ORDER BY ... LIMIT ... OFFSET`. Both read floors funnel through it:
GraphQL `query_rows` (`graphql.rs:529`, whose ad-hoc single-column filter param
this replaces) and the REST `list_rows` path. This is not merely tidy — SQLite
has **no RLS backstop**, so in v1 this composer *is* the policy engine (Postgres
ANDs the policy in-engine; Spock ANDs it in the query builder). Any row-returning
read path added outside the composer is, in v1, a silent policy bypass. The
composer is therefore the sole derived-read chokepoint by construction — with one
named, honest exception (§7: authored read fns).

## 4. The v0 operator vocabulary

The closed set — every entry present in **both** borrowed dialects and lowering
1:1 to SQLite with no Postgres dependency:

| IR | Hasura | PostgREST | SQLite | notes |
|---|---|---|---|---|
| `Eq` | `_eq` | `eq` | `"c" = ?` | `null` value forbidden → `IsNull` (§8) |
| `Neq` | `_neq` | `neq` | `"c" <> ?` | excludes NULL rows (3VL) |
| `Gt`/`Gte`/`Lt`/`Lte` | `_gt`/`_gte`/`_lt`/`_lte` | `gt`/`gte`/`lt`/`lte` | `"c" > ?` … | |
| `In` | `_in` | `in.(…)` | `"c" IN (?,…)` | empty ⇒ `FALSE` |
| `Nin` | `_nin` | `not.in.(…)` | `"c" NOT IN (?,…)` | empty ⇒ `TRUE`; NULL in list forbidden (§8) |
| `IsNull` | `_is_null: true\|false` | `is.null` / `not.is.null` | `"c" IS [NOT] NULL` | the sole null surface |
| `Ilike` | `_ilike` | `ilike` | `"c" LIKE ? ESCAPE '\'` | ASCII case-insensitive; `_like` refused (below) |
| `And` | `_and: [ ]` | `and=(…)` / implicit | `(A AND B)` | object form refused (§5) |
| `Or` | `_or: [ ]` | `or=(…)` | `(A OR B)` | object form refused (§5) |
| `Not` | `_not: { }` | `not.` prefix | `NOT (A)` | |
| boolean eq | `_eq: true\|false` | `is.true` / `is.false` | `"c" = 1` / `"c" = 0` | bool stores INTEGER (§9) |

**`_like` is refused, on purpose.** graphql.md §7 currently lists `_like _ilike`;
this RFD ships `_ilike` only and reconciles the spec (§12). The reason is
soundness, not scope: SQLite's `LIKE` is ASCII-case-*insensitive* and can only be
made case-sensitive through `PRAGMA case_sensitive_like`, the same connection-
global prepare-time hazard RFD 0013 closed for validators — so a `_like` built on
`LIKE` would be a case-*insensitive* operator wearing a case-sensitive name, and a
`_like` faked with `GLOB` lies harder still (different wildcards, no `ESCAPE`,
no literal `[`). Both are lies about the floor. `_like` (and an honest `_glob`)
wait for either a real Postgres backend or a decided case-fold story. The refusal
is a specific `bad_request` naming `_ilike` as the case-insensitive substitute,
surfaced in `/~contract` so codegen sees `_like` is absent by design.

**The refused tail — honest, not silent.** Every other operator in the two
dialects is a loud `bad_request` in v0, and the `CmpOp` enum stays extensible so
a bought backend can fill them in: the pattern/regex family (`_like`/`_nlike`/
`_similar`/`_regex*`, PostgREST `match`/`imatch`), full-text (`fts`/`plfts`/
`phfts`/`wfts`), JSONB containment (`_contains`/`cs`/`cd`), PostGIS `_st_*`, range
(`ov`/`sl`/`sr`/`nxr`/`nxl`/`adj`), the `any`/`all` modifiers, and `isdistinct`
(clean SQLite `IS DISTINCT FROM`, but *no Hasura twin* — omitted to keep the two
frontends symmetric; a REST-only operator would be its own small asymmetry, and
the demand is zero). Refusing beats faking: "a prototype language must refuse
rather than fake."

## 5. Frontend A — Hasura `bool_exp` (GraphQL)

For every table `t`, derive `<t>_bool_exp` and `<t>_order_by` input types and
attach `where`, `order_by`, and `offset` to `Query.<t>`, to every reverse
collection (`<t>_by_<field>` — which already calls `query_rows`, so it inherits
filtering for free), and (later) to the bulk mutations. Naming and suffixes are
graphql.md §3's, already reserved.

- A predicate is `{ column: { _op: value } }`; multiple keys in one object are
  implicit `AND`. `_and`/`_or` take **arrays**; `_not` takes one object.
- **The `_or`/`_and` object form is refused.** Hasura's engine quietly degrades
  `_or: { a, b }` (object, not array) to `AND` — a bug-for-bug quirk that would
  make `_or` mean `AND` on GraphQL while `or=(…)` means `OR` on REST. Requiring
  the array (object form → `bad_request`) is a deliberate deviation (§12) that
  buys cross-frontend semantic identity, which matters more than quirk parity.
- **Closed-set fields** (RFD 0013) stay GraphQL `String` (no minted enum —
  Tier-1 fidelity); `_eq`/`_neq`/`_in`/`_nin`/`_is_null` are the meaningful
  operators, and operands are validated against the set's members at parse (§9).
- **Booleans** filter through `_eq: true|false` → `= 1`/`= 0` (§9); `_is_null`
  covers the third state.

**Reference fields — the one place the borrow needs a ruling, decided now.**
Spock has no raw FK scalar sibling (deviation D5): the field `post.author` is the
`user` *object* on output, with the key one hop away (`author { id }`). A
`bool_exp` input field can carry exactly one input type, so this cannot be typed
scalar today and relationship-shaped tomorrow without a breaking schema change.
The ruling: **a ref field in `<t>_bool_exp` is typed as the target's
`<target>_bool_exp`**, from day one — the faithful Hasura spelling for a
relationship, and forward-compatible because the field's type never changes.

```graphql
# v0 — key sub-field folds to a direct FK comparison, no EXISTS:
where: { author: { id: { _eq: $me } } }        #  ⇒  "author" = ?
# v1 — non-key sub-field is the reserved Exists traversal, refused in v0:
where: { author: { verified: { _eq: true } } } #  ⇒  bad_request (v0)
```

Because a ref targets a single-column key (composite-key targets are already
disallowed), filtering the relationship by *its key* reduces to a comparison on
the parent's own FK column — no correlated subquery, no join, exactly the
"flat FK equality" real apps need — while any *non-key* column is genuine
traversal and lowers (in v1) to `EXISTS (SELECT 1 FROM user u WHERE u.id =
post.author AND <inner>)`, never a join (join multiplies parent rows), bounded by
the existing depth cap D6 (32). The `Exists` node is thus reserved on the *same
field type* the flat case already uses: v0 ships the FK case, v1 lights up the
traversal, and no client's `where` breaks across the seam.

## 6. Frontend B — PostgREST operators (REST)

The REST floor borrows `?column=operator.value`. Multiple params `AND`; explicit
logic is `and=(…)`, `or=(…)`, and a `not.` prefix on an operator (`?a=not.eq.2`)
or a group (`?not.and=(…)`), nesting arbitrarily. All of it parses into the same
`Predicate` — the meeting point of the two dialects.

- **`is` is faithful to PostgREST:** `is.null` → `IS NULL`, `not.is.null` →
  `IS NOT NULL` (composed from the `Not` node — *not* an invented `is.not_null`,
  which PostgREST has no such spelling for), `is.true` → `= 1`, `is.false` →
  `= 0`, `is.unknown` → `IS NULL`. This is the idiomatic REST boolean filter; a
  PostgREST-fluent client that writes `?active=is.true` must not hit a wall.
- **`*` aliases `%` in the REST `ilike` skin only** (PostgREST's own convention,
  to dodge URL-encoding), recorded as a deviation (§12): a literal `*` is
  therefore unmatchable via REST `ilike` — exactly as upstream PostgREST — while
  Hasura `_ilike` does not alias. This is the one cross-frontend expressibility
  asymmetry, named rather than hidden.
- **Reference fields** filter as `?author=eq.$me` — the FK scalar column exists
  in the row, so REST addresses it directly, lowering to the *same* `"author" =
  ?` as the GraphQL key-sub-field case. Embedded-resource traversal
  (`?author.verified=eq.true`) is the reserved `Exists` → `bad_request` in v0.
- **Ordering/paging** are `?order=col.asc,col2.desc`, `?limit=`, `?offset=`
  (§7). A REST **response** carries `Content-Range` for parity; the `Range`
  *request* header is not ingested in v0 (query params are the LLM-obvious path).

Two REST-specific plumbing obligations, both startup-total (never request-time):
the parser is a real **tokenizer** honoring PostgREST's reserved-char quoting
(`%22`-quoted values, escapes inside `in.(…)`), never split-on-comma; and the
**reserved control keys** guarded against real column names at load are the
*exact* set the parser consumes — `order`, `limit`, `offset`, `select`, **and
`and`, `or`, `not`** (a column literally named `or` must fail loudly at startup,
not shadow a logical group at request time), extending the existing
`ReservedRestSegment` guard (`http.rs`).

Both frontends produce byte-identical `Predicate`s for the scalar core; the two
named asymmetries (`isdistinct` REST-absent by omission, `*`-aliasing REST-only)
are the register, and there is nothing else.

## 7. Ordering and pagination — the part this RFD owns

This is the debt fn v2 assigned here (RFD 0012 §3): "pagination discipline
belongs to the universal dynamic query layer ... applied uniformly to tables,
future views, and row-returning fns." Discharged as follows.

**Ordering.** `order_by` is an ordered list of `{ col: asc | desc }`. The
nulls-placement variants (`asc_nulls_first`/…) are deferred (§12) — but the
lowering **always emits explicit `NULLS`**: `NULLS LAST` for `asc`, `NULLS FIRST`
for `desc`, matching Hasura/Postgres, because SQLite's *implicit* null placement
is inverted and must never be inherited. Columns are validated against the
declared field set before quoting.

**The one invariant Spock forces: a stable total order.** Every derived
row-returning surface appends the **primary key as the terminal `ORDER BY` term**,
whether or not the client ordered — because a non-unique sort key lets the engine
silently skip or duplicate rows across pages, which is textbook slop. The pk is
appended **in the direction of the last user sort term** (a `desc` query tiebreaks
`pk DESC`), so the total order stays single-direction — this costs nothing for
offset and keeps a future keyset seek expressible. Neither Hasura nor PostgREST
forces this; it is the honest move and the exact invariant v1 policy and any later
cursor layer will also require.

**The page, capped server-side (D2).** `limit` defaults to 50 and clamps to the
200 ceiling; `offset` is a validated non-negative integer; both bind as
parameters. The cap is injected in the lowering, uniformly across GraphQL, REST,
and reverse collections, and both floors route limit/offset validation through
the one `ApiError` envelope (reconciling today's asymmetry — GraphQL's explicit
`read_limit` error vs REST's raw parse failure).

**Offset is honest about its two costs.** Forcing a stable total order kills
tie-nondeterminism but does **not** remove offset's insert/delete *page drift*
(inherent to skip-take), and offset is **O(n) in depth** — SQLite walks and
discards every skipped row while holding the single serialized DB lock, so a deep
offset against a large filtered set is a multi-second stall and a trivial
single-connection DoS. The page-*size* cap does nothing for page *depth*. v0
therefore also caps **paging depth** (an `offset` ceiling; the exact value is an
open decision, §14) and states plainly: drift-sensitive or deep reads want a
keyset cursor, which is deferred.

**Keyset is not productized in v0 — and the RFD will not pretend otherwise.**
The honest reason is structural: a sound keyset seek is a *row-value* comparison,
`(sort_col, pk) > (:last, :lastpk)`, and (a) the flat `Predicate` has no tuple
node to emit it, and (b) **neither borrowed dialect has a tuple-comparison
surface** — keyset in both Hasura and PostgREST is hand-built `where + order_by +
limit`. A client *can* compose the correct compound predicate with the shipped
operators — `_or: [ {k:{_gt:$k}}, {_and:[ {k:{_eq:$k}}, {pk:{_gt:$pk}} ]} ]` —
and v0 does nothing to stop it, but v0 does **not** advertise a bare
`{k:{_gt:$last}}` as a safe cursor, because on a non-unique key it silently skips
the tie rows straddling the page boundary (the exact G16 bug). Productized keyset
— whether a real cursor parameter or a tuple IR node with a decided surface —
waits for a later tier; the forced total order laid down now is precisely the
substrate it will need, so it lands additively.

**Read fns are the one honest exception to "every surface."** The universal layer
composes `WHERE`/`ORDER BY`/`LIMIT` for surfaces the composer *builds*. An
authored read fn's body is opaque SQL executed as written — Spock cannot inject a
tiebreaker into a SELECT that may carry its own `ORDER BY`, a `GROUP BY`, or a
`UNION` — and the fn v2 boundary already says the author owns `LIMIT` there
(graphql.md §5.1). So this RFD narrows fn v2's punt precisely: the universal layer
governs **derived** surfaces (tables, reverse collections, and `view` when it
lands); a row-returning **read fn** stays author-owned for order, page, *and*
stability. This is not a gap so much as the reason `view` exists — the deliberate
read-side sibling of `fn` (RFD 0009 §10) is composer-built, so it inherits the
universal layer, where an opaque fn body cannot. The honest line: a read fn that
wants a governed page should become a `view`.

## 8. Safe lowering — the injection-proof, NULL-correct, deterministic checklist

Injection safety here is **structural, not escaping-based**. The lowering obeys:

1. **Values are always bound `?` parameters** (`rusqlite::params_from_iter` for
   variable-length `IN` lists). No value is ever formatted into SQL; the only
   length-variable emission is the *count* of `?` in an `IN` list.
2. **Operators map through a fixed match arm** from the closed `CmpOp` enum to SQL
   tokens. No client operator string ever reaches SQL.
3. **Columns are resolved against the declared field set before lowering** (an
   unknown column is a refusal, never emitted), then double-quoted with any
   embedded `"` doubled.
4. **DQS off:** assert `SQLITE_DBCONFIG_DQS_DDL`/`DML` disabled at open, so a
   quoted identifier can never silently degrade into a string literal.
5. **The NULL law (three-valued logic).** `null` is forbidden as a `Cmp` value at
   build time (route to `IsNull`) — `= NULL` is always NULL/no-match; a NULL
   inside an `in`/`nin` list is rejected (`NOT IN (…, NULL)` silently returns zero
   rows); `IsNull` is the sole null surface. These match Hasura's `_is_null` and
   Postgres RLS, so filter, cursor, and policy agree with SQL.
6. **Empty-set canonicalization:** `In []` → constant `FALSE`, `Nin []` → `TRUE`,
   empty `And` → `TRUE`, empty `Or` → `FALSE` (SQLite `IN ()` is a syntax error).
7. **Bound-parameter budget is tree-global.** The per-`IN`-list guard against
   `SQLITE_LIMIT_VARIABLE_NUMBER` (~32 766) is not enough — a wide, shallow
   `_or` of many small `IN` lists can exceed the connection variable limit in
   aggregate and fail at `prepare()` as a raw 500. v0 counts total bound params
   across the whole lowered predicate (plus limit/offset) and refuses over-budget
   with `bad_request`, and caps combinator-array **breadth** (the depth cap D6
   bounds nesting, not breadth).
8. **Never `PRAGMA case_sensitive_like`** (echoing RFD 0013): `_ilike` is
   deterministic *because* the connection's LIKE state is never touched.
9. **Bare column vs bound param** — never wrap the column in `CAST`/functions —
   so the declared affinity governs coercion and the predicate stays
   index-sargable.
10. **One db-lock scope; no `await` while held** (spec §7.2), so a filtered read
    cannot interleave and stall the single connection.

**A recorded substrate assumption.** Range and ordered comparisons on
TEXT-stored `uuid`/`timestamp` are correct only because those values are stored
canonically and thus sort lexically in value order (timestamps canonical per
v0.md; keys UUIDv7). The floor write path enforces this, but a `mut fn` escape
can `INSERT` a non-canonical timestamp and silently mis-order a range or page
with no error. v0 does not emit `STRICT` tables (`ddl.rs`), so the honest scope
is: **lexical-order-equals-value-order holds for floor-written data**; whether to
adopt `STRICT` tables + canonicalizing constraints and extend the guarantee to
escape-written data is an open decision (§14).

## 9. The value tier, references, and `storage_object`

Operator legality and value coercion are computed off `storage_type()`
(`ir.rs:434`), not the surface kind:

- **Booleans** store as `INTEGER`, so `_eq: true` binds integer `1` and REST
  `is.true` lowers to `= 1`.
- **Closed-set types** (RFD 0013): only `_eq`/`_neq`/`_in`/`_nin`/`_is_null` are
  meaningful, and operands are validated against `Set.values` (`ir.rs:124`) at
  parse — a fail-loud, pre-flightable `type_mismatch`, not a silent empty result.
  The field stays GraphQL `String`.
- **References** type against the target key's scalar via `value_type()`
  (`ir.rs:450`), which by construction never yields a `Set` (a set type may not be
  a key, E043), and reuse the by-pk key-argument canonicalizer (`arg_to_sql`,
  `graphql.rs`) so a `uuid`/`timestamp` filter operand parses identically to a
  by-pk key.
- **`storage_object`** (the builtin file table, RFD 0018) is read-only on the
  floor but *readable*; the read passes do not skip it, so it auto-gains
  `where`/`order_by`/`offset` (a filterable, read-only surface — correct). No
  filter machinery is added to the mutation passes that *do* skip builtins.

## 10. Errors

Filter faults route to the **reserved** code family (`ir.rs:313-319`) — no new
derived per-table code:

- **Unknown filter/order column** → `unknown_field` (422, table-scoped, reusing
  the write path's constructor).
- **Malformed predicate / unknown operator token / `null` where forbidden / NULL
  in an `in` list / over-budget / object-form `_or`** → `bad_request` (400).
- **Well-typed column, untypeable value / non-member closed-set value** →
  `type_mismatch` (422).

A per-table `<table>_<field>_<kind>` filter code is deliberately *not* minted: it
would re-enter the non-injective underscore-join collision the E044 pass exists
to reject (`check.rs`), and the routing channel for derived codes is the
constraint name, which a request-shaped fault has none of. (Whether an unknown
*operator* deserves its own code rather than folding into `bad_request` is a
minor open call, §14 — the recommendation is to fold it, for vocabulary
minimalism.)

## 11. The v1 `policy` dry-run — the forward-pointer, and the honest gap

Both prior systems confirm the "same tree" claim, and Hasura is a verbatim proof:
its row-permission grammar *is* its query `bool_exp`, and a Postgres RLS `USING`
clause is a boolean expression over the row's columns and a session/claims
context, AND-ed into every query that touches the table. So this RFD builds v1's
predicate engine as a side effect, provided it reserves the right seams now:

- **Two slots, both named.** `USING` (read/visibility) *is* the v0 filter path —
  a policy predicate that evaluates false/NULL for a row silently excludes it,
  matching Postgres 3VL exactly. `WITH CHECK` (write-validity) is v1-additive and
  wires to the existing refusal path (`spock_refuse` → 422/`invalid`) on a
  false/NULL result. Naming both now makes v1 purely additive.
- **Composition is server-side and non-dilutable.** The final predicate is
  `And([ policy_using, client_filter ])`, assembled *after* the client filter is
  parsed, so a client can never remove or weaken the policy conjunct. Because
  SQLite has no RLS, this composer is the enforcement point — which is why §3
  makes it the sole derived-read chokepoint.
- **The claims binding.** `Operand::Actor` (reserved, §3) resolves per request
  from `spock_actor()` / `X-Spock-Actor` (RFD 0014) — Spock's `current_setting` /
  `X-Hasura-*`. Every claims leaf type-checks against its column at contract load.
  A hard safety rule falls out: an actor-referencing predicate may be bound **only
  inside the per-request composer**, never inline-expanded into a DDL
  `DEFAULT`/`CHECK`/generated column (the RFD 0013 lowering path) — doing so would
  bind one request's actor into another's rows, the confused-deputy bug.
- **Cross-table reach** is the reserved `Exists` node (§5). Deciding its *shape*
  now (a `RelRef` + inner `Predicate`, forward via a `Ref` field, reverse via an
  inbound ref) is what makes the tree genuinely policy-shaped from day one.

**The gap, stated honestly.** v0 rehearses the claims *binding* (leaf-parametric
`Operand`) and reserves cross-table *reach* (the `Exists` shape), but it does not
exercise the `Exists` *lowering* — no v0 query emits a correlated subquery. That
lowering, `WITH CHECK` enforcement, per-role schema re-derivation, and the actor
resolve pass are the work v1 must still prove. What this RFD guarantees is that
v1 adds variants and a resolve pass to an existing tree and composer — not a
redesign.

## 12. What this deliberately does not do

- **Nested relationship traversal lowering** — the `Exists` node ships reserved
  and un-emitted; non-key ref sub-fields are `bad_request` in v0 (§5).
- **Productized keyset / Relay connections / opaque cursors** — v0 owns the
  stable total order and offset; a first-class seek cursor is deferred (§7).
  Relay's `edges/node/pageInfo` is a separate opt-in mode even in Hasura; the
  forced pk-tiebreak is exactly the invariant it would later require.
- **`count` / aggregates** — Hasura `_aggregate`, PostgREST `count=exact`. Exact
  count is O(n); planned/estimated need planner stats SQLite lacks. Deferred (and
  the G15 "counters recounted on every read" problem is a derived-fields RFD, not
  this one).
- **Case-sensitive `_like` and `_glob`, and the `nulls_first/last` order
  variants** — deferred (§4, §7). Acceptance entails reconciling graphql.md §7,
  which today still advertises `_like`: strike it (or mark it deferred with a
  deviation) so the spec and the floor agree.
- **`isdistinct` and the whole Postgres-only operator tail** — refused, enum kept
  extensible (§4).
- **Filtered and bulk *writes*** — `update_<t>(where!:, _set:)`, `delete_<t>
  (where!:)`, `insert_<t>(objects:)` → `<t>_mutation_response { affected_rows,
  returning }`, and the PostgREST filtered `PATCH`/`DELETE`. They build directly
  on this exact `Predicate` and are the reason "REST writes wait on the filter
  decision" — but they are gated to the REST-writes milestone, which must keep
  Hasura's **non-null `where`** anti-footgun wall (an all-rows write demands an
  explicit `{}`). This RFD makes them a thin skin over an IR that already exists.
- **v1 `policy`** — `Operand::Actor` construction, `WITH CHECK` enforcement,
  `Exists` lowering, per-role re-derivation (§11).

## 13. Contract mechanics (§6 freeze discipline)

In v0, **nothing changes in the contract IR.** `<t>_bool_exp` and `<t>_order_by`
are derived at runtime from a table's existing fields; the filter is protocol,
not contract (§3). The additive `#[serde(default)]` obligation the §6 freeze
imposes is owed only when v1 `policy` first attaches an authored predicate to
`Table` (parallel to `checks`/`uniques`) — at which point the predicate node
gains a normative serde shape and a legacy-JSON regression, exactly as every
prior additive field did. One totality obligation *is* owed now, in the runtime:
`<t>_bool_exp`/`<t>_order_by` must be **claimed in the type-name claim pass**
(`graphql.rs:115` region) under the existing duplicate-name guard — spec §3
reserves the suffixes but the code does not yet claim them, a real request-time-
shadow gap this RFD closes.

## 14. Open decisions

Recommendations first; these are the genuine forks left for discussion.

1. **Offset-depth ceiling (§7).** Page *size* is capped at 200 (D2); page *depth*
   needs a bound too, or offset is a DoS on the single connection. Options: a hard
   `offset` ceiling, or a window ceiling (`offset + limit ≤ N`).
   **Recommendation:** a window ceiling (say `offset ≤ 10 000`) with a
   `bad_request` past it whose message points at the deferred keyset path — a
   stopgap, honestly labeled, removed when keyset lands.
2. **`STRICT` tables (§8).** Adopt `STRICT` tables + canonicalizing constraints so
   lexical order equals value order for *all* writers (including `mut fn`
   escapes), or scope the range/order guarantee to floor-written data and record
   the escape caveat. **Recommendation:** scope-and-record for v0 (STRICT is a
   broader engine change with its own migration surface); revisit alongside the
   Postgres engine flip.
3. **Ref filter granularity (§5).** Ship the shallow key-traversal
   (`where:{author:{id:{_eq}}}` typed as `<target>_bool_exp`) now, or restrict v0
   to flat FK equality only. **Recommendation:** ship the shallow key form — it is
   the forward-compatible shape that never breaks the `where` schema when the
   `Exists` traversal lights up in v1.
4. **Unknown-operator code (§10).** A dedicated code, or fold into `bad_request`?
   **Recommendation:** fold, for vocabulary minimalism; revisit only if a client
   needs to distinguish it programmatically.

## 15. The doctrine line

fn v2 proved the contract could reach into the escape; the value tier proved the
escape could reach back out. The filter sub-language proves the third thing: that
the surface a prototype exposes for *reading* can be borrowed whole — two dialects
an LLM already writes, lowered through one owned tree to one honest `WHERE` — with
the language contributing not a syntax but a *discipline*: a stable order nobody
has to remember, a page nobody has to hand-roll, an escape nobody can inject
through, and a predicate shaped, from its first day, like the governance that will
one day AND itself in. The floor is borrowed; the discipline is Spock's. The
escape may replace the body, never the contract — and now neither the query.

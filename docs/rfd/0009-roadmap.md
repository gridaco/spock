# RFD 0009 — the v0.x roadmap

Status: working plan. §2's track inventory reflects what is built and what the
specs already commit to; §3's ordering is the plan of record until revised;
§4 records the decisions that remain genuinely open, with recommendations.
This RFD continues RFD 0008 — it is the sequencing document for everything
between the table slice and v1's governance flip.

## 1. Where the build stands

The table slice (RFD 0008) shipped end to end: the `table` + `seed` language
with checker and diagnostics, the contract IR, SQLite materialization, and an
HTTP runtime — `/~contract` and `/~health`, read-only `/rest/v1/{table}`, and
`/graphql/v1` with reads and derived auto-CRUD writes (the ungoverned
prototype tier, by decision). Two spec documents split *is* from *ought*:
`docs/spec/v0.md` is the as-implemented contract; `docs/spec/graphql.md` is
the normative dialect target (Hasura-mirrored), superseding v0.md §8.2 where
they differ.

Of RFD 0008's three open questions, one is now closed: the read interface is
**borrowed** — GraphQL, dialect spec'd, introspection delivering the
"surface as data for tooling" goal for free. The other two — when `fn`
enters, when the engine flips to Postgres — become a track (§2.6) and an open
decision (§4) below.

## 2. The tracks

**1. GraphQL Tier-1 alignment.** The runtime still speaks the pre-dialect
surface (`User`, `user_list`, `create_user` with inline args);
`docs/spec/graphql.md` §9 is the rename-by-rename checklist to the
Hasura-mirrored names. Small — and net-negative in code, since verbatim
lowercase type names delete the PascalCase collision machinery. This goes
first because every downstream track binds to names; each day it waits, more
tests, docs, and demos bind to names scheduled for deletion.

**2. Client consumption, borrow-first.** Because the dialect mirrors Hasura
and introspection is standard, the existing GraphQL client ecosystem —
`graphql-codegen`, `gql.tada`, urql, Apollo — works against `/graphql/v1`
with zero Spock-side code. The first deliverable is therefore a *proof*, not
a package: point a codegen at a running `spock run`, get end-to-end
TypeScript types, write the walkthrough. A `spock-js` package
(supabase-js-shaped, `from('post').select()`) earns its keep only once REST
has writes. Prerequisites: track 1 (stable names), plus freezing the
`/~contract` JSON shape and the derived error-code vocabulary — clients bind
to both.

**3. The filter sub-language.** The most leveraged pending design. One
predicate IR in the contract layer, two borrowed frontends: Hasura
`bool_exp` for GraphQL (`where: {author: {_eq: $id}}`) and PostgREST
operators for REST (`?author=eq.X`). It unblocks tracks 4 and 5's filtered
halves, and it is a dry run for `policy` predicates in v1 — row predicates
over a claims context are the same tree with one more binding. One design,
three consumers: RFD before code.

**4. GraphQL Tier 2.** `where`, `order_by`, `offset`, bulk mutations,
`<t>_mutation_response` — shapes already spec'd (`docs/spec/graphql.md` §7,
Tier 2), blocked on track 3 only. This is what moves reads from limit-only
to app-usable.

**5. REST writes.** By-key writes — `POST /rest/v1/{t}`, `PATCH` and
`DELETE /rest/v1/{t}/{id}` — mirror PostgREST and need **no** filter
language: they are a thin HTTP skin over the shared write path, shippable
any time after track 1. Only filtered/bulk REST writes wait on track 3.

**6. `fn`, minimal — SHIPPED.** The deliberate surface next to the borrowed floor —
the two-layer story that distinguishes the language from a generic
auto-CRUD gateway. Minimal shape: named, typed signature, SQL-escape body
(per the escape-hatch doctrine and RFD 0008's sketch: the escape may replace
the body, never the contract), surfaced as `Mutation.<name>` (the
Hasura-Actions analogue, `docs/spec/graphql.md` §1) and
`POST /rest/v1/rpc/<name>` (PostgREST RPC symmetry). Errors derive from the
signature, never from the body — deriving from arbitrary SQL is a tarpit.
Open within the track: return shape (row / rows / scalar / void), whether
read-only fns surface on `Query`, whether `seed` may call fns (lean no).

**7. Auth.** The architecture is already banked (RFD 0008 §4): mirror the
GoTrue contract (~5 endpoints), never run the binary; `auth.users` as a
builtin static table; dev-header identity populating the same claims seam a
verified JWT later fills. What remains is timing and scope, not shape — and
identity is inert until something consumes it. So implementation follows
`fn`: an actor binding in a `fn` body is identity's first consumer,
`policy` in v1 its second. Doing auth before either yields signup/login
that gates nothing.

**8. Language L2 — the syntax iteration.** The friction-driven pass over
the grammar once real programs exist: upsert semantics
(`examples/instagram/v1-FEEDBACK.md` L2 — this is what gates
`on_conflict` in graphql.md Tier 3) and enum/check candidacy — now the
`format` question, see §4. Partially consumed by the July 2026 dogfood
batch, exactly as this track intended (fed by demo friction):
`on delete set null` SHIPPED, escape-reachable defaults SHIPPED
(`spock_uuid()`/`spock_now()` + DDL DEFAULT; richer default *forms* —
expressions — remain future), plus `float` and scalar fn returns from
the same batch (v0-FEEDBACK G7/G4/G10/G9).

**9. DX and the demo.** Small items, outsized demo value:

- `spock run --watch` — recompile, rematerialize, reseed on file change.
  Disposable state (v0.md §1) makes this nearly free, and it *is* the
  executable-PRD demonstration: edit the schema, watch the surface
  re-derive live.
- An automated test that compiles `examples/instagram/v0.spock` — the
  `like` table currently appears in no test program.
- Conformance tests organized per graphql.md tier; an editor grammar;
  a README walkthrough written against the Tier-1 surface.

**10. The v1 horizon, named.** Not v0.x work; listed so the map is total.
`policy` / RLS — the governance flip that turns the ungoverned tier into
per-role derived schemas (same derivation, run per role). The engine flip
to Postgres (RFD 0008 §3; de-risked, paths recorded). `view` as the
deliberate read-side sibling of `fn`. Subscriptions and aggregates stay out
until doctrine asks (graphql.md §7).

## 3. The order

Tier-1 rename → codegen proof → `fn` → **fn v2 (RFD 0012)** →
**value constraints (RFD 0013)** → **filter, read half (RFD 0021)** →
REST/GraphQL bulk writes → auth — with track 9 filling gaps and track 8
trailing usage throughout.

(Revised July 2026: fn v2 — declared refusals, multi-statement bodies,
read/write polarity — jumped ahead of the filter RFD on the dogfood
evidence (v0-FEEDBACK G2/G3/G11), by the third rule below. The filter
RFD's scope grew in exchange: pagination/cursor discipline for *all*
row-returning surfaces — tables, future views, and read fns — was
deliberately kept out of fn v2 and assigned to the universal query layer.)

(Revised again July 2026: the value tier — closed-set types and
validator-fn `check`s, RFD 0013 — jumped ahead of the filter RFD by the
same third rule. It resolves the `format` question §4 deferred, and it
un-collapses the fn-guard refusals the borrowed floor cannot keep
(v0-FEEDBACK G1/G13). The filter RFD is next.)

(Shipped July 2026: the filter sub-language's read half — RFD 0021. One
owned predicate IR, two borrowed frontends, forced stable total order,
page + offset-depth caps, the pagination debt above discharged for derived
surfaces (read fns stay author-owned — the `view` boundary). Dogfooded by
the technical fixture `examples/filter-lab/`. Filtered/bulk *writes* and v1
`policy` build on the same IR.)

Four rules generate this ordering; if the ordering is ever revisited, argue
with the rules, not the sequence:

- **Names before bindings.** Nothing client-facing ships against a surface
  scheduled for renaming. Track 1 is first and cheap.
- **Borrow before build.** The codegen proof costs near zero and answers
  "can it be consumed" before any package is written; the filter dialects
  are borrowed (Hasura, PostgREST) over one owned IR.
- **Language work is the differentiator.** `fn` outranks Tier-2 breadth:
  surface breadth is borrowed shape, `fn` is the language's own
  contribution — and the demo story ("borrowed floor, deliberate surface")
  needs it.
- **Identity needs a consumer.** Auth implementation waits for `fn` so the
  claims seam has something to bind to; it lands as the bridge into v1,
  where `policy` becomes its second consumer.

## 4. Open decisions

- **Engine-flip timing** (carried from RFD 0008). The auth milestone runs
  on current storage — the GoTrue *contract* is storage-agnostic even
  though the binary is Postgres-only, so the flip is not forced by auth.
  Recommendation: revisit the flip alongside `policy`/v1, where RLS is
  Postgres-native and the differential-testing payoff (RFD 0005, 0006)
  compounds.
- **Client posture.** ~~Recommendation: borrow codegen now (track 2);
  build `spock-js` only when REST writes exist to wrap.~~ **Resolved —
  RFD 0010**: three artifacts (in-binary `spock gen` generator, generic
  hand-written `spock` npm client after REST writes, per-app generated
  types); generate types, never the client; the GraphQL path stays
  borrowed.
- ~~**Filter dialect.**~~ **Ratified and shipped — RFD 0021.** One owned
  predicate IR (`spock-runtime`), two mirrored frontends (Hasura `bool_exp`,
  PostgREST operators); the read half (`where`/`order_by`/`offset`, forced
  stable total order, page + depth caps) is live and dogfooded by
  `examples/filter-lab/`. Filtered/bulk *writes* build on the same IR and land
  with the REST-writes milestone; the IR is shaped as the v1 `policy` dry-run.
- ~~**`format` — column formats as a language feature**~~ **Resolved —
  RFD 0013** (July 2026). The research ran (judged design panel: curated
  format vocabulary vs raw-SQL check vs named domain — all three rejected)
  and the answer is neither a format vocabulary nor a domain grammar but
  two constructs governed by a new doctrine law (**LLM-writability**: the
  surface must be SQL-exact or radically simple, never a bespoke mini-
  grammar): a **closed-set type** (`status: "pending" | "ready"`) for the
  enum case (G1), owned end-to-end by the checker (seed/default checked,
  TS emits the literal union); and a **validator fn** referenced by
  `check` (field and cross-column) for length/charset/range/non-empty/
  ordering, inline-expanded into a named SQLite CHECK whose name is the
  derived `<table>_<fields>_invalid` code. A violation is kind `invalid`,
  422, and un-collapses the floor-leaked fn refusals (G1/G13) for free.
  Curated named formats (`email`) stay deferred until vocabulary
  versioning is solved; nominal `domain` declarations stay deferred until
  reuse demand appears (a validator fn is already the reusable unit).

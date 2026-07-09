# RFD 0005 — The proving ground: first tasks, the gauntlet, and the database inventory

Status: working method + reference. This RFD defines *how* the language gets
built and attacked, not new language surface.

## 1. The first task is not syntax

Syntax is UI over the IR. The load-bearing artifact is the **IR — the contract
as a data structure** — because every consumer (checker, runtime, `/~contract`,
codegen) reads the IR, and semantics changes ripple where syntax changes are
cheap. Languages that defined semantics first (GraphQL is execution semantics
with incidental syntax; Wasm is a spec with two interchangeable text formats)
aged better than languages that were syntax-first.

Build order:

- **Task 0 — paper programs.** Write the flagship example (Instagram) fully in
  imagined syntax, by hand, before any tooling. Timeboxed; its purpose is
  discovery, not beauty. Every sentence of the PRD that cannot be said is a
  design bug found early.
- **Task 1 — the IR and the checker, no parser.** Define the contract as plain
  data. Hand-author the username problem (G0 below) directly in IR. Build the
  checker over it: derive the error set, the reachability, the surface ledger.
  This is the falsifiable core of RFD 0002 — if the checker cannot derive
  these, no syntax will save the thesis.
- **Task 2 — the tracer bullet.** One thin vertical slice through every layer:
  the same hand-authored IR executed by the SQLite-backed runtime, served over
  HTTP, consumed by a generated TypeScript client, with the unique-violation
  arriving as the declared error — under a deliberate race.
- **Task 3 — the parser, last.** By now the syntax has been rehearsed on paper
  against real programs, and it parses into an IR that is already proven.

## 2. "Flawless" is not a verifiable property

Nothing is verified flawless; specific claims are. The method is to restate
every design claim as a falsifiable property and industrialize the attempts to
falsify it:

- **The gauntlet (§3)** — a corpus of atomic problems every language change
  must keep expressible. Examples demonstrate; the gauntlet falsifies.
- **Differential testing** — the same contract run against the prototype
  runtime and against generated Postgres; same inputs, same outcomes. SQL is
  the oracle.
- **Property fuzzing** — random contracts, random actors, random transition
  sequences. Properties: policy never leaks (no observation reaches an actor
  the ledger does not grant), the ledger is sound (observed surface ⊆ declared
  surface), transactions preserve declared invariants.
- **Conformance suite as the spec** — in the Wasm/TOML tradition: the test
  corpus is the normative artifact a second implementation must pass.
- **Optionally, Alloy** for the exposure algebra: check that no sequence of
  grants produces unintended reachability. Small relational models are exactly
  Alloy's home ground.

Each RFD claim should name its test family. A claim without a falsifier is
marketing.

## 3. The gauntlet — atomic technical problems

Small, sharp, one mechanism each. `examples/` holds product scenarios; the
gauntlet holds knives.

**Identity & uniqueness**

- **G0 · Username claim** (the tracer bullet). Unique column, `@protected`,
  changed only via `fn` — under a deliberate race. Stresses: derived errors,
  atomicity, unlocks, codegen, the whole pipe.
- **G1 · Case-insensitive username.** `Bob` vs `bob`. Stresses: uniqueness
  over an expression (lower), formats/collation — constraint derivation beyond
  plain columns.
- **G2 · Re-register a deleted username.** Soft delete vs uniqueness: the
  partial unique constraint (`WHERE NOT is_deleted`). Stresses: what
  `respects softdelete` really means for constraints.

**Counters & contention**

- **G3 · Like counter.** `count = count + 1`, not read-modify-write; a
  materialized count on the post. Stresses: relative updates (the client
  sketch's `i.a1 += ...`), `@materialized` consistency.
- **G4 · Inventory oversell.** `stock >= 0` under concurrent checkout.
  Stresses: CHECK as guard, serialization, derived `out_of_stock`.
- **G5 · Balance transfer.** Move X from A to B, no negatives, atomic.
  Stresses: multi-row invariant in one `fn` — the locality law's home case.
- **G6 · Gapless invoice numbers.** Sequences have gaps by design; legal
  invoicing cannot. Stresses: serialized allocation; whether the language
  admits "this fn is a bottleneck on purpose."

**Ordering & shape**

- **G7 · Kanban reorder.** Fractional index (single-row view write) vs
  renumber-all (fn). Stresses: the view/fn boundary as a *derived* decision —
  both designs must be expressible, and the laws must sort them.
- **G8 · Booking overlap.** No two reservations of one room may intersect.
  Stresses: range/EXCLUDE-style constraints → derived `slot_taken`.

**Time**

- **G9 · Editable for 15 minutes.** Policy depends on the clock. Stresses:
  time as an effect in guards; purity vs `now()`.
- **G10 · Auction close.** A transition with no actor, fired by time.
  Stresses: durable timers (rung 2); simulated time in the prototype runtime.

**Authorization edges**

- **G11 · Ownership transfer.** The mutation changes the row its own guard
  reads. Stresses: policy evaluates against pre-state, and that must be
  specified, not accidental.
- **G12 · Owner sees email.** The same row shows different fields depending on
  the viewer's *relationship to the row*, not just the role. Stresses:
  per-row conditional projection — one view or two?
- **G13 · Add a member.** Many-to-many junction with payload (role field).
  Stresses: INSERT-through-view (deferred in RFD 0003) — this common case
  presses on that deferral immediately.

**Modeling edges**

- **G14 · Comment thread.** Self-referential replies, rendered nested.
  Stresses: recursion or bounded depth in views; N+1 by construction.
- **G15 · Comment on post or photo.** Polymorphic reference — SQL's classic
  wart. Stresses: whether Spock's sum types can beat the type+id idiom.
- **G16 · Avatar upload.** Signed URL, upload, attach; orphan cleanup on
  abandonment. Stresses: storage builtin, two-phase flows, `once` values.

**Cross-cutting**

- **G17 · Audit everything.** Every mutation appends an audit row. Stresses:
  cross-cutting declaration without aspect-flavored magic.
- **G18 · Concurrent edit.** Two admins, one post. Stresses: optimistic
  concurrency (RFD 0003's open question) — version column as language
  feature or idiom.
- **G19 · Webhook ingestion.** At-least-once delivery, idempotency key,
  exactly-once effect. Stresses: dedupe as contract, protocol rung.

Pass criteria for every G: expressible in the language without waivers (or
with a *named* waiver the RFDs predict), checkable by the linter, runnable in
the prototype, and the derived client types tell the truth.

## 4. The database inventory

Every concept a modern database (Postgres-centric) carries. Method: each
concept gets a disposition — **absorb** (becomes language semantics),
**surface** (pass-through annotation), **defer** (named, later), **out**
(production/ops, explicitly not Spock's job). A concept that resists
disposition is a design bug, found early.

**Types & definition**

- scalar types (int/bigint/decimal, text, bool, bytea) — absorb
- money — absorb as semantic format, never float
- timestamps & timezones — absorb; stance: instant-only (timestamptz),
  timezone at the edges
- date / time / interval — absorb
- uuid — absorb; stance: time-ordered v7 default for identity
- json/jsonb — absorb (document fields), but structured types preferred
- arrays — absorb
- enums — absorb (language-grade, with exhaustive match in guards)
- composite/sum types — absorb; Spock's answer to polymorphic refs (G15)
- range types — absorb (booking, G8)
- vector embeddings — defer (rung 4 module)
- geospatial — defer (module)
- domains / constrained types — absorb: this is `format`
- NULL, three-valued logic — **absorb by replacement**: optional types (`?`);
  3VL never reaches the author
- defaults (`@default`), generated/computed columns — absorb
- sequences / identity — absorb, gaps documented (G6)

**Constraints** (each one is an error to derive — RFD 0002)

- primary key — absorb (the view-addressing key, RFD 0003)
- unique: plain, composite, partial, expression — absorb (G0, G1, G2)
- foreign key + referential actions (cascade/restrict/set null) — absorb;
  stance: delete behavior must be declared per relation, restrict by default
  (fail-safe)
- CHECK — absorb (guards on state, G4)
- NOT NULL — subsumed by optional types
- EXCLUDE (overlap) — absorb (G8)

**Indexes**

- unique indexes — absorb: they are semantics
- everything else (btree/gin/gist/partial/covering) — surface: performance
  annotations, derivable, never contract
- collation/citext — absorb into format semantics (G1)

**Queries**

- project/filter/join — absorb: views
- aggregates — absorb: computed/materialized fields (G3)
- window functions — defer (computed fields v2)
- recursive queries — absorb, bounded (G14)
- pagination — absorb at protocol level; stance: keyset cursors, not offset
- full-text search — defer (rung 4 boundary)
- materialized views — absorb as `@materialized` with a stated consistency
  promise (transactional first)

**Mutations**

- insert/update/delete — absorb (views for local writes, fns for deliberate)
- upsert (ON CONFLICT) — absorb as a first-class verb: the idiomatic cure for
  check-then-act
- RETURNING — absorb: fn return shapes
- bulk operations — defer (bulk fn semantics unclear)

**Transactions & concurrency**

- ACID, isolation levels — **absorb by stance**: every fn is one serializable
  transaction; the runtime auto-retries serialization failures and deadlocks
  bounded times; effects fire only after commit (outbox). The author never
  chooses an isolation level.
- explicit locks, SELECT FOR UPDATE — out of the author's hands; the stance
  above covers the need
- SKIP LOCKED / job queues — defer (rung 2/4)
- advisory locks — out
- optimistic version columns — absorb or bless as idiom (G18, open)

**Security**

- SQL injection — **absorbed by construction**: no string SQL exists to inject
- roles/grants, column privileges — absorb: the exposure model (RFD 0004)
- RLS — absorb: policy (README "Policy ergonomics")
- definer vs invoker — absorb by stance: fns run with an explicit actor
  context, no ambient definer authority; escalation only via named waiver
  (kills the confused deputy)
- security-barrier views / leaky predicates — absorb: the runtime must not
  evaluate user expressions before policy predicates

**Server-side machinery**

- functions/procedures — absorb: fn
- triggers — absorbed by generation (write-through putbacks, materialized
  maintenance, audit); never user-authored
- INSTEAD OF — the compilation target of RFD 0003
- LISTEN/NOTIFY — defer: live views over the protocol
- scheduled jobs — defer: rung 2 timers
- foreign data wrappers — out (rung 4 is the boundary, not FDW)

**Folklore patterns** (no SQL keyword, everyone builds them)

- soft delete — absorb (`@softdelete`, with G2's constraint interplay)
- audit trail — absorb, declared (G17)
- created/updated stamps — absorb (`@createdat`, `@updatedat` — vision draft)
- status state machines — absorb: enum + transition guards; a `fn` may declare
  `from status to status`
- multi-tenancy — open: first-class tenant scope vs policy idiom; decide on a
  SaaS example
- event sourcing — out (a different church; Spock is state-first)
- outbox, idempotency keys — absorb: the effect machinery (RFD 0001)
- hierarchical data — G14; adjacency first
- i18n fields — defer

**Operations** (named so the boundary is explicit)

- backups/PITR, replication topology, pooling, vacuum, partitioning,
  tablespaces, EXPLAIN — **out**: production concerns; the artifact hands off,
  the prototype never meets them. CDC/logical replication appears only as the
  sync substrate already in the doctrine.

## 5. What the inventory forces immediately

Walking the list surfaces the stances the language must take now, each with
its gauntlet problem:

1. **Optionals, not NULL** — 3VL never reaches the author.
2. **Serializable-by-default fns with auto-retry**, effects after commit.
3. **Upsert as a verb.**
4. **Explicit actor context** — no definer/invoker split, no confused deputy.
5. **Unique indexes are semantics; other indexes are performance.**
6. **Referential delete behavior is declared, restrict by default.**
7. **Keyset pagination at the protocol.**
8. **uuidv7 identity, instant-only time.**
9. Multi-tenancy: undecided, needs the SaaS example.

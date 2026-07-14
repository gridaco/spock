# RFD 0023 — Development reload, state continuity, and the auto-migration boundary

Status: **problem study; long-term direction under evaluation.** This document
does not amend the v0 contract, promise state-preserving reload, or introduce
production migrations. A deliberately smaller interim host policy was accepted
on 2026-07-15 so framework composition can proceed without selecting a world,
rebase, or migration model.

The working candidate is **non-destructive development worlds with an optional
three-way state rebase**. Worlds are the safety and rollback primitive. Rebase
is only a convenience for initializing a new world when compatibility can be
proved. Both parts remain subject to experiments and review.

## Interim implementation policy — client live, backend pinned

During the first combined `spock dev`, one valid Spock backend generation is
constructed and activated exactly once. Uhura client changes continue to use
coherent capture, current diagnostics, immutable candidates, newest-result
ordering, and last-known-good publication against that active generation.

Backend source, referenced seed assets, and backend/topology manifest changes
are observed and fingerprinted but receive the terminal disposition
`restart_required`. They never call `engine::open`, delete or reopen a
database, replay seed, construct a shadow backend, or swap any backend-owned
contract, connection, signer, blob store, route, or background task. Reverting
all changed backend inputs and topology to the active fingerprints clears the
warning without touching state. Client publication may continue during the
warning and must name the active backend generation, not the changed source.

An explicit process restart is the only way this first implementation adopts
a backend edit, and the CLI warns that current v0 startup reconstructs the
database from seed. This policy is not a candidate answer to auto-migration;
it is the safe no-activation baseline against which every later proposal must
improve. The implementation has exactly one future marker at the disposition
seam and no migration logic elsewhere:

```text
TODO(RFD-0023): replace restart-required with off-path backend candidate
construction and an explicit activation policy after development-world
semantics are accepted. Never reopen or mutate the active world here.
```

## 0. Why this must be decided before runtime composition

RFD 0006 promises a fast source-to-play loop by reloading checked IR into a
static runtime. RFD 0009 sketches `spock run --watch` as “recompile,
rematerialize, reseed.” That was coherent while all v0 state was explicitly
disposable. It is no longer a sufficient product answer once a developer can
spend minutes creating data through a live Uhura experience and then edit the
backend contract.

Three naive interpretations of “reload” disagree materially:

1. keep the database and replace only the API/runtime artifact;
2. mutate the current database to match the new source; or
3. discard and reseed the database.

If the framework host is built before this distinction is explicit, one of
those behaviors will become the de facto contract simply because it was
easiest to wire. That is exactly the kind of accidental state semantics Spock
is meant to prevent.

The current normative rule remains unchanged: the v0 engine materializes a
fresh database and replays seed on each load; there are no migrations and
state is disposable ([v0 specification](../spec/v0.md), §7.3). This RFD asks
what `spock dev` should eventually do. `spock run` may retain its current
meaning independently.

### A correction about the parity target

Uhura does not currently migrate a running Play session across source edits.
Its accepted RFC deliberately calls the feature **saved-source live
rebuilding**, and lists Play runtime-state migration/HMR as a non-goal
([Uhura RFC 0002](https://github.com/gridaco/uhura/blob/42ece8e3c44efe89d3c9417761504e7b190db230/docs/rfcs/0002-model-driven-editor-live-updates.md)).

What Uhura already establishes — and what a combined host should share first
— is:

- coherent, debounced saved-project capture;
- monotonically ordered source revisions;
- complete candidate construction before publication;
- last-known-good retention when a candidate is invalid;
- explicit current/stale diagnostics; and
- browser notification only after an eligible generation exists.

Spock database continuity is a second axis. The two ecosystems can have one
observable reload UX without pretending their internal state has the same
lifetime or migration problem.

## 1. Goals and non-goals

### Goals

1. During one `spock dev` process, a source error never kills the last working
   project. Preserving that generation across process restart requires the
   persisted artifact bundle discussed in Section 14.
2. An ordinary behavior/client edit does not unnecessarily reset
   authoritative data.
3. A structural edit never silently loses or reinterprets live data.
4. A candidate can be fully constructed and load/state-validated before it
   becomes visible; this does not claim exhaustive function-behavior testing.
5. When a client is configured, it and the backend activate as one coherent
   project generation.
6. Conflicts are understandable and actionable rather than hidden behind a
   generic SQLite failure.
7. The design preserves the fast prototype loop and remains appropriately
   small for local development.
8. The model can be specified independently of one SQLite trick, even when
   SQLite is its first implementation.

### Non-goals

- ordered, committed production migration history;
- upgrading an arbitrary persistent database across released Spock versions;
- inferring that a delete-plus-add was intended as a rename;
- executing user effects while reconstructing state;
- reproducing wall-clock timing or request interleavings;
- preserving browser-local Uhura Play state across a hard client reload;
- making Studio a general database migration or operations console;
- zero-pause cutover for large databases in the first version; or
- guaranteeing that every valid source edit can preserve existing state.

“Auto migration” is useful as the name of the problem. It should not be the
name of the solution unless Spock eventually accepts the production-grade
obligations implied by that term.

## 2. Current implementation facts

This problem is constrained by behavior already in the repository.

### 2.1 Load means destroy, derive, and seed

[`engine::open`](../../crates/spock-runtime/src/engine.rs) deletes the target
database plus its WAL/SHM companions, opens a new connection, enables foreign
keys, emits every contract table, creates the hidden blob table when needed,
validates function SQL and defaults, and replays seed. It is an excellent full
load proof. It cannot be called against the active development database by a
watcher.

SQLite does support a limited set of direct `ALTER TABLE` operations and a
documented generalized table-rebuild procedure, but those mechanisms only
answer how to transform storage. They do not answer Spock's semantic questions
about seed provenance, live edits, rename intent, or rollback. See SQLite's
[ALTER TABLE documentation](https://sqlite.org/lang_altertable.html).

### 2.2 Seed is executable materialization, not a static row file

Seed rows execute in order through the same write path used by the runtime.
They may bind an earlier row and refer to its key. Defaults such as `auto` and
`now` are evaluated during materialization, so the same source can produce
different UUIDs and timestamps on two loads.

`file(...)` makes the problem wider than relational rows: it reads bytes,
creates a fresh committed `storage_object`, writes metadata and the hidden
blob transactionally, and returns the new generated object ID. Asset content
is therefore part of the seed input even though the contract IR carries only
its path.

### 2.3 Runtime mutations are not equivalent to API calls

A mutating Spock function may contain allowed raw row SQL (`INSERT`, `UPDATE`,
`DELETE`, `REPLACE`, or a write-bearing `WITH`) and executes its statements in
one transaction. Foreign-key cascades, set-null actions, storage sweeps, and
future triggers can create indirect row changes. Generated UUID/time values
are materialized outcomes.

Consequently, replaying GraphQL/RPC requests is not a faithful state model.
Re-execution under a new function body, actor, clock, or generated identifier
can yield a different database even when every request succeeds.

### 2.4 The current runtime has no generation seam

[`App`](../../crates/spock-runtime/src/lib.rs) owns an immutable `Contract`, a
single serialized `Connection`, a randomly generated URL signer, and a blob
store. The GraphQL schema and Axum router eagerly capture that `App`. A reload
design therefore needs a new immutable generation boundary and a supervisor;
mutating fields inside the current `App` is not a coherent cutover protocol.

### 2.5 Storage is part of authoritative state

The default blob implementation stores bytes in a hidden SQLite table keyed
to `storage_object` metadata
([`storage/blob.rs`](../../crates/spock-runtime/src/storage/blob.rs)). A
metadata-only transfer can create committed objects whose bytes are absent.
The signer is random per `App`, so recreating an `App` on a behavior-only edit
currently invalidates outstanding signed URLs. A sweep task is spawned for a
serving storage contract and has no generation-level cancellation handle.

Rows, blob bytes, pending uploads, signing lifetime, and sweep ownership all
belong in the reload study.

## 3. Terminology

Use these terms to keep source, behavior, and state lifetimes distinct.

- **Source revision** — one coherent saved project snapshot. A filesystem
  event is not a revision.
- **Candidate** — checked artifacts and any prepared state that have not been
  published.
- **Project generation** — an immutable published binding of a Spock
  contract/router, one selected world handle, route table, generation ID, and,
  when a client is configured, matching Uhura Play artifacts. Its artifacts
  and binding do not mutate; the active world's authoritative rows still
  change through requests.
- **Behavior/API fingerprint** — the parts of a checked contract that affect
  protocol or execution without changing persisted-state validity.
- **State ABI** — the canonical physical/logical state plan: tables, fields,
  types, keys, references, defaults, checks, uniqueness, deletion behavior,
  hidden engine tables, and engine/storage ABI version.
- **State ABI fingerprint** — a canonical hash of that plan. It is not a hash
  of the whole contract.
- **Seed source fingerprint** — canonical seed IR plus content digests for
  every referenced asset.
- **Seed materialization** — the exact generated rows, timestamps, IDs,
  metadata, and bytes produced from seed.
- **Baseline (`B`)** — an immutable seed-only materialization used as a
  comparison base.
- **Live database (`L`)** — a baseline plus runtime interaction.
- **Live delta (`D`)** — the logical keyed difference from a baseline to its
  live database.
- **World** — one unique development-state instance: exact baseline identity,
  live database and blobs, lineage, compiler/runtime ABI, and lifecycle
  metadata. An active world is mutable authoritative state. Multiple
  behavior-only project generations may share it.
- **Archived world snapshot** — a closed, immutable checkpoint of a former
  active world. Restoring it creates a new descendant world rather than
  reopening the archive for mutation.
- **State rebase** — a three-way attempt to apply a live delta from an old
  baseline onto a new baseline. This is Spock terminology for the
  `B0/L0/B1` semantic merge; it is not SQLite's differently scoped
  `sqlite3_rebaser` changeset operation.
- **Fresh activation** — activate the new baseline without the old live
  delta.
- **Migration** — a durable, ordered transformation intended to upgrade a
  persistent store. This RFD does not propose one.
- **Stale** — the newest source revision is invalid or state-blocked while an
  older project generation continues serving.

The seed fingerprint is not the state ABI fingerprint. A seed edit can create
a new baseline within the same ABI family. It does, however, participate in a
world's lineage and reuse identity.

## 4. Proposed safety invariants

Any accepted design should satisfy these before optimizing speed.

1. **The active world is never a migration target.** Candidate construction,
   schema work, and rebasing occur elsewhere.
2. **Candidate invisibility.** Invalid, conflicted, crashed, or superseded
   work changes no served endpoint or active state.
3. **No mixed server generation.** A request sees a coherent contract, router,
   database, blob store, and client compatibility ID. Browser coherence is
   completed by the generation handshake in §12, not by assuming an already
   loaded page changes atomically with a server pointer.
4. **No lost committed write during preservation.** Every transaction
   committed in the old world before a reuse/rebase activation appears in the
   activated world or activation aborts, even if its HTTP response was lost or
   not yet delivered.
5. **Explicit fresh divergence remains recoverable.** Fresh activation may
   omit the old live delta only under the explicit policy in invariant 11;
   every omitted committed write remains in the archived old-world snapshot
   until documented retention removes it.
6. **Request boundary.** One request never starts under one backend generation
   and commits through another.
7. **Newest eligible revision wins.** Slow older work cannot activate after a
   newer source revision.
8. **Rebase is all-or-nothing.** No silent partial row/table transfer.
9. **Target invariants win.** The new contract's types, checks, uniqueness,
   keys, references, and blob invariants must all validate before activation.
10. **No guessed identity.** Table, field, or key renames and semantic type
   conversions require explicit future mapping or a fresh world.
11. **No silent fresh reset with live work.** If the live delta is nonempty,
    switching to a fresh baseline requires an explicit policy or user action.
12. **No effect replay.** State reconstruction neither calls external systems
    nor replays user requests.
13. **Recoverable history.** Replaced worlds are archived immutably and remain
    restorable until the documented, bounded retention policy removes them;
    cleanup need not require a separate user action for every archive.
14. **Structured status.** Source freshness, state conflicts, world identity,
    and active generation are machine-readable, not only terminal prose.

These are proposed invariants, not an assertion that the current runtime
satisfies them.

## 5. The change dimensions

“The file changed” is too coarse to choose a state action. At least four
fingerprints matter:

```text
source revision
├── behavior/API fingerprint
├── state ABI fingerprint
├── seed source fingerprint
└── compiler/runtime/storage ABI
```

Examples:

| Edit | Behavior/API | State ABI | Seed |
|---|---:|---:|---:|
| documentation | changed | same | same |
| ordinary function SQL/body | changed | same | same |
| function signature or returned record | changed | same | same |
| validator function used by a field/table check | possibly changed | changed because SQL is inlined into DDL | same |
| field default | possibly changed | changed | same |
| nullable field addition | changed | changed | same |
| seed scalar | same | same | changed |
| bytes behind `file("avatar.png")` | same | same | changed |
| engine changes hidden storage layout | maybe | changed by ABI version | maybe |

Hashing the entire contract as “schema” would rematerialize state for harmless
documentation and function edits. Hashing only table names would miss checks,
defaults, and storage behavior. The fingerprint needs a canonical semantic
plan. Emitted DDL is one input, not the definition: mandatory non-DDL markers
include Spock logical types that share SQLite affinity, identity-anchor and
builtin roles, canonical UUID/timestamp/bool rules, hidden blob/checksum
semantics, mint behavior, asset/MIME handling, and engine/storage ABI versions.

## 6. Candidate models

The models below are alternatives at the semantic level. SQLite mechanisms
are evaluated separately because a mechanism is not a state policy.

| Model | Active-state safety | Live continuity | Seed semantics | Assessment |
|---|---|---|---|---|
| Reset and reseed on every valid save | Safe only if old DB is retained | None | Simple | Too disruptive; useful as a baseline/fallback, not the shared DX. |
| Reuse DB until physical change, then reset | Good for behavior edits | Lost on every schema edit | Simple | Credible minimum but still surprising during schema work. |
| Generate in-place `ALTER` operations | Mutates the only good world | Potentially high | Unsolved | Reject as the foundation; this becomes a migration engine. |
| Clone live DB, then alter the clone | Active world safe | Potentially high | Cannot distinguish seed from live changes | Better mechanically, incomplete semantically. |
| Replay API/RPC operations | Active world safe | Superficial | Replays old actions against new behavior | Reject as a correctness model. |
| Copy matching columns from live into a fresh DB | Active world safe | Partial | Overwrites/confuses new seed intent | Reject; provenance is missing. |
| Non-destructive worlds, no rebase | Strong | Old state retained but not carried | New seed is clear | Strong first deliverable and permanent fallback. |
| Worlds plus logical three-way rebase | Strong if fail-closed | High for provable changes | Explicit three-way rules | Working candidate; highest semantic cost. |

### 6.1 Why in-place auto-migration is the wrong foundation

An in-place planner must infer rename intent, order transformations, preserve
data through key/type changes, reconcile seed, handle application writes
during migration, and recover from partial failure. It is no longer a local
watch feature; it is an operational migration system acting on the only good
copy.

Prisma's development-oriented `db push` is useful precedent for the boundary:
it synchronizes desired schema without creating migration history, but blocks
or requires explicit acceptance when it predicts data loss and cannot
orchestrate custom data migration. See Prisma's
[schema prototyping documentation](https://www.prisma.io/docs/orm/prisma-migrate/workflows/prototyping-your-schema).
Spock should not hide a stronger and riskier promise behind an automatic save.

### 6.2 Why operation replay is not state replay

Request replay changes meaning when:

- a function body changes;
- `auto`, `now`, or actor-derived values are evaluated again;
- raw function SQL performs several writes;
- cascades or background cleanup create indirect changes;
- a file upload's bytes are no longer available as a request body; or
- future external effects would run twice.

The state transfer must operate on committed outcomes, not intentions captured
at a higher layer.

## 7. Working candidate: worlds first, rebase second

The recommended conceptual hierarchy is:

> A separate world makes candidate work non-destructive and keeps the prior
> state recoverable while retained. A rebase may make a new world feel
> continuous, but no rebase is required for candidate safety.

### 7.1 World identity and lineage

A world needs more than a schema hash:

```text
World {
  world_id,                  # unique instance, not a content hash
  parent_world_id?,
  state_abi_hash,
  seed_source_hash,
  seed_materialization_hash,
  baseline_snapshot_id,
  compiler_runtime_abi,
  created_at,
  last_activated_at,
  live_write_revision,
  state_path,
  blob_state,
}
```

Two explicit resets may produce different worlds even when every source hash
matches. Reverting source to an earlier fingerprint may offer the most recent
compatible world, but it must not make world identity itself a content hash.
An active world changes through requests. Archival first closes/checkpoints an
exact immutable snapshot; restoring that snapshot forks a new `world_id` with
the archive as its parent.

Database branching products are useful precedent for treating isolated data
states as named development environments rather than overwriting one shared
database; see Neon's
[branching overview](https://neon.com/docs/guides/branching-intro). Spock's
local SQLite worlds would be much simpler and do not inherit Neon's storage or
copy-on-write guarantees.

### 7.2 The rebase equation

For an old world and new source:

```text
B0 = exact old seed baseline
L0 = active live state derived from B0
D  = logical difference B0 -> L0

B1 = exact new seed baseline under the candidate contract
N  = apply compiler-mapped D onto B1
```

`N` is a candidate world. It becomes active only when mapping is supported,
the entire delta applies without unresolved conflicts, every target invariant
passes, and no committed tail write was missed.

If rebase fails:

- `L0` remains active and unchanged;
- `B1` may remain available as a fresh candidate;
- the status explains why automatic preservation stopped; and
- the developer may explicitly activate fresh state or keep working on the
  last-good generation.

The unresolved policy is whether a failed rebase should automatically switch
to fresh `B1` because the old world is retained. The safer working
recommendation is: automatic fresh activation is allowed when `D` is empty;
otherwise default to stale/blocked and require an explicit switch. A
`fresh-on-conflict` mode could choose the opposite UX later.

### 7.3 Activation decision tree

```text
candidate source invalid?
  yes -> reject; keep active generation
  no
  |
  +-- state ABI/runtime ABI unchanged, and seed source belongs to the
  |   active world's existing baseline lineage?
  |     yes -> reuse world; replace behavior/API generation
  |     no
  |     |
  |     +-- live delta empty?
  |     |     yes -> activate fresh candidate world
  |     |     no
  |     |     |
  |     |     +-- supported rebase succeeds and validates?
  |     |           yes -> activate rebased candidate world
  |     |           no  -> keep last good; expose fresh candidate/reset action
```

The reuse fast path does **not** rematerialize unchanged seed. An unchanged
seed source fingerprint within the active world's baseline lineage reuses that
exact existing materialization, including its generated values. Only a seed
change or a state/runtime ABI change constructs a new `B1`; that candidate then
records an exact materialization hash. Identical source text outside an
existing lineage is not by itself proof that two independently materialized
baselines have equal UUID/time values.

## 8. Logical three-way rules

Rows are identified by their declared primary-key tuple. Fields are mapped by
checked logical name and compatible type, never by SQLite ordinal alone.

### 8.1 Field merge

For a row present in all three states:

| Old baseline `B0` | Live `L0` | New baseline `B1` | Result |
|---|---|---|---|
| `x` | `x` | `y` | take new seed value `y` |
| `x` | `y` | `x` | carry live value `y` |
| `x` | `y` | `y` | same physical value; preserve live intent metadata or conservatively conflict |
| `x` | `y` | `z` | conflict when `x`, `y`, `z` are distinct |

Disjoint field edits on the same row may merge. There is no implicit “live
wins” or “seed wins” rule for a true conflict. Equal final values need the
provenance rule in §8.3; value equality alone does not prove that future seed
edits may forget the live edit.

### 8.2 Row merge

- Runtime insert, key absent from both baselines: carry the inserted row.
- New seed insert, key absent from old baseline/live: accept the seed row.
- Independent runtime and seed insert with the same key: conflict in the
  conservative version even when rows are currently equivalent, unless live
  overlay provenance is preserved explicitly.
- Runtime delete while new seed is unchanged: preserve the delete.
- Seed delete while live is unchanged: accept the seed delete.
- Runtime delete versus seed update: conflict.
- Runtime update versus seed delete: conflict.
- Row inserted and later deleted within the live world: no final delta.
- Primary-key change: model as delete plus insert and block in the conservative
  version unless both sides are unambiguous.

### 8.3 State equality is not provenance equality

A snapshot-only three-way merge can erase developer intent. Suppose live state
changes `x -> y` and the new seed independently changes `x -> y`. If the new
world stores only physical rows, it equals `B1`; its next computed live delta
is empty. A later seed edit or deletion can then replace/remove the value even
though it originally came from live interaction. Equivalent independent
inserts and delete/delete convergence have the same problem.

The design must choose one of these honestly:

1. **State-only merge.** Equal values converge and live intent is deliberately
   forgotten. This weakens the claim that live work is never reinterpreted.
2. **Conservative conflict.** Any independent same-key/same-field change is
   blocked even if values are equal.
3. **Persisted live overlay provenance.** A rebased world stores carried field
   edits, inserted-row ownership, and delete tombstones separately from its
   physical rows, so its next rebase still knows which intent came from live
   interaction.

The working safety recommendation is option 2 until option 3 is specified.
Calling the output `N` is therefore not enough; a future provenance-preserving
world would contain both physical target state and a rebased logical overlay.

### 8.4 Schema compatibility

The first conservative mapper could use these boundaries:

| Change | Candidate treatment |
|---|---|
| Same table, key, field name, and logical type | Eligible. |
| Add table | Eligible; new seed supplies its baseline. |
| Add nullable field | Eligible; carried rows use `NULL` unless seed/new data says otherwise. |
| Add field with a deterministic literal default | Eligible; candidate insertion may use the target default after validation. |
| Add field with `auto`, `now`, or `me` | Block initially absent an explicit fill law; reload-time entropy/time/actor is not historical data. |
| Add required field without a default/fill rule | Conflict for every carried row that lacks a value. |
| Add/change UNIQUE, CHECK, FK, requiredness, or delete behavior | Eligible only if full candidate application and validation pass. |
| Change a default or validator/check body | Rebuild/rebase; it changes the state ABI even if current rows pass. |
| Reorder fields | Eligible only through name-aware logical mapping, never raw positional replay. |
| Remove field/table with no live-only information | Potentially eligible; must be reported as intentional source data removal. |
| Remove field/table carrying live-only information | Block or require explicit fresh activation. |
| Rename table/field | Block absent explicit future mapping. |
| Change key shape, key type, or reference target | Block in the conservative version. |
| Change scalar type | Block unless a future conversion lattice proves total and lossless. |
| Change/remove identity anchor | Block initially; protocol and actor state also change. |
| Remove storage while objects or file-backed fields exist | Block/fresh; never silently orphan bytes. |

After logical mapping, the new database is the final judge. SQLite constraint
success alone is not enough; the host should also perform semantic,
foreign-key, contract-load, blob, and router/schema proofs before publication.
A full physical `integrity_check` is useful for fault-injection, restore, and
periodic validation, but may be too expensive to require on every hot
candidate without measurement.

### 8.5 Desired-state installation, not delta-operation replay

The three-way rules determine a **desired final logical target**, not an
ordered script of old mutations. This distinction prevents indirect effects
from being applied twice.

For example, deleting a parent in `L0` may already have cascade-deleted a
child. Both absences appear in the final logical delta. When candidate
installation deletes the parent in `B1`, its new cascade may remove the child
first; the child target is then already satisfied, not a missing-row conflict.
The same rule applies to `set null`, storage sweeps, and other indirect
changes.

A conservative installer therefore:

1. computes merge conflicts from the three complete logical states;
2. constructs or streams the desired target rows for every mapped table;
3. reconciles `B1` toward that target in dependency-aware phases or staging
   tables, treating an already-achieved final state as a no-op; and
4. validates the completed target as one transaction before publication.

The implementation may optimize this into inserts/updates/deletes, but those
operations are derived from the desired target. Their execution artifacts do
not redefine the merge laws.

The first installer also needs an explicit foreign-key strategy. A complete
valid graph may contain cycles that cannot be loaded row-by-row while
non-deferrable checks are active. One candidate is to disable foreign-key
enforcement **only on the off-path empty candidate connection**, load the
complete desired rows, re-enable it, and require `foreign_key_check` to return
clean before activation. This must be tested for cyclic/self references and
must never weaken checks on the active world.

SQLite physical validity is not Spock value validity. The current tables are
not `STRICT`, and raw function SQL can place a wrong SQLite storage class or a
non-canonical bool, UUID, or timestamp into an affinity-compatible column. The
rebaser must compare raw typed SQLite values rather than lossy wire JSON and
run an explicit full contract-value scan over the desired target. Equality
rules for `NULL`, floats (including `-0` and non-finite values), canonical
timestamps/UUIDs, blobs, and logical booleans are part of the model still to
specify.

## 9. Seed is the hardest identity problem

A three-way merge assumes that “the same baseline row” can be recognized
across materializations. That is not always true today.

### 9.1 Strong and weak identities

Strong candidates:

- an explicitly authored complete primary key; and
- a named seed binding, when the binding denotes one key-addressable row.

Weak or absent identities:

- an unbound row whose key is generated with `auto`;
- source ordinal (“the third seed row”), because reordering changes identity;
- a timestamp generated with `now`; and
- a `file(...)` storage object whose object ID is freshly minted.

Identical seed source can therefore materialize as delete-plus-insert across
every auto-keyed row. A source hash alone cannot repair this.

### 9.2 Candidate seed policies

Three coherent policies deserve explicit comparison:

1. **Seed is reset-only.** Validate seed on every candidate but apply seed
   edits only on an explicit fresh world. This is simplest and makes seed
   non-reactive during `dev`.
2. **Seed edits always fork fresh.** A valid seed edit activates or offers a
   fresh world; no live delta crosses it. Old worlds remain restorable.
3. **Seed participates in rebase.** Store exact baselines and stable generated
   seed identity, then apply the three-way rules above. This gives the richest
   UX and the most machinery.

The working rebase model assumes policy 3 eventually, but this RFD does not
accept it yet.

### 9.3 Seed mint ledger

If policy 3 is chosen, the host likely needs a persisted, world-lineage-scoped
`SeedMintLedger`:

```text
(stable seed identity, field) -> exact generated value
```

The ledger would survive dev-host restarts and reuse generated keys,
timestamps, and storage-object IDs for the same logical seed declaration
across descendant worlds. New identities receive new values. A changed
`file(...)` may retain object identity while its content digest, metadata, and
bytes change in the new baseline.

Open choices:

- require bindings for every auto-keyed seed row that should survive reload;
- add stable seed declaration IDs to the language/IR;
- use explicit keys only for the first rebase implementation; or
- let the ledger block ambiguous rows and offer a fresh world.

Do not silently use source order as identity. That turns harmless reordering
into cross-row data corruption.

## 10. Delta capture and SQLite tooling

The semantic model should be specified as a logical delta even if SQLite
helps encode it.

### 10.1 Baseline-versus-live logical diff

The clearest first implementation is to retain an immutable exact baseline
and compare its contract tables with the quiesced live world by declared key.
This captures final effects from GraphQL, REST, raw function SQL, cascades,
storage sweeps, and generated values without replaying operations.

It is O(database size) per structural reload. That may be entirely acceptable
for the local prototype scale and is easier to audit than an incremental
journal. Blob checksums can avoid comparing every byte when both metadata and
content-address assumptions are valid.

SQLite's [Online Backup API](https://www.sqlite.org/backup.html) can make a
consistent snapshot of a live or in-memory database and is suitable for
baseline/world checkpoints. It produces a database snapshot; it does not
translate schemas.

### 10.2 SQLite Session extension

SQLite's [Session extension](https://www.sqlite.org/sessionintro.html) is the
closest built-in precedent for `B0 -> L0`. It records row-level inserts,
updates, and deletes, coalesces repeated edits, supports inversion, and has a
conflict model. It requires declared primary keys and is designed for the same
schema or a compatible target.

That makes it useful as:

- an efficient same-world delta recorder;
- a way to calculate baseline/live differences; and
- a source of tested conflict concepts.

It is not the cross-schema semantic mapper. Changesets encode table name,
column count, and primary-key positions. SQLite's
[`changeset_apply`](https://www.sqlite.org/session/sqlite3changeset_apply.html)
compatibility rules allow only constrained layout differences, and an
incompatible target table may be skipped with a schema warning. Spock must
preflight every mapped table and fail closed rather than treating an API
success as proof that all logical state moved.

The workspace currently enables `rusqlite`'s `bundled` and `functions`
features, not `session`. The pinned `rusqlite` version exposes an optional
Session API, but enabling it changes SQLite build features and introduces
release/cross-compilation work. It should be an optimization after the logical
laws are tested, not an MVP dependency.

### 10.3 Avoid a custom high-level journal

A request journal misses indirect database changes. A low-level materialized
change journal eventually duplicates SQLite Session while still needing the
same schema mapper. Start with baseline/live truth; optimize only after a
measured need.

## 11. Storage, hidden state, and effects

### 11.1 Blob atomicity

A world includes both `storage_object` and hidden `storage_blob`. Candidate
construction must copy or merge metadata and bytes in one transaction and
verify that every committed metadata row has the expected bytes/checksum.
Archived worlds must preserve their blobs as long as they are restorable.

`file(...)` seed assets participate in the exact baseline. A changed asset
digest is a seed change even when the `.spock` text is identical.

### 11.2 Signing lifetime

Behavior-only generation swaps should not invalidate every displayed image or
pending signed upload, so a same-world generation should retain its signing
identity. Reusing one unscoped session secret across different worlds is not
safe: current signatures bind method, object ID, and expiry, so an old URL may
authorize a different object that happens to reuse the ID in the new world.
Pending PUT URLs are especially dangerous.

A world transition must therefore do one of the following:

- rotate the secret and invalidate old-world URLs;
- bind `world_id`/lineage (and where needed content identity) into the signed
  path and signature; or
- route a generation/world-addressed **read** URL to the retained old world
  until expiry.

The last option creates a new archived-data serving surface and constrains
retention: a world cannot disappear before its URL policy permits. Fresh-world
behavior and object-ID collisions need explicit tests before one signing
lifetime is selected. It cannot preserve a pending PUT by writing into the
archived world: archives are immutable. Uploads must instead be invalidated,
drained before cutover, or transferred under separately specified semantics.

### 11.3 Background work

Only an active world should run its orphan sweep. The supervisor needs a
cancellable task handle per generation/world. Cutover must stop or fence the
old sweep before taking its final live delta; archived worlds must not mutate
in the background.

### 11.4 Engine-owned tables

Unchecked function SQL is validated after the hidden blob table is created.
Before promising generic state preservation, the implementation should test
whether user escape SQL can address engine-owned tables and decide whether to:

- forbid it at validation;
- include every engine table in the logical transfer contract; or
- expose a deliberately versioned internal-state ABI.

Silently ignoring hidden writes would make rebase incomplete.

### 11.5 External effects

Future effects must not run during baseline creation or state transfer. The
rebase operates on materialized durable outcomes only. A candidate build needs
an explicit “no external handlers” execution mode before effectful language
features arrive.

## 12. Atomic project-generation activation

Shadow construction solves destructive schema work but introduces a race:
the active server may accept writes while a candidate is being prepared. A
snapshot taken at the beginning is stale by cutover time.

“Stable source revision” must mean a captured input bundle, not merely stable
paths: `spock.toml`, `.spock` bytes, every `file(...)` asset byte, any declared
Uhura/provider inputs, and any generated contract inputs used by the build.
Today `engine::seed_file` rereads an asset path during materialization; a dev
host must instead provide captured bytes or retry against a proven complete
bundle. Otherwise a candidate can combine old source with a newer asset.

Every authoritative mutation pathway must advance one transactional
`live_write_revision`: GraphQL/REST writes, raw function SQL, cascades, upload
metadata and blob commits, and background sweeps. If any path can commit
without participating in that revision/barrier protocol, the host cannot
prove that a preservation candidate contains every committed tail write. The
implementation therefore needs one central commit/revision seam or a complete
proof that every transaction entrypoint participates; scattered best-effort
counters are insufficient.

The initial protocol can favor correctness over zero pause. Its admission
barrier applies to same-world behavior swaps too: an old mutating function
must not start before the swap and commit its old body after the new behavior
is advertised.

1. Capture one stable project source revision spanning Spock and, when
   configured, Uhura; assign it a monotonic ID.
2. Compile/check all artifacts off-path.
3. When the state/seed decision requires a new world, materialize its new seed
   baseline and perform most candidate work off-path; the exact-world reuse
   path does not rematerialize an unchanged seed.
4. Close admission to new backend mutations and pause the old world's
   background tasks.
5. Drain in-flight mutations under a generation write barrier.
6. For preservation activation, capture the final live write revision/delta,
   or verify that the prepared delta is current. For explicit fresh
   activation, checkpoint the old world for recovery instead.
7. Apply/reapply the preservation delta when relevant and run all candidate
   validations.
8. Atomically replace one `Arc<ProjectGeneration>`-like active pointer.
9. Resume requests and broadcast one generation event.
10. Cancel retired generation tasks, while retaining the old world according
    to policy.

The current single serialized SQLite connection makes a short quiescent
barrier feasible for an initial implementation. If rebase time becomes
visible, later designs can snapshot early and catch up a tail changeset. The
barrier cannot be skipped: otherwise a request may commit to the old world
after its delta was captured and disappear from the activated one.

Old reads may finish against their captured generation and carry its
generation ID; all old mutations and internal jobs must drain before the new
generation becomes active. As soon as source revision `N+1` is observed,
unfinished revision `N` is permanently ineligible to activate, even when
`N+1` later rejects. This is the “newest revision wins” rule used by the
current Uhura host.

Long-lived SSE connections do not hold a mutation permit forever. They receive
a generation-change event and reconnect. An `Arc` swap cannot atomically
replace JavaScript already executing in a browser, so client coherence is a
handshake:

- HTML, provider code, and boot artifacts are generation-addressed or
  content-hashed;
- integrated Uhura provider/API calls carry the generation ID they booted
  under; and
- a mismatched backend rejects them with a development-only
  generation-changed response that triggers reload.

SSE makes the reload prompt, while request fencing makes old-client/new-backend
mixing safe during the eventual browser transition.

When a client is configured, integrated Uhura Play advances only after the
backend candidate and state decision activate. Uhura Editor may independently
display a newer static preview and diagnostics, but it must label integrated
Play as running an older project generation when Spock is rejected or
state-blocked.

## 13. Observable development UX

The host should publish a structured state machine such as:

- `building` — candidate for source revision N is off-path;
- `active_reused` — revision N activated against the existing world;
- `active_rebased` — revision N activated against a newly rebased world;
- `active_fresh` — revision N activated with a fresh baseline;
- `rejected_source` — source invalid; last-good still serves;
- `blocked_state` — source valid, but preservation is ambiguous or invalid;
- `fresh_available` — a valid fresh baseline can be explicitly activated;
- `superseded` — newer source made this candidate ineligible; and
- `cold_invalid` — no valid generation exists yet, but diagnostics UI may
  still be available.

Every status includes at least:

```text
latest_source_revision
active_generation_id?
active_source_revision?
active_world_id?
state_abi_hash?
state_outcome: reused | rebased | fresh | unavailable
diagnostics[]
conflicts[]
```

Illustrative terminal/UI messages:

```text
revision 18 active — backend behavior updated; world 7ac2 reused

revision 19 active — state rebased into world b31d
previous world 7ac2 retained

revision 20 blocked — running revision 19
post.title was edited both by live state and the new seed
[inspect] [activate fresh] [keep current]

revision 21 rejected — running revision 19
backend/app.spock:18: ...
```

Potential commands are deliberately not accepted here, but the model can
support:

```text
spock dev --fresh
spock state list
spock state inspect <world>
spock state restore <world>
spock state clean
```

“Reset” should create and activate another world, not immediately delete the
only previous copy. “Restore” likewise forks an archived snapshot into a new
descendant world; it never reopens immutable history for mutation.

## 14. Crash recovery and retention

World metadata must be registered with a crash-safe state machine. Candidate
files use unique world-specific names and are never renamed over an open SQLite
database. SQLite explicitly warns that unlinking or renaming an open database
file can cause undefined behavior and corruption; use supervisor pointer
activation, not filesystem replacement. See SQLite's
[corruption guidance](https://www.sqlite.org/howtocorrupt.html).

Generation and world lifecycles are separate:

```text
ProjectGeneration:
  building -> eligible -> active -> retired
        \-> rejected/superseded

World:
  preparing -> validated -> active -> archived -> deleted
        \-> abandoned
```

A behavior-only activation retires one project generation while its world
remains active under the next generation. Archival closes/checkpoints a world;
restore clones an archive into a new descendant and activates it only with a
compatible current contract or the matching persisted project artifacts.

Durable activation needs an ordering rule under the cutover barrier:

1. finish and sync the validated candidate world and metadata;
2. durably record the selected active world/generation reference;
3. install the in-memory active-generation pointer; and
4. only then admit writes and acknowledge the new generation.

On startup, `preparing` or incomplete worlds are abandoned and the durable
pointer selects the last committed world. This alone does **not** recreate a
last-good project generation when current source is invalid: that would also
require a versioned persisted artifact bundle containing the compiled Spock
contract, route/link metadata, generation ID, signer/world reference, and,
when a client is configured, matching Uhura Play assets. Until such a bundle
exists, crash recovery promises world-file safety and a rebuild from current
valid source, not cross-restart last-good serving.

WAL/SHM files must be checkpointed/closed or captured through the backup API
rather than copied naively. A per-project supervisor lock must prevent two
`spock dev` processes from racing world registration, cleanup, signer/ledger
state, or active selection.

Retention needs explicit bounds:

- location, likely below `.spock/dev/`, which an implementation must add to
  `.gitignore` and exclude from its source watcher;
- maximum bytes across baselines, live databases, archived worlds, WAL files,
  and blobs — not merely a world count;
- LRU versus age policy;
- protection for the active and immediately previous worlds;
- request/task leases or reference counts that prevent closing/deleting a
  world while an admitted request, signed-URL route, or background task still
  uses it;
- cleanup of abandoned candidates and blob data; and
- whether restore survives stopping/restarting `spock dev`.

“Retain every world” is safe semantically and unsafe operationally. The RFD
must not leave disk growth unbounded.

## 15. Phased investigation and delivery

### P0 — model and fixtures

- Land this study without changing runtime behavior.
- Build a contract-diff fixture corpus and expected classifier results.
- Specify source revision, project generation, world, baseline, and conflict
  data structures on paper or as test-only models.

### P1 — coherent last-good generations

- Add stable project snapshot capture and newest-revision ordering.
- Build complete backend candidates off-path.
- Keep the current generation serving on source failure.
- Use fresh shadow worlds only; make no preservation claim.
- Track a conservative `dirty_since_baseline` bit through the same central
  commit seam required by §12. If any mutation may have occurred, require
  explicit fresh activation rather than switching silently; uncertainty means
  dirty.
- Archive rather than destroy the previous world during activation.

### P2 — same-world fast path

- Define canonical behavior, state ABI, and seed fingerprints.
- Reuse the active world when exact state/seed compatibility is proven.
- Move signer and background-task ownership to the appropriate session/world
  boundary.

### P3 — exact baselines and seed identity

- Persist baseline snapshots, including hidden storage state, and live-write
  revisions.
- Decide seed policy and prototype the mint ledger or explicit-key boundary.
- Detect whether the live delta is empty.

### P4 — storage and world correctness foundation

- Prove committed/pending blob behavior, signed URL policy, and sweep fencing.
- Add world list, inspect, forked restore, reset, leases, and bounded cleanup.
- Keep automatic rebase disabled for storage-active contracts until these
  invariants pass.

### P5 — conservative logical rebase

- Support exact-name/type/key mappings and additive nullable/defaulted fields.
- Implement field/row three-way rules and structured conflicts.
- Validate constraints and activate only all-or-nothing candidates.
- Include storage-active contracts only after P4 proves their complete state
  transfer.

### P6 — optimize only after semantics hold

- Measure logical full diff and cutover pause.
- Evaluate SQLite Session or a tail journal against the pinned SQLite build.
- Consider copy-on-write/checkpoint improvements.
- Add explicit rename maps or type conversions only when real projects demand
  them.

[RFD 0022](0022-spock-framework.md)'s combined `spock dev` should not be
treated as complete before at
least P1 and P2. Whether P5 is required for the first public release is an
open product decision; it should not delay the correctness model itself.

## 16. Required experiments and conformance matrix

The design should be attacked with at least these cases:

### Candidate ordering and recovery

- valid -> invalid -> valid source;
- rapid saves where older work finishes last;
- crash at each world lifecycle transition;
- crash between durable active-pointer recording and the in-memory swap;
- first source revision invalid with diagnostics UI still reachable; and
- fork/restore the previously active world under compatible and incompatible
  current source.

### Same-world changes

- docs-only edit retains every row and blob;
- ordinary read/mutating function body edit retains state;
- an old mutating request spans a same-world behavior swap;
- function signature/GraphQL shape changes without DB changes;
- validator function edit is classified as physical; and
- behavior reload does not invalidate a signed storage URL unless policy says
  it should.

### Schema transfer

- add table;
- add nullable field;
- add fields with literal, `auto`, `now`, and `me` defaults;
- add required field without a default;
- new UNIQUE/CHECK/FK passing and failing existing rows;
- reorder fields;
- remove empty versus live-populated field/table;
- table/field rename;
- key shape/type change;
- reference target and on-delete change;
- scalar type changes; and
- composite-key rows;
- cyclic and self-referential foreign-key graphs; and
- raw SQL values with wrong SQLite storage classes or non-canonical
  bool/UUID/timestamp encodings.

### Seed three-way cases

- seed-only update with live row unchanged;
- live-only update with seed unchanged;
- disjoint seed/live field updates;
- conflicting same-field updates;
- equal live/seed field updates followed by another seed edit;
- runtime insert colliding with new seed insert;
- equal independent runtime/seed inserts followed by a seed deletion;
- runtime delete versus seed update;
- explicit-key seed row;
- bound auto-key seed row;
- unbound auto-key seed row;
- binding rename and mint-ledger recovery across process restart;
- `now` default stability;
- unchanged/changed/missing `file(...)` asset; and
- an asset changing while a candidate snapshot is captured.

### Runtime side effects on state

- raw function SQL changing several tables;
- raw SQL changing a primary key;
- `cascade`, `set null`, and `restrict` outcomes;
- storage sweep mutation;
- committed and pending uploads;
- signed URL object-ID collision across fresh worlds;
- metadata/blob mismatch rejection; and
- attempted access to hidden engine tables.

### Cutover races

- write begins before barrier and commits before cutover;
- write waiting on the old connection when barrier begins;
- new request arrives during activation;
- old background sweep ticks during activation;
- old browser/provider request reaches the new generation; and
- no committed tail write disappears.

### Packaging and portability

- pinned bundled SQLite version, not only the newest documentation;
- `rusqlite` Session feature on every npm release target if adopted;
- in-memory and file-backed worlds;
- two `spock dev` hosts attempting to own one project;
- generated `.spock/dev/` changes never trigger the source watcher;
- archived-world cleanup waits for request/task leases;
- world cleanup after abnormal termination; and
- disk quota behavior with large blobs.

## 17. Decisions required

This study intentionally leaves these open:

1. Is automatic state rebase an eventual product requirement, or only a
   candidate optimization over non-destructive worlds?
2. Which seed policy from §9.2 applies during `dev`?
3. Are bindings required for continuity of auto-keyed seed rows, or does the
   language/IR gain stable seed IDs?
4. Does a failed rebase keep last-good by default, or activate a retained fresh
   world with a prominent notice?
5. What is the initial compatible type/change lattice?
6. What counts as live data loss for a removed field whose values are all null
   or equal to defaults?
7. Are explicit rename mappings ever supported, and where would they live?
8. What pause budget is acceptable for a quiescent cutover?
9. When do SQLite Session changesets become worth their build and complexity
   cost?
10. Where are worlds stored, how long do they survive, and what are the default
    count/byte limits?
11. What happens to pending uploads and outstanding signed URLs at fresh-world
    activation?
12. Can a dependency-aware linker prove that a client-only candidate may reuse
    the existing backend artifact and world without rebuilding them? The
    result would still publish as one atomic project generation.
13. Is the world/rebase model a SQLite-only host feature or a runtime-neutral
    contract future backends must implement?
14. What subset, if any, should later graduate from an RFD into normative
    `spock dev` specification?
15. Does equal-value convergence intentionally erase live provenance, does it
    conservatively conflict, or does each world persist a logical overlay?
16. What raw-value equality and candidate-loading laws cover floats,
    canonical scalar encodings, and cyclic foreign keys?
17. Is last-good recovery only in-process, or are complete versioned project
    generation artifacts persisted for restart recovery?
18. What exact captured-input interface prevents seed assets and subsystem
    files from changing underneath a candidate build?

## 18. Working conclusions

This document recommends discussion around the following, without marking them
accepted:

1. Match Uhura's last-known-good publication semantics before promising state
   migration parity that Uhura itself does not claim.
2. Never perform candidate schema work against the active database.
3. Make non-destructive worlds the correctness and rollback primitive.
4. Separate behavior/API, state ABI, seed, and runtime ABI fingerprints.
5. Reuse the current world for exact state/seed-compatible edits.
6. Model preservation as a three-way baseline/live/new-baseline rebase.
7. Treat rebase as all-or-nothing and optional; fresh worlds always remain the
   fallback.
8. Solve stable seed identity before claiming seed-aware continuity.
9. Treat blob bytes, signer lifetime, background tasks, and tail writes as
   first-class state.
10. Specify logical conflict laws before selecting SQLite Session or another
    optimization.
11. Publish the Spock backend and state world, plus integrated Uhura Play when
    configured, as one project generation.
12. Keep production migrations explicitly outside this work.

The central idea is not “migrate on every save.” It is:

> Build every risky revision as another world. Reuse state when identity is
> exact; rebase it only when compatibility is proved; keep the last good world
> whenever either claim fails.

## 19. References and related work

### Repository documents

- [RFD 0006](0006-language-identity-ir-first.md) — static runtime and IR
  reload architecture.
- [RFD 0009](0009-roadmap.md) — original disposable `spock run --watch`
  sketch.
- [RFD 0015](0015-studio.md) — Studio boundary; explicitly not a
  migration/ops tool.
- [RFD 0018](0018-storage.md) — disposable storage substrate and signer
  assumptions.
- [RFD 0022](0022-spock-framework.md) — Spock framework project, command, and
  combined host study.
- [v0 specification](../spec/v0.md) — current fresh materialization and seed
  semantics.
- [Uhura RFC 0002](https://github.com/gridaco/uhura/blob/42ece8e3c44efe89d3c9417761504e7b190db230/docs/rfcs/0002-model-driven-editor-live-updates.md)
  — saved-source capture, stale publication, and Play migration non-goal.

### External primary sources

- [SQLite ALTER TABLE](https://sqlite.org/lang_altertable.html) — direct and
  generalized SQLite schema-change procedures.
- [SQLite Session extension](https://www.sqlite.org/sessionintro.html) — keyed
  row changesets, conflicts, and same/compatible-schema scope.
- [SQLite changeset apply](https://www.sqlite.org/session/sqlite3changeset_apply.html)
  — target compatibility and conflict behavior.
- [SQLite Online Backup API](https://www.sqlite.org/backup.html) — consistent
  database snapshots.
- [SQLite integrity and foreign-key pragmas](https://www.sqlite.org/pragma.html)
  — distinct validation responsibilities.
- [SQLite corruption guidance](https://www.sqlite.org/howtocorrupt.html) — open
  database file, WAL, and filesystem replacement hazards.
- [Prisma schema prototyping](https://www.prisma.io/docs/orm/prisma-migrate/workflows/prototyping-your-schema)
  — desired-schema push without migration history and explicit data-loss
  boundary.
- [Neon branching overview](https://neon.com/docs/guides/branching-intro) —
  isolated database states as development environments.

# RFD 0018 — Storage v0: files as governed rows

Status: accepted; implementation on the `storage-v0` branch. This RFD is
**decision-only** — it adjudicates the forks and cites RFD 0017
(`0017-storage-research.md`) for all research and prior art. It continues
RFD 0017 §6's open questions and closes them for v0.

## 0. Where this fits

RFD 0009 carries storage only as an implicit side system; the plan of record is
the filter RFD next. Storage jumps that queue on the same rule fn v2 and value
constraints did — dogfood evidence (the Instagram avatar punt) plus a written
acceptance test: **G16** (RFD 0005) — *avatar upload: signed URL → upload →
attach → orphan cleanup*. The filter RFD remains the immediate next after this.

The doctrine this delivers on: *"Storage is built in, so files are linked,
queryable, governable entities rather than detached blobs"* (README). The build/
borrow/buy decision is already taken: **build** the IR + a native signed URL
now; **borrow** an S3-compatible facade later; **buy** a real object store as a
production backend later (RFD 0017 §5).

## 1. The decisions

### 1.1 Object model — a builtin `storage_object` table (D1)

A file is a **reference to a builtin `storage_object` metadata table**, not a
new scalar or an opaque handle. This is the Supabase `storage.objects` model and
the one that makes files *queried, joined, and governed like rows*. Concretely:

```spock
auth table user {
    handle: text @unique
    avatar: storage_object?     // a plain reference — no new keyword, no `::`
}
```

`storage_object` is a normal table name; `avatar: storage_object?` lowers to an
ordinary `Type::Ref`. So DDL foreign keys, the derived `<t>_<f>_not_found`
error, GraphQL nested-object expansion, and TypeScript emission all apply **with
no new machinery**. This continues the `auth::user → auth table` reversal
(RFD 0014): the vision's `storage::object` becomes a plain ref, and Spock gains
no `::` token and no import grammar.

**Injected, gated on a consumer.** The table is synthesized into the contract
only when a program references it — the same "needs-a-consumer" rule that
registers `spock_actor()` only under an `auth table` (RFD 0014). A program with
no file field has no `storage_object` table and no storage surface. The name is
reserved (**E048** if a user declares it).

### 1.2 The floor is read-only for `storage_object`

The metadata table is **queryable and joinable** but **not writable through the
open floor** — no `insert_storage_object_one` / `update` / `delete`. This is the
one place the design would otherwise re-open the split-brain RFD 0017 §5.4
claims Spock closes: a client that could `UPDATE storage_object` directly could
desync metadata from bytes. Writes flow only through the storage protocol (§3)
and deliberate fns. Mechanically this is a `builtin` flag on the table that
suppresses floor-mutation generation; reads are untouched. Per-row *read*
governance is v1 `policy` — v0's floor reads are open, as everywhere else.

### 1.3 Upload/serve surface — mirror the Supabase Storage HTTP API

The control plane is an **HTTP protocol dialect on the open floor**, sibling to
`/rest` and `/graphql` — not author-written fns. It **mirrors the Supabase
Storage API** so a supabase-js-shaped client is immediately familiar: a signed
upload URL, a signed download URL, and raw byte transfer (§3). This is
"borrow before build" applied to the byte plane: upload/download mechanics are
generic floor operations, not business logic, so they belong on the borrowed
floor. Attach — writing the object id into a domain row — is an ordinary write
and needs no new surface.

### 1.4 Signed URLs — native HMAC (build)

A signed URL is a **native HMAC-SHA256 bearer token** binding **method | object
id | expiry**, with a per-run ephemeral secret. This is the offline-verifiable,
method-bound, time-limited construction of S3 SigV4 presigned URLs (RFD 0017
§1), minus the SigV4 wire fidelity — which, with a real S3 *backend* and the
deferred S3 *facade*, can be borrowed later. The secret dies with the process;
since the DB is also wiped per run, a stale URL points at an id that no longer
exists. `once`-single-use (RFD 0001) is **not** required — the URL is multi-use
until expiry, as S3 presigned URLs are; a re-PUT after commit is independently
refused (§4).

### 1.5 Byte substrate — SQLite BLOB behind a trait (D3, resolved for v0)

Bytes live in a **SQLite BLOB**, behind a `BlobStore` trait. BLOB is the most
self-contained prototype substrate (disposable with the `.db`, no stray files)
and lets the byte write and the `pending→committed` flip be **one SQLite
transaction** — closing the byte/metadata atomicity gap that S3 and Supabase
both leave open (RFD 0017 §5.2). The trait keeps D3 a swap, not a rewrite: a
sidecar-dir or real-object-store impl drops in behind the same interface (the
`FileBackend`/`S3Backend` split Supabase already runs).

### 1.6 Lifecycle — metadata-authoritative, bytes-swept

An object is `pending` at mint and `committed` when its bytes land (the PUT
flips it atomically with the blob write). The **metadata row is authoritative**;
bytes are reconciled to it by an **in-process orphan sweep** covering the two
orphan classes RFD 0017 names: *unattached pending* (minted, never uploaded) and
*unreferenced committed* (uploaded, never attached, or attached-then-detached).
The sweep runs on an in-process ticker — a subprocess `spock sweep` would delete
and recreate the disposable DB, and startup sees an empty DB (both incoherent).

### 1.7 Seed assets — `file("./path")`

Bytes enter storage two ways: the HTTP gate, or the **seed**. A seed field that
references `storage_object` may take **`file("./path")`** — a seed-time asset
load: at load the runtime reads the file (resolved relative to the source
directory), materializes a **committed** `storage_object` (`name` = basename,
`content_type` = mime-guessed, `size`, `checksum` = sha256, `owner` = NULL),
stores the bytes, and the field takes the new object's id. It is the third kind
of seed value beside literals and bindings, and it is **seed-only** — nowhere
else can bytes enter without the gate.

- `file` is a **soft keyword**: special only as `file(` in seed-value position,
  so a field or binding named `file` is unaffected.
- The path is validated at **compile** time (relative, no `..` escape, no drive
  prefix — **E050**) and must feed a `storage_object` reference (**E049**); the
  bytes are read at **load** time, so the contract carries the path, not the
  bytes, and a missing asset fails `spock check`'s full load (fail-closed).
- Consequence: a seed is now a **bundle** — the `.spock` plus its referenced
  assets, resolved from the file's directory.
- Same sweep caveat as any object: a seeded file that no row references is
  unreferenced-committed and will be reclaimed, so seed files are attached in
  practice (`user.avatar`, `media.file`). A standalone bound form
  (`logo = file(...)`) is a future addition.

## 2. The `storage_object` schema

| field | type | notes |
| --- | --- | --- |
| `id` | `uuid = auto` | the key; the value a file ref stores |
| `owner` | `ref <anchor> = me` | **only when an `auth table` exists**; optional (anonymous → NULL) |
| `name` | `text?` | original filename (Content-Disposition) |
| `content_type` | `text?` | set at upload |
| `size` | `int?` | set at upload |
| `checksum` | `text?` | sha256 hex — an **explicit** checksum, not an ETag (RFD 0017 §1) |
| `state` | `"pending" \| "committed" = "pending"` | closed-set (RFD 0013) |
| `created_at` | `timestamp = now` | |

No `bucket`, `path`, `version`, or `metadata jsonb` — buckets and versioning are
out of v0, objects are id-addressed, and columns are explicit (no jsonb type in
v0).

## 3. The protocol (`/storage/v1`, active only when `storage_object` exists)

| Route | Supabase analogue | Behavior |
| --- | --- | --- |
| `POST /storage/v1/object/upload/sign` | `createSignedUploadUrl` | insert a `pending` object (`owner = me`), return `{ id, url }` (signed PUT) |
| `PUT /storage/v1/object/{id}?exp&sig` | `uploadToSignedUrl` | verify; store bytes + flip `pending→committed` (one tx); record size/checksum/content_type |
| `POST /storage/v1/object/sign/{id}` | `createSignedUrl` | return `{ url }` (signed GET, short expiry) |
| `GET /storage/v1/object/{id}?exp&sig` | signed download | verify; 404 unless `committed`; serve bytes + content-type |
| `POST /storage/v1/object` *(optional)* | `.upload()` | one-shot: create + bytes + commit |

## 4. Security

Method-bound (a GET signature never validates a PUT), short expiry checked after
a **constant-time** HMAC verify, tamper-evident. Size capped at the PUT boundary
(413 before any byte is hashed). Content-type recorded at PUT, unconstrained in
v0 (no buckets → no mime allow-list). A PUT to a non-`pending` object is refused
(409), so an upload URL cannot overwrite committed bytes within its window. A
signed URL inherits its minter's authority; where it is minted is where
authorization will live (v1 `policy`).

## 5. What this deliberately does NOT do

Buckets/quotas · multipart/TUS/resumable · versioning/object-lock · image
transforms/variants · public-object + CDN serving · the S3-compatible *facade*
(borrow, later) · a real object-store *backend* (buy, production) · a
`spock_sign_url` author-callable SQL scalar (a later ergonomic for projecting a
URL inside a fn/view) · auto-minting a `url` on every projection (unsound —
serving is explicit) · per-row/actor read governance (v1 `policy`).

## 6. Open questions carried

- **The S3-compatible facade** — the borrow track. Does `spock run` eventually
  speak enough SigV4 + bucket/object semantics that stock S3 SDKs upload against
  it? (RFD 0017 §5.5 — the pivotal later fork.)
- **`storage_object` read exposure** — `owner` expands to the full actor row
  through the ref like any other; hiding stored columns needs the exposure model
  (RFD 0004), unbuilt. v0 exposes all stored columns.
- **The commit↔attach gap** — attach is a separate client transaction; nothing
  binds "these bytes are now referenced" to the commit. Covered by the sweep's
  grace window and the serve-time state check; a language-level "ref target must
  be committed" guard is a possible future tightening.

## 7. Storage is the first *external* surface (RFD 0019)

One honesty gap this v0 leaves open, flagged here and carried to its own design
record: **the byte plane serves from a SQLite BLOB in microseconds and never
fails.** In production those bytes live in a real object store — a `GET` takes
~100ms and can time out or 503. As implemented, storage v0 therefore teaches the
prototype that consumes it two falsehoods — *files are instant* and *files never
fail* — and a client built on those assumptions omits the loading, error, retry,
and reconciliation states production demands.

The correct framing is that **this is not a storage problem**. Storage is merely
the *first* surface backed by an out-of-process service; email, payment, search,
and jobs will share the same latency-and-failure shape. The fix is therefore not
a storage-specific delay but a **universal, DevTools-style latency/fault
simulator** across the whole external plane — off by default, opt-in, emitting
honest wire semantics — so a Spock prototype can be *made* to wear production's
latency and failure on demand.

This needs its own ideation and design and lands **no code here**. It is grounded
in **RFD 0019 — The external plane** (`0019-external-plane.md`), which the
byte-substrate trait (§1.5) and the two-phase `pending → committed` lifecycle
(§1.6) already anticipate. Until then, storage v0 stays instant and infallible by
construction; the simulator, when built, is additive.

## 8. Proposals for later (no code — future design seeds)

Three forward-looking proposals, recorded as design seeds. Each **extends or
reconsiders a v0 decision**; none is decided here, and none of the three is a
storage-only concern — they interlock (8.1 is the prerequisite for 8.2; 8.2 gives
8.3 its richest constraints), and 8.1 and 8.3 land in territory already owned by
RFD 0019 and RFD 0013.

### 8.1 A namespace for the growing builtin vocabulary (`storage.object`)

v0 deliberately made `storage_object` a **flat** table name (§1.1) — no `::`, no
import grammar, LLM-writable — and that was right for a *single* builtin table.
But the external plane (RFD 0019) will add siblings: `email.message`,
`payment.charge`, `job.run`, `search.index`. A flat namespace of reserved
snake_case names (`storage_object`, then `email_message`, `payment_charge`, …)
grows collision-prone, and the reserved-name list (E048 and its future kin)
becomes compiler lore rather than something legible on the page.

**Proposal:** a **dotted namespace** for protocol-owned builtins — `storage.object`
(or `storage::object`). Open questions: dot vs. `::`; whether it is a real module
system or just sugar over a reserved prefix; whether it *stays a plain ref* so all
of §1.1's ref-machinery reuse survives; how it reads in a field type
(`avatar: storage.object?`); and the GraphQL projection (dots are illegal in type
names → `StorageObject`). The v0 reversal (`auth table`, not `auth::user`) sets the
bar: a namespace earns its separator only when flat names actually *fail* — and
that is an **external-plane-wide** decision (RFD 0019), not a storage-local one.
Recorded here because storage is where the first reserved name lives.

### 8.2 First-class well-known file types (`storage.object.image`)

Today every file is an **untyped** `storage_object` — `name`, `content_type`,
`size`, `checksum`. An image's width/height, a video's duration/codec, an SVG's
viewBox: none are captured, so every client re-derives them from the bytes.

**Proposal:** **typed object kinds** — `storage.object.image` carrying
`width`/`height`/`format`, `.video`, `.svg`, … — where the runtime extracts the
kind's intrinsic metadata at upload and stores it as first-class, **queryable**
columns (the differentiator over S3/Supabase: the dimensions are *contract-visible
and governable*, like any other column, not opaque object metadata). This is also
where the instinct *"the backend already validates the file — why not inline the
constraint"* lands: a typed image can carry declarative limits (max dimensions,
allowed formats) the runtime can enforce because it already decoded the bytes
(§8.3).

Design questions: subtype-with-extra-columns vs. a separate builtin table vs. a
discriminator column; the extraction cost and a real image/video **decode
dependency** — whose own failure and oversize modes are themselves external-plane
work (RFD 0019); which kinds earn first-class support first (image — the dogfood's
`avatar` and `media`); and how an unknown/unsupported type degrades (fall back to
plain `object`). Needs §8.1's namespace to name the sub-kind.

### 8.3 Unify value validation over storage objects (the `format` question, extended)

*"Do we have a `format` RFD?"* — yes, effectively. **RFD 0013** resolved the
`format` question RFD 0009 §4 deferred, for **text and numbers**, and it did so by
*rejecting* a named-format vocabulary (`format(email)` hides its rule in compiler
lore — an LLM-writability failure) in favor of **validator `check` fns**
(field- and row-level) plus closed-set types. That is the mechanism to extend —
**not** a new inline format mini-language.

The gap: RFD 0013's field `check` lowers to a SQL `CHECK` over the row's *own*
columns, but a `storage_object` field holds only an **id**. So it cannot reach the
object's intrinsic characteristics (`size`, `content_type`, and — with §8.2 —
`width`/`height`). *"Max 5 MB, `image/png|jpeg` only, ≤ 4096 px"* is therefore
**unsayable at the table tier today** — the exact G13 floor-leak RFD 0013 set out
to kill, re-opened for files.

**Proposal:** let a constraint **reach the object's characteristics**, evaluated
at **upload/attach**. Two shapes, weighed against RFD 0013's own panel:

- **(a) a row `check` on `storage_object`** naming a validator fn over its own
  columns. Purest reuse — and it lowers to a real SQL `CHECK` that fires on the
  `pending → committed` UPDATE (§1.6), since `size`/`content_type` are that table's
  own columns. But it is **global** to all objects, and today users cannot attach
  anything to the builtin table.
- **(b) a ref-reaching field check** — `avatar: storage.object.image check
  avatar_ok` — so different fields impose different limits. More expressive, but it
  **cannot** be a column `CHECK` (SQLite `CHECK` forbids subqueries/joins), so it
  must be enforced at the storage-protocol boundary (a runtime guard at PUT/attach,
  or a trigger), not by RFD 0013's CHECK-lowering.

Open questions: *where* it fires (reject at the signed PUT before commit, or at
attach?) and what the client sees (a `413`/`422` with a derived code, per §4);
whether a *narrow* declarative sugar bounded to intrinsic columns (`size < 5MB`,
`mime in (…)`) is worth it or it stays a `check` fn; and how it composes with
§8.2's typed metadata. This is the natural next **value-tier** increment after
RFD 0013, scoped to storage.

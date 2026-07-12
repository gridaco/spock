# RFD 0017 — Storage: prior art and the component taxonomy

Status: research reference. **This RFD commits to nothing.** It is the survey
the storage *decision* RFD (0018) will cite — how Amazon S3, Supabase Storage,
and the S3-compatible ecosystem actually work, mapped to the decisions Spock
must make. In the tradition of RFD 0005 (the proving ground) and RFD 0011
(the verification-line taxonomy), it is method + reference, not new surface.

## 0. Where this fits

Storage was never a numbered v0.x track — RFD 0009 carries it only as an
implicit side system (`storage · email · jobs · search`), and the plan of
record is the filter RFD next. Picking storage up now jumps that queue, with
precedent (fn v2 and value constraints both jumped it); RFD 0018 will owe the
roadmap a one-line ordering note. The acceptance case is already written:
**G16 — avatar upload** (RFD 0005): *"Signed URL, upload, attach; orphan
cleanup on abandonment. Stresses: storage builtin, two-phase flows, `once`
values."*

Three framing decisions are already taken (session grounding, feeding RFD 0018):

- **D1 — object model:** a builtin `storage.objects` table; a `storage::object`
  field is a *reference* to it (parallels `auth table` / `auth.users`,
  RFD 0014). The Supabase model the README names.
- **D3 — byte substrate:** **deferred.** Production wants dedicated object
  storage; the prototype may use a SQLite BLOB. The contract is
  substrate-agnostic, so this does not block RFD 0018.
- **D6 — scope:** attach + serve + orphan-sweep. Buckets/quotas and processing
  out of v0.

This document exists because storage is the first Spock concern whose substrate
(object bytes) is genuinely new to the runtime, and the right design is not
inventable from first principles — it is well-trodden ground with sharp,
non-obvious edges (non-atomic completion, orphaned bytes, opaque ETags). We
surveyed rather than guessed.

Confidence legend:
- **[V]** triple-verified (3–0 adversarial, primary AWS/Supabase docs)
- **[P]** primary-source, single pass (official docs; not triple-voted)
- **[P~]** secondary/comparison source (blogs; directional)
- **[K]** well-established background (standard behavior; not re-verified here)

## 1. The S3 model — the de-facto standard

**Namespace & addressing.** Buckets contain objects addressed by a **flat key
namespace** — no real directories. "Folders" are an illusion: the `delimiter`
parameter rolls keys sharing a substring (between prefix and first delimiter)
into a single `CommonPrefixes` element that acts like a subdirectory and is not
returned elsewhere. `ListObjectsV2` returns **at most 1,000 keys** per request;
pagination is `IsTruncated` + `NextContinuationToken`. **[V]**

**Object identity, metadata & integrity.** An object carries `Content-Type`,
`Content-Length`, `Last-Modified`, user metadata (`x-amz-meta-*`), a storage
class, and an **`ETag`**. Critically, the ETag is **not a universal content
hash**: for a multipart object it is opaque, of the form
`<md5-of-concatenated-part-md5s>-<part-count>` (e.g. `3858…c11f-9`), detectable
by a hyphen / non-hex chars / length ≠ 32. **ETag cannot serve as an integrity
checksum across single-shot and multipart objects.** **[V]**

**Operations.** `PUT` / `GET` / `HEAD` / `DELETE` / `ListObjectsV2`; **range
GETs** via the `Range` header **[K]**; conditional requests (`If-Match` /
`If-None-Match` on ETag) **[K]**.

**Multipart upload lifecycle.** Three steps: `CreateMultipartUpload` (returns an
**upload ID** — the correlation handle for everything after) → `UploadPart`
(independent parts) → `CompleteMultipartUpload` (S3 concatenates parts in
ascending part-number order; caller supplies the full `{PartNumber, ETag}`
list; out-of-order → `InvalidPartOrder`). Two sharp edges:

- **An initiated upload never expires on its own.** Its parts consume (billed)
  storage until you explicitly `Complete` or `Abort`. **[V]**
- **Completion is non-atomic.** S3 returns a `200 OK` header, then emits
  keep-alive whitespace during multi-minute processing, and **may embed an error
  inside the 200 body**. Callers must parse the body, not trust the status
  line. **[V]**

**Orphan/pending cleanup.** The canonical mechanism is a bucket **lifecycle
rule** with the `AbortIncompleteMultipartUpload` action (`DaysAfterInitiation:
N`). Ordinary object-expiration rules **do NOT** remove incomplete uploads —
they are a separate cleanup axis. In-progress uploads are enumerable via
`ListMultipartUploads`. **[V]** *This is the textbook justification for a
two-phase-upload + orphan-sweep design (G16).*

**Presigned URLs — the load-bearing mechanism for Spock.** A presigned URL is a
**self-contained SigV4 bearer token**. The signature cryptographically binds the
bucket, object key, **HTTP method** (GET download / PUT upload / HEAD), the host
header, and every auth query parameter (`X-Amz-Date`, `X-Amz-Expires`,
`X-Amz-SignedHeaders`, …) **except** `X-Amz-Signature` itself, into a canonical
request. All auth lives in the query string, so it is issuable as a plain URL
with no headers; S3 recomputes the signature and rejects any mismatch —
**offline-verifiable and tamper-evident**. Properties:

- **Time-limited** by `X-Amz-Expires`: min 1s, **max 604800s (7 days)** via
  SDK/CLI; also dies when its signing credential dies (whichever is first). **[V]**
- **Inherits the signer's permissions** — grants no more than the principal who
  minted it could do. **[V]**
- For uploads the payload isn't known at signing time, so the canonical request
  uses the constant `UNSIGNED-PAYLOAD`; the host header must be signed. **[P]**
- **Multi-use until expiry**; a presigned `PUT` **overwrites** any object at
  that key. **[P]**
- Presigned `POST` can additionally enforce **Content-Type match, size range,
  required metadata** as upload-time conditions. **[P]**
- Operators can tighten with condition keys like `s3:signatureAge` and
  `aws:SourceIp` — these can only *shorten* the window. **[P]**

**Access control (how the layers compose).** S3 authorizes by evaluating three
sequential contexts (user → bucket → object) over the **union** of applicable
IAM policies, bucket policies, and ACLs. Rules: **default DENY**; **explicit
DENY always beats ALLOW** (a bucket owner can deny any object regardless of
owner); cross-account needs permission from **both** accounts. Since 2023 ACLs
are **disabled by default** (Object Ownership = bucket-owner-enforced), so the
ACL leg is often inert. **[V]**

**Consistency.** Strong read-after-write for PUTs and deletes, globally, since
Dec 2020. **[K]** **Retention surface** (mostly out of Spock's near-term
scope): object versioning; Object Lock (retention + legal hold = WORM);
lifecycle transitions/expiration. **[K]**

## 2. Supabase Storage — Spock's named reference model

**The core architecture is exactly what Spock wants.** Postgres stores **only
metadata** about buckets and objects; the bytes live in a separate **pluggable
byte backend** behind a storage-abstraction layer with concrete `S3Backend` and
**`FileBackend` (local filesystem)** implementations. "S3-compatible object
storage service that stores metadata in Postgres." **[V/P]**

**`storage.objects` schema** (the metadata row): `id` (uuid PK), `bucket_id`,
`name`, `path_tokens` (text[]), `metadata` (jsonb), `version`, `owner_id`,
`created_at` / `updated_at`. **[P]**
**`storage.buckets`**: `public` (bool), `file_size_limit` (bigint),
`allowed_mime_types` (text[]). **[P]**

**Governance = RLS on `storage.objects`.** Per-object access control is Postgres
Row-Level Security — **the same policy engine as ordinary tables**. This is the
whole "files governed like rows" thesis, realized. The S3-compatible endpoint
offers two auth models: server-side access-key/secret-key, and **user-scoped
credentials that respect the same RLS**. **[P]**

**Upload/serve protocols, all interoperable over the same objects:** standard
HTTP/REST, **TUS resumable uploads**, an **S3-compatible endpoint** (SigV4),
**signed upload URLs** and **signed download URLs**. A file uploaded via TUS is
servable via REST and manageable via S3. **[P]** **Processing:** built-in
on-the-fly image transformation. **[P]** Public buckets are CDN-fronted. **[P~]**

**Two lessons Spock must internalize:**

1. **The metadata row and the byte are NOT atomically coupled.** "Deleting the
   metadata row does not remove the stored object" → orphaned bytes that still
   bill. **[P]**
2. **Supabase treats the storage schema as effectively read-only** and routes
   *all* mutations through the Storage API, because **direct SQL desynchronizes
   metadata from the bytes.** **[P]** A split-brain reconciled by *convention* —
   precisely the seam Spock can close by *construction* (§5.4).

**Even the reference impl ships a subset:** Supabase's own S3 endpoint omits
object versioning, ACLs, Object Lock, SSE, object tagging, and lifecycle. **[P]**

## 3. The S3-compatible ecosystem — "S3-compatible" is a spectrum

The S3 HTTP API is the lingua franca; every provider claims compatibility, but
it "ranges from passes-the-full-test-suite to core-operations-only —
integration testing, not vendor pages, is the only reliable check." **[P~]**
The takeaway for Spock: **the mandatory core is small.** Buckets + objects +
`PUT`/`GET`/`HEAD`/`DELETE`/`LIST` + multipart + presigned + SigV4. Versioning,
Object Lock, ACLs, SSE, and rich lifecycle are **routinely omitted** — clearly
optional.

| Feature | AWS S3 | MinIO | Garage (Rust, 1-binary) | R2 |
| --- | --- | --- | --- | --- |
| Multipart + presigned/SigV4 | yes | yes | yes | yes |
| Object versioning | yes | yes | no (stub) | no |
| ACLs / bucket policy | yes (IAM+policy+ACL) | yes (policy) | no (own key→bucket model) | no |
| Object Lock / WORM | yes | yes | no | no |
| Lifecycle rules | yes (full) | yes | partial (AbortIncompleteMPU + Expiration) | partial |
| Metadata ↔ byte split | internal | inline | `meta/` KV vs `data/` blocks | internal |
| Consistency | strong R-A-W | strong | eventually consistent (CRDT) | strong |

Confidence: Garage row **[P]** (its own S3-compatibility doc); MinIO/R2 **[P~]**.

**Garage is Spock's closest architectural cousin** — a single self-hostable
binary that **physically separates metadata from bytes** (`meta/` table store
vs `data/` content-addressed blocks), keeps only the essential S3 surface, and
**drops versioning, ACLs, and bucket policies** for a minimal permission model.
It proves a credible self-hosted S3 layer can be tiny. Its one anti-lesson:
CRDT eventual consistency — Spock's SQLite metadata is strongly consistent for
free, so Spock is *strictly better positioned* on the metadata side.

## 4. The component taxonomy — what defines a storage layer

Each row maps to a decision RFD 0018 must make.

| Component | S3 | Supabase | → Spock (proposed) |
| --- | --- | --- | --- |
| a. Namespace & addressing | flat keys; fake folders | bucket + `path_tokens` | builtin `storage.objects` keyed by id; path is metadata |
| b. Identity, metadata, integrity | key + opaque ETag; `x-amz-meta-*` | uuid + `metadata` jsonb + `version` | uuid id; `content_type`, `size`, **explicit `checksum` (not ETag)** |
| c. **Metadata ↔ byte split** | opaque | **Postgres meta + pluggable backend** | **THE architecture** — meta table + pluggable byte backend (D3) |
| d. Ingest / upload | presigned PUT / multipart | REST / TUS / S3 / signed-URL | v0: single-shot signed PUT; multipart/TUS deferred |
| e. Egress / serve | presigned GET; range; CDN | signed download; public + CDN | v0: signed GET by runtime; range nice-to-have; CDN out |
| f. Access & governance | IAM+policy+ACL union, default-deny | **per-object RLS (same engine as tables)** | v0: signed-URL-gated; v1: `policy`/RLS (RFD 0004) |
| g. Lifecycle / orphans | never auto-expire; abort rule | **row-delete leaves orphan byte** | **forced:** pending → committed + sweep |
| h. Consistency & atomicity | strong R-A-W; complete non-atomic | **row ≠ byte, not coupled** | metadata-authoritative (fn = 1 txn); bytes swept |
| i. Processing / derivation | none (client) | image transform | **OUT of v0** (effect/extern rung, RFD 0001) |
| j. Durability / replication / backends | internal | pluggable backend | pluggable backend trait; prototype substrate = D3 |

## 5. What this means for Spock (the design pull)

The load-bearing takeaways, ranked:

1. **D1 is validated by everyone.** The metadata-store / byte-store split *is*
   the architecture (S3, Supabase, Garage all do it). Spock's `storage.objects`
   builtin table is the metadata store; the deferred D3 substrate is the
   pluggable byte backend — literally Supabase's `S3Backend`/`FileBackend`
   split. → A **backend trait** is worth having in v0 even with a single impl,
   so D3 is a swap, not a rewrite.

2. **Atomicity is the sharp edge, and it's forced by physics.** *No* system
   atomically couples the metadata row and the bytes — S3's complete returns
   200-with-embedded-error; Supabase's row-delete orphans the byte. So a
   **two-phase (pending → committed) lifecycle + orphan sweep is not optional
   polish; it is the only correct design** (this is exactly G16's "orphan
   cleanup on abandonment"). Spock's **advantage to claim**: the compiler owns
   *both* the object row and the byte path, and a `fn` is one serializable
   transaction — so the *metadata* commit is atomic (the row is there or it
   isn't) and only the *bytes* need eventual reconciliation. Frame:
   **metadata-authoritative, bytes-swept.**

3. **Presigned URL = the `once` mechanism, confirmed.** SigV4 is precisely an
   offline-verifiable HMAC bearer token binding **method + key + expiry**.
   Spock's signed URL is the same construction with the runtime's own HMAC key;
   the signed upload URL is a `once secret` (RFD 0001). Security essentials the
   research forces: **bind the method** (an upload URL must not also read),
   **short expiry**, and validate **content-type/size** at the boundary.

4. **Spock can beat the reference model on the split-brain.** Supabase's row and
   byte desync unless you "route all mutations through the API and treat the
   schema as read-only" — reconciled by *convention*. Because Spock's compiler
   owns the write path, the object row and byte lifecycle can be **co-managed by
   construction**, not convention. A doctrine win worth stating in RFD 0018.

5. **The big new fork: borrow or build the wire protocol?** Should `spock run`
   **speak an S3-compatible API subset** (presigned PUT/GET + SigV4)? Upside:
   every existing S3 SDK/CLI/tool uploads against Spock with **zero client
   code** — the same "borrow before build" move (RFD 0009) that borrowed Hasura
   for GraphQL and PostgREST for REST. Downside: faithfully emulating SigV4 +
   bucket/object semantics is a non-trivial surface. This is the pivotal open
   decision the research surfaced; it deserves its own section in RFD 0018.

6. **v0 can ship a tiny surface, guilt-free.** "S3-compatible is a spectrum" and
   Garage/R2 drop versioning/ACL/object-lock entirely. Spock v0's floor =
   objects + signed PUT + signed GET + sweep. Multipart/TUS, versioning,
   object-lock, transforms all defer without controversy.

7. **Governance maps cleanly onto open-floor-now / policy-later.** Supabase
   governs files by RLS on `storage.objects` — Spock's **v1** target (`policy`,
   RFD 0004). Spock's **v0** floor is **signed-URL-gated serving**: a signed URL
   inherits its minter's authority, so *where the URL is minted* (a view
   projection, a fn return) is where authorization lives. The actor seam
   (`spock_actor()`, RFD 0014) stamps ownership (`owner = me`).

8. **ETag is not a checksum** — for integrity, store an **explicit checksum
   column**; do not reuse the byte-store's ETag.

9. **Multipart/resumable is deferrable.** Single-shot signed PUT is the v0 floor;
   G16 (avatar) never needs multipart.

## 6. Open questions RFD 0018 inherits

- **Borrow or build the wire protocol?** S3-compatible subset (real SDKs work)
  vs a bespoke minimal Spock upload/serve protocol. *(New — §5.5, the pivotal
  fork.)*
- **Byte substrate (D3, deferred):** SQLite BLOB vs sidecar dir vs a backend
  trait with both. The `S3Backend`/`FileBackend` precedent argues for a trait
  even in v0.
- **Atomicity state machine:** pending → committed transitions and the sweep
  trigger — lazy / on-startup / `spock` subcommand. (A durable background job is
  rung-2, deferred.)
- **Two orphan classes:** unattached (uploaded, never finalized) and
  unreferenced (finalized, row later deleted). Sweep must handle both.
- **Governance minting points:** confirm that projecting a `storage::object` in
  a view/fn result mints the signed serve URL, and who may.
- **`once` dependency:** presigned URL ⇒ `once secret` (RFD 0001, still a
  discussion draft). Decide whether storage v0 requires the `once` checker or
  ships the URL as a plain non-persisted return first.
- **Buckets: in or out of v0?** Supabase makes buckets the unit of
  public/private + size + MIME policy. Spock could fold those onto the
  field/type and skip a bucket concept in v0. (Lean: no first-class buckets.)

## Sources

Primary (triple-verified core): AWS S3 User Guide & API Reference — multipart
overview, CreateMultipartUpload, CompleteMultipartUpload, UploadPart,
ListObjectsV2, using-presigned-url, sigv4-query-string-auth,
how-s3-evaluates-access-control, intro-lifecycle-rules,
mpu-abort-incomplete-mpu-lifecycle-config; AWS security blog (IAM/policy/ACL).
Supabase docs — storage/schema/design, guides/storage, storage/s3/compatibility,
the s3-compatible-storage blog, github.com/supabase/storage. Ecosystem
(single-pass): Garage HQ S3-compatibility & internals docs; MinIO / SeaweedFS /
R2 comparison writeups. Upload patterns: AWS Compute blog "Patterns for building
an API to upload files to S3", AWS presigned-URL best-practices PDF.

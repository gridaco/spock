# RFD 0015 — Studio: the human-developer layer

Status: **accepted and implemented**. Four framing decisions were settled with
the author before drafting (§2), and §12 records how implementation resolved
the remaining choices. Studio is *ecosystem tooling* — a local web app over the
introspectable contract — so it rides alongside the language roadmap, never
competing for the differentiator slot. It added one small backend seam (§6);
the app itself remains a pure consumer of an active Spock authority generation,
whether served by `spock run`, `spock start`, or `spock dev`.

The name "Studio" is provisional and fully reversible.

**Implementation status (2026-07-15).** Shipped in the runtime: the two
endpoints (`/~personas`, `/~whoami`) and a working console at `/~studio`, a
Supabase-shaped three-pane shell (rail · object list · work area) with a
**neutral black/white palette** — the Actor persona selector sits where a DB
studio parks its role dropdown. The console evolved through three realizations;
the **current** one is a **Vite + React + TypeScript SPA** styled with
**Tailwind v4 + shadcn/ui** (the Mira / neutral preset `b1D0dv72`, Inter
vendored, lucide icons) and **react-data-grid** for the table view
(`crates/spock-runtime/studio/`). `pnpm build` compiles it to a gitignored
`dist/` that release CI guards and embeds via **`rust-embed`**. Only
`dist/.gitkeep` is committed so a clean checkout still compiles; a useful
source-built Studio requires the SPA build before `cargo build`. It is served
same-origin at `/~studio` (+ hashed assets under `/~studio/{*path}`) — the
console stays fully offline (no CDN). This **reversed the original "no bundler /
cargo-only CI" call recorded in Q1/Q6/§7**: Studio is now a real front-end app with an
authoring-time Node build
(`pnpm build` → `cargo build`); the single-`spock`-binary + offline guarantees
are preserved (the bundle and font embed into the binary). The original MVP
surface (§5.1) is complete; the shipped Studio now also consumes RFD 0021's
server-side filtering, ordering, and offset paging and can create rows through
the compiled GraphQL contract. Updating or deleting existing rows remains
deferred (§5.2).

---

## 0. The question

Spock already publishes everything a human needs to *see* their prototype: the
compiled contract as data (`/~contract`), open reads (`/rest/v1/{table}`), the
deliberate surface (`/rest/v1/rpc/{fn}` + GraphQL), and — since the actor seam
shipped in the runtime (RFD 0014) — a dev-time impersonation knob
(`X-Spock-Actor`). What was missing when this RFD was written was the *console*:
a place a developer opens,
sees the shape of the world
they declared, and **plays it as maya, then as luis, then as nobody** — watching
the same fn answer differently each time.

That last loop is the whole thesis made visible. Everything else Studio does —
browse the schema, inspect rows, run a fn — is table stakes that
Supabase/Prisma/Drizzle Studio already ship. **Impersonation is what makes this
ours.** Studio-style pickers impersonate real auth users under RLS; nobody ships
*seed rows as personas* — zero-auth, wired to a declared identity anchor, fed to
the same runtime seam — out of the box.

The design tension is entirely one of *restraint*: a GUI over a schema is
exactly where a language quietly grows a second source of truth. This RFD's job
is to draw the walls that keep Studio a consumer, and to scope an MVP that proves
the headline without waiting on unshipped language work.

**Why it landed.** The actor seam was invisible to humans and reachable only by
hand-setting `X-Spock-Actor` on a `curl`. A persona switcher was the cheapest
way to make that seam demonstrable and pressure-test its resolution before
`policy` builds on it. Studio did not outrank the filter work on the language
track — it is not language work (§3); RFD 0021 subsequently shipped, and Studio
now consumes its query surface without owning a second filter grammar.

## 1. What already exists to build on

Every render source is already live in `spock run` (verified against the
runtime, July 2026):

- **`GET /~contract`** — the whole compiled contract as JSON: top-level keys
  `spock`, `tables`, `records`, `fns`, `seed`. This is the single render source.
  Per-table it carries `key`, `fields` (with `type`, `optional`, `unique`,
  `default`, `check`), `uniques`, `checks`, `errors[]`, and — the auth signal —
  `anchor: true` on exactly one table (omitted elsewhere). Per-fn it carries
  `readonly`, `params`, `returns`, `errors[]`, `refusals[]`, and the raw
  statement bodies `sql[]`.
- **`GET /~health`** — `{"ok": true}`.
- **`GET /rest/v1/{table}`** (PostgREST-shaped column predicates, `order`,
  `limit`, and `offset`; default 50 / max 200; envelope `{"rows":[...]}`) and
  **`GET /rest/v1/{table}/{id}`** (single-key tables).
- **`POST /rest/v1/rpc/{fn}`** (JSON args) and **`GET /rest/v1/rpc/{fn}`** (read
  fns, query-string args; a `mut` fn → 405).
- **`POST /graphql/v1`** and **`GET /graphql/v1`** (GraphiQL; note its assets
  load from a CDN — blank offline).
- **The actor seam** (RFD 0014, shipped in the runtime):
  `X-Spock-Actor: <anchor-key>` is read per-request on fn/rpc and GraphQL
  execution, canonicalized against the anchor key's type; absent → anonymous →
  `spock_actor()` is `NULL`. `= me` fields
  (`default: {"kind":"actor"}`) are server-stamped and removed from the GraphQL
  insert/update surface.

Studio added **zero** new render sources. It added two tiny *actor* endpoints
(§6) that RFD 0014 §4.3 had already specified as part of the recommended seam
but left unimplemented when the seam's core shipped.

## 2. The four decisions, settled

Before drafting, four open decisions were settled with the author:

1. **Host.** Studio is **served by the `spock` binary at `/~studio`** — a
   Vite-built React SPA embedded with `rust-embed` and served **same-origin**
   (exactly as GraphiQL is served today). It remains one runtime process and
   requires no Node installation for an end user (§7).
   `spock run` → open `http://127.0.0.1:4000/~studio`. This makes the headline
   feature ship *out of the box*; same-origin is the primary browser boundary
   even though the standalone development router also permits local CORS (§7).
2. **Name.** "Studio" is kept as the provisional working name.
3. **Scope.** The MVP is **read + impersonate + run** (§5.1). RFD 0021 later
   supplied filtering, ordering, and offset paging, and Studio added
   contract-derived row creation through GraphQL. Updating or deleting existing
   rows remains deferred (§5.2).
4. **Backend-first.** The two enabling endpoints (`/~personas`, `/~whoami`) ship
   **first**, server-side (§6). `/~whoami` is genuinely *authoritative* — it
   echoes the server's own key-type canonicalization, which a client cannot
   reliably replicate; `/~personas` is a canonical, DRY projection of the picker
   from the anchor table's current rows (§12.2).

A full design panel (as in RFD 0013 §2 / 0014 §2) was deliberately skipped: Wall
3 scopes Studio too small to earn that budget, and the substantive trade-offs
live where they bind — Host in §7 (same-origin vs a `CorsLayer` + a second
process), the persona source and framework in §12. Before Studio, playing a
persona means `curl`-ing `/~contract` and hand-setting `X-Spock-Actor`; after,
it's the console.

## 3. The doctrine walls (non-negotiable)

Three walls, each traced to a prior RFD. Studio is disqualified the moment it
crosses one.

**Wall 1 — Studio is a pure consumer; the contract is the only source of
truth.** Schema lives in `.spock`; the running server's compiled contract is
authoritative; Studio *reads* it, exactly as introspection is meant to serve
"clients, tools, and agents" (RFD 0002 §5). Studio never becomes a second place
schema is defined, edited, or cached-as-truth. It has no hidden store. This is
the same discipline RFD 0002 §2 already imposes on the most insert-shaped
operation in the system — *seeding runs through the contract, not around it* —
so a mere console has strictly less license.

**Wall 2 — Authoring intelligence is welcome, but it emits `.spock` the human
commits — never runtime magic.** RFD 0004 §1 fixes the pattern: "intelligence is
welcome *at authoring time*: the compiler (or an agent) may scaffold proposed
views and fns … the human accepts them into source, where they are explicit
forever. Same pattern as the seed LLM: **intelligence when authoring,
determinism when compiled.**" So any "scaffold this fn / propose this persona"
affordance Studio grows (deferred from the MVP, §5.2) must **write `.spock`
source** for the human to review and commit — it may never mutate a running
world through a side channel or persist design decisions anywhere but source.

**Wall 3 — Borrow, don't build; ride alongside, never compete.** RFD 0009's
third ordering rule is "**language work is the differentiator** … surface
breadth is borrowed shape." Introspection was chosen precisely so tooling comes
"for free" (RFD 0009 §1). Studio is that free tooling made concrete: a web app
over the borrowed, introspectable surface. It must not acquire a design budget
that competes with `fn`, the filter IR, or `policy`. Every screen is a
projection of data the server already emits.

## 4. The contract-consumer contract

What Studio binds to, precisely — and the three things that are **stable
bind-points** (RFD 0009 §2, track 2): the Tier-1 GraphQL/REST names, the
`/~contract` JSON shape, and the derived error-code vocabulary. Studio reads:

| Studio surface | Read from | Notes |
|---|---|---|
| Schema browser | `contract.tables`, `.records`, `.fns` | render `type`/`optional`/`unique`/`default`/`check`; refs as edges |
| Row viewer | `GET /rest/v1/{table}` (+`/{id}`) | list → `{"rows":[…]}` (`limit` cap 200); `/{id}` → bare row object, 404 on miss |
| Fn runner | `contract.fns` → `POST\|GET /rest/v1/rpc/{fn}` | `readonly` picks GET vs POST; `params` build the form; `returns` shapes output |
| Persona switcher | `GET /~personas` → header on every request | the differentiator (§6.1) |
| Actor echo | `GET /~whoami` | confirms the server's resolution (§6.2) |
| Surface ledger | `table.anchor`, `field.default==actor`, `*.errors[]`, `fn.refusals[]` | the v0 slice of RFD 0004 §5 (see §8) |
| GraphQL panel | link/embed `GET /graphql/v1` | borrowed 100% (RFD 0009: "the GraphQL path stays borrowed") |

### 4.1 The honesty finding — where impersonation actually bites

This is the load-bearing correctness point for the whole MVP. The server reads
`X-Spock-Actor` **only on fn/rpc and GraphQL execution.** Plain table reads
(`/rest/v1/{table}`, `/{id}`) never look at the actor, and v0's auto-derived
GraphQL table reads are ungoverned. So switching persona **visibly re-answers**:

- **fns that consult `spock_actor()`** — a guard refuses for a non-owner:
  `archive_post`/`unarchive_post`/`delete_post` succeed for the author and refuse
  (arity-miss → `not_found`) for everyone else and for anonymous.
- **`= me` write-stamping** — an insert with a `= me` column stamps
  `author = maya` server-side; forging it is rejected.

In the v0 instagram dogfood the actor is consulted by exactly those three `mut`
fns (`WHERE author = spock_actor()`) plus the `post.author = me` insert stamp;
**no read fn consults `spock_actor()` yet** — reads still take an explicit
`viewer`/`user` param (G12 unretired) — so the persona switch bites a small,
enumerable set, and the repeatable non-destructive demo is the
`archive_post`/`unarchive_post` pair on a post you own. One caveat on the
write-stamp: no dogfood fn *inserts* a post, so `= me` stamping is exercisable
only through the borrowed GraphQL mutation `insert_post_one` — the panel that is
blank offline (§1); the `spock_actor()` guards carry the impersonation demo
natively.

It **does not** change plain table browsing or auto-derived reads — those are
actor-blind at the floor by design (RFD 0014 §5: the seam is "preparatory, not
protective" in v0; the floor stays ungoverned until `policy`/v1). **Studio must
show impersonation biting on the fn runner and write-stamps, and must not imply
that browsing rows is access-governed.** Faking governance the language does not
yet have would be the exact dishonesty RFD 0004 §1 warns against. Studio's value
here is the opposite: it makes the ungoverned floor *visible* (§8), which
correctly prioritizes closing it.

## 5. Scope — shipped surface, honest about what waits

### 5.1 SHIPPED (read + impersonate + run)

1. **Schema / contract browser** — tables, fields, refs (as a graph), records,
   and fns, rendered from `/~contract`. Each fn shows its signature, its declared
   error set, and its refusals.
2. **Table + row viewer** — `GET /rest/v1/{table}` with server-side predicates,
   multi-column ordering, `limit`, and bounded `offset` paging. The grid remains
   read-only for existing rows; **Add row** derives a GraphQL insert form from
   the compiled contract, including `= me`, references, and file fields.
3. **Persona switcher — the differentiator.** A dropdown populated from
   `/~personas`; selecting a persona sets `X-Spock-Actor` on *every* subsequent
   request. An "anonymous" entry sends no header. `/~whoami` echoes the resulting
   actor. The UI foregrounds *where the switch bites* (§4.1).
4. **Fn runner** — a form derived from each fn's `params`; `readonly` fns issue
   `GET /rest/v1/rpc/{fn}`, others `POST`; the response (and any derived error
   envelope, with its `code`/`kind`/`fields`/`message`) is rendered. Run the
   same fn under different personas and diff the outcomes.
5. **GraphQL entry** — the existing GraphiQL is linked from Studio (§1: its
   assets load from a CDN — blank offline). Borrowed wholesale.
6. **The surface ledger** — the v0 slice (§8): identity anchor, `= me`-stamped
   columns, per-op error/refusal sets, and the ungoverned-floor warning.

### 5.2 DEFER (named, not faked)

Five things wait — the canonical list, with what each blocks on, is §11:

- **Existing-row update/delete** — the grid stays read-only; creation goes
  through the compiled GraphQL insert surface, while broader editing waits on a
  deliberate write UX and protocol boundary.
- **Exact counts and safe keyset cursors** — the shipped pager is bounded
  `offset`, honestly labeled; it does not pretend to be a cursor.
- **Authoring-time scaffold to `.spock`** — additive fast-follow (Wall 2).
- **Live views / effect streams (WS/SSE)** — post-v0.
- **Hostable / tunneled Studio** — local-only for MVP.

Studio makes each gap *visible* rather than faking the capability.

## 6. The two enabling endpoints

Both are additive `~`-meta endpoints next to `/~contract` and `/~health`, and
both came straight from RFD 0014 §4.3, which specified them as part of the
recommended seam but shipped no implementation. They are the only backend
change this RFD introduced: read-only and forward-compatible with the v1 GoTrue
swap.

### 6.1 `GET /~personas` — the picker

The anchor table's rows, projected to the dev picker's shape:

```
GET /~personas
→ 200 [ { "actor": <anchor-key value>, "label": <string> }, … ]
```

- `actor` = the row's **anchor key value** — verbatim what goes in
  `X-Spock-Actor`.
- `label` = the value of the **first unique text field** of the anchor table,
  else the key value. (For the instagram dogfood, `user.username` is the first
  unique text column → the label is the username.) When the anchor has no unique
  text column the label degrades to the raw key — the picker still lets you
  *select* a real actor rather than type one, but authors wanting recognizable
  labels should give the anchor a unique text column, as `user.username` does.
- Rows come from the anchor table's current contents, which begin as the seed
  personas in v0 and reflect later inserts in the active generation. The
  projection is capped at 100 rows and ordered by the anchor key so a large dev
  database cannot blow up the picker (§12.2–3).
- **No `auth table`** → `[]`. Studio then shows "no identity table — impersonation
  unavailable" instead of an empty dropdown that looks broken.

Resolves "pick a known identity, don't type a raw UUID" with no persona-name
mini-grammar — the picker *is* the anchor table's seed rows.

**Write-form feedback (S1).** This endpoint defines a label for actors, but the
compiled contract defines no equivalent presentation field for generic
references. Studio currently uses unique-text → text → key as a heuristic.
Making a generic label authoritative requires an additive contract decision;
the finding and close condition live in
`crates/spock-runtime/studio/FEEDBACK.md` S1.

### 6.2 `GET /~whoami` — the echo

The dev-tier mirror of GoTrue's `GET /user`; a debugging primitive that **never
rejects**:

```
GET /~whoami            (with or without X-Spock-Actor)
→ 200 { "actor": <key|null>, "anonymous": <bool>, "known": <bool> }
```

- `anonymous` = true when **no header was sent** (or no anchor exists) — keyed on
  header *presence*, not on whether the value resolved.
- `actor` = the resolved actor key; `null` when anonymous, or when a header *was*
  sent but its value doesn't parse as the anchor key type.
- `known` = whether the sent key exists as a row in the anchor table. A typo'd,
  wrong-type, or nonexistent value surfaces as `anonymous: false, known: false` —
  it does **not** error. This is why `/~whoami` catches the dogfood's classic
  mistake: sending the *username* (the picker's label) when the anchor key is a
  `uuid` is a present-but-unparseable value that must read as `known: false`, not
  as anonymous. It answers "am I sending the header right?" and "why does my guard
  match nothing?".

### 6.3 Implementation record

- Register both routes in `router()` (`crates/spock-runtime/src/http.rs`).
- `/~personas`: `app.contract.anchor()` gives the table; find its key and its
  first unique text field; `SELECT key, label FROM <anchor> LIMIT N`; project.
  Reuse the existing row-serialization path.
- `/~whoami`: `anonymous` keys on `headers.get("x-spock-actor").is_none()` (or no
  anchor) — **not** on `resolve_actor`'s `None`, because `resolve_actor` collapses
  absent, no-anchor, *and* present-but-unparseable into `None`
  (`crates/spock-runtime/src/http.rs`). When the header is present, canonicalize it
  via the same `path_key_value`; a value that fails to parse → `anonymous: false,
  known: false`; a value that parses → `known = EXISTS(SELECT 1 FROM <anchor>
  WHERE <key> = ?)`.
- Studio calls both endpoints same-origin (§7). The standalone development
  router also permits cross-origin local clients; the combined host owns its
  public transport policy. Both endpoints are read-only and touch no write path.

**Forward-compat (RFD 0014 §9):** `/~whoami` becomes GoTrue's `GET /user` under
v1 auth; `/~personas` becomes a dev-flag-gated seed-persona picker; the
`X-Spock-Actor` header survives as a dev override. Studio binds to the *shape*,
which is stable across that swap.

## 7. Host & architecture

**Served by the binary at `/~studio`, same-origin.** Studio is a Vite + React +
TypeScript SPA under `crates/spock-runtime/studio/`, styled with Tailwind and
shadcn/ui and built to gitignored `dist/`. `rust-embed` places the built JS,
CSS, and font in the native binary, which serves the SPA and its history
fallback at `/~studio`; Node is an authoring/build dependency, never a runtime
dependency. Consequences:

- **Ships out of the box.** No runtime `npm install`, no second process — a
  distributed `spock` binary already contains the console. The Studio assets
  and vendored font are offline; the separately borrowed GraphiQL screen still
  fetches its own assets from a CDN.
- **CORS ownership stays explicit.** Studio itself uses same-origin requests.
  The standalone language server applies permissive local-development CORS,
  including `OPTIONS`, for browser clients on another local origin. The
  listener-free authority router deliberately applies none because `spock-host`
  owns the combined public listener and its transport policy.
- **Dev loop.** `pnpm dev` runs Vite with HMR and proxies the Spock protocol to
  `127.0.0.1:4000`; `pnpm build` regenerates `dist/`, and the following Cargo
  build embeds it. Release CI performs and guards that sequence.
- **Cost, stated plainly.** Studio contributors and release jobs need the pinned
  Node/pnpm toolchain; users still receive a single native process with no Node
  runtime dependency. The framework is justified by the shipped multi-view,
  data-grid, filtering, paging, and contract-derived write surface; it does not
  move authority into the client.

`spock run` prints a startup-banner line advertising `/~studio`, matching the
existing banner style.

## 8. The surface ledger — the v0 slice

RFD 0004 §5 defines the ledger as the complete surface emitted as data —
"role × field × read/write × via (view or fn)" — reviewable as "a table, not an
audit," with a `terraform plan`-style **surface diff** on every change. That full
ledger needs `role`, `view`, and `policy`, all v1. Studio renders the **honest
v0 projection** of it — everything today's contract actually carries:

- **Identity anchor** — which table is the actor (`table.anchor`).
- **Server-stamped identity columns** — every field with `default:
  {"kind":"actor"}` (`= me`): shown as "stamped from the current actor,
  unforgeable on the floor" (RFD 0014 §14.6 — *provenance, not governance*: it
  fills a column, makes no allow/deny decision, filters no row).
- **Per-op outcomes** — each table's `errors[]` and each fn's declared `errors[]`
  with its minted `refusals[]` subset tagged (`spock_refuse` vs constraint-backed;
  `refusals[] ⊆ errors[]`), so "what can this operation throw?" is a rendered
  fact, not a guess (RFD 0002 §1: derived, not guessed).
- **The ungoverned-floor warning** — per identity-bearing table, a visible "⚠
  ungoverned floor write — no guard" row (RFD 0014 §5's recommended dark-write
  ledger), so Studio advertises *no* soundness the floor negates. This is the ledger
  earning its keep in v0: it makes the gap the language still has to close
  **reviewable**.

Two honest caveats: (a) there is **no** `role`/`policy` structure in the v0
contract (RFD 0014 W4/§5/§10), so the ledger's rows/columns are the degenerate
"no roles yet" case; (b) "which fns read the actor" is **not** an authoritative
contract bit — `reads_actor` was deferred as a fragile vocabulary-scan (RFD 0014
§8). Studio *may* substring-scan `fn.sql[]` for `spock_actor(` as a labeled
**heuristic** hint ("appears to read the actor"), but must present it as a
heuristic, never as contract truth. The authoritative signals are the anchor, the
`= me` defaults, and the declared error/refusal sets.

## 9. What this deliberately does NOT do

- **Not a schema editor.** No GUI path mutates `.spock` implicitly (Wall 1).
  Authoring affordances, when they come, emit source (Wall 2).
- **Not an access-control demo it can't back.** It does not gate row browsing by
  persona, because v0 doesn't (§4.1). It shows the floor is open, rather than
  pretending it's closed.
- **Not a client library.** The `spock` npm name now belongs to the framework
  CLI distribution; Studio does not create a second data-layer package or
  claim authority over application data. It may *consume* `spock gen types`.
- **Not a migration/ops tool.** No DB management, no seed regeneration UI (seed
  regeneration is a deliberate authoring-time act, RFD 0002 §2).
- **Not on the language critical path.** If Studio and a language milestone
  contend for attention, the language wins (Wall 3).

## 10. Forward-compat

- The two endpoints survive the v1 auth swap (§6.3); Studio binds to their shape.
- The ledger widens automatically: when `role`/`view`/`policy` land, the same
  screen gains the role and via columns of RFD 0004 §5 — Studio renders whatever
  the contract grows, additively (RFD 0014 §8 keeps the contract additive).
- RFD 0021's filter dialect already widened the row viewer without an
  architectural change. Contract-derived creation now uses GraphQL; future
  update/delete UX can consume a deliberate write boundary without changing
  Studio's authority posture.
- The `X-Spock-Actor` knob is stable; a v1 `Authorization: Bearer` path is a
  second, additive credential source Studio can offer alongside the dev header.

## 11. Deferrals — every one named

1. Existing-row **update/delete** — creation is shipped through the compiled
   GraphQL insert surface; the data grid itself remains read-only.
2. **Exact counts and safe keyset cursors** — filtering, ordering, and bounded
   offset paging are shipped; a deep offset is not presented as a stable cursor.
3. **Authoring-time scaffold** to `.spock` — additive, Wall-2-bound, fast-follow.
4. **Live views / effect streams** — post-v0.
5. **Hostable / tunneled** Studio and framework-host cross-origin policy —
   local-only for MVP. The standalone language server's permissive development
   CORS does not turn Studio into a hosted surface.
6. **`reads_actor`** authoritative bit — deferred by RFD 0014 §8; Studio uses a
   labeled heuristic meanwhile.
7. **Role / policy / view** ledger columns — arrive with v1 governance.
8. **Consuming `spock gen types`** inside Studio — allowed, not required for MVP;
   the SPA can hand-type the few shapes it needs first.
9. **Reference-picker search and paging** — the filter/query layer now supplies
   `offset`, filtering, and ordering, so basic remote lookup is Studio-owned
   implementation work (S2). Exact counts and safe keyset cursors remain the
   protocol findings already recorded in `examples/filter-lab/FEEDBACK.md`.

## 12. Decisions resolved by implementation

1. **Framework.** Vite + React + TypeScript, styled with Tailwind/shadcn and
   embedded with `rust-embed`. This changed the authoring implementation, not
   the one-process/offline runtime boundary.
2. **`/~personas` source.** Current rows from the anchor table, so inserts in the
   running generation are reflected.
3. **`/~personas` cap and ordering.** At most 100 rows, ordered by the canonical
   anchor key.
4. **Startup default.** Always serve Studio and advertise it in the startup
   banner; never open a browser automatically.
5. **Heuristic actor-read hint.** Show a clearly non-authoritative scan for
   `spock_actor(` in function SQL. The contract still does not claim a
   `reads_actor` bit.
6. **CI posture for the Node build.** The pinned Node/pnpm build is accepted and
   required before the Rust build in release CI. The resulting assets embed in
   the binary, so Node remains absent from the user runtime.

## 13. What ships, in one paragraph

A `spock run` server exposes two read-only `~`-endpoints — `/~personas` (the anchor
table projected to `{actor, label}`) and `/~whoami` (`{actor, anonymous, known}`,
never rejects) — and serves a same-origin SPA at `/~studio`. Studio is a pure
consumer of `/~contract`: it browses the schema, inspects rows over
`/rest/v1/{table}` with server-side filters, ordering, and bounded offset paging,
creates rows through the compiled GraphQL insert surface, runs fns over
`/rest/v1/rpc/{fn}`, links GraphiQL, and renders the v0 surface ledger. Its
differentiator is a persona switcher that sets
`X-Spock-Actor` on every request, so fns and `= me` write-stamps re-answer as maya,
luis, or anonymous — the executable PRD, played. It never edits schema, never
gates what the floor doesn't gate, and never competes with the language roadmap:
existing-row update/delete, exact counts, and keyset cursors remain explicit
deferrals rather than invented capabilities.

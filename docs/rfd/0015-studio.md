# RFD 0015 — Studio: the human-developer layer

Status: **discussion draft**. A stance is recommended here; four framing
decisions were settled with the author before drafting (§2), and open questions
for ratification are in §12. Studio is *ecosystem tooling* — a local web app
over the introspectable contract — so it rides alongside the language roadmap
(the filter RFD is still the immediate next language milestone), never competing
for the differentiator slot. It proposes one small, already-designed backend
addition (§6); the app itself is a pure consumer of a running `spock run`.

The name "Studio" is provisional and fully reversible.

**Implementation status (2026-07-12).** Shipped in the runtime: the two
endpoints (`/~personas`, `/~whoami`) and a working console at `/~studio`, a
Supabase-shaped three-pane shell (rail · object list · work area) with a
**neutral black/white palette** — the Actor persona selector sits where a DB
studio parks its role dropdown. The console evolved through three realizations;
the **current** one is a **Vite + React + TypeScript SPA** styled with
**Tailwind v4 + shadcn/ui** (the Mira / neutral preset `b1D0dv72`, Inter
vendored, lucide icons) and **react-data-grid** for the table view
(`crates/spock-runtime/studio/`). `pnpm build` compiles it to a committed
`dist/` that is embedded via **`rust-embed`** and served same-origin at
`/~studio` (+ hashed assets under `/~studio/{*path}`) — the console stays fully
offline (no CDN). This **reverses the "no bundler / cargo-only CI" call in
Q1/Q6/§7**: studio is now a real front-end app with an authoring-time Node build
(`pnpm build` → `cargo build`); the single-`spock`-binary + offline guarantees
are preserved (the bundle and font embed into the binary). The MVP surface
(§5.1) is complete; edit and filter stay deferred (§5.2).

---

## 0. The question

Spock already publishes everything a human needs to *see* their prototype: the
compiled contract as data (`/~contract`), open reads (`/rest/v1/{table}`), the
deliberate surface (`/rest/v1/rpc/{fn}` + GraphQL), and — since the actor seam
shipped in the runtime (RFD 0014) — a dev-time impersonation knob
(`X-Spock-Actor`). What is missing is the *console*: a place a developer opens,
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

**Why now.** The actor seam just shipped in the runtime but is invisible to
humans and unexercised — you reach it only by hand-setting `X-Spock-Actor` on a
`curl`. A persona switcher is the cheapest way to make the just-built seam
demonstrable and to pressure-test its resolution before the filter RFD and
`policy` build on it. Studio does not outrank the filter RFD on the language
track — it is not language work (§3); it rides alongside, and §7/§9 keep it
honestly subordinate.

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
- **`GET /rest/v1/{table}`** (`?limit`, default 50 / max 200; envelope
  `{"rows":[...]}`) and **`GET /rest/v1/{table}/{id}`** (single-key tables).
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

Studio adds **zero** new render sources. It adds two tiny *actor* endpoints (§6)
that RFD 0014 §4.3 already specced (as part of the recommended seam) but left
unimplemented when the seam's core shipped.

## 2. The four decisions, settled

Before drafting, four open decisions were settled with the author:

1. **Host.** Studio is **served by the `spock` binary at `/~studio`** — a
   self-contained page embedded in the binary, served **same-origin** (exactly
   as GraphiQL is served today). *As shipped* it is one no-bundler page
   (`include_str!`); a Vite/`rust-embed` build is the growth path (§7).
   `spock run` → open `http://127.0.0.1:4000/~studio`. This makes the headline
   feature ship *out of the box* and sidesteps the fact that the server sends no
   CORS headers (§7).
2. **Name.** "Studio" is kept as the provisional working name.
3. **Scope.** The MVP is **read + impersonate + run** (§5.1); inline row
   **editing** and table **filtering** are deferred (§5.2), because both depend
   on language work that has not shipped (REST writes; the filter RFD).
4. **Backend-first.** The two enabling endpoints (`/~personas`, `/~whoami`) ship
   **first**, server-side (§6). `/~whoami` is genuinely *authoritative* — it
   echoes the server's own key-type canonicalization, which a client cannot
   reliably replicate; `/~personas` is a canonical, DRY projection of the picker
   (its live-vs-seed row source is open — §12 Q2).

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

## 5. Scope — MVP now, honest about what waits

### 5.1 NOW (read + impersonate + run)

1. **Schema / contract browser** — tables, fields, refs (as a graph), records,
   and fns, rendered from `/~contract`. Each fn shows its signature, its declared
   error set, and its refusals.
2. **Table + row viewer** — `GET /rest/v1/{table}` with the `limit` control; a
   single-row view via `/{id}` for single-key tables. Read-only.
3. **Persona switcher — the differentiator.** A dropdown populated from
   `/~personas`; selecting a persona sets `X-Spock-Actor` on *every* subsequent
   request. An "anonymous" entry sends no header. `/~whoami` echoes the resulting
   actor. The UI foregrounds *where the switch bites* (§4.1).
4. **Fn runner** — a form derived from each fn's `params`; `readonly` fns issue
   `GET /rest/v1/rpc/{fn}`, others `POST`; the response (and any derived error
   envelope, with its `code`/`kind`/`fields`/`message`) is rendered. Run the
   same fn under different personas and diff the outcomes.
5. **Embedded GraphQL** — the existing GraphiQL, reachable from Studio (§1: its
   assets load from a CDN — blank offline). Borrowed wholesale.
6. **The surface ledger** — the v0 slice (§8): identity anchor, `= me`-stamped
   columns, per-op error/refusal sets, and the ungoverned-floor warning.

### 5.2 DEFER (named, not faked)

Five things wait — the canonical list, with what each blocks on, is §11:

- **Inline row editing** — read-only until REST writes land; the writes that *do*
  exist go through the fn runner / GraphQL (correct: the deliberate surface).
- **Table filtering / sort / keyset paging** — `limit`-only until the filter RFD.
- **Authoring-time scaffold to `.spock`** — additive fast-follow (Wall 2).
- **Live views / effect streams (WS/SSE)** — post-v0.
- **Hostable / tunneled Studio** — local-only for MVP.

Studio makes each gap *visible* rather than faking the capability.

## 6. The two enabling endpoints (ship first)

Both are additive `~`-meta endpoints next to `/~contract` and `/~health`, and
both come straight from RFD 0014 §4.3, which specced them as part of the
recommended seam but shipped no implementation.
They are the only backend change this RFD proposes — ~30 lines, no new concepts,
read-only, and forward-compatible with the v1 GoTrue swap.

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
- Rows come from the anchor table's current contents (proposed; live-rows vs
  seed-projection is open — §12 Q2), which in v0 *are* the seed personas
  (RFD 0014: "a persona in v0 is a seed row in the anchor table"). Cap the
  projection (proposed: 100) so a large dev DB can't blow up the picker.
- **No `auth table`** → `[]`. Studio then shows "no identity table — impersonation
  unavailable" instead of an empty dropdown that looks broken.

Resolves "pick a known identity, don't type a raw UUID" with no persona-name
mini-grammar — the picker *is* the anchor table's seed rows.

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

### 6.3 Implementation sketch (for the milestone, not this RFD to fix)

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
- No CORS needed — same-origin (§7). Both endpoints are read-only and touch no
  write path.

**Forward-compat (RFD 0014 §9):** `/~whoami` becomes GoTrue's `GET /user` under
v1 auth; `/~personas` becomes a dev-flag-gated seed-persona picker; the
`X-Spock-Actor` header survives as a dev override. Studio binds to the *shape*,
which is stable across that swap.

## 7. Host & architecture

**Served by the binary at `/~studio`, same-origin.** The `spock` binary already
serves a same-origin browser UI (GraphiQL) from an in-binary HTML shell; Studio
extends that posture. As shipped it is a **single self-contained page** (vanilla
HTML/CSS/JS, no framework, no CDN — fully offline) at
`crates/spock-runtime/studio/index.html`, embedded via `include_str!` and served
at `/~studio`. A Vite/`rust-embed` build is the documented growth path if the
page outgrows one file; a framework was deliberately deferred (Wall 3).
Consequences:

- **Ships out of the box.** No `npm install`, no second process — `spock run` and
  the console is there. That is what shipping the headline feature out of the box
  demands. (One honest bound: the *borrowed* GraphiQL screen still fetches its
  assets from a CDN, so "out of the box" means install-free and single-process,
  not fully offline — vendoring GraphiQL's assets via `rust-embed` is a
  fast-follow.)
- **No CORS problem.** The server sends *no* `Access-Control-*` headers today,
  and the custom `X-Spock-Actor` header plus JSON bodies both force an `OPTIONS`
  preflight that would 404. Same-origin serving avoids the question entirely — and
  avoids adding a security-shaped `CorsLayer` to the language runtime for the sake
  of tooling (a Wall-3 smell).
- **Dev loop.** Edit `crates/spock-runtime/studio/index.html` and rebuild — the
  page is `include_str!`-embedded, so `cargo build` picks it up. No dev server is
  required today; a future Vite build would proxy to `127.0.0.1:4000`
  (server-side — no CORS).
- **Cost, stated plainly.** As shipped there is **no new toolchain**: the page is
  a committed `.html` embedded via `include_str!`, so CI stays cargo-only and the
  binary stays single-file. This is the lightest realization of the host decision
  and best-honors Wall 3. The Node/Vite cost only arrives *if* the page later
  grows into a bundled SPA — a deliberate future choice, not a v0 commitment.
- **Framework.** None (resolved). The console is ~6 views over `fetch` in vanilla
  JS — small enough that a framework would be pure overhead. One can be adopted
  later without changing the host decision. No GraphQL client beyond the borrowed
  GraphiQL.

`spock run` gains a startup-banner line advertising `/~studio`, matching the
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
- **Not a client library.** It does not squat the reserved `spock` npm name
  (that's the future generic protocol client, RFD 0010) and does not ship a
  data-layer package. It may *consume* `spock gen types`.
- **Not a migration/ops tool.** No DB management, no seed regeneration UI (seed
  regeneration is a deliberate authoring-time act, RFD 0002 §2).
- **Not on the language critical path.** If Studio and a language milestone
  contend for attention, the language wins (Wall 3).

## 10. Forward-compat

- The two endpoints survive the v1 auth swap (§6.3); Studio binds to their shape.
- The ledger widens automatically: when `role`/`view`/`policy` land, the same
  screen gains the role and via columns of RFD 0004 §5 — Studio renders whatever
  the contract grows, additively (RFD 0014 §8 keeps the contract additive).
- When REST writes and the filter dialect land, the deferred edit/filter surfaces
  (§5.2) unlock with no architectural change — the row viewer already speaks
  `/rest/v1/{table}`.
- The `X-Spock-Actor` knob is stable; a v1 `Authorization: Bearer` path is a
  second, additive credential source Studio can offer alongside the dev header.

## 11. Deferrals — every one named

1. Inline row **editing** — waits on REST writes (RFD 0009 track 5).
2. Table **filter / sort / keyset paging** UI — waits on the filter RFD (track 3).
3. **Authoring-time scaffold** to `.spock` — additive, Wall-2-bound, fast-follow.
4. **Live views / effect streams** — post-v0.
5. **Hostable / tunneled** Studio, and any **`CorsLayer`** — local-only for MVP.
6. **`reads_actor`** authoritative bit — deferred by RFD 0014 §8; Studio uses a
   labeled heuristic meanwhile.
7. **Role / policy / view** ledger columns — arrive with v1 governance.
8. **Consuming `spock gen types`** inside Studio — allowed, not required for MVP;
   the SPA can hand-type the few shapes it needs first.

## 12. Open questions (for ratification)

1. ~~**Framework.**~~ **Resolved:** the MVP shipped no-framework (vanilla,
   `include_str!`-embedded) — the smallest, fastest, most Wall-3-aligned option.
   Revisit only if the page outgrows a single file.
2. **`/~personas` source — live rows vs seed projection.** Proposed: the anchor
   table's *current* rows (reflects inserts during a session). Alternative: only
   `contract.seed` rows (stable, but stale after inserts). (Lean: live rows, capped.)
3. **`/~personas` cap and ordering.** Proposed cap 100; ordering by key. Is a
   deterministic order (seed order) worth preserving?
4. **Startup default.** Should `spock run` print/open `/~studio` by default, or
   behind a `--studio` flag / a `spock studio` subcommand? (Lean: serve always,
   advertise in the banner, never auto-open a browser.)
5. **Heuristic actor-read hint.** Is the `fn.sql` substring scan (§8) worth
   showing at all in v0, given RFD 0014 deliberately deferred `reads_actor` as
   fragile? (Lean: show it, clearly labeled "heuristic," because the alternative
   is the developer scanning bodies by hand.)
6. ~~**CI posture for the Node build.**~~ **Moot as shipped:** there is no Node
   build — the page is a committed `.html` embedded via `include_str!`, so CI
   stays cargo-only. This returns only if a bundled SPA is later adopted.

## 13. What ships, in one paragraph

A `spock run` server gains two read-only `~`-endpoints — `/~personas` (the anchor
table projected to `{actor, label}`) and `/~whoami` (`{actor, anonymous, known}`,
never rejects) — and serves a same-origin SPA at `/~studio`. Studio is a pure
consumer of `/~contract`: it browses the schema, inspects rows over
`/rest/v1/{table}`, runs fns over `/rest/v1/rpc/{fn}`, embeds GraphiQL, and renders
the v0 surface ledger. Its differentiator is a persona switcher that sets
`X-Spock-Actor` on every request, so fns and `= me` write-stamps re-answer as maya,
luis, or anonymous — the executable PRD, played. It never edits schema, never
gates what the floor doesn't gate, and never competes with the language roadmap:
editing and filtering wait, visibly, on REST writes and the filter RFD.

# RFD 0008 — v0: the table-first slice

Status: accepted decision — v0 is built `table` first, in isolation, on Rust +
embedded SQLite. `fn`, auth, and `view` are deferred to later milestones, not
cut from the language. This RFD records that sequencing decision and banks the
engine and auth research that produced it, so the deferred parts start from
findings rather than from scratch.

## 1. The decision

A fuller v0 was scoped across several forms — `table`, `fn` with SQL-bodied
escapes, borrowed auth, a borrowed read interface, `seed`. A demo in days
forces a smaller first target than even that cut:

- **Build `table` first, alone.** Types, optionals, defaults, `unique`,
  composite keys, refs with declared on-delete. Nothing else on the surface
  until this is real.
- **Rust + embedded SQLite** (`rusqlite`), per RFD 0007 — one static binary,
  no server, no Docker.
- **No `fn`, no auth, no `view` in this slice.** They are the next milestones,
  informed by §4–§5; they are not removed from the language's intended surface
  (RFD 0002).

The through-line from RFD 0005 (§1, IR-first build order): `table` is the
primitive every later concept references — a `view` projects tables, a `fn`
guards writes to tables, a `policy` predicates over table rows, the
derived-error system reads table constraints. Get the primitive that everything
else points at correct before adding anything that points at it. This is the
first segment of the tracer bullet, not a detour from it.

## 2. What the slice must get right

"Table done right" is why the first milestone is spent here: table churn is the
churn that invalidates everything downstream. The slice owns —

- the type set, kept small at first (`text`, `int`, `bool`, `timestamp`,
  `uuid`); optionals as `?`, never NULL-as-a-value (RFD 0005's forced stance);
- defaults (`= now`, `= auto`) and their compile-time vs runtime split;
- `unique` and composite `key` as *semantics*, not index hints (RFD 0005,
  unique-index = semantics);
- refs by naming a table, with delete behavior declared (restrict default);
- the pipeline itself — grammar → AST → checker → SQLite DDL — as a real
  compiler, not a template. RFD 0006's rule holds: the IR is the artifact; the
  SQLite emission is one conformance of it.

Deliberately out of the slice: `state` machines, `derived` counters, partial
or conditional uniqueness (`examples/instagram/v1-FEEDBACK.md`, L1),
write-through provenance (RFD 0003). Each waits for a milestone that needs it.

## 3. Why SQLite is enough now, and why it is not forever

The prior recorded cut (RFD 0002, "The v0 cut") already named embedded SQLite.
This RFD keeps it for the table slice and records why the eventual move to
Postgres is understood and low-risk — so choosing SQLite now is not a corner
painted into.

**SQLite is sufficient for the table PoC.** Real constraints, real DDL, real
errors, zero dependencies, instant drop-and-rebuild — everything the primitive
needs to be exercised end to end.

**Postgres is the eventual engine, for reasons the research made concrete:**

- it is already Spock's oracle for differential testing (RFD 0005) and the
  target of `spock2sql` (RFD 0006), so one dialect spans interpreter, oracle,
  and second implementation;
- the derived-error feature is materially richer on it — real SQLSTATE codes
  and named constraints (as `examples/instagram/pg.sql` exploits) versus
  SQLite's coarse, stringly-typed errors;
- v1's authorization surface (RLS, policies) is Postgres-native.

**The switch is de-risked because "Postgres" no longer implies "Docker."** Two
no-Docker embedded paths were verified to exist:

- **PGlite** — Postgres compiled to Wasm, ~3 MB, in-process, in-memory or
  single-directory persistence (the JS/Wasm path);
- **`postgresql_embedded`** (theseus-rs) — downloads/caches, or compile-time
  bundles, a real Postgres binary and runs it as a managed subprocess (the Rust
  path, the relevant one given RFD 0007).

So SQLite → Postgres is a backend swap behind the same IR, on a runtime that
stays a single self-contained binary either way. The one cost banked by
deferring it: while the interpreter and `spock2sql` both target Postgres, the
cross-engine differential signal RFD 0006 wanted for axis-2 is smaller — a
hardening concern, not a slice concern.

## 4. Auth: recorded, deferred

Auth is out of the table slice, but the research behind the earlier plan is
banked here so the auth milestone starts from findings, not zero.

- **GoTrue (now `supabase/auth`) is a *contract*, not a gateway.** Client
  libraries speak its REST surface; Kong is the gateway, GoTrue is a standalone
  JWT API server behind it. The coupling between auth and the rest of the system
  is exactly two artifacts: the `auth.users` table and the JWT claims (`sub`,
  `role`, `aud`). Everything downstream — RLS, an actor binding in a `fn` body —
  reads only that seam.
- **You mirror the contract; you do not run the binary.** `supabase/auth` is
  Postgres-only (the SQLite/MySQL drivers in its `go.mod` are transitive ORM
  artifacts, not app support), so it could not run on an embedded-SQLite engine
  regardless. No loss: the plan was always to reimplement the ~5-endpoint
  contract (`/signup`, `/token?grant_type=password`,
  `/token?grant_type=refresh_token`, `/logout`, `/user`) against Spock's own
  storage, with `auth.users` as a builtin static table.
- **Dev identity is the same seam, unsecured.** A header selecting the actor
  populates the same claims context a verified JWT later will; downstream code
  is identical whether identity came from a header (dev) or a signed token
  (prod). The swap is one resolver function.
- **The landscape, if the borrow target is ever reconsidered.** SaaS (Auth0,
  Okta, Clerk) can't be self-hosted or embedded — out by construction. Heavy OSS
  servers (Keycloak, Zitadel, Authentik) need a server + DB — out for "minimal."
  The ones that fit the GoTrue niche: **Ory Kratos** (headless REST, SQLite for
  dev, but a chattier flow API), **SuperTokens** (SQLite dev, own contract), and
  **Better Auth** (a TypeScript library that runs *in-process* on your own DB —
  the tightest fit, but TS-only, and its Rust port is too young to depend on).
  The vendor-neutral contract underneath all of them is **OIDC/OAuth2** — the
  right long-term surface, but more to build than GoTrue's password grant, so a
  v1+ direction, not a v0 one.
- **Netlify's original GoTrue is deprecated** (Feb 2025; security-only patches;
  Netlify itself names Supabase Auth the successor). If a GoTrue-shaped contract
  is ever mirrored, mirror the Supabase one.

Ordering consequence, already logged in v1-FEEDBACK (B6, L3): several
account-lifecycle states are only reachable through auth-boundary events. Until
the auth milestone exists, those states stay deliberately out of scope — not
declared-but-unreachable, which the totality linter would reject.

## 5. "No shortcuts" still holds

The cut is to *scope*, not to *method*. The language is still built
language-first — real grammar, real AST, real checker, real compile to real
constraints (RFD 0005, 0006). What shrank is how much surface the first
milestone covers, not how honestly it is built. Borrowing SQLite as the engine —
and later Postgres, a read interface, and auth as engines and interfaces — is the
principle the README's escape-hatch doctrine already implies: own the contract
layer, borrow everything that is not the language.

## Open questions

- **When `fn` enters** — the next milestone after table, and the first consumer
  of a write path. Its body form (a SQL escape, per the README's "escape hatch
  may replace the body, never the contract") is sketched but unresolved;
  `upsert` semantics remain open (v1-FEEDBACK, L2).
- **When the engine flips to Postgres** — before or after `fn` and auth. Earlier
  is cheaper (less SQLite-specific emission to unwind); later keeps the table
  slice minimal.
- **Whether the read interface is borrowed (GraphQL) or native** — deferred with
  the `view` milestone; GraphQL introspection would deliver the "surface as data
  for tooling" goal for free.

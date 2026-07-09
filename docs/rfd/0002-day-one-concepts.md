# RFD 0002 — Day-one concepts

Status: accepted direction. The concepts below are fixed; every syntax
fragment is illustrative and open.

## The atom

A general-purpose language is a language for memory operations. Spock is a
language for **guarded state transitions over durable state**. Every construct
decomposes into that atom:

- `table` — the state space
- `view` — observations of state, per actor; where fields are writable, the
  open surface's writes
- `fn` — named transitions; the deliberate surface
- `role` — the actor taxonomy
- `policy` — the guards: who may fire which transition, under what conditions
- `error` — the declared failure outcomes of a transition
- `seed` — a concrete initial state to walk from
- effects — a transition's external emissions

The linter is logically possible because of this frame. The state space is
closed (declared tables and constraints), the actor space is closed (declared
roles), and the transition space is closed (declared views and fns). In a
closed world, "what can happen here?" is computable — so "what did you
forget?" is computable too. A transaction is a transition that preserves
declared invariants (the C in ACID); the linter is a totality check over
transitions: every derivable outcome must be acknowledged, every transition
must be reachable by some actor, every table must be observable by someone.

## 1. `error`

Carried forward from the vision draft (0000), now fixed:

- Spock defines its own error concept — product-level, not transport-level.
  Not 500-ish: the real, logical outcomes, mostly database constraint
  rejections.
- A `fn` declares what it can throw, Result-style.
- The linter warns when a standard logical error is possible but not
  explicitly acknowledged.

The key property: the error set is **derived, not guessed**. Every UNIQUE,
foreign-key, CHECK, and NOT NULL constraint on a transition's write set is a
possible rejection; every policy guard is a possible denial. The author's job
is to name, message, handle, or explicitly waive each one.

```spock
// illustrative only
error username_taken extends unique_violation(user_profile.username) {
    "this username is already taken"
}

fn change_username(name: username) -> user_profile ! username_taken
```

Lint: `change_username` writes a UNIQUE column, so `unique_violation` is a
possible outcome, so it must be acknowledged. The same move as match
exhaustiveness in languages with algebraic data types.

## 2. `seed`

Fixed: a dedicated seed layer populates the world. Three parts:

1. **Generator.** Compile tables — with their formats and doc comments — to
   JSON Schema with descriptions. Optionally put an LLM in the middle so the
   data makes sense semantically, not just structurally ("maya, a pro
   subscriber with three past orders"), then validate every candidate row back
   through the contract.
2. **Procedure.** Seeding runs *through the contract* — fns and views, not raw
   inserts — so the seed doubles as a validation pass over the contract
   itself.
3. **Determinism.** Generated data is committed to the repo. The LLM runs at
   authoring time, never at run time; regeneration is a deliberate act.
   Prototype runs stay reproducible.

Personas belong to seeds: a seed also declares actors (role plus identity), so
policies can be played immediately.

## 3. `table` and `view`

Fixed: a `table` is the definition itself; a `view` is how one interacts with
it — read and, where fields allow, write.

- Views carry field-level mutability (the vision draft's `mut` fields). A
  writable view field is the open surface's write half: single-row,
  policy-governed, no ceremony. SQL precedent: auto-updatable views.
- `fn` remains the deliberate surface: multi-step, multi-table, effectful.

## 4. `role` and `policy`

Fixed:

- `role` defines the actor taxonomy, extending built-in auth bases (vision
  draft: `role post_author extends user on model::post { check: .author }`).
- `policy` is **not** a SQL predicate. It is a named block of real business
  logic — declared once, referenced by views and fns, composable and testable
  on its own. Deterministic, per actor, per row: the RLS property kept, the
  RLS ergonomics replaced (README, "Policy ergonomics").

Because policies are named guards on transitions, they feed the linter: a
transition no role can fire is dead; a table no role can observe is dark data.
Both are warnings.

## 5. Protocol

Fixed in spirit: a running `spock dev` server is reachable by ordinary
clients. Decision: **boring HTTP + JSON**.

Precedent: GraphQL runs over plain HTTP — a single POST endpoint with a JSON
body — with subscriptions over WebSocket. PostgREST serves schema-derived HTTP
routes. Supabase is both, plus bearer-token auth. gRPC's binary HTTP/2 framing
is browser-hostile and buys nothing at prototype scale.

Sketch of the surface (shape, not final):

- `GET   /view/<name>?<filters>` — read a view
- `PATCH /view/<name>/<id>` — write a writable view field
- `POST  /fn/<name>` — invoke a function, JSON in, JSON out
- `GET   /~contract` — the compiled contract as data: introspection in the
  GraphQL tradition, for clients, tools, and agents
- `Authorization: Bearer <persona-token>` — dev tokens minted from seed
  personas
- WebSocket or SSE for live views and effect streams, later

Tunneling the local server to a shareable URL (cloudflared/ngrok-style) is a
feature on top of this, not a protocol decision.

## The v0 cut

v0 = parse → check → run → serve. One binary.

1. Grammar for `table` / `view` / `fn`, plus minimal `error`, `role`,
   `policy`.
2. A checker with a handful of rules that prove the atom: unacknowledged
   outcome, unreachable transition, dark table, view over a missing field.
3. Engine: embed SQLite in-process. It supplies real constraints — UNIQUE,
   foreign keys, CHECK — which the error system leans on, and real
   transactions, for free. This satisfies the "small runtime before Postgres"
   path; SQLite is an implementation detail hidden behind the language.
4. `spock dev`: the HTTP surface above, with persona tokens.
5. Seeds as committed files; the LLM generator arrives later without changing
   the model.

v0's job is to falsify or validate the atom on one real example (the Instagram
PRD in `examples/`): can real product rules be stated, linted, and played?

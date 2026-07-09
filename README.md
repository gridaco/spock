# 🖖 spock

> It's only logical.

Spock is an early programming language for describing application backends as a
small, inspectable source of truth.

Most application backends spread the same intent across too many layers:
database schema, API serializers, mutation handlers, validation, authorization,
storage rules, background jobs, third-party channels, and tests. That is not a
discipline problem. Teams work hard to keep the code simple, manageable, and
trackable — and it fragments anyway, because the product logic has no single
place to live. Fragmentation is the natural outcome, not the exception.

Spock exists to give that logic a single, first-class home.

## Doctrine

Modern apps are mostly rules:

- what data exists
- what public shape that data has
- who may see or mutate it
- what operations can be called
- what each operation accepts, returns, and affects
- which external systems must stay in sync

Those rules are usually implemented in separate tools that do not understand
each other. A schema migration does not know about the API response. A route
handler does not know if its input shape still matches the public contract. A
validator does not know which model it is really protecting. A storage object
exists somewhere else, even when the product treats it as a real domain entity.

The backing philosophy of Spock is simple:

> The database should be the single source of truth, and mutations should live
> beside the data they change.

Seen through that philosophy, a backend has two kinds of surface:

- **The open surface.** Users act freely on data, and policy decides — per
  viewer, per row — what each of them may see or change. You should not need a
  server full of bespoke routes just to show data: if a client is allowed to
  see something, it should be able to request it through a declared read
  contract.
- **The deliberate surface.** Some operations are intentionally not open. A
  mutation that touches several records, enforces an invariant, or reaches
  external systems is a contract someone chose to define — and it should be
  expressed as one, not as incidental TypeScript wrapped around a database
  connection.

If you know CQRS, you already recognize the cut: the open surface is the query
side, the deliberate surface is the command side. Spock's position is that both
belong in one artifact, beside the data they govern.

The industry has largely solved the open surface. The deliberate surface — and
the seam between the two — is where backends still fragment. The next two
sections make both claims concrete.

## What has been solved

Spock is not trying to rediscover every backend primitive. Much of the ground
it stands on is already solved, and the doctrine is honest about where.

Every product owns the same baseline concerns: identity, file storage, and
third-party channels such as email. Supabase proved those can belong to one
Postgres-backed mental model — auth, storage, database access, security policy,
and generated APIs as one cohesive product surface instead of several
disconnected services.

Terraform proved that infrastructure can be described declaratively: a desired
state in source control, with tooling that can plan and apply the difference.

GraphQL and PostgREST proved that much of the traditional API contract is
really a query problem. The client requests a shape, the server responds with
that shape, and security stays a server-managed property: you see what you
were meant to see.

Postgres row-level security — an SQL feature, though Supabase made it the
default workflow — showed that access control can live with the data itself.
Users act freely on top of guarded tables and views, and the database enforces
what they were meant to see or change. Between client-shaped reads and
data-attached policy, perhaps 95% of the traditional API surface stops needing
bespoke endpoints at all. The caveats are real; they are the next section.

Supabase Storage points at the right model for files: real object storage
linked back into database rows (`storage.objects`), so a file is not merely a
URL a server once returned. Files become logical entities that can be queried,
joined, and governed by the same policy as everything else.

Prisma made database design approachable by giving schema modeling its own
compact language and generating useful tooling from it. Its boundary is just
as instructive: it is strongest as a modeling and ORM generator, and it does
not try to go further.

FlatBuffers is a useful reference for explicit schemas as compiled contracts:
a small declarative source producing concrete runtime artifacts without making
the schema disappear.

## What is still hard

The unsolved part is not plain reads. It is the deliberate surface: the
product logic that sits between data, permission, mutation, and side effects.

**Mutations.** Supabase encourages the right idea — business logic lives in
the database, beside the data it changes. But the deliberate contracts, the
mutations that touch multiple tables and are deliberately not open, must be
written as verbose PL/pgSQL RPC functions. The concept is great; maintaining
the artifact is the pain. So teams escape to a TypeScript service layer over a
database connection, which is friendlier to author but moves the logic outside
the database's isolation envelope and reintroduces the anomalies it had
already solved — lost updates, write skew, check-then-act races — plus a
second policy layer that can drift from the one the data enforces. Neither
path is smooth.

**Views.** A product wants named, stable, public shapes of data. SQL views
express some of that, generated APIs cover the common cases, and GraphQL lets
clients select shapes dynamically. None of them fully answers the
product-facing question: what is this thing, who may see it, and what contract
does the rest of the system depend on? GraphQL deserves a specific note here:
it genuinely solves the dynamic server-to-client contract and client codegen,
but that is the interface axis — a different axis. A perfect query layer still
says nothing about what the product's operations are or who may perform them.

**Sync and nuanced policy.** This is the most important gap. Real products
touch email, payments, storage, notifications, analytics, search indexes, and
other external systems. The important behavior is rarely a single table
update. It is the whole rule: what changed, who was allowed to do it, which
durable state moved, which side effects must happen, and what must remain true
afterward. Today that rule has no home — it lives scattered across handlers
and job queues, if it is written down at all.

Spock targets that gap.

## What Spock is

Spock is a compact language for definitive backend contracts. The surface is
deliberately small:

- `model` — persistent application data; the durable truth
- `view` — named public projections; the stable shapes the open surface reads
- `fn` — deliberate mutations and backend operations; the contracts that are
  intentionally not open

The shape of the bet is familiar: Prisma gave the database schema its own
compact language. Spock extends that move to the whole backend contract —
data, shape, permission, mutation, and sync.

Around that core, Spock is opinionated about the concerns every product owns.
The auth contract is built in, following the proven Supabase-style shape
instead of reinventing identity per project. Storage is built in, so files are
linked, queryable, governable entities rather than detached blobs. Channels —
email and other outbound systems — are planned to join them as explicit
contracts, though that part of the design is not settled.

```spock
// durable truth
model post {
    id: uuid
    title: text
    body: text
    published: bool
}

// public shape
view post_preview from post {
    id: .id
    title: .title
    published: .published
}

// deliberate mutation
fn publish_post(id: uuid) -> post_preview {
    // an explicit backend contract, not incidental glue
}
```

The first useful version should define the core contract: data, public views,
and callable functions. The long-term direction is a logic container that can
be reasoned about, compiled, checked, tested, and presented as data — an
artifact that generators, clients, tooling, and agents can consume, not only a
codebase humans read.

In short: Spock succeeds Supabase's mental model, but with its own compact
language for the parts that are still too verbose, scattered, or implicit.

## Why this is possible

Doctrine without machinery is wishful. Spock's bet is narrower than it looks:
the grammar is new; the machinery is not.

**No new information is required.** Every backend already contains its
contract — smeared across schema, serializers, validators, handlers, and tests
as redundant encodings of one intent. Fragmentation is humans projecting one
definition into n layers by hand. Compilation is the same projection performed
deterministically: state the definition once, derive the layers. Same source,
same artifacts — the property that makes `terraform plan` trustworthy.

**Reads stand on fifty years of theory.** A `model` is a relation schema; a
`view` is a derived relation — Codd's relational algebra, which SQL was built
to encode. Mapping them to DDL, SQL views, and row-level predicates is
mechanical translation, not research. RLS itself is attribute-based access
control expressed as selection predicates the planner appends to every query.

**Writes stand on named patterns.** The deliberate surface looks bespoke, but
its reliability machinery is catalogued: transactional writes, the
transactional outbox, idempotency keys, at-least-once delivery with idempotent
consumers, compensation in the saga style, change data capture for downstream
sync. Teams hand-write these patterns repetitively and imperfectly — exactly
the profile of code a compiler should emit. A `fn` that declares its inputs,
its write set, and its effects gives the compiler everything a careful
engineer uses to write that boilerplate today. The author owns the *what*; the
*how* is known.

**Small languages are checkable.** Spock follows the rule of least power: a
contract grammar, not a general-purpose language. A closed world of models,
views, functions, and declared effects can be checked statically — every view
projects fields that exist, every mutation writes tables it declared, every
effect has a handler, every access path passes through declared policy. No
general-purpose codebase can offer that. The smallness is the feature.

**The precedent is consistent.** Every layer of the stack that stabilized grew
a compact declarative language: relations got SQL, infrastructure got HCL,
client reads got GraphQL SDL, schema modeling got Prisma, wire formats got
protobuf. Supabase stabilized the backend's shape. The next compression is a
language.

## Why this is the right path

Three audiences that rarely agree on anything arrive at the same conclusion
here, each for their own reasons.

### The database professional

Fragmented logic is an update anomaly. Normalization exists because storing
one fact in n places guarantees eventual disagreement; a backend that restates
one rule in the schema, the validator, the route handler, and the client has
reproduced that failure mode over logic instead of data. Spock is
normalization applied to the contract: one authoritative representation per
rule, everything else derived.

The transaction argument is just as classical. The database is the only
component that can run logic under real isolation; a service layer executes
outside that envelope, where lost updates, write skew, and check-then-act
races return. "Mutations live beside the data" is not taste — it keeps every
invariant inside the strongest consistency model the system has. And
data-attached policy is what security engineering calls complete mediation: a
predicate the planner applies to every access path cannot be forgotten by a
new route.

### The startup

Speed compounds where marginal cost falls. In a conventional backend, each
feature pays for a route, a serializer, a validator, authorization glue, and
tests for all of it — plumbing that grows linearly with product surface. Under
a compiled contract the plumbing is derived, and the marginal cost of a
feature approaches the cost of stating it. Supabase's adoption is market
evidence that this compression is real. Spock's aim is to remove the cliff
where it currently ends: the day the product outgrows open reads and someone
starts hand-writing the service layer.

A compact contract is also the cheapest thing to change: small diffs, fast
review, and a definition short enough for a human — or a code-generating
agent — to hold in full. And because the contract is backend-agnostic, the
expensive commitments (engine, hosting, scaling strategy) stay open until they
actually matter. That is option value, not indecision.

### The senior engineer

Spock is designed to be boring underneath. It spends its innovation tokens in
exactly one place — the grammar — and composes proven primitives everywhere
else: relational semantics, RLS, transactions, outbox delivery, change data
capture. That is Gall's law respected rather than fought: a system grown from
a simple system that demonstrably works, not a new universe invented from
scratch.

Two more familiar rules do the safety work. Make illegal states
unrepresentable: a view over a dropped field, a mutation without a permission,
an effect with no handler — compile errors, not incident reports. And
information hiding in Parnas's original sense: the contract hides the decision
most likely to change, the engine, behind the statement least likely to
change, the product's rules. That inversion is exactly what the V1 question
tests.

## V1 direction

Spock is design-stage, and the implementation path is intentionally open.
Three candidate paths are on the table:

1. A deterministic `spock2sql` generation engine: Spock source in; SQL schema,
   views, policies, and RPC contracts out. This fits the doctrine most
   directly, but it may make the prototype expensive too early.
2. No SQL at first: prototype a small database or runtime that proves the
   language model before binding it to Postgres.
3. Closer to systems like TimescaleDB: build the runtime in Rust, possibly
   behind a Wasm boundary, acting as a gateway or database-adjacent execution
   layer.

The v1 question is not "which backend is final?" It is whether Spock can define
data flow, permission, and mutation contracts clearly enough that multiple
backends could implement them.

## Implementation

Spock will be implemented as a real language with a compiler and runtime built
on Rust.

The npm package metadata lives under `npm/` only to reserve the package name.
It is not the primary implementation target.

## Repository Layout

- `examples/` contains product requirements and current-valid Spock examples.
- `docs/rfd/` contains discussion drafts and proposal-only language ideas.
- `npm/` contains package metadata for npm name reservation.

## References and prior work

Spock is inspired by prior work and studies in schema-first, declarative, and
backend-as-platform systems:

- Supabase, for the cohesive backend mental model around Postgres, auth,
  storage, generated APIs, and data-attached policy.
- Postgres RLS, for deterministic access rules attached to data.
- GraphQL and PostgREST, for client-shaped reads over server-governed data.
- Terraform, for declarative desired state, planning, and source-controlled
  operational change.
- Prisma, for compact schema modeling and generated developer tooling.
- FlatBuffers, for explicit schemas that compile into concrete runtime
  artifacts.

Underneath the products sit the older results this doctrine leans on without
ceremony: the relational model and derived views, transaction isolation,
complete mediation, information hiding, and the rule of least power.

## Status

Spock is currently a design-stage proposal. There is no compiler, runtime, or
stable specification yet.

The older, more ambitious draft has been moved to
`docs/rfd/0000-vision.spock`. It is a sketch of possible direction, not the v0
implementation target.

The name and phrase stay:

> Spock. It's only logical.

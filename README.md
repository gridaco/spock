# 🖖 spock

> It's only logical.

Spock is an early programming language for prototyping application backends as
a small, inspectable source of truth.

Most application backends spread the same intent across too many layers:
database schema, API serializers, mutation handlers, validation, authorization,
storage rules, background jobs, third-party channels, and tests. That is not a
discipline problem. Teams work hard to keep the code simple, manageable, and
trackable — and it fragments anyway, because the product logic has no single
place to live. Fragmentation is the natural outcome, not the exception.

Spock exists to give that logic a single, first-class home — as a prototype
language: real enough to run, deliberately not aimed at production. You
validate the product by running its backend, and the artifact that survives is
the spec.

## The picture

Today, one product rule — say, *"a member may publish a draft post"* — is
restated in every layer, and the side systems are synced by hand:

```text
client ──▶ api routes ──▶ service layer ──▶ ORM ──▶ database
           serializers    business logic           schema
           validators     auth checks              RLS
                                │
                                └──▶ storage · email · jobs · search
```

With Spock, the rule is stated once, and the layers — side systems included —
are derived from it:

```text
           app.spock
           table · view · fn
               │
               │  compiles, deterministically
               ▼
client ──▶ database as the single source of truth
           schema · views · policies · rpc
               │
               │  declared effects
               └──▶ storage · email · jobs · search
```

The first picture restates the rule in every box, and each copy can drift.
The second states it once and derives the rest.

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
validator does not know which table it is really protecting. A storage object
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

**How it should feel.** Imagine the database were as unceremonious as memory
and the filesystem: reading a record is reading a property, changing one is an
awaited assignment, and persistence, policy, and transactions simply hold
underneath. Figuratively — if this were TypeScript, every entity would be an
async class behind a getter/setter proxy. (It is not TypeScript; Spock
compiles to real database primitives.) ORMs chased that feeling and broke at
the boundaries, because a library cannot see the whole contract. A compiler
that owns the contract can keep the promise — the old research dream of
orthogonal persistence, taken only as far as it stays practical: infrastructure
disappears from the author's view, not from the system.

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

Protocol Buffers and gRPC proved contracts as compiled artifacts at industry
scale. One compact `.proto` source declares both the data shapes (`message`)
and the callable operations on them (`rpc`), and the toolchain derives the
rest — clients, servers, serialization — in every language, without the schema
ever disappearing. Even evolution is disciplined: fields are numbered,
compatibility is checkable. Thrift, Avro, FlatBuffers, and their kin repeat
the same lesson.

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

**Policy ergonomics.** Row-level security is powerful, but it is not
intuitive, and it is dangerous exactly where it is subtle: a policy that is
slightly wrong does not fail loudly — it quietly leaks data, or quietly hides
it. Worse, RLS thinks in whole rows, so schemas get bent to its mental model:
tables split unnaturally, columns exiled to side tables so a rule can guard
them, storage shaped around access instead of meaning. Security should not
deform the domain. Spock's direction — an idea, not yet a design — is to let
policy be declared at the granularity the product thinks in, columns and
shapes included, and compile it down to whatever mechanism enforces it best.
In relational terms much of that may reduce to views, and that is fine. The
mechanism is not the point; the ergonomics are.

**Sync and nuanced policy.** This is the most important gap. Real products
touch email, payments, storage, notifications, analytics, search indexes, and
other external systems. The important behavior is rarely a single table
update. It is the whole rule: what changed, who was allowed to do it, which
durable state moved, which side effects must happen, and what must remain true
afterward. Today that rule has no home — it lives scattered across handlers
and job queues, if it is written down at all.

Spock targets that gap.

## The LINQ lesson

One prior attempt deserves its own section, because it came closest to the
feel this doctrine is after. LINQ (C#, 2007) integrated queries into the
language itself: a typed query over a remote database, written as if over an
in-memory collection, checked by the compiler, composable like any other
expression. The machinery underneath was genuinely deep — lambdas reified as
expression trees, translated to SQL by a provider at runtime; in theory terms,
monad comprehensions shipped in a mainstream language. Over in-memory objects
LINQ remains one of the most loved features in .NET. The model was right.

The database half stalled anyway — partly technical, partly market, and the
two causes fed each other. (Even the vendor split its own bet: LINQ to SQL was
shelved in favor of Entity Framework within a year of shipping.)

- **The illusion leaked at runtime.** `IQueryable` promised that any C#
  expression was a query, but only an undeclared subset translated to SQL, and
  the type system could not say which. Failures arrived at runtime — an
  exception, or worse, a silent client-side evaluation quietly dragging a
  table into memory. Add the semantic seams (SQL's three-valued NULL logic
  against C#'s two-valued booleans, collation quirks, N+1 loading) and the
  "it is just a collection" promise broke exactly where it mattered.
- **It was read-only.** LINQ made queries first-class and left mutations as
  imperative change tracking. No declared operations, no policy, no effects:
  the deliberate surface stayed outside the model entirely.
- **The contract lived inside one host language.** A query model embedded in
  C# could not be handed to a JavaScript client, a mobile app, or another
  backend. It stayed locked to one runtime in the very years software went
  polyglot — and GraphQL, a strictly weaker query model that was portable and
  contract-first, took the seat instead.

Not a failure of the idea; a failure of the packaging. Each cause maps to a
Spock decision. LINQ's translatable subset was undeclared — Spock is a closed
language, so translatability is total by construction and checked at compile
time. LINQ stopped at reads — `fn`, policy, and effects exist precisely for
the writes. LINQ's contract was trapped in its host runtime — Spock's contract
is the artifact itself, portable to any client and any backend.

## What Spock is

Spock is a compact language for definitive backend contracts. The surface is
deliberately small:

- `table` — persistent application data; the durable truth
- `view` — named public projections; the stable shapes the open surface reads
- `fn` — deliberate mutations and backend operations; the contracts that are
  intentionally not open

The keywords are SQL's own primitives, on purpose. A `table` is a table, a
`view` is a view, a `fn` is a function — Spock declines to invent a "model"
abstraction that would promise more than the database underneath it delivers.
The language adds contract, not distance.

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
table post {
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

## Where the model breaks

A contract language earns trust by naming its own limits. The model breaks
along a gradient — four rungs, each needing a different answer.

**Effects, not algorithms.** Consider minting an API key that is revealed
exactly once. It needs entropy, a hash, and — the interesting part — a return
value that is deliberately *not derivable from stored state*: only the digest
persists, so a database dump cannot leak keys. Nothing here needs a
general-purpose language; it needs effectful builtins and a way to say "this
value exists only in this response":

```spock
// sketch — see docs/rfd/0001, not accepted syntax
fn mint_api_key(name: text) -> { id: uuid, key: once secret } {
    let key = std::crypto::token(32)      // effect: entropy
    let row = insert api_key {
        name: name,
        prefix: key.prefix(8),
        hash: std::crypto::sha256(key),   // only the digest is stored
    }
    return { id: row.id, key: key }       // plaintext exists only here, once
}
```

It does force a doctrine amendment, worth stating plainly: this is the proof
that `fn` is not `view`. Views project durable truth; functions may return
transaction-local values that are destroyed on commit. Password setup, TOTP
secrets, signed upload URLs, and verification tokens are the same family.

**Protocols, not transactions.** A checkout that authorizes a card cannot be
one atomic function: no transaction stays open across an external, fallible,
multi-second call, and commit or abort depends on the external answer. The
industry's answer is a state machine with compensation — sagas — plus durable
timers for the time-shaped cousins: auctions that close at a deadline, trials
that expire, approvals that wait days for a human. Still declarable — states,
steps, and compensations are a grammar — but it is a major language surface,
and the runtime it implies is a workflow engine.

**Algorithms, not contracts.** No declarative grammar will express an
order-matching engine, CRDT merge for collaborative editing, argon2's
internals, video transcoding, or a living tax rulebook. This rung genuinely
requires Turing-completeness, and the answer is the one every surviving
declarative system converged on: a typed foreign-function boundary. SQL has C
extensions; Terraform has providers — the graph stays declarative while
computation plugs in. For Spock, an `extern fn`: the body is foreign (Wasm is
the natural host), the contract is not — signature, write set, and effects
stay declared, and policy still wraps the call. The LINQ lesson supplies the
design rule: LINQ died because its untranslatable subset was invisible until
runtime; `extern` is the same subset made lexically visible. The escape hatch
may replace the body, never the contract. A declarative system lives or dies
by the quality of its escape hatch.

**Other ecosystems, not the backend.** Search relevance, ML inference,
realtime presence, hot-path rate limiting. Spock should interface with these
through declared sync boundaries — the sync pillar — and never absorb them.

One boundary statement falls out: "modern apps are mostly rules" has a domain
of validity. SaaS, marketplaces, commerce, communities — mostly rules, and
Spock's home turf. When the product *is* the algorithm — an exchange, a game
server, a collaborative editor — the center of gravity belongs elsewhere.

There is a second answer to all four rungs, and it reframes the project.

## A prototype language, on purpose

Spock is a real language — parsed, checked, runnable — but it is not aimed at
production serving. It is a prototype language for backends: you build the
backend to prove the product. Run the contract, exercise the policies, call
the mutations, watch the effects fire — against believable data, before anyone
writes production code.

At prototype grade, every rung above dissolves. Entropy and hashing are
builtins. A saga runs its steps in simulated time — the auction closes in
seconds, not days. An `extern fn` is stubbed with scenarios: a `charge_card`
that returns success, decline, or timeout is exactly the fidelity validating a
checkout needs. Other ecosystems are faked at their declared boundary. What
would be a dangerous shortcut in production is the correct fidelity for
validation — which is why, within this domain, coverage is total.

Validation is also how the spec gets finished. Every gap you hit while playing
the prototype is a decision surfaced: a policy nobody wrote, an operation
nobody named, an invariant nobody stated. Fix them and the contract hardens.
What survives at the end is a complete, executable definition of the backend —
what data exists, who may do what, what each operation changes, what must stay
in sync. Call it what it is: an executable PRD. Production teams — human or
agent — build against it, in whatever stack they choose.

The domain is strangely vacant. There has never really been a prototype
language — only diagram grammars, UML and ERD and boxes-and-arrows, which
describe a system without running it. And commercial prototyping tools
prototype the surface: click-through screens, and at their most ambitious a
first-party CMS — records with fields — and it ends there. No permission, no
invariant, no multi-table mutation, no side-effect contract; the backend of
every prototype is imaginary. That is the domain Spock intends to take over
completely.

## What Spock is not

- **Not a production server.** Spock never aims to host your app at scale.
  Production is built against the contract, not on the prototype runtime.
- **Not a diagram.** UML, ERD, and boxes-and-arrows describe a system; Spock
  runs one. What cannot execute cannot validate.
- **Not a no-code builder.** No canvas, no CMS-with-fields standing in for a
  backend. It is a language, with everything a compiler earns you.
- **Not an ORM.** It does not wrap a production database from a host
  language — the LINQ lesson is exactly why. It defines the contract others
  implement.
- **Not a general-purpose language.** The rule of least power is load-bearing:
  what escapes the grammar escapes through a declared boundary, never a
  smuggled one.

## Why this is possible

Doctrine without machinery is wishful. Spock's bet is narrower than it looks:
the grammar is new; the machinery is not.

**No new information is required.** Every backend already contains its
contract — smeared across schema, serializers, validators, handlers, and tests
as redundant encodings of one intent. Fragmentation is humans projecting one
definition into n layers by hand. Compilation is the same projection performed
deterministically: state the definition once, derive the layers. Same source,
same artifacts — the property that makes `terraform plan` trustworthy.

**Reads stand on fifty years of theory.** A `table` is a relation schema; a
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
contract grammar, not a general-purpose language. A closed world of tables,
views, functions, and declared effects can be checked statically — every view
projects fields that exist, every mutation writes tables it declared, every
effect has a handler, every access path passes through declared policy. No
general-purpose codebase can offer that. The smallness is the feature.

**The precedent is consistent.** Every layer of the stack that stabilized grew
a compact declarative language: relations got SQL, infrastructure got HCL,
client reads got GraphQL SDL, schema modeling got Prisma, wire formats got
protobuf. The counter-example teaches the same rule: LINQ embedded the query
contract in a host language instead of giving it one of its own, and the idea
never escaped that runtime. Supabase stabilized the backend's shape. The next
compression is a language.

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

## A stronger paradigm, held loosely

There is a more radical version of the doctrine, recorded here as an idea and
not a commitment: do not declare tables at all. Declare only the shapes the
product actually uses — fields, and how fields relate — and let the compiler
fit the physical schema to them, the way a query planner fits an execution
plan to a query. Storage layout stops being a hand-made input and becomes an
optimization output.

This is less exotic than it sounds. Relational theory has pointed this way
from the start: given attributes and the dependencies between them, normalized
schemas are derivable — schema synthesis rather than schema design. It would
also dissolve the policy problem named above: if a rule needs a table split to
stay enforceable, the compiler makes the split, and the domain the author sees
never deforms.

It is not the v1 plan — `table` stays a first-class declaration. It is the
honest end-state of "state the contract, derive the rest," and it is worth
keeping in view.

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

The prototype-first mission tilts the opening move toward the second path: a
small runtime that makes contracts playable is the product, before any SQL is
emitted. The other paths remain bridges from a validated spec toward
production-grade artifacts.

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
- Protocol Buffers and gRPC, for data shapes and callable operations declared
  in one compiled, evolvable contract.
- LINQ, for typed, composable queries integrated into a mainstream language —
  and for the lessons in where that illusion leaked.

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

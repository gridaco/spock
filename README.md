# 🖖 spock

> It's only logical.

Spock is an early programming language for describing application backends as a
small, inspectable source of truth.

Most application backends spread the same intent across too many layers:
database schema, API serializers, mutation handlers, validation,
authorization, storage rules, background jobs, third-party channels, and tests.
Everyone tries to keep that code simple, manageable, and trackable. The system
still fragments, because the product logic has no single place to live.

Spock exists to make that logic first-class.

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

You should not need a server full of bespoke routes just to show data. If a
client is allowed to see something, it should be able to request it through a
declared read contract. If a change touches multiple records, policies, or
external systems, that mutation should be expressed as a deliberate backend
contract, not as incidental TypeScript wrapped around a database connection.

## What has been solved

Spock is not trying to rediscover every backend primitive.

Supabase proved that a Postgres-backed backend can feel like a cohesive product
surface. Auth, storage, database access, security policy, and generated APIs can
belong to one mental model instead of several disconnected services.

Terraform proved that infrastructure can be described declaratively: a desired
state in source control, with tooling that can plan and apply the difference.

GraphQL and PostgREST proved that much of the traditional API contract is
really a query problem. The client requests a shape, the server responds with
that shape, and security remains a server-managed property. For a large share
of reads, "you may see it if policy says you may see it" is a better contract
than hand-written endpoint plumbing.

Postgres row-level security, popularized in this workflow by Supabase, showed
that access control can live with the data itself. Users can act freely on top
of guarded tables and views, while the database enforces what they were meant
to see or change.

Supabase Storage also points at the right model for files: external objects are
linked back into database rows, so storage is not just a URL factory. Files
become logical entities that can be queried, joined, and governed.

Prisma made database design more approachable by giving schema modeling its own
compact language and generating useful developer tooling from it. Its boundary
is also clear: it is strongest as a model and ORM generator, not as a complete
product logic container.

FlatBuffers is a useful reference for explicit schemas as compiled contracts:
a small declarative source can produce concrete runtime artifacts without
making the schema disappear.

## What is still hard

The unsolved part is not plain reads. It is the product logic that sits between
data, permission, mutation, and side effects.

Supabase encourages the right idea: put business logic close to the database.
But complex mutations often become verbose PL/pgSQL RPC functions. That is a
strong backend contract, but it is not especially friendly to write, review, or
evolve. Teams often escape back to a TypeScript service layer, which is easier
to author but moves logic away from the database and reintroduces concurrency,
locking, transaction timing, and policy drift problems.

Views have a similar tension. A product wants named, stable, public shapes of
data. SQL views can express some of that, GraphQL can expose dynamic client
selection, and generated APIs can cover common cases. But none of those fully
captures the product-facing question: what is this thing, who may see it, and
what contract does the rest of the system depend on?

The hardest gap is sync and nuanced policy. Real products touch email,
payments, storage, notifications, analytics, search indexes, and other external
systems. The important behavior is rarely a single table update. It is the
whole rule: what changed, who was allowed to do it, which durable state moved,
which side effects must happen, and what must remain true afterward.

Spock targets that gap.

## What Spock is

Spock is a compact language for definitive backend contracts:

- `model` declarations for persistent application data
- `view` declarations for public projections
- `fn` declarations for intentional mutations and backend operations
- built-in auth assumptions instead of reinventing identity per project
- built-in storage assumptions instead of treating files as detached blobs
- eventually, explicit channel contracts for email and other external systems

The first useful version should define the core contract: data, public views,
and callable functions. The long-term direction is a logic container that can
be reasoned about, compiled, checked, tested, and presented as data.

```spock
model post {
    id: uuid
    title: text
    body: text
    published: bool
}

view post_preview from post {
    id: .id
    title: .title
    published: .published
}

fn publish_post(id: uuid) -> post_preview {
    // deliberate backend contract
}
```

In short: Spock succeeds Supabase's mental model, but with its own compact
language for the parts that are still too verbose, scattered, or implicit.

## V1 direction

Spock is currently design-stage, so the implementation path is intentionally
open.

The most direct v1 would be a deterministic `spock2sql` generation engine:
Spock source in, SQL schema, views, policies, and RPC contracts out. That path
fits the doctrine well, but it may also make the prototype expensive too early.

A second valid path is to avoid SQL at first and prototype a small database or
runtime that proves the language model before binding it to Postgres.

A third path is closer to systems like TimescaleDB: build the runtime in Rust,
possibly with a Wasm boundary, and let it act as a gateway or database-adjacent
execution layer.

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

## Status

Spock is currently a design-stage proposal. There is no compiler, runtime, or
stable specification yet.

The older, more ambitious draft has been moved to
`docs/rfd/0000-vision.spock`. It is a sketch of possible direction, not the v0
implementation target.

The name and phrase stay:

> Spock. It's only logical.

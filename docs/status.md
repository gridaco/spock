---
description: What is stable, experimental, in progress, and deliberately absent in Spock v0 — plus the project vocabulary.
---

# Project status

Spock is pre-1.0 software, currently 0.5.3; minor versions may break, and the
[changelog](../CHANGELOG.md) records every cut. Spock v0 is a local prototype
runtime for building and studying authority contracts; it is not a production
platform. Its state is disposable by doctrine — the database is rebuilt from
source and seed on every start, and there are no migrations. Its identity seam
is deliberately forgeable — v0 secures nothing. What v0 does freeze is the
contract: the compiled contract shape and the derived error vocabulary are
frozen additively for v0.x, while source syntax may still change between minor
versions.

That is the whole disclaimer. Every page distinguishes shipped behavior from
any explicitly labeled current-source integration that is waiting for a
release; pages otherwise describe shipped behavior in plain terms and point
here instead of hedging inline.

## Legend

| Status | Meaning |
| --- | --- |
| **Stable** | Shipped and specified; frozen where the specification says so. Still pre-1.0. |
| **Experimental** | Shipped behind an explicit preview; may change or vanish in any release. |
| **In progress** | Committed direction, not shipped yet. |
| **Not planned** | Deliberately absent from v0 — a decision, not an omission. |

## Language

| Feature | Status |
| --- | --- |
| Tables, records, keys, unique fields and groups, references, `on delete` actions | Stable |
| Closed-set types and validator `check` fns | Stable |
| `fn` / `mut fn`, SQL escape bodies, read/write polarity | Stable |
| Refusals via `spock_refuse` | Stable |
| Top-level `error` declarations ([RFD 0024](rfd/0024-error-declarations.md) implementation preview) | Experimental |
| Doc comments (`///`, `//!`) carried into the contract | Stable |
| Seed blocks and seed assets (`file()`) | Stable |
| The actor seam: `auth table`, `X-Spock-Actor`, `spock_actor()`, `= me` | Stable — an unverified development seam |
| Verified identity (JWT), `role`, `policy`, per-row governance | In progress |
| `view` (write-through projections) | In progress — design study |
| Native fn statement grammar (beyond the SQL escape) | In progress — design |
| State machines | Not planned (v0) |
| Cross-file programs and modules | Not planned (v0) |
| Partial or conditional uniqueness | Not planned — undecided, so unshipped |
| Curated formats (`email`) and nominal domain types | Not planned (v0) |

## Derived API

| Feature | Status |
| --- | --- |
| Contract JSON at `/~contract`, frozen additively for v0.x | Stable |
| `gen types` (TypeScript) and `gen graphql-schema` | Stable |
| REST reads: lists, by-key, filters, ordering, offset, rpc | Stable |
| REST table writes | In progress — writes are GraphQL-first for now |
| GraphQL Tier 1: single-row reads and writes | Stable |
| GraphQL Tier 2 reads: `where`, `order_by`, `offset` | Stable |
| GraphQL Tier 2 bulk writes: `insert_<t>`, `update_<t>`, `delete_<t>` | In progress |
| GraphQL Tier 3: `on_conflict` upserts, `_inc`, aggregates | In progress |
| GraphQL subscriptions, Relay connections, `distinct_on`, `_like` | Not planned |
| 64-bit `int` over GraphQL `Int` (width gap) | In progress — open conformance gap |

## Runtime and framework

| Feature | Status |
| --- | --- |
| Embedded SQLite runtime (single serialized connection) | Stable — a prototype property |
| Storage plane `/storage/v1` (signed URLs, per-run secrets) | Stable — a prototype byte plane |
| Durable storage and migrations | Not planned — disposable state is doctrine |
| Studio, `/~personas`, `/~whoami` | Stable — development surfaces |
| `spock new` / `init` / `check` / `start` / `dev` | Stable |
| `spock dev` client live reload (last-known-good) | Stable |
| `spock dev` backend reload (currently `restart_required`) | In progress |
| Uhura client language | Experimental — strict 0.4 is integrated in current source but not yet shipped in npm; published 0.5.3 embeds the retired frontend. See [Uhura](uhura.md) |

## Distribution

| Feature | Status |
| --- | --- |
| npm package: macOS arm64/x64, Linux x64 (glibc), Windows x64; Node ≥ 18 | Stable |
| Alpine / musl Linux | In progress — not supported in 0.5.x |
| VS Code grammar (locally packaged VSIX) | Experimental |

## Vocabulary

One canonical name per concept, used consistently across this site, the
[v0 specification](spec/v0.md), and the compiled contract:

| Term | Meaning |
| --- | --- |
| **contract** | The compiled JSON artifact a program produces, served verbatim at `GET /~contract`. |
| **authority** | The Spock-owned backend: durable truth, policy, and guarded mutations. |
| **derived error** | A failure code minted from a schema constraint, such as `user_username_taken`. |
| **product error** | A top-level `error` declaration (experimental RFD 0024 preview). |
| **refusal** | A product error raised from a fn body via `spock_refuse`; kind `refused`, REST status 409. |
| **reserved code** | A protocol-owned code: `not_found`, `type_mismatch`, `unknown_field`, `bad_request`, `internal`, `unauthorized`, `conflict`. |
| **diagnostic** | A compile-time error with a stable code (`E001`–`E053`, `L001`–`L012`) and a source span. |
| **the floor** | The derived per-table read/write surface every table receives. |
| **the deliberate surface** | The functions a program declares on top of the floor. |
| **polarity** | Whether a function reads (`fn`) or writes (`mut fn`). |
| **escape** | One `unchecked sql("...")` statement in a fn body, carried verbatim in the contract. |
| **actor / actor seam** | The current identity value / the deliberately unverified path that carries it (`X-Spock-Actor`). |
| **anchor** | The one `auth table` a program may declare; identity references point at it. |
| **persona** | A seeded actor row used for impersonation during development. |
| **seed replay** | Rebuilding the database from source and seed on every load, through the ordinary write path. |
| **disposable state** | The doctrine that v0 state is never migrated, only replayed. |
| **framework project** | A directory with a `spock.toml` manifest, served by `spock start` / `spock dev`. |
| **standalone program** | One explicit `.spock` file, served by `spock run`. |

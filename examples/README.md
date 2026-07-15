# Examples

This directory is for example application domains.

The canonical combined-framework example lives at
[`uhura/examples/instagram`](../uhura/examples/instagram). It keeps the same
Instagram product domain but adds the authoritative backend, complete Uhura
client, and `spock.toml` topology required by `spock start` and `spock dev`.
This directory retains the product requirements, answer sheets, runnable
language slices, generated clients, and future-facing language drafts.

Each domain may start with a `PRD.md` that describes the product requirements
without caring about Spock. The Spock language should fit the requirements, not
the other way around.

Accepted Spock files in this directory should use the language surface the
current toolchain implements:

- `table` and `auth table` for persistent data
- `record` for named return shapes
- `fn` and `mut fn` for read and mutation operations
- `seed` for representative development data

Do not use proposal-only concepts in `.spock` examples. Future-facing language
ideas belong in `docs/rfd/`.

## Scenario Tracks

Spock examples should be grounded in real application engineering problems.
Good example tracks include:

- `reddit` - communities, posts, comments, voting, moderation workflows
- `commerce` - products, carts, orders, payments, fulfillment state
- `saas` - workspaces, projects, members, invitations, billing-facing records
- `marketplace` - sellers, listings, buyers, offers, reviews
- `support` - customers, tickets, assignments, status transitions
- `cms` - authors, articles, drafts, publishing, public content views

Each scenario should stay small enough to read quickly, but real enough to show
why persistent data, named shapes, and deliberate functions belong together.

## Example Rules

An example should:

- model a concrete application scenario
- use only accepted Spock syntax in `.spock` files
- use records for named return shapes where they clarify the contract
- include functions for deliberate operations that would be called over RPC
- avoid speculative authorization, effects, traits, decorators, or test syntax

If an example needs a language feature that does not exist yet, write an RFD
instead of adding the feature here.

## Answer Sheets (`pg.sql`, …)

A scenario may also include a reference implementation in a conventional
stack (e.g. `pg.sql` for plain PostgreSQL). These are answer sheets: they
solve the PRD for real, with the patterns a specialist would actually use,
and stand entirely on their own.

## Runnable Slices (`v0.spock`)

A file named `v0.spock` is the **runnable slice**: it uses only the surface
the toolchain in `crates/` actually implements (`docs/spec/v0.md`) and runs
for real:

```sh
cargo run -p spock-cli -- run examples/instagram/v0.spock
```

## Design Drafts (`vN.spock`, N ≥ 1)

Exception: files named `v1.spock`, `v2.spock`, … inside a scenario are
**design drafts** — paper programs (see `docs/rfd/0005`, Task 0) written to
pressure-test the language before it exists. They may invent syntax freely;
every invention must be marked in the file and indexed for review. Drafts are
review artifacts for the language design, not accepted-surface examples, and
are superseded by the next version rather than maintained.

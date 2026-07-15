# Spock v0 Boundaries

State these boundaries plainly. Do not imply production guarantees that v0 does not provide.

## Language boundaries

- One source file per program; no modules or cross-file imports.
- No implemented `view`, `role`, `policy`, state-machine, native statement, effect, or external-service syntax.
- Function bodies are SQL escapes only. The compiler verifies the contract, statement safety envelope, loadability, parameters, and result shape, but not the business meaning of SQL.
- Records are function return shapes only. They are not stored and cannot be function parameters.
- References cannot target composite-key tables.
- Seed data cannot call functions.
- No partial or conditional uniqueness.
- No reusable nominal domain types or curated format types such as `email`.

## Security and identity boundaries

- v0 is an ungoverned prototype tier.
- `X-Spock-Actor` is forgeable and unverified. It is an impersonation seam for local development, not authentication.
- Table reads are public. GraphQL table writes and REST function calls are not protected by policy/RLS.
- `auth table`, `spock_actor()`, and `= me` provide identity plumbing, not authorization.
- Never describe v0 as secure for multi-tenant or production data.

## Data and runtime boundaries

- The database is recreated on every `run`; there are no migrations.
- The default database is in memory. `--db` selects a recreated local file, not durable migration-managed storage.
- REST and GraphQL list reads default to 50 rows and cap at 200.
- REST tables are read-only. Use GraphQL for table writes and REST RPC for declared functions.
- Function list returns are uncapped; function authors must apply their own `LIMIT` when appropriate.
- Storage bytes live in the local runtime. Signing secrets, signed URLs, and objects do not survive a restart.
- The in-process storage sweeper reclaims abandoned objects; storage is a prototype byte plane, not a production object store.

## Modeling guidance

- Store source facts, not display counters. Derive follower, comment, like, save, and similar counts from relationship rows.
- Keep private concepts semantically private in consumers even though the v0 open read floor cannot enforce row visibility.
- Prefer idempotent mutation functions for retryable UI commands.
- Return the authority echo from a mutation instead of inventing client state.
- Declare refusals for product rules and preserve constraint-derived errors for data laws.

## Accepted versus speculative inputs

Accepted smoke inputs:

```text
examples/filter-lab/schema.spock
uhura/examples/instagram/backend/app.spock
examples/instagram/v0.spock
```

Do not use these as conformance inputs:

```text
examples/instagram/v1.spock
docs/rfd/0000-vision.spock
```

They intentionally contain future syntax.

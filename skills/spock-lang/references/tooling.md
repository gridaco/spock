# Spock Tooling and Verification

## Select the CLI

Inside the Spock repository, use the current checkout so the compiler matches the source under review:

```sh
cargo run --locked -p spock-cli -- check app.spock
```

If a current local debug binary is already built, `target/debug/spock` is equivalent and faster. For a consumer project, first respect any version pinned in its package manifest, lockfile, script, or user request. If the project does not select a version, use the current published package and record the version used:

```sh
npx --yes spock --version
npx --yes spock check app.spock
```

Do not debug a current-source program with an older npm-cached binary. Reproduce disagreements with the repository-local CLI.

Spock v0 does not implement `spock init`. Creating a project means creating the target directory and its single `.spock` program, conventionally `app.spock`.

## Core commands

```sh
spock check app.spock
spock build app.spock -o contract.json
spock gen types app.spock -o contract.ts
spock gen graphql-schema app.spock -o schema.graphql
spock run app.spock --port 4000
spock run app.spock --port 4000 --db /tmp/app.sqlite
```

`check` is more than syntax checking. It materializes the schema in memory, validates function SQL bodies and return shapes, and replays seed data through the runtime write path.

`run` recreates the selected database and replays seed data on every start. Omit `--db` for an in-memory database.

## Runtime endpoints

After `spock run app.spock --port 4000`:

```text
GET  /~health
GET  /~contract
GET  /~studio
GET  /~personas
GET  /~whoami
GET  /rest/v1/{table}
GET  /rest/v1/{table}/{id}
GET  /rest/v1/rpc/{read_fn}
POST /rest/v1/rpc/{fn}
GET  /graphql/v1             GraphiQL
POST /graphql/v1             GraphQL execution
```

When the contract references `storage_object`, `/storage/v1` is also mounted.

Basic probes:

```sh
curl -sS http://127.0.0.1:4000/~health
curl -sS http://127.0.0.1:4000/~contract
curl -sS http://127.0.0.1:4000/~personas
curl -sS http://127.0.0.1:4000/~whoami
```

Inspect `/~contract` before constructing table or RPC probes. Use only table names, function names, parameter shapes, and actor ids declared by the target program. Use `X-Spock-Actor` only to exercise the v0 development identity seam, and test anonymous behavior by omitting it.

## GraphQL

GraphQL reads and writes are mounted at `/graphql/v1`. GET serves GraphiQL; POST accepts the standard `{query, variables, operationName}` envelope. Introspection is enabled.

List reads default to 50 rows and cap at 200. Generated writes use the same runtime validation and derived errors as seed replay.

For offline tooling:

```sh
spock gen graphql-schema app.spock -o schema.graphql
```

## Storage verification

The upload sequence is:

1. `POST /storage/v1/object/upload/sign` to mint a pending object and signed PUT URL.
2. `PUT` bytes to that URL with a content type.
3. Attach the returned object id through a normal table write or function.
4. `POST /storage/v1/object/sign/{id}` to mint a signed GET URL.
5. `GET` the signed URL and verify bytes and content type.

Signed URLs and stored objects are local to one run. Do not persist or expose them as durable product URLs.

## Spock and Uhura together

For the canonical checked-in Instagram framework project, build its provider
and run one process:

```sh
corepack pnpm@10.11.0 -C uhura/web install --frozen-lockfile
corepack pnpm@10.11.0 -C uhura/web build:provider
spock dev uhura/examples/instagram
```

Then verify:

```text
http://127.0.0.1:4000/          Uhura Editor
http://127.0.0.1:4000/play      live Uhura Play
http://127.0.0.1:4000/~studio   Spock Studio
```

All routes follow the framework `--port`. The legacy
`scripts/spock-uhura.sh` runner is useful only when explicitly comparing two
separate roots.

## Completion checklist

- Run `spock check` on every changed accepted-language file.
- Run focused Rust tests when changing compiler or runtime code.
- Run `cargo test --workspace --locked` for broad Spock changes.
- Regenerate and inspect the contract, TypeScript, or GraphQL schema when their surface changes.
- Probe the exact runtime path affected by the task.
- For Uhura integration, verify Editor, Play, actor switching, at least one read, and at least one relevant command.
- Record unchecked escape counts and prototype limitations in the handoff.

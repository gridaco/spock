# Spock Tooling and Verification

## Install the npm CLI

Spock is distributed as the `spock` npm package with prebuilt binaries. It
requires Node.js 18 or newer and supports macOS arm64/x64, GNU-libc Linux x64,
and Windows x64. Alpine and other musl-based Linux distributions are not
supported in v0.5.0.

Install the current release once at user level, then use the `spock` command
directly for every project:

```sh
npm install --global spock@latest
spock --version
```

If `spock --version` already reports the required release, do not reinstall it.
When the user explicitly requests another version, install that version
globally with `npm install --global spock@VERSION`. Use plain `spock` commands
after installation.

## Framework projects

Create a full-stack Spock plus Uhura project, or a backend-only project:

```sh
spock new demo
spock new api --backend-only
```

Adopt existing unambiguous sources without moving or overwriting them:

```sh
spock init path/to/project
```

Check and serve a framework project:

```sh
spock check path/to/project
spock start path/to/project --port 4000
spock dev path/to/project --port 4000
```

`start` serves one fixed checked generation. `dev` publishes valid client
changes live while reporting backend or topology changes as restart-required;
it does not migrate, reseed, or replace the active database in place.

## Standalone language commands

Use explicit `.spock` files with the standalone commands:

```sh
spock check app.spock
spock build app.spock -o contract.json
spock gen types app.spock -o contract.ts
spock gen graphql-schema app.spock -o schema.graphql
spock run app.spock --port 4000
spock run app.spock --port 4000 --db /tmp/app.sqlite
```

`check` is more than syntax checking. It materializes the schema in memory,
validates function SQL bodies and return shapes, and replays seed data through
the runtime write path.

`run` recreates the selected database and replays seed data on every start.
Omit `--db` for an in-memory database.

## Runtime endpoints

After `run`, `start`, or `dev` on port 4000, inspect the applicable endpoints:

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

Inspect `/~contract` before constructing table or RPC probes. Use only table
names, function names, parameter shapes, and actor ids declared by the target
program. Use `X-Spock-Actor` only to exercise the v0 development identity seam,
and test anonymous behavior by omitting it.

## GraphQL

GraphQL reads and writes are mounted at `/graphql/v1`. GET serves GraphiQL;
POST accepts the standard `{query, variables, operationName}` envelope.
Introspection is enabled.

List reads default to 50 rows and cap at 200. Generated writes use the same
runtime validation and derived errors as seed replay. Generate the schema for
offline tooling with `spock gen graphql-schema`.

## Storage verification

The upload sequence is:

1. `POST /storage/v1/object/upload/sign` to mint a pending object and signed
   PUT URL.
2. `PUT` bytes to that URL with a content type.
3. Attach the returned object id through a normal table write or function.
4. `POST /storage/v1/object/sign/{id}` to mint a signed GET URL.
5. `GET` the signed URL and verify bytes and content type.

Signed URLs and stored objects are local to one run. Do not persist or expose
them as durable product URLs.

## Framework verification

For a project with an Uhura client, `start` and `dev` serve one origin:

```text
http://127.0.0.1:4000/          Uhura Editor
http://127.0.0.1:4000/play      live Uhura Play
http://127.0.0.1:4000/~studio   Spock Studio
```

Verify Editor diagnostics, Play, actor switching, at least one relevant read,
and at least one relevant command.

## Completion checklist

- Run `spock check` on the whole framework project or every changed standalone
  program.
- Regenerate and inspect the contract, TypeScript, or GraphQL schema when its
  source surface changes.
- Probe the exact runtime path affected by the task.
- For Uhura integration, verify Editor, Play, actor switching, at least one
  read, and at least one relevant command.
- Record the installed CLI version, unchecked escape counts, and prototype
  limitations in the handoff.

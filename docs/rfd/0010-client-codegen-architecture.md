# RFD 0010 — client and codegen architecture

Status: accepted decision — client-side consumption splits into three
artifacts on three change cadences; **the binary owns generation**; the
client package is generic and hand-written, never generated; the GraphQL
path stays borrowed. This resolves RFD 0009 §4's "client posture"
decision and extends it with the generator-ownership rule.

## 1. The decision: three artifacts, three clocks

1. **The generator — `spock gen`, inside the binary.** Emits derived
   artifacts from the contract: TypeScript types (`spock gen types`) and
   the GraphQL SDL (`spock gen graphql-schema`). Versioned with the
   *language*: the commit that grows the IR grows the emission.
2. **The client — the reserved `spock` npm package, generic,
   hand-written, later.** A thin protocol client over `/rest/v1` (and
   `/auth/v1` when it exists), parameterized by generated types —
   `createClient<contract>(url)`. Contains zero generated code; versioned
   against the *HTTP protocol*, which changes slowly. Timing unchanged
   from RFD 0009: it earns existence when REST has writes to wrap.
3. **Per-app generated types — in the consumer's repo.** Regenerated,
   never published, never edited; versioned by *their schema*.

Fusing any two mixes clocks. The cautionary tale is the generated-client
shape (Prisma): heavyweight, version-coupled, hard to tree-shake — and
being retreated from by its own authors. The pattern that aged well is
Supabase's: `supabase gen types typescript` + a generic `supabase-js`.
Spock takes that pattern with one improvement — the generator does not
need its own CLI distribution, because Spock's binary *is* the toolchain.

**Generate types, never the client.** The client is code; code is
maintained, versioned, and debugged. Only types are derived.

## 2. Why the binary owns generation

- **Version skew is the killer argument.** The contract shape is frozen
  additive-only (spec §6), but it will grow — `fn` is next. An
  out-of-binary generator parses contract JSON and must release in
  lockstep with the language forever; the day `spock run` serves a
  contract the npm generator does not understand, the ecosystem has
  drift. In-binary generation cannot lag: same commit, same IR structs,
  serde-guaranteed fidelity.
- **It is already the doctrine.** A TypeScript emission is
  Contract → text — a sibling of Contract → SQLite DDL and the future
  `spock2sql` (RFD 0006: the IR is the artifact; emissions are
  conformances). RFD 0005's tracer bullet named a "generated TypeScript
  client" from the start; this is that thread.
- **It works offline.** `spock gen types app.spock` needs no server, no
  node, no network — compile and emit. The single-binary story survives.
- **What it emits is value no borrowed tool provides**: row/insert/update
  shapes straight from the contract, and the **frozen error vocabulary as
  literal unions** (spec §6.1) — derived errors reaching autocomplete.
  Later, `fn` signatures land in the same emission.

The counter-argument — "a TS emitter should be written in TS" — is real
but weak at this scale: the emitted declarations are simple, and a
verbatim golden test in the language crate plus `tsc --noEmit` over
emitted output (the client example's walkthrough exercises it) verify
the *product* more rigorously than the implementation language would.

## 3. The borrowed path stays borrowed

Spock never reimplements GraphQL codegen. The GraphQL surface speaks a
specified dialect (docs/spec/graphql.md) precisely so that
`graphql-codegen`, gql.tada, urql, and Apollo work unmodified —
`examples/instagram/client/` is the standing proof. The binary's one
contribution to that path is a bridge, not a replacement:
`spock gen graphql-schema` prints the SDL the runtime would serve
(literally the same schema object, `.sdl()`), so the borrowed toolchain
can run from a file, with no live server.

## 4. The TypeScript emission, specified

Naming follows the same verbatim-lowercase law as the GraphQL dialect
(graphql.md §3): per table `t` —

| Emission | Shape |
|---|---|
| `interface <t>` | the row, as reads return it: `f: T`, `\| null` iff optional; a reference field is the target key's type, written `target["<key>"]` |
| `interface <t>_insert` | what insert accepts (§7.2): required-no-default fields required; everything else `?:`; `\| null` only on optional fields (on insert `null` is absence, §5.1) |
| `interface <t>_update` | non-key fields only (keys are immutable): all `?:`; `\| null` only on optional fields (`null` clears; on required fields it is the derived error, so the type excludes it). A pure-key table emits no `_update`; its map entry is `never` |
| `type <t>_error` | the table's derived codes (§6.1), a literal union, contract order |
| `interface contract` | one entry per table: `{ row; insert; update; error }` — the generic parameter the future client takes |
| `type reserved_error`, `type error_code` | the five reserved codes; the union of everything this contract can produce |
| `type uuid`, `type timestamp` | `string` aliases, mirroring the wire |

Totality, as everywhere: the emission has reserved names (JavaScript
reserved words, TypeScript's predeclared type names, and the emission's
own — `contract`, `reserved_error`, `error_code`, `uuid`, `timestamp`),
and cross-table collisions on the four derived names are possible
(a table literally named `user_insert`). Both fail generation with a
stated reason — never emit code that does not compile.

The output is deterministic (contract order, no timestamps): regeneration
diffs are schema diffs.

## Open questions

- **Other target languages** (`spock gen types --lang …`) — the
  subcommand shape leaves room; nothing is promised.
- **Whether `interface contract` is exactly the generic the `spock`
  npm client will take** — decided when that client is built (RFD 0009:
  after REST writes).
- **`fn` in the emission** — when the language grows `fn`, its typed
  signatures join both emissions (TS and SDL) in the same commit as the
  IR change; that co-versioning is the point of §2.

# Study — Browser-Wasm feasibility for the Spock authority runtime

- **Study state:** open
- **Kind:** informal implementation-feasibility study
- **Author:** [@softmarshmallow](https://github.com/softmarshmallow)
- **Created:** 2026-07-23
- **Review state:** initial crate, dependency, and target audit completed;
  authority-runtime experiment not started
- **Language-problem issue:** none
- **Working group:** none
- **RFD:** none

> **Non-normative feasibility research; no support is being adopted.** This
> study does not add browser Wasm to Spock's supported targets, select a
> runtime or SQLite toolchain, promise native/Web parity or persistence, change
> the CLI or server contract, schedule implementation, or authorize a
> prototype. It records current evidence, risks, and falsifiable experiment
> gates only.

## Question

Could the semantics of a running Spock authority be hosted optionally inside a
browser session through WebAssembly, without a separately running Spock
process—and could that remain a bounded secondary host rather than a permanent
constraint on the native runtime?

The question has two independent parts:

1. whether the current compiler and runtime dependencies can execute faithfully
   on a browser Wasm target; and
2. whether supporting that target would fit Spock's prototype-language mission
   without making the browser the capability floor for future development.

This is not a question about compiling the current `spock` executable or its
TCP server unchanged. A browser cannot host that native process model. The
possible target is a listenerless authority session exposing direct operations
to its containing page.

## Outcome at this checkpoint

The compiler produces a browser-targeted Wasm build, and a temporary wrapper has
executed under Node. It has not yet run in a real browser or passed
native/browser differential tests. A full authority session has a credible
architecture, but its decisive SQLite experiment has not been completed. The
current server runtime does not compile for `wasm32-unknown-unknown`.

Therefore:

- browser-Wasm support is **not part of current Spock**;
- no implementation is planned or authorized by this study;
- the remaining runtime work must not be described as minor until SQLite
  behavior is proven; and
- if the question is revisited, the first work is one isolated database
  compatibility experiment, not a runtime refactor.

## Scope and non-goals

This study covers:

- the compiler, checked contract, DDL, and generated artifacts;
- the SQLite-backed authority semantics used to load and play a contract;
- direct GraphQL, function, actor, transaction, refusal, seed, and blob
  behavior;
- the boundary between portable semantics and native hosting;
- browser Worker, clock, entropy, and byte-transfer requirements; and
- the long-term maintenance consequences of an optional Web target.

It does **not**:

- propose a supported browser runtime;
- make browser execution a language or product requirement;
- put the CLI, Axum listener, filesystem discovery, watcher, or process
  lifecycle in the browser;
- provide a production authority, security boundary, secret store, or durable
  database;
- select Wasm persistence, service workers, multi-tab coordination, or offline
  deployment;
- select a generic database abstraction or a second SQL implementation;
- change the current native protocol;
- guarantee that future native features will have browser equivalents; or
- change any specification, RFD, roadmap, or release commitment.

## Method

The initial investigation used:

1. a crate and dependency-boundary audit;
2. direct `wasm32-unknown-unknown` target checks;
3. temporary `wasm-bindgen` compiler and dependency probes outside the
   repository;
4. a source audit of the authority engine, HTTP surface, storage plane, and
   generation lifecycle; and
5. an explicit capability and falsification analysis.

All executable probes were temporary. This study records their relevant
results; it does not add their wrappers or toolchains to the repository.

## Four distinct host claims

The phrase “Spock on the Web” is too broad for useful evaluation.

| Claim | Meaning | Current finding |
| --- | --- | --- |
| Browser compiler | Parse and check source; emit diagnostics, contract JSON, DDL, or types. | Wasm build and Node execution succeed; real-browser identity is untested. |
| Browser authority session | Materialize and mutate an ephemeral authority in a Worker. | Plausible; SQLite proof required. |
| Browser server | Run the native HTTP listener, filesystem host, and process lifecycle in Wasm. | The current native TCP/process model is not the studied target. |
| Production Web authority | Treat client-controlled execution and state as a remotely trusted deployed backend. | Client-controlled execution cannot provide that trust; local-first trust models were not evaluated. |

Only the second claim is the unresolved subject of this study. It may reuse
the same logical operations as the native protocol, but it would not emulate a
localhost server inside the page.

## Current repository boundary

### Language and project layers

`spock-lang` is already close to a portable semantic core. Its compiler accepts
source text and produces a serializable checked `Contract`; DDL and TypeScript
generation are pure transformations over that contract. It performs no
production filesystem, networking, process, or thread work.

`spock-project` contains reusable manifest and planning logic, but its
discovery and loading paths use the native filesystem. A browser caller would
provide captured source and asset bytes rather than project paths.

### Runtime layer

`spock-runtime` currently combines several concerns:

- SQLite connection creation, DDL, builtins, function-body validation, seed
  replay, reads, writes, and transactions;
- dynamic GraphQL schema construction and execution;
- Axum HTTP/REST/GraphQL routing;
- URL signing and storage HTTP behavior;
- Tokio lifecycle and background storage work; and
- embedded Studio assets.

The core authority state directly owns a `rusqlite::Connection`, signer, and
blob store. This is appropriate for the current native prototype but is not
yet a portable session boundary.

One useful seam already exists. `CapturedBackend` accepts the exact source and
seed-asset bytes without reading the filesystem, and
`BackendGeneration::from_captured` compiles and materializes them. The same
generation type presently also exposes an Axum router and owns Tokio
background-task lifecycle. A future experiment would have to separate those
native host responsibilities from the captured authority session.

### Native host and CLI

`spock-host` and `spock-cli` own project discovery, filesystem observation,
process behavior, listener setup, route composition, and developer lifecycle.
They are native applications and are not candidates for a browser target.

## Target and dependency evidence

### Compiler

This target check succeeds:

```sh
cargo check --locked \
  -p spock-lang \
  --target wasm32-unknown-unknown \
  --features uuid/js
```

The explicit `uuid/js` feature is needed because the workspace currently
enables UUID-v7 generation globally, even though `spock-lang` only parses UUID
literals. A cleaner future dependency split could keep UUID generation
features in runtime crates, but this is not required to answer compiler
feasibility.

A temporary `wasm-bindgen 0.2.126` wrapper was built with Rust 1.92.0 at
repository commit `73b482330bcd06a8844e4715a112f18c2c505952`. Its Node-target
package executed under Node 24.14.0 and compiled
`examples/instagram/v0.spock` into contract JSON. Diagnostic output was also
exercised. Native/Wasm contract or diagnostic identity was not compared, and
the generated Web-target package was not executed in a real browser.

The wrapper was temporary and is not retained as a repository artifact, so
this study deliberately makes no bundle-size or performance claim from it.

### Native networking and lifecycle

The current workspace-wide Tokio configuration enables multithreaded runtime,
networking, and signals. The following target check pulls Mio through that
graph, and Mio rejects `wasm32-unknown-unknown`:

```sh
cargo check --locked \
  -p spock-runtime \
  --target wasm32-unknown-unknown \
  --features uuid/js,getrandom/js
```

Axum serving, TCP listeners, signals, and background host lifecycle are
neither portable nor needed for the reference listenerless session.

This is an architectural boundary, not a reason to emulate native networking.

### Current SQLite substrate

The workspace uses `rusqlite 0.32` with bundled SQLite. Its
`libsqlite3-sys` build compiles native SQLite C and does not provide the needed
browser-Wasm C environment. The target probe fails while compiling SQLite,
beginning with unavailable C system headers.

SQLite is not incidental to current runtime behavior. Spock relies on:

- schema batch execution and foreign keys;
- immediate and deferred transaction behavior;
- custom `spock_uuid`, `spock_now`, `spock_refuse`, and `spock_actor`
  functions;
- named-parameter and result-column metadata;
- statement read-only inspection;
- constraints, extended error codes, `RETURNING`, JSON behavior, and rollback;
  and
- BLOB storage.

Most runtime modules directly use `rusqlite`. Replacing it with a distinct
JavaScript database API would be a substantial engine fork, not an adapter.

`rusqlite 0.40.1` has a browser-Wasm path through `sqlite-wasm-rs 0.5.5`.
That path is a candidate, not a proven solution for Spock. A temporary probe
reached the SQLite C build but the available Apple Clang 21 lacked a
`wasm32-unknown-unknown` code-generation backend. A Clang/LLVM toolchain with a
Wasm backend is still required to perform the decisive test.

### Clock and entropy

Runtime UUID creation needs browser entropy, and `OffsetDateTime::now_utc()`
needs a browser clock implementation. The existing dependencies can expose
those facilities through target-specific features such as `uuid/js`,
`getrandom/js`, and `time/wasm-bindgen`.

Those adaptations are bounded. They do not resolve SQLite or decide whether
time and randomness should instead be injected for deterministic prototype
sessions.

### GraphQL

The dynamic `async-graphql` dependency compiled in an isolated browser-Wasm
probe. A browser authority could build a schema and call `Schema::execute`
directly. It would not need Axum, an HTTP request, or an actor header; request
context could carry the actor value directly.

This dependency probe is not yet an integrated Spock runtime test.

## Findings

1. **Compiler Wasm packaging and Node execution are demonstrated.**
   Real-browser conformance remains unproven.
2. **The complete current runtime is not a browser-Wasm crate.** Native
   transport and lifecycle are mixed into it.
3. **A listener is unnecessary for an embedded session.** Direct operation
   dispatch is the smaller and more truthful boundary.
4. **SQLite is the go/no-go dependency.** The chosen candidate must preserve
   Spock's actual statement, error, function, and transaction behavior.
5. **Using a behaviorally different browser database would create semantic
   drift.** That cost is too high for a demonstration target.
6. **Browser execution is not trusted authority.** It can demonstrate and
   validate declared rules, but the user controls its code, state, actor,
   clock, and storage.
7. **Wasm inevitably adds a build and conformance axis.** It need not become
   the capability floor for the language or native host.

None of these findings adopts support.

## Architectural fit and long-term risk

Spock is a prototype language whose runtime makes a backend contract playable.
An ephemeral, zero-install browser session is therefore a natural
demonstration host. It is less natural as a general server or deployment
target.

One favorable evaluation criterion would make Wasm a **host and packaging
axis** only: portable semantics remain shared, while persistence, transport,
lifecycle, and operating-system integration remain host capabilities.

A disqualifying outcome would make the browser a **parity floor**: every
future database feature, effect, plugin, concurrency improvement, or host
integration would have to fit the browser before native Spock could evolve.

As an evaluation criterion, a future experiment should be removable as one
leaf host without redesigning the language or native runtime. The experiment
should also demonstrate that a capability absent from the browser can be
rejected explicitly rather than approximated silently. Adopting either rule as
lasting project policy would require a separate decision.

## Reference experiment shape, not a selected design

If the question is reopened, the smallest falsifiable arrangement is:

```text
Browser page
  -> dedicated Web Worker
     -> optional Spock Wasm session
        -> spock-lang
        -> portable authority semantics
        -> one in-memory SQLite-Wasm connection
```

The Worker would receive source text and captured seed-asset bytes. A narrow
byte/JSON boundary could expose operations such as:

```text
create(source, seed_assets)
contract()
graphql(request, actor)
call(name, arguments, actor)
put_blob(bytes, metadata)
get_blob(id)
reset()
```

This is deliberately:

- one page, one Worker, one connection, and serialized operations;
- ephemeral and reconstructed from source on refresh;
- listenerless and filesystem-free;
- free of persistence, migration, cross-tab, and production-security claims;
  and
- built from the same SQLite semantics, or not built at all.

The exact crate split, ABI, names, and public status remain unselected.

## Decisive go/no-go gates

### Gate A — compiler identity

Run the same source corpus natively and in a real browser. Require identical
canonical contract JSON, diagnostics, DDL, and generated types.

The completed target build and Node probe make this gate plausible; they do
not complete it.

### Gate B — SQLite runtime identity

Using one browser-Wasm SQLite candidate, prove:

1. an in-memory connection with foreign keys;
2. immediate and deferred transactions, commit, refusal, and rollback;
3. all four Spock custom scalar functions;
4. named parameters, statement read-only inspection, and result metadata;
5. Spock DDL materialization and complete seed replay;
6. constraint and extended-error routing;
7. representative reads, writes, and SQL function bodies; and
8. BLOB put/get behavior.

Stop if these require a second behavioral database implementation or material
weakening of native semantics.

### Gate C — real authority fixture

Load `examples/instagram/v0.spock` and its captured seed assets. At this
checkpoint the source is SHA-256
`e787b5c7c3418122ba725f671bdb5b8cd53562eb62b8849db7fdd8dd76b55bb8`.
Require its boot query, representative accepted mutations, representative
refusals, and post-mutation reads to match native results. If the fixture
changes before the experiment, record the replacement revision and digest.

### Gate D — browser session boundary

Run the session in a Worker and demonstrate deterministic disposal, structured
diagnostics, bounded memory transfer, and recovery from initialization and
operation failures.

Only after all four gates would a runtime-core separation be evidence-based.

## Counterevidence and costs

A browser authority could become architectural baggage through:

- divergent SQLite versions, compile options, VFS behavior, or error details;
- a single-threaded and single-connection ceiling;
- browser quotas, eviction, Worker lifecycle, and cross-tab locking if
  persistence is later implied;
- the absence of runtime-loaded native SQLite extensions;
- different plugin, network, clock, entropy, and observability models;
- a separately versioned JavaScript/Wasm ABI and packaging toolchain;
- browser-specific debugging and compatibility expectations;
- growing download, instantiation, and seed sizes; and
- pressure to present a client-controlled simulation as production authority.

These costs appear proportionate only for an explicitly bounded prototype
session. Persistence, concurrent authority, external connectors, arbitrary
plugins, or production deployment would be reasons to stop and evaluate a
different scope rather than silently expand the reference experiment.

## Candidate capability profile if reconsidered

This matrix is an evaluation aid, not current policy.

| Capability class | Possible Web posture |
| --- | --- |
| Parse, check, IR, diagnostics, DDL, generated types | Portable and exact |
| DDL load, seed replay, reads, mutations, functions, refusals | Experimental only after conformance gates |
| Captured asset bytes and direct BLOB access | Optional session adapter |
| Clock, entropy, actor context | Explicit host inputs or narrow browser adapters |
| TCP, Axum, CORS, headers, Studio serving | Excluded from this experiment; currently native |
| Filesystem discovery, named database paths, watchers, locks, signals | Excluded from this experiment; currently native |
| Durable signed URLs and secret-bearing authority | Excluded from this experiment; currently native |
| Long-running jobs, subprocesses, unrestricted connectors, native extensions | Excluded from this experiment; currently native |

One proposed experiment criterion is that an unsupported capability rejects
the session before execution with a stable, inspectable error rather than
being approximated without being named. This is not current policy.

## Limitations

The study has not:

- built current Spock against modern browser-Wasm SQLite;
- run the authority engine in a browser;
- compared native and Wasm SQLite compile options or errors;
- run the real authority conformance fixture;
- measured full runtime size, startup, memory, or browser coverage;
- tested persistence or multiple browser sessions;
- designed a stable capability manifest or Wasm ABI; or
- estimated ongoing release and maintenance cost from lived experience.

The compiler measurements were made with a temporary wrapper and are useful
only as feasibility evidence.

## Disposition and follow-up

The compiler-to-Wasm build path is feasible; real-browser conformance remains
unproven. The authority path is credible enough to preserve as a research
option, but not proven enough to support or implement.

The project is **not pursuing browser-Wasm support now**. No workspace member,
feature flag, compatibility promise, CI target, release artifact, or public
runtime tier should be added on the strength of this study.

If a later product need reopens the question, begin with Gate B in isolation.
If it passes, review the host boundary and support posture before any shared
runtime refactor. If it fails materially, retain compiler-only browser
feasibility and stop.

Any lasting supported architecture would require its own review and durable
decision record. This study cannot authorize it.

## Primary technical references

- [Rust `wasm32-unknown-unknown` target](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html)
- [`rusqlite`](https://github.com/rusqlite/rusqlite)
- [`sqlite-wasm-rs`](https://github.com/Spxg/sqlite-wasm-rs)
- [SQLite Wasm persistence](https://sqlite.org/wasm/doc/trunk/persistence.md)

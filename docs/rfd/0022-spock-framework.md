# RFD 0022 — Spock as a framework: one project, one command, two languages

Status: **study draft and discussion record.** This RFD records the current
working direction for a unified Spock project and toolchain. It proposes no
implementation and does not merge Spock and Uhura semantics. Exact manifest
syntax, route allocation, packaging, and development-state behavior remain
open. The state problem is studied separately in
[RFD 0023](0023-development-state-reload.md) and must be decided before the
combined development host is implemented.

## 0. The question

The installed `spock` command currently takes a `.spock` file as its unit of
work. The Spock–Uhura composition proof has a different unit:

1. one Spock authority program;
2. one optional Uhura client project; and
3. composition knowledge currently carried by a shell script, two process
   lifecycles, and duplicated port configuration.

The proof is real: [`scripts/spock-uhura.sh`](../../scripts/spock-uhura.sh)
builds and runs both runtimes against the Instagram example. It is also the
clearest inventory of what a framework host must replace. The script accepts
one `.spock` program and one Uhura project root, builds two binaries, builds
the Uhura web application, verifies that separately built Uhura Wasm artifacts
exist, binds two ports, waits for health, and coordinates shutdown. The Uhura
provider repeats the Spock port in `uhura.toml`.

“Spock as a framework” should mean:

- `spock` becomes the installed project-level front door;
- Spock remains the authority language and authority runtime;
- Uhura remains the client/experience language and runtime;
- a composition host loads them as one coherent project; and
- absorbing Uhura means absorbing discovery, distribution, lifecycle,
  diagnostics, linking, and hosting — **not semantic ownership**.

This document records that direction before either runtime is reshaped around
it. It deliberately does not solve development database reload; pretending
that question is a host implementation detail would bake an accidental answer
into the framework boundary. RFD 0023 owns it.

### In scope

- the project as the new unit of work;
- `spock.toml` and canonical project layout;
- the public command ecosystem;
- logical crate and host responsibilities;
- one-origin serving and the existing web products;
- compatibility with file-oriented Spock commands; and
- the build/repository boundary around the Uhura submodule.

### Out of scope

- backend reload, state migration, state rebase, and reset semantics (RFD
  0023);
- deployment, production process supervision, and remote environments;
- generalized monorepo or microservice orchestration;
- multiple authority databases or distributed transactions;
- merging either language's grammar, IR, checker, or state machine; and
- replacing explicit provider/link contracts before a linker is designed.

## 1. The boundary composition must preserve

The existing responsibility split remains the load-bearing rule:

| Concern | Owner |
|---|---|
| Durable product truth, constraints, policy, and guarded operations | Spock language/runtime |
| UI-session state, experience transitions, semantic presentation, and service requests | Uhura language/runtime |
| Project discovery, cross-artifact linking, hosting, lifecycle, and combined diagnostics | Spock framework layer |
| Layout, paint, native widget mechanics, and device integration | Renderer/host drivers |

No authoritative fact may live in both Spock and Uhura. The framework layer
may compose checked artifacts and route requests between them; it may not
reinterpret either source language or invent compatibility that their public
contracts do not prove.

The current repository boundary also remains intentional:

- Uhura's canonical source remains its own repository, included at `uhura/`
  as a git submodule;
- Uhura remains independently buildable and testable;
- `spock-runtime` does not acquire UI-session or renderer behavior;
- a combined host should consume a proposed versioned Uhura host/library
  boundary; and
- the standalone `uhura` command remains useful to contributors and for
  isolated language work even though it is not the public framework front
  door.

“Client” is the project-topology word. Calling Uhura `ui` would be misleading:
Uhura owns a language, checker, deterministic state machine, Editor, Play
runtime, provider ports, and renderer-neutral semantics — not merely a bundle
of visual assets.

## 2. Vocabulary

The name “Spock” now appears at several layers. Use these terms precisely:

- **Spock language** — `.spock` source, its checker, contract IR, and authority
  semantics.
- **Spock runtime** — materializes and serves one checked authority contract.
- **Uhura** — the client/experience language, checker, headless runtime,
  Editor, and Play surfaces.
- **Spock framework** — the project manifest, public CLI, composition/link
  layer, combined host, and distribution envelope.
- **Spock project** — one `spock.toml`, one required backend entry, and zero or
  one client root in the first version.
- **Project generation** — one coherent checked backend artifact, host routing
  table, binding to backend development state, and, when a client is
  configured, the matching integrated Uhura Play artifact. Uhura Editor may
  publish a newer static read model with explicit freshness independently;
  RFD 0023 defines the backend-state and activation parts of this term.

This vocabulary prevents two opposite mistakes: shrinking Spock back to only
the language when discussing the installed product, and growing
`spock-runtime` into an unbounded application framework.

## 3. The project is the new unit of work

The recommended generated shape is:

```text
my-app/
├── spock.toml
├── backend/
│   ├── app.spock
│   └── seed/                 # optional file(...) assets
└── client/                   # optional; shown here as the full-stack shape
    ├── uhura.toml
    ├── uhura.lock
    ├── app/
    ├── components/
    ├── surfaces/
    ├── ports/
    ├── providers/
    ├── fixtures/
    ├── catalog/
    └── styles/
```

`app.spock` is the conventional backend name, not `main.spock`:

- it already appears in examples, command tests, and the composition proof;
- a declarative authority contract is an application boundary, not a
  procedural entry function; and
- the manifest still permits another filename when a project needs one.

The first manifest should be small and topological:

```toml
version = 1

[project]
name = "my-app"

[backend]
root = "backend"
entry = "app.spock"

[client]
root = "client"
```

This exact spelling is provisional. The responsibilities are the important
part:

1. `spock.toml` is required for project/framework commands.
2. Exactly one Spock backend entry is required in the first version.
3. The Uhura client is optional; when configured, its root contains its own
   `uhura.toml`.
4. Every path is relative to the project root, normalized, and forbidden from
   escaping it.
5. `spock.toml` describes composition topology. It does not duplicate Uhura's
   catalogs, ports, fixtures, assets, provider profiles, or language settings.
6. Start with singular `[backend]` and `[client]`. Named arrays create
   unearned multi-authority and multi-client semantics.
7. Internal ports and absolute provider URLs do not belong in the minimum
   committed schema. A one-origin host can derive relative endpoints.

The framework manifest is therefore the master **composition** config, while
`uhura.toml` remains the master **client-language** config. “Master” must not
mean “copy every subsystem setting upward.”

## 4. Empty authority is valid, but explicit

Even a project that currently uses only Uhura must carry the configured
`backend/app.spock`. It may be empty or comment-only. This gives every Spock
project one invariant and makes adding authoritative behavior later an edit,
not a project-topology conversion.

The user guidance can be simple:

> This project has no backend contract yet. Keep `backend/app.spock` empty
> until it does.

Do not scaffold a fake table merely to satisfy GraphQL. An empty authority
means “no authoritative capabilities,” not “invent a placeholder data model.”
The host should still be able to serve project health and the configured
client; authority routes may be absent or expose a deliberately empty
contract according to a later route decision.

This is not current behavior end to end. An empty source can compile and the
SQLite engine can materialize zero contract tables, but the eager GraphQL
builder rejects a `Query` with no fields. “Empty authority boots” is therefore
an implementation prerequisite for the framework, not a claim about v0.

## 5. The command ecosystem

The recommended public surface is:

| Command | Unit | Meaning |
|---|---|---|
| `spock new <name>` | new project | Create the selected canonical project template, always including the manifest and required backend. |
| `spock init [path]` | existing directory | Adopt existing sources without moving or overwriting them. |
| `spock check [path]` | project | Check backend, client, manifest, and every currently provable provider/link contract as one result. |
| `spock dev [path]` | project | Observe saved source and serve coherent last-good project generations. Exact backend state behavior is RFD 0023. |
| `spock start [path]` | project | Check once and serve one fixed combined generation with no source watcher. |
| `spock run <file.spock>` | Spock program | Preserve the standalone authority/file escape hatch. |
| `spock build`, `spock gen` | artifact | Preserve existing language artifacts; project-aware variants are a separate design. |

### 5.1 `new` and `init` are different

`spock new` is the create-a-project command. It owns the canonical template
and fails if its destination already contains conflicting files.

`spock init` is adoption. It creates only missing framework files, discovers
an unambiguous existing `.spock` entry and `uhura.toml`, never moves user
sources, never silently overwrites them, and fails with choices when discovery
is ambiguous. Adopting an Uhura-only directory creates the required empty
backend entry and points `spock.toml` at it; it does not invent a placeholder
table or relocate the Uhura project.

RFD 0020 recorded an unimplemented `spock init [name]` as the general
first-run gap. If this RFD is accepted, it refines that spelling: `new` creates
a named project; `init` adopts a directory. The distribution requirement
remains intact.

### 5.2 `dev` and `start` are different

`spock dev` means saved-source observation, current diagnostics, coherent
candidate ordering, last-good retention, and browser generation events. It
must not promise that authoritative state survives a structural edit until
RFD 0023 defines when and how that is true.

`spock start` means a fixed generation: resolve, check, construct, bind, and
serve without watching or automatic replacement. The name does not imply that
the prototype runtime is production-ready.

### 5.3 Language escape hatches remain

`spock run app.spock`, `spock check app.spock`, `spock build app.spock`, and
the current `spock gen` forms remain valuable for language development, CI
fixtures, backend-only experiments, and compatibility with the published
command.

Directory or omitted targets should select project mode; an explicit
`.spock` target should retain file mode. Whether project discovery walks up
through parents or only accepts the current/explicit root remains open.

Uhura-only expert operations such as formatting and deterministic traces need
a later namespace decision. `spock client fmt` and `spock client trace` are
better candidates than placing every subsystem command at the global top
level, but this RFD does not lock them.

## 6. Logical crate and host topology

Do not create `spock-framework-cli`, and do not demote the existing crate to
`spock-language-cli`. There should be one public command and one crate that
owns its argument parsing: `spock-cli`.

The recommended responsibility topology is:

```text
crates/
├── spock-lang
├── spock-runtime
├── spock-project
├── spock-host
└── spock-cli                 # [[bin]] name = "spock"

uhura/                        # unchanged git submodule boundary
└── crates/
    ├── uhura-core, uhura-check, ...
    ├── uhura-host
    └── uhura-cli
```

The dependency/ownership direction is:

```text
spock-cli
  ├── spock-project ── manifest, discovery, paths, scaffolding
  └── spock-host ───── project generations, routes, listener, lifecycle
        ├── spock-project ── project/snapshot types and path rules
        ├── spock-runtime ── one authority contract/database/API router
        │     └── spock-lang
        └── uhura-host ───── reusable Editor/Play host service

uhura-cli ── uhura-host       # standalone contributor/subsystem command
```

### `spock-project`

Owns `spock.toml` parsing and version diagnostics, root discovery, normalized
paths, the immutable project-input/snapshot data model, scaffold templates,
and adoption planning. Each language subsystem remains responsible for
enumerating and coherently capturing its semantic inputs through its library
boundary; `spock-project` must not duplicate Uhura's established
project-capture rules. It owns no filesystem watcher, HTTP listener, or live
database.

### `spock-host`

Owns the combined HTTP listener, route-collision policy, project-generation
construction and activation, lifecycle, shutdown, status/events, and the
coordination needed by both `dev` and `start`. For `dev`, that includes
filesystem observation, monotonic source-revision assignment, and coherent
cross-subsystem capture orchestration; the language subsystems still enumerate
and capture their own semantic inputs. The host consumes `spock-project`'s
project/snapshot types and normalized path rules; the CLI also consumes that
crate directly for `new`, `init`, and project discovery. `spock-host` is
clearer than `spock-dev-server`: it serves fixed generations too. It is clearer
than `spock-framework`, which names the product concept rather than a concrete
responsibility.

The host orchestrates whatever state capability RFD 0023 selects; it need not
own the state ABI mapper or rebase laws itself. Those may belong beside the
runtime or in a later focused crate once their boundary is understood.

### `spock-runtime`

Continues to implement one Spock authority generation. It should expose
constructible service/router boundaries, but it must not become the project
manifest reader, Uhura host, or master process supervisor.

### `uhura-host`

Should be extracted inside the submodule from the current
`uhura-cli::cmd::dev` behavior. It owns reusable Uhura Editor/Play state and
routes without binding its own mandatory listener or returning CLI-specific
exit codes. `uhura-cli` then remains a thin standalone entrypoint over it.

The logical dependency is settled more easily than the physical Cargo and
release linkage. A root crate path-depending directly on an uninitialized
submodule would end the current “core checkout builds without Uhura” property,
even if the dependency were feature-gated: Cargo still resolves path
manifests. Preserving that property requires a real package/manifest boundary,
such as a published internal Uhura crate, a vendored/generated artifact, or a
separate full-framework manifest/build that is used only when the submodule is
present. Requiring the submodule for the distributed binary is another
explicit option. This choice remains open; it must not be hidden inside the
crate diagram.

## 7. One host and one origin

One public listener and one browser origin are the preferred direction. An
illustrative, non-final route allocation is:

```text
/                         Uhura Play, Editor, or project landing (open)
/~editor                  Uhura Editor
/play                     Uhura Play (current convention; final route open)
/~studio                  Spock Studio
/~project/*               status, diagnostics, and generation events
/~contract                Spock contract
/graphql/v1
/rest/v1/*
/storage/v1/*
```

One origin gives the product:

- one process lifecycle and advertised URL;
- relative provider endpoints rather than committed localhost ports;
- no cross-origin configuration for the integrated path;
- one place to detect route collisions; and
- a possible atomic project-generation switch rather than two independently
  advancing servers.

The final route map remains open. In particular: `/` may be Play, Editor, or a
project landing page; `dev` and `start` may choose different primary surfaces;
and a backend-only project needs a useful root.

Current Spock uses Axum while Uhura's native host owns a `tiny_http` listener.
A real single listener therefore requires a service/router boundary from
Uhura. Two hidden listeners behind an umbrella reverse proxy are an acceptable
transition experiment, not the desired ownership model.

## 8. Both web products remain

The web playgrounds are not a reason to avoid the framework. They are separate
tools with separate authority:

- **Spock Studio** inspects authoritative contract, rows, personas, and
  protocol behavior.
- **Uhura Editor** presents checked client previews and authoring diagnostics.
- **Uhura Play** runs the experience and its provider link.
- **GraphiQL** remains a backend protocol tool.

They should be mounted separately, not collapsed into a single frontend or
renamed as though they did the same work. Each subsystem retains ownership of
its web source and browser-facing protocol.

The distributed build must eventually build and package both web products and
Uhura Wasm before compiling or assembling the `spock` package. RFD 0020's
current Studio non-empty guard should expand to verify every required asset
family and exercise their routes from the published npm package. Node and
Vite remain build-time dependencies; neither running server should need a
Node process.

The release design must explicitly answer what happens when a source checkout
lacks an initialized Uhura submodule. It may preserve language-only builds,
introduce a framework feature, or require the submodule for the public binary.
Silently shipping an empty Editor/Play is not an option.

## 9. Configuration and linker ownership

The composition pipeline is:

```text
backend/app.spock ──> checked Spock contract ──┐
                                               ├─> composition/link result
client/uhura.toml ──> checked Uhura program ───┘
```

`spock.toml` identifies these inputs and host policy. It must not redefine
either language. Uhura's required ports and Spock's exported surface should
eventually meet at a versioned linker/provider boundary.

The current application-owned TypeScript provider is a valid explicit adapter
and remains the integration proof. Some of that code is opaque to today's
language checkers. Project checks should validate every declared contract they
can prove and report the remaining adapter seam as unchecked, rather than
claiming a static link proof. A future linker may validate or generate more of
this seam, but the framework RFD cannot assume that contract exists before its
own design document and conformance tests.

When a client is configured, the host should supply same-origin endpoint
configuration to integrated Play at runtime. A committed Uhura profile can
still carry absolute URLs for standalone or remote-provider work; the local
framework path should not require the developer to keep two port declarations
synchronized.

## 10. Lifecycle

### Fixed `start`

1. Discover and validate `spock.toml`.
2. Capture the declared project files.
3. Compile/check the backend and any configured client.
4. Validate every currently declared/checkable provider contract and report
   opaque adapter seams.
5. Construct one immutable project-generation binding to its selected state.
6. Bind the public listener only after the generation is valid.
7. Serve that artifact/state binding unchanged until shutdown; ordinary
   application requests may still mutate the selected database.

### Watched `dev`

1. Capture one coherent saved project snapshot.
2. Prepare the backend and any configured client candidates from that same
   revision.
3. Reject stale/out-of-order work.
4. Activate the backend and, when configured, integrated Uhura Play only as a
   coherent pair.
5. Retain the last-good generation when any configured subsystem is rejected.
6. Publish current diagnostics and active-generation freshness separately.

When a client is configured, Uhura Editor may still publish the latest static
render and diagnostics while integrated Play remains paired to an older
backend. If the first project revision is invalid, `spock dev` may bind a
diagnostics/control shell without an active project generation; fixed `spock
start` may instead fail before binding. The exact cold-invalid route behavior
remains open.

The lifecycle stops there in this document. Whether a backend candidate
retains a database, creates a new world, rebases state, or requires a reset is
the subject of [RFD 0023](0023-development-state-reload.md).

## 11. Alternatives considered

### Add `spock-framework-cli`

Rejected as the working recommendation. It creates two possible owners of the
same public command, forces artificial dispatch between them, and encodes a
distinction the installed product is trying to remove.

### Rename the existing crate to `spock-language-cli`

Rejected. Language behavior already has homes in `spock-lang` and
`spock-runtime`; the CLI should become thinner, not fork into another user
surface. It also creates compatibility and publishing churn for no semantic
gain.

### Put Uhura into `spock-runtime`

Rejected. The authority runtime and composition host have different
responsibilities and different state. This would erase the boundary the two
languages were created to preserve.

### Keep the shell script and two public ports

Retained only as a proof and transition oracle. It cannot provide one origin,
project-level diagnostics, one packaged command, or atomic combined
generations.

### Merge repositories or Cargo workspaces

Rejected as a premise. Composition does not require giving up Uhura's
submodule, independent build, or public contract boundary.

### Call the client root `ui`

Rejected. It understates Uhura's language, state, and host responsibilities.

### Absorb `uhura.toml` into `spock.toml`

Rejected. It duplicates semantic authority, harms standalone checking, and
turns the composition manifest into a grab bag. Point to the client root;
leave the client contract there.

## 12. Working conclusions and open decisions

The recommendations this study asks the project to discuss are:

1. One installed/public command: `spock`.
2. `spock-cli` remains its crate and binary owner.
3. A required `spock.toml`, one required backend, and an optional client.
4. `backend/app.spock` is conventional and may represent an empty authority.
5. Project vocabulary is backend/client, not backend/UI.
6. `new`, `init`, `dev`, and `start` have distinct roles.
7. Add `spock-project` and `spock-host` responsibility crates.
8. Extract a reusable `uhura-host` inside the submodule.
9. Preserve Studio, Editor, Play, and GraphiQL as separate surfaces.
10. Prefer one listener and one origin.

The following remain open:

1. Exact `spock.toml` keys, versioning, and diagnostics.
2. Parent-directory project discovery behavior.
3. The default `new` template, whether it has `--backend-only`, and whether a
   client-focused form is useful (it would still contain the required empty
   backend).
4. Exact empty-authority HTTP and GraphQL behavior.
5. Final route allocation and root-page behavior.
6. The shared HTTP service abstraction between Axum and Uhura.
7. Public names for Uhura-specific `fmt`, `trace`, and Editor-only commands.
8. Whether multiple client/backend roots ever earn syntax.
9. Missing-submodule and language-only source-build behavior.
10. Release embedding versus a packaged internal helper/runtime.
11. The provider overlay and future linker format.
12. What operational guarantees the name `start` implies, including whether
    it always materializes fresh seed, may open a named state/world, accepts a
    database path, and exposes development-only browser tools.
13. Every backend state/reload decision delegated to RFD 0023.

## 13. Sequence before implementation

1. Land this framework record and
   [RFD 0023](0023-development-state-reload.md) as studies.
2. Decide the development-state model in
   [RFD 0023](0023-development-state-reload.md).
3. Add `spock-project` with manifest, discovery, scaffold, and adoption tests.
4. Refactor current file commands behind reusable library entrypoints.
5. Extract `uhura-host` without changing direct Uhura behavior.
6. Implement project-wide `check`, currently provable provider diagnostics,
   and an explicit unchecked-adapter report.
7. Compose one listener and both asset families.
8. Implement fixed-generation `start`.
9. Implement `dev` only against the chosen RFD 0023 semantics.
10. Expand RFD 0020's npm build and verification pipeline.

The ordering is deliberate: combining two live runtimes before deciding what
a backend save means would make the hardest product behavior an accidental
property of whichever host refactor lands first.

## 14. Acceptance scenarios for a later implementation

- `spock new demo` produces the documented project shape.
- `spock init` never overwrites or silently relocates existing sources.
- A client project with an empty `backend/app.spock` checks and starts.
- A backend-only project starts and exposes its backend tools.
- One public port serves the configured Spock APIs and browser tools.
- A broken half never publishes a mixed integrated-Play/backend generation;
  Editor freshness remains explicit.
- The npm package contains both web products and Uhura Wasm.
- `spock run app.spock` remains valid.
- Direct Uhura checks and tests remain independently runnable.

## 15. Related documents

- [RFD 0006](0006-language-identity-ir-first.md) — language identity, static
  runtime, and IR reload architecture.
- [RFD 0009](0009-roadmap.md) — the existing `spock run --watch` direction.
- [RFD 0010](0010-client-codegen-architecture.md) — client/codegen ownership;
  Uhura is not generated client code.
- [RFD 0015](0015-studio.md) — Spock Studio's embedded same-origin host.
- [RFD 0020](0020-distribution.md) — the npm-distributed `spock` binary and
  asset build.
- [RFD 0023](0023-development-state-reload.md) — saved-source reload and
  authoritative development state.
- [Uhura RFC 0001](https://github.com/gridaco/uhura/blob/42ece8e3c44efe89d3c9417761504e7b190db230/docs/rfcs/0001-project-foundation.md)
  — the language/runtime ownership boundary.
- [Uhura RFC 0002](https://github.com/gridaco/uhura/blob/42ece8e3c44efe89d3c9417761504e7b190db230/docs/rfcs/0002-model-driven-editor-live-updates.md)
  — coherent saved-source capture and last-known-good publication.
- [Uhura specification](https://github.com/gridaco/uhura/blob/42ece8e3c44efe89d3c9417761504e7b190db230/docs/spec/README.md)
  — current Uhura contract authority.

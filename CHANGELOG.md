# Changelog

All notable changes to Spock. Spock is a pre-1.0 prototype language; minor
versions may break.

## 0.5.4 — 2026-07-23

This release replaces the retired Uhura client with the strict, machine-first
Uhura 0.4 language and restores checked Web application ergonomics on top of
that kernel. The Spock authority language and contract are unchanged.

- **Machine-first Uhura 0.4.** Client state and transitions are expressed as a
  deterministic checked machine; UI is an explicit observation and event
  binding layer. The old UI-first client syntax has no compatibility parser.
- **Checked Web application topology.** The opt-in `web-app@1` profile
  discovers routed pages, pure components, surfaces, and colocated examples;
  it generates the route table and root `Application` as ordinary checked
  source rather than introducing another runtime.
- **Typed UI composition.** Reusable UI declarations have exact immutable
  props and finite emitted-event protocols. Calls are checked, acyclic, and
  wrapper-free, including explicitly imported public dependency components.
- **One project contract.** CLI checking and formatting, the native host,
  Editor, Play, source provenance, annotations, and the packaged Spock host
  now consume the same resolved Uhura project and Router selection.
- **Canonical application proof.** Instagram is organized as nine pages,
  eight pure components, and one surface while preserving its 91 Editor
  previews, replay evidence, dynamic routes, provider boundary, and live
  behavior.
- **Distribution and guidance.** The npm sidecar, release gates, documentation,
  and public Uhura skill now describe and exercise the same strict 0.4
  application contract. Source contributors also gain path-honest `just`
  wrappers for the local Spock CLI.

## 0.5.3 — 2026-07-17

This release refreshes the bundled Uhura editor and runtime with built-in
widget foundations, checked font icons, and richer structure connectors. The
Spock authority language and contract are unchanged.

- **Built-in widget foundations.** Uhura now has a canonical catalogue for
  `<scroll>`, `<icon>`, `<button>`, `<view>`, and `<img>`. This deliberately
  breaking pre-v1 update replaces `<image>` with `<img>` and `<text-field>`
  with `<textfield>`.
- **Checked icon fonts.** `<icon name="heart" />` uses bundled Lucide by
  default, while projects may register local WOFF2 families with JSON
  name-to-codepoint maps. Families and names are checked before rendering, and
  the core model carries semantic icon tokens instead of engine-owned paths or
  SVG commands.
- **Editor structure.** The bundled Uhura revision adds selected-frame
  structure connectors and refines workflow-connector routing and
  presentation, while keeping editor state and content-addressed font
  resources coherent across publication.
- **Distribution and docs.** Framework packaging now carries and verifies the
  icon-font resources through the native host. The Spock website adds an
  explicit technical-preview notice, while exploratory view work remains in
  the repository study surface.

## 0.5.2 — 2026-07-16

This release includes explicit product-error declarations as an experimental
RFD 0024 implementation preview, publishes the Spock website and language
governance framework, and syncs the bundled Uhura revision.

- **Explicit product errors.** Top-level `error name` declarations own a
  reusable product-error code and optional documentation. Function `!` clauses
  resolve against that registry, schema-derived errors, and function-applicable
  protocol errors; E051-E053 diagnose duplicates, unknown names, and ownership
  collisions.
- **Compatibility.** Programs that relied on implicit refusal minting must add
  one declaration per authored refusal. Authored refusals named `unauthorized`
  or `conflict` must be renamed because those codes are storage-protocol-owned.
  Wire envelopes, refusal status and rollback behavior remain unchanged.
- **Contract and tooling.** Contract JSON adds an additive top-level `errors`
  array. Generated TypeScript and Studio expose the registry, while the VS Code
  grammar highlights declarations and old contract JSON remains readable.
- **Uhura sync.** The pinned Uhura revision adds checked interaction-graph
  projection, reconstructed nested surface hierarchy, frame-avoiding workflow
  connectors, and explicit declarations in the canonical Instagram authority.
- **Project surfaces.** The repository now carries the spock.sh site, public
  Uhura skill, and contribution, governance, language-change, RFD, and
  working-group processes and templates.

RFD 0024 remains draft and non-normative. Its implementation preview is
experimental and unstable in `0.5.2`; inclusion does not record language
acceptance or a compatibility promise.

## 0.5.1 — 2026-07-15

This maintenance release refreshes the framework example and editor tooling
shipped after `0.5.0`.

- **Editor annotations and workflows.** The bundled Uhura revision preserves
  source-backed canvas comments alongside replay workflow connector lines;
  `Shift+Y` toggles the combined annotation layer.
- **Canonical Instagram project.** Contributor guidance now uses
  `uhura/examples/instagram`, and the duplicated proof-of-concept project and
  legacy two-process runner are retired.
- **VS Code language support.** A repository-owned grammar, language
  configuration, corpus, and installation guide track the Spock lexer and
  parser contracts.
- **Current CLI guidance.** Documentation and the public Spock skill use the
  installed framework CLI for project checks, starts, and watched development.

## 0.5.0 — 2026-07-15

This feature release makes the installed `spock` command the project-level
host for a Spock authority and an optional Uhura client while preserving every
standalone `.spock` language command.

- **Framework projects.** `spock new` creates the canonical project shape;
  `spock init` adopts existing sources without moving or overwriting them; and
  project-wide `check`, `start`, and `dev` compose both language subsystems.
- **One host.** Spock Studio, Uhura Editor and Play, framework status, and the
  authority protocols share one process, port, and origin.
- **Honest development reload.** Client generations hot-reload last-known-good.
  Backend source, seed assets, and topology changes are detected and reported
  as restart-required; they never migrate, reseed, or replace the active world.
- **Distribution.** The npm package adds an executable-bound Uhura
  web/WebAssembly sidecar beside the four prebuilt native binaries and exercises
  the exact guarded tarball on macOS, Linux, and Windows before release.
- **Safety.** Project creation is capability-pinned, create-new-only, portable
  across supported filesystems, and conservative under rollback and concurrent
  replacement.

Spock remains authoritative for durable facts, policy, and mutations. Uhura
owns presentation, experience behavior, and non-authoritative UI-session state;
composition does not make any fact authoritative in both languages.

## 0.4.0 — 2026-07-15

This feature release turns the derived data floor into a practical query and
write surface, and brings the embedded Studio with it.

- **Filtered reads.** GraphQL now derives typed `where` and `order_by` inputs;
  REST exposes the matching operator vocabulary. Both lower through one
  predicate engine, including boolean groups, reference-key traversal,
  deterministic multi-column ordering, and bounded offset paging.
- **Studio data workflows.** The table view adds contract-aware filters,
  multi-column sorting, paging, and URL-addressable navigation that survives
  reloads and browser back/forward.
- **Studio row insertion.** A metadata-driven side sheet derives controls from
  field types and distinguishes required, defaulted, and nullable values. It
  includes foreign-key and actor pickers, plus direct storage upload or
  selection of an existing committed object.
- **Browser clients.** The loopback development API now accepts cross-origin
  browser requests, including actor and content-type headers.
- **Compatibility.** REST filter control words (`order`, `limit`, `offset`,
  `select`, `and`, `or`, `not`) are now reserved as column names. The generated
  GraphQL filter types likewise add schema-name reservations.

Filtered and bulk writes, plus Studio row editing, remain deferred.

## 0.1.3 — 2026-07-13

First distributed release (RFD 0020). Spock is now installable from npm — no
clone, no build. (`0.1.0`–`0.1.2` were skipped while shaking out an npm
provenance-publish glitch that left those versions reserved-but-unpublished.)

- **Distribution.** `npm i -g spock` / `npx spock` on macOS (arm64, x64), Linux
  (x64), and Windows (x64). One npm package bundles a prebuilt binary per
  platform; a zero-dependency shim execs the one matching the host. Published
  tokenlessly from CI via npm Trusted Publishing (OIDC) with signed provenance,
  and verified install-and-run on all three platforms
  (`.github/workflows/npm.yml`).

Everything the language already did — `spock check` / `build` / `run` / `gen`,
the GraphQL + REST surface, the `/~studio` console, and storage (RFD 0018) —
now runs from that installed binary, offline.

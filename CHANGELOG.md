# Changelog

All notable changes to Spock. Spock is a pre-1.0 prototype language; minor
versions may break.

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

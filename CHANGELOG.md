# Changelog

All notable changes to Spock. Spock is a pre-1.0 prototype language; minor
versions may break.

## 0.1.2 — 2026-07-13

First distributed release (RFD 0020). Spock is now installable from npm — no
clone, no build. (`0.1.0` and `0.1.1` were skipped: an npm publish glitch left
those versions reserved-but-unpublished on the registry.)

- **Distribution.** `npm i -g spock` / `npx spock` on macOS (arm64, x64), Linux
  (x64), and Windows (x64). One npm package bundles a prebuilt binary per
  platform; a zero-dependency shim execs the one matching the host. Published
  tokenlessly from CI via npm Trusted Publishing (OIDC) with signed provenance,
  and verified install-and-run on all three platforms
  (`.github/workflows/npm.yml`).

Everything the language already did — `spock check` / `build` / `run` / `gen`,
the GraphQL + REST surface, the `/~studio` console, and storage (RFD 0018) —
now runs from that installed binary, offline.

# RFD 0020 — Distribution: shipping the `spock` binary

Status: **accepted; implemented and verified on `main`** (2026-07-13). v0
delivery is **npm-only** — every other channel is deferred behind it (§2). The
pipeline (`.github/workflows/npm.yml`) builds four platform binaries, publishes
the single `spock` package tokenlessly via OIDC trusted publishing, and
verifies install-and-run on macOS/Linux/Windows — `npx spock` works. First
published version: **`0.1.3`**. Three v0 simplifications departed from the
original draft — a single bundling package instead of `optionalDependencies`,
glibc instead of musl on Linux, and provenance attestation left off (it
intermittently races the large tarball's publish, §5) — each forced by a
concrete constraint and marked inline. The maintainer decisions that remain
genuinely open are in §10.

## 0. Where this fits

Today `spock` runs one way: `git clone`, `cargo build`, run the binary out of
`target/`. Everything the language has earned — `fn`, the value tier, the
studio console, storage — is invisible to anyone who cannot build the repo.
Distribution is the crux that turns Spock from *a repository you build* into
*a tool you install and run*. It ships no new language surface; it makes the
existing surface reachable.

Three doctrine anchors set the shape of the answer:

- **Borrow before build.** Prefer maintained tooling over hand-rolled scripts —
  *unless* the borrowed tool's value is mostly unused, in which case the
  hand-rolled surface is smaller. That caveat decides the orchestrator here (§5).
- **The audience is JS/TS application developers.** They build the prototypes
  that consume Spock; `spock gen` emits TypeScript; they reach for `npx` and
  `npm i` by reflex. npm is not just the front door — for v0 it is the only
  door, and that is a defensible scope because the audience already has Node.
- **Prototype, not production.** Distribution should be simple and
  low-maintenance, and it should stay honestly pre-1.0.

And three facts about the build — established by reading the crates, not
assumed — make this tractable and name the one hard part:

1. **One binary.** The workspace ships a single artifact, `spock` (from
   `spock-cli`), at a single workspace version (`0.0.1`).
2. **One native dependency.** The only C code in the shipped binary is
   `rusqlite`'s `bundled` SQLite, compiled from source via the `cc` crate.
   `reqwest` is a dev-dependency only, so the binary carries **no TLS/OpenSSL**
   — the usual Rust cross-compilation tar pit is absent. Every target needs
   only a C compiler, and one exists for every target we care about.
3. **The binary embeds a web app.** `rust-embed` bakes
   `crates/spock-runtime/studio/dist` into the binary at compile time. That
   directory is git-ignored except a `.gitkeep`; it is produced by
   `pnpm build` (tsc + vite). A `cargo build` that skips the SPA build
   **still succeeds** and ships a binary whose `/~studio` console serves
   nothing. This is the one genuine complication, and it is silent.

Two ownership facts are settled: the npm name **`spock` is owned** by the
project (today a reservation stub in `npm/`). The bare **`spock` crate name on
crates.io is taken** by an unrelated crate — which costs us nothing, since
crates.io is out of scope for v0 and a future presence there would publish the
`spock-lang` / `spock-runtime` / `spock-cli` names regardless.

## 1. The shape of the answer

Ship Spock as a **single npm package** — `spock` — that bundles a prebuilt
binary for every platform, with a ~20-line Node shim (`bin/spock.js`) that
resolves and execs the one matching the host's `os`+`arch` (§7). A user runs
`npx spock run app.spock` or `npm i -g spock`; no build step, no `postinstall`,
no network at install time.

Bundling every binary into one package — rather than the lighter esbuild-style
`optionalDependencies` (one package per platform) — is a deliberate v0 choice
forced by publishing. **npm Trusted Publishing (OIDC) is configured for the
`spock` package only.** Each `@scope/spock-<plat>` package would need its own
trusted-publisher config, which cannot be set until the package exists — a
bootstrap that needs a one-time token. Bundling keeps v0 **tokenless** through
the one already-trusted package. At ~5–6 MB per binary (LTO + strip), four
platforms pack to an ~11 MB tarball — cheap enough that simplicity wins.
`optionalDependencies` remains the documented end-state (§7), for when the
platform packages are worth bootstrapping.

The binaries are produced by a **small hand-rolled GitHub Actions workflow**,
`.github/workflows/npm.yml` (the filename the trusted publisher is pinned to):
a 4-target build matrix (each job builds the studio SPA, then compiles the
binary) uploads four artifacts; a publish job assembles them into the `spock`
package and publishes it via OIDC; a verify job installs the published package
on macOS/Linux/Windows and runs it. No `dist`, no GitHub Release, no installers,
no Homebrew, no crates.io — for now. Because the binaries are built in CI
regardless, every one of those deferred channels is nearly free to add later
(§2).

## 2. Channels

| Channel | Status | Note |
|---|---|---|
| **npm** — single `spock` package, all binaries bundled | **v0 (P0)** | The only user-facing channel. Name owned; publishes tokenlessly via OIDC. |
| README "Install" section | **v0 (P0)** | `npx spock` / `npm i -g spock`; nothing is discoverable without it. |
| `spock init` scaffolding | **v0 (P1)** | The only real repo-less first-run gap (`spock run` already works). |
| GitHub Release binaries | **deferred** | The same CI binaries, also uploaded to a Release — a fallback download for node-less users. ~10 lines. |
| `curl \| sh` + PowerShell installers | **deferred** | Generated by `dist` once we adopt it for the multi-channel pass. |
| Homebrew tap (`gridaco/homebrew-tap`) | **deferred** | `brew install` for the mac/Linux slice; formula points at the Release binaries above. |
| crates.io + `cargo binstall` | **deferred** | Bare `spock` name taken; would publish under `spock-*`. Publishes are permanent. |
| Docker · Scoop | **deferred** | Container/Windows niceties. |
| winget · generated landing page | **out** | Enterprise-grade friction / stale generator. |

**The npm-only tradeoff, stated honestly.** Node becomes a hard prerequisite:
a Linux CI box or a container without Node cannot get `spock` at v0. That is
acceptable because the audience already runs Node — and the escape hatch
(upload the CI binaries to a GitHub Release) is a ten-line addition whenever a
node-less user appears. npm-only is a strict subset of the full pipeline, not a
design we have to unwind.

## 3. The target matrix

The `bundled` SQLite compiles wherever a C compiler exists; every target below
has one on its native runner. Four binaries, three build jobs — one macOS
runner produces both Apple targets by cross-compiling within the Apple SDK.

| Rust triple | npm platform key | Runner | Note |
|---|---|---|---|
| `aarch64-apple-darwin` | `darwin-arm64` | `macos-14` | Native. |
| `x86_64-apple-darwin` | `darwin-x64` | `macos-14` | Same job: `rustup target add` + `--target`; Apple clang cross-compiles the bundled SQLite C to x86_64. |
| `x86_64-unknown-linux-gnu` | `linux-x64` | `ubuntu-22.04` | glibc, built on the older image for a wide glibc floor. **v0 ships gnu; musl is the follow-up (see below).** |
| `x86_64-pc-windows-msvc` | `win32-x64` | `windows-2025` | Native MSVC `cl.exe`. |

**Linux: glibc for v0, musl as the immediate follow-up.** The single bundling
package sidesteps the ecosystem's `libc`-field filtering bugs regardless — the
shim branches only on `os`+`arch`, never on which package installed, so there is
**no `detect-libc`**. That leaves glibc-vs-musl a pure question of *which* Linux
hosts the one `linux-x64` binary runs on. v0 ships **glibc** built on
`ubuntu-22.04` (glibc 2.35) to isolate CI variables for the first verified
release; it covers Ubuntu/Debian/Fedora/WSL and most non-Alpine containers.
**Static musl** — which additionally runs on Alpine/musl hosts — is a one-line
matrix change (target + `cargo-zigbuild` or `musl-tools`) once the pipeline is
proven; because `rusqlite`-on-musl has a history of subtle segfault reports,
that switch lands with a **CI smoke test** on the musl artifact (`spock run` a
fixture → hit `/graphql/v1` → assert 200) before it can publish. The build
matrix already smoke-tests each native-arch binary with `spock check`.

**`[profile.release]`:** `lto = true`, `codegen-units = 1`,
`strip = "symbols"`, `panic = "abort"`. **Keep `opt-level = 3`** (the release
default) — lowering it would feed a smaller `-O` to the `cc`-compiled SQLite,
and the size-vs-speed trade isn't worth taking for a prototype.

**Growth path** (same shape, new rows): `aarch64-unknown-linux-musl` on the
free `ubuntu-24.04-arm` runner → `linux-arm64`; `aarch64-pc-windows-msvc` on
`windows-11-arm`. Sunset the Intel-mac target when Apple and GitHub drop it
(~Fall 2027).

## 4. The studio-embed problem

`rust-embed` reads `studio/dist` **from disk at compile time**. If a build
skips `pnpm build`, cargo succeeds and the console is blank. In npm-only mode
this has a single, simple answer: **every build job runs `pnpm build` before
`cargo build`, guarded.** Per job:

1. `actions/checkout`
2. `pnpm/action-setup@v4`; `actions/setup-node@v4` with `node-version: 22`,
   `cache: pnpm`,
   `cache-dependency-path: crates/spock-runtime/studio/pnpm-lock.yaml`
3. `pnpm -C crates/spock-runtime/studio install --frozen-lockfile`
4. `pnpm -C crates/spock-runtime/studio build`
5. **guard:** fail if `studio/dist/index.html` is absent or trivially small
6. `cargo build --release --target <triple>` → `rust-embed` embeds a populated
   SPA.

The SPA output is platform-independent, so building it once and fanning it out
via `upload-artifact` would also work — but with only three jobs, the per-job
build is simpler (no artifact plumbing) *and* it validates that `pnpm build`
succeeds on macOS and Windows, which it has never been exercised on (the studio
has only ever been built on one Mac). That validation is worth the few extra
seconds. The non-empty guard makes the silent failure a hard one.

(The crates.io answer — package the built `dist/` into the crate tarball via
Cargo `include` plus a check-only `build.rs` — is deferred with crates.io
itself; it is recorded in §9/D5 so it isn't re-derived later.)

## 5. The release pipeline

**Triggers:** `workflow_dispatch` (with `version` / `dist_tag` / `dry_run`
inputs, for dry runs and prereleases under a dist-tag) and a pushed `vX.Y.Z`
tag (publishes `latest`). One hand-written `.github/workflows/npm.yml` that we
own — the filename is load-bearing, because the npm trusted-publisher config is
pinned to it.

```
dispatch / tag vX.Y.Z
 └─ build   (matrix: macos-14 → darwin-arm64 + darwin-x64; ubuntu-22.04 → linux-x64; windows-2025 → win32-x64)
 │    ├─ pnpm install + build studio              (§4)
 │    ├─ guard: studio dist/index.html non-empty
 │    ├─ cargo build --release --target …          (rust-embed bakes the SPA)
 │    ├─ smoke-test native-arch binaries (spock --version && spock check)
 │    └─ upload-artifact: bin-<key>
 └─ publish   (id-token: write)
 │    ├─ download all bin-* artifacts → assemble npm/binaries/<key>/ (+chmod +x on unix)
 │    ├─ guard: all four platforms present
 │    ├─ stamp the resolved version into package.json
 │    └─ npm publish   (OIDC, tokenless; --dry-run when the dry_run input is set)
 └─ verify   (skipped on dry runs; matrix: macos-14, ubuntu-22.04, windows-2025)
      ├─ npm i -g spock@<version>   (retry for registry propagation)
      ├─ spock --version && spock check <fixture>
      └─ [unix] spock run + curl /~studio  → proves the embedded console is served
```

**Publishing is tokenless.** The publish job carries `id-token: write` and
upgrades to the latest npm (Trusted Publishing needs ≥ 11.5.1); `npm publish`
then authenticates through the OIDC trusted publisher configured for `spock` —
no `NODE_AUTH_TOKEN`. **Provenance is disabled** (`--no-provenance`): with an
~11 MB tarball the attestation step intermittently races the package PUT and
the registry returns a false `E400 "cannot publish over previously published
version"` — which *burns* the version (it's reserved but never served, and can't
be reused). Publishing to a `next` dist-tag happened to dodge it while `latest`
hit it repeatedly; disabling provenance removed the race entirely. Tokenless
auth is independent of provenance, so nothing else changes. A `dry_run` input
gates real publishes so the cross-platform build can be proven without spending
an npm version; prereleases go out under a `next` dist-tag so `latest`
only ever moves on a real cut.

**Why hand-rolled, not `dist`.** `dist`'s value is the matrix + C-toolchain
provisioning + installers + Homebrew formula + GitHub Release, generated
together. In npm-only mode we use only the first two, publish a bespoke
single-package layout it doesn't model, and would be fighting its generated
workflow to *not* emit the outputs we don't want. The hand-rolled surface is
~200 lines of YAML over `actions/setup-node`, `pnpm/action-setup`,
`dtolnay/rust-toolchain`, and `actions/{upload,download}-artifact`. `dist`
becomes the right borrow **later**, when installers + Homebrew are added,
because then it regenerates all of them from one config; the deferred-channel
note in §2 is where it re-enters. `release-plz` is not used (Spock's commit
style isn't Conventional Commits, so its auto-changelog would be noise).

## 6. Versioning

- **Single source of truth:** `[workspace.package] version`. The git tag
  mirrors it; every npm package version equals it, exact-pinned.
- **Trigger:** bump the version → commit → `git tag vX.Y.Z && git push --tags`.
- **First public release is `0.1.3`.** `0.0.1` was a name-reservation
  placeholder; the first real distributed release is `0.1.x`, staying honestly
  pre-1.0 (`1.0.0` is reserved for a stability commitment Spock is not making
  yet). `0.1.0`–`0.1.2` were burned by the provenance-publish race (§5): each
  hit the false "already published" *after* its provenance attestation was
  recorded, leaving it reserved-but-unserved and unrecoverable. Disabling
  provenance fixed it and `0.1.3` published cleanly. A version can never be
  reused; a burned one is simply skipped.
- **Changelog: hand-curated `CHANGELOG.md`.** Given the commit style,
  auto-generation is noisier than a short hand-written "what changed in the
  language surface" per release.

## 7. The npm package

v0 is a **single package** — `spock` — carrying the shim and all four platform
binaries. Zero runtime dependencies, no `postinstall`, no network at install.

```jsonc
// npm/package.json  (the reservation stub, grown up)
{
  "name": "spock", "version": "0.1.0",
  "bin": { "spock": "bin/spock.js" },
  "files": ["bin/", "binaries/"],
  "publishConfig": { "access": "public", "provenance": false }
}
```

The tree published to npm:

```
spock/
  bin/spock.js                 the shim (committed)
  binaries/darwin-arm64/spock  ┐
  binaries/darwin-x64/spock    │ assembled in CI from the build artifacts
  binaries/linux-x64/spock     │ (git-ignored; never committed)
  binaries/win32-x64/spock.exe ┘
```

The shim resolves the host's `os`+`arch`, checks the bundled binary exists, and
execs it with argv verbatim, propagating the child's exit code. Detecting
`os`+`arch` at runtime (rather than trusting which package installed) is the
portable path across npm/pnpm/yarn/bun:

```js
const key = `${process.platform}-${process.arch}`;              // e.g. darwin-arm64
const exe = process.platform === "win32" ? "spock.exe" : "spock";
const bin = path.join(__dirname, "..", "binaries", key, exe);
if (!fs.existsSync(bin)) { /* clear error, exit 1 */ }
execFileSync(bin, process.argv.slice(2), { stdio: "inherit" });
```

The unix binaries are `chmod +x`'d in CI before packing (`upload-artifact`
drops the exec bit); npm preserves the mode into the tarball, so the installed
binary is executable. The whole package is ~11 MB packed / ~23 MB unpacked.

**Version.** CI stamps `package.json` to the version resolved from the tag or
dispatch input, so the npm version always equals the Rust build it wraps. The
`spock` reservation stub is grown in place (`main`/`index.js` dropped — a CLI
wrapper needs only `bin`); the unrelated legacy 2014 `spock` versions are
`npm deprecate`d so a range-less `npm i spock` can't resolve an old `0.3.x`.

**The `optionalDependencies` end-state (deferred).** The lighter, canonical
layout (esbuild, `@swc/core`, `@biomejs/biome`, oxlint) is a thin `spock`
wrapper listing one prebuilt-binary package per platform as exact-pinned
`optionalDependencies` (each with `os`/`cpu`), so a user downloads only their
platform's binary (~5 MB, not ~11 MB). v0 does **not** use it: each
`@scope/spock-<plat>` package needs its own trusted-publisher config, which
can't be set until the package exists — a bootstrap requiring a one-time token,
against the tokenless goal. When the download-size saving is worth the
bootstrap: create a scope (recommend `@gridaco`, matching the GitHub org),
publish the platform packages **first** and the wrapper **last** (else its
optional deps 404), and the same shim — already keyed on `os`+`arch` — drives
it unchanged.

## 8. First-run experience

A distributed user has the binary and their own `.spock` file, no repo.
`spock run app.spock` already works standalone (disposable state, embedded
SQLite, self-served studio), so the onboarding gap is small — one thing:

- **`spock init [name]` (P1)** — write a minimal starter `.spock` (an
  `include_str!`'d template) so a first-time user has something to `run`
  immediately.
- `spock run --watch` is the live "executable PRD" demo (roadmap track 9);
  independently valuable and pairs well here, but P2 for distribution.

## 9. Decisions

| # | Decision | Recommendation | Trade-off |
|---|---|---|---|
| D1 | Orchestrator | **Hand-rolled `npm.yml`** for npm-only; adopt `dist` later when installers + brew are added | ~200 lines we own vs bending a generator; `dist` re-enters when its unused outputs become wanted. |
| D2 | npm layout | **Single package, all binaries bundled** (not `optionalDependencies`) | ~11 MB download; but tokenless through the one trusted `spock` package. `optionalDependencies` deferred until worth a bootstrap token (§7). |
| D3 | Linux binary | **glibc for v0** (`ubuntu-22.04`); static-musl as the follow-up | glibc covers most hosts and isolates first-release CI variables; musl (Alpine) is a one-line matrix change + smoke test. |
| D4 | Studio prebuild | **plain `pnpm build` step per job + non-empty guard** | Rebuilds the SPA in each build job (cheap) and validates the mac/Windows build. |
| D5 | Studio for crates.io | *(deferred)* `include = ["studio/dist/**"]` + check-only `build.rs` | Recorded so it isn't re-derived when crates.io is picked up. |
| D6 | npm name | **`spock` (owned)**; no scope needed at v0 (single package) | A scope (`@gridaco`) only becomes relevant if `optionalDependencies` is adopted. |
| D7 | First version | **`0.1.0`** | Signals a real release; keeps `1.0` for a promise not yet made. |
| D8 | Changelog | **Hand-curated `CHANGELOG.md`** | Manual, but the commit style defeats auto-generators. |
| D9 | crates.io | **deferred** (bare `spock` taken; irrelevant at v0) | A future presence publishes under `spock-*`. |
| D10 | Housekeeping | **Delete the stray root `now` file; keep `studio/dist/.gitkeep`** | Zero cost; the `.gitkeep` keeps the rust-embed folder present on fresh checkouts. |
| D11 | First-run UX | **Add `spock init` (P1)**; `--watch` P2 | Closes the only repo-less onboarding gap. |

## 10. Open questions for the maintainer

1. **Switch Linux to static-musl now, or ship glibc for v0?** v0 ships glibc
   (D3); flipping to musl (adds Alpine/musl hosts) is a one-line matrix change
   plus a smoke test whenever an Alpine user appears.
2. **macOS `universal2` (one fat binary via `lipo`) vs two thin slices?**
   Recommend two thin slices for v0; `universal2` only if a future single-file
   download UX wants it.
3. **Adopt `optionalDependencies` later?** Worth it once the ~11 MB download
   matters more than the one-time token needed to bootstrap the platform
   packages (§7); until then the single package is simpler and stays tokenless.
4. **When do the deferred channels turn on?** Recommend adding the GitHub-Release
   upload (near-free) the first time a node-less user needs a binary, and the
   `dist`-driven installers + Homebrew pass once there's traction to justify it.

## 11. Phased rollout

**P0 — `npm i -g spock` (or `npx spock`) and run it. _Done._**
- [x] Housekeeping: `git rm now`; `studio/dist/.gitkeep` kept committed.
- [x] Add the `[profile.release]` levers (§3); keep `opt-level = 3`. Binary ~5 MB.
- [x] Trusted publishing configured for `spock` (OIDC; no token). _(maintainer)_
- [x] Grow `npm/` into the `spock` package + `bin/spock.js` shim (§7).
- [x] Write `.github/workflows/npm.yml`: the 4-target build matrix (pnpm build
      studio → non-empty guard → cargo build → `spock check` smoke → upload
      artifact), the publish job (assemble all binaries, stamp version, OIDC
      `npm publish`, dry-run gate), and the cross-OS verify job.
- [x] Bump the workspace version; disable provenance (§5); ship `0.1.3`
      (`0.1.0`–`0.1.2` burned by the provenance race).
- [x] Verify e2e: dry-run → `0.1.0-rc.1` under `next` → `0.1.3` to `latest`;
      CI verify installs from npm and runs on macOS/Linux/Windows, and a final
      `npx spock@latest` on a dev Mac renders `/~studio` and serves `/~contract`.
- [x] Add the README "Install" section (`npx spock` primary).
- [x] Write `CHANGELOG.md`.

**P1 — smoother first run.**
- [ ] Re-enable provenance once the tarball is smaller (optionalDependencies)
      or via a post-attestation-`E400`-tolerant publish retry (§5, §12).
- [ ] Switch `linux-x64` to static-musl (adds Alpine) + its smoke test (D3).
- [ ] Add `spock init [name]` (embed a starter via `include_str!`).
- [ ] Add the `aarch64-unknown-linux-*` and `aarch64-pc-windows-msvc` targets.

**Later — the deferred channels (each cheap because the binaries already exist).**
- [ ] Upload the CI binaries to a GitHub Release (node-less fallback).
- [ ] Adopt `dist` for `shell`/`powershell` installers + the Homebrew tap.
- [ ] crates.io: version the path-deps, publish `spock-lang`/`spock-runtime`/
      `spock-cli` in topological order, add `[package.metadata.binstall]` +
      the `include`/`build.rs` studio fix (D5).
- [ ] Docker (musl → distroless, GHCR); Scoop bucket.

## 12. Risks and maintenance burden

Honest accounting, framed for a low-maintenance prototype.

- **Silent empty studio (medium unguarded → low with the guard).** The
  signature failure: cargo build succeeds, the console is blank. *Mitigation:*
  the §4 non-empty `index.html` guard makes it a hard failure; P0 verification
  renders `/~studio` on macOS and Windows.
- **npm publish atomicity (low).** The single-package design publishes one
  package per release, so there is no partial/half-released state — the whole
  binary set is in one tarball. The residual rule is npm's own: a version can't
  be reused, so never re-tag a version. *Mitigation:* the `dry_run` gate proves
  the build before any real publish; prereleases use a `next` dist-tag.
- **Provenance is off (accepted; supply-chain nicety deferred).** With an
  ~11 MB tarball the provenance attestation intermittently races the package
  PUT and burns the version (§5); it cost `0.1.0`–`0.1.2` before we disabled it.
  Tokenless OIDC auth is unaffected; only the signed SLSA attestation is missing.
  *Re-enable path (P1):* shrink the per-publish tarball via the
  `optionalDependencies` split (§7), or add a bounded publish retry that treats
  a post-attestation `E400` as success-if-the-version-appears.
- **Linux is glibc-only at v0 (accepted).** Alpine/musl hosts aren't served
  until the musl follow-up (D3). *Mitigation:* covers the large majority of the
  audience's Linux now; musl is a one-line matrix change + smoke test.
- **Windows studio build (now proven).** The SPA build had only run on one Mac;
  the build matrix exercises `pnpm build` on Windows and macOS every release,
  and the verify job renders `/~studio` live on macOS/Linux.
- **npm-only means Node is required (accepted).** No binary for node-less
  environments at v0. *Mitigation:* the deferred GitHub-Release upload is a
  ten-line addition the day it's needed.

**Net:** after P0, a release is *bump version → tag → push*, and one tag builds
four binaries and publishes one npm package that bundles them — verified live on
macOS, Linux, and Windows. The single hand-owned piece — the ~20-line shim — is
exactly the place where every borrowed default was the wrong fit, and the
pipeline stays a strict subset that the deferred channels extend without rework.

# RFD 0020 — Distribution: shipping the `spock` binary

Status: **accepted**. The original npm-only binary pipeline was implemented and
verified on `main` (2026-07-13); the RFD 0022 framework-sidecar extension is
implemented in the release workflow, and its first `0.5.0` full-matrix dry run
passed on 2026-07-15
([Actions run 29379605382](https://github.com/gridaco/spock/actions/runs/29379605382)).
v0 delivery remains **npm-only** — every other channel is deferred behind it
(§2). The pipeline (`.github/workflows/npm.yml`) builds four platform binaries
and one shared framework asset sidecar, publishes the single `spock` package
tokenlessly via OIDC trusted publishing, and verifies install-and-run on
macOS/Linux/Windows — `npx spock` works. First published version:
**`0.1.3`**. Three v0 simplifications departed from the
original draft — a single bundling package instead of `optionalDependencies`,
glibc instead of musl on Linux, and provenance attestation left off (it
intermittently races the large tarball's publish, §5) — each forced by a
concrete constraint and marked inline. The maintainer decisions that remain
genuinely open are in §10.

Local framework acceptance assembles the exact 21-file package topology and
sidecar inventory. Release CI remains authoritative for the four real platform
artifacts and rejects any package above 26 MiB. The `0.5.0` dry run installed
and exercised that exact guarded tarball on macOS, Linux, and Windows after all
four native targets built successfully; verification against the first real
framework publish remains pending.

## 0. Where this fits

Before this pipeline, `spock` ran one way: `git clone`, `cargo build`, then run
the binary out of `target/`. Distribution is the crux that turned Spock from
*a repository you build* into *a tool you install and run*. The RFD 0022
extension now distributes the framework host's Uhura assets beside that same
binary; it does not create a second CLI or release channel.

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
   `spock-cli`), at the single `[workspace.package]` version.
2. **One native dependency.** The only C code in the shipped binary is
   `rusqlite`'s `bundled` SQLite, compiled from source via the `cc` crate.
   `reqwest` is a dev-dependency only, so the binary carries **no TLS/OpenSSL**
   — the usual Rust cross-compilation tar pit is absent. Every target needs
   only a C compiler, and one exists for every target we care about.
3. **The binary embeds one web app and distributes another as a sidecar.**
   `rust-embed` bakes
   `crates/spock-runtime/studio/dist` into the binary at compile time. That
   directory is git-ignored except a `.gitkeep`; it is produced by
   `pnpm build` (tsc + vite). A `cargo build` that skips the SPA build
   **still succeeds** and ships a binary whose `/~studio` console serves
   nothing. The framework host also serves Uhura Editor/Play and Wasm from a
   shared executable-relative directory. Those assets are platform-independent
   and belong once in the npm package, not once in every native binary.

Two ownership facts are settled: the npm name **`spock` is owned** by the
project and the real package lives in `npm/`. The bare **`spock` crate name on
crates.io is taken** by an unrelated crate — which costs us nothing, since
crates.io is out of scope for v0 and a future presence there would publish the
`spock-lang` / `spock-runtime` / `spock-cli` names regardless.

## 1. The shape of the answer

Ship Spock as a **single npm package** — `spock` — that bundles a prebuilt
binary for every platform, one shared Uhura web/Wasm sidecar, and a small Node
shim (`bin/spock.js`) that resolves and owns the binary matching the host's
`os`+`arch` (§7). A user runs `npx spock new demo`, retains
`npx spock run app.spock` as the language escape hatch, or installs globally;
there is no build step, `postinstall`, or network access at install time.

Bundling every binary into one package — rather than the lighter esbuild-style
`optionalDependencies` (one package per platform) — is a deliberate v0 choice
forced by publishing. **npm Trusted Publishing (OIDC) is configured for the
`spock` package only.** Each `@scope/spock-<plat>` package would need its own
trusted-publisher config, which cannot be set until the package exists — a
bootstrap that needs a one-time token. Bundling keeps v0 **tokenless** through
the one already-trusted package. The workflow's hard 26 MiB limit keeps the
all-platform trade-off explicit and makes package growth fail visibly. Uhura
0.4 moved the original limit by one MiB only after the exact 21-file dry-run
package measured 26,501,976 bytes; the increase is accounted for by four
statically linked copies of the language engine and the expanded Wasm sidecar,
not an accidental file. That remains cheap enough for v0 that simplicity wins.
`optionalDependencies` remains the documented end-state (§7), for when the
platform packages are worth bootstrapping.

The package is produced by an **explicit hand-owned GitHub Actions workflow**,
`.github/workflows/npm.yml` (the filename the trusted publisher is pinned to):
an assets job builds and inventories Uhura once and publishes the exact raw
manifest SHA-256; a dependent 4-target build matrix (each job builds the Studio
SPA, then compiles that identity into the binary) uploads four native artifacts;
a publish job assembles them into the `spock` package and publishes it via OIDC;
a verify job installs either the exact guarded dry-run tarball or the published
registry package on macOS/Linux/Windows and runs it. No `dist`, no GitHub
Release, no installers, no Homebrew, no crates.io — for now. Because the
binaries are built in CI regardless, every one of those deferred channels is
nearly free to add later (§2).

## 2. Channels

| Channel | Status | Note |
|---|---|---|
| **npm** — single `spock` package, all binaries bundled | **v0 (P0)** | The only user-facing channel. Name owned; publishes tokenlessly via OIDC. |
| README "Install" section | **v0 (P0)** | `npx spock` / `npm i -g spock`; nothing is discoverable without it. |
| `spock new` / `spock init` scaffolding | **framework (implemented)** | Create a canonical project or adopt existing sources without a checkout. |
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

The `bundled` SQLite compiles wherever a C compiler exists. Four matrix jobs
produce four binaries; the two Apple jobs share the `macos-14` runner image,
and its SDK cross-compiles the Intel slice.

| Rust triple | npm platform key | Runner | Note |
|---|---|---|---|
| `aarch64-apple-darwin` | `darwin-arm64` | `macos-14` | Native. |
| `x86_64-apple-darwin` | `darwin-x64` | `macos-14` | Apple clang cross-compiles the bundled SQLite C; `file` verifies the architecture and Rosetta executes version and language-check smokes. |
| `x86_64-unknown-linux-gnu` | `linux-x64` | `ubuntu-22.04` | glibc, built on the older image for a wide glibc floor. **v0 ships gnu; musl is the follow-up (see below).** |
| `x86_64-pc-windows-msvc` | `win32-x64` | `windows-2025` | Native MSVC `cl.exe`. |

**Linux: glibc for v0, musl as the immediate follow-up.** The single bundling
package sidesteps the ecosystem's `libc`-field filtering bugs regardless — the
shim branches only on `os`+`arch`, never on which package installed, so there is
**no `detect-libc`**. That leaves glibc-vs-musl a pure question of *which* Linux
hosts the one `linux-x64` binary runs on. v0 ships **glibc** built on
`ubuntu-22.04` (glibc 2.35) to isolate CI variables for the first verified
release; it covers Ubuntu/Debian/Fedora/WSL and most non-Alpine containers.
**Static musl** — which additionally runs on Alpine/musl hosts — requires a
separate target and linker toolchain (`cargo-zigbuild` or `musl-tools`). Because
`rusqlite`-on-musl has a history of subtle segfault reports, that addition lands
with a **CI smoke test** on the musl artifact (`spock run` a
fixture → hit `/graphql/v1` → assert 200) before it can publish. The build
matrix already smoke-tests each native-arch binary with `spock check`.
The npm shim detects a reported non-glibc Node runtime before launch and gives
an explicit Alpine/musl error instead of exposing a native-loader failure.

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
2. `pnpm/action-setup@v4`; `actions/setup-node@v4` with the repository's
   Node 24 LTS `.nvmrc`,
   `cache: pnpm`,
   `cache-dependency-path: crates/spock-runtime/studio/pnpm-lock.yaml`
3. `pnpm -C crates/spock-runtime/studio install --frozen-lockfile`
4. `pnpm -C crates/spock-runtime/studio build`
5. **guard:** fail if `studio/dist/index.html` is absent or trivially small
6. `cargo build --release --target <triple>` → `rust-embed` embeds a populated
   SPA.

The SPA output is platform-independent, so building it once and fanning it out
via `upload-artifact` would also work — but with only four target jobs, the per-job
build is simpler (no artifact plumbing) *and* it validates that `pnpm build`
succeeds on macOS and Windows, which it has never been exercised on (the studio
has only ever been built on one Mac). That validation is worth the few extra
seconds. The non-empty guard makes the silent failure a hard one.

(The crates.io answer — package the built `dist/` into the crate tarball via
Cargo `include` plus a check-only `build.rs` — is deferred with crates.io
itself; it is recorded in §9/D5 so it isn't re-derived later.)

The Uhura assets have a different ownership shape from embedded Studio. One
Linux `assets` job initializes the pinned Uhura submodule, uses pnpm 10.11.0,
runs the complete `uhura/web` check/build gate, builds the lockfile-matched
`wasm-bindgen` web bundle, and
assembles `share/spock/uhura/{web,wasm}` once. A
`spock-asset-sidecar/1` manifest records the root and Uhura commits, every
spoken framework/Uhura protocol, and a sorted SHA-256/size inventory of every
asset. Manifest paths use a portable ASCII segment grammar, reject Windows
device names and case-folded collisions, and sort by UTF-8 bytes (equivalent to
ASCII byte order here). Publish and installed-package verification both
recheck the exact inventory; the four binary jobs never duplicate or rebuild
the sidecar.

## 5. The release pipeline

**Triggers:** `workflow_dispatch` (with optional `version` assertion plus
`dist_tag` / `dry_run`) and a pushed semver tag. A dispatch without `version`
derives it from `[workspace.package]`; any supplied value and every pushed tag
must equal that source of truth. Stable tags publish `latest`, while tags with
a prerelease suffix publish `next`. One hand-written
`.github/workflows/npm.yml` is owned here — the filename is load-bearing,
because the npm trusted-publisher config is pinned to it.

```
dispatch / tag vX.Y.Z
 ├─ assets  (once, ubuntu-22.04)
 │    ├─ initialize recursive submodules
 │    ├─ pnpm 10.11.0 install + full Uhura web/provider check and build
 │    ├─ build Uhura Wasm with the lockfile-exact wasm-bindgen CLI
 │    ├─ guard web routes, Wasm artifacts, hashes, sizes, and protocols
 │    └─ upload-artifact: framework-assets
 ├─ build   (matrix: macos-14 → darwin-arm64 + darwin-x64; ubuntu-22.04 → linux-x64; windows-2025 → win32-x64)
 │    ├─ pnpm install + build studio              (§4)
 │    ├─ guard: studio dist/index.html non-empty
 │    ├─ cargo build --locked --release --target … (rust-embed bakes the SPA)
 │    ├─ smoke native binaries; execute macOS x64 through Rosetta
 │    └─ upload-artifact: bin-<key>
 └─ publish   (id-token: write)
 │    ├─ download bin-* and framework-assets
 │    ├─ assemble npm/binaries/<key>/ (chmod 0755 on Unix)
 │    ├─ guard: all four platforms and the sidecar are present
 │    ├─ stamp the resolved version into package.json
 │    ├─ npm pack: exact file set, exact 0755 executables, tarball ≤ 26 MiB
 │    ├─ upload the guarded tarball as a one-day workflow artifact
 │    └─ npm publish the same guarded .tgz (OIDC, tokenless; --dry-run when requested)
 └─ verify   (matrix: macos-14, ubuntu-22.04, windows-2025)
      ├─ dry run: install the exact guarded tarball workflow artifact
      ├─ publish: npm i -g spock@<version> (retry for registry propagation)
      ├─ assert npm-visible and full Cargo/binary versions independently
      ├─ spock --version && spock check <fixture>
      ├─ verify the installed sidecar manifest and file hashes
      ├─ [unix] spock run + curl /~studio  → proves the embedded console is served
      └─ [all OSes] spock new + start + route probes → proves Editor/Play/Wasm
         and the framework status/environment protocols are served
```

**Publishing is tokenless.** The publish job carries `id-token: write` and
installs pinned npm 11.6.2 (Trusted Publishing needs ≥ 11.5.1); `npm publish`
then authenticates through the OIDC trusted publisher configured for `spock` —
no `NODE_AUTH_TOKEN`. **Provenance is disabled** (`--no-provenance`): with the
original ~11 MB binary-only tarball, the attestation step intermittently races
the package PUT and the registry returns a false `E400 "cannot publish over
previously published version"` — which *burns* the version (it's reserved but
never served, and can't be reused). Publishing to a `next` dist-tag happened to
dodge it while `latest` hit it repeatedly; disabling provenance removed the
race entirely. Tokenless auth is independent of provenance, so nothing else
changes. A `dry_run` input gates real publishes while the exact guarded tarball
is installed and exercised on the full macOS/Linux/Windows verification matrix
without spending an npm version. Prereleases go out under a `next` dist-tag so
`latest` only ever moves on a real cut. Both dry and real `npm publish` receive
that same guarded `.tgz` as their package argument; neither branch silently
repacks the source directory. The dry-run command alone uses `--force`, because
npm otherwise rejects `--dry-run` when the workspace version is already in the
registry; the real publishing branch never uses `--force`.

**Why hand-rolled, not `dist`.** `dist`'s value is the matrix + C-toolchain
provisioning + installers + Homebrew formula + GitHub Release, generated
together. In npm-only mode we use only the first two, publish a bespoke
single-package layout it doesn't model, and would be fighting its generated
workflow to *not* emit the outputs we don't want. The hand-owned surface is now
about 500 lines because it includes framework-asset assembly, exact package
guards, and installed cross-platform route smokes, over `actions/setup-node`,
`pnpm/action-setup`,
`dtolnay/rust-toolchain`, and `actions/{upload,download}-artifact`. `dist`
becomes the right borrow **later**, when installers + Homebrew are added,
because then it regenerates all of them from one config; the deferred-channel
note in §2 is where it re-enters. `release-plz` is not used (Spock's commit
style isn't Conventional Commits, so its auto-changelog would be noise).

## 6. Versioning

- **Single source of truth:** `[workspace.package] version`. The git tag
  mirrors it exactly. Cargo's parsed version is the SemVer authority; release CI
  does not maintain a second regex. The workflow removes `+build` metadata from
  npm's registry-visible version because npm's version-stamping command does
  not preserve it, while the binary retains the complete Cargo version. CI also
  uses that metadata-free spelling for prerelease channel selection, so a
  hyphen inside build metadata cannot select `next`.
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

v0 is a **single package** — `spock` — carrying the shim, all four platform
binaries, and one shared Uhura sidecar. Zero runtime dependencies, no
`postinstall`, no network at install.

```jsonc
// npm/package.json  (the former reservation, now the real distribution)
{
  "name": "spock", "version": "<workspace-version>",
  "bin": { "spock": "bin/spock.js" },
  "files": ["bin/", "binaries/", "share/", "THIRD_PARTY_NOTICES.md"],
  "publishConfig": { "access": "public", "provenance": false }
}
```

The tree published to npm:

```
spock/
  package.json                 npm metadata and `spock` bin declaration
  README.md                    installed-package usage
  LICENSE                      project license
  THIRD_PARTY_NOTICES.md       bundled Uhura and Wasm notices
  bin/spock.js                 the shim (committed)
  binaries/darwin-arm64/spock  ┐
  binaries/darwin-x64/spock    │ assembled in CI from the build artifacts
  binaries/linux-x64/spock     │ (git-ignored; never committed)
  binaries/win32-x64/spock.exe ┘
  share/spock/uhura/
    manifest.json               executable-bound integrity + compatibility
    web/                        Uhura Editor and Play browser application
    wasm/                       wasm-bindgen web module and Wasm binary
```

The shim resolves the host's `os`+`arch`, checks the bundled binary exists, and
spawns it with argv verbatim. It forwards terminal signals and propagates the
child's exit status so long-lived framework commands retain one owner.
Detecting
`os`+`arch` at runtime (rather than trusting which package installed) is the
portable path across npm/pnpm/yarn/bun:

```js
const key = `${process.platform}-${process.arch}`;              // e.g. darwin-arm64
const exe = process.platform === "win32" ? "spock.exe" : "spock";
const bin = path.join(__dirname, "..", "binaries", key, exe);
if (!fs.existsSync(bin)) { /* clear error, exit 1 */ }
const child = spawn(bin, process.argv.slice(2), { stdio: "inherit" });
// Forward SIGINT/SIGTERM/SIGHUP; translate close into the shim's exit status.
```

The committed shim is mode `0755`, and CI restores that exact mode on the Unix
native binaries (`upload-artifact` drops native executable bits); npm preserves
those modes in the tarball. Release CI rejects any missing or unexpected packed
path, checks the shim plus all three Unix native binaries are exactly `0755`,
and measures the actual artifact against the framework's 26 MiB budget.

**Version.** CI derives the version from `[workspace.package]`; a tag or an
optional dispatch value is an exact assertion, never an independent source. It
stamps `package.json` with the Cargo version minus optional `+build` metadata,
which npm does not preserve in its registry version, and separately verifies
that the installed binary reports the complete Cargo version. The former
`spock` reservation stub was grown in place
(`main`/`index.js` dropped — a CLI wrapper needs only `bin`); the unrelated
legacy 2014 `spock` versions are
`npm deprecate`d so a range-less `npm i spock` can't resolve an old `0.3.x`.

**The `optionalDependencies` end-state (deferred).** The lighter, canonical
layout (esbuild, `@swc/core`, `@biomejs/biome`, oxlint) is a thin `spock`
wrapper listing one prebuilt-binary package per platform as exact-pinned
`optionalDependencies` (each with `os`/`cpu`), so a user downloads only their
platform's native binary plus the shared sidecar instead of every platform
binary. v0 does **not** use it: each
`@scope/spock-<plat>` package needs its own trusted-publisher config, which
can't be set until the package exists — a bootstrap requiring a one-time token,
against the tokenless goal. When the download-size saving is worth the
bootstrap: create a scope (recommend `@gridaco`, matching the GitHub org),
publish the platform packages **first** and the wrapper **last** (else its
optional deps 404), and the same shim — already keyed on `os`+`arch` — drives
it unchanged.

## 8. First-run experience

A distributed user has the binary and no checkout. `spock run app.spock`
remains the standalone language escape hatch. RFD 0022 replaces the earlier
undifferentiated `spock init [name]` sketch with two project commands:
`spock new NAME` creates the canonical full-stack project (or
`--backend-only`), while `spock init [path]` adopts existing sources without
moving or overwriting them. Canonical scaffold bytes stay embedded in the
binary; the sidecar is runtime browser machinery, not a template dependency.

## 9. Decisions

| # | Decision | Recommendation | Trade-off |
|---|---|---|---|
| D1 | Orchestrator | **Hand-rolled `npm.yml`** for npm-only; adopt `dist` later when installers + brew are added | About 500 explicit lines including framework/package verification; `dist` re-enters when its currently unused outputs become wanted. |
| D2 | npm layout | **Single package, all binaries bundled** (not `optionalDependencies`) | Hard 26 MiB release gate; tokenless through the one trusted `spock` package. `optionalDependencies` is deferred until worth a bootstrap token (§7). |
| D3 | Linux binary | **glibc for v0** (`ubuntu-22.04`); static-musl as the follow-up | glibc covers most hosts and isolates first-release CI variables; musl needs another target, linker toolchain, and runtime smoke. |
| D4 | Studio prebuild | **plain `pnpm build` step per job + non-empty guard** | Rebuilds the SPA in each build job (cheap) and validates the mac/Windows build. |
| D5 | Studio for crates.io | *(deferred)* `include = ["studio/dist/**"]` + check-only `build.rs` | Recorded so it isn't re-derived when crates.io is picked up. |
| D6 | npm name | **`spock` (owned)**; no scope needed at v0 (single package) | A scope (`@gridaco`) only becomes relevant if `optionalDependencies` is adopted. |
| D7 | First successful version | **`0.1.3`** | `0.1.0`–`0.1.2` were burned by the provenance race; `1.0` remains a future stability promise. |
| D8 | Changelog | **Hand-curated `CHANGELOG.md`** | Manual, but the commit style defeats auto-generators. |
| D9 | crates.io | **deferred** (bare `spock` taken; irrelevant at v0) | A future presence publishes under `spock-*`. |
| D10 | Housekeeping | **Delete the stray root `now` file; keep `studio/dist/.gitkeep`** | Zero cost; the `.gitkeep` keeps the rust-embed folder present on fresh checkouts. |
| D11 | First-run UX | **`spock new` creates; `spock init` adopts** (RFD 0022) | Closes the repo-less onboarding gap without conflating creation and adoption. |
| D12 | Framework assets | **One shared Uhura web/Wasm sidecar plus an executable-bound integrity manifest** | Avoids four copies in native binaries; serializes asset and native builds so every binary carries the exact manifest SHA-256 and rejects another executable-relative tree. |

## 10. Open questions for the maintainer

1. **When should a musl artifact be added?** v0 ships glibc (D3) and reports
   the unsupported runtime clearly; add and smoke-test musl when Alpine demand
   justifies another Linux target.
2. **macOS `universal2` (one fat binary via `lipo`) vs two thin slices?**
   Recommend two thin slices for v0; `universal2` only if a future single-file
   download UX wants it.
3. **Adopt `optionalDependencies` later?** Worth it once the measured
all-platform package size matters more than the one-time token needed to
bootstrap the platform packages (§7); until then the single package is simpler
and stays tokenless.
4. **When do the deferred channels turn on?** Recommend adding the GitHub-Release
   upload (near-free) the first time a node-less user needs a binary, and the
   `dist`-driven installers + Homebrew pass once there's traction to justify it.

## 11. Phased rollout

**P0 — `npm i -g spock` (or `npx spock`) and run it. _Done._**
- [x] Housekeeping: `git rm now`; `studio/dist/.gitkeep` kept committed.
- [x] Add the `[profile.release]` levers (§3); keep `opt-level = 3` and enforce
      the package-size budget in release CI.
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
- [x] Extend the package with one shared Uhura web/Wasm sidecar; build it once,
      record commits/protocols/hashes/sizes, bind its manifest SHA-256 into all
      four binaries, and enforce the 26 MiB packed gate.
- [x] Run the first framework release dry run: the exact guarded `0.5.0`
      tarball passed the four-target build and macOS/Linux/Windows
      installed-package verification, including framework routes and sidecar
      ([run 29379605382](https://github.com/gridaco/spock/actions/runs/29379605382)).

**Framework release follow-through.**
- [ ] Repeat the full-matrix verification against the first real framework
      publish.

**P1 — smoother first run.**
- [ ] Re-enable provenance once the tarball is smaller (optionalDependencies)
      or via a post-attestation-`E400`-tolerant publish retry (§5, §12).
- [ ] Switch `linux-x64` to static-musl (adds Alpine) + its smoke test (D3).
- [x] Refine `spock init [name]` into RFD 0022's `spock new` and adopting
      `spock init` commands with embedded canonical scaffolds.
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
  the §4 non-empty `index.html` guard runs in every build job; installed-package
  verification serves `/~studio` on macOS and Linux.
- **Missing or mismatched framework sidecar (medium unguarded → low with the
  executable binding).** A native binary alone cannot serve Uhura Editor/Play.
  The asset job validates required route literals and Wasm magic, records exact
  commits and protocols beside every hashed file, then emits the SHA-256 of the
  raw manifest. Every distribution binary captures that value and checks it
  before parsing the executable-relative manifest; the manifest inventory is
  then checked against both the filesystem and the immutable bytes that will be
  served. This detects corruption or coherent sidecar replacement while the
  executable remains trusted. It is not a package signature and cannot defend
  against replacement of both binary and sidecar or compromise of release CI.
  Explicit paired source/test overrides are intentionally outside this package
  identity boundary. `npm pack` additionally proves the exact tarball tree,
  both publish branches consume that same file, and cross-platform verification
  installs it before exercising framework routes.
- **npm publish atomicity (low).** The single-package design publishes one
  package per release, so there is no partial/half-released state — the whole
  binary set is in one tarball. The residual rule is npm's own: a version can't
  be reused, so never re-tag a version. *Mitigation:* the `dry_run` gate installs
  and exercises the exact guarded tarball on every supported OS before any real
  publish; prereleases use a `next` dist-tag.
- **Provenance is off (accepted; supply-chain nicety deferred).** With the
  original ~11 MB binary-only tarball, the provenance attestation intermittently
  races the package PUT and burns the version (§5); it cost `0.1.0`–`0.1.2`
  before we disabled it.
  Tokenless OIDC auth is unaffected; only the signed SLSA attestation is missing.
  *Re-enable path (P1):* shrink the per-publish tarball via the
  `optionalDependencies` split (§7), or add a bounded publish retry that treats
  a post-attestation `E400` as success-if-the-version-appears.
- **Linux is glibc-only at v0 (accepted).** Alpine/musl hosts aren't served
  until the musl follow-up (D3). *Mitigation:* the npm shim diagnoses the libc
  mismatch directly and points users to a GNU-libc host or a source build.
- **Cross-platform browser assets (now gated).** The build matrix exercises the
  embedded Studio build on Windows and macOS. Installed-package verification
  serves Studio on Unix and probes framework Editor, Play, Wasm, status, and
  environment routes on macOS, Linux, and Windows.
- **npm-only means Node is required (accepted).** No binary for node-less
  environments at v0. *Mitigation:* the deferred GitHub-Release upload is a
  ten-line addition the day it's needed.

**Net:** after P0, a release is *bump version → tag → push*, and one tag builds
four binaries plus one shared framework sidecar and publishes one npm package
that bundles them — verified live on macOS, Linux, and Windows. The small
hand-owned shim and sidecar verifier are the package-specific boundaries; the
pipeline stays a strict subset that the deferred channels extend without
rework.

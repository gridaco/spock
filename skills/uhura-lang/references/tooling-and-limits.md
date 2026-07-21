# Uhura 0.4 Tooling and Current Limits

## Compatibility gate

This skill describes the strict machine-first language selected by:

```toml
[project]
language = "0.4"
```

The current repository implements that language. The published
`spock@0.5.3` npm package predates it and accepts the retired v0 client model
instead.

For a public user workflow:

1. Require Node.js 18 or newer.
2. Run `spock --version` and record the installed version.
3. For an existing project, read its `uhura.toml` and run
   `spock check path/to/project`.
4. For a new project, stop on known-incompatible `spock@0.5.3`. For any later
   public release, generate a disposable project under the operating-system
   temporary directory, require exact language 0.4 in its manifest, and check
   that probe before writing the requested target.

If the installed CLI rejects language 0.4 or generates an older manifest, stop
and report the distribution mismatch. Do not remove the version, translate
source to v0, use old page/store syntax, or bypass the result with a source
checkout. Never use the requested project path as a compatibility probe. A
contributor may explicitly choose the repository toolchain, but that is a
separate workflow and must not leak into public user instructions.

When a compatible release exists, install it once and use plain commands:

```sh
npm install --global spock@VERSION
spock --version
```

Do not claim that `spock@latest` supports 0.4 without checking the installed
binary.

## Public framework commands

The npm-distributed framework CLI is the public interface:

```sh
spock new my-app
spock init path/to/existing-project
spock check path/to/project
spock dev path/to/project --port 4000
spock start path/to/project --port 4000
```

`spock check` validates the configured backend and client as one project.
`start` serves one fixed checked generation. `dev` can publish valid client
changes while reporting backend or topology changes as requiring restart.

The public CLI has no standalone Uhura `fmt`, `trace`, `project`, `editor`, or
`play` commands. Do not invent substitutes or tell a public user to build
Uhura from source.

## One-origin framework surfaces

After `spock start` or `spock dev` on port 4000:

```text
http://127.0.0.1:4000/                 Uhura Editor and checked previews
http://127.0.0.1:4000/play             live Uhura Play
http://127.0.0.1:4000/~studio          Spock Studio
http://127.0.0.1:4000/~health          readiness
http://127.0.0.1:4000/~project/status  generation and reload state
```

Editor and Play consume the same checked generation and same-origin authority
capabilities. Do not start a second backend or Uhura server for a normal
framework project.

Editor is a deterministic, read-only projection of checked evidence. It cannot
edit source or prove live provider behavior. Play runs the admitted machine,
presentation, and host adapters. Restart resets the Uhura application session,
but not necessarily provider truth.

## Diagnostics and live reload

- Fix the earliest project diagnostic first; later errors may cascade.
- A valid client save may publish a new generation in `spock dev`.
- An invalid client save retains the last good Play generation when available.
- Backend, seed, provider artifact, or framework-topology changes may require a
  process restart.
- An occupied port or final bind failure means the project is not running.
- Inspect `/~project/status` before blaming stale Play behavior on source.

## Current 0.4 bounds

- Web is the only UI profile.
- Editor is read-only; there is no Canvas-to-source round trip.
- UI declarations are pure whole-machine observation projections. Reusable UI
  invocation, slots, and component-local semantic state are not selected.
- Routing is explicit: import the router contract, declare its machine port,
  and bind that port to `web.history`. Filesystem routing is not implicit.
- Framework features are explicit imports and host bindings; there is no
  ambient meta-framework module behavior.
- There is no arbitrary JavaScript or foreign-source escape inside `.uhura`.
- The npm host serves provider JavaScript but does not compile application
  TypeScript.
- The UI element/widget catalogue is finite.
- A 0.4 host manifest admits one live entry with application-session lifetime.
- Command cancellation, timeouts, retained-session migration, and reusable
  presentation composition remain unselected or limited.
- Local actor switching is not production authentication or authorization.

Do not hide these boundaries in CSS, provider callbacks, duplicated state, or
retired syntax. Report a missing capability plainly.

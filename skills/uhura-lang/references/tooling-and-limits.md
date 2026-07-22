# Uhura 0.4 Tooling and Current Limits

## Compatibility gate

This skill describes the strict machine-first language selected by:

```toml
[project]
language = "0.4"
```

`spock@0.5.4` is the first npm package that implements that language and its
explicit `web-app@1` profile. Framework releases `0.5.0` through `0.5.3`
accept the retired v0 client model instead; releases through `0.4.0` expose no
framework client.

For a public user workflow:

1. Require Node.js 18 or newer.
2. Run `spock --version` and record the installed version.
3. For an existing project, read its `uhura.toml` and run
   `spock check path/to/project`.
4. For a new project, use known-compatible `spock@0.5.4`. Stop on `0.5.3` or
   earlier. For a later public release, generate a disposable project under
   the operating-system temporary directory, require exact language 0.4 in its
   manifest, and check that probe before writing the requested target.

If the installed CLI rejects language 0.4 or generates an older manifest, stop
and report the distribution mismatch. Do not remove the version, translate
source to v0, use old page/store syntax, or bypass the result with a source
checkout. Never use the requested project path as a compatibility probe. A
contributor may explicitly choose the repository toolchain, but that is a
separate workflow and must not leak into public user instructions.

Install the compatible release once and use plain commands:

```sh
npm install --global spock@0.5.4
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
- Machine presentations are pure whole-machine observation projections. The
  selected Web application profile also supports typed, wrapper-free pure UI
  components with checked props and emitted events. Slots, component-local
  semantic state, lifecycle, and loaders are not selected.
- Routing is explicit in ordinary projects. A project may explicitly select
  `web-app@1`, which discovers `app/**/page.uhura` and generates a route table
  checked against its configured location type. This is opt-in filesystem
  routing, not an ambient interpretation of arbitrary files.
- `web-app@1` also discovers `components/`, `surfaces/`, and colocated
  `*.examples.uhura`; the authored core/evidence module map and host bindings
  remain explicit.
- Web-app v1 path parameters are required named fields. Optional segments,
  query-derived location fields, layouts, and loaders are not selected.
- There is no arbitrary JavaScript or foreign-source escape inside `.uhura`.
- The npm host serves provider JavaScript but does not compile application
  TypeScript.
- The UI element/widget catalogue is finite.
- A 0.4 host manifest admits one live entry with application-session lifetime.
- Command cancellation, timeouts, and retained-session migration remain
  unselected or limited.
- Local actor switching is not production authentication or authorization.

Do not hide these boundaries in CSS, provider callbacks, duplicated state, or
retired syntax. Report a missing capability plainly.

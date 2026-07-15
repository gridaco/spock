---
name: uhura-lang
description: Create, inspect, validate, run, debug, and modify Uhura client experiences inside Spock framework projects using the published npm distribution. Use when translating product requirements into pages, components, surfaces, examples, typed ports, fixtures, provider-backed behavior, Canvas previews, or live Play interactions; repairing Uhura diagnostics; or verifying the boundary between disposable UI-session state and authoritative Spock behavior.
---

# Uhura Language

Treat the request as an executable experience-engineering task. Produce or
modify the client, validate the whole framework project, repair failures within
scope, and finish with concrete Editor, Play, and authority evidence.

Use only the published `spock` npm package and the files in the user's target
project. Do not require any additional implementation toolchain or examples.

## Establish the environment

1. Locate the target directory, `spock.toml`, configured client root, and its
   `uhura.toml` and `.uhura` files.
2. Require Node.js 18 or newer. Run `spock --version`; if unavailable, install
   it once with `npm install --global spock@latest`, then use plain `spock`
   commands and record the installed version.
3. Confirm that the target is a framework project with a configured client.
   Use `spock new` to create a new backend-and-client project or `spock init`
   to adopt an existing unambiguous project.
4. Preserve unrelated changes. Read the affected manifest, definitions,
   examples, ports, fixtures, styles, and provider seams before editing.

## Select the source of truth

- Treat the installed npm CLI as the executable contract.
- Use the bundled references for implemented syntax, project contracts,
  workflows, and limits; never substitute proposed syntax from memory.
- Run `spock check` instead of guessing whether a construct is accepted.
- If a reference and the installed version disagree, preserve the installed
  version's behavior and report the version difference.

Read references only as needed:

- Read [references/workflows.md](references/workflows.md) before creating,
  adopting, modifying, or repairing a client.
- Read [references/source-language.md](references/source-language.md) before
  authoring or substantially changing `.uhura` source.
- Read [references/project-and-providers.md](references/project-and-providers.md)
  for manifests, ports, examples, fixtures, providers, and ownership.
- Read [references/tooling-and-limits.md](references/tooling-and-limits.md) for
  npm tooling, Editor, Canvas, Play, live reload, and current limitations.

## Start with ownership

- Put durable records, authorization, accepted mutations, files, shared
  workflow, and cross-device truth in Spock or another authority.
- Put selected tabs, drafts, pending flags, optimistic overlays, notices,
  logical navigation, and mounted surfaces in Uhura.
- Leave pixels, layout measurement, pointer mechanics, native media state,
  clocks, network, files, and device I/O to renderers or declared provider
  seams.
- Never make the same fact authoritative in both Uhura and its provider.

## Follow the execution loop

1. Convert the request into observable states and event paths: loading, ready,
   empty, failed, pending, accepted, refused, retry, navigation, and surfaces.
2. Run a healthy `spock check` baseline before changing an existing project.
3. Define or update typed projections and commands before consuming them in
   the client. Keep authoritative operations in Spock.
4. Choose the correct source kind: page for a route, component for reusable
   presentation, or surface for a mounted sheet, dialog, or popover.
5. Add only reconstructible UI-session state. Make guards, optimism,
   settlement, rollback, dismissal, and navigation explicit.
6. Add pinned examples for meaningful static states and derived examples for
   reachable interaction states.
7. Run `spock check` after each coherent change. Fix the earliest diagnostic
   first and repeat until the whole project checks.
8. Use `spock dev` while iterating on client source or `spock start` for one
   fixed checked generation. Verify Editor, Play, Studio, actor selection, and
   the affected provider boundary on the same origin.
9. Stop processes started for verification and report changed files, exact
   commands, outcomes, provider/actor evidence, and material limitations.

## Preserve deterministic semantics

- Keep component behavior as typed emits over props. Put state machines in
  pages and surfaces, not renderer callbacks.
- Use only implemented store statements: `set`, `send`, `open-surface`,
  `dismiss`, and navigation variants.
- Dispatch one external event per core step. Do not invent timers, randomness,
  ambient I/O, hidden queues, or host-language escapes.
- Guard duplicate commands. Keep pending and optimistic state explicit and
  clear or roll it back on settlement.
- Handle command success and refusal/unavailable paths and projection
  availability where required.
- Mount surfaces with `open-surface` and close them with `dismiss`; preserve
  ownership, modality, and focus restoration.
- Use catalog semantics and authored CSS. Do not attach arbitrary DOM events
  or hide product truth in styling.

## Validate proportionally

Every created or modified client requires at least:

```sh
spock --version
spock check path/to/project
```

For behavior work, serve the project on an available port and verify `/`,
`/play`, `/~studio`, `/~project/status`, and the exact affected read or command.
Canvas proves checked preview projection; Play plus the authority proves live
behavior. Never treat a screenshot alone as behavioral proof.

The npm CLI does not expose standalone Uhura format, trace, project, or editor
subcommands. State that tooling boundary and use project check, Editor
previews, Play, and authority probes for the evidence the distributed product
supports.

---
name: uhura-lang
description: Create, inspect, validate, run, debug, and modify strict Uhura 0.4 machine-first programs and Web UI applications inside Spock framework projects. Use when translating requirements into machines, explicit UI projections, deterministic evidence, host bindings, or provider-backed behavior; repairing Uhura diagnostics; or verifying Editor, Play, and authority behavior with the npm-distributed Spock CLI.
---

# Uhura Language

Treat Uhura as a deterministic machine language first. Keep its Web UI profile,
evidence modules, and live host admission as explicit layers over the checked
machine. Produce or modify the program, validate the complete framework
project, repair failures within scope, and finish with concrete evidence.

Use the public npm workflow for user projects. Do not require a Rust checkout,
workspace build, or unpublished binary unless the user explicitly changes the
task to contributor development.

## Pass the compatibility gate first

1. Locate `spock.toml`, the configured client root, `uhura.toml`, `host.toml`
   when present, and every mapped `.uhura` source file.
2. Require Node.js 18 or newer. Run `spock --version`; if unavailable, install
   the requested public release once and then use plain `spock` commands.
3. For an existing project, read `[project].language` before editing and run
   `spock check`. This skill documents strict language `"0.4"` only.
4. For a new project, prove compatibility before writing the requested target.
   Stop immediately on published `spock@0.5.3`. For a later public release,
   generate and check a disposable probe under the operating-system temporary
   directory; require its manifest to select exact language `"0.4"`.

The current repository uses Uhura 0.4, while the published `spock@0.5.3`
release predates that language. If the installed CLI is that release, generates
an older manifest in the disposable probe, or rejects
`[project] language = "0.4"`, stop and report a distribution mismatch. Never
use the requested target as the probe, rewrite the project in retired v0
syntax, remove the language gate, or silently fall back to the installed
legacy grammar.

Read references only as needed:

- Read [references/workflows.md](references/workflows.md) before creating,
  adopting, modifying, or repairing a client.
- Read [references/source-language.md](references/source-language.md) before
  authoring or substantially changing `.uhura` source.
- Read [references/project-and-providers.md](references/project-and-providers.md)
  for manifests, module maps, evidence, host entries, providers, and ownership.
- Read [references/tooling-and-limits.md](references/tooling-and-limits.md) for
  the compatibility gate, npm tooling, Editor, Play, and current limitations.

## Start with ownership

- Put durable records, authorization, accepted mutations, files, shared
  workflow, and cross-device truth in Spock or another authority.
- Put deterministic state, events, outcomes, guards, drafts, pending flags,
  optimistic overlays, notices, and logical navigation in an Uhura machine.
- Keep a `pub ui` declaration a pure projection of one machine observation.
- Leave pixels, layout measurement, pointer mechanics, native media state,
  clocks, network, files, and device I/O to renderers or declared host adapters.
- Never make the same fact authoritative in both Uhura and its provider.

## Preserve the layer boundaries

- `uhura.toml` selects language 0.4 and maps logical core and evidence modules.
- Core `.uhura` modules define types, values, parts, ports, and `pub machine`
  declarations.
- A UI module must explicitly `use uhura::ui;` before defining a `pub ui`
  projection for a machine.
- Evidence modules define scenarios, checkpoints, and named examples over the
  same checked machine; they do not define another runtime.
- `host.toml` selects one live machine, optional presentation, lifetime,
  stylesheet, and exact adapter bindings after source checking.
- A provider module implements only ports assigned to its adapter identity.
  It does not own machine state or UI semantics.

Do not recreate retired path-based pages, component stores, surface stores,
TOML port contracts, fixture scripts, or catalog manifests.

## Follow the execution loop

1. Convert the request into state, input, outcome, command, observation, and
   invariant contracts.
2. Run a healthy `spock check` baseline before changing an existing project.
3. Change the machine contract before its UI projection or host adapters.
4. Make loading, ready, empty, failed, pending, settlement, rollback, and
   navigation states explicit where the product requires them.
5. Bind semantic UI events directly to checked machine inputs. Do not embed
   JavaScript callbacks or mutate state from markup.
6. Add evidence that reaches and pins meaningful states through checked
   inputs, deliveries, outcomes, and emitted commands.
7. Run `spock check` after each coherent change. Fix the earliest diagnostic
   first and repeat until the whole project checks.
8. Use `spock dev` while iterating or `spock start` for one fixed checked
   generation. Verify Editor, Play, Studio, actor selection, and the affected
   adapter boundary on the same origin.
9. Stop processes started for verification and report changed files, exact
   commands, outcomes, provider evidence, and material limitations.

## Validate proportionally

Every created or modified client requires at least:

```sh
spock --version
spock check path/to/project
```

For behavior work, serve the project on an available port and verify `/`,
`/play`, `/~studio`, `/~project/status`, and the exact affected read or command.
Editor proves checked evidence projection; Play plus the authority proves live
behavior. Never treat a screenshot alone as behavioral proof.

The npm CLI does not expose standalone Uhura format, trace, project, or editor
subcommands. State that tooling boundary and use project check, Editor
previews, Play, and authority probes for the evidence the distributed product
supports.

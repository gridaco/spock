---
name: spock-lang
description: Create, inspect, validate, run, debug, and modify Spock v0 backend and framework projects using the published npm distribution. Use when turning product requirements into a Spock program; scaffolding or adopting a framework project; changing tables, records, functions, seed data, actor-aware operations, storage, or refusals; generating contracts, TypeScript, or GraphQL; or verifying Spock-backed Uhura behavior.
---

# Spock Language

Treat the user's request as an executable engineering task. Produce or modify
the program, run the relevant Spock commands, fix failures within scope, and
finish with concrete verification evidence.

Use only the published `spock` npm package. Do not assume the Spock source
checkout, a Rust toolchain, or source-built web assets are available.

## Establish the environment

1. Locate the target directory, `spock.toml`, and existing `.spock` files.
2. Require Node.js 18 or newer. Run `spock --version`; if the command is
   unavailable, install it once with `npm install --global spock@latest`, then
   record the installed version. Use plain `spock` commands afterward.
3. Classify the target as a framework project or a standalone `.spock`
   program. Framework commands operate on a project; `build`, `run`, and `gen`
   operate on an explicit standalone file.
4. Preserve unrelated user changes and never replace an existing model before
   understanding its tables, functions, constraints, seeds, and consumers.

## Select the source of truth

- Treat the globally installed npm CLI as the executable contract.
- Use the bundled references for concise guidance. Consult the
  [Spock v0.5.0 specification](https://github.com/gridaco/spock/blob/v0.5.0/docs/spec/v0.md)
  when exact syntax or semantics matter.
- Use the
  [filter lab schema](https://github.com/gridaco/spock/blob/v0.5.0/examples/filter-lab/schema.spock),
  [Instagram backend](https://github.com/gridaco/uhura/blob/77bee48bae90b0246351dc6ad27b27f34bbc0a65/examples/instagram/backend/app.spock),
  and
  [standalone Instagram program](https://github.com/gridaco/spock/blob/v0.5.0/examples/instagram/v0.spock)
  as accepted-language examples.
- Do not copy syntax from the
  [Instagram paper program](https://github.com/gridaco/spock/blob/v0.5.0/examples/instagram/v1.spock),
  [vision RFD](https://github.com/gridaco/spock/blob/v0.5.0/docs/rfd/0000-vision.spock),
  or other proposals. They do not override the published compiler.
- Run the installed toolchain instead of guessing whether a construct is
  accepted. If a bundled reference and the installed CLI disagree, preserve
  the installed version's behavior and report the version difference.

Read the bundled references only as needed:

- Read [references/workflows.md](references/workflows.md) before creating,
  adopting, modifying, or repairing a project.
- Read [references/language.md](references/language.md) before authoring or
  substantially changing a `.spock` program.
- Read [references/tooling.md](references/tooling.md) for global CLI
  installation, framework commands, generated artifacts, HTTP checks, Studio,
  storage, or Uhura integration.
- Read [references/limits.md](references/limits.md) before making security,
  production-readiness, persistence, policy, or unsupported-feature claims.

## Follow the execution loop

1. Convert the request into observable acceptance scenarios before editing:
   required data, reads, writes, refusals, actors, storage, and generated
   consumers.
2. Use `spock new` for a new framework project, `spock init` to adopt an
   existing directory, or create one `.spock` file for a standalone program.
   For an existing target, run a baseline check before changing it whenever it
   is expected to load.
3. Make the smallest coherent change. Keep durable source rows authoritative
   and derive display counts or projections in consumers instead of seeding
   decorative totals.
4. Run `spock check` after each meaningful change. Fix the earliest diagnostic
   first and repeat until the complete target checks successfully.
5. Run `spock build` or `spock gen` when a standalone contract or generated
   consumer changes.
6. Run the target and probe the exact affected behavior. Cover success and
   refusal paths, anonymous and actor-bound paths, or storage paths when
   relevant.
7. For framework projects with Uhura, verify the integrated Editor, Play,
   Studio, and the live provider boundary rather than treating fixture output
   as backend proof.
8. Stop processes started for verification and leave no temporary output in
   the user's project unless requested.
9. Report changed files, exact commands, outcomes, unchecked escape count,
   runtime evidence, and material v0 limitations.

## Preserve the language contract

- Prefer declarative constraints over repeating checks in every function:
  keys, references, `unique`, closed sets, and validator functions belong on
  the data.
- Use references for relationships. A reference stores the target table key
  and cannot target a composite-key table in v0.
- Use `auth table` only for the single identity anchor. Treat
  `X-Spock-Actor` as a forgeable development seam, never authentication.
- Use `= me` only on a reference to the auth table. Seed rows must still
  provide required actor-backed fields because seed replay has no actor.
- Use `storage_object` references for files. Move bytes through `/storage/v1`;
  never inline bytes in Spock values or function payloads.
- Keep each `unchecked sql(...)` escape to one statement. Use `:param`
  placeholders only. The final statement must return columns matching the
  declared return contract.
- Mint product refusals in a function `!` clause and raise them with
  `spock_refuse`. Do not fake derived or reserved errors.
- Do not introduce `view`, `role`, `policy`, state-machine syntax, modules,
  native function statements, or other reserved or proposed syntax.

## Validate proportionally

Every created or modified target requires at least a check through the globally
installed npm-distributed CLI:

```sh
spock --version
spock check path/to/project-or-app.spock
```

For runtime work, start the project or standalone program on an available port,
wait for `/~health`, probe the changed read or function path, and stop the
process cleanly. Never treat a rendered Studio screen alone as proof of backend
behavior.

## Handle diagnostics

- Fix the earliest diagnostic first; later errors may be cascades.
- Preserve stable `L...` and `E...` codes in reports.
- If `check` reports unchecked escapes, explain that contracts and loadability
  were checked but SQL business logic remains author-asserted.
- If behavior appears version-dependent, report `spock --version`. Upgrade the
  global package only when the user requests the current release or the task
  requires it.

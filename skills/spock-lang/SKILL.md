---
name: spock-lang
description: Build, inspect, validate, run, debug, and modify current Spock v0 backend prototype projects (`*.spock`). Use when turning product requirements into a new Spock program; changing tables, records, functions, seed data, actor-aware operations, storage, or refusals; investigating compiler or runtime behavior; generating contracts, TypeScript, or GraphQL; or verifying Spock-backed Uhura integration.
---

# Spock Language

Treat the user's request as an executable engineering task. Produce or modify the program, run the relevant Spock commands, fix failures within scope, and finish with concrete verification evidence.

Work against the implemented Spock v0 surface, not proposed syntax. Never invent a `spock init` command, multi-file module system, or future language construct. A new v0 project may be a directory containing a single `app.spock` program.

## Establish the environment

1. Locate the target directory and any existing `.spock` files.
2. If the target is the Spock source checkout, use its locked Cargo workspace, normative specification, and accepted examples.
3. If the target is a consumer project, do not assume the Spock source tree or Rust toolchain is present. Use the bundled references and the project's installed, locked, or explicitly requested Spock CLI version.
4. Preserve unrelated user changes and never replace an existing model before understanding its tables, functions, constraints, seeds, and consumers.

## Select the source of truth

- In the Spock source checkout, read `docs/spec/v0.md` when exact syntax or semantics matter and inspect `crates/spock-lang` when diagnosing the implementation.
- In that checkout, use `examples/filter-lab/schema.spock`, `examples/instagram-poc/app.spock`, and `examples/instagram/v0.spock` as accepted-language examples.
- Do not copy syntax from `examples/instagram/v1.spock`, `docs/rfd/0000-vision.spock`, or other paper programs. They explain design direction but do not override the current compiler.
- In a consumer project, treat the selected CLI plus the bundled references as the usable contract. Record the CLI version when behavior differs from the references.
- Run the toolchain instead of guessing whether a construct is accepted.

Read the bundled references only as needed:

- Read [references/workflows.md](references/workflows.md) before creating a new project, modifying an existing project, or repairing a failure.
- Read [references/language.md](references/language.md) before authoring or substantially changing a `.spock` program.
- Read [references/tooling.md](references/tooling.md) for CLI, generated artifacts, HTTP checks, Studio, storage, or Uhura integration.
- Read [references/limits.md](references/limits.md) before making security, production-readiness, persistence, policy, or unsupported-feature claims.

## Follow the execution loop

1. Convert the request into observable acceptance scenarios before editing: required data, reads, writes, refusals, actors, storage, and generated consumers.
2. For a new program, choose keys and relationships before functions, create `app.spock`, then add representative seed data. For an existing program, run a baseline check before changing it whenever the current file is expected to load.
3. Make the smallest coherent change. Keep durable source rows authoritative and derive display counts or projections in consumers instead of seeding decorative totals.
4. Run `spock check` after each meaningful change. Fix the earliest diagnostic first and repeat until the whole program materializes and seed replay succeeds.
5. Run `spock build` or `spock gen` when the contract or a generated consumer changes.
6. Run the server and probe the exact affected behavior. Cover success and refusal paths, anonymous and actor-bound paths, or storage paths when relevant.
7. For Spock-backed Uhura work, verify the live provider boundary rather than treating fixture-backed Canvas output as backend proof.
8. Stop processes started for verification and leave no temporary output in the user's project unless requested.
9. Report changed files, exact commands, outcomes, unchecked escape count, runtime evidence, and material v0 limitations.

## Preserve the language contract

- Prefer declarative constraints over repeating checks in every function: keys, references, `unique`, closed sets, and validator functions belong on the data.
- Use references for relationships. A reference stores the target table key and cannot target a composite-key table in v0.
- Use `auth table` only for the single identity anchor. Treat `X-Spock-Actor` as a forgeable development seam, never authentication.
- Use `= me` only on a reference to the auth table. Seed rows must still provide required actor-backed fields because seed replay has no actor.
- Use `storage_object` references for files. Move bytes through `/storage/v1`; never inline bytes in Spock values or function payloads.
- Keep each `unchecked sql(...)` escape to one statement. Use `:param` placeholders only. The final statement must return columns matching the declared return contract.
- Mint product refusals in a function `!` clause and raise them with `spock_refuse`. Do not fake derived or reserved errors.
- Do not introduce `view`, `role`, `policy`, state-machine syntax, modules, native function statements, or other reserved/future syntax.

## Validate proportionally

Every created or modified accepted-language program requires at least:

```sh
spock check path/to/app.spock
```

For Spock repository-source work, prefer the current checkout over an older globally installed or npm-cached binary:

```sh
cargo run --locked -p spock-cli -- check path/to/app.spock
```

For consumer work, respect the project's locked or explicitly requested version. If none exists, use the current published CLI and record the reported version before interpreting a mismatch.

For runtime work, start the program on an available port, wait for `/~health`, probe the changed read or function path, and stop the process cleanly. Never treat a rendered Studio screen alone as proof of backend behavior.

## Handle diagnostics

- Fix the earliest diagnostic first; later errors may be cascades.
- Preserve stable `L...` and `E...` codes in reports.
- If `check` reports unchecked escapes, explain that the contracts and loadability were checked but SQL business logic remains author-asserted.
- If current source and an npm binary disagree, reproduce with the repository-local CLI before changing the program.

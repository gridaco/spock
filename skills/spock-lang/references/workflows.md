# Spock User Workflows

Use these workflows to turn a product request into a verified Spock v0 project
or to modify an existing project safely with the npm-distributed CLI.

## Contents

- [Define the task contract](#define-the-task-contract)
- [Choose the project shape](#choose-the-project-shape)
- [Create or adopt a project](#create-or-adopt-a-project)
- [Modify an existing project](#modify-an-existing-project)
- [Diagnose and repair a failure](#diagnose-and-repair-a-failure)
- [Choose verification scenarios](#choose-verification-scenarios)
- [Report completion](#report-completion)

## Define the task contract

Extract observable requirements before modeling:

1. Durable entities and their identities.
2. Relationships and delete behavior.
3. The one identity-bearing entity, if actor behavior exists.
4. Stored facts versus derived presentation values.
5. Required reads and their cardinality: one, optional, or list.
6. Required writes, retry behavior, and authority echoes.
7. Product refusals versus constraint-derived errors.
8. Representative seed scenarios.
9. File or media fields that require storage.
10. Generated contract, TypeScript, GraphQL, or Uhura consumers.

Turn each requirement into a scenario with a request and an observable result.
Do not treat a schema that merely compiles as proof that a product rule works.

## Choose the project shape

- Treat a directory with `spock.toml` as a framework project. Check and serve
  the project as a unit.
- Use `spock new NAME` when the user wants a new framework project. Add
  `--backend-only` when no Uhura client is wanted.
- Use `spock init [PATH]` to adopt existing unambiguous sources without moving
  or overwriting them.
- Use one explicit `.spock` file when the user asks for only a standalone
  authority program or a language-level artifact.

## Create or adopt a project

1. Confirm the target directory and avoid overwriting existing files.
2. Install the global npm CLI as described in [tooling.md](tooling.md) and
   record `spock --version`.
3. Run `spock new` for a new framework project, `spock init` to adopt an
   existing directory, or create `app.spock` for a standalone program.
4. Read the generated or adopted topology before editing it.
5. Define tables from durable facts:
   - choose stable keys;
   - use references for relationships;
   - use composite keys for relationship identity when appropriate;
   - add delete actions deliberately;
   - encode closed choices and reusable validators as constraints.
6. Add an `auth table` only when the program needs an actor. Use at most one.
7. Define records only for scalar function return projections.
8. Define reads with `fn` and writes with `mut fn`. Declare return cardinality
   and product refusals explicitly. Never raise derived or reserved codes such
   as `not_found` with `spock_refuse`; choose a product-specific code or let the
   runtime produce its derived error through the corresponding mechanism.
9. Add seed rows that cover important relationships and function scenarios.
   Provide actor-backed required fields explicitly because seed replay is
   actorless.
10. Run `spock check` and fix diagnostics until the whole target succeeds.
11. Generate requested standalone artifacts, then start `dev`, `start`, or
    `run` and execute the relevant scenario matrix below.

Do not add migrations, speculative language syntax, or multiple Spock source
modules. Preserve the topology generated or adopted by the distributed CLI.

## Modify an existing project

Preserve the existing model and prove both the baseline and the requested
delta.

1. Find `spock.toml`, `.spock` files, package-manager metadata, and commands
   used by the project.
2. Read the complete target program before editing. Inventory:
   - tables, keys, references, and delete actions;
   - records and return projections;
   - read and mutation functions;
   - refusals and constraint-derived errors;
   - seed bindings and their order;
   - storage references;
   - generated artifacts and Uhura integration.
3. Run the existing check command before editing when the target is expected
   to be healthy. Record pre-existing failures instead of attributing them to
   the requested change.
4. Identify the smallest affected surface and its consumers.
5. Apply the requested change without reformatting unrelated declarations or
   replacing accepted syntax with proposed syntax.
6. Run `check` immediately. Fix the earliest new diagnostic first.
7. Regenerate artifacts only when their source contract changes. Inspect the
   resulting diff.
8. Run focused runtime scenarios for the changed behavior and at least one
   nearby regression path.
9. Preserve unrelated working-tree changes and remove temporary runtime data.

For a schema change, review seed data, function SQL result shapes, REST and
GraphQL exposure, generated types, and Uhura expectations. A table edit is
rarely isolated to the table declaration.

## Diagnose and repair a failure

1. Reproduce the failure with the globally installed `spock` command.
2. Record `spock --version`. Upgrade the global package only when the user
   requests the current release or the task requires it.
3. Fix the earliest compiler diagnostic first; parse failures commonly cause
   cascades.
4. Use stable diagnostic codes and source spans when reporting the cause.
5. For `check` failures, distinguish:
   - lexing or parsing;
   - name and type checking;
   - schema materialization;
   - SQL preparation or result-shape validation;
   - constraint or seed replay failure.
6. For runtime failures, capture the route, method, actor header, request body,
   status, and response body.
7. Make the narrowest repair, rerun the original reproduction, then run a
   nearby success or regression scenario.
8. Do not silence a product rule by deleting a constraint or refusal unless
   the user requested that semantic change.

## Choose verification scenarios

Use every row that applies to the task.

| Changed surface | Required proof |
| --- | --- |
| Framework topology or manifest | Project-level `spock check`; verify the selected backend and client roots |
| Tables, fields, references, constraints, or seeds | `spock check`; confirm full schema materialization and seed replay |
| Read function | Call the read route; verify cardinality, returned fields, ordering, filtering, and empty behavior |
| Mutation function | Call a success case and every affected declared refusal; verify the returned authority echo |
| Retryable mutation | Repeat the same request and verify the intended idempotent or refusal behavior |
| Actor-aware behavior | Exercise anonymous and `X-Spock-Actor` requests with at least two relevant actors |
| GraphQL surface | Generate or inspect the schema and execute the affected query or mutation |
| Generated TypeScript | Regenerate the file and inspect the public type diff |
| Storage reference | Sign upload, upload bytes, attach the object, sign download, and verify bytes and content type |
| Delete action | Delete or simulate the parent operation and verify `restrict`, `cascade`, or `set null` behavior |
| Spock-backed Uhura | Verify Editor, live Play, Studio, actor switching, one affected read, and one affected command |

`spock check` proves that the program loads, SQL statements satisfy the checked
envelope, and seeds replay. It does not prove that unchecked SQL expresses the
intended business meaning. Runtime scenarios supply that missing evidence.

## Report completion

Provide a compact handoff containing:

1. Files created or modified.
2. Product behavior implemented or repaired.
3. Installed npm CLI version and exact validation or runtime commands.
4. Outcomes, including table, record, function, seed-row, and unchecked-escape
   counts reported by the CLI when available.
5. Runtime request and response evidence for affected paths.
6. Generated artifacts changed or intentionally left unchanged.
7. Relevant v0 limitations, security boundaries, or remaining failures.

Never report completion while required checks are failing, a started runtime is
still unmanaged, or the result depends on proposed syntax.

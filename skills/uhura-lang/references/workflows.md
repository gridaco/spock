# Uhura Project Workflows

Use these workflows with the npm-distributed `spock` command. Treat the whole
framework project as the executable unit.

## Define the experience contract

Before editing, identify:

1. Pages, routes, and route parameters.
2. Components and their typed props and emits.
3. Sheets, dialogs, popovers, and surface ownership.
4. Provider projections needed to render each state.
5. Commands and refusals needed to change authoritative truth.
6. Reconstructible UI-session state, guards, optimism, and rollback.
7. Loading, ready, empty, failed, pending, accepted, refused, and retry states.
8. Navigation and focus-restoration behavior.
9. Pinned and derived examples needed for Canvas coverage.
10. Live Play scenarios needed to prove the provider boundary.

Spock owns durable truth and accepted operations. Uhura owns disposable
experience state. Renderers own pixels and device mechanics.

## Create a project

1. Run `spock --version` and record the installed version.
2. Run `spock new NAME`; do not use `--backend-only` when an Uhura client is
   required.
3. Read `spock.toml`, the generated backend, and the configured client before
   replacing placeholders.
4. Define provider-owned facts and operations in the backend.
5. Define typed ports before client source reads or sends provider data.
6. Add pages, components, and surfaces with semantic events and authored CSS.
7. Add fixture-backed examples for static and reachable interaction states.
8. Run `spock check` and repair the earliest diagnostic until the complete
   project succeeds.
9. Run `spock dev`, inspect Canvas, then exercise the same paths in Play with
   the intended actor and authority.

Do not copy speculative syntax or claim completion from one happy-state frame.

## Modify an existing project

1. Locate the nearest `spock.toml` and identify its backend and client roots.
2. Read the affected manifest, source definition, examples, imported
   components/surfaces, ports, fixtures, styles, and provider adapter.
3. Run a baseline `spock check` when the project should be healthy.
4. Make the smallest ownership-correct change. Do not move authority into a
   client store to avoid a provider change.
5. Update semantic events and handlers before appearance when behavior changes.
6. Update affected examples so new meaningful states remain visible in Canvas.
7. Run `spock check` after each coherent edit.
8. Run `spock dev`; inspect changed previews and `/~project/status`.
9. Verify the live Play path, actor, provider consequence, and a neighboring
   regression path.
10. Stop the server and preserve unrelated working-tree changes.

When a port changes, review its client imports, fixtures, provider mapping,
backend contract, and every affected example. A port change is not isolated.

## Diagnose and repair a failure

1. Reproduce with the globally installed `spock` command and record its
   version.
2. Separate project diagnostics from environment failures such as occupied
   ports, a missing generated provider artifact, or stale browser state.
3. Fix the earliest parse, resolution, type, catalog, contract, example, or
   provider diagnostic first.
4. For Canvas failures, distinguish checking, example replay, asset resolution,
   model publication, and browser rendering.
5. For Play failures, inspect the final server output, `/~project/status`, and
   browser console/network evidence before changing source.
6. For provider failures, capture actor, route, request, response/refusal, and
   the resulting authoritative projection.
7. Make the narrowest repair, rerun the original reproduction, and verify one
   adjacent success or refusal path.

Do not bypass a distributed-product failure with an unpublished toolchain.
Report a missing public capability when the installed CLI cannot produce the
desired evidence.

## Prove Canvas projection

1. Run `spock check` first.
2. Start `spock dev` or `spock start` and open `/`.
3. Inspect affected preview groups, examples, notes, data origins, and declared
   interactions.
4. Confirm derived examples retain correct direct-parent replay provenance.
5. Inspect the provenance, annotations, and surface information actually
   exposed by the installed Editor; do not claim newer Canvas affordances
   without observing them.

Canvas must remain read-only and must not be treated as proof of provider
mutation or authorization.

## Prove Play and provider behavior

1. Use the same running framework process and open `/play`.
2. Select the intended actor and restart the Uhura session before the focused
   scenario.
3. Exercise the exact semantic event path.
4. Verify pending or optimistic state before settlement where applicable.
5. Verify accepted behavior and every affected refusal/unavailable path.
6. Inspect Studio or the affected endpoint to prove durable truth changed only
   on the authority side.
7. Restart Play and distinguish session reset from provider persistence.

## Choose verification scenarios

| Changed surface | Required proof |
| --- | --- |
| Page, component, surface, or CSS | Project check, affected Canvas examples, visual inspection |
| Handler, guard, or local state | Checked derived examples, success path, refusal/failure path |
| Optimistic command | Pending state, one command, accepted settlement, rollback |
| Projection availability | Loading, ready, failed, and empty states as applicable |
| Navigation | Push/replace/back behavior and retained/initialized state |
| Surface | Open, ownership, stack, dismissal, and focus restoration |
| Port or provider | Project check, client imports, live provider compatibility, authority consequence |
| Canvas tooling | Preview metadata, connectors/annotations, selection and visibility behavior |
| Spock-backed experience | Editor, Play, Studio, actor, affected read, affected command |

## Report completion

Report changed files, installed Spock version, exact commands, project-check
result, affected previews, Play actor/provider, success and refusal evidence,
authority consequences, and current tooling/runtime limits. Do not report
completion while checks fail, a server is unmanaged, or Canvas is the only
proof of live behavior.

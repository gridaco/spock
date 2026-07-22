# Uhura 0.4 Project Workflows

Use these workflows with a compatible npm-distributed `spock` command. Treat
the complete framework project as the executable unit and preserve the
machine/UI/evidence/host boundaries.

## Define the program contract

Before editing, identify:

1. Machine configuration and construction requirements.
2. Typed state and invariants.
3. External events and qualified port deliveries.
4. Commit-policy and abort-policy outcomes.
5. Ordered commands and required host ports.
6. Public observation needed by presentations and tools.
7. Loading, ready, empty, failed, pending, accepted, refused, retry, and
   navigation states.
8. Pure UI projections and semantic element-to-input bindings.
9. Scenario checkpoints and examples needed for Editor.
10. Live host adapters and Play paths needed to prove external behavior.

Spock owns durable truth. The Uhura machine owns deterministic session
behavior. UI projects observation. Evidence proves reachable states. The host
binds external capabilities. Renderers own pixels and device mechanics.

## Create a project

1. Run `spock --version` and record the installed version.
2. Require known-compatible `spock@0.5.4`; stop before any target write on
   `0.5.3` or earlier.
3. For a later public release, create a disposable probe beneath the
   operating-system temporary directory. Require its generated
   `client/uhura.toml` to select exact language `"0.4"` and require
   `spock check` to pass. If either condition fails, report the distribution
   mismatch and do not continue with v0.
4. Only after the probe passes, run `spock new NAME`; do not request a
   backend-only project when a client is required. Enter `NAME`.
5. Reconfirm `client/uhura.toml`, then run `spock check` before replacing
   generated source.
6. Read `spock.toml`, the backend, module map, generated machine, UI, evidence,
   host entry, and any selected framework profile and discovered UI tree.
7. Define the machine contract before consuming its observation or inputs in
   UI.
8. Add deterministic scenarios and examples for meaningful states.
9. Bind every live port explicitly in `host.toml`; build any custom provider
   artifact separately.
10. Run `spock check`, then `spock dev`; inspect Editor, Play, Studio, and the
    intended provider consequence.

Do not infer a core or evidence logical module from an arbitrary filename or
add an unmapped `.uhura` file. The only filename-derived UI modules are the
page/component/surface conventions admitted by an explicit `web-app@1`
profile.

## Modify an existing project

1. Locate the nearest `spock.toml` and identify its backend and client roots.
2. Run the compatibility gate. Stop if the installed CLI cannot check 0.4.
3. Preserve unrelated changes and run a healthy `spock check` baseline.
4. Read the affected logical modules, imports, evidence, `host.toml`, style,
   provider artifact, authority contract, and any selected framework profile,
   generated route contract, or discovered UI source involved in the change.
5. Make the smallest ownership-correct change:
   - machine behavior first;
   - observation needed by UI second;
   - pure UI projection third;
   - evidence and host bindings last.
6. Update every affected scenario expectation and named example.
7. Run `spock check` after each coherent edit.
8. Run `spock dev`; inspect changed examples and `/~project/status`.
9. Exercise the live Play path, intended actor, provider consequence, and one
   neighboring regression or refusal path.
10. Stop processes started for verification.

A port change crosses machine source, evidence bindings, `host.toml`, provider
adapter, and authority behavior. Review all five deliberately.

## Diagnose and repair a failure

1. Reproduce with the globally installed `spock` command and record its
   version.
2. Separate a language-distribution mismatch from a source diagnostic. Never
   “repair” 0.4 by converting it to v0.
3. Separate project diagnostics from environment failures such as an occupied
   port, missing provider JavaScript, or stale browser state.
4. Fix the earliest parse, resolution, type, machine, UI, evidence, host, or
   provider diagnostic first.
5. For Editor failures, distinguish checking, scenario replay, asset
   resolution, model publication, and browser rendering.
6. For Play failures, inspect final server output, `/~project/status`, and
   browser console/network evidence before changing source.
7. For provider failures, capture actor, port, command/delivery, settlement,
   and authoritative result.
8. Make the narrowest repair, rerun the original reproduction, and verify one
   adjacent path.

Report a missing public capability instead of bypassing it with an unpublished
toolchain.

## Prove deterministic evidence

1. Run `spock check`.
2. Confirm every machine with required ports binds deterministic evidence
   adapters before `start`.
3. Confirm each scenario's ordered `send`, `deliver`, `expect`, and `pin`
   sequence.
4. Confirm named examples select the intended `pub ui` and checkpoint.
5. Start `spock dev` or `spock start`, open `/`, and inspect affected examples,
   notes, and reachable state.

Editor remains read-only and does not prove provider mutation or authorization.

## Prove Play and provider behavior

1. Use the same running framework process and open `/play`.
2. Select the intended actor and restart the Uhura session before the focused
   scenario.
3. Exercise the exact semantic UI event path.
4. Verify pending or optimistic state before settlement where applicable.
5. Verify accepted behavior and each material refusal or unavailable path.
6. Inspect Studio or the affected endpoint to prove durable truth changed only
   on the authority side.
7. Restart Play and distinguish machine-session reset from provider
   persistence.

## Choose verification scenarios

| Changed layer | Required proof |
| --- | --- |
| Type, state, event, outcome, or handler | Project check and focused scenario outcome |
| Command or port | Ordered evidence command, host binding, live adapter consequence |
| Observation or UI | Project check, affected Editor examples, visual inspection |
| Semantic event binding | Checked input type and live Play dispatch |
| Optimistic mutation | Pending state, one request, accepted settlement, rollback |
| External observation | Loading, ready, failed, and empty states as applicable |
| Routing | Evidence delivery/command plus live `web.history` behavior |
| Evidence only | Replay, checkpoint, example projection, and note |
| Provider | Exact port coverage, deferred delivery, settlement, authority result |
| Framework experience | Editor, Play, Studio, actor, affected read, affected command |

## Report completion

Report changed files, installed Spock version, compatibility result, exact
commands, project-check result, affected examples, Play actor/provider,
accepted and refused evidence, authority consequences, and current limits. Do
not report completion while checks fail, a server is unmanaged, or Editor is
the only proof of live behavior.

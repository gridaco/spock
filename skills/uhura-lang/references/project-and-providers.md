# Uhura 0.4 Projects, Evidence, and Providers

Keep the machine, presentation, evidence, and deployment boundaries visible.
They share checked types and one runtime kernel, but they are not one ambient
application language.

## Framework project topology

A small framework project whose Uhura client does not select `web-app@1`
normally has this explicit shape:

```text
spock.toml
backend/
  app.spock
client/
  uhura.toml
  host.toml
  machine.uhura
  ui.uhura
  evidence.uhura
  styles/theme.css
  providers/
    app.js
```

For an explicit client, the physical `.uhura` filenames are conventional, not
semantic. `uhura.toml` is the source of truth for logical modules:

```toml
[project]
name = "spock.starter"
version = 1
language = "0.4"

[modules]
starter = "machine.uhura"
ui = "ui.uhura"

[evidence.modules]
examples = "evidence.uhura"
```

Every owned `.uhura` file in that explicit shape must appear exactly once in
`[modules]` or `[evidence.modules]`. Imports use the logical name:

```uhura
use crate::starter::Starter;
```

Moving a physical file and updating only the module map must not change the
checked program identity. Public declaration identity follows package identity
and public name, not a directory route.

The only current filename-derived exception is an explicitly selected closed
Web application profile:

```toml
[framework]
profile = "web-app"
version = 1
machine = "crate::application::ApplicationMachine"
location = "crate::routing::Location"
```

`web-app@1` admits exactly these profile-owned source shapes in addition to the
explicit core and shared-evidence maps:

```text
app/**/page.uhura
app/**/page.examples.uhura
components/**/*.uhura
components/**/*.examples.uhura
surfaces/**/*.uhura
surfaces/**/*.examples.uhura
```

The root `app/page.uhura` is required. A discovered page contains exactly one
public machine-bound `*Page` UI. A component or surface contains exactly one
public pure component named from its filename; calls use exact immutable props
and finite emitted-event protocols and must be acyclic. Sibling example files
are discovered as evidence for that subject. These profile-owned files must
not also appear in the explicit maps, and unrecognized `.uhura` files under
the owned roots are rejected rather than becoming ambient modules.

Optional `[assets]` and `[icons]` tables also belong in `uhura.toml`. Do not
invent a separate catalog manifest.

`uhura.lock` exists exactly when `[dependencies]` is non-empty. Never
hand-author, retain, or delete it to bypass resolution. A local project with no
dependencies must not carry a lock.

## Deterministic evidence

Evidence modules use the same 0.4 lexer, imports, values, and machine contracts.
They define scenarios, checkpoints, and named UI examples without contributing
runtime declarations:

```uhura
use crate::starter::Starter;
use crate::ui::StarterWeb;

scenario walkthrough for Starter {
  start
  pin welcome

  send Increment
  expect Accepted commands []
  pin incremented
}

example welcome
  for StarterWeb as page default
  note "The clean starting point."
  = walkthrough::welcome;

example incremented
  for StarterWeb as page
  note "The same program after one accepted input."
  = walkthrough::incremented;
```

Use evidence to prove reachability and exact machine behavior:

- `start` constructs the machine;
- `send` dispatches an external event;
- `deliver` supplies a typed port input;
- `expect` checks the outcome and ordered commands;
- `pin` captures a checked checkpoint; and
- `example` selects a UI projection over a checkpoint for Editor.

Bind deterministic standard port drivers in a scenario before `start` when the
machine requires them. Prefer a scenario chain over duplicating state literals.
Cover loading, ready, empty, failure, pending, accepted, refused, retry, and
navigation states that materially change the experience.

Evidence is not live provider proof. It executes the ordinary deterministic
machine with checked inputs and adapters; Play exercises the admitted host
deployment.

## Live host admission

`host.toml` is read after the project resolves and checks. It selects one live
entry in 0.4:

```toml
[entry.starter]
machine = "crate::Starter"
presentation = "crate::StarterWeb"
lifetime = "application-session"
stylesheet = "styles/theme.css"
```

Host selectors use public declarations, not logical module paths. The entry
selects:

- one public machine;
- an optional `pub ui` bound to that machine;
- exactly the `"application-session"` lifetime;
- required configuration when the machine config is not `Unit`;
- a stylesheet;
- exact port-adapter bindings; and
- an optional provider module and configuration.

Core checking and evidence do not require `host.toml`. Play does.

## Port and provider boundary

Machines declare typed ports in `.uhura` source. The host maps each required
port locator to an adapter identity:

```toml
[entry.instagram.ports]
router = "web.history"
authority = "app.provider"
mutations = "app.provider"

[entry.instagram.provider]
module = "providers/dist/spock.js"

[entry.instagram.provider.config]
endpoint = "http://127.0.0.1:4000"
```

`web.history` is a checked built-in adapter. `app.provider` names ports supplied
by the configured provider module. These sets must cover the deployment
requirements exactly.

A custom module exports the provider adapter factory expected by the admitted
host, currently `createUhuraAdapters(config, host)`. Use the host-provided port,
adapter, contract hash, and contract-instance hash. Do not calculate or
hardcode compiler-owned identities.

Provider rules:

- implement only the ports assigned to the provider's adapter identity;
- preserve typed wire values and core-minted correlation ids;
- deliver one ordered, deferred stream so foreign code cannot synchronously
  re-enter a machine reaction;
- produce exactly one eventual settlement for each request;
- convert transport failures into modeled deliveries rather than throwing them
  into Core; and
- keep browser-native and authority-specific objects outside machine state.

The npm Spock host serves a generated JavaScript provider module but does not
compile app-specific TypeScript. Build provider source separately and point
`host.toml` at the generated JavaScript artifact.

## Authority boundary

Spock or another authority owns users, records, permissions, transactions,
files, accepted mutations, and durable timestamps. Uhura owns deterministic
session state, drafts, optimistic overlays, pending markers, notices, and
logical navigation.

Do not duplicate one authoritative fact in machine and provider state. Verify
durable consequences through Studio or the affected endpoint, not by trusting
Play state alone. Actor selection in a prototype is impersonation, not
production authentication or authorization.

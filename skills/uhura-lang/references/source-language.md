# Uhura 0.4 Source Language

Use `spock check` from a compatible npm distribution as the acceptance test.
Uhura 0.4 is strict, machine-first, and not backward-compatible with the
retired v0 page/store grammar.

## Rust-shaped core

Core source deliberately follows a bounded Rust shape:

- `use`, `pub`, and `pub use` for resolution and visibility;
- `struct`, `enum`, constructors, patterns, and exhaustive `match`;
- immutable `let`, `const`, `fn`, lexical `return`, and block-tail values;
- semicolon-terminated statements and comma-separated fields; and
- snake_case values, SCREAMING_SNAKE_CASE constants, and UpperCamelCase types.

It does not borrow Rust ownership, traits, macros, async execution, or unsafe
foreign calls. Do not import JavaScript or treat expressions as JavaScript.

Imports are explicit and inert:

```uhura
use uhura::observation::Observation;
use crate::notice::Notice;
```

`crate` names the current package's single closed compilation unit and module
tree—the role Rust calls a crate. `uhura` names checked standard contracts.
Outside an explicitly selected framework profile, physical filenames do not
infer modules; `uhura.toml` maps each logical core or evidence module to one
source file. The closed `web-app@1` profile is the only current exception: it
derives UI and sibling-evidence modules from its admitted `app/`,
`components/`, and `surfaces/` paths. Arbitrary filenames remain nonsemantic.

## Machine contract

A minimal complete machine:

```uhura
pub machine Starter {
  events {
    Increment,
  }

  outcomes {
    commit Accepted,
  }

  state {
    count: Nat = 0,
  }

  observe {
    count,
  }

  on Increment {
    count = count + 1;
    Accepted
  }
}
```

The machine is the state-transition authority. Its named sections may include:

- `config` and `require` for admitted construction parameters;
- `events` for external inputs;
- `commands` and typed `port` declarations for output and later delivery;
- `outcomes`, with explicit `commit` or `abort` publication policy;
- `state`, `computed`, and `observe`;
- `invariant` checks; and
- one `on` reaction per admitted input pattern.

Each reaction executes transactionally. A commit-policy outcome publishes its
new state and commands. An abort-policy outcome discards the draft and emits no
commands. Model pending work, optimism, settlement, and rollback as ordinary
typed state; do not hide them in a renderer or provider.

Use ports for typed host capabilities:

```uhura
use uhura::observation::Observation;
use uhura::ports::RequestPort;
use uhura::web_router::{Router, Routes};

pub machine Application {
  port router = Router<Location> {
    routes: APP_ROUTES,
  };
  port authority = Observation<Authority> {};
  port mutations = RequestPort<RequestId, Mutation, Settlement> {};

  // events, outcomes, state, observation, and reactions
}
```

Qualified port deliveries such as `mutations.Settled(request, result)` are
checked inputs. `emit mutations.Request(...)` publishes a typed command. A
provider is not called inline and cannot synchronously re-enter a reaction.

## Explicit Web UI profile

Core is complete without presentation. A source module activates Web UI only
with the exact direct import:

```uhura
use uhura::ui;
use crate::starter::Starter;

pub ui StarterWeb for Starter(view) {
  <main aria-label="Spock starter">
    <p>Count: {view.count}</p>
    <button on press -> Increment>
      Increment
    </button>
  </main>
}
```

`pub ui Name for Machine(view)` is a named pure projection of that machine's
observation. It does not allocate an instance, own state, access private
machine fields, or grant browser authority.

The 0.4 UI profile is Svelte-shaped where familiarity is useful:

```uhura
{#if view.loading}
  <p>Loading…</p>
{:else}
  <ul>
    {#each view.items as item (item.id)}
      <li>{item.label}</li>
    {/each}
  </ul>
{/if}
```

Expressions inside braces are Uhura expressions, not JavaScript. UI bodies
cannot mutate state, emit commands, run callbacks, access the DOM, or execute
host code.

Semantic event binding constructs exactly one machine input:

```uhura
<input
  value={view.query}
  on input -> SearchChanged(event.value)
/>

<button on press -> OpenProfile(person.user.id)>
  Open profile
</button>
```

The right side is a checked input constructor, not an eager call and not a
general function invocation. `event` exists only in the binding expression and
has the finite payload declared by the selected element contract.

Use standard framework vocabulary explicitly. For example, importing
`uhura::web_router::{Link, Router}` does not install routing or grant browser
history; the machine declares the `Router` port and `host.toml` binds it to a
host adapter.

## Current presentation bounds

- Web is the only presentation target.
- The checked UI element and widget vocabulary is finite.
- Reusable pure UI declarations and calls have exact immutable props, finite
  emitted-event protocols, and an acyclic call graph. `web-app@1` assigns
  component and surface file roles; the component semantics are part of the
  language rather than a second runtime. Slots and component-local semantic
  state remain unselected.
- A `pub ui` is independently selectable by host admission or evidence.
- Stylesheets are external files selected by `host.toml`; there is no Uhura
  `<style>` language block.
- There is no arbitrary DOM event, JavaScript callback, lifecycle hook, timer,
  randomness, ambient network, storage, or browser global in source.

Do not emulate missing language features through hidden provider state or
markup-side effects.

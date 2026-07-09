# RFD 0001 — Effects, `once` values, and `extern fn`

Status: discussion draft. Nothing in this document is accepted syntax.

Motivated by the README section "Where the model breaks": the pure contract
model strains along four rungs — effects, protocols, algorithms, and other
ecosystems. This RFD sketches language surface for rungs 1 and 3, and leaves
notes toward rung 2.

## 1. Effectful std builtins

Some values the grammar cannot compute but the contract must name: entropy,
time, digests.

```spock
use std::crypto
use std::time
```

- A `fn` that touches an effectful builtin is effect-marked. The compiler
  therefore knows which functions are deterministic and replayable, and which
  are not.
- The prototype runtime may run entropy from a seed, so a prototype session is
  reproducible on demand.

## 2. `once` values — ephemeral outputs

A `once T` value is transaction-local by construction:

- it may not be stored in any `table` field
- it may not appear in any `view`
- it appears in at most one `fn` return, exactly once
- it is redacted from logs and traces

Motivating example — an API key revealed exactly once, with only its digest
persisted:

```spock
fn mint_api_key(name: text) -> { id: uuid, key: once secret } {
    let key = std::crypto::token(32)      // effect: entropy
    let row = insert api_key {
        name: name,
        prefix: key.prefix(8),
        hash: std::crypto::sha256(key),   // only the digest is stored
    }
    return { id: row.id, key: key }       // plaintext exists only here, once
}
```

Doctrine note: `fn` outputs are not projections. Views project durable truth;
functions may return values destroyed on commit. `once` turns "revealed only
once" from a comment into a checkable property.

Same family: password setup, TOTP secrets, signed upload URLs, email
verification tokens.

Open question: may a `once` value flow into a declared channel effect (a
verification token sent by email)? Probably yes, with the channel typed for
secrets — but never into storage.

## 3. `extern fn` — the typed foreign boundary

Rung 3 (order matching, CRDT merge, argon2 internals, transcoding, tax
rulebooks) genuinely requires Turing-completeness. Every surviving declarative
system converged on the same answer: a foreign-function boundary behind a
declared interface — SQL's C extensions, Terraform's providers. Spock's
version must keep the contract even when the body is foreign:

```spock
extern fn charge_card(amount: money, card: card_token) -> charge_result
    writes payment
    effects net::stripe
```

- The body is hosted as a Wasm component with no ambient authority: it can
  touch only what the declaration grants.
- Policy wraps the call like any other `fn`.
- The foreign subset is lexically visible in the source — the LINQ rule. LINQ
  failed because its untranslatable subset was undiscoverable until runtime;
  `extern` is the same subset made explicit.
- The escape hatch may replace the body, never the contract.

### Prototype stubbing

In the prototype runtime an `extern fn` is a controllable fixture:

```spock
stub charge_card {
    scenario approved -> { status: "approved" }
    scenario declined -> { status: "declined", reason: "insufficient_funds" }
    scenario timeout  -> !net::timeout
}
```

Validating a checkout means playing all three scenarios. What would be a
dangerous shortcut in production is the correct fidelity for validation.

## 4. Notes toward protocols (future RFD)

Rung 2 — operations that span an external decision or real time (card
authorization, auction close, approval flows) — implies a saga grammar:
states, steps, compensations, durable timers. The prototype runtime should run
these in simulated time (the auction closes in seconds, not days). Deferred to
its own RFD.

## Open questions

- Effect granularity: `net` vs `net::stripe` — how fine should the effect
  lattice be?
- `once` values crossing into channels (see §2).
- The extern capability model: exactly what a Wasm body may be granted.
- Deterministic replay in the presence of entropy — seeded prototype mode
  vs recorded effects.

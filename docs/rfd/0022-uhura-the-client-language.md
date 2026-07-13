# RFD 0022 — Uhura, the client language beside Spock

Status: **discussion draft**. This RFD records why the [Uhura](../../uhura/README.md)
project now lives in this repository, what the integration between the two
languages will look like, and where the hard seam is. It lands no code: the
contract projection and the adapter it describes are direction, not
implementation. The type-mapping decisions in §4 are recorded as open, not
decided.

## 1. What Uhura is

Uhura is an incubating declarative UI language and deterministic headless
experience runtime — "a minimal Svelte without JavaScript." Svelte-flavored
markup over a closed transition language and typed service ports, compiled by
a Rust checker into a replayable machine that evaluates a checked program into
a renderer-neutral semantic view and emits typed commands and platform
intents. It owns UI-session state and experience behavior; it does not paint
pixels, perform I/O, or hold authoritative product truth. The authoritative
design lives in `uhura/docs/`, exercised end to end by the Instagram slice at
the repo-root `examples/instagram-uhura/`.

## 2. Why it lives here — the vacant client slot

Spock's own doctrine forbids Spock from building a client language. RFD 0010's
governing rule is "generate types, never the client"; RFD 0009's walls are
"borrow, don't build" and "language work is the differentiator" — where the
language is Spock itself. Everything client-side is deliberately borrowed:
graphql-codegen, urql, Apollo, a future thin protocol client. The one thing
that doctrine rules out of Spock is exactly the space a *sibling* language
occupies: a specification language for the client half of the product, with
its own grammar, checker, and runtime.

The fit was designed in from both sides. Uhura's design doc names Spock as its
intended real provider — its architecture diagram reads `fixture driver ⇄
future Spock adapter`, its seam crate is documented as "the Spock seam crate,"
and its §9.6 is titled "The Spock-replaceability argument," pinning exactly
what stays byte-identical when Spock replaces the test fixture. Together the
two languages state the whole product: Spock is the executable PRD for the
backend; Uhura is the executable PRD for the experience. Spock's README notes
that commercial prototyping tools stop at click-through screens — "the backend
of every prototype is imaginary." Uhura is the inverse image: a real client
prototype whose backend has so far been a scripted fixture. Each is the
other's missing half, and the shared Instagram dogfood makes that literal
(§6).

**The relationship is canonical provider, not hard-wiring.** Uhura's port seam
(`uhura-provider/0` envelopes, hash-pinned port contracts, a conformance suite
that runs against any driver) stays provider-neutral, and the scripted fixture
remains the permanent CI test double — both per uhura design §9.6. Spock
becomes the privileged first provider: dogfooded, contract-projected, and what
`uhura.lock` binds to by default. This mirrors Spock's own relationship to
SQLite — first conforming engine, not the definition.

Boundary hygiene, restated from both projects' founding docs: no fact may be
authoritative in both languages; Uhura is a separate nested Cargo workspace
with its own toolchain pin and no dependency on Spock internals; the `spock`
npm name and binary surface are untouched. Uhura sits beside the reserved
slots, never inside them.

## 3. What binds them — the contract, not the crates

Direct code sharing between the two workspaces is deliberately near-zero. The
value models are doctrinally incompatible: Spock has `float` and wall-clock
`timestamp`/UUIDv7 throughout; Uhura's core denies float arithmetic at the
compiler level, bans clocks and randomness from the machine, and its
canonical-JSON hasher rejects floats outright. Forcing a shared base crate
would make one language lie about its doctrine. What is genuinely shared is
one level up: **the contract as data**.

Spock already emits its whole contract as a frozen-additive JSON document
(`GET /~contract`, spec §6) that downstream tools consume without touching the
compiler. Uhura already consumes typed port contracts (`*.port.toml`,
hash-pinned) from whoever provides them. The integration artifact is a
projection from the first to the second.

## 4. The contract projection

A deterministic generator — working name `spock gen ports` (binary-owned, per
RFD 0010's "the binary owns generation"), or equivalently `uhura import spock`
on the other side; which binary owns it is an open decision — that reads the
contract JSON and emits Uhura port contracts:

| Spock (contract IR)                     | Uhura (port contract)                        |
| --------------------------------------- | -------------------------------------------- |
| `fn` (mut)                              | command with outcome union                   |
| minted refusals (`!` codes, 409)        | refusal variants of the outcome              |
| derived error codes (per constraint)    | typed failure outcomes                       |
| reserved codes / transport failure      | `unavailable` (adapter-mapped, §5)           |
| read `fn` / future `view`               | read-model projection                        |
| `record` (flat, scalar-only)            | port record                                  |
| `set` (closed string union)             | `enum`                                       |
| `uuid`                                  | `id`                                         |
| `text`, `int`, `bool`                   | `text`, `int`, `bool`                        |

Two type decisions are recorded here and deliberately left open:

- **`timestamp`** — Uhura has no datetime type and forbids clocks. Candidates:
  project as an opaque nominal (echo-only, ordered server-side), or exclude
  from port shapes and require contracts to expose ordering some other way.
- **`float`** — excluded from ports. Uhura cannot carry it; a contract field
  that a client genuinely needs must be reshaped (fixed-point int, text) on
  the Spock side.

The projection must be total and checked: a Spock contract that cannot be
projected should fail generation with a stated reason, never emit a port
contract that lies (the same law RFD 0010 sets for the TypeScript emission).

## 5. The adapter

A real provider adapter — the thing uhura's design calls the "future Spock
adapter" — owns everything the port seam forbids from crossing into the
machine:

- **Wall-clock and transport.** The machine has no timeouts; the adapter maps
  transport failure to `unavailable`.
- **Identity.** The `X-Spock-Actor` header (RFD 0014), later the bearer token,
  lives entirely in the adapter. Nothing identity-shaped crosses the seam.
- **Settlement.** Uhura requires exactly-one-outcome-eventually and read-model
  consequences settled at or before the outcome. Spock satisfies the hard
  half for free: fns return the written row, which is precisely the
  "read-model fragment carried in the command response" pattern uhura design
  §9.4 names as the conforming CQRS shape.
- **Conformance.** The adapter passes the same conformance suite the fixture
  passes. The fixture stays the CI double; the adapter is for live play.

## 6. The determinism boundary

The one deep tension. Spock mints wall-clock UUIDv7 ids and real UTC
timestamps on every write — both on Uhura's forbidden-inputs list. Against a
live Spock, Uhura's *per-step* determinism is untouched (that property never
depended on the provider), but *byte-golden replay* of full traces is not
possible: two identical runs produce different ids and timestamps.

Uhura's design already absorbs this — golden traces live on the fixture,
live delivery order is allowed to be nondeterministic — so nothing breaks. But
if end-to-end golden traces across both languages are ever wanted, the
resolution is on the Spock side: a deterministic prototype mode (seeded id
mint, virtual clock). That fits Spock's identity — the README already imagines
sagas running in simulated time — but it is a real RFD of its own and is
explicitly out of scope here.

## 7. Acceptance direction

Both repos dogfooded the same product. Spock's `examples/instagram/v0.spock`
models 19 tables and 43 fns; its TypeScript client demo exercises five floor
CRUD calls and zero deliberate fns — no actor header, no refusal branching, no
pagination (`v0-FEEDBACK.md`, G12/G13/G16 and the client README's own scope
note). Uhura's Instagram spike is exactly the missing consumer: optimistic
like with rollback, typed refusal branching, cursor pagination, keyed
navigation.

The integration's acceptance proof is therefore already written: **Uhura's
Instagram slice running against Spock's `v0.spock` through the projection and
the adapter.** The day that works, the client-surface gaps in Spock's own
dogfood ledger close from the outside, and the two executable PRDs compose
into one.

The two slices sit side by side today as distinct repo-root folders —
`examples/instagram/` (Spock backend) and `examples/instagram-uhura/` (Uhura
client) — sharing only the canonical PRD, because they are not yet wired.
Merging them into a single `examples/instagram/` domain (backend + client
sides) is deferred to the milestone that lands the projection and adapter,
when the runtime wire dictates the shared layout.

## 8. Non-goals

- No code lands with this RFD — projection and adapter are future milestones.
- No npm name reservation for uhura; it publishes nothing.
- The `UHURA_REQUIRE_PARITY=1` CI gate stays deferred (uhura's own debt
  ledger).
- No repository rename is proposed.
- No change to Spock's language surface, roadmap order (RFD 0009), or the
  reserved `spock` npm client slot (RFD 0010).

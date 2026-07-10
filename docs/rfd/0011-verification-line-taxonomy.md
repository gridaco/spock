# RFD 0011 — the verification line and its vocabulary

Status: accepted decision — Spock is safe by default, and code below the
language's verification line must be lexically acknowledged. The SQL
escape body becomes `unchecked sql("...")` (forced; unmarked bodies are a
parse error), `spock check` counts unchecked bodies, and the tier
vocabulary is fixed for the language's future: **`unchecked`** for
verification gaps, **`unsafe`** reserved for the runtime-integrity tier,
the **vouch** tier stays with RFD 0004's waivers.

## 1. The problem

The `fn` escape body (§7.4) is verified by the *engine* at load — syntax,
name resolution, placeholders, result shape — but not by the *language*:
`spock check` passes a program whose SQL logic is wrong. The doctrine
already demands that such a gap be visible (the LINQ lesson: "the
untranslatable subset made lexically visible", README rung 3; RFD 0002:
"every derivable outcome must be acknowledged"). `sql(` is greppable, but
nothing in the grammar *costs* anything at the boundary, and when native
body statements land (v1.spock's `require`/`let`/verbs), verified and
unverified bodies will be indistinguishable at a glance. Pre-release is
the only cheap moment to add the marker: retrofitting it later breaks
every program.

## 2. The survey: does anyone need more than `unsafe`?

The keyword choice hinged on one question — do languages that live long
enough grow a *taxonomy* of these markers, or does one word suffice? A
three-family survey (systems, managed, verification-oriented; every
load-bearing claim source-verified) answers decisively: **multi-marker
taxonomies are the norm, not the exception.**

| Language | Runtime-integrity tier | Verification-gap tier | Author-vouches tier |
|---|---|---|---|
| C# | `unsafe` (keyword) | `checked`/`unchecked` (keywords, overflow) | `!` null-forgiving (no runtime effect) |
| Java | `Unsafe` (API, now JEP-471-gated) | "unchecked" — the JLS's own word (`@SuppressWarnings("unchecked")`, `-Xlint:unchecked`) | `@SafeVarargs` ("programmer assertion") |
| Swift 6 | `@unsafe` + `unsafe` expressions (SE-0458) | `!`, `try!`, `as!`, `&+` (trapping/defined) | `@safe`, `@unchecked Sendable`, `nonisolated(unsafe)` |
| Rust | `unsafe` blocks/fns (graded by position, RFC 2585) | `wrapping_*` naming (defined-behavior axis) | `unsafe impl Send`, `unsafe extern` (2024) |
| Kotlin | — | `!!`, `as` | `@RequiresOptIn`/`@OptIn` — a user-extensible vouch *framework* |
| Safe Haskell | `Unsafe` pragma | — | `Trustworthy` — "the guarantee is provided by the module's author" |
| SPARK/Ada | `pragma Suppress` (erroneous if wrong) | `SPARK_Mode Off` | `pragma Assume` + justifications |
| Dafny | — | `{:verify false}` | `assume`, `{:axiom}` |

Two findings beyond the headline:

- **The ledger is standard practice in the verification family.** `dafny
  audit` enumerates every soundness-limiting construct; GNATprove counts
  Justified vs Unproved checks; Lean/Rocq print axiom dependencies. A
  language that aspires to be an executable specification reports its own
  holes.
- **A caution: "unchecked" straddles two meanings in the wild.** In
  C#/Java/Swift it means a *defined-behavior* check is skipped (runtime
  stays sound); in Rust's `_unchecked` method names it means
  UB-if-violated. A new language must pin one meaning per word and never
  cross them.

## 3. The decision: three tiers, three words, fixed now

- **`unchecked` — the verification-gap tier** (the C#/Java majority
  sense, pinned): the *checker* does not verify this; the runtime stays
  sound. First occupant: the SQL escape body. Spock's escape can never be
  Rust-`unchecked` — parameters are bound (no injection by
  construction), the engine load-verifies shape/names/placeholders, and
  execution is transactional with derived-error routing. What is skipped
  is exactly `spock check`'s verification of the body's logic.
- **`unsafe` — reserved, unoccupied.** If any future construct deserves
  its connotations it is the rung-3 authority boundary (RFD 0001's
  `extern fn` with effects and Wasm hosts) — genuine
  runtime-integrity/ambient-authority territory. Burning the universal
  lexeme on SQL-against-your-own-schema would leave nothing for the tier
  that needs it. (`unsafe` joins the reserved keyword list.)
- **The vouch tier stays with RFD 0004** — waivers and `todo()` as
  counted, named assertions (the Safe-Haskell-`Trustworthy` shape). Not
  new syntax now.

The marker sits **on the escape, not the fn signature** — Rust's own
logic: callers get the full contract (signature, arity, derived errors)
regardless of body form, so a fn with an escape body is "safe to call"
and only the body crosses the line. GraphQL and TypeScript surfaces are
unchanged. When native statements arrive they are checked by default,
and `unchecked sql(...)` keeps its mark inside otherwise-verified
bodies — block granularity.

```spock
fn rename_user(user: user, name: text) -> user ! user_username_taken {
  unchecked sql("""
    UPDATE user SET username = :name
    WHERE id = :user
    RETURNING *
  """)
}
```

Unmarked `sql(...)` is a parse error whose message states the reason:
the acknowledgment is the point. `unchecked` is contextual, like `sql` —
a legal identifier everywhere else.

## 4. The ledger

`spock check` reports the count — `ok: 5 table(s), 1 record(s), 3 fn(s)
(3 unchecked bodies), 13 seed row(s)` — the RFD 0004 counted-holes move,
now backed by the `dafny audit` precedent. The number is the language's
own maturity metric: as native body statements land, it trends toward
zero, and the trend is visible.

## Open questions

- Whether `unchecked` ever marks anything besides escape bodies (an
  unchecked cast? none exist yet — the word is scoped to the escape
  until something else earns it).
- The vouch tier's syntax when RFD 0004's waivers ship, and whether
  `spock check --audit` grows a Dafny-style enumeration (holes with
  spans) beyond the count.

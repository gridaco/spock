# RFD 0004 — The exposure model: defaults, obligations, and the surface ledger

Status: discussion draft. A stance is proposed here; nothing is fixed.

## The question

How does the language enforce good design, instead of trusting the expert —
human or AI — to make good choices? Concretely: the decision of what becomes a
`view` and what becomes a `fn` should be language-proposed, not taste. And the
default posture of the surface (public? private? inferred?) must be chosen.

Languages have exactly four mechanisms for this, and Spock should use all of
them:

1. **Fail-safe defaults** — nothing is reachable unless granted.
2. **Friction proportional to risk** — the dangerous thing costs more
   keystrokes and is loud in the diff; the safe thing is the short path.
3. **Closed-world checks** — the totality lints from RFD 0002.
4. **Obligations** — declared intent the compiler closes the loop on. This is
   the new piece this RFD adds.

## 1. The default: deny, always

The menu was: all views public by default, all private by default, or
"neutral/smart." The failure modes decide it:

- Forgetting to **expose** fails loudly and cheaply: a missing field, a 403,
  discovered the first time anyone plays the prototype.
- Forgetting to **hide** fails silently and catastrophically: a leak,
  discovered in production or never.

**Defaults should fail toward the failure you can see.** So: deny by default.
A table with no view has no surface. A view with no role binding is inert (and
the dark-data lint flags it). A field not projected does not exist to the
outside. The prototype mission makes this nearly free — missing exposure
surfaces within minutes of play.

"Smart" defaults are rejected *for the source*: inferred exposure makes the
contract implicit again, which is the disease this language treats. But
intelligence is welcome *at authoring time*: the compiler (or an agent) may
scaffold proposed views and fns from a policy or an obligation — the human
accepts them into source, where they are explicit forever. Same pattern as the
seed LLM: intelligence when authoring, determinism when compiled.

## 2. The exposure gradient

Per field, a small ladder — names illustrative:

- **internal** (default) — exists in the state space; no surface at all.
- **granted** — projected by a view bound to a role. The grant lives on the
  view, not the field.
- **protected** — may never be writable through any view; the only write path
  is a `fn` that names it. The vision draft already had this instinct:
  `@protected` on `username`, and `fn change_username ... unlocks .username`.
  This RFD systematizes it.
- **immutable** — written at insert, never again, by anything (`author`,
  `created_at`).

Binding a protected field writable into a view is not a warning — it is an
error whose message proposes the missing `fn`. If someone truly wants the
bypass, it must be a named waiver (see §6), never a silent flag.

## 3. View vs fn is derived, not chosen

Three laws make the split checkable:

1. **Purity** — a view has no effects.
2. **Provenance** — a view writes only through write-through fields
   (RFD 0003).
3. **Locality** — a view write maintains no cross-row or cross-table
   invariant.

Anything that breaks a law is, by definition, a `fn`. "User purchases X"
touches orders and inventory (breaks 3) and emits a payment effect (breaks 1)
— so the compiler does not merely reject the view attempt; it proposes the
deliberate transition, with the signature, the unlocked fields, and the
derived error set pre-filled. **The language proposes; the author confirms.**

## 4. Obligations — intent the compiler can hold you to

"A user should be able to purchase X" is not a guard; it is a *requirement*.
Guards say **may**; obligations say **must be able to**. Sketch:

```spock
// illustrative only
expect role::user can purchase(product)
expect role::visitor can read view::post_preview
```

An obligation is a reachability claim over the transition system, so it is
checkable: does a transition exist, is it granted to the role, is it
completable (its policy satisfiable, its error set acknowledged)? Two lint
directions squeeze the design from both sides:

- **Grant without need** — exposure no obligation or flow requires (surface
  to reconsider).
- **Obligation without path** — the emitted, machine-derived **design todo
  list**. This is what "language-proposed design" means in practice: the gap
  between what the product promises and what the contract grants is computed,
  not remembered.

Obligation + seed persona = a playable acceptance test: "maya (role user) can
complete purchase(keyboard)" runs against the prototype. The PRD stops being
prose.

## 5. The surface ledger

Because exposure is all grants, the compiler can emit the complete surface as
data: role × field × read/write × via (view or fn). Two consequences:

- The ledger is reviewable — the attack surface is a table, not an audit.
- Every change produces a **surface diff**, in the `terraform plan` tradition:
  "this change lets `role::user` write `b.email` via `view::z`." Review the
  diff, not the codebase.

## 6. Escapes that stay honest

Strictness without paralysis: deferral is explicit and enumerable.

- A **waiver** names the law it suspends and appears in the ledger.
- A `todo()` body is a first-class hole: allowed, counted, reported. A
  prototype may ship full of declared holes; what it may not have is an
  unknown one.

## Open questions

- Are guards and obligations both `policy`, or two keywords? (Lean: two —
  `policy` guards, `expect` obliges; naming open.)
- Grant granularity: where does deny-by-default verbosity actually bite, and
  what bulk-grant forms fix it without weakening the default? Measure on the
  Instagram example.
- State-conditional exposure (the vision's `respects draft`): grants that
  depend on row state — needed, but semantics unsettled.
- Prototype-mode strictness: same rules, cheaper waivers — or looser rules?
  (Lean: same rules; looseness poisons the artifact.)

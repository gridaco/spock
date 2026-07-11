# RFD 0014 — The actor seam (identity milestone, slice 1)

Status: **discussion draft**. A stance is recommended here; two decisions are
left open for ratification (§11). This RFD proposes the *first* slice of the
auth track (RFD 0009 track 7) — an actor context and dev-time impersonation —
deliberately *without* roles or RLS, which stay v1. No implementation is
proposed yet; this is the design record that a later milestone would build
from.

---

## 0. The question

Can a Spock prototype be *played as different users* — and can a `fn` body
know *who is calling* — before the full auth/`policy` machinery exists? The
demand is concrete and the doctrine already points at the answer; what was
missing was the exact surface. This RFD settles it.

Two framings were on the table, both from the same brief:

1. **Conventional** — an explicit actor/role concept with an `impersonate`
   affordance, so the prototype is playable and a `fn` can read the caller.
2. **Unconventional** — declare `table user extends auth` and let *the
   reference graph* decide ownership: whoever points at `user` (by key or
   unique) becomes that user's "role-defining table," inferred rather than
   declared.

The recommendation keeps framing 1's *mechanism* and framing 2's *anchor
instinct*, and **kills framing 2's inference** — for reasons that are doctrine,
not taste (§6). The design was produced by a judged panel of four competing
designs and hardened by an adversarial review; the review found two soundness
defects in the panel's own winner, both folded in below.

---

## 1. What the dogfood proves (the pain is named: G12)

`examples/instagram/v0.spock` threads identity as a **client-asserted
parameter** through nearly every fn — `viewer: user`, `actor: user`,
`author: user` — and checks it in raw SQL:

```spock
// today — the runtime believes whoever names `actor`. The guard is theater.
mut fn delete_comment(comment: comment, actor: user) -> comment? {
  unchecked sql("""
    DELETE FROM comment WHERE id = :comment
      AND (author = :actor OR :actor = (SELECT author FROM post WHERE id = comment.post))
    RETURNING *
  """)
}
```

The file's own header says it: *"every fn below takes its actor as a
client-asserted parameter (G12)."* This is two problems in one:

- **Unsound.** A client passes any `actor` it likes and deletes anyone's
  comment. Ownership guards across the file are decorative.
- **Verbose.** `:viewer`/`:actor` is repeated in ~8 fns and every read that
  wants a personalized answer (`home_feed`, `saved_posts`, `notifications`).

And a third, quieter one: reads that *should* depend on the viewer structurally
cannot. `post_comments` notes it can't show "visible, **or** held-and-mine"
because it has no viewer; `search_profiles` can't exclude profiles that blocked
the searcher. The missing actor is a *capability gap*, not only a soundness gap.

## 2. How the design was chosen

A four-way judged panel (doctrine / LLM-writability / forward-compat lenses)
scored: **hybrid 59, minimal 57, `extends`-inference 46, explicit-roles 37**.
The explicit-roles design was disqualified for shipping RLS-lite (an
engine-injected `for <role>` precondition — "this is RLS") a full milestone
ahead of the roadmap's auth-then-policy sequencing. The `extends`-inference
design shipped the same clean core but buried it under a reference-graph→role
scaffolder that overreaches doctrine (§6). The winner (hybrid) was then
adversarially reviewed; the review confirmed the core is sound but found **two
HIGH defects** in it — the nullability hazard (§4.4) and a `DIRECTONLY`
category error (§4.2) — both corrected in this RFD, plus grafts from the other
three designs.

---

## 3. The doctrine walls

Any actor design must clear five fixed constraints already in the RFDs. They do
most of the design work.

| # | Wall | Source | Consequence here |
|---|---|---|---|
| W1 | **Deny by default; no inferred *source*.** *"inferred exposure makes the contract implicit again, which is the disease this language treats"* — with the escape *"intelligence is welcome at authoring time"* (the compiler may **scaffold** proposed source the author accepts). | RFD 0004 §1 | The anchor is *declared*, never inferred (§4.1). The reference-graph idea survives only as an authoring-time scaffold (§6). |
| W2 | **Explicit actor context; no definer/invoker split, no confused deputy.** | RFD 0005 #4 | `spock_actor()` is the *invoker's* asserted identity, made available — never a runs-as-owner authority, and never leaked into a context it wasn't request-scoped for (§4.2). |
| W3 | **Identity needs a consumer.** *"identity is inert until something consumes it"*; a fn-body binding is its **first** consumer, `policy` its **second**. | RFD 0009 track 7 | The seam lands *in fn bodies* first; the floor and reads-governance wait for v1. And the consumer cannot exist without the anchor (E-ACT03, §4.2). |
| W4 | **Roles/RLS are v1.** `role`/`policy`/`view` are reserved-and-absent (spec §2.3, L005). | RFD 0002 §4, 0009 track 10 | No role taxonomy, no per-row governance, no `role` value on the wire in this slice (§10). |
| W5 | **LLM-writability.** SQL-exact or radically simple; never a bespoke mini-grammar. | RFD 0013 | One anchor token; the seam is `spock_actor()`, the shape an LLM already writes as `auth.uid()`. No new rule grammar. |

The banked auth architecture (RFD 0008 §4) supplies the shape: *"a header
selecting the actor populates the same claims context a verified JWT later
will; downstream code is identical … the swap is one resolver function."* This
RFD is the v0 half of exactly that sentence.

---

## 4. The recommendation: the actor seam

Four moving parts, each minimal. The whole milestone is *one scalar, made
truthful*: relocate identity from "client argument" to "runtime-populated
seam," changing nothing else about the SQL.

### 4.1 The anchor — mark which table is identity

One marker on the table that is the actor. **Recommended spelling: a prefix
modifier, `auth table`** (parallel to `mut fn`), *not* `extends`:

```spock
auth table user {
  key id: uuid = auto
  username: text check valid_username unique
  private: bool = false
  joined_at: timestamp = now
}
```

**What it means, exactly:** *this table's key is the actor identity.* Its key
values are what `spock_actor()` returns, what the impersonation header names,
and what a JWT `sub` will later carry. It is the app-side projection of the
future builtin `auth.users` seam (RFD 0008 §4), named now so fn bodies and the
claims seam have a binding target.

**What it deliberately does NOT do** — the least-magic guarantees:

- **Zero storage change.** No new column, no `auth.users` FK (v1), no DDL
  difference. A pure contract marker.
- **Zero roles, zero grants, zero predicate.** An anchor widens no reachability;
  deny-by-default (W1) is untouched.
- **Zero runtime scoping.** Nothing is auto-filtered by owner. The floor stays
  actor-blind (§5). This is *declaration*, stating a fact the language cannot
  today — *which table is the actor* — and nothing more.

**Load rules (three new checks):**

- **E-ACT01 — at most one anchor.** The actor space has one identity table.
  (v1 may add other bases; v0 has exactly `auth`.)
- **E-ACT02 — the anchor key is a single scalar column.** `spock_actor()`
  returns one value; a composite-key anchor has no scalar identity — rejected
  outright, not deferred. `key id: uuid` qualifies; a natural key
  (`key handle: text`) makes `spock_actor()` return `text` (already works
  end-to-end, dogfood C6).
- **E-ACT03 — a consumer needs the identity** (§4.2).

**Why `auth table`, not `table user extends auth`** (this reverses the
originally-proposed spelling — see §11.A). The review and all three judge
lenses converged on it:

1. `extends` **connotes field inheritance** (TypeScript). The anchor inherits
   *no* columns — an LLM writing `table user extends auth` will then write
   `SELECT email FROM user WHERE id = spock_actor()` expecting an inherited
   `email`, and `prepare()` fails "no such column." The token actively
   mis-teaches — a direct W5 violation.
2. `extends` is **spoken for by the role layer.** The vision draft
   (`docs/rfd/0000-vision.spock`) already earmarks it for
   `role post_author extends user` / `role user extends auth::user` — a
   *different* semantic layer (a role refining a base) from "this table *is* the
   identity." Spending `extends` on the anchor overloads it across two
   client-facing layers with no room to disambiguate. **Names before bindings**
   (RFD 0009): keep `extends` unspent for v1's roles.
3. `auth table` is Spock's own established shape (`mut fn`, `auth table`) — a
   *tag*, not inheritance.

`auth` and `extends` should both be reserved in spec §2.3 the day this ships,
so the choice is a decision, not an accidental grab.

### 4.2 The seam — `spock_actor()`

A fourth engine builtin alongside `spock_uuid()` / `spock_now()` /
`spock_refuse()`. Zero-arg, returns the anchor key scalar, **NULL when
anonymous**. This is the literal shape of the RFD 0008 §4 mirror target,
`auth.uid()`:

```spock
mut fn delete_comment(comment: comment) -> comment? {
  unchecked sql("""
    DELETE FROM comment WHERE id = :comment
      AND (author = spock_actor()
           OR spock_actor() = (SELECT author FROM post WHERE id = comment.post))
    RETURNING *
  """)
}
```

**It is invisible to fn-body load-validation** — the single strongest property.
A scalar function call is an ordinary expression node, *not* a `:param` bind
slot, so the both-directions "every `:param` is a declared parameter, every
parameter is used" rule (spec §7.4) never sees it — exactly as it never sees
`spock_now()`. **No new fn-body validation rule.**

**Registration is DIRECTONLY, and gated on the anchor.** Two corrections to the
naïve design, both load-bearing:

- **DIRECTONLY — `spock_refuse`'s posture, NOT `spock_now`'s** (corrects a HIGH
  defect). `spock_uuid`/`spock_now` are registered *non*-DIRECTONLY *on
  purpose* — [engine.rs:111](../../crates/spock-runtime/src/engine.rs):
  *"DEFAULT clauses must be able to call them."* They are stateless, safe
  anywhere. `spock_actor()` is **request-scoped state**, correct *only* inside
  a `func::call` where it is re-bound per request. Registering it
  non-DIRECTONLY would let a floor `DEFAULT spock_actor()` or an
  inline-expanded `CHECK(... spock_actor() ...)` (RFD 0013) evaluate it *outside*
  `func::call` — against the connection-global closure still holding the *last*
  fn caller's actor. On the single Mutex-serialized connection that is a
  **cross-request confused deputy** (W2): an anonymous floor insert stamps rows
  as whoever called last, and it silently corrupts RFD 0013's actor-blind-CHECK
  determinism law. `spock_refuse` is already DIRECTONLY
  ([engine.rs:123](../../crates/spock-runtime/src/engine.rs)) — *"a refusal is
  a fn-body statement speaking; nothing indirect may raise"* — and works fine
  inside read-fn SELECTs, which proves DIRECTONLY does **not** block top-level
  fn bodies. `spock_actor()` takes the same posture: usable in every fn-body
  read and write, loudly rejected in any DEFAULT/CHECK/trigger/view/
  generated-column, which is precisely correct.
- **Gated on the anchor (E-ACT03).** Register `spock_actor()` only when a table
  is `auth`-marked. A body that calls it with no anchor then fails at
  `prepare()` during load ("no such function"). *You cannot consume an actor
  you never anchored* — W3 enforced mechanically, for zero extra lint code.

**Per-request population — the "one resolver function."** The resolved actor
threads into `func::call` (the shared execution primitive for REST
`/rest/v1/rpc/{fn}` and every GraphQL fn root field), which re-binds the
`spock_actor` closure to this request's value (NULL if anonymous) right before
the transaction opens. Because there is exactly one serialized connection, the
re-bind is race-free and the value is fixed for the fn's one serializable
transaction (evaluated against pre-state, per RFD 0012). **Reads carry the
actor symmetrically with writes** — one thread serves GraphQL Query fields and
`GET /rest/v1/rpc/{fn}` alike; the polarity marker only picks the HTTP verb and
transaction mode, never which builtins are callable.

### 4.3 Impersonation — one knob: dev header + seed personas

Impersonation is **not a second code path**. Following the cross-system lesson
(PostgREST's role claim, Hasura's admin-secret + `x-hasura-role`, Studio's user
picker), it is the *normal actor-carrying seam fed a value chosen by a trusted
caller*. So v0 builds exactly one knob and refuses an `/impersonate` endpoint or
a `spock run --as` flag.

- **The header:** `X-Spock-Actor: <anchor-key>` — the actor's key value,
  verbatim, **unverified in v0**. This *is* the future `sub`. Absent →
  anonymous → `spock_actor()` NULL. Deliberately forgeable, honest about the
  ungoverned prototype tier — not pretending to secure what a v1 signature will
  secure.
- **The resolver is anchor-key-type-aware** (corrects a LOW defect). It parses
  and canonicalizes the header value by the anchor's key type exactly as
  `path_key_value` already does for by-key GETs
  ([http.rs](../../crates/spock-runtime/src/http.rs)) — so a `uuid` sent
  uppercased or braced still matches the stored lowercase-canonical key. This
  same resolver is the *one function* the v1 JWT swap replaces (it will verify
  the signature and read `sub` instead of trusting the header).
- **Seed personas — zero new syntax.** A persona in v0 is a seed row in the
  anchor table; the dogfood already ships five (`maya`, `luis`, `noor`, …).
  RFD 0002's "personas belong to seeds (role + identity)" degenerates cleanly,
  in a role-free v0, to *identity only*. Seed still may not call fns and runs
  before the listener binds, so `spock_actor()` is simply NULL through seed
  replay, harmlessly.
- **Two `~`-meta endpoints** (additive, next to `/~contract` and `/~health`):
  - `GET /~personas` — the dev actor picker: the anchor table projected to
    `[{ actor: <key>, label: <first unique text field, else key> }]`. Pick
    `maya`, send her key in the header. Resolves "pick a known identity, don't
    type a raw UUID" with no persona-name mini-grammar.
  - `GET /~whoami` — the debugging primitive (the dev-tier mirror of GoTrue
    `GET /user`): echoes `{ actor, anonymous, known }` where `known` = the key
    exists in the anchor table. Answers "am I sending the header right?" and
    "why does my guard match nothing?" — a typo'd key surfaces as
    `known: false`. Never rejects.

### 4.4 The nullability law — the swap is NOT text substitution

This is the first HIGH defect the review found, and it changes how the milestone
must be described. The client param `actor: user` was **required** (non-null);
`spock_actor()` is **NULL for anonymous**. So the swap is *not* semantics-
preserving, and "anonymous fails safe for free" is **false in general**:

- **`= spock_actor()` guards** fail safe (anonymous matches no owned row — NULL
  compares unequal). ✅
- **Identity stored into a NOT NULL column** (`INSERT … author = spock_actor()`)
  fails safe — an anonymous write trips the derived `<t>_author_required`. ✅
- **Negation guards `x <> spock_actor()`** do **NOT** fail safe. `x <> NULL` is
  NULL, not TRUE, so a `spock_refuse` gated on it *never fires* for anonymous.
  The guard flips from "deny non-owners" to "**allow anonymous.**"

The concrete escalation, which the naïve scaffolder itself would generate:

```spock
// add_to_collection, after a blind :actor → spock_actor() swap.
// Anonymous caller (spock_actor() = NULL):
//   not_owner guard:  (owner <> NULL) → NULL  → refusal NOT raised
//   not_saved guard:  EXISTS(owner = NULL)    → false → refusal NOT raised
//   the INSERT writes collection_post(collection, post) — NO actor column,
//   so nothing trips NOT NULL.
// ⇒ an anonymous, header-less request adds ANY post to ANY user's private
//   collection. A real JWT does not fix this — the hole is the NULL branch,
//   a first-class v0 state.
```

**The law:** an actor-consuming fn that must reject anonymous callers declares
it, and the swap is audited per-fn, never applied blindly. The primitive is a
one-line **authenticated-required guard** (grafted from the roles design), which
needs nothing beyond this milestone:

```spock
unchecked sql("SELECT spock_refuse('unauthenticated') WHERE spock_actor() IS NULL")
```

Kind `refused`, 409 (RFD 0012 minted-refusal machinery, unchanged). This turns
the silent no-op — the second MED defect (an anonymous
`mark_notifications_read` returns `{rows:[]}` with 200, indistinguishable from
"nothing was unread") — into a loud, honest refusal. A future `policy`/`for
user` marker (v1) can make it declarative; v0 ships the primitive.

### 4.5 The self-vs-object rule

The swap replaces only the parameter that names *the caller* (the SELF side);
parameters that name *another party* (the OBJECT side) stay parameters. The
scaffolder (§6) and any author must respect this or they strip the wrong param:

- `follow(follower, target)` → `follower` becomes `spock_actor()`; **`target`
  stays** a param. `follow(target: user)`.
- `approve_follow_request(requester, target)` → the *approver* is `target`, so
  `target` becomes `spock_actor()`; **`requester` stays**.
- `block_user(blocker, blocked)` → `blocker` → `spock_actor()`; `blocked` stays.

### 4.6 Load-validation deltas — the complete list

- **fn bodies calling `spock_actor()`: no change** (invisible to the `:param`
  rule; polarity/`readonly` check untouched — a read fn may call it, as a read
  fn already calls `spock_refuse`).
- **The anchor:** E-ACT01 / E-ACT02 (§4.1).
- **The consumer:** E-ACT03 (§4.2), enforced by conditional registration.
- **Floor / auto-CRUD: unchanged, deliberately actor-blind** (§5).

That is the whole delta: three anchor/consumer checks, and **nothing** added to
the body-level `:param` rule.

---

## 5. What this deliberately does NOT do (and the honest framing)

The floor — `GET /rest/v1/{table}`, GraphQL table roots, and the auto-CRUD
mutations `insert_<t>_one` / `update_<t>_by_pk` / `delete_<t>_by_pk` — does
**not** route through `func::call` and does **not** see `spock_actor()`. This
is doctrine-aligned (spec §9 "no actor context yet"; W3 makes fn bodies the
*first* consumer, policy the second). But it forces an honest reframing the
panel's winner overclaimed (a MED defect):

> **The seam is preparatory, not protective, in v0.** Making a fn's ownership
> guard *sound* provides **zero v0 security benefit** while a byte-for-byte
> bypass sits next to it. `add_comment` may store `author = spock_actor()` so
> "the client cannot forge authorship" *through the fn* — but an attacker
> ignores the fn and calls `insert_comment_one(object: {author: VICTIM, …})` on
> the open floor. G12's **unsoundness inside the deliberate surface** is what
> retires; the floor stays ungoverned until v1 `policy` re-derives it per actor.

Two consequences for how the RFD (and any future contract) must talk:

- **No "G12 retired" claim.** Say: *G12's unsoundness inside the fn surface is
  retired; the floor remains an ungoverned bypass by decision (spec §9).*
- **A dark-write ledger** (grafted from roles, serving RFD 0004 §7's
  surface-as-data): the contract/introspection should surface, per
  identity-bearing table, a *"⚠ ungoverned floor write — no guard"* row — so the
  real attack surface (the floor bypass) is reviewable as data, not just the
  governed fns. This pre-stages the v1 `role × field × read/write × via` ledger
  and stops the seam from *advertising* a soundness the floor negates.

---

## 6. The unconventional idea, adjudicated

> *"define a table as `user extends auth` and whoever uses `user` as pk or
> unique automatically becomes that user's role-defining table."*

**The anchor instinct is right and is kept** (§4.1) — declaring *which* table is
identity is exactly the missing fact. **The inference is wrong and is killed.**
Three doctrine kills plus empirical undecidability:

- **W1 (no inferred source).** A role/ownership map derived from FK shape *is*
  the contract inferred from structure — "the disease this language treats."
  RFD 0004 §1's escape licenses scaffolding *implementation from a stated
  policy*, not *policy from graph shape*.
- **W2 (confused deputy) / W5 (LLM-writability).** A rule you cannot see is a
  rule you cannot audit or write.
- **It is undecidable on the real dogfood.** The reference graph does not encode
  ownership unambiguously:
  - `post.author` / `comment.author` are **not key members**, so a "PK/unique
    reference" rule *misses the two most obvious owners*.
  - `follow(follower, target)`, `block(blocker, blocked)`,
    `restriction(restricting, restricted)` have **two co-equal user refs** in
    the key — which owns? And inferring the *second* (`block.blocked`) would
    **leak the block-list to the blocked party**.
  - `mention.profile`, `media_tag.tagged`, `report.profile` name the **subject**,
    not the actor — inferring ownership there *inverts* it.
  - `follow_request` "ownership" depends on the **operation** (request vs
    approve), not the row.
  - `collection_post` has **zero** user refs.

**The salvage — the only doctrine-legal home for the instinct** — is an
**authoring-time scaffold** (W1's escape), which *proposes* source the author
commits, never runtime behavior. Grafted from the `extends`-design's classifier,
with a concrete four-way disposition per ownership-candidate edge:

| Disposition | When | Example |
|---|---|---|
| **clean-propose** | one unambiguous self-ref | `post.author` → propose the `spock_actor()` guard |
| **transitive-propose** | ownership one hop away | `media_tag` via `media.post.author` |
| **surface-with-flag** | plausible but risky | flag, don't auto-accept |
| **refuse-with-reason** | accepting would leak or invert | *decline* `block.blocked` and **say in the diff**: "proposing this owner leaks the block-list to the blocked party" |

The narrow slice **v0 can actually ship** (because it needs no `policy` to emit
into) is a lint + rewrite proposal:

- **L-ACTOR** — a fn param typed as the anchor table (`actor:`/`viewer:`/
  `author: user`) compared to an ownership column is flagged: *"this identity is
  client-asserted (G12); it can be forged."*
- The scaffold proposes the §4 rewrite (drop the param, `:actor` →
  `spock_actor()`, respecting the self-vs-object rule §4.5), delivered as a
  **git diff the author reviews and commits** (the `terraform plan` tradition,
  RFD 0004 §5/§7) — "the language proposes; the author confirms."

The full ownership-*policy* scaffolder (proposing `policy`/guards) needs
`policy` to emit into, so it is **v1**. What ships of the unconventional idea in
v0: the *declared* anchor, and the G12 lint + `spock_actor()`-swap proposal.
The inferred role-defining table is killed with no salvage.

---

## 7. The dogfood, before → after

**`delete_comment` — the canonical soundness fix** (§1's example): the param
vanishes, the predicate is identical modulo `:actor` → `spock_actor()`, and the
"author-or-post-owner may delete" rule is finally *true* rather than asserted.

**`home_feed` — a read gains a viewer it structurally lacked:**

```spock
// AFTER — :viewer → spock_actor(); reads carry the actor with zero new plumbing
fn home_feed(before: timestamp?) -> [feed_item] {
  unchecked sql("""
    SELECT p.id AS post, u.username AS author, p.caption, p.published_at,
      (SELECT count(*) FROM "like"   WHERE post = p.id) AS likes,
      (SELECT count(*) FROM comment  WHERE post = p.id AND status='visible') AS comments,
      EXISTS(SELECT 1 FROM "like" WHERE post = p.id AND user = spock_actor()) AS viewer_liked,
      EXISTS(SELECT 1 FROM save   WHERE post = p.id AND user = spock_actor()) AS viewer_saved
    FROM post p JOIN user u ON u.id = p.author
    WHERE (p.author = spock_actor()
           OR EXISTS (SELECT 1 FROM follow WHERE follower = spock_actor() AND target = p.author))
      AND NOT EXISTS (SELECT 1 FROM block
                      WHERE (blocker = spock_actor() AND blocked = p.author)
                         OR (blocker = p.author AND blocked = spock_actor()))
      AND p.archived_at IS NULL
      AND (:before IS NULL OR p.published_at < :before)
    ORDER BY p.published_at DESC, p.id DESC LIMIT 20
  """)
}
```

Anonymous (NULL) sees an empty personalized feed and `false` viewer flags — the
correct anonymous behavior, no error. And the upside the seam unlocks:
`search_profiles` can finally exclude profiles that blocked the searcher;
`post_comments` can show "visible **or** held-and-mine" — reads that were
impossible without a viewer.

**`add_comment` — identity into a stored column, safely:** `:author` →
`spock_actor()` including the written `author` column, so the client can no
longer forge authorship *through the fn*; an anonymous write trips
`comment_author_required`.

**`add_to_collection` — the cautionary tale (§4.4):** this fn must gain the
`unauthenticated` guard, because a blind swap turns it into an anonymous
privilege escalation. It is the proof that the swap is per-fn audited, not
mechanical.

Across the file the swap retires ~8 client-asserted identity params — visible in
the contract *diff* as parameters disappearing (surface-as-data, RFD 0004 §7).

---

## 8. Contract-JSON additions (additive, `#[serde(default)]`)

- **The anchor marker on the identity table** — `Some("auth")` on the anchor,
  absent elsewhere; typed `Option<String>` (not `bool`) for forward-compat with
  v1 bases. Marks which table is the actor so generators, introspection, and the
  future ledger know. `spock_actor()`'s return type needs no field — it is the
  anchor's key type, derivable by any consumer.
- **`reads_actor` (an "is this fn actor-dependent" bit): recommended DEFERRED.**
  The panel proposed it, but the review is right that its *derivation* is
  net-new and the only cheap implementation is a substring scan for
  `spock_actor` — which false-matches a string literal or a comment and
  contradicts the codebase's "no vocabulary scan" ethos
  ([func.rs](../../crates/spock-runtime/src/func.rs)). Ship it only with a
  robust derivation (walk the prepared statement's referenced-function set), or
  not at all. A fragile bit in a *frozen* contract is worse than no bit.

Everything else is additive: fns shed params (shorter `params` list); no
existing field changes shape.

---

## 9. Forward-compat

**Into v1 `policy`/RLS.** `spock_actor()` *is* the substrate `policy`
re-derives over (RFD 0009: same derivation, run per role). The vision's
`role post_author extends user on model::post { check: .author }` compiles to a
predicate comparing `author` to `spock_actor()`. Policy is added *around* the
seam, not in place of it — no rework; pre-state evaluation and barrier-ordering
are policy-engine concerns layered on an inert seam. The dark-write ledger (§5)
pre-stages the per-role surface table.

**Into the GoTrue mirror (signup/login fills the same seam).** When the ~5
endpoints land and `auth.users` becomes a builtin:

- The anchor table FK-ties to `auth.users.id` (the `sub`), or becomes a view
  over it — `auth`-marking is where that tie attaches. Projection semantics to
  specify then: `email` read-only (`@protected`), credential never projected,
  profile columns author-owned.
- `Authorization: Bearer <JWT>` replaces `X-Spock-Actor`; the resolver verifies
  the signature and reads `sub`. **`spock_actor()` and every fn body are
  byte-identical across the swap** — only §4.3's `resolve_actor` changes.
  `~whoami` becomes GoTrue's `GET /user`; the dev header survives as a
  dev-flag-gated override (Hasura-style).
- The `role` claim arrives on the same token → feeds `spock_role()`/`policy`
  (v1). Auth-gated lifecycle states (`banned`, `verified`) arrive *with* the
  transitions that reach them — this slice adds no unreachable states.

Sequencing check (RFD 0009's four rules): **names before bindings** — the seam
names match the `auth.uid()`/`sub` world so they don't churn, and no unused
`role` name ships early; **borrow before build** — the header→claims seam is
Hasura/PostgREST, `spock_actor()` is `auth.uid()`, GoTrue is borrowed later;
**language work is the differentiator** — the differentiator (`policy` as
named/testable/composable guards) is correctly held for v1; **identity needs a
consumer** — this lands exactly at track 7's first consumer.

---

## 10. Deferrals — every one named

1. **Role taxonomy** — `role`, `policy`, `spock_role()` stay reserved/absent (v1).
2. **No unused `role` value on the wire in v0** — a client-facing scalar nothing
   consumes is a name scheduled for meaning (violates names-before-bindings).
   Add it *with* policy.
3. **JWT verification + GoTrue endpoints** — v1. The v0 header is deliberately
   forgeable.
4. **`auth.users` builtin table** — v1. v0's anchor is the app's own table.
5. **Floor / auto-CRUD actor governance** — v1 policy (§5). v0 floor is
   actor-blind by decision.
6. **The ownership-*policy* scaffolder** (proposes `policy`/guards) — v1; needs
   `policy` to emit into. v0 ships only the G12 lint + swap proposal (§6).
7. **Per-row conditional field projection** (owner-sees-email, G12's projection
   half) — v1 (views/policy).
8. **Named waivers / `service_role` bypass** — v1; nothing to bypass until
   policy exists.
9. **Composite-key anchor / tuple actor** — rejected outright (E-ACT02), not
   deferred.
10. **`reads_actor` contract bit** — deferred pending a robust derivation (§8).
11. **OAuth/OIDC, multi-actor act-as, auth-gated lifecycle states** — v1+.

---

## 11. Open questions (for ratification)

**A. The anchor keyword — `auth table user {}` (recommended) vs the originally
proposed `table user extends auth`.** §4.1 argues hard for the prefix modifier
(no inheritance mis-signal, `extends` kept free for v1 roles, Spock's own
`mut fn` shape). The counter-argument for `extends`: it is familiar and reads
naturally. This reverses a spelling the brief proposed, so it is explicitly the
author's call. A third option — `auth user { … }` (dropping `table`, since the
anchor is always a table) — is more novel but loses the "it's a normal table,
just tagged" read. **Recommendation: `auth table`.**

**B. Ship the `unauthenticated` guard as a raw idiom, or as a marker?** §4.4's
one-line `spock_refuse('unauthenticated') WHERE spock_actor() IS NULL` needs no
new surface and ships today. A declarative `for user` / `requires actor` marker
is cleaner but edges toward the role layer (W4). **Recommendation: raw idiom in
this slice; revisit the marker with `policy`.**

**C. Roadmap placement.** The plan of record (RFD 0009 §3) sequences full auth
*after* the filter RFD. This slice is far lighter than full auth — one builtin,
one marker, one header — and it is what makes every `fn` guard the dogfood
already wrote actually *true*. It could land before the filter RFD (as fn v2 and
the value tier did, on dogfood evidence) or stay after. **Recommendation:
decide against the four ordering rules, not the sequence** — the honest read is
that the seam is *preparatory* (§5), so its urgency is lower than the filter
layer's, which unblocks real read/write breadth. Likely: **after** the filter
RFD, still before v1 policy.

**D. Secrecy tables on the open floor** (raised by the RLS cross-check, §13.4).
`block` / `restriction` / `follow_request` are the one family the harness cannot
even *partly* rescue in v0: a table whose row's *existence* is the secret leaks
wholesale on `GET /rest/v1/block`, and no fn un-leaks it. Options: (a) accept it
as the ungoverned-tier decision (spec §9) and ship a **loud lint** naming each
secret-bearing floor table; (b) add a minimal **not-floor-exposed** table marker
(deny-by-default reads for sensitive tables) — small and W1-aligned, but it
edges toward the v1 governance flip. **Recommendation: the lint now; the marker
weighed with `policy`.** This is the only point where the cross-check pushes back
on "the floor stays fully open by decision."

---

## 12. What ships, in one paragraph

One keyword (`auth table`), one DIRECTONLY builtin (`spock_actor()`, NULL when
anonymous, registered only when an anchor exists), one runtime-minted default
(`= me`, the write/population half — §14: a strong Hasura-style preset removed
from the client insert/update surface, so the floor auto-stamps the actor and no
client can forge it), one forgeable dev header (`X-Spock-Actor`) resolved
anchor-key-type-aware against committed seed personas, two `~`-endpoints
(`/~whoami`, `/~personas`), a per-request actor threaded into `func::call` and
the floor insert, the anchor/consumer checks (E-ACT01–03) plus the E016 `me`
carve-out and E022 seed tightening, additive contract markers, and one
authoring-time G12 lint/scaffold. Zero new fn-body validation rules. The
prototype can be played as maya, luis, noor, or anonymous; identity columns are
server-stamped and unforgeable on the floor via `= me`; every ownership guard
inside the deliberate fn surface becomes *sound*; and the rest of the floor
stays an ungoverned bypass **by decision**, surfaced as data, until v1 `policy`
governs it. The reference-graph "infer the roles" instinct lives on only where
doctrine allows it — as a diff the author commits, never a rule the runtime
guesses.

---

## 13. The RLS cross-check (second harness)

The adversarial review (§2) was the first harness. The second is differential
(the RFD 0005 method — Postgres as the oracle): write the same product as a
**Supabase-native RLS schema** — where auth *is* ready and RLS *is* the
authorization layer — and cross-check every policy against this seam. The oracle
is [`examples/instagram/pg-rls.sql`](../../examples/instagram/pg-rls.sql): 59
policies over 22 tables plus a small enterprise RBAC addendum
(`organization` / `membership(role)` / `org_setting`), hardened by a Postgres
pedant (anti-recursion `SECURITY DEFINER` helpers in a non-exposed `private`
schema; per-command `USING`/`WITH CHECK`; the admin→owner escalation closed).

**Why this oracle, and what it measures.** The two Postgres *styles* map onto
Spock's two *surfaces*. The existing `examples/instagram/pg.sql` — "an API of
`SECURITY DEFINER` functions, RLS as defense in depth" — is Spock's **deliberate
fn surface** (the guard lives in the function). Supabase's "RLS on
directly-hit tables" is Spock's **floor + v1 `policy`** (the guard lives in
per-row policy on the table the client hits). The actor seam sits between them:
it makes the *function-style* guard sound (`spock_actor()` = `auth.uid()`), and
leaves the *RLS-style* floor governance to v1. So cross-checking against the
Supabase file measures exactly how much of "RLS-as-authorization" the v0 seam
reaches. Every policy was judged on **two axes**: (A) *expressibility* — can the
seam state the predicate with `spock_actor()` + SQL in a fn? (B) *enforceability
in v0* — is it actually enforced, given the floor stays an open, actor-blind
bypass? Each verdict was then adversarially verified (rescue-a-CANNOT /
demote-a-FITS).

### 13.1 The result — the boundary is *one gap*

| | count | reading |
|---|---:|---|
| **Expressible** with `spock_actor()`+SQL | **58 / 59** | the seam's predicate vocabulary is already sufficient |
| **Enforceable in v0** | **4 / 59** | almost nothing is *enforced*, because the floor is open |
| — of which need only the floor closed (`floor-governance` gap) | **54** | one gap, not many |
| — need a capability beyond the seam (`jwt-claims`/roles) | **1** | the sole true capability gap |

Verdicts (post-verify, 0 flipped): **FITS 2 · TRICK 54 · CANNOT 3.** The TRICK
bucket splits 18 reads / 36 writes.

The single most telling line: **the only two policies that FIT are the two that
need no governance at all** — the `USING true` public reads (`location`,
`hashtag`), where the open floor *is* the policy. Everything that actually
*governs* is TRICK or worse. That is §5's "preparatory, not protective," now
measured: of 59 real policies, the seam soundly enforces the 2 that ask for
nothing.

### 13.2 The three piles

- **FITS (2)** — `location_read`, `hashtag_read`: `USING true`. `spock_actor()`
  is never even invoked; `GET /rest/v1/location` already *is* `TO anon USING
  true`. The floor being open is *correct* here.

- **TRICK (54) — expressible, but the open floor leaks.** Two shapes:
  - *Writes (36).* The `:actor` → `spock_actor()` swap makes the sanctioned fn
    path sound (`INSERT INTO post(author,…) VALUES(spock_actor(),…)`), NULL-safe
    against anonymous. **But** `insert_post_one(object:{author: VICTIM})` on the
    floor forges authorship regardless — the §5 archetype, confirmed policy by
    policy. The fn is sound; the floor next to it is the hole.
  - *Reads (18).* A read fn carries the viewer and even preserves Postgres's
    *forbidden ≡ not_found* (a blocked viewer's `profile(username)` returns
    `not_found`, no existence leak). **But** `GET /rest/v1/post` /
    `/rest/v1/follow` still serve archived posts, private-authored posts, and the
    entire follow graph actor-blind. The fn adds a governed *view*; it cannot
    subtract the floor's ungoverned one.

- **CANNOT (3) — genuinely out of v0's reach**, and instructively of two kinds:
  - `block_read`, `restriction_read` — **secrecy reads.** A read fn *can* state
    "my blocks" (`WHERE blocker = spock_actor()`), but it is a **non-solution**:
    the whole content of the policy is secrecy, and the `block` table sits on the
    open floor, so `GET /rest/v1/block` publishes every block to everyone. When
    the leak *is* the vulnerability, no fn rescues it — it needs floor read-
    governance (v1). This is the one place the harness argues a v0 slice is not
    merely "preparatory" but actively *misleading* if shipped alone (see 13.4).
  - `report_read_moderator` — the **only capability gap**: it reads a signed JWT
    claim (`auth.jwt() -> app_metadata ->> role = 'moderator'`). The seam exposes
    exactly one identity fact — `spock_actor()`; there is no `auth.jwt()`, no
    `spock_role()`, no claims map in v0 (W4). Not expressible at all — v1.

### 13.3 What it validates about the seam

1. **§5, quantified.** 54 of 59 policies fail on a *single* axis — the open
   floor — not on expressibility. The seam is preparatory, not protective, and
   now we know the exact ratio.
2. **§9 forward-compat, strongly.** 58/59 expressible means the seam is the
   **right substrate**: the `spock_actor()` predicates authors write in fns
   *today* are the same predicates v1 `policy` will hoist onto the floor. v1
   policy is not a redesign — it is "apply these predicates to the floor" plus a
   claims/role seam. The cross-check is direct evidence for §9's "`spock_actor()`
   *is* the substrate `policy` re-derives over."
3. **The v1 scope, sharpened.** Beyond floor-governance there is exactly **one**
   missing capability: roles / JWT claims (1 of 59). Everything else v1 policy
   needs, the actor seam already carries.

### 13.4 What it surfaces as new design input

- **A named visibility predicate fn (cheap v0 win).** The verifier flagged that
  `can_view_post` is re-inlined across six read policies (`post`, `media`,
  `like`, `comment`, `mention`, `post_hashtag`). A shared, checker-owned
  `fn can_view_post(post) -> bool` — the RFD 0013 validator-fn / named-callable-
  predicate shape — would DRY the dogfood, is client-pre-flightable, and composes
  into every read. This is the "graft named predicates from the roles design"
  idea (§2 grafts), now independently rediscovered with evidence. Worth adopting
  *with* or before this slice.
- **Secrecy tables want off the open floor.** The `block` / `restriction` /
  `follow_request` CANNOTs are the sharpest finding: for a table whose *existence*
  is the secret, an fn cannot un-leak what the floor publishes. This is a
  concrete argument that even the v0 slice may need a way to mark a table
  **not-floor-exposed** (deny-by-default reads for sensitive tables) — a small,
  doctrine-aligned (W1) addition — or at minimum a **loud lint**: "table `block`
  is on the open floor and its rows are a secret." Recorded as a candidate, not
  folded into the slice (it edges toward the governance flip); it is the one
  place the harness pushes back on "floor stays fully open by decision."
- **Confirmations.** The `unauthenticated` guard (§4.4) is exercised by the
  write policies (anonymous inserts fail safe via NOT NULL or the guard);
  *forbidden ≡ not_found* (Postgres P27) is preserved by the read-fn pattern.
  Both design choices survived the cross-check.

### 13.5 Bottom line

The RLS harness **validates the seam as the correct substrate and refutes it as
an authorization mechanism** — exactly the honest split §5 claims. 58/59 policies
are sayable with `spock_actor()`; 54 are unenforceable only because the floor is
open (v1 closes them with the same predicates); 2 fit because they govern
nothing; 3 genuinely cannot — 2 secrecy-reads that need floor read-governance,
and the 1 true capability gap (roles/claims). It also hands v1 its scope on a
plate — floor governance + a claims/role seam — and hands v0 one cheap win (the
named visibility predicate) and one open question (secrecy tables on the open
floor, §11.D).

---

## 14. The population seam — `= me` (the write half)

§4.2's `spock_actor()` is the *read* half of identity: it makes a fn's guards
sound. This section is the *write* half, and it is the more ergonomic one — the
Supabase `author uuid default auth.uid()` move, where data flows *from* identity
automatically. In Spock: **`author: user = me`**. It is a self-contained
primitive — it works, and is worth having, with RLS entirely set aside.

### 14.1 Surface & symmetry — `me` joins `auto`/`now`

A third runtime-minted default keyword, bare, next to `auto` and `now`. The
mapping is clean and total: each default keyword is backed by an engine builtin.

| default | mints | builtin |
|---|---|---|
| `auto` | a UUIDv7 | `spock_uuid()` |
| `now` | the UTC instant | `spock_now()` |
| **`me`** | **the current actor's key** | **`spock_actor()`** |

`me` is legal only on a field that is a **reference to the `auth`-anchored actor
table** (`author: user = me` where `user` is `auth`-marked, §4.1); on a non-actor
field, or with no anchor declared, it is a compile error. One real obstacle: the
checker rejects *any* default on a reference field today (E016,
[check.rs](../../crates/spock-lang/src/check.rs)) — E016 must gain a carve-out
for exactly this case (`me` on an anchor-typed ref). That carve-out is the only
non-mechanical part of the change.

### 14.2 Strong, not weak — `me` leaves the client's write surface

The decisive design choice, and the peer survey is unanimous on it. Two forms
were on the table:

- **Weak** (the `auto`/`now` shape): populate-when-absent, but *client-
  overridable* — a client may still send `author: VICTIM`. This is Supabase's
  `DEFAULT auth.uid()`, which is only safe **paired** with a `WITH CHECK (author
  = auth.uid())` RLS policy. Spock has no such pairing in v0.
- **Strong** (Hasura's "column preset from a session variable"): the field is
  *removed from the client write surface entirely* — absent from
  `<t>_insert_input` **and** `<t>_set_input` — so a client cannot name it at all.

**Recommendation: the strong form.** Every system that *can* structurally remove
an identity column from client input does so; every system that instead defaults
it treats the default as one half of a mandatory two-piece mechanism, and ships
the other half (a CHECK/policy) with it. Spock has no near-term other half (that
is v1 `policy`), but it *does* have the generated-schema machinery to remove the
field cheaply and correctly today. So `me` is a **preset, not a default** — and
that is the one piece of §5's "the floor stays open by decision" that does **not
have to stay open**: a client cannot forge a `me` column on the floor, because
the column is not in the floor's input.

The asymmetry with `auto`/`now` (which stay in the input, overridable) is
principled, not arbitrary: a client may legitimately supply an id or a timestamp
(an import, a backfill); a client *asserting identity* is forgery, always.
Mechanically, `insert_input_type` / `set_input_type`
([graphql.rs:352](../../crates/spock-runtime/src/graphql.rs)) skip
`me`-default fields, and async-graphql's schema validation then rejects any
object carrying one as an unknown input field **before the resolver runs**
(verified against async-graphql v7 on both literal and variable paths).

### 14.3 Mechanism — runtime-materialized, DIRECTONLY-safe, no drift

`me` is materialized on the **write path** in Rust, exactly where the floor
already mints `auto`→`new_uuid()` and `now`→`now_utc()`
([write.rs:39](../../crates/spock-runtime/src/write.rs)) — the arm gains
`Some(DefaultValue::Actor) => …the request actor…`. It is **not** a DDL `DEFAULT`
clause, because `spock_actor()` is DIRECTONLY (§4.2) and cannot appear in one.

A pleasant structural consequence falls out: **the floor never *calls* the
DIRECTONLY builtin at all** — it passes a pre-resolved Rust value into the
insert — so it sidesteps by construction the confused-deputy hazard DIRECTONLY
exists to prevent. The floor becomes actor-*aware* without ever *evaluating*
`spock_actor()`. And there is one source, so **no drift**: the floor
materializes the request actor in Rust; an escape's `INSERT … VALUES(spock_actor(),
…)` evaluates the builtin, which `func::call` re-binds to the *same* request
actor (§4.3's one resolver). Both read one value.

**The wiring must be per-request — this is the trap the stress pass caught.**
The actor value has to be injected *per request*, via async-graphql's
`schema.execute(request.data(actor))` (read back with `ctx.data::<Actor>()`),
with `graphql_post` gaining the same `X-Spock-Actor` `HeaderMap` extractor §4.3
adds. It must **not** ride the schema-global `.data(Arc<App>)` channel
([graphql.rs:285](../../crates/spock-runtime/src/graphql.rs)) — that is set once
at startup and shared by every request, so a request with no header would read
the *previous* caller's actor and stamp their identity: the exact §4.2 cross-
request confused deputy, reintroduced on the newly actor-aware floor. Per-request
`Request::data`, never schema-builder `.data`, for the actor.

### 14.4 Every path, and the two corrections the stress pass forced

- **Floor insert.** The client omits the `me` column (it isn't in the input);
  the runtime stamps the request actor. `insert_post_one(object: {caption})`
  yields a post authored by the caller.
- **Anonymous → the derived `required` error, NOT a 500** (corrected HIGH). A
  required `me` column with no actor must route to `<t>_<field>_required` (422).
  The naïve arm (materialize `NULL` → hit engine `NOT NULL`) instead surfaces a
  raw `SQLITE_CONSTRAINT_NOTNULL` that the floor's `map_conflict_error` does not
  translate → **HTTP 500**, while the *escape* path's `map_fn_engine_error` maps
  the same `NOT NULL` to the 422 — a real floor-vs-escape drift, refuting a
  first-draft "no drift" claim. Fix: when the resolved actor is `None` and the
  field is non-optional, `write.rs` emits the derived `required` error directly
  (like its existing required path), never pushing `NULL`. Net effect: an
  anonymous, header-less floor insert of an owned row **cannot happen** — it must
  assert an identity. (Fail-safe, aligning §4.4.)
- **Seed — anonymous at seed time, must name `me` explicitly** (E022 tightening).
  Seed runs before the listener binds, so the actor is `NULL` through replay, and
  seed enters `write.rs` *directly* (bypassing the GraphQL input filter) — so,
  unlike a client, it *may and must* name the `me` column to author fixtures.
  E022 must treat an `Actor` default as *no default for seed purposes* so the
  checker **demands the column in every seed row at load time**, rather than
  aborting at replay on `NOT NULL`. This is the same authoring-tier bypass every
  preset system has (Hasura admin, Postgres `service_role`) — named, not
  invented.
- **Updates — stamped once, immutable.** Defaults never re-apply on update, so an
  omitted `me` column keeps its author for free; and removing it from
  `set_input` (§14.2) stops a later editor from *re-stamping* ownership to
  themselves. **Guard the degenerate case** (corrected HIGH): `has_settable_fields`
  ([graphql.rs:301](../../crates/spock-runtime/src/graphql.rs)) does not inspect
  defaults, so a table whose only non-key field is `= me` would generate an
  **empty `_set` input object** — invalid GraphQL, the server fails to boot.
  `has_settable_fields` (and the insert-input emptiness guard) must exclude
  `Actor`-default fields.
- **Escapes name `spock_actor()`.** With no DDL DEFAULT for `me`, an escape that
  omits the column gets no auto-fill; the author writes `spock_actor()` (the §5.3
  pattern) — same value, no drift.
- **The actor must exist as an anchor row.** The floor `me` stamp is ref-checked
  like any reference, so a header naming a key absent from the anchor table
  (`~whoami`'s `known: false`) yields `ref_not_found`, not a dangling author.
  Correct: you cannot author as a user who does not exist.

### 14.5 The dogfood, before → after

```spock
// BEFORE — author is client-supplied; insert_post_one(object:{author: VICTIM})
// forges authorship on the open floor.
table post { key id: uuid = auto  author: user  caption: text?  … }

// AFTER — author leaves post_insert_input; the floor auto-stamps the caller,
// and no client can name it. The ~8 fns that threaded `author:`/`actor:` shed
// the param (§5) AND the floor stops accepting a forged author for free.
table post { key id: uuid = auto  author: user = me  caption: text?  … }
```

`comment.author`, `collection.owner`, `save.user`, `like.user`,
`report.reporter`, `media_tag.tagged` (the tagged-self case), `block.blocker`,
`follow.follower` are the same shape — the "self" side of each. (The "object"
side — `follow.target`, `block.blocked` — stays a normal client field; §4.5's
self-vs-object rule applies to the write seam too, and a checker guard should
keep `= me` to the self side.)

### 14.6 The stance shift, and its exact bound

This makes the floor **actor-aware for population** — a real change from §5/§6's
"actor-blind floor." It is doctrine-safe because population is **provenance, not
governance**: `= me` makes no allow/deny decision, applies no predicate, filters
no row — it fills a column. W3 orders identity's *governance* consumers (fn body,
then `policy`); population is a third kind, orthogonal to that ordering, so it
does not front-run `policy`.

But state the win at its true size, no larger: `= me` **closes the floor-forge
for the identity column it marks** — a bounded, real gain (an attacker can no
longer set `author` on `insert_post_one`). It does **not** make the floor safe:
every *other* floor write and every floor read stays ungoverned (that is still
v1). "Identity columns become server-stamped and unforgeable on the floor" is the
honest headline — not "G12 is closed."

### 14.7 Verdict & open questions

Is `= me` "flawless in isolation," as asked? The *primitive* is — a strong,
server-owned, runtime-materialized, immutable, DIRECTONLY-safe actor stamp is a
clean self-contained concept. But the first draft was **not** flawless: the
stress pass found three HIGH implementation-shape holes — the schema-global
wiring reintroducing cross-request bleed (§14.3), the anonymous floor insert
500-ing instead of 422-ing (§14.4), and the empty-input-object boot failure
(§14.4). With those three fixed, it is. That is the harness working as intended.

Open questions:

**E. Should `= me` look different from `auto`/`now`?** The peer survey flags a
real trap: `= me` sits grammatically in the `auto`/`now` family (which are weak
and overridable) but behaves like a Hasura preset (strong, non-settable). That is
the same "the token mis-teaches" hazard §4.1 used to reject `extends` — an author
may assume override semantics transfer. Options: accept it with crisp load-time
messaging, or give the preset a distinct marker. **Recommendation: keep `= me`
(the ergonomics win of looking like a default is large) but have the checker
state the non-settable semantics wherever it is declared.** The user's `me()`
call-spelling vs the bare `me` (matching `auto`/`now`) is the sub-question here.

**F. At most one `= me` per table, and self-side only?** A checker guard (the
write-seam analog of §4.5) that keeps `= me` on the ownership/self column and
flags a second one avoids nonsensical double-stamping. Lean: yes, a lint.

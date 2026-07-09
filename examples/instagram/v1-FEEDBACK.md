# v1.spock — feedback

A review of `v1.spock` against two oracles: the PRD, and `pg.sql` — the
answer sheet, a verified plain-PostgreSQL implementation of the same PRD.
The question here is **logical and feature fidelity only**: what does the
contract fail to say, say wrongly, or have no syntax to say?

The answer sheet also embodies a second column of knowledge — indexing
strategy, trigram search, statement-level triggers, fillfactor, queue
claiming with `SKIP LOCKED`, advisory-locked maintenance, unlogged rate
tables, RLS as defense in depth, pooling, feed fan-out mechanics. None of
that is reviewed here, deliberately: it is exactly the layer a prototype
language exists to *not* say. Where such a pattern has a logical shadow
(partial uniqueness, upsert conflict targets), the shadow appears below;
the pattern itself stays in `pg.sql`.

Findings are numbered: **B** = v1 states something wrong, **F** = the
product needs it and v1 lacks it, **L** = v1 *could not have said it* —
a language gap. Each ends with the v2 disposition.

---

## B — Bugs: where v1 is wrong, not incomplete

**B1 · A denied follow request can never be re-requested — and the caller
is told it was.**
`fn follow` does `upsert follow_request { requester, target, status: pending }`
(v1.spock:782). Under v1's own upsert semantics (keyed insert-or-ignore,
INVENT(2)), an existing `denied` row makes this a no-op — the row stays
denied — and the fn still returns `{ outcome: requested }`. A silent lie.
`pg.sql` needed a *conditional* conflict update (`do update … where
status = 'denied'`) plus an explicit `denied>pending` edge, and it
re-notifies the target on re-request. v1's `state request_status` has no
such edge, so even a correct upsert would be rejected by the machine.
→ v2: add the `denied -> pending` edge; see L2 for the upsert form.

**B2 · A removed media tag can never be re-applied.**
`media_tag` declares plain `unique (media, tagged)` (v1.spock:292) with a
`removed_at` soft-removal. Once a user removes themselves, any re-tag
violates the unique — or, if `upsert` is read generously, silently
resurrects the removed row, erasing the removal history. `pg.sql` needed
uniqueness *over active rows only* (one live tag per person per media,
removed rows kept as history). Same shape as the username-reuse question
v1 deliberately deferred (R15) — the answer sheet shows conditional
uniqueness is not an exotic future need; this PRD needs it twice.
→ v2: blocked on L1.

**B3 · Moderator disable is conflated with self-deactivation.**
Both `disable_account` and `moderate_target(profile, removed)` transition
to `deactivated` (v1.spock:755, 1099, 1139) — the same state a user
enters voluntarily. The two differ in the one place that matters: what
sign-in does next. Self-deactivation reactivates on sign-in;
a moderation disable must refuse. `pg.sql` needed two distinct states
(`deactivated` vs `disabled`) with different sign-in edges.
→ v2: split the state; the sign-in edge itself is L3.

**B4 · Self-mentions notify their author.**
`on create mention` guards only on visibility (`can_see_source`). Maya
writing "@maya" in her own caption mentions and then notifies herself.
The like reaction remembers to check (`.post.author != .profile`); the
mention and tag reactions don't. `pg.sql` never has this class of bug
because every notification passes through one chokepoint
(`push_notification`) that centrally drops self-notifications and
blocked pairs. v1 re-states suppression per reaction, ad hoc — scattered
guards are exactly the disease the language claims to treat.
→ v2: see L4 (a single declared suppression rule).

**B5 · Held comments are a dead end.**
A restricted user's comment lands `moderation: held` (v1.spock:1001) —
visible to its author and the post owner, per policy. And then nothing:
no fn lets the post owner approve it. The restriction flow, one of the
PRD's subtler features, is declared and then abandoned half-way.
`pg.sql` has `approve_comment` (owner flips held→visible).
→ v2: add `fn approve_comment` for the post author.

**B6 · Declared states no transition can reach.**
`deactivated -> active` (reactivation) and `deletion_pending -> active`
(grace-window cancel) have edges but no fn fires them; `deleted` is
unreachable (R14 anticipated this one). v1's own totality doctrine calls
these dead transitions — the linter this language promises would reject
its own flagship example. The missing writers are real product flows,
both of which live at the auth boundary (they happen *at sign-in*).
→ v2: blocked on L3. Related: `publish_post` declares `! mention_limit`
but no statement in its body can produce it — a dead error declaration,
same lint family.

---

## F — Missing features (the PRD or the answer sheet has them; v1 doesn't)

**F1 · The deletion endgame does not exist.** Account deletion in v1 is a
one-way transition to `deletion_pending` and a `todo()`. There is no due
timestamp (nothing records *when* the grace window ends), no statement of
what deletion *means* — which fields scrub, what the username becomes,
what survives for audit. `pg.sql` answers all of it: a `deletion_due_at`,
an anonymization pass (identifiers nulled, username renamed to a
tombstone — which *is* the username-reuse policy, stated as code). The
timer is rung 2 and may stay a `todo()`; the *meaning* of deletion is
pure contract, and an executable PRD that cannot say what "delete my
account" means is failing at its one job.
→ v2: add `deletion_due_at`; declare the scrub as a fn the (future)
timer invokes; the tombstone rename answers R15's product half.

**F2 · No per-collection removal.** v1 can save, save-into-collection,
unsave-everything, and delete a collection — but cannot take one post out
of one collection while keeping it saved. (`pg.sql`:
`remove_from_collection`.)
→ v2: add the fn; one keyed delete.

**F3 · Own posts are absent from the home feed.** v1's feed is strictly
"posts of accounts I follow"; the product (and `pg.sql`) includes the
viewer's own posts.
→ v2: `or post.author == actor.profile` in the feed membership.

**F4 · No unread-notifications count.** The badge every social client
renders. Expressible today (`derived unread_count = count of
notification.recipient where .read_at absent` on `profile`) — just never
written.
→ v2: one derived field.

**F5 · A private profile page cannot say "private".** v1's `profile_page`
shows the shell and gates the grid per-row — so a viewer sees an empty
grid, indistinguishable from "no posts". `pg.sql` returns an explicit
`content_visible` flag so the client can render the lock screen.
→ v2: add `content_visible: can_see_content_of(actor.profile, this)`.

**F6 · Avatars skip the media pipeline.** `avatar: storage::object?` is a
raw object slot, writable through settings. The PRD: "avatar uploads must
be processed before being shown as final." `pg.sql` types the avatar as a
`media_asset` reference, so it inherits the processing state machine.
→ v2: `avatar: media_asset?` (+ ready-state policy on write).

**F7 · Caps and limits are inconsistent.** Comment mentions are capped
(10) but caption mentions aren't; hashtags are uncapped (pg caps 30);
`like` has no rate limit (pg: 300/h). Individually trivial; collectively
the kind of drift a contract language should make hard.
→ v2: fix the instances; L5 asks whether limits belong on the parse
helpers rather than each call site.

**F8 · Moderation has a queue view but no claim.** Two moderators reading
`report_queue` will review the same report. `pg.sql` claims atomically
(`claim_next_report`). For a prototype this is arguably fine — assignment
is a workflow nicety — but the *shape* (claim → decide) is product logic,
not plumbing. → v2: optional; defer with a note.

Shared, deliberate punts — recorded, not owed: notification retraction on
unlike (v1's R7 — `pg.sql` leaves the stale row too), `collection.cover`
is a dead field in both (nothing writes it; v1's own dark-field lint
should flag it), push-delivery workers, video-specific validation.

---

## L — Language gaps: what v1 had no way to say

**L1 · Conditional (partial) uniqueness.** "Unique among live rows" is
needed twice in this one example (B2 re-tagging, F1/R15 username
tombstones) and was deliberately not invented in v1. Promote it:
`unique (media, tagged) where .removed_at absent` or equivalent. The
derived-error story extends naturally (the violation error carries the
condition).

**L2 · `upsert` is underspecified in exactly two ways the example
tripped on.** (a) Which key? — `media_tag` has a surrogate `key id` plus
a semantic `unique`; upsert must name its conflict target. (b) What
happens to non-key fields on conflict? — B1 needs
insert-or-*conditionally-update* ("if denied, back to pending"), which is
neither insert-or-ignore nor blind overwrite. SQL's answer
(`on conflict … do update … where …`) is ugly but complete; v1's verb is
pretty but ambiguous. Decide the semantics before v2 leans on it again.

**L3 · The auth boundary has no events.** Reactivate-on-sign-in,
cancel-deletion-on-sign-in, refuse-when-disabled: all product rules that
fire *when std::auth authenticates someone*. v1 can neither express "on
sign-in, transition status" nor let std::auth consult the contract
("may this account get a session?"). Until it can, every account
lifecycle state that changes at login is unreachable by construction
(B6). Sketch for discussion: `on std::auth::session(account) { … }` — a
reaction on a builtin event — or a declared `policy may_sign_in(account)`
that std::auth is contracted to honor, plus a transition hook.

**L4 · Notification suppression wants one rule, not N guards.** The
`pg.sql` chokepoint (never notify yourself; never notify across a block;
drop silently) is a *global invariant of the notification concept*, not
per-event logic. v1 restates fragments of it per reaction and misses one
(B4). Language answer: either a `notify` verb with the invariant built
in, or a declarable guard on the table ("every insert into notification
respects …") — the latter generalizes.

**L5 · Where do write-path caps live?** Mention caps, hashtag caps, rate
limits — v1 scatters them across call sites (F7). The natural home is
the format/parse layer (`std::text::mentions` capped by contract) or the
table ("at most 10 mention rows per source"). Table-level cardinality
constraints ("at most N children per parent") would also subsume the
carousel's 1..10, currently a bounded-list type on one fn's signature.

**L6 · Protocol defaults, not per-view boilerplate.** v1 puts
`limit 20 per request` on two search views and nothing anywhere else;
`pg.sql` caps every read path (`least(p_page, 50)`). Page-size ceilings
should be a protocol-level default a view may tighten — otherwise every
view carries boilerplate or silently allows unbounded reads.

**L7 · Confirmations from the answer sheet, no action needed.**
R3 (optionals in composite keys): `pg.sql` avoided it with a second table
and composite FKs — both designs work; v1's is defensible, keep it.
R10 (rate-limit "per what"): resolved — every limited verb requires a
signed-in actor, so per-actor is well-defined. R12 (strict `expect` vs
eventually-consistent counters): `pg.sql` lives the same tension (its
counters converge and are reconciled, not transactional) — the tension is
real and stays open. Counters themselves: v1's derived counters are the
stronger model (drift is unrepresentable; `pg.sql` needs triggers plus a
repair job — the repair job is the confession).

---

## Where v1 is ahead

For balance: the contract states things the answer sheet cannot.
Machine-checked expectations including negative leak proofs (`expect rex
cannot read …`) — `pg.sql`'s equivalent is a hand-written smoke test.
Seeds as played walks through the contract. Derived counters with no
repair-job caveat. One write-through settings view spanning two tables
(`pg.sql` needs two views, one per table, by SQL's updatable-view rules).
Errors as declared outcomes in signatures rather than SQLSTATE mapping
conventions. And per-collection listing (`collection_posts`), which the
answer sheet forgot — answer sheets have bugs too.

---

## v2 worklist (proposed order)

1. Decide L2 (upsert semantics) and L1 (conditional uniqueness) — both
   are blocking bug fixes.
2. Fix B1, B2 with them; add the `denied -> pending` edge.
3. Split `disabled` from `deactivated` (B3); decide L3's shape enough to
   make B6's edges reachable, even if the mechanism is a marked stub.
4. Add F1's deletion contract (due timestamp + scrub fn + tombstone).
5. One-liners: F2, F3, F4, F5, F6, B5, F7's instances.
6. Adopt L4's suppression rule; sweep the reactions.
7. L5/L6 are design discussions — RFD material, not v2 edits.

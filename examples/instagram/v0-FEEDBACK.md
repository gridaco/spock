# v0.spock — feedback

Where `v1-FEEDBACK.md` reviewed a *design draft* against the PRD, this file
reviews the **shipped language**: `v0.spock` is everything the implemented
v0 surface (docs/spec/v0.md) can honestly say about the PRD, verified by
`spock run` — every escape body engine-validated at load, every flow below
exercised live. The oracles are the PRD, `pg.sql` (the answer sheet), and
`v1.spock` (the aspiration). The question: what could v0 not say, what
could it say only through a workaround, and what did dogfooding confirm?

The file grew from 5 tables / 3 fns to **19 tables, 4 records, 37 fns**
(5 records at review time; `record unread` died when G9 was taken) —
and the growth was mostly friction-free where the table tier is concerned.
The walls are concentrated in one place: the fn body. That concentration
is itself the finding.

Findings are numbered: **G** = language gap (no way to say it), **W** =
workaround (sayable, but the saying is evidence), **C** = confirmation
(a v0 design dogfooding validated). Each ends with a v0.x disposition.
Cross-references like v1-L1 point into `v1-FEEDBACK.md`.

**Status.** The decision-free table-tier items shipped days after this
review and the example now uses them — G4 (engine builtins + DDL
defaults), G7's set-null half, G9 (scalar returns), G10 (float) are
marked *taken* below, and W3 dissolved with G4 as predicted. **fn v2
followed (RFD 0012)**: G2, G3, and G11 are *taken* — the example now
mints its refusals, spans tables, and files its reads under Query — W1
softened with them, and the upgrade surfaced one new wall (G17:
statements cannot pass values). Pagination (G16) was *deliberately
deferred* to the universal query layer by decision, not omission.
**The value tier followed (RFD 0013)**: G1 is *taken* — the six
closed-set columns are literal-union types (checker-owned: seed/default
checked, TS unions), and the column-validation half is `check`
validator fns — so seven fn-guard refusals (`invalid_username`,
`empty_body`, `body_too_long`, `empty_reason`, `self_follow`×2,
`self_block`, `self_restrict`) retired into table constraints the floor
enforces too (G13's leak, closed for these — see G13).

---

## G — Gaps: what v0 has no way to say

**G1 · Closed text sets.** Six columns in one example hold a closed
vocabulary as open `text`: `media.kind`, `media.status`,
`follow_request.status`, `comment.status`, `report.status`,
`notification.kind`. The legal values live in comments; the floor accepts
`status: "pnding"`; the TS artifact says `string`; GraphQL says `String`.
v1's `state` machines are the ceiling, but the floor of this need is just
an enum — and it extends everything that already works: the checker
rejects bad seed literals, insert_input un-shadows to a union type,
a new derived error names the violation.
→ v0.x: a closed-set text type (`text in ("pending" | "ready" | …)` or
similar) — smallest possible RFD; transitions stay v1.
*Taken* (RFD 0013), and simpler than the sketch: the type **is** the
literal union — `status: "pending" | "ready" | "failed"`, TypeScript's
own syntax, no new keyword. The checker owns it end to end (bad seed
literals are E023, bad defaults E009, the TS artifact emits the union),
and a violation is the derived `<table>_<field>_invalid` code, kind
`invalid`, 422. The column-validation half (username charset/length,
body/reason non-empty, the 0.0–1.0 media_tag range, self-pair
distinctness) is its sibling: a `check` naming a validator fn.

**G2 · A fn cannot name its refusals.** The `!` clause vocabulary is
derived codes ∪ the reserved five (E039), and — precisely — there is no
way to surface a code that isn't backed by a real constraint violation.
Escape bodies *can* surface derived codes when a constraint genuinely
fires (that's C3, and it is how `rename_user ! user_username_taken` is
fulfilled); what they cannot do is name a *guard*: a product rule
expressed as a row filter surfaces only as the arity miss. Live
evidence: `follow` refuses self-follow, private targets, and blocked
pairs, and all three return the same envelope —
`{"error":{"code":"not_found","kind":"not_found","table":null,
"fields":[],"message":"fn `follow`: the SQL matched no row"}}`;
`add_comment` folds five guards into the same shape. Distinct product
rules, zero distinct names. (An author could even engineer a fake
violation — CASE a guard into a NOT NULL column — to borrow a derived
name that semantically lies; that such a trick is *almost* attractive is
the measure of the missing channel.) This is the LINQ lesson inverted:
the contract half is now less expressive than the escape half.
→ v0.x: fn-declared error codes plus a raise channel from the body
(design needed — a reserved error-column convention, a `CASE`-to-code
mapping, whatever survives prepare()-validation). Highest-leverage
single feature for fn honesty; pairs with G3 as one RFD.
*Taken* (RFD 0012). The `!` clause mints: a code that is neither
derived nor reserved is the fn's own refusal, raised via the engine
builtin `spock_refuse('<code>')` — and routed only if minted by that
fn, so derived codes stay evidence (the fake-violation trick this
entry feared is now structurally dead). `follow`'s refusals carry
their own names. The file minted 15 distinct refusal codes at fn v2;
the value tier (RFD 0013) then retired seven of them into table
`check`s (see Status), leaving 8 — the cross-table/state rules
(`blocked`, `account_private`, `request_denied`, …) a row-local CHECK
cannot see, which is exactly the boundary between the two mechanisms.

**G3 · One statement means one table.** The hardest wall, hit four ways:

- `approve_follow_request` is **broken by construction**: approving must
  flip the request *and* create the follow row. One statement writes one
  table, so the fn flips the request and stops. The *deliberate* surface
  cannot repair it — `follow` refuses private targets, correctly. The
  only repair is the raw floor (`insert_follow_one`), which bypasses
  every guard this file wrote: the flow's logic leaks to the client, and
  what makes the workaround possible is exactly the floor's inability to
  filter (G13). Within the surface the contract *means*, an approved
  request never becomes a follow (v1-B6's lint family, now forced by the
  language rather than authored).
- `block_user` cannot sever the existing follow edges (PRD requires it).
- `like_post` / `add_comment` cannot write the notification the PRD says
  the act produces — `notification` is a table **no fn can write** next
  to the act it describes; its rows come from the seed.
- `unsave_post` cannot also clear `collection_post`, so unsaving leaves
  stale collection entries.

Note what the wall is *not*: complexity. W2 shows one statement carrying
real policy. The wall is strictly table-count.
→ v0.x: multi-statement escape bodies in the existing one-transaction
envelope — each statement still individually prepare()-validated, params
checked across the set, the *last* statement's shape is the return.
Same RFD as G2; together they unbreak every case above.
*Taken* (RFD 0012), exactly as proposed. All four cases repaired and
verified live: `approve_follow_request` flips, creates the edge, and
notifies in one transaction; `block_user` severs follows and pending
requests; `like_post`/`add_comment` write their notifications (held
comments stay silent); `unsave_post` clears collection entries. One
successor wall discovered in the repair: G17.

**G4 · Declared defaults don't reach the escape.** The DDL carries no
DEFAULT clauses — `= auto` and `= now` live in the runtime's write path,
which escape bodies bypass. Consequences, all visible in the file: every
INSERT hand-writes `strftime('%Y-%m-%dT%H:%M:%fZ','now')`; `add_comment`
hand-rolls a UUIDv4 from `randomblob()` in four lines of line noise
because SQLite has no uuid function and the table's own `= auto` is
unreachable — and `report_post` copy-pastes the same four lines, which
is how this disease spreads. The seams show at runtime: engine-written
rows carry v7 ids (`019f4c43-…`) and sub-µs timestamps, fn-written rows
carry v4 ids (`615a5ebe-…`) and millisecond timestamps.
→ v0.x: emit DEFAULT clauses where SQLite can own them, and register
engine builtins (`spock_uuid()`, `spock_now()`) so a body borrows the
engine's own generators. Cheap, and W3's precision drift dissolves
with it.
*Taken.* Both halves shipped (spec §7.1): the builtins are the write
path's own generators, extracted and shared, and SQLite proved willing
to call them from column DEFAULTs. The hand-rolled v4 is gone from the
example; escape INSERTs simply omit defaulted columns.

**G5 · Exactly-one-of.** `mention`, `report`, and `notification` target
one of several kinds; v0 spells that as N optional refs and cannot say
"exactly one is set." The floor happily accepts a report aimed at nothing
or at three things at once. The PRD's "no unbounded duplicate reports per
reporter and target" is uniqueness *over* that one-of — unsayable twice.
→ v0.x: a `one of (post, comment, profile)` table constraint; the
derived-error story extends. Constraint tier, pairs with G6.

**G6 · Partial uniqueness — third strike.** v1 hit it twice (v1-L1:
re-tagging, username tombstones); this file hits it again in the same
place: `media_tag` wants `unique (media, tagged)` **among live rows**, so
a removed tag blocks any re-tag forever. SQLite has partial indexes; the
feature is not exotic and the example now needs it three times.
→ v0.x: `unique (…) where .field absent` — promote from v1 wishlist to
the table tier.

**G7 · The deletion vocabulary is two words, and both are wrong
somewhere.** v0 says `restrict` or `cascade`; the PRD needs more:

- `comment.parent` — the PRD wants deleted parents to *not* break
  replies. That is `set null` or soft delete; v0 has neither, so the
  example cascades — deleting a comment silently deletes its thread,
  a policy deviation forced by the type system.
- `report` / `notification` — cascade erases the audit trail together
  with the target; restrict makes reported content undeletable. The real
  answer is soft delete (v1-F1's deletion contract), which also owns
  archived-vs-deleted, recovery windows, and username tombstones.

→ v0.x: `on delete set null` is a one-line DDL fact — take it now. Soft
delete is doctrine — RFD alongside the filter work (a soft-deleted row
is precisely a row every surface must filter).
*Half taken.* `set null` shipped (optional refs only, E040):
`comment.parent` now follows the PRD, and reports outlive their targets
with nulled links. The soft-delete half stays an RFD.

**G8 · Records are flat and scalar.** A post-detail page is three calls
(`post` by pk, `post_media`, `post_comments`) because a record can't hold
a list; `feed_item` flattens its author to one column because a record
can't nest. Meanwhile the floor right next door serves nested selection
sets. The fn surface is the one place the graph goes flat.
→ v0.x: needs design, not a quick fix — either record fields of
table/record type (one join level, still prepare-validatable?) or a
blessed fn→floor handoff (fn returns keys; the client hydrates through
the floor's nesting). Interacts with the filter RFD; decide there.

**G9 · A scalar return needs a costume.** `unread_count` returns one
integer, so the file declares `record unread { count: int }` — a named
wire shape whose only job is to dress a number. (Mechanically: `-> int`
dies in the grammar itself — type keywords are not accepted in return
position, L010 — and E037 is the checker error for a return *identifier*
naming no declared table or record. Both paths close the door.)
→ v0.x: allow `-> int` / `-> text` / etc. Trivial grammar + arity
extension; the REST/GraphQL mappings are obvious.
*Taken.* Scalar returns shipped under the unchanged arity scheme (one
result column, any name); `record unread` is deleted and
`unread_count` returns a bare number.

**G10 · No float.** `media_tag` positions are `x_pct/y_pct: int` because
v0 has no floating-point type; the PRD's 0.0–1.0 tag placement is
quantized to percent.
→ v0.x: add `float` (SQLite REAL). Small; the only real decision is the
JSON/GraphQL mapping.
*Taken.* `float` shipped (REAL / JSON number / GraphQL Float / TS
number); tag placement is `x: float` 0.0–1.0 — though the *range* still
has no home until the `format` question (RFD 0009 §4) is decided.

**G11 · Twelve reads live on the Mutation root.** `home_feed`,
`profile`, `search_profiles`, `post_engagement`, `post_media`,
`post_comments`, `hashtag_page`, `location_page`, `saved_posts`,
`tagged_posts`, `notifications`, `unread_count` — twelve of 37 fns are
pure reads, served as GraphQL mutations. Clients won't cache them,
tooling won't parallelize them, and the semantics are simply misfiled.
Known deferral, now with a number attached: a third of the deliberate
surface.
→ v0.x: a signature-level read marker (`fn … query` or similar). It
cannot be inferred from the body — the body is unverifiable by design —
so it must be declared and engine-enforced (reject DML at load: prepare()
already knows `stmt.readonly()`).
*Taken* (RFD 0012), with the polarity inverted from this entry's
sketch: the unmarked fn is the read (the forgotten-marker failure is
then the loud one), `mut` marks the writes, and the engine holds every
statement of an unmarked fn to `stmt.readonly()` at load. The twelve
reads now live on the Query root and answer `GET /rest/v1/rpc/<fn>`.

**G12 · Every actor is client-asserted.** `delete_comment(comment,
actor)` believes whoever names an actor; `post_comments` can't show
held-comments-to-their-author because there is no viewer identity, only
another parameter anyone can set. This is not a discovered gap — it is
the auth track's reason to exist (RFD 0009) — but dogfooding sharpened
its first deliverable: **actor binding in fn signatures**, not sessions
or providers. One trusted parameter unlocks delete-own,
approve-as-owner, held-and-mine, blocked-excluded search.
→ v0.x: confirms the roadmap; auth's first slice is the fn actor.

**G13 · The floor leaks what the fns hide.** `hashtag_page` excludes
archived posts and private authors; `GET /rest/v1/post` serves them all.
Every fn-level guard in this file is a promise the borrowed floor cannot
keep, because the floor cannot filter. Harmless in v0's open tier
(everything is open by decision), fatal the day auth lands.
→ v0.x: ratifies the filter sub-language as the roadmap's next language
work — and says its scope must include *policy* filtering (row
visibility), not just query predicates.
*Partly closed* (RFD 0013): the leak had two kinds. The **value** kind —
a bad username, an empty body, a self-follow — was a guard the floor
ignored; those are now table `check`s and closed-set types the floor
*must* enforce, because they are engine constraints, not fn code. Seven
such guards retired. The **visibility/state** kind — archived-excluded,
blocked-excluded, private-only — is cross-row/cross-table and stays the
filter/policy layer's job (a row-local CHECK cannot see it). So the tier
split the leak cleanly: constraints for value rules, policy for
visibility.

**G14 · Cross-table invariants beyond the FK.** To be precise about
what a ref already buys: `post.author: user` *is* a declared, enforced
cross-table existence invariant — the canonical case is covered. What
has no declaration: correspondence into a composite-key identity ("a
collection entry must also be a save" — refs can't target composite
keys), minimum-child cardinality ("a post needs at least one media"),
and count caps ("at most N mentions per comment", v1-L5). A fn write
guard can *enforce* the first at its own chokepoint —
`add_to_collection` checks the save exists — but nothing declares the
rule, so the floor writes straight past it.
→ v0.x: record only. This is v1 territory (`expect`/policy); don't
invent it piecemeal.

**G15 · Counters are recounted on every read.** The PRD's own engineering
caveat ("should not be computed by counting all related rows on every
read") is violated by every count() in `home_feed`, `profile`, and
`post_engagement` — the only way v0 can say a count. v1's derived
counters remain the strongest answer in either direction (drift is
unrepresentable; pg.sql needs triggers plus a repair job).
→ v0.x: derived fields RFD when read-perf matters; not before filter
and fn-v2.

**G16 · Every fn read re-solves solved problems, and gets them wrong.**
Adversarial review of this file's *first draft* found three read-path
defects in one pass: cursored reads ordered by a non-unique timestamp
with no tiebreaker (boundary ties silently skipped), two discovery pages
with a LIMIT but no cursor at all (posts beyond the newest 30
unreachable), and a search that concatenated user input into LIKE
unescaped (`%`/`_` as a user-controlled wildcard language). All three
are the same finding: pagination discipline, stable ordering, and
pattern escaping are *protocol* problems being hand-solved per fn in an
unverifiable body — v1-L6 (protocol defaults, not per-view boilerplate)
seen from below. Even the fixed file only approximates keyset
pagination: a single `before` cursor with an id tiebreaker still skips
exact-boundary ties; a correct compound cursor is more boilerplate per
fn than the query itself.
→ v0.x: fold into the filter RFD — whatever read sub-language emerges
should own page shape (cursor, ordering, ceiling) the way §8 already
owns it for the floor, leaving the fn author only the predicate.
*Deferred, by decision* (July 2026, recorded in RFD 0012 §3 and RFD
0009 §3): fn v2 deliberately did **not** grow a per-fn cursor
convention. A row-returning fn is conceptually a named filter —
view-shaped — and pagination discipline belongs to the universal
dynamic query layer, applied uniformly to tables, future views, and
read fns. The hand-rolled cursors below stay until that layer lands.

**G17 · Statements cannot pass values.** Discovered *inside* the G3
repair: `add_comment` now writes its notification, but the
notification cannot carry the comment it announces — the new comment's
id exists only in the INSERT's discarded result, and the next
statement has no way to name it. The file links the post instead and
declines the `last_insert_rowid()` trick (real, but a rowid pun on a
uuid-keyed table is exactly the kind of cleverness an unverifiable
body shouldn't host). The plural form of the same wall: mention
parsing wants one insert *per parsed row* — statement count is static,
row fan-out is dynamic. This is the first ask that points at the
native statement grammar (v1.spock's `let`) rather than at more escape
plumbing.
→ v0.x: record only. A binding form between statements is native-body
territory; don't bolt variables onto the escape.

---

## W — Workarounds: sayable, but the saying is evidence

**W1 · The idempotency incantations.** Insert-if-absent-return-the-row
has no verb, so the file writes a no-op upsert — `ON CONFLICT … DO
UPDATE SET at = at` (once `since = since`) — six times, an update whose
entire purpose is smuggling the existing row into RETURNING. The
denied→pending revive in `request_follow` needs a three-branch CASE
*per column*. And the incantation composes badly with guards: a guarded
upsert refuses before the conflict clause can return the existing row,
so `follow` needs a leading already-exists escape — without it, an
existing edge whose target later went private would be refused by its
own guards — while `like_post` omits the escape and documents the
wrinkle instead. All of it works; all of it reads as mistakes waiting
for a reviewer. This is v1-L2 (upsert semantics are underspecified)
confirmed from the implementation side: when v0.x grows a write
language, `upsert` must be designed, and "which key", "what happens to
non-key columns on conflict", *and* "how guards compose with the
conflict path" are the whole design.
*Softened* by fn v2: the guard/conflict composition problem is now
spelled as a visible `NOT EXISTS` exemption on each refusal guard
(`follow` and `like_post` handle the wrinkle the same way — the
asymmetry died), and the no-op-upsert incantation survives only as
the return-the-existing-row idiom. The CASE-per-column revive
stands untouched. The upsert-design ask stands in full.

**W2 · One statement carries more policy than expected.** `add_comment`
enforces three guards and routes a restricted commenter to `held` — the
restriction flow that v1 declared and abandoned (v1-B5) — in a single
INSERT with a CASE. Verified live: vera's comment on luis's post lands
`"status":"held"`. The lesson sharpens G3: the single-statement wall is
about *table count*, not logic capacity. Multi-statement bodies, not a
smarter statement, are the ask.

**W3 · Two clocks, two id mints.** Engine writes (seed, floor) stamp
RFC 3339 with sub-µs precision and v7 ids; escape writes stamp
millisecond precision and v4 ids. Timestamps stay mutually parseable and
cursor comparisons are lexicographically sane at second granularity, but
sub-second interleavings of engine- and fn-written rows can order
inconsistently. Dissolves if G4's builtins land.
*Dissolved.* G4 landed exactly so: one clock, one id mint, both paths.

---

## C — Confirmations: v0 designs the dogfood validated

**C1 · Arity is an idempotency statement.** `-> T?` on `unfollow`,
`unlike_post`, `unsave_post`, `unblock` reads exactly like the PRD's
"unfollowing is idempotent" acceptance criteria: miss = null = already
done. Verified live (double-unfollow → `null`, no error). Worth blessing
explicitly in docs as the idiom for idempotent deletes.

**C2 · Structured mentions survive renames by construction.**
`mention.profile` is a ref; `rename_user` touches one row and every
mention follows. A PRD acceptance criterion ("username changes must
preserve existing mention references") met by the type system, with no
code to review. pg.sql needed the same design as a convention.

**C3 · Cross-table derived-error routing pays for itself.** `like_post`
on a dead post surfaces the FK as `bad_request`; a duplicate seed
username surfaces as `user_username_taken` — from inside escape bodies,
with zero declarations. The RFD-0011 bet (runtime-sound escape) holds
under 31 bodies.

**C4 · The save/collection two-table split.** A save-with-optional-
collection wants an optional field in a composite key, which v0 rightly
refuses; splitting into `save` + `collection_post` is the same shape
pg.sql chose independently, and the same trade v1 already weighed
(v1-L7's note on R3: both designs work). Second confirmation; keep the
rule.

**C5 · The ledger scales as intended.** `spock check` now prints
`37 fn(s) (37 unchecked bodies)` — the verification debt of the whole
example in one number, exactly what RFD 0011 §4 wanted the ledger to be.
Every G2/G3 disposition that moves logic from SQL to contract moves that
number toward zero.
*Sharpened* by fn v2: the ledger counts **escapes** now (67 after the
upgrade — multi-statement bodies made the honest unit the statement,
not the body). Note the direction: taking G3 moved the number *up*,
truthfully — the debt was always there, hidden in flows the language
refused to express. The value tier (RFD 0013) then moved it to **65**:
seven refusal-guard escapes retired into table `check`s, six validator
fns added (each one escape) — the debt shifting from opaque guards to
declared constraints. The number still trends to zero the day native
statements arrive.

**C6 · Natural text keys work end to end.** `hashtag { key tag: text }` —
refs bind the text key, `post_hashtag` seeds by handle, GraphQL surfaces
`String!`, REST accepts the tag itself. No uuid ceremony for an entity
whose identity *is* its text.

---

## The suggested next iteration

The table tier absorbed a 4× growth with only additive asks (G1, G5–G7,
G10). The fn tier is where the product outgrew the language: refusals
have no names (G2), flows can't span tables (G3), and a third of the
surface is misfiled as mutations (G11). So the dogfood's proposal for
the next v0.x milestone, as one coherent unit:

1. **fn v2 — "name your refusals, span your tables"** (one RFD): G2
   declared errors + raise channel, G3 multi-statement bodies, G11 the
   read marker. All three are signature-side; the escape stays the
   escape, prepare()-validation extends statement-by-statement. This
   unbreaks `approve_follow_request`, lets `block_user` sever follows,
   gives notifications a writer, and files reads under Query.
   ***Taken*** — RFD 0012, shipped exactly as this item asked (the
   read marker inverted to `mut`-marks-writes). The roadmap call below
   was made: fn v2 jumped the filter RFD.
2. **Table-tier small batch**: G1 closed text sets, G7 `set null`,
   G9 scalar returns, G10 float, G4 DDL defaults + engine builtins.
   Each is small; together they delete most of the apology comments in
   `v0.spock`. ***Taken*** — G1 last, as the value tier (RFD 0013):
   closed-set types plus validator-fn `check`s, which also retired seven
   fn-guard refusals and closed G13's value half.
3. **Constraint tier 2** (after the batch): G6 partial unique, G5 one-of.
   The value tier reserved a **row-level `check (a, b) fn`** for the
   cross-column rules it couldn't inline as columns — timestamp orderings
   (`responded_at >= requested_at`), conditional presence
   (`(kind='video') = (duration_ms IS NOT NULL)`), and G5's one-of half;
   the mechanism (name==code CHECK routing) is already live.
4. Already-planned tracks, priorities re-ratified by this exercise:
   the **filter sub-language** must cover policy/row visibility (G13)
   and own read-page discipline — cursor, ordering, ceiling (G16,
   ratified as *the* pagination owner when fn v2 declined it: one
   discipline for tables, views, and read fns alike), **auth** should
   lead with fn actor binding (G12), **derived fields** (G15) and
   **nested reads** (G8) queue behind them. G17 (statement value
   passing) waits for the native body, deliberately.

With fn v2 landed, the filter RFD is next on the plan of record
(RFD 0009 §3) — and it inherits three riders from this file: policy
filtering (G13), page discipline (G16), and the floor-leak closure
both imply.

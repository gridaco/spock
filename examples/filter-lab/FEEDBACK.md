# filter-lab — feedback

Where `examples/instagram/*-FEEDBACK.md` reviews a *product* against a PRD,
this file reviews the *filter sub-language* (RFD 0021) against a schema built
only to stress it. Every finding below was produced by exercising
`schema.spock` end to end through both frontends
(`crates/spock-runtime/tests/filter.rs`) — the Hasura `bool_exp` surface on
`/graphql/v1` and the PostgREST operator surface on `/rest/v1`.

The fixture is deliberately meaningless: one `widget` table with a column of
every filterable shape (text, int, float, bool, a closed set, a nullable
column, a timestamp, a self-reference) plus a composite-key `edge` table, and
five seed rows chosen to straddle the edges — three rank-1 rows (an ordering
tie), one null note, labels carrying a literal `%`, a literal `_`, and a
non-ASCII accent.

Cross-references like §7 point into `docs/rfd/0021-filter.md`; F1-style tags
are this file's own.

## Where the language holds

The load-bearing claims of RFD 0021 survive contact with the fixture:

- **One IR, two skins, one result.** Every probe that has both spellings —
  `?active=is.true` vs `where: {active: {_eq: true}}`, `?rank=gt.1` vs
  `where: {rank: {_gt: 1}}` — returns the identical row set. The frontends are
  genuinely projections of one predicate tree.
- **The forced total order is real (F-none).** Three rows share `rank = 1`.
  Paged `order=rank.asc&limit=2` in three windows, the concatenation is
  byte-identical to the un-paged order — no row skipped, none doubled. The
  appended primary-key tiebreak (§7) is doing exactly its job.
- **Refusals are caller-shaped, never 500.** The NULL law, `like`, cross-table
  traversal, an off-set closed value, the offset ceiling, an unknown column,
  a bad order direction — every one is a `bad_request` / `type_mismatch` /
  `unknown_field` with the §8.1 envelope. Nothing reaches SQLite to fail at
  `prepare()`.
- **Closed-set membership fails loud.** `kind=eq.delta` is a `type_mismatch`
  (422), not a silently-empty result — a typo in an enum filter is caught, not
  swallowed.
- **`storage_object`-style read-only builtins and composite keys** filter and
  order through the same composer with no special-casing.

## Findings

**F1 · A naive `_gt` keyset silently skips tie rows — and the language is
right not to hide it.** Ordering `widget` by the non-unique `rank`, the
"obvious" page-2 cursor `?rank=gt.<lastRank>` drops *every other* row sharing
that rank: page 1 returns two rank-1 rows, page 2 (`rank=gt.1`) returns only
ranks 2–3, and the third rank-1 row is returned by *no* page — 4 of 5 rows
seen, one lost. This is `examples/instagram/v0-FEEDBACK.md` G16 reproduced in a
petri dish. The forced pk tiebreak fixes the `ORDER BY`, but it cannot fix a
client's `WHERE` boundary, and neither borrowed dialect has a tuple-comparison
surface to express the sound compound cursor. The fixture is the evidence that
§7's call — ship offset + a forced order, *defer* a productized keyset rather
than advertise a broken one — is the honest one. → §7 confirmed; the test
`naive_keyset_skips_ties` pins the footgun so no client mistakes it for a
cursor. Productized keyset stays deferred.

**F2 · `_neq` (and the NOT-family) drops NULL rows.** `note=neq.first` returns
three rows — not four. The row with a *null* note is excluded, because
`NULL <> 'first'` is `NULL`, not `true` (three-valued logic). This is
SQL-exact and matches Postgres/Hasura verbatim, so "correcting" it would break
the borrow — but it is a real footgun: a user reads `note != "first"` as
"everything except first" and does not expect the unset rows to vanish.
→ §8.5: the NULL law forbids `_eq: null` and routes null intent through
`_is_null`, which is the right lever; the residual 3VL exclusion is inherent
to the borrowed semantics and is documented, not patched.

**F3 · `ilike` case folding is ASCII-only.** `_ilike: "CAF%"` matches `café`
(the ASCII head folds), but `_ilike: "CAFÉ"` matches *nothing* — `É` is not
folded to `é`. This is SQLite's bundled `LIKE` (§4), and it is a genuine
surprise for any non-English data: a case-insensitive search that quietly
isn't, past the ASCII range. → §4: `_ilike` is honestly the ASCII-case-fold
`LIKE`; a Unicode-aware match waits for a real backend (or an ICU build). Worth
a one-line caveat wherever `_ilike` is documented for clients.

**F4 · `%`/`_` are wildcards with a non-obvious escape, and REST `*` cannot be
matched at all.** The label `50% off` contains a literal `%`; `beta_three`
contains a literal `_`. Both characters are `LIKE` wildcards, so filtering for
them literally requires the `\` escape the lowering already installs
(`ESCAPE '\'`, §4) — reachable but undocumented and easy to miss. Worse on
REST: because the skin aliases `*` → `%` (§6), a literal `*` in the data is
*unmatchable* through `ilike` there. → §6 records the `*` alias as a deviation;
this fixture shows its user-visible edge. Candidate for the clients doc:
"to match a literal `%`/`_`, escape it `\%`/`\_`; `*` is not literally
matchable over REST `ilike`."

**F5 · The GraphQL reference-filter spelling is verbose next to REST's.** To
filter posts by author on REST it is the terse `?parent=eq.<id>`; on GraphQL it
is the nested `where: {parent: {id: {_eq: <id>}}}` — the key must be named
because the ref field is typed as the target's `bool_exp` for forward
compatibility (§5). The nesting is correct and future-proof (the reserved
`Exists` node attaches to the same field type), but it is one level deeper than
a naive author writes, and the two frontends read asymmetrically for the single
most common relationship filter. → §5, accepted: the verbosity buys a `where`
schema that never breaks when cross-table traversal lands. Filtering a ref for
*null* works through the same door — `{parent: {id: {_is_null: true}}}` folds
to `"parent" IS NULL` — which is handy but equally indirect.

**F6 · A closed-set column accepts ordered operators that mean nothing.**
Because a set field is a GraphQL `String`, its `bool_exp` is
`String_comparison_exp`, so `kind=gt.alpha` or `kind=ilike.*a*` parse and run
as lexical string operations. Harmless (they lower to valid SQL) but
semantically meaningless — only `_eq`/`_neq`/`_in`/`_nin`/`_is_null` are
sensible on a closed set. → minor; §9 already notes only the equality family is
"meaningful". Restricting the operator set per column type is a possible later
tightening, but it costs a per-scalar comparison-exp split for a footgun that
merely returns odd-but-defined results.

**F7 · No total count for a paged client.** Offset paging is available and
stable, but there is no way to ask "how many rows match this filter?" — so a
client cannot render "page 3 of 12" or a result count without over-fetching.
→ §12, expected: `count` (Hasura `_aggregate`, PostgREST `count=exact`) is
explicitly deferred. This fixture confirms the gap is felt the moment you page.

## The one-line verdict

The filter layer does what §0 promised — the author writes a predicate, and the
protocol owns the page, the order, and the escaping — and every place it is
*sharp* (F1–F4) is a place the RFD already chose to be honest rather than
clever. Nothing here is a bug; the findings are the price list, and it is the
one the design already quoted.

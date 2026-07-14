# Studio write-form feedback

This file records gaps exposed by exercising Studio's metadata-driven insert
sheet against the compiled contract, GraphQL floor, REST query surface, and
storage gate. It follows the repository's fixture-feedback convention: each
finding states the current evidence, names the component that owns the close,
and gives a concrete completion condition.

Tags `S1`–`S6` belong to this file. "Owner" means where the design or
implementation decision belongs; it does not imply that Studio should paper
over a missing contract fact.

**Disposition (July 2026): all deferred.** The current Studio and protocol
behavior is locked for this milestone. Reopen a finding only as explicit work
owned by the component named below.

## Owner index

| Finding | Owner | State |
| --- | --- | --- |
| S1 · canonical reference labels | Studio presentation + compiled contract | deferred design |
| S2 · scalable reference lookup | Studio, using the existing query protocol | deferred implementation |
| S3 · structured validator hints | value-constraint contract | deferred design |
| S4 · explicit NULL on defaulted insert | GraphQL/write semantics | locked v0 semantics |
| S5 · 64-bit integers on the GraphQL/browser wire | GraphQL scalar mapping | deferred conformance work |
| S6 · persisted upload filename | storage protocol implementation | deferred implementation |

## Findings

**S1 · Generic references have no canonical display label.** The actor picker
has a protocol-defined `label`, but a normal reference carries only its target
table and key. Studio currently chooses the first unique non-key text field,
then the first non-key text field, then the key. That keeps the picker useful,
but it is a presentation heuristic, not contract truth.

→ **Owner:** Studio presentation in
[`docs/rfd/0015-studio.md`](../../../docs/rfd/0015-studio.md), with any authored
or derived marker added to the compiled-contract IR in
[`crates/spock-lang/src/ir.rs`](../../spock-lang/src/ir.rs). Because the v0
contract is additive, a marker must be optional. Close when all contract
consumers can select the same authoritative label without guessing.

**S2 · The reference picker loads one capped page.** Studio currently requests
at most 200 target rows and cannot search or page within the sheet. This is
primarily Studio debt, not a missing basic query contract: REST already exposes
`limit`, `offset`, filters, and total ordering. Exact counts and a safe compound
keyset remain protocol-level gaps already evidenced by
[`filter-lab` F7 and F1](../../../examples/filter-lab/FEEDBACK.md).

→ **Owner:**
[`src/views/insert-row-sheet.tsx`](src/views/insert-row-sheet.tsx), using the
query layer specified by
[`docs/rfd/0021-filter.md`](../../../docs/rfd/0021-filter.md). Close the Studio
part with debounced server search, offset paging, stale-request cancellation,
and a way to select every reachable row. Do not invent an ad-hoc keyset cursor.

**S3 · A `check` names a validator but does not describe its controls.** The
contract can tell Studio that `valid_username` validates a field, but not that
the underlying rules are a length bound, pattern, numeric range, or a
conjunction. Studio can name the validator and surface its server error; it
cannot honestly derive `min`, `max`, or `pattern` attributes from arbitrary
validator SQL.

→ **Owner:** the value-constraint contract in
[`docs/rfd/0013-value-constraints.md`](../../../docs/rfd/0013-value-constraints.md).
Close only with an explicit additive hint/structured-constraint design; parsing
validator SQL in Studio is not a valid close.

**S4 · A defaulted optional field cannot be inserted explicitly as SQL NULL.**
Insert semantics intentionally treat `null` as absence, so omission and
explicit `null` both apply the default. Update has the distinct operation
needed to clear the field afterward, but a one-step insert cannot express it.

→ **Owner:** the normative write rules in
[`docs/spec/v0.md`](../../../docs/spec/v0.md) §5.1 and
[`docs/spec/graphql.md`](../../../docs/spec/graphql.md) §5. This is already a
declared v0 limitation, not a Studio bug. Close only with an unambiguous wire
representation that preserves GraphQL's unprovided-variable semantics.

**S5 · Spock `int` is wider than both GraphQL `Int` and an exactly representable
browser number.** The language/storage contract is signed 64-bit, GraphQL
currently exposes the built-in 32-bit `Int`, and JavaScript `number` is exact
only through `2^53 - 1`. Studio rejects unsafe JavaScript integers to prevent
silent rounding, but values outside the GraphQL range can still fail at the
wire boundary.

→ **Owner:** scalar mapping in
[`docs/spec/graphql.md`](../../../docs/spec/graphql.md) and
[`crates/spock-runtime/src/graphql.rs`](../src/graphql.rs). Close by choosing a
truthful end-to-end representation (for example a custom 64-bit scalar/string
wire, or a deliberately narrower language type), then pin boundary tests and
generated TypeScript behavior.

**S6 · Browser-uploaded filenames are session-only.** The accepted storage
schema says `storage_object.name` is the original filename, but the mint request
accepts no filename and the signed PUT currently persists only content type,
size, and checksum. Studio can show `File.name` immediately after upload; a
later session may have only the object id.

→ **Owner:** the storage protocol and handler in
[`docs/rfd/0018-storage.md`](../../../docs/rfd/0018-storage.md) and
[`crates/spock-runtime/src/storage/mod.rs`](../src/storage/mod.rs). Close when a
defined, safely handled filename travels through mint or a signature-bound PUT
header, is persisted to `storage_object.name`, and is covered by storage tests.

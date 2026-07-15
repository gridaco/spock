---
rfd: "0024"
title: Nominal product-error declarations
authors:
  - "@softmarshmallow"
sponsor: "@softmarshmallow"
shepherd: "@softmarshmallow"
decision: draft
implementation: unplanned
review-start: null
review-end: null
tracking-issue: null
implemented-in: null
supersedes: []
superseded-by: null
---

# RFD 0024 — Nominal product-error declarations

## Summary

Spock should require every authored product-error code in a function's `!`
clause to resolve to one top-level nominal declaration. The declaration owns a
stable name and optional documentation; it does not yet define a payload,
hierarchy, status, message, recovery policy, or new raise mechanism. Derived
schema errors and protocol-reserved errors remain owned by the compiler and
runtime and cannot be redeclared.

This is deliberately a small language slot. It fixes name ownership and typo
detection now while leaving the difficult shape and propagation questions to
later evidence and separate decisions.

## Problem

RFD 0012 gave functions named product refusals, but it made an otherwise
unknown identifier in a function's `!` clause mint that refusal locally:

```spock
mut fn follow(target: user) -> follow ! account_private {
  unchecked sql("...")
}
```

That rule creates three concrete problems:

1. A typo silently creates public metadata. If `account_private` already has
   product meaning and another function writes `acount_private`, both spellings
   compile as different refusal codes.
2. The product error has no declaration site. Documentation and tools must find
   every function that happens to mention the string before they can learn that
   the concept exists.
3. Sharing one product error across operations is accidental string agreement,
   not name resolution. No declaration owns the public code or provides a
   stable place for future evolution.

Existing Spock cannot fix these problems with derived errors: a derived code is
evidence of a particular schema constraint, while a product error names a rule
implemented by a deliberate operation. Protocol-reserved codes are runtime
failures and are not application vocabulary. A nominal declaration is the
smallest construct that distinguishes these categories without prematurely
designing a complete error type system.

## Goals

- Give each authored product-error code one explicit, inspectable declaration
  site.
- Make misspelled or otherwise unknown codes in `fn !` clauses compile errors.
- Preserve the existing runtime refusal behavior and wire envelope.
- Carry product errors and their optional docs in an additive, evolvable
  contract shape.
- Keep schema-derived, protocol-reserved, and authored product errors in one
  collision-free public code vocabulary.
- Reserve a language-owned slot that later RFDs can extend without deciding
  those extensions here.

## Non-goals

- Structured error payloads or typed fields.
- Author-authored runtime messages, localization, or presentation copy.
- Per-error HTTP or GraphQL status selection.
- Error inheritance, aliases, categories, qualification, or modules.
- Replacing, wrapping, or customizing derived constraint errors.
- Error inference, exhaustiveness, reachability, or unused-declaration checks.
- A checked statement for raising errors; `spock_refuse` remains the v0 escape
  channel.
- Error propagation across function calls, effects, jobs, or external systems.
- Retry, recovery, compensation, observability, or privacy metadata.

## Design-principle fit and tensions

This proposal primarily serves [one authoritative
representation](../governance/design-principles.md#1-one-authoritative-representation),
[add contract, not
distance](../governance/design-principles.md#3-add-contract-not-distance),
[state once; derive
deterministically](../governance/design-principles.md#5-state-once-derive-deterministically),
and [name effects and failure
boundaries](../governance/design-principles.md#8-name-effects-and-failure-boundaries).
A product-error name gains one declaration site, function signatures resolve to
it, and generators consume the resulting contract object.

It also follows [the least powerful sufficient
language](../governance/design-principles.md#4-prefer-the-least-powerful-sufficient-language):
`error ident` establishes name ownership and nothing more. Adding payloads,
inheritance, statuses, or messages without use-case evidence would turn this
small correctness fix into an unearned error type system.

The tension is compatibility. RFD 0012 intentionally accepted that an unknown
`!` code minted a refusal. Requiring a declaration breaks those programs at
check time. Spock is pre-1.0, and the failure is direct and usually fixed by one
declaration; product codes that collide with protocol-owned names require a
rename (§Compatibility). The stronger typo and ownership invariant is worth
that source break if this RFD is accepted.

## Background and evidence

Spock already has three wire-level error populations:

- **derived errors**, whose codes and evidence come from table constraints;
- **reserved errors**, whose codes and meaning belong to the protocol/runtime;
  and
- **product refusals**, raised by a function body through `spock_refuse` and
  exposed as kind `refused`.

RFD 0012 made product refusals visible in `FnDef.errors` and
`FnDef.refusals`, but deliberately left global name collisions unenforced and
noted that a misspelled derived code becomes dead refusal metadata. That was a
reasonable first slice: it proved named refusal routing without needing a new
declaration. It is now the evidence for the smaller missing concept—nominal
ownership—rather than evidence for immediately designing rich errors.

The `0.5.2` toolchain includes a prototype to test parser, checker, contract,
and tooling consequences. It is experimental and unstable. Under the
[language-change
process](../governance/language-change-process.md#exploration-before-10), that
prototype is non-normative evidence: it does not make this draft accepted,
change the supported v0 contract, or change `implementation: unplanned` for a
supported implementation.

No language-problem issue or committee triage record is linked yet. This draft
must not enter public review until that problem record exists and the committee
confirms that it is mature enough for review.

## Prior art

The [Rust Reference's enum
model](https://doc.rust-lang.org/reference/items/enumerations.html) demonstrates
the stronger nominal design: a declaration owns variants and may attach data,
while matching can be exhaustive. Spock takes only the name-ownership lesson;
payloads and exhaustiveness are expressly outside this RFD.

[PostgreSQL SQLSTATE
codes](https://www.postgresql.org/docs/current/errcodes-appendix.html) show the
value of a stable, centrally owned machine vocabulary. They are a fixed system
taxonomy, however, not declarations of application-specific product rules.

The [OpenAPI Components
Object](https://spec.openapis.org/oas/latest.html#components-object) provides a
reusable object-shaped registry that operations can reference. That supports
the separation between declaring a reusable concept and listing it on an
operation, but OpenAPI response objects also decide protocol representation;
this RFD intentionally does not.

## Proposed design

### Declaration and documentation

A product error is a top-level declaration:

```spock
/// The current actor may not see this account.
error account_private
```

Its grammar is exactly:

```ebnf
error_decl = "error" ident ;
```

`error` becomes an active keyword. The declaration has no body and no
terminator. It may appear anywhere a top-level declaration may appear. Error
declarations and function error clauses resolve over the whole file as one
order-independent relation, so a function may reference an error declared
later in the file.

An outer `///` doc comment may document an `error` under the existing doc
attachment and normalization rules. The text is contract documentation, not a
runtime message. An undocumented declaration has no `doc` field in the
contract. Inner `//!` file documentation is unchanged.

### Namespaces and resolution

Product errors occupy one program-wide **product-error namespace**. It is
separate from table/record type names and from function names, so this is legal:

```spock
table account_private { key id: uuid = auto }
fn account_private() -> bool { unchecked sql("SELECT true") }
error account_private
```

The public error-code vocabulary is broader than the declaration namespace. It
is the union of:

1. schema-derived codes (§6.1 of the v0 specification);
2. protocol-reserved codes (`not_found`, `type_mismatch`, `unknown_field`,
   `bad_request`, `internal`, `unauthorized`, and `conflict`); and
3. explicitly declared product-error codes.

Those three populations must be disjoint. A declaration whose code equals a
derived or reserved code is invalid; authored code cannot impersonate evidence
owned by a constraint or the runtime.

`unauthorized` and `conflict` are included because the shipped storage plane
already emits those codes. This proposal makes their existing protocol
ownership visible to the checker; it does not add a storage error or change its
wire behavior.

The global vocabulary is broader than a function's possible failure surface.
A function's `!` clause may name a declared product error, a derived error, or
one of the five protocol codes its invocation plane can emit (`not_found`,
`type_mismatch`, `unknown_field`, `bad_request`, and `internal`). The
storage-only `unauthorized` and `conflict` codes remain reserved globally but
are invalid in a function `!` clause. This restriction makes an old implicit
product refusal with either spelling fail checking instead of silently
changing from a raisable refusal into unreachable metadata. Resolution is
order-independent.

A declared product error does not automatically belong to every function. A
function may raise it through the existing v0 mechanism only when its own `!`
clause names it. Multiple functions may name the same declared product error.
A declaration may be unused; this RFD adds no reachability or lint rule.

### Diagnostics

The following diagnostics are normative if this RFD is accepted:

- **E051** — two top-level `error` declarations use the same identifier;
- **E052** — a code in a function `!` clause is neither a declared product
  error, a derived error, nor a protocol-reserved error available to the
  function invocation plane; and
- **E053** — an explicit product-error declaration collides with a derived or
  reserved error code.

Existing **E039** continues to diagnose the same code repeated within one
function's `!` clause. Existing **E044** continues to diagnose collisions
between derived codes. Diagnostics point at the authored occurrence and should
identify the conflicting declaration, derivation, or reserved owner when one
exists.

### Contract IR

The compiled contract gains one additive top-level field:

```json
{
  "spock": "v0",
  "errors": [
    {
      "code": "account_private",
      "doc": "The current actor may not see this account."
    },
    {
      "code": "follow_self"
    }
  ]
}
```

The IR shape is `Contract.errors: [ErrorDef]`, where each object is:

```text
ErrorDef {
  code: string,
  doc?: string
}
```

The array preserves source declaration order for deterministic display; order
has no semantic meaning. `errors` defaults to `[]` when absent so contracts
produced before this addition still load. New producers emit the array,
including an empty array when there are no declarations.

A legacy contract may therefore contain `FnDef.refusals` entries without
matching top-level objects. Readers preserve and route those historical string
entries exactly as before; they do not synthesize declarations or docs. The
resolution invariant applies to contracts newly compiled from source under
this proposal. This artifact-reading fallback is not source permission to mint
an undeclared refusal.

The object shape is intentional. A bare string array would be sufficient for
today's code, but would require a breaking shape change as soon as declarations
carry any additional contract-owned metadata. The object does **not** reserve
or decide any particular future field beyond the existing optional `doc`.

`FnDef.errors` remains the function's complete declared failure-code list.
`FnDef.refusals` remains its subset of product errors that the escape body may
raise with `spock_refuse`. Both remain string arrays. Each `FnDef.errors` entry
resolves to a top-level product object, a derived table error, or a
function-applicable reserved error; every newly compiled `FnDef.refusals`
entry resolves to a top-level product object. Table-local `errors` arrays
remain derived-error objects and are unchanged. Legacy-artifact fallback is as
described above.

### Runtime behavior

Runtime behavior is unchanged:

- a declared product error named in the current function's `!` clause may be
  raised by `spock_refuse('<code>')`;
- it uses kind `refused`, REST/`ApiError` status 409, `table: null`, and
  `fields: []`; GraphQL remains HTTP 200 and omits status from extensions;
- raising a product error not listed by the current function is `internal`;
- derived and reserved codes cannot be raised by `spock_refuse`;
- a refusal rolls back the function transaction; and
- declaring an error performs no runtime work and creates no new failure path.

The declaration is therefore name ownership and metadata only. It does not
change the v0 envelope or claim that the deferred semantics below have been
solved.

### Semantics and invariants

If accepted, the following invariants hold:

1. Every product-error code newly compiled from conforming source has exactly
   one top-level declaration.
2. Every code in a newly compiled function `!` clause resolves to exactly one
   declared, derived, or function-applicable reserved owner.
3. Declared, derived, and reserved code populations are pairwise disjoint.
4. A product error may be raised only by a function that lists it.
5. Declaration order cannot change resolution or runtime behavior.
6. Documentation cannot affect checking, routing, status, or messages.
7. Contract consumers can distinguish the declaration registry from each
   function's selected failure surface without parsing source or SQL.

### Examples

One declaration can be shared by multiple operations and declared after use:

```spock
mut fn follow(target: user) -> follow
    ! account_private | follow_self | not_found {
  unchecked sql("...")
}

mut fn request_follow(target: user) -> follow_request ! account_private {
  unchecked sql("...")
}

/// The target account does not accept this actor's request.
error account_private
error follow_self
```

Here `account_private` and `follow_self` resolve to product declarations,
`not_found` resolves to a protocol-reserved error, and declaration order is
irrelevant. A schema-derived code resolves the same way without an authored
declaration.

An unknown code is rejected rather than minted:

```spock
error account_private

// E052: `acount_private` resolves nowhere.
fn profile(target: user) -> profile ! acount_private {
  unchecked sql("...")
}
```

Derived and reserved names cannot be claimed:

```spock
// E053: owned by the protocol/runtime.
error not_found

// E053 if this program derives the same unique-constraint code.
error user_username_taken
```

Duplicate declarations are invalid even when their docs differ:

```spock
error account_private
/// Alternate wording does not create another declaration.
error account_private // E051
```

## Alternatives considered

### Keep implicit per-function minting

This preserves source compatibility and keeps the grammar smaller. It also
preserves the typo hole, leaves shared concepts as accidental string equality,
and gives documentation no authoritative home. It does not address the stated
problem.

### Declare errors inside each function

A local declaration would make minting explicit but would not give a shared
product concept one owner. Reusing a code across functions would still be
duplicated or would require an additional import/alias system. Top-level
declarations are smaller and match the program-wide contract vocabulary.

### Scope product codes to function endpoints

Endpoint-scoped namespaces would let an authored `error conflict` coexist with
the storage protocol's `conflict` code because clients could disambiguate them
by route and error kind. That preserves two plausible product names and reduces
migration breakage. It also means the same public code string has multiple
owners, weakens global code unions and cross-protocol tooling, and makes a
future shared error registry context-sensitive. This proposal chooses one
program-wide code vocabulary instead.

### Introduce a complete error enum now

An enum with payloads, variants, exhaustive matching, and hierarchy could solve
future problems. No current evidence chooses those semantics, and adopting them
would couple a typo/ownership fix to several difficult decisions. This RFD
creates the nominal slot without pretending to know its eventual full shape.

### Use string literals in `!` clauses

Quoted strings would make codes visually explicit but would still not declare
or resolve them. They would worsen typo detection by treating every string as
valid and would diverge from Spock identifiers and generated code unions.

### Make no language change

Tools could infer product errors by scanning every function's `refusals` list.
That reproduces the current accidental registry, cannot distinguish a typo from
a new concept, and provides no source location for shared documentation.

## Compatibility and migration

This is a source-level break for every program that relies on RFD 0012's
implicit minting. Normally the migration is mechanical: add one top-level
`error <code>` declaration for each distinct product refusal, optionally move
shared documentation there, and keep each function's `!` clause unchanged.
Derived and reserved codes receive no declarations.

There is one deliberate exception. Existing product refusals named
`unauthorized` or `conflict` collide with codes already owned by the storage
protocol and must be renamed before declaration. The checker reports E052 for
either spelling in a function `!` clause even when no declaration was added,
so unchanged source cannot compile with silently different refusal routing.
That is a public source and wire-code break for such a program, not an
automated declaration-only fix. The alternative would be endpoint-scoped code
namespaces; this RFD instead chooses one collision-free public vocabulary so a
code has one owner.

`error` was already reserved in v0, so conforming programs could not use it as
an identifier; activating it creates no additional identifier break.

The contract change is additive under the v0 freeze: new readers default a
missing top-level `errors` field to `[]`, and the new field uses objects so
future optional metadata can be additive. Existing `FnDef.errors`,
`FnDef.refusals`, table error objects, wire envelopes, GraphQL behavior, data,
and storage behavior are unchanged. New readers continue honoring legacy
`FnDef.refusals` even when the registry is absent. The generated
`reserved_error` union widens to include the two already-shipped storage codes.

Rollback before adoption means removing the declarations and restoring
implicit minting. After adoption, removing a public declaration or renaming its
code is a public API break even if no function currently raises it; the
declaration itself places the code in the contract.

## Consequences

**Security and privacy.** Failure routing and wire behavior are unchanged, but
declarations and their docs become public contract metadata. Authors should
avoid putting sensitive facts in public code names or docs because the v0
contract is openly introspectable. The proposal does not solve whether a
particular refusal leaks sensitive state.

**Performance and operations.** Checking adds only program-wide name-set
construction and collision checks. The runtime performs no new work. Contract
size grows by one small object per declared product error.

**Implementation complexity.** Parser, AST, checker, IR serialization, doc
attachment, generated TypeScript, Studio, and editor syntax need to recognize
one declaration. The checker must derive the full schema error set before
finishing collision and function-clause resolution, regardless of source
order.

**Teachability.** The rule is direct: derive infrastructure failures, declare
product failures, list operation failures. There is one extra line per shared
code, offset by one obvious source of truth and typo diagnostics.

**Long-term evolution.** An object-shaped IR and a nominal source slot leave
room for later proposals. That room is not an implicit acceptance of payloads,
statuses, hierarchy, or any other deferred feature.

## Implementation and conformance plan

Acceptance would authorize, but does not schedule, the following work:

1. Parse top-level `error ident`, activate the reserved keyword, and attach
   outer docs.
2. Add product-error declarations to the checked model and implement E051,
   E052, and E053 after the full derived/reserved vocabulary is known.
3. Add `Contract.errors: Vec<ErrorDef>` with a default for old contract JSON;
   preserve `FnDef.errors` and `FnDef.refusals` semantics.
4. Update generated TypeScript, Studio/introspection, and editor syntax to read
   the declaration registry while preserving existing per-function surfaces.
5. Add parser, checker, serialization/back-compatibility, runtime-regression,
   codegen, Studio, and editor conformance tests.
6. While the RFD is draft, keep every prototype fixture migration prominently
   marked experimental, unstable, and non-normative. Inclusion in the `0.5.2`
   toolchain does not make those fixtures supported-language conformance.
7. Reconcile `docs/spec/v0.md`, `docs/spec/graphql.md`, user documentation, and
   conformance fixtures when supported implementation ships.

Required conformance cases include declarations before and after use; documented
and undocumented objects; shared use across functions; unused declarations;
E039/E051/E052/E053; collisions against every derived kind and every reserved
code; rejection of the two storage-only reserved codes on function surfaces;
old contracts missing the field; and proof that refusal envelopes and
transaction rollback are unchanged.

The implementation preview included in `0.5.2` while this RFD is draft is an
experimental, unstable, non-normative prototype. It must not be presented as
supported v0 behavior or merged into the normative specification before the
language-change process authorizes it.

## Specification changes

If accepted and implemented, the following normative sections change:

- `docs/spec/v0.md` opening scope and §§1, 2.3, and 2.4: add the declaration and
  its doc-comment attachment; move `error` from reserved to active;
- `docs/spec/v0.md` §3: add `error_decl` and the order-independent resolution
  rules;
- `docs/spec/v0.md` §4: add E051, E052, and E053 and revise E039's unknown-code
  note;
- `docs/spec/v0.md` §§6 and 6.1: add `Contract.errors: [ErrorDef]`, describe its
  compatibility default and legacy fallback, include the two existing storage
  codes in the protocol-owned registry, and replace implicit refusal minting
  with declared product-error resolution;
- `docs/spec/v0.md` §7.4: preserve runtime raising while requiring a product
  declaration plus per-function listing; and
- `docs/spec/graphql.md` §§5.1 and 6: describe declared product errors instead
  of implicitly minted refusals; the GraphQL schema and envelope do not change.

A proposed spec patch may live beside an implementation preview for review,
but it is non-normative while this RFD remains draft and supported
implementation remains unplanned.

## Unresolved questions

- Which language-problem issue and committee triage record will ground public
  review?
- Does the name-ownership rule and source break have sufficient evidence to
  enter public review?
- Should an accepted migration provide an automated fix, or is the diagnostic's
  suggested `error <code>` declaration sufficient at v0 scale?
- Should declaration order remain visible in the contract, as proposed, or
  should producers sort by code? This affects deterministic presentation only,
  not semantics.

Payloads, messages, statuses, hierarchy, aliases, customization of derived
errors, propagation, exhaustiveness, reachability, and recovery metadata are
not unresolved details of this proposal. They are explicitly deferred language
problems requiring their own evidence and decision.

## Decision record

<!-- Maintained by the Language Design Committee. -->

- Decision: not decided
- Decision date: not applicable
- Review period: not started
- Committee participants: not applicable
- Conflicts and recusals: not applicable
- Eligible voting members and quorum calculation: not applicable
- Deliberation record: not applicable
- Consensus or vote, including names and positions: not applicable
- Required threshold and whether it was met: not applicable
- Bootstrap provision used: not applicable
- Design Steward invoked: no; if yes, record the committee's equivalence
  finding and link the steward's rationale
- Rationale: not applicable
- Material objections and responses: not applicable
- Dissent: not applicable

## Implementation record

<!-- Update links and state without rewriting the decided design. -->

- Tracking issue: not assigned
- Implementation pull requests: [spock#24](https://github.com/gridaco/spock/pull/24), [uhura#14](https://github.com/gridaco/uhura/pull/14)
- Conformance tests: prototype coverage is included in spock#24; accepted-language conformance has not started
- Documentation and specification reconciliation: non-normative preview text only
- Shipped release: `0.5.2` experimental implementation preview; no supported implementation

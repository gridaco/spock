# Requests for Discussion

An RFD is Spock's durable record of a proposed design and its disposition. It
exists so adoption follows a concrete problem, evidence, sponsorship, public
review, and a coherent decision. Sketches and prototypes remain welcome inputs
before and during that process.

The [language-change process](../governance/language-change-process.md) is the
authoritative gate. This file details the RFD artifact, lifecycle metadata,
and permanent index; its stage summary does not replace that process.

This process applies prospectively. RFDs `0000` through `0023` predate it and
remain **legacy records** with their original titles and status wording. The
legacy index below reports what those files say; it does not retroactively
accept, reject, or normalize them.

## When an RFD is required

An RFD is required before adopting a change into any supported public language
contract, including:

- syntax, grammar, static semantics, runtime semantics, or diagnostics policy;
- the type system, standard language concepts, or standard modules;
- compatibility guarantees or a normative serialization/IR contract;
- behavior exposed by a specified protocol or generated client surface; and
- a lasting architectural decision that constrains future language evolution.

An ordinary bug fix, conformance fix, refactor with no public consequence, or
documentation correction does not need an RFD. When classification is unclear,
the Language Design Committee decides during triage.

An isolated, opt-in pre-1.0 experiment may be shared as non-normative evidence
under the [language-change process](../governance/language-change-process.md#exploration-before-10).
A pull request seeking to make that experiment supported or default behavior is
not a substitute for an RFD and may be redirected or closed as procedurally
premature without discarding useful evidence.

## Lifecycle

### 1. State the language problem

Anyone may open a language-problem issue. It must identify concrete use cases,
the current limitation, and evidence that existing Spock cannot address it. A
preferred spelling or implementation by itself is not a language problem, but
either may accompany the problem as exploratory evidence.

### 2. Committee triage

The Language Design Committee classifies the issue as an ordinary bug or
implementation task, a duplicate, covered by a current adoption default,
outside Spock's direction, an individual research question, a candidate for a
temporary working group, or ready for a sponsored RFD. The committee triages as
capacity allows; the issue may remain useful evidence even when no WG or formal
review follows.

### 3. Study when needed

The committee may charter a [working group](../working-groups/README.md) for a
bounded problem that needs organized research, prior art, experiments, or
competing approaches. A WG produces evidence and a final report. It cannot
make a language decision or accept its own recommendation.

### 4. Obtain a sponsor and shepherd

Before an RFD receives a number, it needs:

- a **sponsor**, who is a voting Language Design Committee member and attests that the
  problem is in scope and mature enough to consume review time; and
- a **shepherd**, who owns the proposal's progress, makes sure objections and
  alternatives are answered, and prepares it for a decision.

Sponsorship means “worth formal review,” not endorsement. The sponsor and
shepherd may be the same person, but both responsibilities must be named. The
committee or its RFD editor assigns the next unused four-digit number; numbers
are never reused.

Copy [TEMPLATE.md](TEMPLATE.md) to `NNNN-short-title.md`. All required sections
and metadata must be complete before public review.

### 5. Draft

The author and shepherd develop the proposal in a pull request. A draft must
connect the problem to Spock's design principles, state goals and non-goals,
show realistic examples and counterexamples, describe exact semantics, examine
prior art and alternatives, and identify compatibility and implementation
consequences.

Prototypes may support a draft or precede sponsorship. They do not create
precedent, reserve syntax, or authorize graduation into supported behavior.

### 6. Public review

When the shepherd and committee agree the draft is ready, the RFD enters
`decision: review` and receives announced review dates. Public review lasts at
least **10 calendar days**. The committee may extend it; a substantial design
change may restart it so the changed proposal receives a real review.

Review is a search for evidence, consequences, and unresolved objections—not
a popularity vote. Committee and WG discussion may occur offline, but any
design reasoning relied on for the decision must be summarized in the
repository before the decision becomes binding.

### 7. Decide

After review, the committee records one of the decision states below and adds
its rationale to the RFD. The decision record must address material objections,
name important tradeoffs, and preserve dissent when consensus was not reached.

An accepted RFD authorizes the described design to graduate into supported
implementation. Acceptance does not promise priority, staffing, release timing,
or compatibility with unfinished prototypes.

### 8. Implement and reconcile

Adoption is tracked separately from the design decision. An accepted RFD should
link a tracking issue, and pull requests that graduate the accepted design into
supported behavior must link the RFD.

`implementation: implemented` may be recorded only when all of the following
are complete:

- the implementation is merged;
- relevant conformance tests pass;
- user-facing documentation is updated; and
- every affected normative specification is reconciled with the shipped
  behavior.

## Two independent status axes

Every prospective RFD uses both metadata fields from the template.

### `decision`

| Value | Meaning |
| --- | --- |
| `draft` | The proposal is being developed and has no committee decision. |
| `review` | The complete proposal is in its announced public review period. |
| `accepted` | The committee authorizes this design to graduate into supported implementation. |
| `rejected` | The committee considered and declined this design. |
| `deferred` | The committee has postponed a decision pending named evidence, timing, or dependencies. |
| `withdrawn` | The author or sponsor ended the proposal before a terminal committee decision. |
| `superseded` | A later RFD replaces this decision; `superseded-by` must identify it. |

### `implementation`

| Value | Meaning |
| --- | --- |
| `unplanned` | No supported implementation is scheduled or actively tracked. Before acceptance, graduation into supported behavior is unauthorized. |
| `planned` | An accepted design has an implementation plan or tracking issue. |
| `in-progress` | Implementation work is active but the language/specification is not fully reconciled. |
| `implemented` | Code, conformance tests, documentation, and affected specifications are aligned. |

This implementation axis tracks adoption, not isolated pre-1.0 experiments.
The axes answer different questions. In particular, `decision: accepted` plus
`implementation: unplanned` is valid and means “approved, but not committed to
a schedule.” Acceptance must never be presented as “already in Spock.”

## Required proposal contents

The template requires:

1. a summary and concrete problem statement;
2. goals and explicit non-goals;
3. fit with and tensions among Spock's published design principles;
4. background, evidence, and relevant prior art;
5. the complete proposed design, semantics, and invariants;
6. realistic examples, counterexamples, and failure behavior;
7. alternatives considered, including making no language change;
8. compatibility and migration consequences;
9. security, privacy, performance, and operational consequences as applicable;
10. an implementation and conformance-test plan;
11. the exact normative specification sections that would change;
12. unresolved questions; and
13. a committee-owned decision record.

An RFD can say a section is not applicable, but it must explain why rather than
delete the section.

## Normative and historical boundary

RFDs are permanent decision history. They explain why a direction was accepted
or declined and, while work is unfinished, what an accepted implementation is
authorized to build. They do **not** define current language behavior.

Only [the specification](../spec/) is normative for current Spock. If an
accepted RFD and the current specification differ, the specification describes
what users can rely on today. Once an RFD is implemented, the specification—not
the RFD—remains the current contract.

After a terminal decision, edit an RFD only for clerical corrections, links,
implementation tracking, or an explicitly labeled decision addendum. A design
change requires a follow-up RFD that links and, when appropriate, supersedes
the earlier record. Rejected and superseded RFDs remain in this directory.

WG notes, prototypes, and examples remain non-normative even when cited by an
RFD.

## Prospective RFD index

Entries in this table use the current two-axis process. A draft records a
proposal under development, not an adopted language feature; implementation
status tracks supported adoption and excludes branch experiments.

| RFD | Title | Decision | Implementation |
| --- | --- | --- | --- |
| [0024](0024-error-declarations.md) | Nominal product-error declarations | Draft | Unplanned |

## Legacy RFD index

Every entry below is a **legacy record**. “Recorded legacy status” closely
preserves the wording at the top of the file and must not be read as a new
committee judgment under this process.

| RFD | Title | Recorded legacy status |
| --- | --- | --- |
| [0000](0000-vision.spock) | `0000-vision.spock` (no RFD heading) | No `Status:` declaration. |
| [0001](0001-effects-once-extern.md) | Effects, `once` values, and `extern fn` | Discussion draft; nothing is accepted syntax. |
| [0002](0002-day-one-concepts.md) | Day-one concepts | Accepted direction; concepts fixed, syntax illustrative and open. |
| [0003](0003-write-through-views.md) | Write-through views | Accepted direction; semantics sketched, syntax and API illustrative. |
| [0004](0004-exposure-model.md) | The exposure model: defaults, obligations, and the surface ledger | Discussion draft; stance proposed, nothing fixed. |
| [0005](0005-proving-ground.md) | The proving ground: first tasks, the gauntlet, and the database inventory | Working method + reference; no new language surface. |
| [0006](0006-language-identity-ir-first.md) | Language identity and the IR-first architecture | Accepted direction; identity claims and Option-A verdict fixed, concrete encoding and timeline open. |
| [0007](0007-implementation-language.md) | Implementation language | Accepted decision — Rust, with named escape routes. |
| [0008](0008-v0-table-first.md) | v0: the table-first slice | Accepted decision — build v0 `table` first on Rust + embedded SQLite; later concepts deferred, not cut. |
| [0009](0009-roadmap.md) | the v0.x roadmap | Working plan. |
| [0010](0010-client-codegen-architecture.md) | client and codegen architecture | Accepted decision. |
| [0011](0011-verification-line-taxonomy.md) | the verification line and its vocabulary | Accepted decision. |
| [0012](0012-fn-v2.md) | fn v2: name your refusals, span your tables, declare your polarity | SHIPPED (July 2026). |
| [0013](0013-value-constraints.md) | value constraints: validator fns and closed-set types | ACCEPTED (July 2026), implementing now. |
| [0014](0014-actor-seam.md) | The actor seam (identity milestone, slice 1) | Discussion draft; decisions open, no implementation proposed. |
| [0015](0015-studio.md) | Studio: the human-developer layer | Accepted and implemented. |
| [0016](0016-doc-comments.md) | doc comments: `///` outer, `//!` inner, carried in the contract | ACCEPTED (July 2026), implemented. |
| [0017](0017-storage-research.md) | Storage: prior art and the component taxonomy | Research reference; commits to nothing. |
| [0018](0018-storage.md) | Storage v0: files as governed rows | Accepted; implementation on the `storage-v0` branch. |
| [0019](0019-external-plane.md) | The external plane: latency and failure as first-class | Discussion draft; no implementation proposed and commits to nothing. |
| [0020](0020-distribution.md) | Distribution: shipping the `spock` binary | Accepted; original pipeline and framework-sidecar extension recorded as implemented and verified. |
| [0021](0021-filter.md) | the filter sub-language: one predicate IR, two borrowed frontends | ACCEPTED — read half implemented in v0; filtered/bulk writes and v1 policy deferred. |
| [0022](0022-spock-framework.md) | Spock as a framework: one project, one command, two languages | Accepted and implemented for the initial framework host (2026-07-15). |
| [0023](0023-development-state-reload.md) | Development reload, state continuity, and the auto-migration boundary | Problem study; long-term direction under evaluation. A smaller interim host policy is recorded as accepted and implemented. |

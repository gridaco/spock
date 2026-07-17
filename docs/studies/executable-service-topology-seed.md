# Problem seed — Executable service topology

- **State:** inspiration captured; research not started
- **Kind:** problem seed, not a study
- **Created:** 2026-07-17
- **Language-problem issue:** none
- **Working group:** none
- **RFD:** none

> **Need and question only.** This note is not research, a finding, a
> feasibility claim, an architecture or language proposal, reserved
> terminology or syntax, a roadmap commitment, or authorization to implement.

## Need

Some backend products are operated as multiple services with independent
ownership, state, transaction, deployment, and failure boundaries. That
architecture creates substantial engineering and operational cost, but its
boundaries may also be part of how the product remains governable, changeable,
and resilient.

Current Spock does not represent that system shape. The initial framework
defines one project with one Spock authority, and explicitly leaves
microservice orchestration, multiple authority databases, and distributed
transactions out of scope
([RFD 0022](../rfd/0022-spock-framework.md#out-of-scope)).

Collapsing a multi-service product into one local authority may erase the very
questions that the production architecture must answer. Cross-boundary data
access may become direct, separate work may appear atomic, independent
availability may disappear, and time, delivery, retry, compatibility, and
recovery behavior may never be exercised.

This seed records the need to ask whether Spock can preserve those pressures
without reproducing all of the physical and operational friction of production
infrastructure.

## Inspiration

Imagine that a developer can mirror the meaningful service architecture of a
product in one Spock project. One command checks and runs the whole product
locally. The implementation may collapse processes, networks, databases, and
infrastructure where doing so makes development painless, while retaining the
boundaries whose observable consequences matter.

The shorthand for the aspiration is:

> **Collapse physical distance, never semantic distance.**

In that model, “works and fails the same way” means the same **declared
observable semantics** across a boundary. It does not mean that a local runtime
is physically equivalent to a real network, scheduler, broker, cloud, database
fleet, or deployment.

*Executable service topology* is only a working description for this question.
It is not a selected term or a proposed Spock feature.

## Central question

Could Spock make a production-shaped multi-service topology executable as one
local project and one command without erasing the boundaries that make it
distributed—and, if so, what would Spock need to know?

## Unresolved questions

No answer to these questions is implied:

- Which parts of a service architecture are product or contract truth, and
  which are only changeable deployment choices?
- What must remain independently authoritative, transactional, available, or
  fallible when physical execution is collapsed locally?
- Which observable properties of calls, messages, time, ordering, retries,
  duplication, partial completion, recovery, and version skew would require
  fidelity?
- What may be simulated deterministically, and what can be validated only
  against real distributed infrastructure?
- How would several Spock-owned boundaries differ from one Spock authority
  calling external services?
- What relationship, if any, should exist between product topology and
  deployment topology?
- How could a local execution and a production realization be compared for
  conformance without claiming operational equivalence?
- Does the missing concept, if any, belong to the language, contract artifact,
  project model, runtime, development tools, generators, or some combination?
- Where would faithful simulation become production theater?
- Would the value justify the additional concepts and failure surface?

## Adjacent records, not answers

Existing records contain parts of the motivation but do not ask or answer the
system-level question:

- [RFD 0022](../rfd/0022-spock-framework.md) establishes the current
  one-project, one-command framework around one authority and explicitly
  excludes generalized microservice orchestration and multiple authority
  databases.
- [RFD 0019](../rfd/0019-external-plane.md) asks whether external services
  should exhibit production-shaped latency, failure, and eventual consistency
  in a prototype. It is a discussion draft about external boundaries, not a
  topology of multiple Spock authorities.
- [RFD 0001](../rfd/0001-effects-once-extern.md) sketches typed foreign
  boundaries, controllable failure scenarios, sagas, compensation, and
  simulated time. It is also a discussion draft and defines no current syntax.
- The [Uber-like product harness](../../examples/uber/PRD.md) preserves
  service-wide races, asynchronous work, independent providers, unknown
  outcomes, and recovery requirements while deliberately refusing to choose a
  deployment architecture.
- The [design principles](../governance/design-principles.md) require one
  authoritative representation, named effect and failure boundaries, prototype
  truth rather than production theater, and explicit limits. A future study
  would have to reconcile all four.

These records make the question relevant. They do not constitute prior-art
research, a feasibility argument, or a design.

## Non-claims

This seed does not claim:

- that microservices are universally correct or should be Spock's semantic
  primitive;
- that product planes, bounded contexts, repositories, processes, databases,
  queues, or deployment units should map one-to-one;
- that Spock should infer service boundaries;
- that any particular source syntax, manifest shape, protocol, runtime,
  container, database, queue, or deployment platform should be used;
- that Spock should become a production orchestrator or hosting platform;
- that a local simulation can reproduce all physical, performance, scheduling,
  scaling, or emergent failure behavior of a real distributed system;
- that production artifacts can or should be generated from such a model;
- that the idea is feasible, desirable, prioritized, or compatible with the
  current language direction; or
- that the current singular authority model has been reopened or changed.

## If pursued

This seed is not a required process stage. The next formal step would be a
language-problem issue identifying concrete use cases, the current limitation,
and evidence that current Spock cannot address it. Normal triage may route the
question to individual research, a working group, framework or tooling work,
external orchestration, or no further proposal.

If research is warranted, it should begin from concrete systems and
counterexamples rather than a preferred keyword or implementation. It would
need representative product architectures and failures, conventional expert
solutions, prior art, a defensible fidelity boundary, and experiments comparing
collapsed and genuinely distributed execution. Any language change still
follows the normal
[language-change process](../governance/language-change-process.md).

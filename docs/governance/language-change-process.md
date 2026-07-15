# Language change process

Spock is intentionally small, unconventional, and opinionated. Language
evolution therefore starts with a demonstrated problem and proceeds through
published judgment. Implementations and syntax sketches may be evidence, but
the first working patch or most popular spelling does not decide adoption.

This document defines the gate. The [RFD index and
template](../rfd/README.md) define the proposal artifact in detail.

## Exploration before 1.0

While Spock is pre-1.0, the project expects multiple paths to be tried and
discarded. Anyone may share competing designs, syntax sketches, experiments,
forks, and draft prototypes, including work that conflicts with current
behavior or a published principle. Exploration needs no committee sponsor.

An experiment must identify the question it tests and prominently mark its
issue, pull request, and exposed surface as experimental, unstable, and
non-normative. With maintainer agreement, it may merge only when it is isolated
and opt-in, cannot change default language behavior, the normative
specification, or conformance expectations, and has a clear removal or
graduation path. It creates no precedent, compatibility promise, reserved
syntax, or design authority.

These additional controls apply only when experimental code is proposed for
merge into `main`, not when someone shares an issue, sketch, fork, branch, or
draft pull request:

- an explicit experimental entry point or feature flag is off in ordinary
  builds and releases, and use produces a visible instability warning;
- experimental tests, documentation, examples, and generated surfaces stay in
  clearly marked experimental locations, outside normative conformance
  fixtures, default tutorials, standard examples, public generated contracts,
  and compatibility matrices;
- validation shows the default parser, checker, runtime, CLI, snapshots, and
  conformance suite are unchanged; and
- a linked issue names an owner and a review date or release checkpoint, when
  the experiment must be removed, explicitly renewed, or proposed for
  graduation.

This process gates adoption, not curiosity. An accepted RFD is required before
an experiment graduates into supported or default language behavior. Reaching
1.0 will raise the compatibility bar; it need not end technical exploration.

## What requires the process

An RFD is required before adopting a change into supported or default behavior
that affects:

- syntax, grammar, name resolution, typing, static semantics, or runtime
  semantics;
- language-level diagnostics policy or compatibility guarantees;
- standard concepts, builtins, modules, effects, or protocol behavior;
- the normative specification or public contract IR;
- generated public surfaces when their behavior is part of the language
  contract;
- the boundary of authoritative state between Spock and Uhura; or
- the doctrine, design principles, or this language-change process.

An ordinary conformance fix, internal refactor, test, tooling improvement, or
documentation clarification does not require an RFD if it leaves the public
contract unchanged. The Language Design Committee resolves ambiguous cases.

Experiments may support an informal study, working group, or RFD, but none of
those venues changes the adoption boundary above.

## Stage 1: state the problem

Anyone may open a language-problem issue. It should give concrete use cases,
current limitations, examples and counterexamples, and the reason existing
Spock concepts are insufficient. A syntax preference or patch alone is not a
problem statement, though either may accompany the problem as exploratory
evidence.

The committee triages the issue as:

- a bug or ordinary implementation task;
- duplicate or already answered;
- covered by a current adoption default with published rationale;
- outside Spock's direction;
- needing individual research;
- suitable for a temporary working group; or
- mature enough to seek an RFD sponsor.

The committee triages as capacity allows. An issue may remain useful public
evidence even when the committee does not charter a WG or begin formal RFD
review.

## Stage 2: study when needed

The committee may charter a temporary
[working group](../working-groups/README.md) when meaningful progress requires
organized prior-art research, experiments, or comparison of credible
approaches.

A WG has a bounded charter, committee sponsor, named deliverables, evidence
standard, and target close date. It publishes meeting notes, studies, and a
final report. Its conclusions are non-normative. A WG cannot accept its own
recommendation or reserve a language area.

Not every RFD needs a WG. The committee should use the lightest study structure
that can make the decision responsible.

## Stage 3: obtain sponsorship

A voting Language Design Committee member must sponsor a formal RFD.
Sponsorship means that the problem is within Spock's direction and mature
enough to consume review time. It does not mean the sponsor endorses the
answer.

The proposal also names a shepherd responsible for completeness, progress,
and treatment of objections and alternatives. Sponsor and shepherd may be the
same person. The committee or its RFD Editor assigns the number.

Without sponsorship, a design remains an issue, study, or informal idea.
An explicitly experimental pull request may remain as evidence. Maintainers
route an unsponsored pull request back to the problem or study path when it asks
to introduce or redefine supported or default behavior or the normative
specification. They may close it as a merge request; formal design review
begins after sponsorship, and that disposition does not determine the
experiment's technical merit.

## Stage 4: write the RFD

The author copies [the RFD template](../rfd/TEMPLATE.md). A reviewable proposal
must include:

- the concrete problem, goals, and non-goals;
- relevant evidence and prior art;
- exact semantics, invariants, and failure behavior;
- realistic examples and counterexamples;
- alternatives, including no language change;
- compatibility, security, operational, and implementation consequences;
- a conformance-test and specification plan; and
- unresolved questions.

The RFD must evaluate itself against the
[design principles](design-principles.md). “It feels cleaner” may be a useful
observation, but it is not a complete rationale.

## Stage 5: public review

When the shepherd and committee agree that the draft is complete, the RFD
enters review with public start and end dates. Review lasts at least **10
calendar days**. A substantial design change may restart review so the design
eventually decided is the one the community could inspect.

Review seeks:

- missing use cases or counterexamples;
- semantic ambiguity;
- compatibility and ecosystem consequences;
- conflicts with doctrine or existing features;
- stronger alternatives and relevant prior art; and
- implementation or conformance risks.

Review is not a popularity vote. Comment counts, reactions, status, employment,
or implementation effort do not grant design authority. The committee weighs
the substance of the record.

Meetings may be offline, but reasoning relied upon for the decision must be
published before the decision becomes binding. Sensitive conduct or personnel
matters remain private and are not part of the design record.

## Stage 6: decide

The committee uses the quorum and threshold rules in
[GOVERNANCE.md](../../GOVERNANCE.md). It records one of:

- accepted;
- rejected;
- deferred;
- withdrawn; or
- superseded, when a later RFD replaces an earlier decision.

If the proposal needs further work instead of a disposition, the committee
returns it to draft. That is a lifecycle transition, not an additional
decision status.

The RFD's durable decision record states the rationale, addresses material
objections, names tradeoffs, records recusals and any vote, and preserves
substantive dissent. Rejection is not misconduct and does not imply that the
problem was raised in bad faith.

The constrained Design Steward role may resolve a genuine aesthetic tie only
after the committee records that doctrine, evidence, semantics, compatibility,
security, and cost do not materially distinguish the remaining choices.

## Stage 7: implement

Acceptance authorizes graduation into supported implementation; it does not
promise priority, staffing, release timing, or immediate inclusion. An
implementation issue tracks the work, and implementation pull requests link
both it and the accepted RFD.

Design status and implementation status are independent:

| Decision | Meaning |
| --- | --- |
| draft | No committee decision |
| review | In announced public review |
| accepted | Authorized to graduate into supported implementation |
| rejected | Considered and declined |
| deferred | Waiting on named evidence, timing, or dependency |
| withdrawn | Ended before a terminal committee decision |
| superseded | Replaced by a later RFD |

| Implementation | Meaning |
| --- | --- |
| unplanned | No supported implementation is scheduled or actively tracked; before acceptance, graduation is unauthorized |
| planned | An accepted design has an implementation plan |
| in-progress | Work is active but the language contract is not reconciled |
| implemented | Code, conformance tests, documentation, and specification agree |

The implementation axis tracks adoption, not isolated experiments.
Accepted plus unplanned is valid. It means approved, not promised.

## Stage 8: reconcile the language

An RFD is permanent historical rationale, not the current language reference.
If an accepted RFD and the specification differ, the specification describes
what users may rely on today.

Implementation is complete only when:

1. code is merged;
2. conformance tests cover the behavior;
3. user-facing documentation is updated; and
4. every affected normative specification section matches the shipped
   behavior.

Later design changes use a follow-up RFD; accepted or rejected history is not
silently rewritten.

## Urgent fixes

A security or data-integrity incident may require a minimal, reversible fix
before ordinary review. This exception may restore or restrict existing
behavior; it cannot introduce a new language feature. Any lasting semantic
change still requires an RFD, and the non-sensitive reason for the urgent
action must be recorded.

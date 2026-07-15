# Language Design Committee

The Language Design Committee is Spock's permanent language-design authority.
Its legal and administrative relationship to the Project Lead, membership
rules, voting thresholds, conflicts, and appeals are defined in
[GOVERNANCE.md](../../GOVERNANCE.md). This charter describes how the committee
operates.

## Mandate

The committee guards a coherent language rather than aggregating preferences.
It decides:

- the doctrine and [design principles](design-principles.md);
- syntax, grammar, name resolution, types, static and runtime semantics;
- normative specification and compatibility policy;
- public contract IR and standard language capabilities;
- language-facing boundaries with runtimes, generators, protocols, and Uhura;
  and
- which questions merit organized study or formal RFD review.

The committee does not direct every implementation detail. Maintainers own
ordinary work within accepted language behavior.

## Seats and officers

The target is three to five voting seats. Current membership and terms are
recorded below so design authority is discoverable without reconstructing Git
history.

| Person | Seat | Term | Officer role |
| --- | --- | --- | --- |
| [Universe (@softmarshmallow)](https://github.com/softmarshmallow) | Interim voting member | Bootstrap; until superseded by a recorded appointment or term | Transitional Chair, Secretary, and Design Steward |
| Vacant | Voting member | — | — |
| Vacant | Voting member | — | — |
| Vacant | Optional voting member | — | — |
| Vacant | Optional voting member | — | — |

Spock is currently under the transparent single-member bootstrap provision in
[GOVERNANCE.md](../../GOVERNANCE.md#bootstrap-provision). A committee decision
made in this period must say so. This table must be updated in the same change
that records an appointment, role change, resignation, or removal.

The Chair facilitates but has no additional vote. The Secretary is responsible
for the public record. The Design Steward has only the constrained aesthetic
tie-breaking authority defined in governance.

## Regular work

The committee maintains these queues:

1. untriaged language-problem issues;
2. proposed and active working-group charters;
3. sponsored RFD drafts;
4. RFDs in public review;
5. implementation tracking for accepted RFDs; and
6. specification and decision-record reconciliation.

Cadence follows actual work rather than creating meetings for appearance. The
Chair schedules a meeting when an agenda has meaningful study or a decision
needs deliberation. Triage may be asynchronous if positions and results are
recorded.

## Triage dispositions

The committee gives a language-problem issue one of these dispositions:

- ordinary bug or implementation work;
- duplicate or already answered;
- covered by a current adoption default with published rationale;
- outside Spock's declared direction;
- individual research needed;
- candidate for a temporary working group; or
- mature enough to seek an RFD sponsor.

A disposition should link the applicable specification, principle, precedent,
or missing evidence. “Not tasteful” is not enough. The record must make the
judgment inspectable even when judgment cannot be reduced to a formula.

## Sponsorship and shepherding

Only a voting committee member may sponsor an RFD. Sponsorship means the
problem is in scope and ready to consume formal review time; it is not an
endorsement.

Every RFD also has a shepherd. The shepherd keeps the proposal moving, ensures
that required sections are complete, collects objections and alternatives,
and prepares the decision record. Sponsor and shepherd may be the same person,
but both responsibilities must be named.

The committee or an appointed RFD Editor assigns numbers and maintains the RFD
index. The RFD Editor performs editorial administration and gains no design
vote by holding that role.

## Working-group supervision

The committee charters a working group only for a bounded problem with named
questions, deliverables, evidence standards, and a target close date. It names
a committee sponsor but leaves study to the WG.

The committee may narrow, pause, or dissolve a WG that leaves scope, stops
producing useful evidence, or cannot meet its closure conditions. Receiving a
WG report does not accept its recommendation. Any language change proceeds
through an independently sponsored RFD and full public review.

## Decision quality

Before deciding an RFD, the committee confirms that:

- public review ran for at least 10 calendar days;
- the problem and non-goals are concrete;
- the design is evaluated against the published principles;
- alternatives, counterexamples, and compatibility are addressed;
- material objections have written answers;
- implementation and conformance consequences are understood enough to
  authorize work;
- recusals and quorum are recorded; and
- the specification boundary is explicit.

Review is not a vote of the community. Reactions, polls, and comment counts may
show interest, but they do not establish correctness or coherence.

## Offline-first deliberation

The committee prefers focused offline or synchronous deliberation where it
improves study. The repository remains the durable public record.

An agenda and pre-reading should be published at least 48 hours before a
scheduled design meeting. Notes should be published within seven calendar days
using the [meeting template](meetings/0000-template.md). A decision is not
binding until its written disposition and rationale are public.

The committee does not publish conduct reports, private personnel matters,
security details, or legal advice. The Secretary records only a safe summary
when such a matter affects public project operations.

## Annual health review

At least once each calendar year, the committee records a short governance
health review covering:

- occupied seats and approaching term dates;
- attendance, responsiveness, and conflicts;
- whether bootstrap or vacancies are concentrating authority;
- open RFD and WG load;
- overdue meeting notes or specification reconciliation; and
- whether the reporting and appeals paths provide meaningful independence.

The review may conclude that no change is needed, but it must name unresolved
governance risks.

# Spock Governance

Spock is both an implementation and a language contract. This document says
who may make project decisions, how that authority is constrained, and how the
project moves from its present founder-led stage to a durable committee.

The [Code of Conduct](CODE_OF_CONDUCT.md) governs behavior. It is not a design
test. The [design principles](docs/governance/design-principles.md) and
[language-change process](docs/governance/language-change-process.md) govern
language evolution.

## Authorities

### Project Lead

The Project Lead holds the repository's administrative responsibility:
maintainer access, releases, infrastructure, security response, legal
compliance, and continuity of the project. The current Project Lead is
[Universe (@softmarshmallow)](https://github.com/softmarshmallow).

The Project Lead may block an action that creates a concrete security, legal,
release-integrity, or repository-integrity risk, and must publish the
non-sensitive reason. This is not a general power to bypass the language-change
process. Adopting a new language design as supported or normative still
requires a Language Design Committee decision.

The Project Lead names a successor in a public repository record. If the role
becomes vacant without a successor, the active maintainers select an interim
lead by majority vote and record the result. After bootstrap, the same
continuity rule applies if the Project Lead has not responded to project
business for 90 consecutive days and then does not respond for another 14 days
after direct and public notice from the Secretary or two active maintainers.
Interim authority ends when the Project Lead resumes project business or a
successor takes office.

### Maintainers

Maintainers review and merge ordinary implementation, documentation, tooling,
release, and conformance work within accepted project direction. They may also
review isolated experimental work under the pre-1.0 containment rules in the
[language-change process](docs/governance/language-change-process.md#exploration-before-10),
including experiments that test current direction. They may classify work as a
bug fix, experiment, or language change. They should redirect useful prototypes
into a non-normative study when practical, and may close a pull request that
asks to merge unaccepted language behavior as supported or normative.

Maintainer status is granted or removed by the Project Lead after consulting
the relevant maintainers. Maintainer authority does not include accepting a
language change unless the maintainer also holds a voting committee seat.

### Language Design Committee

The Language Design Committee is the permanent design authority for Spock's
language, normative specification, public contract IR, standard capabilities,
compatibility policy, doctrine, and design principles.

The committee:

- triages language problems;
- charters and dissolves temporary working groups;
- sponsors, shepherds, reviews, and decides RFDs;
- maintains the specification and design principles;
- records decisions and substantive dissent; and
- appoints its officers and the Design Steward.

Working groups advise the committee. They do not make language decisions. An
accepted RFD authorizes graduation into supported implementation; only the
reconciled normative specification describes the behavior users can rely on
today.

The committee's operating charter is
[docs/governance/language-design-committee.md](docs/governance/language-design-committee.md).

### Community Moderators

The Project Lead may appoint Community Moderators to receive reports and
enforce the [Code of Conduct](CODE_OF_CONDUCT.md). Moderation is independent
from technical merit. A moderator may act on safety and participation but
cannot accept or reject a language design through enforcement.

## Committee composition

The committee's target size is three to five voting members. Members serve
two-year terms and may be reappointed. Terms should be staggered once the
committee has enough members for staggering to be meaningful.

Members are selected for demonstrated judgment across Spock's doctrine,
language design, implementation consequences, user needs, and ability to
reason in a public record. Membership is not a reward for contribution volume,
employment, or popularity.

### Bootstrap provision

At adoption, no additional committee members have yet been appointed. Until
the third voting member is seated:

- the current Project Lead is the sole interim voting member and transitional
  Chair while only one voting member is seated;
- the Project Lead may also perform the Secretary and Design Steward duties,
  but each capacity must be named in a decision record;
- while only one voting member is seated, the normal multi-member quorum floor
  is suspended, so that member can triage, charter work, and decide RFDs;
- once a second voting member is seated, founder-only authority ends: both
  non-recused members constitute quorum, and every formal language-design, WG,
  governance-amendment, or appointment decision requires both members'
  recorded assent; if either member is recused or unavailable, that decision
  is deferred;
- removals, appeals, and urgent safety matters follow their specific recusal
  rules and must disclose when independent project review is unavailable;
- every committee decision must state that it was made under this bootstrap
  provision; and
- open seats and the path to nomination must remain published.

This is a disclosure of founder authority, not a claim that one person
constitutes independent review. The committee must actively seek members who
can exercise independent judgment; it must not fill seats merely to ratify
existing preferences.

The bootstrap provision ends automatically when the third voting member
accepts appointment. It does not revive if membership later falls below three:
the vacancy and quorum rules apply instead, with the narrow reconstitution
procedure below available only if ordinary appointment quorum is lost.

### Reconstitution after bootstrap

After bootstrap, a regular member is treated as unavailable for quorum only
after both of these conditions are met:

- the member has not responded to any committee business for at least 90
  consecutive days; and
- the Project Lead or Secretary sends direct notice, publishes the same notice
  in the repository, and receives no response for another 14 calendar days. If
  neither officer responds, two active maintainers may publish and send that
  notice together.

Unavailability is an administrative status, not removal. The public record
must identify the notices and dates, and the status ends as soon as the member
resumes committee business. An unavailable member does not count as seated for
quorum, committee-size, or voting-threshold calculations while the status
lasts, but remains a member and keeps the remainder of their term. A return may
therefore put the committee temporarily above its target size; normal terms,
resignation, and removal rules still apply. A stated disagreement, recorded
abstention, vote against a candidate, or absence from a particular meeting
does not establish unavailability.

If vacancies, required recusals, or defined unavailability leave fewer than
two regular members able to form ordinary appointment quorum, decisions that
lack normal quorum remain paused. The Project Lead may use this section only
to restore appointment quorum and cannot invoke it to bypass opposition to a
candidate:

1. Publish a reconstitution notice naming the surviving members, vacancies,
   nomination route, and the decisions that are paused.
2. Keep each temporary or regular candidacy open for public comment for at
   least 14 calendar days and apply the normal judgment, conduct, and conflict
   criteria.
3. After consulting any surviving member and active maintainers, appoint only
   enough temporary Reconstitution Members to create a two-person,
   non-conflicted appointment panel. If a temporary member resigns, becomes
   conflicted, or fails to respond to direct and public notice of panel
   business for 14 calendar days, appoint only the minimum replacement needed
   under the same public-comment rule.
4. The panel may vote only to appoint regular committee members. An appointment
   requires at least two affirmative votes and unanimous recorded assent from
   all non-conflicted panel members. A temporary member cannot be a candidate
   for regular membership while serving on the panel.
5. Temporary members do not count toward quorum for any other decision, cannot
   act as Design Steward, and gain no authority over the specification,
   principles, RFDs, WGs, moderation, releases, or repository administration.
6. Temporary terms end automatically when two available regular voting
   members can again form ordinary appointment quorum. Those regular members
   then use the ordinary rules to fill the remaining vacancies.

Every action must be published as a reconstitution record. This procedure does
not revive founder authority or permit a language decision while ordinary
quorum is absent. Time-sensitive safety and repository-integrity actions remain
limited to the minimum reversible exception under the normal quorum rules.

### Bootstrap repository enforcement

The `main` branch requires a pull request, one approving review, Code Owner
review for governed paths, and resolution of review conversations. Force
pushes and branch deletion are disabled. While fewer than two independent,
write-enabled committee Code Owners exist, repository administrators retain
GitHub's branch-protection bypass because the sole bootstrap owner cannot
approve their own pull request.

Using that bypass does not waive the language-change process. A bypassed
change to a governed surface must state the bootstrap reason in its public
record and must still have every RFD or governance authorization the change
requires. Once enough independent, write-enabled committee Code Owners can
review one another, the committee must replace the individual bootstrap owner
with that team and decide publicly whether the administrator bypass remains
necessary.

## Appointment, terms, and removal

Anyone may nominate a candidate, including themselves, through the
repository's **Governance request** issue form. The nomination describes the
candidate's relevant work, judgment, expected availability, and disclosed
conflicts. The candidacy remains open for public comment for at least 14
calendar days.

After that period, appointment requires two-thirds approval of all
non-conflicted seated committee members. When only the sole interim member is
seated, that member may appoint a second candidate after the same public
period. In the two-member bootstrap phase, both members must approve the third
candidate. The candidate must affirm the Code of Conduct, disclose material
conflicts, and accept the term in the public record.

A member may resign at any time. Missing three consecutive scheduled committee
meetings without notice, or failing to respond to committee business for 90
days, triggers a membership review rather than automatic removal.

Removal for sustained inactivity, breach of trust, undisclosed conflict, or
serious misconduct requires:

1. written notice of the grounds to the member;
2. a reasonable opportunity to respond;
3. recusal of the subject;
4. two-thirds approval of all other non-conflicted seated members; and
5. a public result with as much rationale as safety and privacy allow.

Once bootstrap has ended, removal requires at least two affirmative votes. If
urgent safety or repository integrity prevents a full process, the Project
Lead may suspend access temporarily while unconflicted people complete the
review. A conduct report and its evidence remain private.

## Officers

The committee selects a Chair and Secretary from its voting members for
one-year renewable terms.

The Chair schedules and facilitates meetings, confirms quorum and recusals,
and announces decisions. The Chair has no extra vote and cannot shorten public
review unilaterally.

The Secretary maintains agendas, attendance, decision records, votes,
dissent, and action items. Meeting facilitation and recordkeeping should be
held by different people once practicable. An absent officer may delegate a
meeting's duties to another non-conflicted member.

## Design Steward

The committee may appoint one voting member as Design Steward. This role gives
Spock a consistent final judgment on a narrow class of genuine aesthetic
ties. The Design Steward may choose among alternatives only after the
committee records that:

- each alternative is consistent with the doctrine and design principles;
- evidence, semantics, compatibility, security, and implementation cost do not
  materially distinguish them;
- the choice is primarily one of coherent language taste; and
- making one stable choice is better than leaving arbitrary variation.

The steward must publish the choice and short rationale. The steward cannot
waive sponsorship or review, create a feature, overrule a substantive
objection, change doctrine or design principles, amend the specification
alone, or turn an unresolved technical question into an aesthetic one. The
committee may overrule or remove the steward by the same threshold used for an
RFD decision.

## Meetings and public record

Committee and working-group meetings are offline-first: focused in-person or
private synchronous study is welcome. Private deliberation does not create
private language law.

For a scheduled design meeting:

- publish an agenda and linked pre-reading at least 48 hours beforehand when
  practicable;
- record attendees, absences, recusals, and whether quorum exists;
- distinguish discussion, non-binding conclusions, and formal decisions;
- preserve material objections and the evidence relied upon; and
- publish notes within seven calendar days.

A transcript is not required. A formal language decision is binding only when
its written disposition and rationale are published in the repository.
Sensitive security, conduct, legal, and personnel matters are handled
privately and are excluded or safely summarized.

Meeting rules and the record template live in
[docs/governance/meetings](docs/governance/meetings/README.md).

## Conflicts of interest

Every committee member and moderator must disclose a material financial,
employment, organizational, close personal, or other interest that a
reasonable observer could expect to affect judgment.

Authorship, sponsorship, or prior public support for an RFD is disclosed but
is not automatically disqualifying; a small language committee must be able to
do design work. A direct financial stake, a close personal dispute, a conduct
matter involving the member, or inability to judge the record fairly requires
recusal.

A recused person may provide requested facts but does not facilitate, count
toward quorum, or vote. If members disagree about recusal, the other
non-conflicted members decide by simple majority. The record names the
disclosure and recusal, but omits private details.

## Quorum and decisions

Consensus is preferred: the committee seeks a direction all members can
support or accept after material objections are answered. Silence is not
consent, and community reaction counts as evidence rather than votes.

The one-member and two-member rules in the bootstrap provision govern until
the third voting member is seated. The normal rules below apply after bootstrap
ends.

After bootstrap, quorum is both:

- a majority of all seated, non-recused voting members; and
- at least two voting members.

For ordinary administration, a quorate meeting decides by simple majority of
votes cast. For accepting or rejecting an RFD, changing governance or design
principles, appointing or removing a member, or overruling the Design Steward,
the committee first seeks consensus. If consensus remains unavailable after
the public review and at least two properly noticed deliberations, the Chair
may call a recorded vote. Passage requires two-thirds of all seated,
non-recused voting members. A tie, failed threshold, or absent quorum preserves
the status quo.

Asynchronous decisions use the same quorum and threshold. Each voting member
must explicitly record a position; lack of response is not an abstention that
helps form quorum.

When recusals make quorum impossible, the committee defers the decision. For a
time-sensitive administrative or security matter only, the Project Lead may
take the minimum reversible action needed and record the non-sensitive reason.
This exception cannot accept a language change.

## Appeals

### Technical and process decisions

Anyone materially affected may request one appeal within 30 calendar days of a
published decision through the repository's **Governance request** issue form.
The request must identify new evidence, a material factual error, an
unaddressed conflict, or a specific failure to follow the published process.
Repeating a preference or counting supporters is not an appeal.

Unconflicted committee members decide whether the appeal meets that threshold.
When possible, a member who did not shepherd the original decision leads the
review. The committee may affirm, reopen, or correct the decision and must
publish its rationale. A later proposal based on materially changed evidence
is a new RFD, not a second appeal.

### Conduct decisions

A person subject to moderation may appeal once within 30 calendar days by
replying privately to the enforcement notice. A moderator who did not handle
the original report reviews the appeal when one is available. The review
considers procedural fairness, proportionality, and new relevant evidence; it
does not disclose the reporter's identity or private evidence unnecessarily.

During bootstrap there may be no independent project moderator. The project
must disclose that limitation rather than route an appeal to an involved
person. GitHub-hosted abuse may be reported through
[GitHub Support](https://support.github.com/contact/report-abuse). Active
safety restrictions remain in place during an appeal.

## Changing this governance

Once bootstrap governance is adopted, a substantive amendment requires a
public pull request, at least 10 calendar days for comment, and the committee
threshold for governance decisions. Clerical fixes and link repairs may use
ordinary maintainer review.

Governance amendments cannot retroactively make an unrecorded language
decision valid.

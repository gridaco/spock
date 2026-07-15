# Governance

Spock separates community behavior, project authority, organized study,
language decisions, and the current language contract.

| Layer | Record | Authority |
| --- | --- | --- |
| Behavior | [Code of Conduct](../../CODE_OF_CONDUCT.md) | Community participation and enforcement |
| Project authority | [GOVERNANCE.md](../../GOVERNANCE.md) | Roles, membership, conflicts, quorum, decisions, and appeals |
| Design authority | [Language Design Committee](language-design-committee.md) | Permanent committee operating charter |
| Design rubric | [Design principles](design-principles.md) | Published tests for coherence with Spock's doctrine |
| Problem study | [Working groups](../working-groups/README.md) | Temporary, non-normative research |
| Proposed decision | [RFDs](../rfd/README.md) | Prospective sponsored proposals and durable dispositions; RFDs 0000–0023 remain legacy records |
| Current language | [Specification](../spec/) | Normative behavior users may rely upon |

The end-to-end gate is described in the
[language-change process](language-change-process.md). Recurring questions and
their current adoption rationale are recorded in
[current adoption defaults](commonly-declined.md).

Committee meeting records live in [meetings](meetings/README.md). A meeting can
produce study and reasoning, but it cannot silently change the language. A
formal design decision becomes binding only when its written disposition and
rationale are published.

## The short rule

Anyone may identify a language problem, sketch a design, or run a clearly
non-normative experiment. Only a committee-sponsored RFD enters formal
adoption review. Only an accepted RFD authorizes graduation into supported
language behavior. Only the reconciled normative specification describes
current Spock.

# Working groups

A Spock working group (WG) is a **temporary, problem-focused study group**
chartered by the Language Design Committee. It creates the time and structure
needed to investigate a difficult question without turning preliminary taste,
prototypes, or meeting consensus into language law.

WG work is non-normative and non-authoritative. A WG may gather evidence,
compare prior art, run experiments, document disagreement, and recommend that
an RFD be written. It cannot accept an RFD, amend the specification, authorize
graduation into supported language behavior, or claim permanent ownership of a
language area.

Anyone may conduct or publish an informal individual or group study without a
charter or committee permission. A charter is required only to use the official
Spock WG name, request sustained committee coordination, or create the formal
records below. Informal and official studies have the same non-normative
status.

## When to charter a WG

A WG is appropriate when a concrete language problem:

- crosses several existing language or implementation concerns;
- needs sustained prior-art study or reproducible experiments;
- has multiple credible approaches whose consequences are not yet understood;
  or
- benefits from a bounded group with named deliverables and a deadline.

A WG is not required for every RFD. It is not a general interest club, a
standing design faction, or a way to reserve a topic indefinitely.

## Formation

1. Start from a triaged language-problem issue.
2. Identify a Language Design Committee sponsor and a chair.
3. Ask the committee Secretary to reserve the next unused four-digit WG
   number in the language-problem issue. Reservation is administrative and
   does not approve the WG.
4. Copy [`0000-template/`](0000-template/) to `NNNN-short-problem-name/` using
   that number, then draft its `CHARTER.md`.
5. The committee approves, revises, or declines the charter. On approval it
   changes the charter status to `active` and links a public decision record.

Reserved numbers are never reassigned. A declined or withdrawn charter remains
as a short historical record so a number cannot conceal a prior proposal. A
bootstrap committee decision must say that it used the bootstrap provision.

The charter must name the problem, questions, scope, non-goals, deliverables,
membership, evidence standard, operating cadence, target close date, and
closure criteria. The sponsor connects the WG to the committee but does not
turn WG conclusions into committee decisions.

## Lifecycle

| Status | Meaning |
| --- | --- |
| `proposed` | A charter is under committee consideration. |
| `active` | The committee has chartered the WG and study is underway. |
| `reporting` | Study is closed to new scope and the final report is being prepared. |
| `closed` | The committee has received the final report or dissolved the WG. |
| `declined` | The committee considered the charter but did not create the WG. |
| `withdrawn` | The proposer ended the charter request before a committee decision. |

WGs are expected to close. The committee may narrow, pause, or dissolve a WG
that leaves its charter, stops producing evidence, cannot meet its closure
conditions, or is no longer useful.

## How a WG works

WG meetings are preferably focused working sessions and may be held offline.
Offline-first must still produce a public design record. Each substantive
meeting gets a note under `meetings/YYYY-MM-DD.md` containing:

- date, attendees, facilitator, and recorder;
- agenda and pre-reading;
- evidence and alternatives considered;
- disagreements and unresolved questions;
- non-binding conclusions; and
- action items with owners and target dates.

A transcript is not required. Reasoning that will be cited by a final report
or RFD must be written down in the repository. Conduct or personnel matters are
handled privately through the [Code of Conduct](../../CODE_OF_CONDUCT.md), not
published in meeting notes.

Studies live under `studies/` and should be reviewable independently of a
meeting. Prefer primary sources, reproducible experiments, realistic examples
and counterexamples, and explicit limitations. A WG may use prototypes to
learn; prototypes confer no design status.

## Outputs and closure

Every WG ends with `FINAL-REPORT.md`, even if it reaches no recommendation. The
report summarizes the evidence, viable options, tradeoffs, dissent, unresolved
questions, and one of these outcomes:

- recommend that a sponsor advance a specific RFD;
- recommend no language change;
- recommend a later study after named evidence or dependencies exist; or
- report that the WG could not reach a useful conclusion.

The committee receives the report and closes the WG in a linked public action
record. A bootstrap action must say that it used the bootstrap provision. If
an RFD follows, it must independently obtain a sponsor and pass the full [RFD
process](../rfd/README.md), including public review. WG participation or
consensus gives no special vote in that decision.

## Directory shape

```text
working-groups/
├── README.md
├── 0000-template/
│   ├── CHARTER.md
│   ├── studies/
│   │   └── README.md
│   ├── meetings/
│   │   └── YYYY-MM-DD.md
│   └── FINAL-REPORT.md
└── NNNN-short-problem-name/
    ├── CHARTER.md
    ├── studies/
    ├── meetings/
    └── FINAL-REPORT.md
```

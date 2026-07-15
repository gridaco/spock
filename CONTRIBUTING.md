# Contributing to Spock

Spock welcomes bug reports, design sketches, experiments, studies,
documentation, tooling, and implementation work. It is early and pre-1.0;
trying and discarding paths is part of the work. Changes to the public contract
follow a design process before they become supported language behavior.

All participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).
Technical authority is governed separately by [GOVERNANCE.md](GOVERNANCE.md).
Agreement with a proposal is never a condition of respectful participation,
and respectful participation never entitles a proposal to acceptance.

## Explore before 1.0

Contributors may test competing syntax, semantics, abstractions, and
implementations in issues, forks, branches, informal studies, working groups,
or draft pull requests. Exploration needs no committee sponsor. State the
question being tested and mark the issue, pull request, and exposed surface
prominently as experimental, unstable, and non-normative.

With maintainer agreement, an isolated experiment may merge when it is opt-in,
cannot change default behavior, the normative specification, or conformance
expectations, and has a clear removal or graduation path. It creates no
precedent, compatibility promise, reserved syntax, or design authority. An
accepted RFD is still required before it becomes supported language behavior.
Experimental code proposed for `main` must also satisfy the containment,
validation, ownership, and expiry checklist in the
[language-change process](docs/governance/language-change-process.md#exploration-before-10).

## Choose the right entry point

### Ask a usage question

Open a usage-question issue for help understanding current Spock behavior,
tooling, or documentation. Include a small example and the version or commit
you are using. Questions do not become language proposals unless they expose a
concrete limitation and are restated through the language-problem path.

### Make a governance request

Use the governance-request issue form for a committee membership nomination,
a technical or process appeal, a proposed governance amendment, or a
correction to an official decision record. A substantive amendment is adopted
only through the public pull request, 10-day review, and committee decision
required by [GOVERNANCE.md](GOVERNANCE.md#changing-this-governance). Conduct
reports and conduct appeals stay private and must follow the Code of Conduct
instead.

### Report a bug

Open a bug issue with a minimal reproduction, expected behavior, actual
behavior, and the Spock version or commit. A behavior that contradicts the
current specification is normally a bug, not a language proposal.

Security-sensitive reports must not be filed publicly. Email
[hello@grida.co](mailto:hello@grida.co) with the subject “Spock security
report.” This is the same shared organization mailbox—and has the same current
confidentiality limitation—described in the
[Code of Conduct](CODE_OF_CONDUCT.md#current-reporting-limitation).

### Raise a language problem

Begin a language-problem issue with the problem. State:

- a concrete use case;
- what cannot be expressed, checked, or understood today;
- examples and counterexamples;
- why existing Spock concepts do not address it; and
- compatibility or ecosystem consequences you already know.

A preferred spelling may be useful evidence, but it is not a language problem
by itself. Candidate syntax and prototypes are welcome after the constraint or
failure they test is clear.

The Language Design Committee may decline, defer, request individual research,
charter a working group, or invite a sponsored RFD. The committee triages as
capacity allows; an issue can remain useful public evidence without becoming a
WG or RFD.

### Propose a language change

Do not present code or specification text as supported language behavior before
acceptance. A change to default syntax, static semantics, runtime semantics,
the public contract IR, standard capabilities, compatibility rules, or the
normative specification requires:

1. a voting committee sponsor;
2. a numbered RFD;
3. the public review and decision described in the
   [language-change process](docs/governance/language-change-process.md); and
4. a separate implementation issue after acceptance.

An explicitly labeled draft experiment may accompany a language problem,
study, or RFD without sponsorship. Maintainers should preserve useful evidence
by redirecting or reclassifying solution-first work when practical. If a pull
request asks to merge unaccepted behavior as the language default or normative
specification, maintainers may redirect it or close it as procedurally
premature. That is not a judgment that the exploration was improper.

Community feedback supplies evidence, use cases, and objections. It is not a
popularity vote. The committee evaluates proposals against the published
[design principles](docs/governance/design-principles.md).

### Join a working group

Working groups are temporary research bodies chartered by the Language Design
Committee. They study a bounded problem and publish evidence; they do not set
the language or accept their own recommendations. See the
[working-group index](docs/working-groups/README.md).

### Implement accepted or ordinary work

Pull requests that graduate a language change into supported behavior must cite
both the accepted RFD and its tracking issue. Ordinary bug fixes, isolated
experiments, tests, documentation corrections, internal refactors, and tooling
changes do not need an RFD when they preserve the specified language contract.

Open or claim an issue when you want project review, coordination, adoption, or
maintenance for nontrivial implementation. You may prototype first; linking
the result to an issue helps others understand and build on the evidence.

## Pull-request expectations

Keep a pull request focused on one problem. It should:

- explain the user-visible and internal effects;
- link the governing issue and, when required, accepted RFD;
- include tests for changed behavior;
- update user documentation and the specification when the accepted change
  reaches implementation;
- preserve unrelated local or generated changes; and
- disclose known limitations or follow-up work.

Run the focused tests for the subsystem you touched and the repository's
formatting checks. Maintainers may request broader validation when a change
crosses the parser, checker, contract IR, runtime, generated interfaces, or
Uhura boundary.

An accepted RFD authorizes graduation into supported implementation; it does
not promise scheduling, staffing, or immediate inclusion in a release.
Graduation is complete only when code, conformance tests, documentation, and
the normative specification agree.

## Review and decision records

Review comments should distinguish blocking correctness or doctrine concerns
from optional preferences. For a language change, durable rationale belongs in
the RFD or its decision record, not only in a pull-request thread or private
meeting.

Offline discussion is welcome. No language decision becomes binding until the
committee publishes the conclusion and reasoning in the repository.

## License

By contributing, you agree that your contribution is licensed under the
repository's [MIT License](LICENSE), except where a file states another
license. The Code of Conduct retains the attribution and license stated in that
document.

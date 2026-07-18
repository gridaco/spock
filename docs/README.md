# Spock documentation

Spock is both an implementation and a language contract. Its documentation is
split by authority so that an experiment, example, or accepted future design
cannot silently redefine the language people are using.

The published documentation is available at
[spock.sh/docs](https://spock.sh/docs/). This directory remains its canonical
source.

New to Spock? Start with the guides: [install](start/install.md),
[quickstart](start/quickstart.md), then the
[tutorial](start/tutorial.md). The [language guide](language/) teaches the
language progressively; the [reference](reference/) covers the CLI, HTTP
protocol, error codes, and project manifest; and [project status](status.md)
states exactly what is stable, experimental, and deliberately absent in v0.

## Authority map

| Area | Purpose | Authority |
| --- | --- | --- |
| [Governance](../GOVERNANCE.md) | Defines who may make project and language decisions, and by what procedure. | Authoritative for decision-making process. |
| [Specification](spec/) | Defines the behavior of the current Spock language and its public dialects. | Normative for current behavior. |
| [Guides](start/) | Explain how to install and use current Spock. | Non-normative. A guide illustrates the specification and cannot amend it; the specification governs where they disagree. |
| [RFDs](rfd/) | Preserve legacy design records plus prospective proposals, decisions, and authorized future direction. | Under the new process, an RFD is a decision record, not the current specification. An accepted RFD authorizes graduation into supported implementation but does not make a feature current. |
| [Problem seeds and informal studies](studies/) | Preserve optional pre-research questions and collect individually organized evidence and evaluation methods. | Non-normative and non-authoritative. A seed or study may inform an issue, WG, or RFD but cannot select a design or amend the specification. |
| [Working groups](working-groups/) | Conduct bounded, organized study of a language problem. | Non-normative and non-authoritative. A WG may recommend an RFD but cannot accept one or amend the specification. |
| [Examples](../examples/) | Demonstrate and test concrete uses of Spock. | Non-normative, even when an example is executable or used by tests. |

[CONTRIBUTING.md](../CONTRIBUTING.md) routes proposed changes through these
areas. [CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) governs conduct in all of
them, including offline meetings; it does not grant design authority.

## Reading conflicts

These documents have authority in different domains rather than forming one
flat hierarchy:

- for what current Spock means, read `docs/spec/`;
- for how a decision may be made, read `GOVERNANCE.md` and the applicable
  process document;
- for why a direction was chosen or what may be implemented next, read its
  RFD;
- for an open pre-research question or evidence gathered before a proposal,
  read the relevant problem seed, informal study, or WG record; and
- for an illustration, read `examples/`, then verify it against the spec.

If an RFD, problem seed, informal study, WG note, or example conflicts with the
current specification, the specification governs current behavior. The
mismatch should be fixed, but it must not be resolved by treating non-normative
material as hidden language law.

## Changes to the language

Anyone may report a concrete language problem or publish a clearly labeled
pre-1.0 experiment. Adopting a change to syntax, semantics, compatibility, or
another supported public language contract requires committee sponsorship and
the [RFD process](rfd/README.md). Direct implementation may supply
non-normative evidence, but it cannot establish adoption. Substantial questions
may first receive a temporary [working group](working-groups/README.md) for
structured study.

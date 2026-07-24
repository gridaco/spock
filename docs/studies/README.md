# Problem seeds and informal studies

This directory contains optional pre-research problem seeds and individual or
group research that has not been chartered as a Spock working group and has not
entered the RFD process. Problem seeds preserve a need and a question. Studies
collect evidence, expose conflicts, define evaluation methods, and make later
design discussion reproducible.

Everything here is **non-normative and non-authoritative**. A problem seed or
study does not define current Spock behavior, record a Language Design
Committee decision, reserve syntax, select a design, or authorize
implementation. Current behavior is defined by the [specification](../spec/);
proposed language changes follow the
[language-change process](../governance/language-change-process.md).

## Problem seeds — research not started

A problem seed is an optional informal capture, not a stage in the
language-change process. It contains no research findings, feasibility claim,
preferred design, or proposal maturity. If the question is pursued, it first
goes through normal language-problem triage. If that triage calls for research,
the seed may be expanded or superseded using the study structure below.

- [Executable service topology](executable-service-topology-seed.md) — whether
  one local Spock project could preserve meaningful distributed-service
  boundaries and failure semantics without inheriting the physical friction of
  production infrastructure.

## Open studies

- [The `view` problem space](view-problem-space.md) — unresolved meanings,
  cross-cutting constraints, prior-art questions, and an evaluation harness for
  future proposals.

## Open design candidates

- [A conservative shape for `view`](view-design-candidate.md) — one concrete,
  non-normative bundle derived from the neutral problem-space study, persisted
  so review can falsify or refine it before any language proposal or
  implementation.

## Expected structure

An informal study should state its question, method, evidence, findings,
counterevidence, limitations, implications, and follow-ups. It should separate
observations from preferences and make experiments reproducible where
possible. If a study later becomes working-group input or motivates an RFD,
the formal record should link back to it; the study itself does not change
authority.

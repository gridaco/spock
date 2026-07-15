## Summary

<!-- What problem does this solve, and what changes for a Spock user or contributor? -->

## Change classification

<!-- Check exactly one. -->

- [ ] Language-contract-preserving implementation, tooling, documentation, tests, or refactor
- [ ] Isolated, opt-in, non-normative pre-1.0 experiment or prototype
- [ ] Committee-sponsored RFD draft, review revision, or decision record
- [ ] Implementation or specification synchronization authorized by an accepted RFD
- [ ] Non-normative working-group study or meeting record
- [ ] Governance or contribution-process change

## Language-change gate

Pre-1.0 experiments are welcome. A prototype may be reviewed as evidence when
it is prominently marked, isolated, and unable to change default behavior or
the normative specification. Graduation into supported syntax, semantics,
compiler-visible contracts, or specification text requires an accepted RFD. A
pull request that asks to merge unaccepted behavior as the language default
will be routed to the problem/RFD path and may be closed as a merge request.

Complete the applicable path:

- Related issue: <!-- Required for nontrivial work; link the bug, question, language problem, implementation issue, or governance request that governs this PR. -->
- Language-problem issue: <!-- Required for an RFD PR or intended language-contract change. -->
- Committee sponsor/shepherd: <!-- Required for an RFD PR. Sponsorship is permission to review, not endorsement. -->
- Accepted RFD: <!-- Required when this PR changes supported or default implementation or the normative specification under a language decision. Use a repository link; do not write "N/A" for such a change. -->
- Implementation tracking issue: <!-- Required when implementing an accepted RFD. -->
- Language-contract preservation: <!-- For ordinary non-experimental work, explain why current specified syntax, semantics, and docs/spec behavior remain unchanged. Non-normative studies should say why they establish no language behavior. -->
- Experiment boundary: <!-- For a prototype, link its issue; name the off-by-default entry point and warning, isolated tests/docs/surfaces, unchanged default validation, owner, and review/expiry date. -->
- Governance public-review dates: <!-- Required for a substantive governance amendment; the window must be at least 10 calendar days. -->
- Governance decision record: <!-- Required for a substantive governance amendment; link quorum, threshold, recusals, and bootstrap use. -->

## Before and after

**Before:**

<!-- Current observable behavior or limitation. -->

**After:**

<!-- New observable behavior. For an internal refactor, state that behavior is unchanged. -->

## Scope and tradeoffs

<!-- What is deliberately out of scope? Note compatibility, migration, security, performance, or maintenance consequences where relevant. -->

## Verification

<!-- List exact tests, commands, fixtures, screenshots, or manual checks. -->

- [ ] I read and followed [CONTRIBUTING.md](https://github.com/gridaco/spock/blob/main/CONTRIBUTING.md).
- [ ] I added or updated focused tests, or explained why no test applies.
- [ ] I updated user-facing documentation and the normative specification when authorized behavior changed.
- [ ] I kept unrelated changes out of this PR.
- [ ] If this is experimental, I prominently marked it and kept it isolated from default behavior, the normative specification, and conformance expectations.
- [ ] I have not presented a working-group study or community preference as an accepted language decision.

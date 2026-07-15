# Spock design principles

These principles turn the [README doctrine](../../README.md#doctrine) into a
review rubric. They do not replace judgment. They make judgment explainable,
consistent, and open to challenge with evidence.

Spock is pre-1.0; these principles are working commitments, not claims that
alternatives are improper. Contributors may challenge them with evidence and
test competing paths. The center should change deliberately rather than by
accident: a proposal that conflicts with a principle must identify the conflict
and show why revising the principle is better than making a local exception. A
substantive adopted change to these principles requires an RFD.

## 1. One authoritative representation

Product truth has one declared home. Data shape, policy, mutation, and effects
must not acquire independent authoritative copies across schema, service,
client, and tooling layers.

For framework projects, Spock owns durable authority and Uhura owns
presentation, experience transitions, and non-authoritative UI-session state.
Composition must not make the same fact authoritative in both languages.

Ask: What is the single source of truth, and which apparent copies are derived?

## 2. Put mutations beside the data they govern

The database is the durable source of truth, and deliberate mutations belong
in the same contract as the data, policy, and invariants they affect.
Convenient host-language plumbing must not create a second business-logic or
authorization layer that can drift.

Ask: Can the full rule be found and checked in one contract?

## 3. Add contract, not distance

Spock uses direct concepts such as table, view, and fn because they name the
underlying primitives honestly. A new abstraction must add checkable contract
information, not merely rename or conceal a mechanism.

Ask: What new invariant can the compiler or a consumer verify because this
concept exists?

## 4. Prefer the least powerful sufficient language

Spock is a closed contract language, not a general-purpose programming
language. Total knowledge of its constructs makes compilation, analysis,
conformance, generation, and agent use possible. Escape hatches may implement
an operation, but they must not replace its declared signature, write set,
effects, failures, or policy.

Ask: Can a smaller, more declarative construct express the needed contract?

## 5. State once; derive deterministically

If a layer can be derived from the contract, derive it. Serializers,
validators, protocols, client types, documentation surfaces, and runtime
artifacts should not require hand-maintained restatements of the same fact.
Generation should be deterministic and inspectable.

Ask: Does this proposal remove duplication, or establish another source that
must be synchronized?

## 6. Preserve the open and deliberate surfaces

Ordinary allowed reads and writes belong to the open, policy-governed surface.
Operations that coordinate records, enforce named invariants, or reach
external systems belong to the deliberate, explicitly declared surface.
Neither side should expand merely because one implementation path is familiar.

Ask: Is this behavior naturally policy-governed access or an intentionally
named operation?

## 7. Keep policy attached, complete, and testable

Authorization stays with the data and public shapes it protects. Security must
not depend on every caller remembering a convention, and the limits of an
enforcement mechanism should not deform the domain model invisibly.

Ask: Is every path to the protected fact mediated by one declared policy
model, with behavior that can be inspected and tested?

## 8. Name effects and failure boundaries

External systems, latency, partial failure, and irreversible actions are not
ordinary transactions. The contract must say when an operation crosses those
boundaries instead of hiding them in an implementation body. Algorithms may
escape to an appropriate host; the callable contract may not disappear with
them.

Ask: Which effects and failures can a caller and tool know before execution?

## 9. Keep the contract portable and inspectable

The contract is an artifact for runtimes, generators, clients, tools, humans,
and agents. It must not be trapped inside one host runtime or recoverable only
by executing arbitrary code.

Ask: Can an independent consumer understand the public contract without
embedding the implementation environment?

## 10. Borrow proven primitives before inventing vocabulary

Spock should compose established database, contract, protocol, and type-system
ideas where they fit. New vocabulary earns its place when existing concepts
cannot express the needed invariant cleanly—not because novelty is attractive.

Ask: What existing primitive was considered, and precisely where does it stop
being sufficient?

## 11. Optimize for prototype truth, not production theater

Spock builds a real, runnable backend to prove a product contract. It is not a
production server or deployment platform. Prototype shortcuts are acceptable
when they preserve the contract boundary and surface the decisions a
production implementation must make; they are not acceptable when they lie
about semantics, security, or authority.

Ask: Does this improve the fidelity of the product contract, or imitate
production infrastructure without strengthening the prototype?

## 12. Be explicit about limits

A contract language earns trust by naming what it cannot guarantee. Proposals
must expose uncertainty, non-goals, unsupported cases, and the point where a
different system takes responsibility.

Ask: Where does the model break, and is that boundary visible to users and
implementers?

## Applying the rubric

No proposal wins by tallying how many principles it mentions. Review should
identify the principles that actually govern the tradeoff, evidence for the
claimed fit, and the cost imposed on other principles.

Taste enters after substantive questions are answered. If multiple choices
are genuinely equivalent under doctrine, evidence, compatibility, security,
and implementation cost, the committee may use the constrained Design Steward
procedure in [GOVERNANCE.md](../../GOVERNANCE.md#design-steward) to keep the
language coherent.

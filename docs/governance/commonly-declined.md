# Current adoption defaults

This page records current adoption defaults for recurring questions. It is
deliberately small. It is not a blacklist, a permanent verdict, a substitute
for committee judgment, or a list of every rejected RFD. Spock is pre-1.0;
experiments that test these assumptions are welcome.

An item belongs here only when it follows directly from published doctrine or
has recurred enough that a stable explanation is useful. A default says what
the project is unlikely to adopt without new evidence, not what contributors
may explore. A materially new use case or new evidence can reopen any item.
Adding a substantive category requires the same public design scrutiny as the
principle it relies on.

## Solution-first language pull requests

**Current adoption default:** redirect a pull request that asks to change
supported or default behavior to a language-problem issue and the RFD path.

Code, grammar, or specification text is not a substitute for a sponsored RFD.
A clearly isolated, non-normative prototype may still be useful evidence. The
default rejects a route to adoption, not the act of exploration or necessarily
the underlying problem.

## Syntax-only preference

**Current adoption default:** request a concrete problem before standardizing
a spelling.

Personal familiarity, terseness, visual symmetry, or resemblance to another
language may motivate a sketch or experiment, but does not by itself justify a
canonical spelling. Show the ambiguity, repeated error, inaccessible concept,
or semantic distinction the syntax solves, with examples and counterexamples.

This does not pre-reject syntax work. Syntax with demonstrated semantic or
usability consequences is substantive and should be evaluated through an RFD.

## Synonyms and compatibility aliases without a migration need

**Current adoption default:** do not adopt without a migration need.

Multiple names or spellings for the same concept make the grammar, teaching
surface, diagnostics, formatters, and generated artifacts larger without
adding contract information. A real compatibility transition may justify a
temporary alias, but it must include a removal or stabilization policy.

## Renaming direct primitives to fashionable abstractions

**Current adoption default:** do not adopt absent new checkable meaning.

Names such as model, resource, resolver, controller, or action should not
replace table, view, or fn merely to resemble a familiar framework. Spock uses
direct underlying primitives on purpose. A proposed concept that carries a
distinct invariant is a different question and deserves substantive review.

## Making client state authoritative in both Spock and Uhura

**Current adoption default:** preserve one declared authority.

Framework composition does not merge authority. Spock owns durable product
truth; Uhura owns presentation, experience transitions, and non-authoritative
UI-session state. A cross-language protocol may move or project information,
but no fact becomes authoritative in both systems for convenience.

This does not pre-reject research into synchronization, offline interaction,
or ownership transfer. Such work must define one authority at each point in
the protocol.

## Production-runtime features with no contract benefit

**Current adoption default:** outside current adoption direction.

Spock is a prototype language and runnable contract, not a production hosting
platform. Scaling, orchestration, deployment, and production availability work
must show how it strengthens or validates the contract rather than merely
imitating production infrastructure.

This does not reject generation of production-grade artifacts or explicit
production boundaries; both may be central to a future RFD.

## General-purpose computation in the language core

**Current adoption default:** do not adopt when the effect is to make the
contract an arbitrary program.

Spock follows the rule of least power so the entire contract remains
checkable, portable, and generatable. Algorithms may use a declared escape
hatch, but signatures, effects, write sets, failures, and policy remain in the
contract.

This does not settle which bounded expressions, effects, or host interfaces
Spock should support. Those are substantive design questions.

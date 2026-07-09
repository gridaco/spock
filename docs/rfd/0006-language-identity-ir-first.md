# RFD 0006 — Language identity and the IR-first architecture

Status: accepted direction. The identity claims (§1–§5) and the Option-A
verdict (§7) are fixed; the IR's concrete encoding and the conformance
timeline are open.

The corpus asserts two things it never argues. The README calls Spock a
language outright ("it is a language, with everything a compiler earns you" —
"What Spock is not"), and it promises the contract is portable ("clearly
enough that multiple backends could implement them" — "V1 direction").
Underneath sits an unresolved instinct: executable JSON documents that carry
logic have existed forever, and nobody calls them languages. If we cannot say
precisely what makes Spock different, the doctrine's central promise — define
the contract others implement — is a slogan, not a property. This RFD makes
the property precise, then shapes the implementation architecture to defend
exactly that property.

## 1. "Language" is the front; "runtime" is the back

The instinct that trips people up is judging languagehood by what executes the
program. That looks at the wrong end. A **language** is a property of the
front: a notation with a grammar, a semantics — what programs *mean* — and a
checker that reasons about programs. A specification. A **runtime** is a
property of the back: whatever machinery is present at execution time — the
store, the transaction engine, the effect handlers, the server.

Every language has a runtime, and the runtime never confers languagehood —
nor denies it. C's runtime is `crt0` and the operating system. Java's is the
JVM. The decisive precedent for Spock: **SQL's runtime is the RDBMS** — the
canonical language whose runtime is a database behind an interface. A runtime
that is "a local database plus an interface" (RFD 0002, "The v0 cut": SQLite
embedded in-process, "hidden behind the language") places Spock squarely in
SQL's family, not outside the category.

Two words should be used precisely from day one. The v0 **interprets** Spock
against an embedded runtime; it does not transpile. Transpilation names
source-to-source compilation into another high-level language, and in this
project it only ever describes the future `spock2sql` back-end (README, "V1
direction", path 1). And "compiles to a runtime" disqualifies nothing — it
merely names which end you are looking at.

## 2. The four-axis test of languagehood

What separates a language from a format — from an "executable JSON with
logic" — is not one property but four. A format typically passes one or two of
these. A language passes all four, and the second is decisive.

1. **Concrete syntax.** A surface notation designed for humans to author,
   read, and diff, parsed into an AST. A serialized AST is not this. (One
   caveat keeps the axis honest: homoiconic Lisp is nearly its own AST and is
   certainly a language — so axis 1 is suggestive, not decisive. Axis 2 is
   decisive.)
2. **Implementation-independent semantics.** Meaning fixed by a spec, such
   that a second party could build a conforming implementation *without
   reading the first interpreter's source*. A language is defined by a spec; a
   format is defined by its one implementation.
3. **A checker that reasons over the space of programs.** Not "does this
   instance match its shape," but reachability, exhaustiveness, dead code —
   judgments quantified over *all* expressible programs.
4. **A generative, compositional grammar.** A finite grammar generating
   unbounded well-formed programs, where the meaning of the whole composes
   from the meaning of the parts.

## 3. The decisive axis is the mission restated

Axis 2 is not a philosophical nicety; it is Spock's mission in different
words. "It defines the contract others implement" (README, "What Spock is
not") and "multiple backends could implement them" (README, "V1 direction")
*are* the re-implementability test. Languagehood is entailed by the doctrine,
not decorative on top of it.

The LINQ lesson is axis 2 narrated as a failure (README, "The LINQ lesson"):
LINQ's query semantics were trapped inside one host runtime, could not be
handed to another client or backend, and the idea never escaped — a strictly
weaker but portable model took the seat. Spock's design answer, "the contract
is the artifact, portable to any client and any backend," is a direct response
to axis 2.

One boundary worth restating so it is never argued by accident:
Turing-completeness is not required for languagehood. SQL's core, regular
expressions, and Datalog are not Turing-complete and are unambiguously
languages. Spock's deliberate non-Turing-completeness (README, "What Spock is
not"; the rule of least power) *strengthens* axis 3 — a closed world is
exactly what lets the checker reason over all programs (RFD 0002, "the atom":
in a closed world, "what can happen here?" is computable).

## 4. What to call it

In established taxonomy, Spock is an **external, declarative DSL**: its own
syntax and toolchain, as opposed to an *internal* (embedded) DSL hosted inside
a general-purpose language — which is what LINQ was; the external/internal
distinction is the LINQ lesson compressed to two words. Its computational
model is the one the corpus already names: a language for **guarded state
transitions over durable state** (RFD 0002, "the atom"). Its artifact's genre
is already coined too: an **executable specification** — the executable PRD
(README, "A prototype language, on purpose").

> Spock is an external, declarative DSL for guarded state transitions over
> durable state, defined by an implementation-independent contract that
> multiple runtimes can implement — an executable specification language.

## 5. Why "executable JSON with logic" was never a language

Apply the four axes to that earlier pattern and the difference stops being a
feeling:

1. It *was* the serialized AST — there was no concrete syntax designed for a
   human to author.
2. It meant whatever its one interpreter did. No spec; no second conforming
   implementation was even possible. The interpreter was the definition.
3. It was validated as a document against a schema — never analyzed as a
   program. Nothing reasoned about reachability or exhaustiveness over the
   space of things it could express.
4. It was a configured shape, not a generative grammar with compositional
   meaning.

> Executable JSON is an abstract syntax with an implementation but no
> independent semantics and no concrete syntax. A language is a concrete
> syntax with an independent, specifiable, compositional semantics that any
> number of runtimes can implement — plus a checker that reasons over the
> whole space of programs the grammar generates.

The intuition behind why it always felt different: nobody ever wrote a grammar
for a human to author, a spec for a second implementer, or a checker that
reasoned about what the logic could do. One interpreter was built and fed
data. Spock crosses the line because its doctrine — define the contract others
implement — forces all four properties into existence.

## 6. The two architectures

There are exactly two ways to implement a language like this.

- **Option A — static runtime, language lowers to IR.** Write *one* runtime.
  The front end lowers `.spock` into a checked **intermediate
  representation**; the fixed runtime loads the IR and executes it. Editing
  source means re-lowering and reloading against the same runtime. The
  precedents are the ones this corpus already leans on: SQL → plan → executor;
  Java → bytecode → JVM; Wasm.
- **Option B — generate the runtime.** For each program, emit bespoke code
  specialized to that contract — SQL DDL and functions, or native code — then
  compile and run *that*. `spock2sql` (README, "V1 direction", path 1) is a
  flavor of B.

The README's three V1 paths map cleanly: path 2 (small runtime, no SQL first)
is Option A and the opening move; path 1 (`spock2sql`) is Option B as a later
back-end; path 3 (Rust runtime behind a Wasm boundary) is Option A with a
heavier runtime. The pipeline trunk is shared regardless — only the tail
differs:

```text
 .spock text
     │  lex · parse
     ▼
    AST
     │  check — the totality and reachability lints (RFD 0002, RFD 0004)
     ▼
     IR                ← the specified, versioned artifact.
   ┌─┴───────────┐        Languagehood (axis 2) lives here.
 interpret      lower
 (A — v0)       (B — later)
   │             │
 SQLite         spock2sql → Postgres · Rust/Wasm gateway
 runtime
```

## 7. Verdict: A first, A forever as the core, B as a back-end

Four reasons, each grounded in doctrine rather than taste.

**The prototype mission is play.** The product promise is
edit-and-immediately-run (README, "V1 direction": "a small runtime that makes
contracts playable is the product"). Option A's loop is milliseconds —
reparse, recheck, reload IR into a running process. Option B pays code
generation plus compilation on every edit.

**Contract-as-data is a first-class feature, not an implementation detail.**
`/~contract` (RFD 0002, §5), the surface ledger and the surface diff (RFD
0004, §5) all require the contract to exist as a live structure the runtime
holds and serves. That structure *is* Option A's loaded IR. Option B dissolves
it into generated code, and the feature has to be rebuilt beside the thing
that destroyed it.

**Re-implementability is axis 2, and A protects it.** Option A cleanly
separates semantics (IR plus interpretation rules) from notation and from
backend. The IR becomes the portable artifact "others implement." Option B
tends to fuse the semantics into the target — which is the LINQ trap,
re-created one layer down.

**One small surface to make correct.** One runtime is one home for
transactions, policy, and effects (README, "The senior engineer": boring
underneath; Gall's law respected).

Two later facts strengthen the verdict. RFD 0005's build order — IR and
checker before any parser, the tracer bullet driven through hand-authored IR —
is this architecture operationalized as a work plan. And RFD 0005's
differential testing (the same contract against the prototype runtime and
against generated Postgres) quietly turns Option B into the **second
conforming implementation**: when `spock2sql` exists, the axis-2 claim stops
being an argument and becomes a passing test suite. B is not merely the
production bridge; it is the languagehood proof made executable.

## 8. The load-bearing rule: the IR is a specified artifact

This is the discipline that decides whether Spock stays a language or quietly
collapses back into executable JSON. The IR must be a real, named, versioned
artifact whose **semantics are independent of the interpreter** — so the
interpreter is a *conformance implementation, not the definition*. If the IR's
meaning is "whatever the interpreter does with it," axis 2 is lost, and every
argument in Part I evaporates.

The analogy that makes it concrete: Java bytecode is "executable data" too,
and Java is still a language — precisely because the bytecode is governed by a
spec, not by whichever JVM happened to run it. The IR is Spock's compiled form
under a spec. That is categorically different from an ad-hoc executable JSON
where the data is the source and the interpreter is the spec.

## 9. Iteration speed is architectural, not linguistic

The prototype's fast loop is delivered by *this architecture* — IR hot-reload
into a static runtime — not by the language Spock happens to be implemented
in. The Spock user's edit→play loop is a runtime reload; it is host-language
agnostic. So arguments of the form "prototyping needs speed, therefore
implement Spock in a fast-to-iterate host language" are a category error: the
speed the product promises is architectural. The host-language question is
real, but different — it is RFD 0007.

## Open questions

- **IR encoding.** Human-readable (text/JSON) or binary? Public, versioned
  interface or internal detail? §8 pushes toward "specified and versioned"
  either way; the encoding itself is open.
- **Conformance timing.** RFD 0005 already commits to a conformance-suite-as-
  spec direction; open is *when* — v1, or after the semantics settle.
- **IR spec location.** Does the IR get its own spec document once its shape
  stabilizes, separate from this RFD?
- **Naming.** What is the IR called in the language's own vocabulary —
  contract, program, plan, artifact?
- **The in-browser playground** (RFD 0007, §4): commit now, since it
  constrains architecture early, or defer?

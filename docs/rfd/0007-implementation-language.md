# RFD 0007 — Implementation language

Status: accepted decision — Rust, with named escape routes. The README
("Implementation") already commits to Rust; this RFD supplies the rationale
that commitment previously lacked and records the alternatives, so a reader
can see it was a decision, not a default.

## 1. Two aspects that pull apart

Building Spock rewards two different kinds of host language, and they are not
the same kinds.

- **Aspect A — a language good for building a language.** Algebraic data
  types, exhaustive pattern matching, cheap modeling of ASTs and symbol
  graphs: the checker's home turf.
- **Aspect B — an ecosystem that builds *this* tool faster.** Parsing
  libraries, embedded SQLite, an HTTP/JSON server, a Wasm host and target, IR
  serialization, a language-server library, single-binary distribution.

The ML family (OCaml, Haskell) wins A and is thin on B. The systems and
scripting families win B and are often weak on A. The choice is decided by
which aspect Spock actually weights.

## 2. The thesis that decides it

Spock is not just a compiler. It is a compiler **plus** an embedded-database
runtime, an HTTP server, and a future Wasm host (RFD 0002, "The v0 cut";
README, "V1 direction", path 3). Half of the system is runtime. So Aspect B
counts as much as Aspect A — and **Rust is the rare mainstream language that
is top-tier on both halves.** That, not fashion, justifies the pick.

## 3. Considered seriously: Rust, OCaml, TypeScript

**Rust — chosen.** Strong on A: enums with enforced exhaustive `match`, no
null, `Result` as the idiom Spock's own error doctrine mirrors. Strongest
available on B for this shape of tool: mature parsing crates, `rusqlite`,
`axum`/`tower`, `wasmtime`, `serde`, `tower-lsp`, and a single static binary
at the end. The known cost is real and named: the borrow checker taxes cyclic
AST/IR/symbol-graph modeling. The mitigation is standard — arena- and
index-based IR (`la-arena`/`id-arena` style) — and it carries a synergy worth
noticing: an IR addressed by indices rather than pointers serializes cleanly,
which makes the "IR as a specified, versioned artifact" discipline (RFD 0006,
§8) cheaper, not harder.

**OCaml — the A-maximal alternative.** The historical compiler language;
variants and `match` are the gold standard; garbage collection makes graph
modeling free; Menhir is superb; the compiler's own inner loop is fast. But it
is thin on B: SQLite bindings, HTTP serving, and especially the Wasm story are
weaker. Choose OCaml only if front-end and checker elegance come to dominate
and the runtime is going to be hand-rolled regardless.

**TypeScript — the B-for-reach alternative.** The largest ecosystem, the best
AI-assist, instant iteration, npm-native distribution (the `npm/` directory
already reserves the name — README, "Implementation" — though for
distribution, not implementation), and a trivial in-browser playground. Weak
on A: no real algebraic data types or exhaustiveness (only the `never` trick),
an unsound and runtime-erased type system, and no clean single binary. Choose
TypeScript only if npm reach and iteration speed are re-ranked above a fast
binary and a totality-checked core.

## 4. The clincher: the deployment matrix

Read the Wasm axis and the distribution axis together. **Rust yields a native
single binary *and* an in-browser playground from one codebase** — compile the
runtime to Wasm and drive SQLite via a Wasm build. That serves the prototype
mission directly: a playable contract you share as a URL, no install — while
honoring "one binary, boring underneath" (RFD 0002, "The v0 cut"). OCaml gives
the native binary with a weak browser story; TypeScript gives the browser with
a weak native story. Rust uniquely spans the whole matrix, and README path 3
was already pointing here.

## 5. Rejected, with reasons

- **Go.** Trivial binaries and great HTTP, but no sum types and no pattern
  matching. Spock's checker is, at heart, one giant exhaustive match over a
  closed transition space (RFD 0002, "the atom"); Go fights exactly the code
  that is Spock's novelty.
- **Scala 3 + GraalVM Truffle.** The most literal "framework for building
  languages," strong ADTs, a free JIT — and a JVM footprint that fights the
  one-small-binary doctrine.
- **Haskell.** A-maximal but nicher than OCaml, and laziness complicates a
  product runtime. For a combined compiler-plus-runtime, OCaml dominates it —
  and OCaml already lost to Rust here.

**Decision: Rust.** Escape routes, should the weights change: OCaml if
front-end purity comes to dominate a hand-rolled runtime; TypeScript if npm
reach and iteration speed outrank the fast binary and the totality-checked
core.

## Open questions

- **Playground timing** — shared with RFD 0006's closing question: commit to
  the Rust→Wasm same-codebase playground now (it constrains early
  architecture) or defer.
- **Language server timing** — the IR-first build order (RFD 0005, §1) makes
  an LSP late by construction; decide when it earns a slot.
- **The second implementation's language.** RFD 0006 makes `spock2sql` the
  second conforming implementation. Should a *reference* implementation ever
  be written in a deliberately different host language, so the spec — not
  shared code — is what carries the semantics?

# RFD 0016 — doc comments: `///` outer, `//!` inner, carried in the contract

Status: ACCEPTED (July 2026), implemented. This RFD is the design of record for
a **documentation tier**: Rust-shaped doc comments (`///` outer, `//!` inner)
that ride in the compiled contract (§6), surface in studio (RFD 0015), and head
the generated TypeScript and GraphQL (RFD 0010). It sits orthogonal to the
filter RFD — it touches no query surface, adds no keyword, and is additive
under `spock: "v0"`. Doc placement errors ship as `L011`/`L012` (§9); studio
*rendering* of the docs is a deferred follow-up (studio is a pure `/~contract`
consumer, RFD 0015).

## 1. The evidence

Every real `.spock` file is already full of documentation — it just cannot
travel. `examples/instagram/v0.spock` opens with a ten-line banner explaining
what the file covers, carries a one-line gloss above nearly every table (`// the
identity anchor (RFD 0014): user.id is the actor`), and annotates fields inline
(`archived_at: timestamp?  // archive keeps likes/comments`). This is good
prose, written by the author who knew the most. And it is **trapped in the
source**: the lexer discards it (`lexer.rs:192`), so it never reaches the
contract, never reaches studio, never reaches the generated TS row type or the
GraphQL SDL a client actually reads. The one place the knowledge exists is the
one place a consumer never looks.

The contract is the interchange artifact (RFD 0006): everything a runtime or
tool needs is supposed to be in it, and nothing is supposed to require reading
the source. Documentation is the glaring exception. A studio user browsing
`media.status` sees `"pending" | "ready" | "failed"` but not *what a status
means*; a client reading the generated `User` type sees field names but not the
author's one sentence about each. The fix is not a new document format — it is
to stop throwing the author's sentences away.

## 2. The shape

Two sigils, exactly Rust's:

- `///` — an **outer** doc comment: documents the entity that *follows* it.
- `//!` — an **inner** doc comment: documents the *enclosing* scope. In spock
  that scope is the file, so `//!` is the **contract**'s documentation.
- `//` (and `////`, `/////`, …) stay ordinary comments, discarded as today.

```spock
//! Instagram, the v0 slice — users, posts, and the graph between them.
//! This text documents the *contract*: it rides in `/~contract`, sits at the
//! top of studio, and heads the generated TypeScript and GraphQL SDL.

/// A person on the network. The identity anchor (RFD 0014): `user.id` is the
/// actor every `spock_actor()` read and every `= me` write resolves to.
auth table user {
  key id: uuid = auto
  /// The handle, shown everywhere. 1–30 chars of `[a-z0-9._]`, unique.
  username: text check valid_username unique
  /// Display name; absent until the user sets one.
  full_name: text?
  joined_at: timestamp = now
}

/// Rename a user, refusing a handle already taken.
mut fn rename_user(
  /// The user to rename.
  user: user,
  /// The new handle; validated by `valid_username` on the write.
  name: text,
) -> user ! user_username_taken | not_found {
  unchecked sql("UPDATE user SET username = :name WHERE id = :user RETURNING *")
}
```

Nothing here is a construct a language model has to be taught. This is the
strongest possible case for RFD 0013's **LLM-writability** law — "a surface must
be either SQL-exact or radically simple." Rust doc comments are neither novel
nor bespoke; they are among the most in-distribution tokens in existence, seen
millions of times per model. There is no new grammar to spell wrong. The doc
tier adds **zero keywords** and **zero reserved words**: `///`/`//!` are lexical
trivia promoted to tokens, not identifiers or clauses.

## 3. Lexical rules

The lexer's comment branch (`lexer.rs:192-198`) today matches `//` and swallows
to end of line, so `///`, `//!`, and `////` all currently lex identically and
vanish. This RFD splits that branch on the third byte, **before** the swallow,
following Rust exactly:

- `//!…` → an **inner** doc token. Content is the rest of the line after `//!`.
- `///…` → an **outer** doc token, **unless** a fourth `/` follows. Content is
  the rest of the line after `///`.
- everything else (`//`, `////`, `/////`, `// x`) → ordinary comment, discarded.

The `////`-is-not-a-doc rule is Rust's, and it matters: a `/////////` divider
line stays an ordinary comment. Precisely: after `//`, inspect byte *i+2* —
`!` → inner; `/` **and** byte *i+3* is not `/` → outer; else ordinary. A doc
comment on the final line with no trailing newline terminates at EOF, mirroring
the existing `//` handling.

**A new token.** The lexer gains `TokenKind::Doc { inner: bool, text: String }`,
span-carried like every token. This is the smallest change that lets the parser
see docs; it keeps all attachment logic in the parser, where item boundaries are
known. (The alternative — a side-table of `(Span, text)` the parser consults —
was rejected: it splits doc state across two structures and complicates the
"documents nothing" diagnostic, which is fundamentally *"this token had no
following item."*)

**Content normalization** (deterministic, specified so every conformance
agrees): strip the sigil; strip at most one leading space (`/// x` → `x`, not
` x`); trim trailing whitespace. Consecutive doc tokens of the same polarity,
separated only by whitespace and ordinary comments, **join with `\n`** into one
doc string bound to one target — so a paragraph written across five `///` lines
is one doc. A run that normalizes to entirely empty attaches nothing (`doc` is
absent, never `Some("")`); a lone `///` is a harmless no-op, not an error.

## 4. Attachment and scope

**Documentable entities** — the contract's consumer-facing surface, and only it:

| Entity | Documented by | Contract carrier |
|---|---|---|
| the contract (file) | `//!` in the file preamble | `Contract.doc` |
| `table` / `auth table` | `///` before the declaration | `Table.doc` |
| `record` | `///` before the declaration | `Record.doc` |
| field (table or record) | `///` before the field | `Field.doc` / `RecordField.doc` |
| `fn` / `mut fn` | `///` before the declaration | `FnDef.doc` |
| fn parameter | `///` before the parameter | `FnParam.doc` |

`///` binds to the **next declaration or member token**, transparently across
the `auth` and `mut` modifier prefixes (which `file()` consumes before
dispatching): `/// …` above `auth table user` documents the table, above
`mut fn f` documents the fn. A parameter doc requires the parameter on its own
line, since the doc binds to the token that follows it — this is the one
position Rust itself does not document, justified because a spock parameter is a
named contract input and a GraphQL argument, both of which carry descriptions.

**`//!` is the file preamble only** — before the first declaration. This is a
deliberate, documented divergence from Rust, which allows `//!` inside any
module. spock has no modules, and every *other* entity has a token in front of
it to hang a `///` on; only the file has no preceding position. So `//!` earns
exactly one job — the contract's docs — and `//!` anywhere after the first
declaration is an error (§9). Ordinary `//` banners and whitespace may precede
the `//!` block; they are trivia and do not count as "the first declaration."

This resolves a real footgun the tree already contains. `examples/instagram/`
`v1.spock` opens with a 20-line `///` banner meant as *file-level* prose. Under
the outer-doc rule that banner would silently attach to the first `table` as its
description — the wrong entity. The guidance, and the reason `//!` exists: **file
docs are `//!`, item docs are `///`.** (v1.spock is a paper design draft the v0
toolchain does not accept, so nothing breaks today — see the compatibility note
below — but it is the exact mistake the two sigils are shaped to prevent.)

**Source compatibility.** Promoting `///`/`//!` to tokens is source-compatible
with every program the v0 toolchain accepts. `//!` occurs nowhere in the tree.
All 31 `///` occurrences live in `docs/rfd/0000-vision.spock` and
`examples/instagram/v1.spock` — both paper drafts the toolchain already rejects
for unrelated reasons (reserved words `iam`/`role`/`@identity`; the explicit
"accepted-syntax suspended" waiver). The one file that actually compiles,
`v0.spock`, uses plain `//` throughout and is untouched. Recorded honestly: for
a *hypothetical* future program that used `///` as decoration, the meaning would
change from "discarded" to "documentation" — which is why the change is called
out here rather than assumed invisible.

## 5. The contract (§6, additive)

Each documentable IR struct gains one field, using the established additive
idiom (the same `#[serde(default, skip_serializing_if = "Option::is_none")]`
pattern that carries `Field.check` and proven by `legacy_contract_json_`
`deserializes`):

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub doc: Option<String>,
```

on `Contract`, `Table`, `Field`, `Record`, `RecordField`, `FnDef`, and
`FnParam`. The name is `doc` (snake_case, verbatim — the contract has no global
rename). Absent means undocumented; old contract JSON keeps loading in new
consumers and new contract JSON keeps loading in doc-aware consumers, both under
the frozen `spock: "v0"` tag. `/~contract` emits the IR directly
(`http.rs:120`), so the field appears the moment it is on the struct — no
handler change.

Threading is mechanical: the AST decl nodes (`TableDecl`, `FieldDecl`,
`RecordDecl`, `FnDecl`, `ParamDecl`, plus a `doc` on `File` for the contract)
gain the same `Option<String>`, the parser fills them from the pending-doc
buffer, and each lowering site in `check.rs` copies one field
(`doc: decl.doc.clone()`). No new checker phase; docs are inert data, never
validated for content.

**Row checks, keys, uniques, and seed rows carry no doc.** They have no name or
identity in the consumer-facing surface — a `unique (post, position)` is a
constraint, not an entity a studio page or a generated type ever renders. A
`///` immediately before one of them documents nothing and is an error (§9),
which is almost always the sign the author meant to document the field above it.

## 6. Emissions and studio — documentation for free

Both codegen targets already have a native description channel; docs flow into
them with no format invention (RFD 0010):

- **TypeScript** (`typescript.rs`). The emitter already writes JSDoc `/** */`
  blocks over rows and a per-field `/** check: … */` line — the exact insertion
  point. A table doc becomes the row type's JSDoc; a field doc a field JSDoc; a
  fn doc the args/fn JSDoc. The golden test is re-pinned.
- **GraphQL** (`graphql.rs`). async-graphql's `Object`, `Field`, `InputObject`,
  and `InputValue` all take `.description(...)`, and the builder already uses it
  for scalars and mutations. A table doc becomes the object-type description
  (rendered as a leading `"""…"""` in SDL); field and argument docs likewise.
  This is GraphQL's own documentation mechanism — introspection tools and
  GraphiQL (already linked from studio) render it automatically.
- **studio** (RFD 0015) is a pure consumer of `/~contract`. It reads `doc` and
  renders it: the contract doc heads the console, table/fn docs describe the
  schema browser and fn runner, field docs annotate columns. This is a thin
  follow-up in the studio client, not part of the language work.

The payoff: one `///` line written once at authoring time becomes a JSDoc
comment in the client's editor, a description in the GraphQL schema explorer,
and a caption in studio — the "written by the author who knew the most,"
delivered to every surface that renders the contract.

## 7. Internal vs public — the visibility tier, deliberately deferred

The requirement to record, per the request: **in production, a doc may be
internal.** An operator's note on a table ("nightly job truncates this") belongs
in studio for the developer but not in the public contract a client downloads.
This RFD does **not** build that split, and the reason it can wait is doctrine,
not laziness.

**v0 is the open tier (RFD 0004, §8).** The whole contract is already public by
decision: `GET /~contract` serves it to anyone, fn SQL bodies travel *verbatim*,
every derived error code is visible before any request. There is no private
surface for a doc to leak *into* — the fn body next to it is already exposed. So
in v0 every doc is public, and a visibility marker would decorate a distinction
that does not yet exist. Adding it now would be inventing surface for a tier the
language has not reached (the exposure model's private tiers are future work).

**The forward path stays open, additively.** When the private tier lands, doc
visibility is a strictly additive change, and today's single `doc: Option`
`<String>` forecloses nothing:

- the additive move is a sibling field — a `doc_internal: Option<String>`, or a
  `visibility` tag beside `doc` — under the same §6 additive law that admitted
  `check` and `anchor`; a pre-visibility reader ignores it exactly as it ignores
  any unknown field;
- the *enforcement* is a **contract projection**, not a parser feature: the
  authoritative contract (studio-admin, all docs) is projected to a **published
  contract** (client-facing, internal docs stripped) at the exposure boundary —
  the same boundary that will gate private tables and fns. Visibility is an
  exposure decision, so it belongs at the exposure layer, not the lexer.

**Untaken syntaxes, named so the door is marked, none chosen:** a distinct sigil
for internal docs (e.g. a `//!!`/`///-` family) — rejected for now as sigil
proliferation against RFD 0013's radical-simplicity law; an attribute line
inside a doc (`/// @internal`) — rejected as inventing a doc mini-grammar, the
precise out-of-distribution move the value-tier RFD killed; a `pub`/private
default flip — premature until there is a private tier to default against.
The decision recorded here is only that **v0 ships one public doc channel**, and
that the internal/public split is an additive exposure-layer feature, not a
lexical one.

## 8. What this deliberately does not do

- **Block doc comments (`/** */`, `/*! */`).** spock has no block comments at
  all; the line forms cover every case and adding block comments is unrelated
  surface. Multi-line docs are a `///` run.
- **Docs on constraints and seeds** (§5): keys, uniques, row checks, seed rows —
  no consumer-facing identity, so no doc slot.
- **Docs on closed-set members.** `"image" | "video"` members have no
  per-member attachment point without block comments; a set is documented as a
  whole on its field. Deferred with the rest of block-comment surface.
- **Markdown or any content semantics.** A doc is an opaque UTF-8 string carried
  verbatim, like a fn's SQL body. The compiler never parses, renders, or
  validates it; whether a consumer treats it as Markdown is the consumer's call
  (studio may; the contract does not say).
- **Doc-completeness lints** ("every public entity must be documented"). Out of
  scope; a possible future `spock check` warning, never an error.
- **The internal/public visibility split** (§7) — deferred to the exposure tier.

## 9. Diagnostics

Two new codes, raised by the parser (only the parser knows item boundaries).
They ship in the **`L0xx`** parse-error family — the spec's own taxonomy is
"L-codes are lexical/parse errors" (§4), and the parser already raises `L010` —
so the working names `E-DOC01`/`E-DOC02` land as **`L011`/`L012`**, the same
renumbering RFD 0014's proposed `E-ACT0x` underwent to ship as `E045`–`E047`:

- **`L011` — dangling doc comment.** A `///` run that documents nothing: before
  `}`, before end of file, before a non-documentable item (key/unique/row-check/
  seed), or mid-declaration where no entity follows. The message names the fix —
  "put `///` directly before the item it documents, or use `//`." Following
  Rust, this is a hard error, not a warning: a doc that documents nothing is a
  latent bug (a deleted item, a mistyped sigil), and spock rejects such things at
  compile time by disposition ("determinism when compiled", RFD 0004). The cost —
  a stray non-empty `///` during editing fails the build — is acknowledged and
  judged worth the caught typo. An *empty* doc run (a lone `///`) is a no-op and
  never trips it.
- **`L012` — misplaced inner doc.** A `//!` after the first declaration. The
  message points to the fix: `//!` documents the whole file and must come before
  the first declaration; use `///` for the item.

Detection is by **consumption tracking**, not position enumeration: every doc
line is attached at a documentable boundary or, if none consumes it, survives to
a final scan that reports the earliest offender. This closes the positions an
enumerated list misses (inside a fn body, between statements, in a seed block).

## 10. Roadmap

The doc tier is small, additive, and orthogonal to the query surface, so it does
**not** displace the filter RFD as the next *language* milestone (RFD 0009 §3,
as revised by RFD 0013). It is a cross-cutting metadata layer: one lexer branch,
one AST/IR field repeated, two emission touch-ups, a thin studio follow-up. It
can land in a quiet slot before or alongside filter without contending for the
differentiator's attention. Implementation order, when green-lit: lexer token →
AST/parser attachment + `E-DOC0x` → IR `doc` fields + lowering → TS/GraphQL
emission (re-pin goldens) → studio rendering → spec §1/§2/§6 updates.

## 11. The doctrine line

RFD 0006 said the contract is the artifact and the source is one conformance of
it. Documentation was the last thing the author knew that the artifact did not
carry — trapped in trivia the lexer threw away. The doc tier moves it across the
line, using the most in-distribution syntax a model has ever seen and inventing
no grammar to do it: `///` for the thing that follows, `//!` for the file, one
`doc` string in the contract, and every generated surface — TypeScript, GraphQL,
studio — documented for free from the sentence the author already wrote. The
open tier publishes all of it today; the private tier will project what stays
in. The escape carries opaque SQL verbatim; the contract now carries the
author's prose the same way — and finally shows it to the reader who needs it.

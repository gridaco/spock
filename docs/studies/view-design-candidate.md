# Candidate study — A conservative shape for `view`

- **Study state:** open candidate
- **Kind:** informal design candidate
- **Created:** 2026-07-23
- **Review state:** initial adversarial review completed; formal review not started
- **Language-problem issue:** none
- **Working group:** none
- **RFD:** none

> **Non-normative candidate.** This document is a concrete design to examine,
> not a specification, committee decision, accepted syntax, implementation
> target, or authorization to build. It records a starting position so review
> can falsify or refine something precise. Current Spock behavior remains
> defined by the [v0 specification](../spec/v0.md).

This candidate is downstream of
[the neutral `view` problem-space study](view-problem-space.md). That study
remains the evidence map and harness; this document deliberately selects one
bundle so it can be challenged. A later proposal may replace, split, or reject
this bundle without rewriting the study's findings.

## Question

Can Spock give `view` a small, SQL-grounded identity while supporting explicit
public shapes and a narrowly provable update-through path, without silently
bundling exposure, authorization, arbitrary gateway behavior, or
backend-specific view updatability?

## Candidate in one paragraph

A `view` is a named, compiler-owned, non-persisted, flat derived relation. It
is private and its fields are read-only by default. `pub` exports a stable
relation symbol and row shape, but a separate fail-closed policy grant is
required before it can be served. A served public view is deliberately an
enumerable relation with bounded outer filtering, ordering, and paging;
authority-bound non-enumerable lookups remain read functions. `mut` opts an
individual field into `UPDATE`-through behavior only when the compiler can
prove a stable, non-aliasing base-column destination. Reads may lower to a
native SQL `VIEW` or an equivalent plan; writes always lower to explicit
guarded base-table updates in a transaction.

## 1. Concept identity

The language-level construct is a **logical derived relation**, not a promise
that a database catalog object exists.

An ordinary view in this candidate is:

- named;
- compiler-owned and structurally inspectable;
- flat and homogeneous;
- non-persisted, meaning it has no independently refreshed stored result;
- unordered unless an outer query supplies an order; and
- bag-valued unless a proven key or another supported operator establishes a
  stronger fact.

Transient optimizer materialization does not violate non-persistence. A
persistent materialized result, cache, or incremental projection would need a
separate freshness and refresh contract.

The initial relational core may use:

- tables and other views as relation sources;
- explicit projection and renaming;
- deterministic scalar expressions and builtins;
- `where` predicates;
- checked inner joins; and
- checked left joins.

The initial core does not include:

- declared parameters;
- request or actor context in the relational definition;
- time, randomness, external calls, or other volatile dependencies;
- nested objects or collections;
- authored `order by`, `limit`, or page envelopes;
- `distinct`, grouping, aggregate rows, windows, or set operations;
- recursion or cyclic view dependencies; or
- materialized or live behavior.

Those exclusions define a starting proof surface, not a claim that the broader
forms are impossible. Each changes identity, composability, provenance, or
portability and should be evaluated separately.

The view definition itself is context-free. The relation an actor observes may
still differ because policy filters public access. Actor-relative computed
values such as `viewer_has_liked`, parameterized searches, rankings, and
purpose-bound lookups remain read functions unless a later contextual-relation
extension is justified.

## 2. Two declaration forms, one IR

### 2.1 Table-nested facets

A table may contain several named view facets:

```spock
table profile {
    key id: uuid = auto
    username: username
    display_name: text
    bio: text?
    created_at: timestamp = now

    pub view summary {
        key id
        username
        display_name
    }

    view moderation_base {
        key id
        username
        created_at
    }
}
```

The nested declaration has one exact conceptual desugaring:

```spock
view profile.summary
    from profile as self
{
    key id: self.id
    username: self.username
    display_name: self.display_name
}
```

Nesting contributes only:

- a canonical namespace such as `profile.summary`;
- an implicit anchor relation;
- a reserved `self` source binding; and
- shorthand for direct anchor fields.

It does **not** imply aggregate ownership, authorization, public exposure,
cardinality, or writability. The compiler lowers nested and top-level forms to
the same relational IR.

Multiple nested facets are expected. Distinct passenger, driver, operator,
summary, settings, and moderation shapes should not require different storage
tables.

`default` has no special meaning in this candidate. `view default` is an
ordinary facet named `default`; replacing a table route or choosing a protocol
default requires a separate decision.

### 2.2 Explicit-source views

A relation with no useful nesting site, or with an intentionally explicit
source graph, is top-level:

```spock
pub view post_card
    from post as p
    join profile as author on author.id == p.author
    where p.published
{
    key id: p.id
    body: p.body
    author_name: author.display_name
}
```

The braces are the output projection. Every public field is stated positively.
`*`, `all`, and `all except` are forbidden because a later table-field addition
must not silently widen a public or writable shape.

Output names must be unique. Types and nullability are inferred from the Spock
schema and checked relational plan, not from backend view metadata.

Views may source other views. The dependency graph must remain acyclic, and
context, provenance, key, and capability facts propagate conservatively
through the graph.

## 3. Names and compatibility

A nested symbol has a canonical qualified identity such as `profile.summary`.
Protocol and code-generation projections may encode the qualification
differently, but every projection must be deterministic and collision-checked.

Renaming the enclosing table renames the nested public symbol and is therefore
a public breaking change. An author who needs public identity independent of a
storage name should use a top-level view.

For a `pub view`, the compatibility surface includes at least:

- qualified name;
- output field names and order-independent identity;
- field types and nullability;
- key and cardinality claims;
- supported outer-query operators;
- writable fields and operations;
- row-membership behavior;
- effective policy bindings; and
- generated protocol and client names.

Every change must appear in the compiled surface diff. Successful database DDL
replacement is not evidence of public compatibility.

## 4. Private, public, and authorized

```spock
view internal_shape { ... }
pub view exported_shape { ... }
```

- `view` is private and has no product-plane protocol surface.
- A private view may be composed by other views and functions.
- In the initial candidate, `mut` is legal only inside a `pub view`; private
  views may carry putback-feasibility evidence in IR but expose no update
  operation.
- `pub view` exports a stable relation symbol and row type into the public
  contract.
- `pub` does not mean anonymous and does not itself grant any actor access.
- In the governed tier, a public view without an explicit read-policy grant is
  a compile-time error, never an allow-by-default route.

The exact policy syntax is outside this candidate, but the dependency is not:
`pub` cannot become operational before Spock has a verified, immutable request
authority context and a fail-closed policy model.

One public view has one stable field schema across actors. Policy may govern
view use, row membership, and write operations; it does not dynamically remove
fields from the schema. Different field sets should be modeled as different
facets, which keeps code generation and compatibility inspectable.

Internal view composition uses relational definitions, not inherited public
grants. Each outer `pub view` is independently reviewed authority, and its
complete source lineage must appear in the surface ledger. If public
relationship expansion is introduced later, entering the target resource must
re-enter that target's public policy rather than opening its base table.

## 5. Public relation behavior

After a read grant binds a `pub view`, it is intentionally an enumerable
relation. The initial public read surface includes:

- bounded list reads;
- by-key reads when the view has a proven key;
- filtering through supported operators on exported fields;
- ordering through supported operators on exported fields; and
- bounded pagination.

This is a semantic boundary, not accidental gateway breadth:

> If a resource may be fetched only through an authority-bound exact lookup
> but must not be enumerated, it is a read `fn`, not a `view`.

An Instagram collection of publicly discoverable posts may be a view. An
unlisted post accessible only by a supplied identifier should be a function.
An Uber passenger's policy-filtered trip history may be a view. A trip visible
to an operator only because it belongs to a particular approved support case
is likely a function with case-purpose authorization.

Only exported fields participate in outer composition. The compiler derives
the operator set from each field's type and expression and applies protocol
depth, breadth, value, row, and execution-cost limits. Caller-provided SQL or
functions never enter the relational plan.

Exact count, general aggregation, relationship expansion, and subscriptions
are not automatic consequences of `pub view` in the initial core.

## 6. Keys, identity, and ordering

A view key is proof, not an assertion or naming convention.

```spock
key id: p.id
```

is accepted only when the compiler can establish non-null uniqueness over the
derived result from trusted:

- primary keys;
- unique constraints;
- foreign keys;
- nullability; and
- join cardinality.

Every relational operator must explicitly preserve, transform, or erase key
evidence. A field named `id` is not a key without proof.

Keyless private read views are allowed. The initial public pageable surface and
every writable view require an exposed proven key. Keyless aggregates or
singleton results can remain internal or use a function until a distinct
public-operation contract exists.

The relation itself is unordered. The public query layer supplies a total
order: an explicit client order receives the proven key as its final
tiebreaker; absent an explicit order, the key is ordered by Spock's canonical
type semantics. This removes tie nondeterminism but does not promise a stable
database snapshot across separate page requests.

## 7. Field-level writeback

Fields are read-only by default:

```spock
display_name
```

Inside a `pub view`, `mut` requests `UPDATE`-through capability for one output
field:

```spock
mut display_name
mut push_likes: prefs.push_likes
```

It does not imply insert, delete, upsert, bulk mutation, authorization, or
optimistic concurrency. Those are functions or separate future capabilities.

For every field, including readonly fields, the compiler and contract IR report
two independent facts:

```text
putback: possible | impossible(reason)
update: exposed | not_exposed
```

A direct readonly projection may therefore have a provable putback without
being writable through the public contract. `mut` is authorial intent that may
only narrow the compiler's proof; it cannot invent an inverse.

### 7.1 Writeback proof

A `mut` field is accepted only when all of the following hold:

1. The view has an exposed, proven key.
2. The expression resolves to exactly one base column through checked
   provenance.
3. Each view key resolves to exactly one existing target row.
4. Distinct writable view rows cannot alias the same target cell.
5. The target is not on the nullable side of an outer join.
6. The target is not part of the view key, a join predicate, target selection,
   a uniqueness proof, the view predicate, or an effective authorization
   dependency.
7. No other output field maps to the same base column.
8. Every key, foreign-key, unique, and nullability fact used by the proof is
   enforced by the runtime backend.
9. The derivation and authorization dependencies are deterministic for the
   transaction.

These rules make mutability local and non-aliasing, not merely traceable.

| Projection | Candidate `mut` status |
| --- | --- |
| Direct anchor field | Eligible |
| Renamed direct anchor field | Eligible |
| Field through a proven injective one-to-one join | Eligible |
| Field on a shared many-to-one parent | Rejected |
| Field on the nullable side of an outer join | Rejected |
| Computed, cast, concatenated, or coalesced expression | Rejected |
| Aggregate or count | Rejected |
| Key, predicate, join, mapping, or authority field | Rejected |

Changing an author's display name through one `post.card` row is rejected even
though the target row is identifiable: many post rows alias the same profile
cell. That mutation belongs on a profile facet or in a named function.

### 7.2 One-to-one joined settings

A table can prove an at-most-one companion with an enforced key:

```spock
table notification_prefs {
    key profile: profile on delete cascade
    push_likes: bool = true
    push_comments: bool = true
}

table profile {
    key id: uuid = auto
    display_name: text
    bio: text?

    pub view settings
        join notification_prefs as prefs
            on prefs.profile == self.id
    {
        key id
        mut display_name
        mut bio
        mut push_likes: prefs.push_likes
        mut push_comments: prefs.push_comments
    }
}
```

For each row produced by the inner join, `prefs` exists and is unique, so the
joined fields have one target. The schema does **not** prove that every profile
has a preferences row. A profile missing its companion disappears from this
facet. If that is unacceptable, the product needs a required-companion
invariant covering every creation path, separate facets, or a function that can
create the missing row. A left join cannot synthesize an update target under
this candidate.

Composed views never inherit public mutability. A derived view must redeclare
`mut`, and the compiler must re-prove every target through the full lineage.

## 8. Mutation execution

A view mutation is a keyed, single-view-row PATCH. It executes as one guarded
protocol:

1. Bind the verified authority context immutably for the request.
2. Resolve exactly one existing visible view row by its public key.
3. Evaluate the separately applicable old-row update eligibility.
4. Resolve every target row and lock targets in deterministic table/key order,
   or use the backend's equivalent serialization mechanism.
5. Authorize every requested output-field update.
6. Apply all base-column changes in one transaction.
7. Re-evaluate the view predicate, exact affected-row cardinalities, and the
   separately applicable new-state admissibility policy.
8. Commit only if every check succeeds; otherwise roll back the whole patch.

Read visibility, old-row update eligibility, and new-state admissibility are
different policy contracts. They correspond roughly to SQL's `SELECT`,
`UPDATE ... USING`, and `WITH CHECK` concerns and must not be collapsed into
one predicate.

If a predicate or policy depends on rows outside the locked target set, the
runtime must use serializable execution with retry or reject the writable plan.
Moving the same logic to a function does not by itself solve concurrency; the
function needs an equally honest isolation contract.

Fields that can move identity, provenance, membership, or authorization are
not writable through a view in this candidate. Membership-changing operations
such as transferring ownership, claiming a queue item, accepting an
assignment, or completing a lifecycle state belong in `mut fn`.

The default concurrency statement is deliberately narrow:

> A view PATCH is a blind overwrite with no revision precondition.

This is not described as last-write-wins because ordinary transactions do not
provide one portable whole-patch arbitration order across multiple target
rows. A general revision or `If-Match` feature may later add optimistic
concurrency without changing field provenance.

Constraint failures from any touched target roll back the transaction and
enter the derived error surface. A mutation requiring an authored product
refusal belongs in `mut fn`.

Write lowering never delegates semantics to a backend's automatic
view-updatability rules. It emits explicit guarded base-table updates. This
avoids the incompatible PostgreSQL, SQLite, and MySQL view-update subsets.

## 9. Policy and complete mediation

At the governed language-version boundary:

- tables become internal product data;
- v0 automatic REST and GraphQL table roots disappear;
- raw generated table mutation inputs disappear;
- generated relationship expansion cannot reopen an internal table;
- public functions cannot return raw internal table shapes;
- views and functions become the product-plane surfaces; and
- direct database privileges must enforce the same boundary or be explicitly
  outside the supported threat model.

This is a versioned migration, never behavior triggered by the presence of the
first view declaration. A deployment must not expose authoritative public
views and the ungoverned v0 table floor simultaneously.

Studio, administrative SQL, seed loading, and repair tooling are control-plane
capabilities. They must be visibly separate from the product plane and cannot
be used as evidence that a public view is bypassable by an ordinary client.

Public functions need their own authorization and explicit public output
shape. A function returning an internal `profile` or `post` row would reopen
every internal field and invalidate view authority. An eventual function may
reuse an exported view row type, but it does not inherit that view's policy,
queryability, key, or write capability.

RLS, security-barrier views, database grants, runtime mediation, and equivalent
mechanisms are possible lowerings. None is the source-language meaning of
policy or `pub`.

## 10. `view` and `fn`

The grammatical boundary is deliberately visible:

```text
view_name             relation
function_name(...)    invocation
```

A view:

- declares no arguments;
- has no authored product-error surface;
- is effect-free when evaluated as a relation and returns no ephemeral result;
- may separately expose a compiler-derived PATCH operation when public fields
  declare `mut`;
- exposes a compiler-owned relational plan; and
- is outer-composable when public and granted.

A function:

- may accept arguments;
- may return one, optional, or many rows or scalars;
- may declare product errors;
- may return transaction-local or ephemeral values;
- owns invocation, cardinality, and ordering semantics; and
- is not automatically outer-composable when its body is opaque.

Scalar deterministic builtins may appear in a view expression. The prohibition
on sourcing functions concerns relation-producing Spock functions, not pure
scalar operations.

## 11. Lowering and conformance

The compiled relational IR, not generated SQL text, is the authority. It must
retain:

- source bindings and lexical scopes;
- output fields, types, and nullability;
- dependency and context closure;
- source-column lineage;
- predicate and join dependencies;
- key and cardinality evidence;
- per-field query operators and cost metadata;
- putback feasibility and reasons;
- exposed read and update capabilities; and
- public names and compatibility identity.

A backend may create a native SQL `VIEW`, inline the checked query, use a CTE,
or choose an equivalent plan. Two conforming lowerings must agree on observable
rows, nulls, duplicate behavior, policy, errors, and mutation results.

Any contextuality introduced by policy must bind trusted request context
immutably and reset backend session state safely across pooled connections.
SQL `CURRENT_USER` is normally the service account, not the product actor.

Cardinality and writeback proofs are valid only when the backend enforces the
constraints they cite. A backend cannot claim conformance while treating
foreign keys or transactions as optional.

## 12. Adoption sequence

This candidate can be investigated and, if later accepted, implemented only in
ordered slices:

1. private, read-only views and relational IR;
2. public views after verified authority, fail-closed policy, operation
   mediation, and closure of the v0 table floor;
3. direct anchor-field `mut`;
4. proven injective one-to-one joined `mut`; and
5. broader relational or mutation capabilities only after separate evidence.

Parsing `pub` or `mut` before its prerequisites exist must not create a partial
surface that appears safer or more capable than it is. An exploratory parser
may reject the form explicitly; it may not serve it with placeholder semantics.

## 13. Initial adversarial review

The first candidate was reviewed independently from three angles:

- database semantics, view-update theory, concurrency, and portability;
- language grammar, namespaces, IR, function boundaries, and evolution; and
- security, complete mediation, and Instagram/Uber product pressure.

That review caused material changes:

- nesting gained an exact desugaring and lost any implied aggregate ownership;
- actor context was removed from core view definitions;
- `pub` became export plus mandatory fail-closed policy, not authorization;
- public view enumeration became an explicit concept boundary with `fn`;
- public keys became proven and exposed rather than inferred from names;
- putback feasibility was separated from declared `mut` exposure;
- joined writes require non-aliasing injectivity, not merely an identifiable
  destination;
- mapping, key, predicate, and authority fields became non-writable;
- view-on-view mutability became non-transitive;
- last-write-wins language was replaced with blind-overwrite truth;
- read, old-state update, and new-state checks became distinct;
- multi-target updates gained locking, serialization, exact-cardinality, and
  rollback requirements; and
- removal of the table floor became a language-version migration rather than a
  declaration-triggered behavior change.

This was informal adversarial review, not governance review or acceptance.

## 14. Unresolved questions for formal review

The candidate intentionally leaves questions that can still falsify it:

1. Is an enumerable, composable public relation the right semantic consequence
   of serving a `pub view`, or must list, by-key, filter, order, and page be
   separately granted capabilities?
2. Is forbidding actor context in the relation too restrictive for ordinary
   personalized products, or does moving those reads to `fn` establish the
   cleaner boundary?
3. Should injective one-to-one joined writes belong in the first accepted
   version, or remain evidence for a later extension after anchor writes?
4. Which deterministic scalar and predicate subset is portable enough for the
   first relational IR?
5. How should protocol projections encode qualified names without unstable or
   collision-prone flattening?
6. Can flat public views and separately governed relationship traversal serve
   Instagram media, comments, and Uber timelines under one snapshot without
   recreating page-shaped gateway responses?
7. Which policy-composition rule is correct when a public view is used as a
   source by another public view?
8. Which public changes are statically breaking versus behaviorally reviewable
   but compatible?

## 15. Required evaluation

This candidate must be scored against the complete harness in
[the problem-space study](view-problem-space.md#proposal-and-development-harness),
not only the favorable examples above. At minimum, review must cover:

- direct projection, rename, computed fields, inner and outer joins, fan-out,
  keyless results, and view-on-view composition;
- outer filtering, ordering, paging, cost limits, and equal sort values;
- direct, joined, optional, aliased, membership-changing, and concurrent
  updates;
- anonymous, owner, ordinary-user, moderator, passenger, driver, and operator
  policy cases;
- direct-table, function-return, relationship, introspection, and database-
  privilege bypass attempts;
- native PostgreSQL view, native SQLite view, and runtime-plan lowerings; and
- Instagram settings, public/unlisted post access, moderation queues, Uber
  trip facets, precise location, support-case access, and active assignment.

A proposal revision is required if the candidate allows any of the following
without explicitly declaring it intended:

- a new table field silently appears in a public view;
- a direct table or function-return path bypasses a public view;
- a by-key-only fact becomes enumerable;
- two view rows mutate the same base cell through an apparently local patch;
- changing a mapping field redirects another field in the same patch;
- a missing one-to-one companion causes an undocumented disappearing surface;
- a multi-target patch partially commits;
- policy is checked against a different authority context inside one request;
- optimizer reordering allows caller-controlled expressions to observe hidden
  rows; or
- PostgreSQL and SQLite claim conformance while producing different observable
  null, duplicate, authorization, or update behavior.

## Follow-ups

- Review this candidate against every classified harness case.
- Create reproducible SQL fixtures before making portability claims normative.
- Resolve the public-operation-capability question before proposing protocol
  routes or generated clients.
- Resolve policy composition and complete mediation before serving `pub`.
- Measure the anchor-only and one-to-one joined write subsets separately.
- If the candidate survives review, open a language-problem issue and follow
  normal triage before any sponsored RFD.
- Keep any prototype isolated and explicitly non-conforming until the language
  process authorizes otherwise.

Success for this document means that review has a precise object to attack. It
does not mean that `view` has landed.

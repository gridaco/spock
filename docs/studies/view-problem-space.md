# Study — The `view` problem space

- **Study state:** open
- **Kind:** informal individual study
- **Author:** [@softmarshmallow](https://github.com/softmarshmallow)
- **Created:** 2026-07-16
- **Review state:** collecting evidence and discussion
- **Language-problem issue:** none
- **Working group:** none
- **RFD:** none

> **Non-normative research.** This document is not a working-group record,
> RFD, committee decision, definition of current Spock behavior, syntax
> proposal, semantic proposal, lowering plan, or authorization to implement a
> design.

## Question

What must be understood before Spock can responsibly define `view`, and what
common evidence and fixtures can be used to compare future proposals?

The word *view* already carries several plausible meanings: an SQL schema
object, a derived relation, a stable public data contract, a protocol resource,
an access-control boundary, a writable lens, a materialized result, and a
page-shaped gateway response. Some of those meanings can coexist. Others
create materially different language contracts.

This study records the problem before selecting among them.

## Scope and non-goals

The study covers the conceptual identity of a view; exposure and authority;
relational shape; joins; client composition; mutation; policy; errors;
materialization; portability; protocol and IR consequences; evolution; and the
boundary between Spock and Uhura.

It does **not**:

- propose or reserve syntax;
- choose whether declaration implies public exposure;
- choose whether the construct maps to an SQL `VIEW`;
- choose a logical, physical, or gateway implementation;
- choose read-only, inferred-write, or explicitly writable behavior;
- define a first-version feature set;
- rank candidate designs;
- amend the current specification or reinterpret legacy records as current
  behavior; or
- authorize a prototype or implementation sequence.

Examples named below are fixtures and evidence. They are not candidate syntax
and do not imply support.

## Method

This first pass uses four kinds of evidence:

1. the current normative Spock specification;
2. existing Spock doctrine, legacy design records, and paper programs, read at
   their stated authority levels;
3. primary documentation for established database and gateway behavior; and
4. concrete counterexamples that force otherwise independent choices to
   interact.

Claims about a particular database or gateway should eventually be backed by
a reproducible experiment recording the product and version, schema, data,
privileges, statement or request, result or error, and query plan when
relevant. This document currently identifies many experiments but does not
claim to have run them.

## Neutral working vocabulary

These terms exist only to keep the study's questions distinct. None defines
what Spock's future `view` will mean.

| Term | Meaning in this study |
| --- | --- |
| **SQL view** | A named derived table created in a database catalog using the database's view facility. |
| **Derived relation** | A relation computed from other relations, independent of whether it is stored as a catalog object. |
| **Public contract** | A named shape and behavior on which external consumers may depend. |
| **Exposure** | Discoverability and reachability through a protocol surface. |
| **Authorization** | Whether a particular actor may perform a particular action or observe particular data. |
| **Gateway resource** | A server-owned virtual resource whose behavior may exceed an SQL relation. |
| **Outer composition** | Additional projection, filtering, ordering, counting, or paging applied by a consumer to a relation-shaped result. |
| **Relation-returning operation** | A callable operation whose result is a typed relation, whether or not it is called a view. |
| **Updatability** | Whether insert, update, or delete can be expressed against a derived result. |
| **Write translation** | The rule that maps a mutation of a derived result back to authoritative base data. |
| **Provenance** | Information connecting an output field or row to the source data from which it was derived. |
| **Logical view** | The observable semantics of a derived result, without committing to a physical execution mechanism. |
| **Materialized result** | Stored derived data with an explicit freshness and refresh contract. |
| **Lowering** | A backend-specific implementation of a language-level contract. |

In particular, *public* may mean a stable contract, a reachable endpoint,
anonymous access, or some combination. The word alone is not precise enough
to settle exposure or authorization.

## Current record

The repository contains several useful but conflicting positions. Their
authority matters.

| Record | Relevant position | Authority for this study |
| --- | --- | --- |
| [Spock v0 specification](../spec/v0.md) | `view` is unsupported and reserved. Every table currently supplies an all-public identity read surface in the ungoverned prototype tier. | Normative for current behavior. |
| [README doctrine](../../README.md) | Describes views as named public projections and says the core concepts intentionally correspond to SQL primitives. | Product doctrine, not a complete view design or current syntax specification. |
| [RFD 0002](../rfd/0002-day-one-concepts.md) | Describes views as the open read and potentially writable surface. | Legacy direction, not the current specification. |
| [RFD 0003](../rfd/0003-write-through-views.md) | Sketches field-level write translation across qualifying joins based on provenance. | Legacy direction with explicitly sketched semantics and illustrative syntax. |
| [RFD 0004](../rfd/0004-exposure-model.md) | Separates table storage from view exposure and makes an unbound view inert. | Discussion draft, not a decision. |
| [RFD 0012](../rfd/0012-fn-v2.md) | Establishes relation-shaped function returns while leaving universal filtering and paging for later work. | Legacy record originally marked shipped. The current specification separately defines shipped function behavior; this record is not a view design. |
| [RFD 0021](../rfd/0021-filter.md) | Gives compiler-derived surfaces a universal query composer while treating opaque read functions as an exception; says future views would be derived surfaces. | Legacy record originally marked accepted with its read half implemented. The current specification separately defines current query behavior; statements about future views remain non-normative. |
| [Instagram paper program](../../examples/instagram/v1.spock) | Exercises internal and exposed views, parameters, actor dependence, nesting, joins, ordering, limits, and selected writable fields. | Non-normative exploratory evidence. |
| [Instagram feedback](../../examples/instagram/v1-FEEDBACK.md) | Identifies nested paging and per-view limits as unresolved and contrasts Spock sketches with PostgreSQL behavior. | Non-normative analysis. |
| [Uhura v0 incubation draft](../../uhura/docs/spec/drafts/v0.md) | Uses *semantic view* for a renderer-neutral UI tree and assigns durable product truth to an external authority such as Spock. | Non-normative v0 design evidence, not a definition of Spock `view`. |

The records expose real design pressure, but combining the strongest claim from
each record does not automatically produce a coherent system.

## Problem and conflict map

### 1. Concept identity

- Is the construct a derived relation, a stable public contract, a gateway
  resource, or a combination with explicit boundaries?
- Does a declaration exist for internal composition, external use, or both?
- Is a physical database catalog object part of the observable contract or an
  implementation choice?
- Which properties must remain equivalent when two backends use different
  lowerings: row membership, types, null behavior, queryability, security,
  mutation, errors, or all of them?
- What distinguishes a view from a table, record, relation-returning function,
  page response, materialized result, or Uhura semantic view?

This is not only a naming question. Each identity implies different closure,
portability, introspection, and compatibility obligations.

### 2. Declaration, public authority, exposure, and authorization

- Does declaring a view make it discoverable or reachable?
- Are declaration, protocol export, role grant, and row authorization separate
  decisions or one decision?
- Does *public authority* mean that the view owns only field names and types,
  or also row membership, identity, order, page behavior, errors, and mutation
  capabilities?
- Can an internal derived relation later be exported without changing its
  meaning?
- Can different roles observe different row types, different rows, or both?
- Is anonymous use a role, absence of an actor, or a separate surface?
- When a public view and a table expose the same facts, which surface is the
  consumer contract? Can one bypass the other?
- Does direct database access remain in scope, and if so, can it bypass the
  intended public authority?

The current v0 table identity surface makes this a migration question as well
as a new-declaration question.

### 3. Relational shape and cardinality

- Must a view produce a flat, homogeneous zero-or-more-row relation?
- Can it instead or additionally represent exactly one row, an optional row, a
  scalar, JSON, a nested object, a nested collection, or a page envelope?
- Are results bags, sets, or keyed collections? Are duplicate rows observable?
- Must every externally queryable result have a stable key? Can a key be
  hidden from the public row shape?
- What capabilities remain meaningful for keyless aggregate or joined results?
- Are field types and nullability inferred, declared, or checked in both
  directions?
- How do outer joins and missing related rows affect nullability, cardinality,
  and identity?

Flat relations preserve ordinary relational composition. Nested and
page-shaped results may be useful product contracts, but they introduce a
different query tree and blur the boundary with gateway response assembly.
The study does not assume either model.

### 4. Relational expressiveness, joins, and provenance

- May a view read from one source, several sources, another view, a
  relation-returning operation, or no table at all?
- Which join forms are representable: inner, outer, semi, anti, self, lateral,
  functional, and to-many?
- Are joins derived only from declared references, or may they use arbitrary
  conditions?
- Are recursion, cycles, common table expressions, set operations, grouping,
  aggregates, windows, and `DISTINCT` within the same concept?
- Which expression subset is deterministic and portable across supported
  backends?
- May evaluation depend on actor, parameters, time, randomness, session state,
  or remote data?
- Does required input change the construct into a relation-returning operation
  or gateway resource?
- Which provenance facts are required for identity, dependency analysis,
  policy, code generation, and possible mutation?

Read joins and write translation through joins are independent capabilities.
Allowing one does not answer the other.

### 5. Client-side composition

- Can a consumer apply projection, filtering, sorting, counting, aggregation,
  limits, paging, or relationship embedding to the result?
- Can it filter or sort by fields it did not select?
- Are computed fields filterable or sortable? Is support declared per field?
- Do the same operators apply to a typed relation returned by a function?
- When the authored relation already filters, orders, limits, groups, or pages,
  how does an outer request compose with it?
- Must outer operations be pushed into the authoritative query, or is pushdown
  only an optimization? Is client-side evaluation ever observable or allowed?
- How are cost, depth, breadth, offset, row, and execution-time bounds stated?
- Which query capabilities are visible to generated clients and generic tools?

[PostgREST functions](https://docs.postgrest.org/en/stable/references/api/functions.html)
are relevant counterevidence to treating composability as exclusive to an SQL
view: table-valued functions can receive the same filtering and paging
operators as tables and views when the return type is a known relation.
[Supabase filters](https://supabase.com/docs/reference/javascript/using-filters)
and [result modifiers](https://supabase.com/docs/reference/javascript/using-modifiers)
similarly compose after relation-shaped RPC calls. These systems do not settle
Spock's design; they show that result shape and SQL-object identity are
separate axes.

### 6. Ordering, pagination, and snapshots

- Does an authored order define result semantics, a default, a required
  prefix, a ranking clients cannot replace, a pagination key, or a physical
  hint?
- If a client supplies an order, does it replace, prepend to, or append after
  the authored order?
- How is a stable total order obtained when values tie or no key exists?
- Which paging models are available, and are they view capabilities or
  protocol-wide capabilities?
- What must a cursor bind to: view identity, schema version, actor, policy,
  parameters, order, query, or database snapshot?
- What happens when rows are inserted, deleted, or change visibility between
  pages?
- Do nested collections have independent order and paging contracts?
- Does one request observe one database snapshot across all projected and
  related data?

The Instagram draft's keyset assumption and the current bounded-offset query
layer are conflicting evidence, not an answer.

### 7. Mutation and the view-update problem

- Is mutation absent, inherited from a database, inferred from provenance, or
  separately authorized?
- Does proof that a write translation exists also authorize the public write,
  or are feasibility and author intent distinct?
- Are update, insert, delete, upsert, and bulk mutation separate capabilities?
- Is capability stated per view, output field, operation, role, or some
  combination?
- What happens to computed fields, aliases, key fields, immutable fields,
  filtered rows, outer-joined rows, fan-out rows, and shared parent rows?
- If an update makes a row no longer satisfy the view predicate, did the
  operation succeed, fail, or violate the view contract?
- For insert, where do hidden required fields, defaults, generated values, and
  joined rows come from?
- For delete, which authoritative rows are removed?
- For upsert, what selects the conflict identity?
- Can a single request update several base tables atomically? What is the
  failure behavior if a later base write fails?
- What concurrency token, if any, represents several underlying rows?
- At what point does a write become a deliberate operation because it carries
  an invariant, side effect, or product-specific refusal?

SQL products do not provide one portable answer. PostgreSQL documents
automatic updates for qualifying simple views, mixed updatable and computed
columns, check options, and trigger-defined behavior for other cases in
[`CREATE VIEW`](https://www.postgresql.org/docs/current/sql-createview.html).
SQLite describes views as read-only and uses `INSTEAD OF` triggers for writes
in [`CREATE VIEW`](https://www.sqlite.org/lang_createview.html). MySQL, SQL
Server, and Oracle define their own eligibility and trigger rules. Therefore,
“mapped to SQL `VIEW`” does not by itself determine Spock's mutation contract.

### 8. Policy, actor context, and security

- Does policy attach to base tables, derived relations, exported resources,
  operations, or several layers with a precise composition rule?
- Are field visibility, row visibility, operation authorization, and protocol
  exposure distinct?
- Is actor context ambient, explicit input, or policy-only context?
- Is a view deterministic relative to a database snapshot and actor even when
  it is not globally context-free?
- How do policies compose across joins and dependent views?
- Do invoker and definer rights, row-level security, security barriers,
  check options, functions, and grants produce equivalent observable behavior
  across lowerings?
- Is denial represented as an absent schema member, an unreachable route, a
  refusal, an empty relation, or a missing row?
- Can counts, sorting, uniqueness failures, timing, relationship presence, or
  cursors reveal hidden rows?
- Do by-key access, outer filters, relationship expansion, writes, caches, and
  subscriptions all pass the same authority boundary?
- How are cached or subscribed results revoked when roles or policies change?

Security cannot be evaluated only from the rows returned by one happy-path
query. Complete mediation and inference resistance require adversarial cases.

### 9. Logical, physical, materialized, and live behavior

- Is an SQL `VIEW` required, preferred, optional, or unrelated to the
  language-level concept?
- Can a backend execute the relation directly without creating a catalog
  object? Is that observable?
- Are a logical view, materialized view, cache, index, and incremental
  projection different concepts or modes of one concept?
- What freshness guarantee applies to stored derived data?
- Who owns refresh scheduling, invalidation, failed refresh, startup rebuild,
  and observability?
- Can actor- or parameter-dependent results be materialized safely?
- What snapshot and ordering semantics apply to live updates?
- Do subscriptions send snapshots, patches, invalidations, or another form?
- How do backpressure, resumption, policy changes, and missed updates behave?

PostgreSQL treats
[`CREATE MATERIALIZED VIEW`](https://www.postgresql.org/docs/current/sql-creatematerializedview.html)
as a distinct facility. That distinction is useful evidence but does not
require Spock to copy it.

### 10. Errors and concurrency

- Is zero rows an ordinary relation result, `null`, not found, or dependent on
  a separately derived singleton accessor?
- Which failures are protocol-owned query errors, policy denials, derived
  schema errors, or authored product errors?
- Can evaluating a view deliberately produce a product error, and if so, how
  does it remain relationally composable?
- How are unknown fields, unsupported operators, bad parameters, excessive
  cost, and malformed cursors reported?
- Which errors can a multi-table mutation produce, and how are they exposed to
  clients without partial success?
- What happens when a row disappears or changes authorization between read and
  write?
- Are writes last-write-wins, conditionally applied, or version-checked?

The existence of explicit product-error declarations elsewhere in Spock does
not decide whether a relation itself can raise them.

### 11. IR, namespace, protocol, and tooling

- What backend-independent facts must an inspectable view definition carry:
  fields, types, nullability, dependencies, derivation, key, provenance,
  exposure, policy, ordering, query capabilities, and mutation capabilities?
- Is the derivation a checked relational structure or opaque backend text?
- Which facts belong to contract IR, and which are per-request query state?
- Do tables, views, records, and functions share a namespace? Which generated
  SQL, REST, GraphQL, and client names can collide?
- If a view has inputs, how are their names distinguished from query controls
  and output fields?
- Are tables and views distinguishable in protocol routes, or intentionally
  uniform?
- Does every view receive list and by-key GraphQL fields? How are relationships
  to and from views represented?
- Can clients inspect whether a result is exported, keyed, filterable,
  sortable, writable by each operation, actor-dependent, materialized, or
  live?
- Can two conforming runtime implementations be tested for equal row, null,
  order, policy, error, and mutation behavior?
- What does editor preview mean when a view requires actor context, parameters,
  large data, or unsupported backend capabilities?

Opaque SQL may be useful as exploratory evidence, but it cannot alone answer
dependency, portability, policy-composition, provenance, or generic-client
questions.

### 12. Evolution and compatibility

- If a view is public authority, which changes are breaking: name, field,
  type, nullability, key, cardinality, predicate, role grant, policy, order,
  page cap, or mutation capability?
- What is the compatibility unit when roles see different schemas or rows?
- How are changes represented in surface diffs and generated-client versions?
- How are dependent views ordered, replaced, or rejected during migration?
- Can a physical replacement preserve grants and triggers while changing
  public semantics, or the reverse?
- When does a cursor become invalid after a contract change?
- Can aliases or versioned views preserve an older contract?
- How can explicit views coexist with or replace v0's automatic public table
  identity surfaces without creating two authorities or an accidental bypass?
- Does introducing governance after an ungoverned tier change the same release
  boundary as introducing views?

Public-contract compatibility is a stronger obligation than the dependency
rules of an internal SQL helper view.

### 13. Spock and Uhura boundary

- Where does authoritative data projection end and presentation composition
  begin?
- At what point do nested objects and collections become a page or component
  tree rather than a reusable data relation?
- May a Spock-owned shape contain UI-session concepts such as selected tabs,
  modal state, viewport windows, or optimistic overlays?
- Which system owns query evaluation, canonical ordering, cursors, request
  coordination, local filters, and loading state?
- How does Uhura reference a Spock projection without copying its shape and
  becoming a second authority?
- How will documentation and tooling distinguish a Spock data view from
  Uhura's renderer-neutral semantic view?

The existing authority rule is evidence: Uhura owns experience behavior and
non-authoritative session state; Spock or another provider owns durable facts,
accepted mutations, and authorization outcomes. A future data-view design must
be checked against that boundary rather than silently redefining it.

## Why the problem is hard

The conflict categories are not independent in their consequences:

- Shape determines possible identity, composition, pagination, protocol, and
  generated types.
- Query structure affects whether a database considers a view updatable.
- Mutation couples provenance, policy, errors, transactions, concurrency, and
  compatibility.
- Parameters and nested responses change the boundary between a relation and a
  callable gateway operation.
- Client composition changes both security risk and operational cost.
- Actor context changes determinism, caching, materialization, and cursor
  validity.
- Public authority creates compatibility obligations that internal database
  views do not necessarily carry.
- Backend-native view behavior differs, and eligibility may change after an
  otherwise ordinary query edit.
- A runtime query plan, a database catalog view, and a gateway resource can
  agree on simple examples while diverging on nulls, permissions, updates, and
  evolution.
- Existing Spock records were written at different stages and authority
  levels. Agreement on a slogan does not imply agreement on all observable
  behavior.

The central methodological risk is **bundle selection**: choosing one familiar
meaning of *view* and accidentally importing all of its associated behavior.
The study instead treats relation, export, authority, composition, mutation,
policy, and physical lowering as separate axes until evidence justifies
coupling them.

## Evidence plan

### SQL behavior

At minimum, reproducible experiments should compare SQLite, used by the
current implementation, with PostgreSQL, which appears as prior art and a
possible lowering in legacy design records but is not established here as a
final backend. MySQL, SQL Server, and Oracle provide useful counterevidence
when their behavior differs.

The experiment set should cover:

- named-query and non-materialization behavior;
- outer filtering, projection, grouping, and ordering;
- absence or presence of parameters;
- simple, joined, aggregate, windowed, union, and recursive views;
- whole-view and per-column updatability;
- check options and rows that leave a view after update;
- `INSTEAD OF` triggers, rules, and trigger-defined write behavior;
- invoker and definer rights, grants, row-level security, and security
  barriers;
- catalog metadata for dependencies, keys, updatability, and replacement;
- null, collation, time, and numeric differences; and
- materialized-view refresh and failure behavior.

Primary starting points include the [ISO SQL/Foundation standard record](https://www.iso.org/standard/76584.html),
[PostgreSQL `CREATE VIEW`](https://www.postgresql.org/docs/current/sql-createview.html),
[SQLite `CREATE VIEW`](https://www.sqlite.org/lang_createview.html),
[MySQL `CREATE VIEW`](https://dev.mysql.com/doc/refman/8.4/en/create-view.html),
[SQL Server `CREATE VIEW`](https://learn.microsoft.com/en-us/sql/t-sql/statements/create-view-transact-sql?view=sql-server-ver17),
and [Oracle `CREATE VIEW`](https://docs.oracle.com/en/database/oracle/oracle-database/26/sqlrf/CREATE-VIEW.html).

### Gateway behavior

PostgREST and Supabase should be studied separately from SQL itself:

- how tables, views, and functions become resources;
- how permissions determine visible verbs and schema entries;
- filtering, projection, ordering, counting, ranges, and pagination;
- composition after a table-valued function call;
- mutation of automatically updatable and trigger-backed views;
- relationship inference and embedding through views;
- filtering by selected and unselected columns;
- schema-cache invalidation;
- interaction with database roles and row-level security; and
- generated OpenAPI and client typing.

[PostgREST resource embedding](https://docs.postgrest.org/en/stable/references/api/resource_embedding.html)
is especially relevant because relationship inference through views depends
on information recoverable from their definitions and has documented limits.

### Research literature

The literature review should cover the view-update problem, relational lenses
and putback laws, provenance and key preservation, bag semantics, null
extension, query containment and rewriting, and incremental view maintenance.
Literature claims should be tied to the exact subset they establish rather
than imported as general authority.

### Observation record

Each experiment should record:

- product and exact version;
- complete setup schema, fixture data, and privileges;
- statement or protocol request;
- result, error, and relevant catalog state;
- query plan when performance or pushdown is at issue;
- whether the observation is specified, implementation-specific, or inferred;
- counterexamples and unsupported cases; and
- enough cleanup and invocation detail to reproduce the result.

## Proposal and development harness

Here, **harness** means a repeatable set of questions, fixtures, observations,
and falsification cases against which competing proposals and experiments can
be evaluated. It is not yet an executable conformance suite, and it does not
encode a preferred answer.

For every harness case, a proposal or prototype should classify the behavior
as one of:

- supported with defined observable behavior;
- rejected statically;
- rejected at runtime with a defined failure;
- backend-dependent with an explicit portability boundary;
- deliberately deferred; or
- outside the concept.

Silence is not evidence of simplicity. An unclassified case is an incomplete
result.

### Required decision ledger

Any future proposal should give an explicit answer or deferral for each of
these axes:

| Axis | Minimum recorded decision |
| --- | --- |
| Concept identity | Relation, public contract, gateway resource, physical object, or stated combination. |
| Declaration and export | Whether existence, discoverability, exposure, and authorization are separate. |
| Authority | Which shape and behavior consumers may treat as canonical. |
| Physical mapping | Required, optional, or prohibited lowerings and their equivalence boundary. |
| Row model | Flat or nested, cardinality, duplicate semantics, field typing, and nullability. |
| Inputs and context | Parameters, actor dependence, determinism, and function boundary. |
| Sources and joins | Allowed dependencies, join classes, recursion, and aggregate forms. |
| Identity | Required, inferred, hidden, absent, or capability-dependent keys. |
| Client composition | Projection, filtering, ordering, count, page, embed, cost, and pushdown behavior. |
| Mutation | Capability by operation and field, authorization, translation, transaction, and concurrency rules. |
| Policy | Attachment points, composition, denial, complete mediation, and side channels. |
| Materialization and live data | Freshness, snapshot, refresh, cache partitioning, and subscription behavior. |
| Errors | Empty and missing behavior, protocol failures, product failures, and write failures. |
| Evolution | Public compatibility unit, surface diff, dependencies, cursor validity, and migration. |
| IR and protocols | Inspectable facts, namespace, REST, GraphQL, client generation, and conformance. |
| Current v0 surface | Coexistence with or replacement of automatic table identity exposure. |
| Uhura boundary | Ownership of authoritative projection versus presentation and session behavior. |

### Capability matrix

Each proposal should fill the following matrix for every representative result
class rather than making one whole-system claim.

Representative classes:

- internal and externally exposed;
- single-source and joined;
- keyed and keyless;
- actor-independent and actor-dependent;
- flat and nested;
- parameterless and parameterized; and
- logical and materialized.

Capabilities to classify:

- select, filter, project, order, limit, page, count, aggregate, and embed;
- update, insert, delete, upsert, and bulk mutation;
- subscribe or otherwise observe changes;
- introspect and generate clients; and
- lower consistently to PostgreSQL, SQLite, and a non-catalog runtime plan.

### Shared fixture world

The existing Instagram domain supplies realistic pressure without adopting its
paper syntax. A repeatable fixture should include:

- a public profile, private profile, follower, blocked actor, anonymous actor,
  and moderator;
- public and private posts, including equal sort values;
- optional related rows and absent one-to-one rows;
- ordered media and comments;
- follows, blocks, saves, collections, hashtags, reports, and notifications;
- profile and notification-preference rows split across tables;
- deliberately duplicated join output;
- a keyless aggregate result; and
- rows whose policy visibility changes during a scenario.

Named examples such as profile summaries, post details, a home feed, personal
collections, account settings, and a moderation queue may be used as fixture
labels. They do not imply declarations or syntax.

### Read and shape cases

| ID | Case |
| --- | --- |
| R1 | Direct field projection and rename from one source. |
| R2 | Computed scalar field. |
| R3 | One-to-one join. |
| R4 | Optional outer join and null extension. |
| R5 | To-many join producing duplicate rows. |
| R6 | Grouped or aggregate result with no obvious row key. |
| R7 | Derived relation depending on another derived relation. |
| R8 | Actor-dependent row membership or field value. |
| R9 | Required lookup or search input. |
| R10 | Nested object and independently sized nested collection. |
| R11 | Recursive or cyclic dependency. |
| R12 | Time-dependent or otherwise non-deterministic expression. |

### Client-composition cases

| ID | Case |
| --- | --- |
| Q1 | Outer projection, filtering, and ordering. |
| Q2 | Filter or order using an unselected field. |
| Q3 | Filter or order using a computed field. |
| Q4 | Relationship embedding through a derived result. |
| Q5 | Count and aggregation over the result. |
| Q6 | Equal sort values and stable pagination. |
| Q7 | Concurrent insert, delete, or policy change during pagination. |
| Q8 | Independent nested-collection filtering and paging. |
| Q9 | The same operations after a typed relation-returning function. |
| Q10 | Query exceeding depth, breadth, row, offset, or cost limits. |

### Mutation cases

| ID | Case |
| --- | --- |
| W1 | Update a direct field from one base table. |
| W2 | Attempt to update a computed field. |
| W3 | Update causes the row to leave the filtered result. |
| W4 | One request updates fields from two joined tables. |
| W5 | A joined related row is absent. |
| W6 | Several result rows identify one shared base row. |
| W7 | A constraint fails after another base write would have succeeded. |
| W8 | Concurrent update and version conflict. |
| W9 | Bulk filtered update. |
| W10 | Insert with hidden required, defaulted, or generated fields. |
| W11 | Delete through a multi-source result. |
| W12 | Upsert and conflict-target selection. |
| W13 | Mutation also requires a product invariant or external effect. |

### Security cases

| ID | Case |
| --- | --- |
| S1 | Anonymous, ordinary user, owner, and moderator observe one candidate. |
| S2 | A blocked viewer receives no protected row. |
| S3 | Public result access is allowed while base-table access is denied. |
| S4 | A hidden field is referenced by a client filter or order. |
| S5 | Count, error, timing, relationship, or cursor reveals hidden-row existence. |
| S6 | View owner and invoking actor have different privileges. |
| S7 | Base-table policy and result-level policy disagree. |
| S8 | A policy change invalidates cached, paged, or subscribed results. |
| S9 | A direct table route remains available beside an intended authoritative view. |

### Portability and lifecycle cases

| ID | Case |
| --- | --- |
| P1 | PostgreSQL native-view lowering. |
| P2 | SQLite native-view lowering. |
| P3 | Runtime-plan or gateway execution without a catalog view. |
| P4 | A backend lacks one claimed capability. |
| P5 | Add, rename, remove, or change nullability of an output field. |
| P6 | Change predicate, key, order, policy, or mutation capability. |
| P7 | Change a base table used by dependent derived shapes. |
| P8 | Preserve or replace grants, triggers, generated routes, and clients. |
| P9 | Materialized or cached execution becomes stale. |
| P10 | Runtime schema cache or generated client is out of date. |
| P11 | Two lowering strategies disagree on null, order, policy, or error behavior. |

### Spock and Uhura boundary cases

| ID | Case |
| --- | --- |
| U1 | Uhura consumes a versioned Spock-owned projection without copying authority. |
| U2 | A nested backend result begins to encode page or component structure. |
| U3 | Local filter, selected sort, viewport, or loading state is mistaken for backend truth. |
| U4 | An optimistic value remains provisional until Spock accepts the mutation. |
| U5 | A cursor and canonical ordering remain Spock-owned while request coordination remains Uhura-owned. |

### Required observations and artifacts

A future proposal or prototype should provide, where relevant:

- a completed decision ledger and capability matrix;
- a supported, rejected, deferred, or out-of-scope classification for every
  fixture;
- observable failure behavior for runtime rejections;
- an authority graph showing ownership across base data, derived data, policy,
  gateway, generated clients, and Uhura;
- inspectable contract IR or an equivalent semantic artifact;
- a dependency and provenance trace;
- backend lowering or interpreter traces for every portability claim;
- REST, GraphQL, introspection, and generated-client consequences;
- a security and side-channel matrix;
- compatibility and surface-diff examples;
- query plans and bounded performance observations for composition claims;
- explicit limitations and deferred cases; and
- counterexamples capable of falsifying the proposal.

### Falsification cases

A proposal needs revision if its stated model permits any of the following
without explicitly classifying it as intended behavior:

- a harmless-looking query refactor silently changes public write capability;
- a client filter weakens the authored or policy-owned row predicate;
- hidden rows leak through counts, ordering, errors, relationships, timing, or
  cursor behavior;
- PostgreSQL and SQLite lowerings disagree on observable semantics while both
  claim conformance;
- a keyless or duplicate-producing result claims stable pagination without an
  identity strategy;
- actor-specific cached data is reused across actors;
- a direct table route bypasses a shape presented as public authority;
- a multi-table write partially succeeds;
- nested backend data makes presentation or UI-session state authoritative in
  Spock; or
- a generated client exposes capabilities the runtime cannot guarantee.

### Development guardrails

Any exploratory implementation used as study evidence should state:

- the exact question it tests;
- the supported subset and all known unsupported cases;
- whether behavior is portable, backend-specific, or only a mock;
- how it is isolated from default and normative behavior;
- how users and tools can inspect its true capabilities;
- what evidence would falsify it; and
- its removal, revision, or formal graduation path.

A prototype is evidence only. It does not change the specification,
conformance expectations, default behavior, or design status.

## Counterevidence and alternative interpretations

Several observations prevent the problem from being reduced to one familiar
meaning of *view*:

- The repository uses *view* both for a public projection and for an inert
  internal declaration, so existing terminology does not resolve whether
  declaration and exposure are identical.
- SQL products disagree on automatic updatability and trigger-based write
  behavior, so SQL naming alone does not resolve mutation semantics.
- PostgREST applies outer query operations to typed relation-returning
  functions, so filterability and sortability do not prove that a resource is
  an SQL view.
- Flat single-source examples can be represented by a catalog view, an inline
  query, or a gateway plan with the same apparent rows; they do not expose the
  differences in portability, security, mutation, or evolution.
- Nested paper examples demonstrate product pressure but do not establish that
  nested response assembly belongs to the same concept as a derived relation.
- Existing write-through sketches demonstrate a desired client experience but
  do not establish portable write translation or authorization rules.

These are reasons to preserve the unresolved axes in the harness, not reasons
to select a system.

## Findings so far

The evidence currently supports only problem-level observations:

1. SQL view identity, public exposure, actor authorization, client
   composability, and writability are separable concerns.
2. “Maps to SQL `VIEW`” is insufficient to determine portable mutation,
   security, materialization, or protocol behavior.
3. Relation-shaped function results in existing gateways demonstrate that
   outer filtering and sorting do not uniquely identify a view.
4. Read joins and write translation through joins must be evaluated
   independently.
5. Making a view the authoritative public shape introduces compatibility and
   migration obligations beyond those of an internal named query.
6. v0's automatic table exposure means an explicit public-view design cannot
   be evaluated as a purely additive declaration.
7. Nested, parameterized, actor-dependent, materialized, and writable results
   each change more than one semantic axis.
8. No current Spock record resolves the full set of conflicts at one coherent
   authority level.

These findings narrow the questions. They do not select a design.

## Limitations

- No syntax has been evaluated.
- No proposed semantic bundle has been scored or ranked.
- The SQL and gateway experiments identified above have not yet been collected
  into reproducible repository fixtures.
- The full ISO SQL text is not freely reproduced here; vendor documentation is
  used conservatively for product behavior.
- Performance, optimizer behavior, and security side channels have not yet
  been measured.
- No commitment is made to a future database backend or lowering strategy.
- No compatibility schedule for replacing or retaining v0 table exposure is
  proposed.
- The fixture list is deliberately broader than a plausible first version so
  that deferrals become explicit.

## Implications

A responsible future proposal needs to declare which meaning of *view* it is
using, which associated capabilities it includes, and which it deliberately
does not include. It also needs evidence across the coupled axes rather than a
single favorable SQL example.

The study can be useful even if the eventual outcome is no language change, a
smaller construct, several separate constructs, or a decision to gather more
evidence.

## Follow-ups

- Link this study from a language-problem issue when one is opened.
- Turn the SQL and gateway evidence plan into versioned, reproducible
  experiments.
- Reduce the fixture list only after each omitted case has an explicit reason.
- Record new conflicts as problem statements before adding candidate syntax.
- If a formal working group is chartered, copy or reference the evidence under
  its numbered record without changing this study's authority.
- If a sponsored RFD follows, require it to complete the harness and identify
  every deliberate deferral.

Completion of this study means the evidence and decision space are
reproducible. It does not mean that a design has been selected.

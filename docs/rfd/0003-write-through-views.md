# RFD 0003 — Write-through views

Status: accepted direction. Semantics sketched; every syntax and API fragment
is illustrative.

## The idea

`fn` is the deliberate way to write two tables at once, and for genuinely
operational writes it is the right way: `place_order` inserts an order,
decrements inventory, and opens a payment intent — that is an invariant-
carrying operation someone chose to define.

But most multi-table writes are not operations at all. A profile settings form
edits `user_profile.display_name` and `notification_prefs.email_digest` — two
tables, zero invariants, just fields that happen to live in different places.
Forcing that through a `fn` is ceremony; forcing it through two separate
single-table writes leaks storage layout into the client.

So: views stay writable across joins, per field, wherever the write can be
traced back.

```text
table a: a1, a2, a3
table b: b1, b2, b3

view z = join(a, b): a1, a2, a3, b1, b2, b3, q1
         └── write-through ──────────────┘  └ computed, read-only
```

All of `a*` and `b*` are mutable through `z`'s endpoint and schema — even
renamed, reshaped, or nested — because each traces to exactly one base column.
`q1` is an expression; it cannot traverse back, so it is read-only by
construction.

## Why views are read-only everywhere else

The SQL standard only makes *simple* views auto-updatable: one base relation,
no joins, no aggregates. A joined view needs hand-written `INSTEAD OF`
triggers, because the database cannot infer where writes should land.
PostgREST (and Supabase's pg-meta) reflect the catalog — introspection cannot
invent putbacks it cannot see. The limitation is upstream of those tools.

Spock is not reflecting a catalog; it owns the whole contract at compile time.
Column provenance is known for every view field, so the compiler can derive
the putback — natively in the prototype runtime, or as generated `INSTEAD OF`
triggers in a future `spock2sql` mode.

## The rule: writability is provenance

A field is writable exactly when the compiler can prove where the write lands.
Per-field classification:

1. **Write-through.** The field traces to exactly one base column through
   pure structural mapping (rename, reshape, nest), along a *functional* join
   path — foreign key to unique key — so each view row identifies exactly one
   base row per table. Note: unambiguous is not isolated; writing a shared
   parent row through one view row is visible to its siblings, exactly as it
   is on the base tables.
2. **Computed.** Any expression (`q1`) has no putback: read-only. A declared
   inverse (a user-supplied put) is a possible future extension.
3. **Fan-out.** Fields flattened from a to-many join are read-only — the
   putback is ambiguous. But *nested* child objects keyed by their own primary
   key remain write-through; nesting preserves provenance.

Additional rules that compose with the rest of the language:

- A writable view must expose a stable key for addressing.
- One write touching columns of several base tables compiles to **one
  transaction** — atomic, always.
- **Policy composes**: a view write is allowed iff every underlying column
  write is allowed. Complete mediation is preserved through the join.
- **Errors compose** (RFD 0002): the outcome set of a view write is the union
  of the derived constraint outcomes of every touched base column. A write to
  `z` can reject with `a`'s unique violation or `b`'s check violation, and the
  linter demands both be acknowledged.

## The theory, briefly

This is the classic **view update problem** from database literature, and the
discipline that solves it in the small is the **lens**: every field has a
*get*; a field is writable exactly when a lawful *put* can be derived, where
lawful means the round trip holds — put a value, get it back unchanged. The
functional-join-path condition above is the classical requirement for
unambiguous update translation. "Reverse-trackable" is the right intuition;
provenance is its name.

## Scope

`UPDATE` first. `INSERT` and `DELETE` through a joined view are ambiguous in a
way updates are not (which tables participate, in what order?) — deferred.
Single-table views and `fn` cover creation and deletion meanwhile.

## Client surface

Codegen from `/~contract` makes writability visible in the client's type
system — the LINQ inversion: what cannot be written is unrepresentable as a
write, at compile time, not discovered at runtime.

```ts
// generated — illustrative
interface Z {
  readonly id: string
  a1: string
  a2: string
  a3: string
  b1: string
  b2: string
  b3: string
  readonly q1: number
}

const z = await spock.from("z").where(eq("id", id)).first()
await spock.from("z").update({ a1: next })   // PATCH → one transaction
```

A proxy-object style (`z.a1 = next` queuing the patch) is optional client
sugar on the same surface.

## Open questions

- **Concurrency.** Last-write-wins vs optimistic concurrency (row version /
  `If-Match`). The "optimistic" client style — apply locally, sync behind —
  pairs naturally with version-checked writes; unresolved.
- Declared inverses for computed fields.
- `INSERT`/`DELETE` semantics through joins.
- Ergonomics of deep reshaping: provenance stays derivable, but at some depth
  it stops being readable to humans; possibly a lint.

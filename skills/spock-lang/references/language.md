# Spock v0 Language Reference

Use this reference for current accepted syntax. Consult `docs/spec/v0.md` for complete semantics and diagnostic codes.

## Lexical rules

- Files are UTF-8 and conventionally end in `.spock`.
- Identifiers match `[a-z_][a-z0-9_]*`.
- Strings support `\"`, `\\`, `\n`, and `\t` escapes.
- Triple-quoted strings are multiline raw strings with no escape processing.
- Integers are signed decimal `i64` values. Floats require digits on both sides of `.` and do not support exponents.
- `//` and `////` or longer are ordinary comments. Exactly `///` documents the next declaration or field. `//!` documents the file and must appear before declarations.

Active keywords:

```text
table auth record fn mut key unique check seed on delete restrict cascade
set null text int float bool timestamp uuid auto now me true false
```

`unchecked` and `sql` are contextual in function bodies. `file` is soft syntax in seed values. Future-reserved words are not accepted as identifiers:

```text
view role policy error state extern unsafe derived protected module enum
expect transition upsert match with
```

## Tables and fields

```spock
auth table user {
  key id: uuid = auto
  username: text unique
  display_name: text
}

table post {
  key id: uuid = auto
  author: user on delete cascade
  body: text
  status: "draft" | "published" = "draft"
  published_at: timestamp?
}
```

Builtins are `text`, `int`, `float`, `bool`, `timestamp`, and `uuid`. A declared table name in type position is a reference that stores the target key. Add `?` for optional values.

Field modifiers must stay in this order:

```text
[key] name: type [?] [check fn] [unique] [= default]
[on delete restrict|cascade|set null]
```

Defaults:

- `auto` on `uuid`
- `now` on `timestamp`
- `me` on a reference to the one `auth table`
- a type-compatible literal

Use a composite key or unique group for multi-field identity:

```spock
table follow {
  key (follower, followed)
  follower: user on delete cascade
  followed: user on delete cascade
  at: timestamp = now
}
```

References cannot target a composite-key table. `on delete set null` requires an optional reference.

## Constraints and validators

Use a closed set for a stored string choice:

```spock
status: "draft" | "published" | "archived"
```

Closed sets are table-field types only in v0. They cannot be keys, record fields, or function parameters.

Use a read function returning `bool` as a value validator:

```spock
fn nonempty(value: text) -> bool {
  unchecked sql("SELECT length(trim(:value)) > 0")
}

table comment {
  key id: uuid = auto
  body: text check nonempty
}
```

Validator functions have stricter laws: one deterministic, clauseless boolean `SELECT`, matching parameter types, and no writes. Cross-column checks use `check (field_a, field_b) validator` inside the table.

## Records

Records are scalar return shapes for functions, not stored data or parameter types:

```spock
record post_summary {
  id: uuid
  body: text
  author_name: text
}
```

Record fields must be builtin scalars. Do not use keys, references, constraints, defaults, or delete actions inside records.

## Functions

Use `fn` for reads and `mut fn` for operations that may write:

```spock
fn find_post(id: uuid) -> post? {
  unchecked sql("SELECT * FROM post WHERE id = :id")
}

mut fn publish_post(id: uuid) -> post ! not_found {
  unchecked sql("UPDATE post SET status = 'published' WHERE id = :id")
  unchecked sql("SELECT * FROM post WHERE id = :id")
}
```

Return forms:

- `t`: exactly one row or scalar
- `t?`: zero or one
- `[t]`: any number

Every body contains one or more `unchecked sql("...")` escapes. Each escape carries exactly one row statement. Statements execute in order in one transaction; the final statement answers the call. Only `:name` placeholders are accepted, and declared parameters must be used.

The final result columns must match the declared table or record field names. A scalar return requires one result column. Use `RETURNING` when the answering statement is DML.

Mint a product refusal in the signature and raise it conditionally:

```spock
mut fn follow_user(target: user) -> follow ! cannot_follow_self {
  unchecked sql("""
    SELECT spock_refuse('cannot_follow_self')
    WHERE spock_actor() = :target
  """)
  unchecked sql("""
    INSERT INTO follow (follower, followed)
    VALUES (spock_actor(), :target)
    RETURNING *
  """)
}
```

## Seed data

Seed statements execute in file order. Bind rows before referencing them:

```spock
seed {
  alice = user {
    id: "10000000-0000-4000-8000-000000000001",
    username: "alice",
    display_name: "Alice"
  }

  post { author: alice, body: "Hello", status: "published" }
}
```

Seed cannot call functions. A binding may only fill a reference to the same target table.

For file-backed seed data, reference the builtin `storage_object` table and use a relative path that stays under the source directory:

```spock
table photo {
  key id: uuid = auto
  image: storage_object
}

seed {
  photo { image: file("./seed/photo.jpg") }
}
```

---
description: The example portfolio — what each harness demonstrates, what runs, and what is deliberately not a program yet.
---

# Examples

The repository's [examples/](../examples/) directory holds product harnesses
and technical fixtures. A product harness starts from a real product problem
whose requirements must not bend around what Spock can express today; a
technical fixture may be small and artificial when it isolates one language or
runtime behavior. Examples are evidence, not the language definition — the
[v0 specification](spec/v0.md) governs current behavior.

## instagram — the runnable flagship

[examples/instagram/](../examples/instagram/) is the mature harness: an
implementation-independent [PRD](../examples/instagram/PRD.md), conventional
PostgreSQL answer sheets, and a complete, runnable authority in
[v0.spock](../examples/instagram/v0.spock) — twenty tables, forty-three
functions, and a seeded world. It requires toolchain 0.5.2 or later (it uses
the experimental `error` declaration preview). The directory also contains a
clearly labeled paper program, `v1.spock`, which intentionally does not parse:
it records what a future native statement grammar could look like, as design
evidence.

```sh
npx spock check examples/instagram/v0.spock
npx spock run examples/instagram/v0.spock
```

## uber — a PRD-only harness, by design

[examples/uber/](../examples/uber/) grounds a mobility product in market and
regulatory reality before any schema exists. It deliberately contains no
Spock program yet — grounding-first is the method, not an omission: the
harness doctrine requires the product to be understood independently of Spock
before a specialist answer is attempted.

## filter-lab — a technical fixture

[examples/filter-lab/](../examples/filter-lab/) is a deliberately artificial
schema built to stress the REST filter surface end to end, with recorded
feedback. It is the opposite of a product narrative, and that is its job.

```sh
npx spock check examples/filter-lab/schema.spock
```

## The full-stack example lives in the Uhura repository

The canonical framework example — authority, Uhura client, and `spock.toml`
served together by one `spock start` — is
[gridaco/uhura/examples/instagram](https://github.com/gridaco/uhura/tree/main/examples/instagram).
Its Play provider needs a one-time `pnpm` build the CLI does not perform;
that example's README documents the exact commands. See [Uhura](uhura.md) for
what the client language is and is not yet.

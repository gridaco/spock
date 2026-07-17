# Examples

This directory contains product harnesses and focused technical fixtures.

A product harness starts from a real product problem. Its requirements must not
shrink or bend around what Spock can express today. A technical fixture may be
smaller and artificial when its explicit purpose is to isolate one language or
runtime behavior.

Examples are evidence, not the language definition. The
[v0 specification](../docs/spec/v0.md) describes current normative behavior;
the [language-change process](../docs/governance/language-change-process.md)
governs how observed gaps can become supported language.

## Portfolio Doctrine

Examples exist to put Spock under pressure from products that were not designed
for it. A product harness should remain a useful backend case study if Spock did
not exist. It is not a demo chosen to flatter the current language, a CRUD
tutorial, or a checklist of features Spock ought to show.

The product is the authority. Its requirements come from genuine difficulties
in the existing market and from the conventional expertise used to address
them. Spock should fit and grow in response to that product; the product must
not be reduced, renamed, or rearranged to fit Spock.

A harness studies the consequential backend of a service, not only its
database. Depending on the product, that may include:

- customer, worker, partner, and operator experiences;
- identity and authority boundaries;
- stateful business lifecycles, time, concurrency, and asynchronous work;
- storage, search, money, messaging, and external providers;
- degraded operation, recovery, correction, disputes, and abuse;
- safety, privacy, policy, regulation, and observability.

These are not boxes every harness must tick. Auth, IAM, queues, storage, AI,
payments, and third-party integrations enter a harness only when the product
naturally requires them. We choose a coherent product first and discover its
system topology; we do not assemble a product around fashionable
infrastructure.

The product-harness portfolio is deliberately small. Broad candidate
exploration is input to selection, not a backlog that every candidate must
become. Products are compared by the hard pressures they add, not by an
industry label or by how many components they contain. A new product harness
must add a genuinely distinct center of gravity or replace a weaker harness.
Relatability and demo value are useful after admission, but are not admission
criteria.

## Selecting and Triaging Candidates

Selection proceeds from product truth toward portfolio fit:

1. **Fan out across concrete products.** Explore products or services with an
   end-to-end promise, not generic buckets such as “commerce” or “SaaS.”
2. **Ground candidates without Spock.** Study real product behavior, domain
   practice, standards, regulation, and provider contracts before selecting a
   Spock-specific schema, syntax, or architecture.
3. **Expose the whole consequential service.** Identify its actors, authority
   surfaces, lifecycles, invariants, external dependencies, failure paths,
   recovery paths, and operational controls.
4. **Compare candidates as a portfolio.** Prefer a product that introduces
   pressures not already forced by an existing harness. Defer a good product
   when its contribution is duplicative or still poorly grounded.
5. **Admit very few.** A candidate becomes a product harness only when it can
   pressure-test Spock without being tailored to what Spock currently supports.

Use this admission gate:

| Question | Required answer |
| --- | --- |
| Product truth | What concrete end-to-end promise does the service make, and what genuinely difficult market problem exists independently of Spock? |
| Service topology | Which actors, authorities, control planes, and external systems are consequential to fulfilling that promise? |
| Behavioral rigor | Can the harness state falsifiable workflows, lifecycles, invariants, failures, recovery behavior, and acceptance cases? |
| Grounding | Is there enough product, policy, standards, provider, regulatory, or domain evidence to write an implementation-independent PRD and eventually evaluate a specialist answer? |
| Portfolio value | What distinct pressure does this add beyond the existing harnesses, and what bounded starting scope can preserve that pressure? |
| Language value | What would we learn if current Spock cannot express the product honestly? |

A generic CRUD reskin, an infrastructure checklist, a database-only model of an
inherently service-wide product, or a happy path with no meaningful failure
semantics does not pass this gate. Neither does an enormous brand clone that
cannot define a bounded reference product.

## Current Product Portfolio

| Harness | Distinct portfolio contribution | Current maturity |
| --- | --- | --- |
| [`instagram`](instagram) | Social publishing and interaction: identity and social graph, viewer-relative visibility, media and storage, derived feeds, moderation, and content lifecycle. | Product PRD, PostgreSQL answer sheets, bounded runnable Spock slice and feedback, generated-client proof, and a historical paper program. |
| [`uber`](uber) | On-demand physical-world marketplace: passenger, driver, operator, and orchestration planes; location and time freshness; dispatch races; trip and safety lifecycles; money and reconciliation; provider and degraded-operation boundaries. | Sourced, implementation-independent PRD and acceptance profile; deliberately no schema, Spock program, answer sheet, or chosen architecture yet. |

A third product harness remains deliberately unselected. It should be added
only when it contributes a distinct product topology, not to make the portfolio
look complete or symmetrical.

## Other Example Classes

Technical fixtures and framework examples support the product portfolio, but
do not count as additional product harnesses:

| Path | Kind | Purpose and status |
| --- | --- | --- |
| [`filter-lab`](filter-lab) | Technical fixture | Artificial, focused filter, ordering, and pagination case with recorded feedback; exercised end to end by runtime tests. |
| [`uhura/examples/instagram`](https://github.com/gridaco/uhura/tree/main/examples/instagram) | Full-stack framework example | Maintained integration vehicle for the Instagram product: canonical combined Spock authority, complete Uhura client, and `spock.toml` project for `spock start` and `spock dev`. |

A technical fixture may be deliberately artificial because its job is to
isolate one language or runtime behavior. A framework example proves delivery
and integration around a harness. Neither substitutes for independently
grounding the product.

## Organizing a Product Harness

Each product directory should make these facts easy to find:

- the reference product and its bounded product promise;
- the consequential actors, surfaces, authorities, and external boundaries;
- the distinct pressure that earns the harness a place in the portfolio;
- the evidence used, the limits of that grounding, and unresolved research;
- the maturity and honest scope of each artifact.

Every product directory has a `README.md` as its stable entry point. It states
the product promise and bounds, portfolio contribution, grounding status,
artifact maturity, and navigation. It either contains the
implementation-independent product grounding or links to a `PRD.md`. A large
PRD may be split under `prd/` by stable product plane, authority, or lifecycle,
with the root PRD preserving the shared scope, vocabulary, invariants, and
acceptance profile. Do not split the product according to current Spock
constructs.

Keep conventional answer sheets, runnable Spock slices, feedback, paper
experiments, and full-stack integrations visibly distinct. Filenames and
version numbers are conveniences, not maturity claims and not a substitute for
stating whether an artifact is grounded, runnable, bounded, or speculative.

## Product-First Method

A product harness may progress through these forms:

1. **Product grounding** — a `README.md` or `PRD.md` defines the product
   promise, actors, surfaces, flows, lifecycles, invariants, failure and
   recovery cases, external authorities, acceptance cases, and open research
   without caring about Spock.
2. **Conventional answer** — one or more optional, domain-appropriate reference
   solutions show how established specialist stacks address a stated part or
   all of the requirements. They may include data models, SQL, service or API
   contracts, asynchronous workflows, provider boundaries, and operational
   controls. Each stands on its own and states its scope.
3. **Runnable Spock slice** — a deliberately bounded program expresses as much
   of the grounded product as the current toolchain can honestly run.
4. **Feedback** — a sibling `*-FEEDBACK.md` records missing concepts,
   workarounds, and product distortions found by building the slice.
5. **Design evidence** — a clearly marked paper program or isolated experiment
   may test a possible direction without presenting it as current Spock.

These are evidence tracks, not a ladder ranked by amount of code. A rigorously
grounded PRD with no implementation may be a stronger harness than a runnable
toy. Not every harness needs every form. A focused fixture is useful when a
product harness is too noisy to test one behavior precisely.

When Spock cannot express a grounded requirement, preserve the requirement and
record the gap. Do not simplify the product to make the language look complete.
A language change begins with the demonstrated problem and evidence; it does
not become an RFD until it reaches the sponsored proposal stage.

## Evolving Existing Examples

Examples are not grandfathered into the portfolio. When an existing example is
revisited, apply the same admission gate used for a new candidate. Existing
code is evidence about an earlier attempt; it is not the authority for
recovering the product.

Depending on the result:

- **re-ground and strengthen** a real product whose requirements were too
  shallow or bent around current Spock;
- **reclassify** an intentionally narrow or artificial case as a technical
  fixture;
- **split** a mixed example into a grounded product harness and focused
  fixtures or design evidence;
- **merge or absorb** an example whose useful pressure is already better
  represented elsewhere;
- **defer or retire** an example that is redundant, misleading, stale beyond
  recovery, or unable to state a unique contribution.

A harness needs re-triage when only happy-path CRUD remains, its grounding no
longer reflects the reference product, its distinctive pressure has
disappeared, or its requirements were silently shrunk to produce a cleaner
Spock program. Historical artifacts remain only when they are clearly labeled
and still provide useful evidence.

## Runnable, Preview, and Paper Programs

A runnable `.spock` file must pass `spock check` and state its scope honestly.
It should follow the normative specification unless it prominently identifies
an implementation preview as experimental, unstable, and non-normative.

Checker acceptance alone does not imply language acceptance. The current
toolchain, for example, parses the
[RFD 0024](../docs/rfd/0024-error-declarations.md) top-level `error`
declaration preview used by both Instagram backends. Those files carry their
own warning; the preview remains outside normative v0 while the RFD is a draft.

Unsupported syntax must not masquerade as a runnable example. The historical
[`instagram/v1.spock`](instagram/v1.spock) is a paper program: it intentionally
does not parse, marks each invented construct, and exists only as design
evidence. New experiments follow the current language-change process rather
than gaining special status from a filename.

## Current Instagram Artifacts

The Instagram harness established several useful artifact names. They describe
the files that exist there, not mandatory names for every future harness:

- [`PRD.md`](instagram/PRD.md) — implementation-independent product
  requirements.
- [`pg.sql`](instagram/pg.sql) and
  [`pg-rls.sql`](instagram/pg-rls.sql) — conventional PostgreSQL answer sheets.
- [`v0.spock`](instagram/v0.spock) — runnable standalone language slice,
  including the prominently marked `error` implementation preview.
- [`v0-FEEDBACK.md`](instagram/v0-FEEDBACK.md) — gaps and workarounds found by
  implementing that slice.
- [`v1.spock`](instagram/v1.spock) — non-runnable legacy paper program.
- [`v1-FEEDBACK.md`](instagram/v1-FEEDBACK.md) — review of that paper design.
- [`client`](instagram/client) — generated-client and GraphQL ecosystem proof.

The canonical maintained full-stack application is the
[`uhura/examples/instagram`](https://github.com/gridaco/uhura/tree/main/examples/instagram) framework project.
It packages its own backend, client, seeds, and manifest rather than treating
the standalone `v0.spock` file as a framework project.

## Try the Runnable Files

With a compatible installed Spock CLI:

```sh
spock check examples/instagram/v0.spock
spock check examples/filter-lab/schema.spock
spock run examples/instagram/v0.spock
```

The Instagram slice currently needs a CLI containing the RFD 0024
implementation preview (`0.5.2` or later). See the
[framework example README](https://github.com/gridaco/uhura/blob/main/examples/instagram/README.md) for full-stack
setup and commands.

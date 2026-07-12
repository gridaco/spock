# RFD 0019 — The external plane: latency and failure as first-class

Status: **discussion draft**. No implementation is proposed. This RFD reframes
storage (RFD 0018) as the *first instance* of a general category — **external
services** — and grounds a universal, DevTools-style latency/fault simulator as
future surface. In the tradition of RFD 0014 (the actor seam) and RFD 0017
(storage research), it is a design record a later milestone would build from;
**it commits to nothing and lands no code.** The separate ideation and design it
calls for is deliberately left to a follow-up.

## 0. The question

Storage v0 (RFD 0018) serves a file's bytes from a SQLite BLOB in microseconds,
and the read never fails. In production that same file lives in S3, a `GET`
takes ~100ms, and it can time out, 503, or rate-limit. So the prototype that
consumes Spock is being taught two falsehoods — *files are instant* and *files
never fail* — and it will pay for both the day it meets a real object store.

The question: should a prototyping backend **deliberately wear the latency and
failure of production**, and if so, how — and at what scope? The claim of this
RFD is that the answer is *yes*, that the right scope is **not storage** but
**every external surface**, and that the mechanism is a single orthogonal
control in the shape of Chrome DevTools' network throttle.

## 1. The doctrine: a prototype must not lie about production

Spock's identity is a prototyping language whose contract *refuses the
comfortable lie*. The actor seam (RFD 0014) refused the lie that a
client-asserted `actor` is trustworthy — *"the guard is theater."* Storage
(RFD 0018) refused the lie that a file is a detached blob rather than a governed
row. Both are the same move: make the prototype honestly shaped like production,
so the client built against it is the client production actually needs.

The most expensive lie a prototype can tell is **"everything is instant and
everything succeeds."** It is expensive because it changes the *shape of correct
client code*. Under real latency and failure, the consumer must build:

- **loading / pending states** — a spinner, a skeleton, a disabled control;
- **error and retry** — a 503 is not a bug, it is Tuesday; the client needs a
  retry, a backoff, an idempotency key;
- **optimistic UI and reconciliation** — show the change, then confirm or roll
  back when the external truth lands;
- **eventual-consistency handling** — write-then-read may not read-your-write
  when the read hits an external index or a replica.

A prototype whose external surfaces are instant and infallible *never exercises
any of this*, and so silently teaches the client to omit all of it. The demo is
flawless; production is a rewrite.

> **Doctrine.** The contract already refuses to lie about *authority* (0014) and
> about *files being real entities* (0018). This refuses to lie about **time and
> reliability**. A Spock prototype's external surfaces should be honestly shaped
> like production — slow, fallible, eventually consistent — so the prototype that
> consumes them is *forced* to build the states production will demand.

This is not a call to make Spock slow. It is a call to make the *degraded case
reachable on demand*, the way DevTools makes Slow 3G reachable without making
your machine slow.

## 2. The reframe: storage is one instance of "external"

The correct subject is not *storage*. Storage is the first member of a category.

**The external plane** = any capability backed by an out-of-process service,
distinguished from Spock's own local transactional database by all of:

- **non-trivial, variable latency** (tens to thousands of ms, not µs);
- **independent failure** — it can be unavailable, time out, rate-limit, or
  partially complete *while the database is perfectly healthy*;
- **frequent eventual consistency** — not read-your-write;
- **its own error taxonomy** — transport/HTTP/provider errors, categorically
  distinct from a DB constraint violation.

Every built-in Spock will grow that has this shape belongs to the same plane and
deserves the same treatment:

| Surface | Production reality it hides in a naive prototype |
| --- | --- |
| **Storage** (bytes) | S3/GCS; ~100ms `GET`, 503s, presigned-URL expiry, upload two-phase |
| **Email** | SES/Postmark; async send, delayed delivery, bounces, webhook confirmation |
| **Payment** | Stripe; `pending → settled`, declines, seconds of latency, webhook is the truth |
| **Search** | external index; write-then-index lag — the classic eventual-consistency gap |
| **Jobs / queues** | deferred completion, at-least-once, retries, dead-letter |
| **AI / LLM** | seconds of latency, rate limits, partial/streamed results, non-determinism |
| **SMS · push · geocode · outbound webhooks** | all out-of-process, all fallible |

These are precisely the surfaces where the naive prototype (*instant, infallible*)
and production **diverge most sharply** — and they share a shape, so they should
share a treatment. Solving latency/failure once, universally, is worth far more
than solving it storage-by-storage.

This generalizes RFD 0001's `extern` — the effect marker for *"this leaves the
process."* The external plane *is* the set of `extern` surfaces; latency and
failure are the properties `extern` was always implying but never simulated.

## 3. The mechanism: one universal fault/latency simulator

The model is **Chrome DevTools' Network throttling** — *Fast 3G · Slow 3G ·
Offline*: a single, global, orthogonal knob that degrades all network I/O so a
developer can *see and build for* the degraded case on demand, then switch it off.

Design properties to preserve (these are the grounding; the follow-up RFD picks
the concrete surface):

1. **Universal / orthogonal, not a storage feature.** One control, applied across
   the whole external plane. It lives at the boundary the runtime *already owns* —
   the protocol dialects (`/storage/v1`, and future `/email`, `/pay`, …) — never
   inside author-written `fn` bodies. The author's contract keeps describing the
   *ideal*; the runtime degrades the *delivery*. This is the crux of "promote it
   to universal": it is deliberately **not** bolted onto storage.
2. **Off by default, opt-in.** The happy path stays instant for fast iteration.
   You switch on "Slow 3G / 10% failure" only when hardening the client — exactly
   why DevTools throttling doesn't slow ordinary development.
3. **Honest wire semantics.** A simulated failure surfaces as the *real* response
   production would emit — a `503` with `Retry-After`, a slow TTFB, a connection
   timeout — **not** a Spock-only sentinel. The client code you write against the
   simulator must be the client code production needs, or the simulation is
   theater.
4. **Composable dimensions.** Latency (fixed · a distribution with jitter · a p99
   tail), failure probability, failure *kind* (timeout vs `503` vs `429` vs
   partial), and total offline. Named presets over raw knobs for the common cases.
5. **Scoped.** A global default plus per-service overrides (storage slow, email
   flaky), because real systems degrade unevenly.
6. **Reproducible on demand.** A seedable mode so a demo or test can pin *"the 3rd
   upload fails, the 4th is slow,"* distinct from a free-running chaos mode.

## 4. The design space (options for the follow-up, not decisions here)

The separate design must settle these forks; naming them is this RFD's job.

1. **Where control lives.** (a) a **studio** panel — the DevTools-shaped home,
   given studio is already "the human-developer console" (RFD 0015); (b) a CLI
   flag (`spock run --throttle slow-3g --fault 0.1`); (c) a per-request control
   header (`X-Spock-Chaos: …`) so an automated test can drive one call; (d) a
   contract-level declaration. Likely several coexist — but which is the source of
   truth, and do they compose?
2. **Dev-tool only, or does the contract carry expectations?** The deeper fork.
   Should the generated client types/SDL *mark* an external field/`fn` as
   *"async, may fail, handle pending"* — so the **type** nudges the client toward
   correct handling regardless of whether chaos is switched on? That makes
   honesty a property of the contract, not just of a runtime knob. It is also a
   much bigger commitment.
3. **Latency model fidelity.** Fixed delay vs. a sampled distribution (p50/p99 +
   jitter) vs. named presets. How much fidelity earns its keep before it is
   itself theater?
4. **Failure taxonomy.** The canonical set of simulable faults per service class,
   each mapped to the real wire response, so the set is honest and finite.
5. **Determinism.** Seedable/reproducible vs. random; how a test pins a specific
   failure without flaking.
6. **Granularity.** Global · per-service · per-operation.
7. **Interaction with two-phase flows and the sweep.** Injecting latency between
   `mint` and `commit`, or failing a `commit`, stresses exactly the orphan and
   eventual-consistency machinery storage v0 already has (RFD 0018 §1.6, §6). The
   simulator is therefore also a **test instrument** for the runtime's own
   reconciliation, not only a client-teaching tool.

## 5. What this deliberately does NOT do

- **No implementation, no keyword, no protocol change** lands from this RFD. It
  is grounding and framing only.
- **It does not special-case storage** (see §6). The whole thesis is that the
  feature is universal; a storage-only latency hack would betray it.
- **It does not change storage v0's semantics.** v0 stays instant and infallible;
  the simulator is additive and off by default.
- **It is not infrastructure chaos-engineering.** This is a *dev-time,
  client-teaching* instrument, not fault injection at a real deployment.
- **It does not pick** where control lives (§4.1) or whether the contract carries
  async/failure expectations (§4.2). Those are the follow-up's to settle.

## 6. The temptation to special-case storage — and why we resist it

The cheapest possible down-payment is real and worth naming: storage v0's serve
path could grow an off-by-default fixed delay (a `--storage-delay-ms` flag or a
studio toggle) in an afternoon, so the studio's file UX is at least exercised
against non-instant serving. It is tempting precisely because storage is the
surface in front of us today.

We **resist** it as the path, for the reason this whole RFD exists: the moment it
is a *storage* delay, the next person adds an *email* delay and a *payment* delay,
and the orthogonal DevTools-style control — the thing actually worth building —
never gets built. The doctrine (§1) and the reframe (§2) both say the same thing:
design the universal instrument, and let storage be its first *client*, not its
owner. If a stopgap is ever taken, it must be explicitly labeled a stopgap for
the universal design, not a storage feature.

## 7. Open questions

- **The contract-honesty fork (§4.2)** — the largest. Runtime knob only, or does
  externality/asynchrony become visible in the generated types? This decides how
  much of the doctrine is enforced by the *type* vs. discovered by *turning chaos
  on*.
- **Naming.** *external · extern · effects · chaos · throttle* — and whether this
  eventually subsumes `once`/`extern` (RFD 0001) into one coherent
  effects-and-externality story.
- **Real-backend interaction.** When storage gains a real S3 backend (RFD 0018's
  *buy* track), latency becomes *real*. Does the simulator then shift from
  *injecting* fake latency to *amplifying/normalizing* real latency, so dev
  against a fast local store still feels production's tail?
- **Ordering.** This sits behind the filter RFD (still the plan of record,
  RFD 0009) like storage did; it should be picked up when a *second* external
  surface (email is the likely candidate) makes the universality concrete rather
  than hypothetical — building the general instrument for a single client is the
  premature-abstraction risk to watch.

---

Related: RFD 0018 (storage — the first external surface; §7 there forward-points
here), RFD 0017 (storage research — the two-phase/eventual-consistency edges this
generalizes), RFD 0001 (effects · `once` · `extern` — the effect vocabulary this
extends), RFD 0015 (studio — the DevTools-shaped home for the control), RFD 0009
(roadmap — where this queues).

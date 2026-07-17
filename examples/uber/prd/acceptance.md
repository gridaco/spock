# End-to-End Acceptance and Metrics

This document defines when the Uber-like product baseline has been satisfied.
It joins requirements across planes; passing one application screen or one
service test is insufficient.

The scenarios are technology-independent. Each implementation must provide
repeatable fixtures, commands, observable outcomes, and durable evidence for
the same product claims.

## Scenario Contract

Every automated or reviewed scenario MUST record:

- market and policy versions;
- passenger, driver, vehicle, operator, and provider preconditions;
- command identities and authoritative revisions;
- simulated times and dependency conditions;
- expected passenger, driver, operator, and financial outcomes;
- durable facts and audit evidence inspected;
- prohibited outcomes proved absent; and
- cleanup or preserved evidence.

A green client screenshot is not sufficient evidence of a passed scenario.

## Core Journey Scenarios

### ACC-001 — Completed private ride

Given an eligible passenger, payment method, driver, vehicle, supported route,
and normal providers, the passenger accepts one valid quote, one driver becomes
authoritatively assigned, the parties verify pickup, the driver starts and
completes the trip, the passenger receives a reconciled receipt, and the driver
receives an explainable earning.

The scenario MUST prove that quote, assignment, trip, fare, collectible
obligation, external collection, earning, and payout projections have distinct
identities and compatible outcomes.

### ACC-002 — No available supply

A valid request with no eligible candidate MUST reach a bounded unfulfilled
outcome. It MUST NOT create a driver assignment, final collection, completed
trip, or fabricated pickup ETA. Any temporary funds reservation follows the
disclosed release policy.

### ACC-003 — Duplicate submission and ambiguous payment readiness

The passenger submits once, loses the response, and retries while payment
readiness or a funds-reservation operation is delayed. One request and at most
one initial provider operation may exist. The client recovers the original
outcome; a provider's late success is reconciled rather than repeated.

### ACC-004 — Exclusive-offer race

An exclusive offer expires or is revoked while the driver response is in
flight. The platform either accepted it before the authoritative boundary or
returns a durable losing outcome. A late tap cannot create an assignment.

### ACC-005 — Multi-driver interest

Two eligible drivers express interest in the same multi-driver offer. Exactly
one may become assigned. The other receives `not matched` or equivalent,
remains eligible for work, and is not counted as canceling a trip.

### ACC-006 — Passenger cancellation versus assignment

Passenger cancellation races with a valid driver response. The platform
records one accepted ordering, one request/assignment outcome, and one
evidence-based fee evaluation. Neither party's client timestamp decides the
winner.

### ACC-007 — Arrival and no-show dispute

The driver declares arrival with imperfect GPS, completes the configured wait
and contact steps, and claims no-show; the passenger disputes the pickup
location. Arrival claim, location evidence, timer, contact attempts, termination,
fee, driver compensation, and review decision remain separately inspectable.

No financial consequence may be justified by proximity alone.

### ACC-008 — Driver disconnect and replay

The assigned driver disconnects after arrival or trip start, continues the
physical duty safely, reconnects, obtains authoritative state, and replays
commands. Duplicate arrival, start, or completion has one business effect and
cannot regress the trip.

### ACC-009 — Ride-verification failure

The passenger or driver reports a person or vehicle mismatch, or a PIN is
incorrect or replayed. The trip cannot start through the failed verification
path; both parties receive a safe support or cancellation route. A valid PIN
for one assignment cannot authorize another.

## Provider and Financial Scenarios

### ACC-010 — Route-provider degradation

Candidate route-matrix results arrive partially, late, and out of order, then
the maps provider becomes unavailable during an assigned trip. Failed
candidates do not become zero-distance candidates, shown ETAs retain
provenance and freshness, and the physical trip remains recoverable and
completable.

### ACC-011 — Collection timeout and late events

A funds-reservation or collection operation times out and later succeeds while
duplicate, out-of-order provider events arrive. The internal operation remains
unknown until reconciled, creates no second provider mutation, and converges to
one collection and one set of ledger effects.

### ACC-012 — Completion while providers are unavailable

The driver physically completes a trip while maps, push, or payment is
impaired. Completion releases the driver exactly once, both clients recover the
trip outcome, the fare obligation is retained, and deferred provider work is
reconciled after recovery.

### ACC-013 — Refund after earning and payout

A reviewed passenger refund occurs after driver earning became available or
was paid out. The original fare, collection, earning, any provider-side
transfer or credit, and payout remain unchanged. Compensating passenger,
driver, or platform entries follow the effective policy and remain explainable
to affected parties.

### ACC-014 — Failed driver payout

A payout request is accepted but the provider or receiving institution later
reports failure. The driver balance and statement distinguish requested,
submitted, failed, returned, and available funds. The product does not claim
bank receipt without evidence.

## Safety, Operations, and Rights Scenarios

### ACC-015 — Safety allegation and safeguard

A passenger or driver submits a serious allegation during or after a trip.
The original report is retained, triage applies any authorized temporary
safeguard, an investigator records evidence and classification, a separate
decision records its basis, and an eligible appeal preserves both decisions.

The report, anomaly signal, allegation, finding, account restriction, refund,
and regulator or emergency disclosure remain distinct.

### ACC-016 — Operator correction after money movement

An authorized operator corrects an erroneous trip completion after collection
and earning work began. The original actor command remains visible; the
correction records case, reason, evidence, grant, policy, before/after state,
approvals, and compensating downstream effects.

An ordinary support role cannot perform the same action.

### ACC-017 — Privacy deletion with preservation hold

A person requests access, correction, and deletion while one trip is under a
scoped legal or safety hold. The response reports field by field what was
disclosed, corrected, deleted, de-identified, retained, or held and why. The
hold protects only its declared subjects, fields, and time range and has a
review or expiry.

### ACC-018 — Privileged access boundary

Role-based tests attempt ordinary support, safety, finance, privacy, compliance,
and IAM actions against raw GPS, identity documents, payment data, unrelated
case evidence, and privilege grants. Each role sees only its required fields.
Break-glass use requires a case and creates an independent review alert.

### ACC-019 — Accessibility need and complaint

A passenger requests an allowed service with operational accessibility
instructions or a service animal. As a deliberate rule for the initial standard
service, the request is not surcharged or penalized for that need. Only
necessary instructions reach the driver after assignment, while any
pre-assignment capability constraint reveals no diagnosis or unnecessary
detail. A denial can be reported as a first-class accessibility case linked to
assignment, cancellation, fare, refund, and investigation.

The passenger can complete the critical flow without relying solely on a map,
color, motion, fine pointer target, or inaccessible countdown.

Where a market enables or requires wheelchair-accessible service, its profile
MUST separately test supply, equivalence, dispatch, wait, and surcharge rules.

### ACC-020 — Eligibility changes during work

A driver or vehicle credential expires or is revoked while offline, available,
assigned, and in trip in separate runs. New availability and assignments stop
according to effective policy; the active-trip safeguard follows an explicit
reviewed rule; historical eligibility is not rewritten.

### ACC-021 — Regulator report and correction

An authorized operator produces a schema-versioned report from an immutable
cutoff, assigns a stable submission identity, retries only under the configured
regulator mechanism, and reconciles any acknowledgement or ambiguous result.
After discovering an error, the operator submits a linked correction through
that mechanism. The original submission and correction reason remain
reproducible.

### ACC-022 — Restore and reconcile

The platform restores authoritative data from backup while provider callbacks
and queued work span the recovery point. Before unrestricted service resumes,
it runs dependency-appropriate recovery for external operations and proves
assignment, trip, ledger, eligibility, policy-version, and audit invariants.

### ACC-023 — Early trip termination

Passenger and driver requests to end an active trip race with ordinary
completion during a safety concern. The platform records the accepted ordering,
physical stop evidence, exceptional trip outcome, participant and vehicle
release, safety case, partial fare and earning, collection, receipt, and
notifications without rewriting the trip as pre-start cancellation.

### ACC-024 — Driver never reconnects

The driver client disappears permanently after physical travel. After the
market's reviewed timeout and evidence process, an authorized exceptional
resolution records what is known and unknown, releases driver and vehicle,
updates the passenger, and creates reviewable fare, earning, payment, and case
work. It MUST NOT forge a driver-authored completion command.

### ACC-025 — Collision and insurance handoff

A collision is reported while the driver is available, approaching, and in
trip in separate runs. Each run preserves the applicable operating phase and
coverage evidence, protects the active participants, restricts unsafe new work,
opens a claims handoff, and keeps the allegation, trip outcome, insurer
response, and platform eligibility decision distinct.

### ACC-026 — Market-policy activation

Two operators attempt overlapping policy activation while requests and trips
span the effective boundary. Exactly one profile resolves for every scope and
business time; author and approver are different eligible people; active work
follows the declared transition rule; and rollback creates a superseding
version rather than editing history.

### ACC-027 — Payment and payout instrument takeover

A passenger payment method and driver payout account are added, verified,
selected, changed, and removed while a financial provider operation is in
flight. The original operation retains its instrument reference, unauthorized
change is blocked or contained, and neither an account takeover nor a late
provider event redirects an existing obligation silently.

### ACC-028 — Two-sided rating and moderation

Passenger and driver submit ratings after one eligible trip. Revision,
withdrawal, moderation, aggregation, and any later offer disclosure follow the
recorded policy. Original submissions remain attributable, and a rating does
not silently become a safety, accessibility, or eligibility finding.

## Cross-Plane Invariant Probes

Every release MUST continuously test:

| Invariant | Required proof |
| --- | --- |
| One active assignment | Competing exclusive and multi-driver responses, rematch, operator action, and cancellation cannot create two winners |
| One active trip per participant in scope | A passenger, driver, or vehicle cannot enter conflicting active trips |
| Idempotent business effects | Repeated client commands, jobs, and callbacks do not duplicate requests, transitions, collections, refunds, earnings, payouts, cases, or reports |
| Historical policy | Replaying evaluation after a policy change preserves the original result and can separately show the new result |
| Location is evidence | Stale, inaccurate, missing, or contradictory samples cannot alone prove arrival, start, completion, no-show, or fault |
| Independent money lifecycles | Fare, collectible obligation, collection, earning, any provider-side transfer, payout, refund, and dispute can advance, fail, and reconcile independently |
| Append-only correction | Original commands, reports, provider receipts, decisions, and financial entries survive every correction |
| Disclosure by phase and role | Passenger, driver, operator, trusted contact, and provider cannot retrieve data outside their current purpose |
| Recoverable projections | Client, realtime, notification, analytics, and operator projections converge from authoritative facts |

## Launch Gates

### Product and traceability

- Every `PAX`, `DRV`, `MKT`, `OPS`, `FIN`, and `PLT` requirement MUST map to at
  least one automated test, reviewed operational exercise, or named pre-launch
  decision.
- Every accepted exclusion and unresolved decision MUST have an owner and
  launch consequence.
- Passenger, driver, operator, and system behavior MUST be reviewed together
  for each end-to-end lifecycle.

### Market policy

The launch market MUST define, approve, and version, at minimum:

- geography, currency, service class, and operating rules;
- passenger, driver, vehicle, and payment eligibility;
- quote terms, driver-offer disclosure, expiry, and adjustment rules;
- dispatch mode and candidate policy;
- arrival, wait, no-show, cancellation, rematch, and fee evidence;
- trip change, early termination, and emergency behavior;
- insurance phase and evidence;
- accessibility and nondiscrimination handling;
- two-sided rating, revision, moderation, and aggregation;
- retention, deletion, recording, consent, and legal-hold rules;
- safety restriction, decision, review, and appeal rules; and
- tax, regulator reporting, and correction obligations.

No market-dependent number may hide as an unversioned service constant.

### Correctness and recovery

- The scenario suite and cross-plane invariant probes MUST pass under duplicate,
  delayed, reordered, lost, and retried delivery.
- Financial reconciliation and dependency-appropriate recovery MUST have no
  unexplained launch-blocking mismatch.
- Backup restoration, queue replay, callback replay, and client reconnect MUST
  be exercised against production-like scale before launch.
- Every critical dependency MUST have a tested degraded mode and recovery
  runbook.

### Security, privacy, and accessibility

- Threat modeling MUST cover passenger, driver, operator, provider, and
  account-takeover paths.
- Role, row/subject, field, purpose, and phase authorization tests MUST pass,
  including denial cases and break-glass review.
- Payment scope, authentication assurance, data inventory, retention, deletion,
  disclosure, consent, and incident response MUST receive specialist review.
- Passenger, driver, and operator critical paths MUST meet the declared WCAG
  2.2 AA target or an equivalent native-platform standard, with assistive
  technology and non-map alternatives tested.
- Accessibility outcomes MUST be evaluated for denial, surcharge, cancellation,
  wait, completion, rating, and support bias.

### Operational and domain review

Production launch requires named approval from qualified owners for:

- mobility marketplace and driver operations;
- passenger and driver support;
- safety, emergency, and incident response;
- accessibility and nondiscrimination;
- payments, accounting, tax, and driver payout;
- privacy, consumer rights, and data governance;
- market-specific legal, regulatory, insurance, and labor questions; and
- security, reliability, capacity, disaster recovery, and provider management.

Public documentation cannot substitute for these reviews
([evidence gaps](sources.md#evidence-gaps-requiring-domain-review)).

## Metrics Contract

### ACC-030 — Metric definition

Every decision metric MUST declare:

- numerator and denominator;
- inclusion, exclusion, and deduplication rules;
- event and authoritative time used;
- market, service class, policy, experiment, and application-version cohort;
- status basis at the reporting cutoff;
- data freshness and known loss;
- privacy and minimum-cohort controls; and
- whether it measures an attempt, report, decision, person, request, assignment,
  trip, or financial operation.

A dashboard label without this definition is not a product metric.

### ACC-031 — Demand and fulfillment

The product MUST define request conversion, payment-readiness failure,
unfulfilled demand, time to assignment, rematch, passenger cancellation,
driver cancellation, no-show, pickup, start, completion, and early termination
using stable lifecycle boundaries.

Rates MUST name their denominator. Cancellation per request and cancellation
per assignment are different measures.

### ACC-032 — Supply and dispatch

The product MUST measure eligible availability, offer delivery, exclusive
response, multi-driver interest, assignment win/loss, offer expiry, assignment
conflict prevention, approach time, and utilization by market and policy
version.

These metrics MUST NOT silently turn interest loss into rejection or
cancellation.

### ACC-033 — ETA and location quality

Pickup and destination ETA quality MUST compare a timestamped estimate with a
defined observed outcome and report estimate age, missingness, provider
fallback, and error distribution. Location freshness and uncertainty MUST be
reported separately from availability of a coordinate.

### ACC-034 — Money and reconciliation

The product MUST measure payment-readiness outcomes, authorization and capture
unknowns where those stages apply, collection success, quote-to-fare deltas,
refund and dispute status, driver earning availability, payout outcomes,
provider-event backlog, reconciliation exceptions, and exception age.

Gross passenger amount, driver earning, platform amount, taxes, tolls,
tip, and any future enabled promotion MUST reconcile without assuming a fixed
commission.

### ACC-035 — Safety and support

Safety and support metrics MUST distinguish:

- reports received;
- machine-generated signals;
- classified allegations;
- affected requests, trips, and people;
- temporary safeguards;
- substantiated, unsubstantiated, and inconclusive factual dispositions;
- precautionary and final account actions, measured separately from factual
  disposition;
- appeal outcomes; and
- response, investigation, and resolution times.

No public or internal query may silently label every report as a confirmed
incident.

### ACC-036 — Accessibility and fairness

The product MUST compare service availability, quote, assignment, wait,
cancellation, no-show, fare, completion, rating, restriction, appeal, and
support outcomes across reviewed accessibility and fairness cohorts.

Collection and use of sensitive attributes require a defined legal basis,
purpose, access boundary, retention, and statistical disclosure protection.

### ACC-037 — Reliability and operator control

The product MUST measure stale projections, reconnect recovery, notification
loss where observable, external-operation unknowns, callback lag, queue
backlog, degraded-provider time, privileged denials, break-glass use, audit
coverage, and restoration exercises.

Metric success cannot excuse a violated invariant. A low duplicate rate is not
permission to create any duplicate collection or assignment.

## Meaning of Complete

This PRD is complete as a product-requirements baseline when:

1. all detailed documents exist and use stable requirement identities;
2. the source ledger states the evidence and its limits;
3. market-specific choices are explicit configuration or launch decisions;
4. each requirement has a verification route;
5. evidence gaps are assigned to qualified review; and
6. a proposed implementation can be judged without weakening the product.

It is not, by itself, proof that a market is legally approved, operationally
staffed, safe at production scale, or ready to launch.

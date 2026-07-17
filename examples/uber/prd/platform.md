# Platform and Reliability Requirements

This document specifies the service-wide contracts that let the passenger,
driver, marketplace, operations, and money planes remain coherent across
clients and independent providers. It defines product behavior under failure;
it does not choose a deployment architecture or vendor.

Grounding: [payment idempotency](sources.md#s-prov-04),
[webhook delivery](sources.md#s-prov-05),
[push delivery](sources.md#s-prov-07),
[route providers](sources.md#s-prov-01),
[rider privacy](sources.md#s-prod-12),
[driver privacy](sources.md#s-prod-13),
[authentication assurance](sources.md#s-std-02), and
[payment-card protection](sources.md#s-std-03).

## Identity, Authority, and Commands

### PLT-001 — Actor and authority

Every consequential command MUST identify:

- the authenticated person, service, or operator;
- the role and authority grant being exercised;
- the passenger, driver, operator, or provider subject;
- the market and business object;
- the client or service instance;
- the purpose or case when privileged access is involved; and
- the authentication and authorization decision used.

Account identity, role, eligibility, ownership, and authority MUST remain
distinct. Authentication proves control of an account session; it does not by
itself authorize a trip or operator action.

### PLT-002 — Command envelope

Every retriable business command MUST carry a stable idempotency identity,
actor, operation type, target, expected revision where applicable, and client
request time. The platform MUST add received time and authoritative decision
time.

Reusing an idempotency identity with different material parameters MUST be
rejected. A repeated identical command MUST return the prior business outcome,
including a prior durable rejection.

### PLT-003 — Revision and ordering

Mutable business aggregates MUST expose an authoritative revision or equivalent
ordering token. A stale command MUST fail with the current state or a recovery
pointer; it MUST NOT move an object backward.

Client clocks determine neither offer expiry nor transition order. When two
valid commands race, the platform MUST preserve the accepted order and the
durable reason the losing command had no effect.

### PLT-004 — Role-specific projections

Passenger, driver, operator, and provider-facing views are projections over
shared business facts. They MAY differ in disclosure and freshness but MUST
converge on the same authoritative outcome.

No client cache, notification, analytics warehouse, search index, or operator
dashboard may become a second writer of trip or financial truth.

## External Provider Operations

### PLT-010 — Explicit authority boundary

Each external integration MUST declare:

- facts the platform owns;
- facts the provider owns;
- identifiers on both sides;
- request and callback versions;
- timeout and retry behavior;
- data disclosed and permitted purpose;
- retention and deletion obligations;
- degradation behavior; and
- dependency-appropriate recovery, recomputation, discard, or reconciliation
  procedure.

A provider response is evidence within that boundary, not universal authority.

### PLT-011 — Durable operation record

Before attempting a consequential external mutation, the platform MUST retain
the internal intent, business operation identity, provider, stable provider
idempotency key when supported, parameters or protected parameter digest, and
expected result.

Every attempt MUST retain request time, response time, provider reference,
outcome, error category, retry decision, and reconciliation state. Retrying the
same business operation MUST NOT mint a second business intent.

### PLT-012 — Transactional publication

A committed internal transition that requires asynchronous work MUST publish
that work durably with the transition. A crash between the business commit and
worker execution MUST leave recoverable pending work rather than a silently
lost notification, collection, payout, report, or projection update.

This requirement does not assume one specific queue or outbox
implementation.

### PLT-013 — Provider event inbox

Provider callbacks MUST be authenticated according to the provider contract,
retained before processing, deduplicated by provider event identity or a
documented equivalent, and safe to replay.

Processing MUST tolerate duplicate, delayed, and out-of-order events. A newer
provider state MUST NOT be regressed because an older callback arrived later.

Basis: representative payment providers explicitly make no ordering guarantee
and retry delivery ([S-PROV-05](sources.md#s-prov-05)).

### PLT-014 — Unknown outcomes

A timeout, connection reset, worker crash, or missing callback MUST produce an
`outcome unknown` or equivalent state when the provider might have accepted the
operation. The platform MUST inspect the existing provider object or reconcile
before creating a replacement operation.

Unknown is not a synonym for failed.

### PLT-015 — Provider receipts and versions

Provider object identifiers, request versions, material response facts, and
legally retainable receipts MUST remain traceable to the internal operation.
Schema or API upgrades MUST preserve the ability to interpret historical
events.

Sensitive provider payloads MUST not be retained wholesale when a minimal
verified result is sufficient.

## Realtime, Notification, and Offline Clients

### PLT-020 — Snapshot and resume

After initial load or reconnect, a client MUST obtain an authoritative snapshot
and a revision or resume position before applying later changes. Missing,
duplicated, or reordered realtime messages MUST trigger replay or a fresh
snapshot.

Realtime transport delivery MAY reduce latency; it MUST NOT define whether a
business transition occurred.

### PLT-021 — Push is a hint

A push provider's acceptance MUST NOT be displayed as device delivery.
Messages MAY expire, collapse, arrive late, or never arrive
([S-PROV-07](sources.md#s-prov-07)).

Every push MUST point the client toward recoverable current state. A delayed
offer push cannot extend the offer, and a lost assignment push cannot undo the
assignment.

### PLT-022 — Offline command policy

The platform MUST define which client actions may be queued offline. Financial
consent, offer acceptance, cancellation, arrival, trip start, completion, and
operator intervention MUST NOT be shown as accepted until the platform has
accepted them.

On reconnect, the client MUST reconcile authoritative state before replaying a
queued command. Commands no longer valid for the current revision receive a
durable rejection.

### PLT-023 — Communication delivery

Masked calls, messages, and safety contact attempts MUST retain purpose,
participants, channel, attempt time, provider outcome, and expiry of contact
authority. Provider acceptance, device delivery, and human response are
different facts.

Message content and recordings MUST follow a separately reviewed retention and
consent policy.

## Location, Routes, and Time

### PLT-030 — Location intake

Each accepted location observation MUST identify subject, source, coordinate,
observed time, received time, accuracy or uncertainty, and relevant device or
provider quality. Invalid, impossible, duplicated, future-dated, or excessively
late samples MUST be rejected or explicitly quarantined.

Processing and downsampling MAY derive route progress or current-location
projections, but MUST preserve which observations supported a consequential
decision.

### PLT-031 — Location access and retention

Raw location samples, active-trip projections, passenger-visible sharing,
trusted-contact sharing, support evidence, and long-term trip summaries MUST
have different purpose, audience, precision, and retention rules.

Ending a sharing purpose MUST revoke future disclosure even when the underlying
record must be retained for a legal or safety reason.

### PLT-032 — Route-provider results

Route and matrix requests MUST retain provider, input reference, requested
mode, material traffic assumptions, computation time, response status, and
fallback indication. Candidate-level failures and reordered results MUST not be
converted to zero distance or zero duration.

Provider geometry and duration are estimates subject to contract and
attribution rules, not proof of physical travel
([S-PROV-01](sources.md#s-prov-01),
[S-PROV-02](sources.md#s-prov-02)).

### PLT-033 — Business time

The platform MUST distinguish occurrence, observation, receipt, processing,
decision, effective, and provider-settlement times. Stored instants use an
unambiguous global representation; passenger, driver, market, and regulator
views apply the recorded time-zone rules.

Policy evaluation uses the version effective for the relevant business time,
not the server's current policy.

## Degradation and Recovery

### PLT-040 — Declared degradation modes

Each critical dependency MUST have an observable normal, impaired, unavailable,
and recovering condition. Entering or leaving a mode MUST be recorded and
visible to affected operators.

The minimum product behavior is:

| Dependency | New work | Existing work |
| --- | --- | --- |
| Maps or route estimates | Stop quoting when required inputs cannot be obtained; do not invent estimates | Keep assignments and trips recoverable; allow safe completion with unavailable or stale routing clearly shown |
| Passenger payments | Stop dispatch when required readiness cannot be established | Do not strand a physical trip; retain the fare obligation and reconcile collection |
| Realtime transport | Permit only flows with authoritative request/response recovery | Preserve transitions and converge clients through snapshot recovery |
| Push or messaging | Do not treat attempted notification as delivery | Preserve underlying state and expose alternate contact or retry where policy allows |
| Identity or screening | Stop new onboarding decisions that require the provider | Preserve last decision only for its declared validity; do not silently extend expired evidence |
| Driver payout | Continue earning accrual under reviewed risk limits | Queue or reconcile payout; never display bank receipt without evidence |
| Configured safety or emergency integration | Keep local safety entry usable and disclose the outage | Provide the reviewed fallback; never claim emergency coordination occurred |

### PLT-041 — Safe trip completion

Loss of a non-safety provider MUST NOT by itself erase an assignment, cancel a
physical trip, or force a driver to keep a physically completed trip open.
Degraded completion MUST retain enough evidence for later fare, support, and
reconciliation work.

### PLT-042 — Recovery and reconciliation

When a provider or internal service recovers, the platform MUST run its
dependency-appropriate recovery procedure. Consequential mutations and durable
external objects require reconciliation; route reads may be recomputed, stale
notifications may be discarded, and clients may resnapshot.

Unrestricted new work MUST remain gated only where unresolved operations could
violate safety, assignment, eligibility, privacy, or financial invariants.
Recovery MUST handle applicable late successes, duplicate callbacks, expired
credentials, stale projections, and partially completed financial actions.

### PLT-043 — Backup and restoration

Authoritative business facts, audit records, policy versions, provider
operation records, and required evidence MUST be covered by a tested backup and
restoration plan. Recovery-point and recovery-time objectives are launch-market
decisions that MUST be approved before production.

A backup is not proven usable until restoration and invariant checks succeed.

## Security, Privacy, and Audit

### PLT-050 — Data classification and purpose

The platform MUST classify at least:

- public or low-sensitivity product data;
- account and contact data;
- precise location and trip history;
- driver and vehicle credentials;
- government identity and screening results;
- payment references and financial records;
- support and safety evidence;
- biometrics or recordings when enabled; and
- authentication secrets and provider credentials.

Collection, use, display, export, retention, and deletion MUST be justified per
class and purpose.

Basis: public rider and driver notices demonstrate that these data categories,
subjects, uses, and retention concerns differ
([S-PROD-12](sources.md#s-prod-12),
[S-PROD-13](sources.md#s-prod-13)).

### PLT-051 — Privileged authentication

Operators with access to identity, raw location, financial, safety, privacy, or
administrative functions MUST use stronger authentication and session controls
than an ordinary passenger session. High-impact actions MUST support
re-authentication, least privilege, short-lived grants, and separation of
duties appropriate to risk.

Basis: [NIST authentication guidance](sources.md#s-std-02).

### PLT-052 — Secret and payment-data boundary

Raw authentication secrets, provider credentials, and prohibited payment-card
authentication data MUST NOT enter ordinary logs, analytics, support views, or
domain records. Tokenization or provider references reduce exposure but do not
by themselves determine compliance scope.

Basis: [PCI DSS](sources.md#s-std-03).

### PLT-053 — Audit

Every privileged read, export, correction, policy change, grant change,
external disclosure, and consequential business mutation MUST retain:

- actor and authenticated role;
- authority grant, case, and purpose;
- subject and market;
- action and before/after references;
- policy version and evidence references;
- occurrence, receipt, and decision times;
- result and failure;
- approval or break-glass use; and
- correlation to external operations.

Audit correction is additive. Ordinary product operators MUST NOT be able to
delete or silently edit audit history.

### PLT-054 — Retention, deletion, and holds

Retention MUST be field- or record-class specific and resolve the ordinary
product lifetime, regulatory minimums, active disputes or claims, safety needs,
and legal holds. A deletion request MUST report deleted, de-identified,
retained, and held fields with the applicable reason.

A hold MUST have authority, scope, creator, start, review or expiry, and
release. A vague permanent safety flag is not a valid retention policy.

### PLT-055 — Export and disclosure

Before a privacy, regulator, insurer, or legal export is released, the platform
MUST produce a reviewable manifest of subjects, trips, fields, date range,
purpose, authority, redactions, and exclusions. Release and recipient receipt
MUST be audited separately.

## Observability and Service Objectives

### PLT-060 — End-to-end correlation

An authorized investigator MUST be able to trace one trip across quote,
request, offers, assignment, location evidence, trip commands, cases, financial
entries, provider operations, notifications, and operator actions without
using personal data as the primary correlation key.

Logs and traces are diagnostic evidence, not replacements for durable business
facts.

### PLT-061 — Product service indicators

The platform MUST measure at least:

- command acceptance, rejection, unknown outcome, and latency;
- stale client and idempotent replay rates;
- dispatch and assignment convergence;
- location and ETA freshness;
- provider attempt, callback, and reconciliation backlog;
- realtime disconnect and snapshot-recovery success;
- financial mismatch and unresolved exception age;
- privileged access and break-glass review; and
- backup restoration and invariant-check results.

Numeric service objectives require expected volume, market risk, support
capacity, and provider contracts. They MUST be approved before launch rather
than invented in this generic harness.

### PLT-062 — Alert ownership

Every launch-critical indicator MUST have a threshold, severity, owner,
runbook, escalation path, and customer-impact definition. An alert is not
resolved merely because a retry succeeded; any duplicated or contradictory
business effect must still be investigated.

## Platform Acceptance Scenarios

The release suite MUST prove:

1. retrying the same command after a timeout produces one business effect;
2. reusing an idempotency key with changed parameters is rejected;
3. a stale client cannot regress request, assignment, trip, or money state;
4. commit followed by worker failure leaves recoverable asynchronous work;
5. duplicate and reordered provider events converge without duplicate effects;
6. a consequential provider mutation timeout remains unknown until retrieval
   or reconciliation establishes the outcome;
7. reconnect begins from an authoritative snapshot and then resumes changes;
8. a lost or delayed push does not lose or reverse a transition;
9. bad, stale, or future-dated coordinates cannot become silent current truth;
10. route-matrix partial failure does not fabricate zero travel;
11. maps and payment degradation preserve an already active physical trip;
12. an expired screening result is not silently extended during provider
    outage;
13. role tests prevent ordinary support from viewing raw identity, payment,
    location, or safety evidence;
14. break-glass access creates a justified, reviewable alert;
15. deletion and a scoped hold produce correct field-level outcomes;
16. a restored backup passes cross-plane invariants before traffic resumes; and
17. one trip can be traced across internal and external operations without
    treating logs as the ledger.

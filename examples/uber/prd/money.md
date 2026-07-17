# Pricing, Payments, Earnings, and Payout Requirements

This document specifies the commercial and financial product for the initial
scope: one immediate private ride, one fare-payment lifecycle, one driver
earning, an optional post-trip tip, one market, and one trip currency. A fare
or tip may require separate provider operations without becoming separate
rides.

It distinguishes what the platform owes or is owed from what an external
payment or payout provider has attempted, processed, settled, or delivered.

Grounding: [driver earnings](sources.md#s-prod-05),
[cancellation review](sources.md#s-prod-06),
[payment-card protection](sources.md#s-std-03),
[marketplace money movement](sources.md#s-prov-03),
[provider idempotency](sources.md#s-prov-04),
[webhook behavior](sources.md#s-prov-05), and
[marketplace disputes and payouts](sources.md#s-prov-06).

## Financial Authority and Representation

### FIN-001 — Separate passenger and driver agreements

The product MUST represent the passenger quote and the driver earning offer as
separate commercial agreements linked to the same trip.

Passenger fare, driver earning, and platform amount MUST NOT be assumed to be
equal or related by a fixed commission unless the effective market policy
explicitly establishes that relationship.

Basis: reference products separately disclose passenger terms and driver
earnings ([S-PROD-05](sources.md#s-prod-05)).

### FIN-002 — Exact monetary values

Every monetary value MUST preserve:

- exact amount;
- currency;
- sign and financial direction;
- component or purpose;
- effective market;
- calculation or policy version;
- effective time; and
- recorded time.

Amounts MUST use integer minor units or another lossless decimal
representation. Binary floating-point values MUST NOT be authoritative for
quotes, fares, earnings, ledger entries, refunds, transfers, payouts, or
reconciliation.

### FIN-003 — Currency and conversion

Every quote, fare, earning, ledger entry, payment operation, transfer, refund,
and payout MUST identify its currency explicitly.

The initial product MUST use one currency for each trip. If later products
convert currency, the conversion MUST be a separate attributable operation
with source amount, destination amount, currencies, rate source, rate time,
fees, and rounding result. A converted amount MUST NOT overwrite its source.

### FIN-004 — Rounding and total integrity

Each market policy MUST define:

- calculation precision;
- the boundary at which each component is rounded;
- the rounding mode;
- treatment of fractional minor units; and
- allocation of any remainder.

Passenger-visible and driver-visible components MUST sum exactly to their
respective displayed totals. Any rounding adjustment MUST be explicit; hidden
remainder MUST NOT be assigned silently to the passenger, driver, or platform.

### FIN-005 — Independent lifecycles

The product MUST distinguish at least:

- passenger quote and acceptance;
- final fare;
- passenger payment readiness, funds reservation when supported, collection,
  and settlement;
- refund and dispute;
- driver offer and acceptance;
- driver earning accrual and availability;
- any provider-side transfer and payout; and
- operator review and correction.

Progress in one lifecycle MUST NOT imply progress in another. In particular,
trip completion is not payment settlement, earning accrual is not payout, and
a successful payout request is not proof of bank receipt.

### FIN-006 — Internal and external authority

The platform's immutable financial records MUST be authoritative for product
obligations among passenger, driver, and platform.

An external provider MUST remain authoritative for the state of its own
payment-readiness, funds-reservation, collection, refund, dispute, optional
transfer, and payout objects. Provider state MUST be reconciled into platform
projections without treating a callback as the platform ledger.

Basis: marketplace providers expose payment, transfer, refund, dispute, and
payout as distinct responsibilities
([S-PROV-03](sources.md#s-prov-03),
[S-PROV-06](sources.md#s-prov-06)).

## Passenger Quote and Final Fare

### FIN-010 — Quote snapshot

Before a passenger requests a trip, the product MUST create a time-bounded
quote that preserves:

- quote identity and revision;
- passenger, market, and service class;
- confirmed pickup and destination;
- route and duration assumptions;
- price components;
- total amount and currency;
- taxes, fees, surcharges, toll assumptions, and any future discount or
  promotion explicitly enabled by a later scope;
- cancellation and no-show terms;
- conditions under which the fare may change;
- pricing-policy version;
- creation and expiry time; and
- whether the presented amount is fixed, estimated, or a range.

The accepted quote MUST remain recoverable after later repricing, completion,
refund, dispute, or correction.

### FIN-011 — Quote acceptance and expiry

The passenger MUST accept the exact quote revision shown by the client.
Acceptance of an expired, superseded, differently scoped, or materially changed
quote MUST fail without dispatch or a new provider payment operation.

Repeated acceptance of the same quote with the same product idempotency key
MUST return the same request and financial outcome.

### FIN-012 — Fare finalization

Trip completion, a policy-recognized early termination, or a compensable
cancellation MUST produce one authoritative fare finalization under the policy
effective for that outcome.

Fare finalization MUST preserve:

- accepted quote;
- final fare components and total;
- actual inputs used by policy;
- policy version;
- each difference from the accepted quote;
- reason and evidence for each difference;
- finalizing actor or system;
- effective time; and
- review availability.

A route estimate, GPS observation, driver declaration, or provider response
may be evidence for fare calculation but MUST NOT alone rewrite the accepted
commercial terms.

### FIN-013 — Fare changes and passenger consent

A passenger-initiated destination or material service change MUST produce
revised financial terms when required by market policy. The revised terms and
required passenger consent MUST be recorded before the change becomes
commercially authoritative.

Unexpected traffic, route deviation, waiting, tolls, and other post-quote facts
MAY change the fare only under conditions disclosed in the accepted quote and
allowed by effective policy. Every change MUST appear as an attributable fare
delta.

### FIN-014 — Fare components

The product MUST represent separately, when applicable:

- transportation amount;
- time and distance amount;
- waiting amount;
- cancellation or no-show amount;
- platform or booking fee;
- tax;
- toll;
- surcharge;
- future passenger promotion or credit, when explicitly enabled;
- tip;
- rounding adjustment; and
- operator adjustment.

A component MUST identify whether it is charged to the passenger, owed to the
driver, retained by the platform, remitted to another party, or funded as a
promotion. One displayed line item MUST NOT conceal several parties'
obligations.

### FIN-015 — Future promotion and subsidy representation

The initial product excludes promotions, incentives, referrals, loyalty, and
subsidized pricing. It MUST NOT apply a promotion or incentive through an
undocumented adjustment.

If a later scope explicitly enables a promotion or subsidy, the product MUST
preserve its sponsor, eligibility rule, face value, applied amount, currency,
expiry, and financial recipient.

In that later scope, a passenger discount MUST NOT silently reduce the driver's
accepted earning, and a driver incentive MUST NOT silently increase the
passenger's accepted fare. Any policy that links them MUST do so explicitly and
preserve both original amounts.

### FIN-016 — Toll and tax treatment

Tolls and taxes MUST retain their jurisdiction, basis, assessed amount,
currency, responsible party, collection status, and remittance status where
applicable.

An estimated toll or tax MUST remain distinguishable from an assessed,
collected, refunded, or remitted amount. Legal review MUST establish the
platform's tax and remittance responsibilities before launch.

### FIN-017 — Tip

A tip MUST be a separate passenger-authored financial event linked to the trip
and driver. It MUST NOT mutate the transportation fare or be required for a
rating.

The initial product MUST credit the full tip to the driver's earning except for
an explicitly disclosed tax or legally required withholding. A tip added after
an earlier payout MUST accrue to a later available balance rather than rewrite
that payout.

### FIN-018 — Cancellation and no-show money

A passenger cancellation charge and driver cancellation earning MUST be
calculated separately under effective policy. They MUST NOT be assumed to be
the same amount.

The decision MUST retain assignment, arrival, waiting, contact, location
quality, cancellation actor, reason, policy, and review outcome. Reversing a
cancellation decision MUST create financial adjustments rather than delete the
original outcome.

Basis: cancellation charges depend on phase-specific, market-specific evidence
and remain reviewable ([S-PROD-06](sources.md#s-prod-06)).

### FIN-019 — Early-termination fare and earning

Transportation that ends after trip start but before the accepted destination
MUST be treated as an early or exceptional termination, not as a pre-trip
cancellation and not as an ordinary completed-destination trip.

The product MUST preserve:

- accepted passenger quote and driver earning offer or basis;
- trip start and termination identities and times;
- intended and actual termination locations with available quality;
- available traveled time, distance, route, and location evidence;
- terminating actor, reason, and safety or support context;
- market and fare and earning policy versions;
- separately calculated passenger fare and driver earning;
- any waiver, refund, adjustment, or platform-funded amount; and
- review and correction paths.

Early termination MUST NOT automatically zero the passenger fare or driver
earning, and it MUST NOT automatically award the ordinary completed-trip
amount. When evidence is insufficient for a policy decision, the financial
outcomes MUST remain action required rather than inventing arrival at the
destination or treating transportation as if it never occurred.

## Driver Offer and Earning

### FIN-020 — Driver earning offer

When the market presents an upfront driver earning, the offer MUST preserve:

- offer identity and revision;
- related passenger request;
- driver and vehicle;
- market and service class;
- amount or permitted range and currency;
- earning components or calculation basis;
- pickup, destination, duration, distance, toll, and waiting assumptions
  disclosed by policy;
- conditions under which the earning may change;
- earning-policy version; and
- creation and expiry time.

When the market presents only an earning basis, the product MUST preserve the
exact rates, rules, and version that the driver accepted.

### FIN-021 — Earning accrual

A completed trip, policy-recognized early termination, or compensable
cancellation MUST produce one authoritative driver earning accrual. The
accrual MUST preserve:

- accepted earning offer or basis;
- final earning components and total;
- related trip and fare finalization;
- each difference from the accepted terms;
- earning-policy version;
- availability conditions;
- effective time; and
- review path.

Passenger payment failure MUST NOT silently erase a driver earning already
owed under policy. The platform MUST record whether it bears collection loss,
may recover under another policy, or requires review.

### FIN-022 — Earning availability

The product MUST distinguish driver amounts that are:

- accrued;
- pending;
- available;
- reserved;
- selected for payout;
- paid out;
- failed;
- adjusted;
- recovered; or
- reversed.

Availability MUST be derived from recorded policy and financial events, not
from a mutable cached balance alone.

### FIN-023 — Driver adjustments

A refund, dispute, safety decision, toll correction, future promotion
correction when enabled, or operator action that changes what the driver is
owed MUST create a new, attributable driver adjustment.

The adjustment MUST preserve the original earning, policy authority, reason,
evidence, amount, currency, effective time, driver-visible explanation, and
review path. It MUST NOT rewrite an accepted offer, historical earning,
provider-side transfer or credit when present, statement, or payout.

### FIN-024 — Driver statements

Trip detail and periodic driver statements MUST reconcile:

- accepted and final trip earnings;
- cancellation earnings;
- tips and, when enabled by a later scope, incentives;
- tolls, taxes, fees, and withholding as applicable;
- adjustments and recoveries;
- pending and available balances;
- provider-side transfers or credits when present;
- payout fees;
- successful and failed payouts; and
- reversals.

Statement correction MUST create a new revision while retaining the prior
statement.

## Passenger Collection Lifecycle

### FIN-030 — Payment readiness

Before dispatch, the product MUST establish payment readiness for the quote
under the market's payment policy. Readiness MAY require a stored payment
instrument, provider confirmation, passenger action, risk decision, funds
reservation, or authorization supported by the selected method.

Payment readiness MUST NOT be represented as funds reservation, collection,
settlement, or final payment.

### FIN-031 — Authorization or funds reservation

When the selected payment method requires authorization, a hold, mandate, or
another funds-reservation step before collection, the product MUST preserve:

- internal operation identity;
- provider and provider-object identity;
- payment instrument and instrument version;
- reservation or authorization type;
- amount and currency;
- payment-method display reference;
- related quote and passenger request;
- provider status;
- required passenger action;
- expiry or continuing-validity condition;
- attempt history; and
- last reconciliation time.

Any temporary hold MUST be displayed as temporary and MUST NOT appear as final
collection. Cancellation, expiry, or replacement MUST trigger the
method-appropriate release or cancellation operation when supported and
preserve its outcome.

Payment methods that do not expose a separate authorization or reservation
stage MUST NOT be forced into a fabricated one.

### FIN-032 — Final collection

The product MUST initiate final collection only from an authoritative fare
finalization or another explicit collectible obligation.

The collection operation MUST preserve its exact amount, currency, payment
instrument version, provider capability and method, required passenger action,
and relationship to any prior authorization, reservation, or mandate.

For a method that supports separate authorization and capture, the captured
amount MUST equal the collectible amount covered by that operation. If the
final amount exceeds an existing reservation, the product MUST obtain permitted
additional authorization, create a separately disclosed collection, or send
the case to an approved recovery path.

For a method that collects immediately or settles asynchronously without
capture, the product MUST use that method's actual provider states and MUST NOT
invent a capture transition. No method may be presented as collected beyond
the amount and authority established for its operation.

Trip completion MUST remain recordable when the payment provider is
unavailable. The collection operation MAY remain pending or require
reconciliation without keeping a physically completed trip open.

### FIN-033 — Provider processing and settlement are distinct

The passenger product MUST distinguish the method-neutral states:

- action required;
- provider operation pending;
- processing;
- succeeded;
- failed;
- canceled or expired; and
- outcome unknown.

It MUST additionally expose capability-specific states when they exist,
including authorization pending, authorized, capture pending, captured,
provider settlement pending, and settled or available.

An unsupported capability-specific state MUST remain absent rather than be
fabricated. The platform MUST NOT promise bank posting time or irreversible
settlement from collection success alone.

### FIN-034 — Refund

A full or partial refund MUST reference the original collectible obligation and
preserve:

- refund identity;
- requested and approved amount and currency;
- reason and policy;
- authorizing actor;
- original collection;
- provider operation and status;
- related driver adjustment or platform-loss decision;
- request, effective, and completion times; and
- passenger-visible status.

Refund status MUST distinguish requested, approved internally, submitted,
pending, succeeded, failed, canceled, and alternative remediation. A pending
or failed external refund MUST NOT be displayed as received by the passenger.

The sum of successful and still-actionable refunds MUST NOT exceed the
refundable amount unless an explicit additional credit is recorded as a
separate obligation.

### FIN-035 — Dispute

A payment dispute MUST remain distinct from a platform fare review or refund.
The product MUST preserve provider dispute identity, amount, currency, reason,
evidence deadline, submitted evidence, provider state, provisional financial
effect, final outcome, fees, and related recovery decisions.

Refund and dispute workflows MUST prevent duplicate reimbursement while
retaining both histories. A dispute after driver payout MUST follow the same
explicit loss-allocation and adjustment rules as another post-payout
correction.

### FIN-036 — Payment data boundary

The platform MUST minimize cardholder data and use provider references or
tokenized payment methods where possible. Passenger and operator surfaces MUST
show only an approved display reference.

Retention, logging, support access, and deletion MUST follow the assessed
payment-card compliance boundary
([S-STD-03](sources.md#s-std-03)).

### FIN-037 — Passenger payment instrument identity

Each passenger payment instrument MUST have one durable platform identity and
one or more versioned provider references. The product MUST preserve:

- owning passenger account;
- provider and instrument type;
- masked passenger-visible label;
- supported markets, currencies, and capabilities;
- verification, readiness, restriction, and removal status;
- provider subject and instrument references where available;
- creation, verification, update, disablement, and removal times; and
- provenance and last reconciliation time.

The platform MUST NOT use raw credentials as the ordinary instrument identity.
A material credential or provider-reference change MUST create a new version
or replacement relationship rather than rewrite historical operations.

### FIN-038 — Passenger instrument selection and lifecycle

The passenger MUST be able to add, verify, select, replace, and remove a
payment instrument subject to market and risk policy.

A default instrument is a convenience, not authority for an already accepted
quote or provider operation. Quote acceptance and every financial provider
operation MUST bind to the exact instrument version selected for that
operation.

Removal or disablement MUST prevent new operations but MUST preserve historical
references and MUST NOT silently cancel, redirect, or erase an active
authorization, collection, refund, dispute, or reconciliation. Any supported
cancellation or replacement of active work MUST be a separate attributable
operation with a recoverable outcome.

An instrument MUST NOT become payment-ready until its required provider,
ownership, capability, market, and risk checks have succeeded.

### FIN-039 — Passenger instrument change safeguards

Adding, replacing, selecting, or removing a payment instrument is a sensitive
account action. The product MUST:

- require authentication appropriate to the account and change risk;
- require recent or stepped-up authentication for a high-risk change;
- record actor, session, device or client context, prior and replacement
  instrument versions, risk decision, and result;
- notify the passenger through an existing verified channel when policy
  requires;
- prevent a newly added or changed instrument from bypassing readiness,
  verification, or fraud controls; and
- provide a lock, recovery, and support path for suspected account takeover.

An account-recovery or credential-change event MUST trigger the recorded review
of active sessions, payment instruments, and pending ride or collection work
required by risk policy. A payment-instrument change alone MUST NOT create a
trip, accept a quote, or initiate collection.

## Transfer and Payout Lifecycle

### FIN-040 — Earning, optional provider credit, and payout are distinct

An internal driver earning, any provider-side balance credit or transfer, and a
payout to an external destination MUST be separate records with separate
states.

Creating or succeeding at one MUST NOT imply success of the next.

If the selected payout provider has no intermediate provider-side balance,
credit, or transfer stage, the product MUST NOT invent one. A direct payout
MUST still identify the internal earnings and balance entries it consumes.

### FIN-041 — Payout request

A scheduled or driver-initiated payout MUST preserve:

- payout request identity;
- driver, payout instrument, and payout instrument version;
- included available-balance entries;
- amount and currency;
- fee;
- payout method;
- schedule or initiating actor;
- provider and provider-object identity;
- attempt and status history; and
- expected and actual completion information.

Repeated submission of the same payout request MUST NOT select the same
available earning twice or create multiple external payouts.

### FIN-042 — Payout status and failure

The driver product MUST distinguish requested, submitted, pending, in transit,
provider paid, failed, canceled, and reversed payout outcomes where supported.

A failed payout MUST restore or otherwise account for the affected internal
balance exactly once, identify whether the payout account requires action, and
preserve the failed attempt. Retrying MUST create a related new attempt rather
than rewriting the failure.

### FIN-043 — Refund or dispute after payout

When a passenger refund, dispute, or correction occurs after related driver
funds were transferred or paid out, the product MUST apply an explicit policy
that records one of:

- platform absorbs the loss;
- available driver funds are adjusted;
- a reserve is used;
- future driver earnings are adjusted;
- external recovery is initiated; or
- operator review is required.

The original earning, any provider-side transfer or credit, and any payout MUST
remain unchanged. The new adjustment MUST identify the affected obligation,
amount, currency, policy, reason, evidence, driver-visible effect, and appeal
path.

The product MUST NOT create an unexplained negative driver balance.

### FIN-044 — Driver payout instrument identity and readiness

Each driver payout instrument MUST have a durable platform identity and a
versioned provider or receiving-institution reference. The product MUST
preserve:

- owning driver and legally relevant beneficiary;
- provider, destination type, country, and supported currencies;
- masked driver-visible label;
- onboarding, ownership, identity, capability, verification, restriction, and
  readiness states;
- provider subject and destination references where available;
- creation, verification, update, disablement, and removal times; and
- provenance and last reconciliation time.

Transportation eligibility, earning accrual, payout-account onboarding, payout
instrument readiness, and payout status MUST remain separate.

An instrument MUST NOT become ready for new payout until required ownership,
identity, provider, market, currency, and risk checks have succeeded.

### FIN-045 — Payout instrument selection and active-operation races

The driver MUST be able to add, verify, select, replace, and remove a payout
instrument subject to market, provider, and risk policy.

Every scheduled or driver-initiated payout MUST bind immutably to the payout
instrument version selected when that payout operation was accepted. Changing,
disabling, or removing the default instrument MUST NOT redirect an in-flight
payout to a new destination.

Disablement or removal MUST block new payouts while preserving earnings,
historical payouts, failed attempts, reversals, and reconciliation against the
original destination. A provider-reported failure or return after removal MUST
still reconcile to the original payout.

A scheduled payout racing with an instrument change MUST have one ordered,
recoverable result: either the payout binds to the previously ready version
before the change, or it waits for a ready replacement. It MUST NOT select an
instrument from an ambiguous intermediate state.

### FIN-046 — Payout-destination takeover safeguards

Adding, replacing, selecting, or removing a payout destination is a high-impact
financial action. The product MUST:

- require recent strong or stepped-up driver authentication;
- record actor, session, device or client context, prior and replacement
  versions, risk decision, and result;
- notify an existing verified contact channel independently of any newly added
  channel, except where a recorded safety or legal rule forbids it;
- keep a new or changed destination unavailable until required verification
  and risk controls finish;
- support a policy-governed hold or manual review before first payout when the
  change is high risk;
- prevent ordinary support from bypassing destination verification or payout
  safeguards; and
- provide a rapid lock, recovery, and dispute path for suspected takeover.

Account recovery, authenticator reset, suspicious session activity, or payout
destination change MUST trigger the recorded review or restriction of pending
payout work required by risk policy. A safeguard MUST NOT erase the driver's
underlying earnings.

## Ledgers, Corrections, and Receipts

### FIN-050 — Immutable financial entries

Every authoritative financial change MUST append an immutable entry. Posted
entries MUST NOT be edited or deleted to correct a business outcome.

Corrections MUST use linked compensating entries that identify the original
entry, reason, policy, authorizing actor, effective time, and recorded time.

### FIN-051 — Balance integrity

Ledger entries MUST identify the parties or accounts whose obligations change
and MUST balance exactly within each currency under the product's accounting
policy.

Passenger receivable, provider clearing, driver payable, platform amount, tax
or toll payable, future promotion funding when enabled, refund, dispute,
optional provider transfer, and payout MUST remain distinguishable even when a
user interface presents a simpler total.

A projection or cached balance MUST be reproducible from authoritative entries
and checkpoints.

### FIN-052 — Receipt revisions

The passenger receipt MUST show the accepted quote, final fare, components,
payment projection, and every later tip, adjustment, refund, or dispute effect.

A later financial event MUST create a new receipt revision. Prior revisions
MUST remain auditable and MUST NOT be presented as the current outcome.

### FIN-053 — Privileged financial action

An operator adjustment, refund approval, driver recovery, manual settlement,
or reconciliation correction MUST record:

- operator identity and role;
- grant used;
- affected financial objects;
- amount and currency;
- reason category and explanation;
- evidence;
- policy or exceptional authority;
- approval or dual-control evidence when required; and
- downstream receipt, statement, balance, and provider consequences.

Operator access MUST NOT permit silent mutation of posted entries.

## Provider Operations and Reconciliation

### FIN-060 — Durable provider operation

Every external financial mutation MUST satisfy the service-wide provider
operation contract in
[PLT-010 through PLT-015](platform.md#external-provider-operations).

Before invocation, its durable financial operation MUST additionally identify:

- stable product operation identity;
- operation type;
- business object and financial obligation;
- payment or payout instrument version;
- exact parameters and currency;
- provider;
- provider idempotency key when supported, or the documented duplicate-control
  and lookup strategy when it is not;
- ledger effect expected only after the provider outcome;
- refundable, transferable, or payable amount consumed or reserved; and
- reconciliation owner and financial exception class.

When the provider supports idempotency keys, the platform MUST use the same key
when retrying the same operation with the same parameters. It MUST use a new
product operation for a different amount, currency, instrument version, or
business effect.

Provider idempotency retention is bounded and MUST NOT replace permanent
product-level duplicate prevention
([S-PROV-04](sources.md#s-prov-04)).

### FIN-061 — Ambiguous provider outcome

Under the unknown-outcome contract in `PLT-014`, a timeout, disconnect, process
crash, or malformed response after financial-provider submission MUST leave
that financial operation `outcome unknown`.

Before initiating a materially equivalent new operation, the platform MUST
use the provider's supported duplicate-control mechanism or retrieve and
reconcile the existing provider object. The related refundable, transferable,
or payable amount MUST remain reserved against a conflicting new operation
until the unknown is resolved or an explicit reviewed exception is accepted.

### FIN-062 — Provider event intake

Financial provider events MUST satisfy the canonical inbox, authentication,
deduplication, replay, and ordering contract in `PLT-013`.

They MUST additionally link the provider object and event to the exact
financial operation, obligation, instrument version, amount, currency, provider
effective time, and expected ledger effect. Processing MUST retrieve current
provider state when an event is missing, stale, contradictory, or received
before a prerequisite financial object.

Event receipt MUST NOT itself post an unexplained ledger entry.

Basis: representative payment webhooks retry, duplicate, and do not guarantee
order ([S-PROV-05](sources.md#s-prov-05)).

### FIN-063 — Cross-system completion

The transactional-publication contract in `PLT-012` governs durable financial
work. The platform MUST NOT assume one atomic transaction can update its ledger
and an external provider.

Internal obligation, provider-operation intent, provider outcome, ledger
effect, and user-visible projection MUST remain recoverable independently.
Queued work and reconciliation MUST converge them without duplicating the
business effect.

### FIN-064 — Reconciliation

The product MUST support scheduled and on-demand reconciliation across:

- internal financial obligations and entries;
- provider payment-readiness, funds-reservation, and collection operations;
- refunds and disputes;
- optional provider-side balances, credits, or transfers where exposed;
- payouts and payout failures; and
- provider balance transactions, fees, and reserves where available.

Reconciliation MUST classify at least:

- matched;
- internally pending;
- provider pending;
- missing internally;
- missing at provider;
- amount or currency mismatch;
- duplicate candidate;
- stale or contradictory status; and
- operator review required.

Every discrepancy MUST retain evidence, owner, severity, age, attempted repair,
and resolution. Reconciliation correction MUST use normal immutable financial
entries and provider operations.

### FIN-065 — Financial degradation

When a payment, refund, transfer, or payout provider is unavailable:

- the product MUST stop new dispatch if payment readiness cannot be
  established;
- an assigned or active trip MUST remain safely progressable;
- trip completion and internal fare and earning obligations MUST remain
  recordable;
- an external operation not yet submitted MUST remain durably pending;
- an operation that may already have been accepted externally MUST remain
  `outcome unknown` until provider retrieval or reconciliation establishes its
  result;
- a known terminal provider outcome MUST remain terminal rather than be
  relabeled because the provider later became unavailable; and
- passenger, driver, and operator surfaces MUST show accurate degraded status.

The product MUST NOT convert provider unavailability into a fabricated success
or failure, silently discard the financial obligation, or assume that payment
and payout use the same provider.

## Financial Acceptance Scenarios

The release suite MUST prove:

1. repeated acceptance of one quote creates one passenger request and at most
   one initial provider payment operation;
2. an expired quote produces new terms without dispatch or collection under
   the old quote;
3. all passenger and driver components sum exactly in the stated currency,
   including an explicit rounding remainder when needed;
4. a passenger destination change requiring new terms cannot change the fare
   without recorded consent;
5. final fare differences retain the accepted quote, policy, reason, evidence,
   and exact delta;
6. passenger fare, driver earning, platform amount, tax, toll, and tip can
   change independently without corrupting another obligation;
7. a payment-readiness or funds-reservation timeout is reconciled without a
   duplicate provider operation;
8. a collection that succeeds before a process crash produces one provider
   collection and one set of ledger effects;
9. duplicate and out-of-order provider events converge to the same financial
   projection;
10. trip completion during provider outage records fare and driver earning
    while collection remains accurately pending or outcome unknown according
    to whether the collection was submitted;
11. a partial refund creates compensating entries and a new receipt revision
    without rewriting the collection or fare;
12. a pending or failed refund is not displayed as received;
13. concurrent refund attempts cannot exceed the refundable amount;
14. a dispute opened during a refund cannot reimburse the passenger twice;
15. a refund after driver payout preserves the original earning and payout and
    creates an explained policy-governed adjustment or platform loss;
16. a tip added after payout accrues to a later driver balance and statement;
17. repeating one payout request cannot select earnings twice or create a
    second payout;
18. a failed payout preserves the attempt, restores or accounts for the balance
    once, and creates an actionable payout-account state;
19. reconciliation detects an external transaction with no internal operation
    and prevents automatic posting without evidence;
20. an operator correction records authority and evidence and updates receipt,
    statement, balance, and provider work through immutable new events;
21. a failed passenger-instrument verification cannot establish payment
    readiness or dispatch;
22. replacing or removing the instrument bound to an active authorization,
    collection, refund, or dispute cannot redirect or erase that operation;
23. a suspicious passenger-instrument change after account recovery creates
    the required authentication, notification, audit, and recovery outcomes
    without creating a ride or collection;
24. replacing a payout destination while a scheduled or initiated payout is in
    flight cannot redirect that payout to the replacement;
25. an unverified, restricted, disabled, or removed payout instrument blocks
    new payout without erasing earnings or historical attempts;
26. a suspected payout-destination takeover invokes strong authentication,
    independent notice, review or hold, and recovery without silently releasing
    funds; and
27. early trip termination preserves traveled evidence and produces separate,
    explainable passenger fare and driver earning outcomes without pretending
    destination completion or pre-trip cancellation.

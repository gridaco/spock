# Driver Requirements

This document specifies the supply-side product from onboarding through
eligibility, availability, offer response, trip execution, earnings visibility,
support, and appeal.

Grounding: [Uber Driver app](sources.md#s-prod-03),
[driver basics](sources.md#s-prod-04),
[driver earnings](sources.md#s-prod-05),
[Lyft driver cancellation](sources.md#s-comp-03), and the
[California eligibility baseline](sources.md#s-reg-01).

## Identity, Onboarding, and Eligibility

### DRV-001 — Separate person, account, driver, and vehicle

The product MUST distinguish:

- person and platform account;
- driver profile;
- market and service-class eligibility;
- vehicle;
- driver credential or screening evidence;
- vehicle credential or inspection evidence;
- payment/payout onboarding; and
- current availability.

Passing one check MUST NOT imply the others.

### DRV-002 — Onboarding status

The driver MUST be able to submit required information and see a status for
each requirement:

- not submitted;
- under review;
- action required;
- verified;
- rejected;
- expired;
- suspended; or
- not applicable.

The product MUST identify the relevant market, service class, vehicle, issuer,
effective time, expiry or review time, and support path.

### DRV-003 — Market-specific evaluation

Eligibility MUST be evaluated from the evidence and policy effective for the
specific market, service class, vehicle, and time. The same driver MAY have
different eligibility in different markets.

Basis: CPUC driver-record and insurance requirements are time- and
jurisdiction-bound ([S-REG-02](sources.md#s-reg-02),
[S-REG-03](sources.md#s-reg-03)).

### DRV-004 — Continuing eligibility

New evidence, expiry, revocation, screening notice, safety restriction, or
policy change MUST trigger a recorded reevaluation. Historical trips retain the
eligibility decision that applied at assignment time.

### DRV-005 — Review and appeal

An adverse or restrictive decision MUST provide the driver with the category,
effective scope, available explanation, correction path, and appeal path that
law and safety permit. The evidence source and platform decision MUST remain
distinct.

### DRV-006 — Payout readiness is separate

A driver may be eligible to transport passengers but unable to receive a new
payout, or vice versa. The driver product MUST show transportation eligibility,
earnings accrual, payout-account readiness, and payout status separately.

### DRV-007 — Account access and recovery

The driver MUST be able to authenticate, recover account access, sign out, and
review or revoke active sessions. Disabled, deleted, or safety-restricted
accounts MUST receive an accurate refusal and an allowed support or review path.

Account recovery MUST NOT create a new driver identity, bypass eligibility,
restore a restriction, or redirect an existing payout. A recovery or
credential change MUST trigger the session, payout-instrument, and active-work
review required by the effective risk policy.

Basis: explicit product decision grounded in the account-based driver
application ([S-PROD-03](sources.md#s-prod-03)).

## Availability

### DRV-010 — Go online

An eligible driver with an eligible vehicle MAY request to go online in a
market. The platform MUST re-evaluate current eligibility, active restriction,
vehicle, service preferences, market status, and conflicting work before
making the driver available.

Basis: reference driver products require approval before online availability
([S-PROD-03](sources.md#s-prod-03),
[S-PROD-04](sources.md#s-prod-04)).

### DRV-011 — Availability states

The driver surface MUST distinguish:

- offline;
- online but not eligible for offers;
- available;
- temporarily reserved;
- assigned and approaching;
- waiting;
- in trip; and
- action required.

The driver MUST not appear available merely because the application is open.

### DRV-012 — Go offline

The driver MAY request to go offline when no assignment or active trip prevents
it. If an offer or assignment races with the request, the product MUST return
the authoritative result rather than showing both offline and assigned.

### DRV-013 — Preferences

The driver MAY select among service classes, vehicle, and market preferences
for which they are eligible. Preferences are not eligibility and do not
guarantee an offer.

## Offers and Assignment

### DRV-020 — Offer disclosure

Before response, an offer MUST show the information required by market policy,
which may include:

- offer type: exclusive or multi-driver;
- expiry;
- pickup area and estimated approach;
- destination or trip direction where required;
- service class;
- estimated duration or distance;
- driver earning or earning basis;
- vehicle or service capability needed to fulfill the selected service class;
- approved passenger rating aggregate and policy version, or verification
  state, where permitted; and
- material toll, airport, or market rule.

The offer MUST identify estimates as estimates.

Protected passenger instructions, including service-animal or assistive-device
handling, MUST ordinarily be disclosed only after assignment. If a capability
must affect candidate eligibility before offer, the offer may disclose the
service or vehicle constraint but MUST NOT disclose a diagnosis or unnecessary
passenger detail. Decline or cancellation after protected instructions are
disclosed MUST retain a reviewable reason and accessibility-case path.

### DRV-021 — Offer audience

The product MUST tell the driver whether accepting an offer will immediately
claim an exclusive opportunity or merely express interest in a multi-driver
offer.

Basis: Uber publicly distinguishes exclusive requests and multi-driver Trip
Radar interest ([S-PROD-03](sources.md#s-prod-03)).

### DRV-022 — Offer expiry

Offer expiry is enforced by the platform clock. A delayed push or offline
client MUST NOT extend it. The driver MAY open an expired offer to learn the
outcome, but cannot revive it.

### DRV-023 — Acceptance

The driver MAY accept according to the offer type. Acceptance MUST use one
idempotent operation and MUST return one of:

- assigned;
- interest recorded, awaiting result;
- expired;
- already assigned elsewhere;
- driver no longer eligible;
- driver no longer available; or
- offer withdrawn.

An acceptance acknowledgment is not an assignment unless the platform says it
is.

### DRV-024 — Decline and ignore

The driver MAY decline or allow an offer to expire. Any effect on metrics,
rewards, or eligibility MUST be disclosed according to market policy and
distinguished from cancellation after assignment.

### DRV-025 — Assignment

After assignment, the driver MUST receive:

- passenger's approved display identity;
- passenger verification state if used;
- pickup and operational notes;
- route to pickup and ETA;
- contact path;
- trip/service terms;
- cancellation and waiting rules; and
- safety entry point.

Only information necessary for the current phase may be disclosed.

## Pickup and Trip Execution

### DRV-030 — Navigate to pickup

The driver MUST be able to navigate toward the confirmed pickup while seeing
route/ETA freshness and material pickup instructions. A maps failure MUST be
visible and MUST NOT cancel the assignment automatically.

### DRV-031 — Arrival

The driver MAY declare arrival only within the market's allowed conditions. The
platform MUST record:

- driver declaration time;
- driver location observation and quality;
- pickup reference;
- route progress;
- platform acceptance time; and
- applied policy version.

The platform MAY reject, defer, or flag an implausible arrival.

### DRV-032 — Waiting and contact

The driver MUST see the start and end conditions for waiting, any chargeable
period, required contact attempt, and no-show eligibility. Waiting time MUST be
measured by the authoritative platform timeline, not only a client countdown.

### DRV-033 — Passenger and ride verification

The driver MUST be able to compare the passenger's approved display identity
and complete any PIN or reciprocal verification. A perceived mismatch MUST
offer a safe cancellation/report path.

Verification state MUST not authorize discriminatory refusal
([S-COMP-05](sources.md#s-comp-05)).

### DRV-034 — Start trip

The driver MAY request trip start only for the active assignment and allowed
phase. The platform MUST reject duplicate, early, stale, or conflicting starts
without moving a newer trip backward.

The start result MUST be recoverable after a timeout.

### DRV-035 — Active trip

The driver surface MUST show:

- authoritative trip state;
- destination and approved changes;
- route and ETA;
- passenger contact;
- safety and support;
- connectivity/freshness;
- material earning terms; and
- completion availability.

Passenger app failure MUST NOT prevent safe trip completion.

### DRV-036 — Destination change

A driver cannot unilaterally change passenger route intent. Driver and passenger
surfaces MUST converge on a passenger-authored accepted change before it
becomes the destination. A live safety matter uses support or the explicit
early-termination workflow; an operator correction is historical and cannot
replace active route intent.

### DRV-037 — Complete trip

The driver MAY request completion only in an allowed phase. The platform MUST
record the declaration, location quality, destination context, accepted time,
and policy. Duplicate completion MUST return the existing outcome.

Maps or payment failure MUST NOT force the driver to keep a physically
completed trip open indefinitely.

## Driver Cancellation and No-Show

### DRV-040 — Driver cancellation

The driver MAY request cancellation before trip start using a reason category.
The result MUST disclose:

- whether the cancellation succeeded;
- whether the passenger request will rematch or terminate;
- effect on driver metrics or compensation;
- any evidence requirement; and
- support or safety follow-up.

### DRV-041 — No-show

The driver MAY claim a passenger no-show only after the configured arrival,
waiting, location, and contact conditions. The claim is an input to a platform
decision, not self-proving financial truth.

Basis: [Uber](sources.md#s-prod-06) and
[Lyft](sources.md#s-comp-03) both require phase-specific evidence and vary by
market.

### DRV-042 — Safety cancellation

The product MUST provide a safety cancellation/report path that can protect the
driver without requiring unsafe continued contact. A safety reason and a
permanent passenger finding are distinct.

### DRV-043 — Cancellation race

Driver cancellation racing with passenger cancellation, rematch, arrival, or
trip start MUST reach one authoritative lifecycle ordering. Passenger charge,
driver compensation, refund, and other financial consequences MUST be derived
separately from that ordering and their effective policies. Neither client may
infer the winner from its local timestamp.

### DRV-044 — End an active trip

After trip start, the driver MUST use an early-termination or safety action
rather than a pre-trip cancellation. The action MUST preserve:

- driver request and reason;
- passenger notification and available response;
- physical stop and passenger-exit evidence when available;
- safety or operator involvement;
- platform-accepted termination outcome;
- driver availability and vehicle release; and
- separate fare, earning, payment, receipt, and case work.

Early termination racing with passenger request or ordinary completion MUST
produce one authoritative trip outcome without disguising it as a trip that
never started.

## Earnings and Payout Visibility

### DRV-050 — Trip earning

After a completed or compensable canceled trip, the driver MUST see:

- accepted earning offer or earning basis;
- final earning;
- base, time, distance, waiting, toll, tip, tax, fee, adjustment, and any
  future-enabled incentive components as applicable;
- why the final amount differs;
- earning availability;
- related trip; and
- review path.

Passenger fare and driver earning MUST NOT be presented as necessarily equal or
as a fixed commission relationship.

### DRV-051 — Balance

The driver product MUST distinguish accrued, pending, available, reserved,
paid, failed, recovered, and reversed amounts. A successful payout request is
not proof that the receiving bank credited funds.

### DRV-052 — Statements

The driver MUST be able to review trip-level activity and periodic statements
that reconcile earnings, tips, adjustments, provider-side transfers or credits
when present, payout fees, failed payouts, and reversals.

### DRV-053 — Adjustment and appeal

A later refund, dispute, safety decision, or operator correction MUST produce a
new driver adjustment under explicit policy. It MUST NOT rewrite a historical
earning or payout. The driver MUST receive an explanation and available review
path.

## Driver Support and Safety

### DRV-060 — Support

The driver MUST be able to open a case for eligibility, documents, vehicle,
offer, pickup, passenger, trip, fare, earning, payout, safety, discrimination,
accessibility, lost property, and account decisions.

### DRV-061 — Safety tools

Safety entry MUST be available while online, approaching, waiting, and in trip.
Trusted-contact sharing MUST not expose passenger pickup or destination beyond
the approved privacy design.

Basis: driver products expose safety and driver-scoped location sharing
([S-PROD-03](sources.md#s-prod-03),
[S-COMP-04](sources.md#s-comp-04)).

### DRV-062 — Restriction and active trip

If eligibility or account status changes during an active trip, policy MUST
distinguish:

- block new offers;
- safely finish the current trip;
- direct the trip to stop;
- require operator intervention; and
- emergency escalation.

A background reevaluation MUST NOT leave passenger and driver clients with
contradictory hidden instructions.

### DRV-063 — Passenger rating and feedback

After an eligible completed or exceptionally terminated trip, the driver MAY
submit one passenger rating and feedback under the market's rating policy. The
submission MUST retain author, subject, trip, scale and policy version,
submitted time, any permitted revision or withdrawal, and moderation status.

A rating MUST NOT itself become a safety finding, accessibility decision, or
eligibility restriction. Any aggregate disclosed in later offers MUST use the
approved aggregation and moderation policy rather than a mutable client value.

## Driver Acceptance Scenarios

The release suite MUST prove:

1. an expired credential blocks new availability but does not rewrite
   historical eligibility;
2. payment onboarding and transport eligibility fail independently;
3. going offline racing with an offer yields one availability state;
4. delayed offer delivery cannot produce a late assignment;
5. duplicate acceptance creates at most one assignment;
6. multi-driver interest can lose without being reported as driver
   cancellation;
7. arrival retains location quality and can be disputed;
8. duplicate start and completion are harmless;
9. a maps outage does not erase an assignment or completed physical trip;
10. no-show compensation requires the configured evidence;
11. an earning adjustment preserves the original earning and payout;
12. an adverse eligibility decision has a recorded policy, evidence source,
    effective scope, and appeal outcome;
13. an early-termination request releases driver and vehicle exactly once while
    preserving separately decided financial consequences; and
14. passenger-rating revision or moderation preserves the original submission
    and cannot silently become a safety finding;
15. driver account recovery cannot bypass eligibility, restore a restriction,
    or redirect an in-flight payout.

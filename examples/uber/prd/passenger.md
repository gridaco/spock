# Passenger and Requester Requirements

This document specifies the demand-side product for the initial scope: the
requester and passenger are one authenticated adult using an electronic payment
method for an immediate private ride.

Grounding: [Uber rider journey](sources.md#s-prod-01),
[Uber app behavior](sources.md#s-prod-02),
[Lyft request comparator](sources.md#s-comp-01),
[Lyft pickup comparator](sources.md#s-comp-02), and
[mobility safety tools](sources.md#s-prod-07).

## Account and Readiness

### PAX-001 — Account access

The product MUST let a passenger create an account, authenticate, recover
access, sign out, and review account status. Disabled, deleted, or
safety-restricted accounts MUST receive a non-deceptive refusal and an
appropriate support path.

Basis: product decision and the account-based reference journey
([S-PROD-02](sources.md#s-prod-02)).

### PAX-002 — Passenger eligibility

The initial product MUST require the passenger to attest that they are an adult
and are requesting for themselves. A later delegated-ride or teen product MUST
introduce separate requester, passenger, guardian, consent, contact, and payment
authority rather than reusing this attestation.

Basis: product boundary; comparator evidence shows these are distinct products,
not edge cases ([S-COMP-01](sources.md#s-comp-01)).

### PAX-003 — Payment readiness

Before dispatch begins, the passenger MUST have a payment method that has
passed the market's readiness check for the quoted amount or stated
funds-readiness policy. A decline MUST not create a searchable trip request.

Payment readiness MUST NOT be displayed as final payment.

Basis: product decision and [marketplace provider
constraints](sources.md#s-prov-03).

### PAX-004 — Location permission is optional

The passenger MUST be able to enter and confirm pickup and destination without
granting continuous precise device location. If device location is used to
suggest pickup, the product MUST show that it is a suggestion and require
confirmation.

Basis: explicit product privacy decision.

## Route Intent and Quote

### PAX-010 — Pickup and destination

The passenger MUST select and confirm one pickup and one destination. Each
confirmation MUST preserve:

- passenger-visible address or place label;
- coordinates used for routing;
- provider place identifier when contractually retainable;
- entrance, pickup note, or access instruction;
- confirmation time; and
- whether the value came from search, device location, map movement, history,
  or manual entry.

### PAX-011 — Service availability

The product MUST evaluate the selected points, market, service class, operating
hours, passenger eligibility, payment readiness, and any active restrictions
before offering a request.

If service is unavailable, the product MUST distinguish at least:

- outside the service area;
- no eligible service class;
- no current supply estimate;
- payment not ready;
- account not eligible;
- maps or pricing unavailable; and
- product temporarily unavailable.

It MUST NOT promise that no driver will ever be available merely because the
current search has no candidate.

### PAX-012 — Quote presentation

Before request, the product MUST present:

- service class;
- passenger price or permitted price range;
- currency;
- estimated pickup time or range;
- estimated trip duration;
- material taxes, toll assumptions, fees, and surcharges;
- quote expiry;
- cancellation and no-show summary;
- material conditions under which the final fare may change; and
- accessibility or capacity characteristics relevant to selection.

The price and ETA MUST be labeled as estimates or guaranteed terms according to
the market policy actually used.

Basis: [Uber](sources.md#s-prod-01) and
[Lyft](sources.md#s-comp-01) both expose service choice before request;
pricing is market-dependent.

### PAX-013 — Quote identity and expiry

The client MUST submit the exact quote identifier it displayed. Dispatch MUST
NOT begin from an expired, superseded, differently scoped, or materially
modified quote without presenting new terms.

Repeated acceptance of the same quote with the same idempotency key MUST return
the same request and MUST NOT create a second provider payment operation or
trip.

### PAX-014 — Accessible comprehension

Route selection, quote comparison, financial consent, cancellation, and safety
controls MUST meet the product's accessibility target and MUST NOT rely only on
map position, color, motion, or a countdown without an accessible alternative.

Basis: [WCAG 2.2](sources.md#s-std-01).

## Request and Matching

### PAX-020 — One intentional request

The passenger MUST explicitly confirm request submission. The product MUST
show one durable request identity and its current outcome:

- searching;
- assigned;
- unfulfilled;
- canceled; or
- action required.

Closing the app, losing a connection, or missing a notification MUST NOT cancel
the request implicitly.

### PAX-021 — Search progress

While searching, the passenger SHOULD see a current status and MAY see a
changing estimate. The product MUST distinguish an estimate from an assignment
and MUST offer cancellation according to effective policy.

### PAX-022 — Assignment disclosure

After assignment, the passenger MUST receive enough information to identify
the expected ride:

- driver first name or approved display identity;
- driver photograph;
- vehicle make, model, color, and license plate;
- service class;
- approved driver rating aggregate and policy version when the market uses it;
- pickup ETA and freshness;
- safety verification method; and
- masked contact path.

Basis: [Uber pickup](sources.md#s-prod-02) and
[Lyft pickup](sources.md#s-comp-02).

### PAX-023 — Rematch

If an assignment is released before pickup, the passenger MUST see whether the
request is searching again, needs new terms, or has terminated. A rematch MUST
NOT silently retain an expired quote or hide a cancellation charge.

## Pickup

### PAX-030 — Driver approach

The passenger MUST be able to see the driver's role-appropriate location,
route progress, and ETA with freshness. Stale or unavailable location MUST be
shown as such; it MUST NOT be animated as current movement.

### PAX-031 — Contact and instructions

The passenger MUST have an in-product, privacy-preserving way to contact the
assigned driver and send pickup instructions. Contact access MUST expire after
the purpose and retention window defined by market policy.

### PAX-032 — Verify the ride

Before boarding, the product MUST tell the passenger to verify driver, vehicle,
and plate. The market MAY require a PIN or equivalent reciprocal check before
trip start.

A successful PIN proves knowledge of that trip's challenge; it does not prove
every real-world safety fact.

Basis: [Uber safety](sources.md#s-prod-07).

### PAX-033 — Arrival and waiting

The passenger MUST see:

- whether the driver has declared arrival;
- when the waiting period began;
- applicable grace and charge rules;
- how to contact the driver;
- how to report an incorrect arrival; and
- how cancellation would be classified at that moment.

The screen MUST NOT imply that GPS proximity alone is conclusive evidence.

### PAX-034 — Accessibility at pickup

The passenger MUST be able to provide operationally necessary accessibility
instructions without disclosing a diagnosis. Service-animal or assistive-device
handling MUST not be represented as an ordinary preference that a driver may
ignore where law or policy forbids denial.

Basis: [service-animal sources](sources.md#s-prod-08) and
[ADA baseline](sources.md#s-reg-06).

## Active Trip

### PAX-040 — Trip start

The passenger MUST be notified when the platform accepts trip start. If the
passenger disputes that the correct trip began, a safety/support path MUST be
available without requiring the passenger to endanger themselves.

### PAX-041 — Active-trip surface

The active-trip surface MUST show:

- driver and vehicle identity;
- current destination;
- route and destination ETA with freshness;
- material trip status;
- safety entry point;
- trusted-contact sharing;
- masked contact;
- destination-change availability; and
- loss-of-connectivity state.

The physical trip MUST remain completable if the passenger application
disconnects.

### PAX-042 — Destination change

The passenger MAY request a destination change only when market policy and the
trip phase allow it. The product MUST show any material price or route effect
and capture required consent before making the new destination authoritative.

Only the passenger may author a live destination change. A safety operator may
provide support or invoke the configured early-termination workflow, but MUST
NOT replace live passenger route intent. A later historical correction cannot
change the active destination.

### PAX-043 — Safety and trusted contacts

The passenger MUST be able to open safety help during pickup and an active
trip. Trusted-contact sharing MUST:

- be explicit or follow a previously selected sharing rule;
- identify what is shared and for how long;
- be revocable;
- expire when its purpose ends; and
- avoid granting account or trip-control authority.

Basis: [Uber](sources.md#s-prod-07) and
[Lyft](sources.md#s-comp-04) safety behavior.

### PAX-044 — Emergency boundary

The product MUST clearly distinguish platform safety support from local
emergency services. If an emergency integration is offered, it MUST disclose
what trip, vehicle, identity, and location information will be shared and
whether location permission affects that sharing.

Basis: [Lyft safety comparator](sources.md#s-comp-06).

### PAX-045 — Request to end an active trip

The passenger MUST be able to request that an active trip end, including a
prominent safety path. The product MUST distinguish:

- the passenger's request;
- instructions to the driver;
- physical stopping or exit;
- platform acceptance of early termination;
- emergency or operator intervention; and
- resulting fare, payment, earning, receipt, and support outcomes.

A client tap MUST NOT fabricate physical completion. If immediate stopping
would be unsafe, the product MUST expose the reviewed market response without
hiding the passenger's original request or its time.

## Completion and Post-Trip

### PAX-050 — Completion

The passenger MUST receive an authoritative completed or exceptional outcome.
A lost completion notification MUST be recoverable from trip history.

### PAX-051 — Receipt

The receipt MUST show:

- accepted quote;
- final fare;
- each material difference and reason;
- each applicable tax, toll, fee, surcharge, future-enabled discount, and tip
  separately;
- payment method display reference;
- payment status;
- trip route summary and times;
- driver and vehicle;
- receipt revision; and
- fare-review and support paths.

The receipt MUST update by new revision when a later adjustment, tip, refund,
or dispute changes the financial projection. Prior revisions remain auditable.

### PAX-052 — Rating and feedback

After an eligible completed or exceptionally terminated trip, the passenger
MAY submit one driver rating and structured or freeform feedback under the
market's rating policy. The submission MUST retain author, subject, trip,
scale and policy version, submitted time, any permitted revision or withdrawal,
and moderation status.

An aggregate shown later MUST identify the governing aggregation policy and
MUST exclude withdrawn or invalidated input as that policy declares. Rating,
safety report, accessibility complaint, payment dispute, and ordinary support
request MUST remain distinct even if the interface offers them together.

### PAX-053 — Tip

The initial product MUST let the passenger optionally add a tip after
completion. A tip is a separate, attributable financial event and MUST NOT
mutate the base fare or be required for a rating.

### PAX-054 — Trip history

The passenger MUST be able to review trips and canceled requests with
role-appropriate status, receipt, and support outcome. Sensitive location
detail MAY be reduced after its active purpose while retaining legally required
records.

### PAX-055 — Support, dispute, and lost property

The passenger MUST be able to open a case against the correct trip for:

- fare or cancellation review;
- payment problem;
- driver or vehicle mismatch;
- lost property;
- safety incident;
- discrimination or accessibility denial;
- account issue; and
- privacy request.

The case MUST preserve the passenger's original statement. Submission does not
make the allegation a confirmed fact.

## Cancellation

### PAX-060 — Passenger cancellation

The passenger MUST be able to request cancellation while searching, assigned,
or at pickup. The result MUST state:

- whether cancellation succeeded;
- the request and assignment state;
- whether a charge or temporary funds reservation exists;
- the applicable policy and evidence;
- any driver compensation consequence that may be disclosed; and
- how to request review.

### PAX-061 — Cancellation race

Cancellation racing with driver acceptance, assignment, arrival, or trip start
MUST resolve to one authoritative outcome. A timeout or ambiguous client
response MUST lead to status recovery, not a second cancellation or request.

### PAX-062 — Fee review

A cancellation or no-show charge MUST be reviewable. The review MUST retain the
policy version, assignment and arrival timeline, location quality, contact
attempts, waiting evidence, and operator decision.

Basis: cancellation evidence differs by market and GPS is imperfect
([Uber](sources.md#s-prod-06),
[Lyft](sources.md#s-comp-03)).

## Passenger Acceptance Scenarios

The release suite MUST prove:

1. repeated quote acceptance creates one request and at most one initial
   provider payment operation;
2. quote expiry before submission produces new terms without dispatch;
3. no available driver ends in an unfulfilled request, not an invented trip;
4. rematch replaces passenger-visible driver/vehicle details without creating a
   second trip;
5. stale driver location is visibly stale;
6. pickup can proceed when the passenger denied continuous location;
7. a delayed push cannot regress the passenger from active trip to searching;
8. cancellation racing with assignment produces one chargeable or
   non-chargeable outcome with evidence;
9. trip completion remains recoverable after client disconnect;
10. a fare adjustment creates a new receipt revision and explains the delta;
11. safety, accessibility, rating, and fare reports remain distinguishable;
12. a privacy request can report which fields were deleted, retained, or held
    and why;
13. a request to end an active trip remains distinct from physical stopping,
    early termination, and its financial outcomes; and
14. rating revision and moderation preserve the original submission while a
    tip remains independent of both rating and base fare.

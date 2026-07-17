# Marketplace and Trip Orchestration Requirements

This document specifies the platform behavior that connects one passenger
request to one driver and one private, immediate trip. Multiple drivers may be
considered or offered the work, but only one assignment and one trip may become
authoritative in the initial scope.

Grounding: [Uber dispatch and trip behavior](sources.md#s-prod-03),
[Uber cancellation review](sources.md#s-prod-06),
[Lyft cancellation and no-show behavior](sources.md#s-comp-03),
[point-to-point routes](sources.md#s-prov-01),
[candidate route matrices](sources.md#s-prov-02), and
[push-delivery constraints](sources.md#s-prov-07).

## Market Configuration and Authority

### MKT-001 — Versioned market profile

Every request, dispatch decision, assignment, and trip MUST resolve through an
identified market profile. Each profile MUST declare a canonical scope key
covering the market, service class, jurisdictional application, and policy
families it governs. An approved profile version MUST define at least:

- jurisdiction, service area, time zone, currency, and operating periods;
- enabled service classes and driver/vehicle eligibility requirements;
- quote validity, price-change, payment-readiness, and final-fare policy;
- offer modes, disclosure, expiry, response, and candidate limits;
- driver and vehicle reservation, assignment capacity, release, rematch, and
  search-expiry policy;
- location quality, freshness, arrival, and route-fallback policy;
- pickup change, destination change, waiting, no-show, and cancellation policy;
- accessibility, safety, privacy, evidence, and retention requirements; and
- exceptional-action, correction, review, and appeal authority.

The profile MUST have an effective interval, lifecycle state, approving
authority, and supersession history. Editing a draft MUST NOT change active
market behavior.

The platform MUST ensure that, for any scope key and business instant, at most
one active profile version applies. While the market is declared available,
exactly one active version MUST resolve. Publication or activation that would
create overlapping applicability MUST fail. A planned or accidental gap MUST
make the affected scope unavailable; it MUST NOT fall through to an arbitrary
older, newer, or less-specific profile.

Profile approval, activation, supersession, and retirement MUST be
idempotent, revision-checked decisions. Concurrent activation attempts MUST
produce one accepted ordering and a recoverable losing result.

Basis: public products vary behavior by market and service, while regulation
changes obligations by jurisdiction
([S-PROD-03](sources.md#s-prod-03),
[S-PROD-06](sources.md#s-prod-06),
[S-REG-01](sources.md#s-reg-01)).

### MKT-002 — Policy resolution

Every consequential decision MUST retain the market profile version and
decision time used to make it. A later profile activation MUST NOT silently
rewrite a quote, offer, assignment, wait window, cancellation decision, or
completed trip.

When policy changes during an active request or trip, the market profile MUST
state which terms remain fixed, which terms may change prospectively, what
notice or consent is required, and when a new decision is mandatory.

If no valid policy resolves, or more than one candidate remains after applying
the declared scope rules, the platform MUST refuse the new safety- or
money-relevant transition and expose an operational fault. It MUST NOT choose
an arbitrary current, historical, or nominally more specific version.

### MKT-003 — Separate lifecycles

The product MUST represent these as related but distinct lifecycles:

- passenger request and search;
- dispatch attempts and driver offers;
- temporary driver and vehicle reservations;
- driver availability;
- authoritative assignment;
- physical trip execution;
- passenger payment and final fare;
- driver earning and payout; and
- support, safety, correction, and appeal.

A transition in one lifecycle MUST NOT imply an unrecorded transition in
another. In particular:

- driver interest is not assignment;
- assignment is not pickup;
- arrival is not trip start;
- trip completion is not successful payment or payout;
- cancellation is not proof of fee eligibility; and
- an allegation or correction request is not a confirmed finding.

### MKT-004 — Transition authority

The initial product MUST apply the following authority boundaries:

| Transition or fact | Author | Platform responsibility |
| --- | --- | --- |
| Submit or cancel a request | Passenger | Validate and accept or reject the command |
| Start, expire, or end search | Platform | Decide under the effective market policy |
| Create, expire, withdraw, or revoke an offer | Platform | Preserve audience, terms, and timeline |
| Accept, decline, or express interest | Driver | Record the response without inventing assignment |
| Reserve or release driver and vehicle capacity | Platform | Apply a bounded dispatch reservation and its fence |
| Assign or release a driver | Platform | Fence competing requests and choose one outcome |
| Driver or passenger location | Client or location provider | Retain as an observation and project with freshness |
| Declare arrival | Assigned driver | Accept, reject, defer, or flag using policy and evidence |
| Start and advance waiting time | Platform clock | Apply the policy attached to accepted arrival |
| Claim passenger no-show | Assigned driver | Decide trip and financial consequences separately |
| Verify pickup | Passenger and driver evidence | Enforce any configured trip-start gate |
| Start or complete the trip | Assigned driver | Accept or reject the command and preserve evidence |
| Change passenger route intent | Passenger | Revalidate, reprice, capture consent, and notify driver |
| Cancel before start | Passenger or assigned driver | Resolve request, assignment, rematch, and fee decisions |
| Terminate after start | Passenger, driver, or operator under policy | Preserve the traveled trip and exceptional outcome |
| Correct an accepted outcome | Authorized operator | Create an audited correction without erasing history |

No client may make its local clock, screen state, retry, or notification
history authoritative for these transitions.

### MKT-005 — Commands, observations, decisions, and effects

The platform MUST distinguish:

- a command expressing requested change;
- observations offered as evidence;
- the platform decision accepting or rejecting the command;
- resulting lifecycle transitions; and
- external effects such as notification, funds reservation, collection, or
  payout.

Every command MUST carry an idempotency identity, actor, target, expected
revision or fence, client-observed time when available, and platform receipt
time. A timeout MUST produce a recoverable indeterminate result, not permission
to invent success or submit a logically new command.

### MKT-006 — Role-specific projections

Passenger, driver, and operator surfaces MUST derive from the same accepted
business outcomes while disclosing only role- and phase-appropriate detail.
Push, email, SMS, and realtime delivery MUST be treated as hints to retrieve
current state, not as state authority.

Delayed or reordered delivery MUST NOT move a client from a newer lifecycle
phase to an older one.

Basis: representative push systems may delay, collapse, expire, or discard
messages ([S-PROV-07](sources.md#s-prov-07)).

## Request and Search

### MKT-010 — Request creation

The platform MUST NOT create a searchable request unless it has validated:

- authenticated and eligible passenger;
- confirmed pickup and destination versions;
- service class and market availability;
- unexpired accepted quote and its terms;
- required payment readiness;
- passenger acknowledgement of material policies; and
- absence of conflicting active passenger work in the initial scope.

The accepted request MUST retain those inputs, the market profile version, an
idempotency identity, and platform acceptance time.

Repeated submission with the same idempotency identity MUST return the same
request and MUST NOT create another search, trip, or provider payment
operation.

### MKT-011 — Search lifecycle

An accepted request MUST have an explicit search outcome:

- searching;
- assigned;
- canceled;
- unfulfilled;
- action required; or
- superseded by an explicitly linked replacement.

Closing a client, losing connectivity, or missing a status notification MUST
NOT end search. Search may end only by an accepted transition under market
policy.

### MKT-012 — Search deadline and unfulfilled outcome

The platform MUST bound each dispatch attempt and the overall search using
market policy. When the search deadline or candidate policy is exhausted, the
request MUST become unfulfilled or action required.

An unfulfilled request MUST state whether the cause was:

- no eligible candidate;
- candidates found but no accepted offer;
- route or maps dependency unavailable;
- market or service suspended;
- request terms no longer valid; or
- another categorized operational failure.

It MUST NOT claim that no driver exists merely because the bounded search found
none.

### MKT-013 — Candidate eligibility

Before creating an offer, the platform MUST evaluate the driver, vehicle,
market, service class, availability, current restriction, conflicting work,
active driver or vehicle reservation, location freshness, and
route-computation result effective at that time.

Candidate eligibility MUST be reevaluated before assignment. Eligibility at
offer creation is evidence, not a permanent reservation or guarantee.

### MKT-014 — Search revisions

A pickup correction, service change, quote replacement, market suspension, or
other material request change MUST create a new request revision. The platform
MUST declare whether existing offers remain valid, are withdrawn, or require a
new dispatch attempt.

An old offer or response MUST NOT apply to a materially different revision.

### MKT-015 — Temporary dispatch reservation

A temporary dispatch reservation is a platform-authored, time-bounded hold on
one driver and one vehicle for one request revision. It is not a scheduled
ride, driver response, assignment, or trip.

Market policy MUST state whether an exclusive offer creates a reservation or
whether reservation begins only after a driver response is selected for an
assignment attempt. Expressing interest in a multi-driver offer MUST NOT by
itself reserve every interested driver or vehicle.

Every reservation MUST identify:

- request revision and dispatch attempt;
- driver and selected vehicle;
- reason and governing policy;
- reservation identity and monotonically advancing fence;
- creation and platform expiry time; and
- consumed, expired, or released outcome.

Creating, consuming, releasing, or expiring a reservation MUST compare and
advance the affected request, driver-capacity, and vehicle-capacity fences.
Offer decline, expiry, or withdrawal; request cancellation or supersession;
and driver or vehicle restriction MUST each produce an explicit reservation
decision. Ending a related offer or request MUST NOT leave an unbounded
capacity hold.

The initial product MUST permit at most one active reservation for a request,
driver, or vehicle. While reserved, the driver and vehicle MUST NOT be
presented as available for a competing assignment. A reservation may become an
assignment only through `MKT-024`; it MUST NOT authorize arrival, start,
completion, or passenger disclosure.

Reservation expiry and release MUST use the platform clock, advance the
reservation fence, and recompute driver and vehicle availability from current
online intent, eligibility, restriction, and conflicting work. A stale
reservation command MUST NOT revive capacity or affect a later request.

### MKT-016 — Vehicle binding and change

A reservation or assignment MUST bind the exact eligible vehicle selected for
that work. The same vehicle MUST NOT be actively reserved, assigned, or in trip
for another driver or request in the initial product.

A driver vehicle-change command MUST be rejected while the driver or current
vehicle has an active reservation, assignment, or trip. Changing the account's
selected vehicle MUST NOT rewrite an existing offer, reservation, assignment,
location context, passenger disclosure, insurance phase, or historical trip.

If safety, mechanical failure, or eligibility change makes the bound vehicle
unusable, the platform MUST use the normal withdrawal, release, rematch, or
early-termination transition. It MUST NOT substitute a vehicle in place.

## Offers and Assignment

### MKT-020 — Offer record

Every offer MUST identify:

- request and request revision;
- dispatch attempt;
- offer type;
- driver audience;
- market and policy version;
- passenger, route, service, and earning disclosures permitted for that offer;
- created, deliver-until, response-until, and withdrawal times;
- response and assignment semantics;
- whether and when it creates a temporary reservation; and
- terminal outcome.

Provider acceptance of a push message MUST NOT count as driver delivery or
driver response.

### MKT-021 — Exclusive offer

An exclusive offer MUST target one driver for a bounded interval. During that
interval, market policy MUST state whether the request may also participate in
another candidate channel and whether the driver and vehicle are temporarily
reserved.

Driver acceptance MAY request immediate assignment, but assignment exists only
when the platform accepts the response and returns an assignment identity.
Offer exclusivity MUST NOT bypass current driver, request, or eligibility
checks.

### MKT-022 — Multi-driver offer

A multi-driver offer MAY be visible to several eligible drivers. A driver's
positive response MUST mean interest in being matched, not possession of the
request.

Interest alone MUST NOT make the driver or vehicle temporarily reserved. If
the platform selects one interested driver for an assignment attempt, any
reservation MUST be created through `MKT-015` before assignment and MUST give
the other interested drivers a recoverable offer outcome.

The platform MUST choose at most one winner using the effective dispatch policy
and MUST give every response a recoverable outcome:

- assigned;
- interest recorded and pending;
- not matched;
- expired;
- withdrawn;
- request no longer assignable; or
- driver no longer assignable.

Losing a multi-driver match MUST NOT be recorded as driver cancellation.

Basis: the reference driver product distinguishes exclusive acceptance from
multi-driver interest and later match feedback
([S-PROD-03](sources.md#s-prod-03)).

### MKT-023 — Offer expiry and withdrawal

Offer expiry MUST use the authoritative platform clock. Late delivery, client
clock drift, application suspension, or offline response MUST NOT extend an
offer.

The platform MAY withdraw an open offer when its request, driver, route,
market, or policy basis is no longer valid. Withdrawal MUST preserve why and
when it occurred and MUST fence a response already in flight.

### MKT-024 — Assignment transaction

Assignment MUST be one platform decision that atomically establishes the
authoritative assignment facts and durable publication work:

- one active request revision;
- the request and search transition to assigned;
- one currently eligible and available driver;
- one eligible vehicle and service class;
- the driver and vehicle capacity transition to assigned under their current
  fences, including consumption of the matching reservation when reservation
  policy requires one;
- one assignment identity and monotonically advancing assignment fence;
- the governing market profile and commercial terms;
- assignment acceptance time; and
- durable work for passenger and driver projections, notifications, audit, and
  other required downstream effects.

Materialized projections and device delivery MAY occur asynchronously. A
worker or notification failure MUST leave recoverable pending publication work
and MUST NOT undo or duplicate the assignment.

The initial product MUST prevent:

- two active assignments for one request;
- one driver holding two active assignments;
- one vehicle holding two active reservations, assignments, or trips;
- one passenger holding two active trips; and
- a losing offer response from mutating the winner.

### MKT-025 — Assignment fencing

Every arrival, no-show, start, destination acknowledgement, cancellation,
completion, and other assignment-scoped command MUST carry the active
assignment identity and fence. Driver-authored commands MUST also resolve to
the driver and vehicle bound to that assignment.

After release, rematch, cancellation, or completion, commands under an older
fence MUST return the newer authoritative outcome without mutating it. A
replayed command from a disconnected client MUST NOT act on the replacement
driver or trip.

A later selected-vehicle change or vehicle eligibility update MUST NOT retarget
an assignment-scoped command. It must be rejected or handled through the
explicit restriction, release, rematch, or early-termination policy.

### MKT-026 — Assignment disclosure

After assignment, the platform MUST make the result durably retrievable before
or independently of notifications. It MUST expose:

- assigned driver and vehicle to the passenger;
- approved passenger identity and pickup instructions to the driver;
- route to pickup and ETA with freshness;
- pickup verification method;
- waiting and cancellation policy;
- masked contact and safety entry; and
- current assignment revision and status.

Drivers who responded but were not assigned MUST receive a terminal offer
outcome without passenger detail beyond the permitted retention window.

### MKT-027 — Race resolution

Offer response, passenger cancellation, driver go-offline, eligibility change,
offer withdrawal, and assignment timeout may race. The platform MUST serialize
their accepted decisions and retain enough ordering evidence to explain the
winner.

Neither client receipt time nor client wall-clock time alone may decide the
race. An ambiguous response MUST be recoverable by request, offer, or command
identity.

## Location, Routes, and ETA

### MKT-030 — Location observation

A location observation used for dispatch, arrival, route progress, safety, or
review MUST retain:

- observed subject;
- coordinates and declared coordinate system;
- source and collection method;
- observation time and platform receipt time;
- accuracy or uncertainty when supplied;
- ordering or sequence information when supplied;
- assignment or trip context when applicable; and
- privacy classification and retention policy.

An observation is evidence, not a timeless current location or proof of a
physical event.

### MKT-031 — Current-location projection

The platform MAY derive a latest usable location only under a declared
freshness and quality policy. Out-of-order observations MAY be retained, but
MUST NOT regress the current projection merely because they arrived later.

Passenger, driver, and operator views MUST label stale, approximate, degraded,
or unavailable location. They MUST NOT animate an old coordinate as current
movement.

### MKT-032 — Route and ETA provenance

Every consequential route or ETA result MUST retain:

- purpose, such as quote, candidate ranking, pickup, or destination;
- origin and destination versions;
- departure or observation time;
- travel mode, traffic preference, and material route modifiers;
- provider and provider-result identity where available;
- returned distance, duration, warnings, and the geometry or geometry reference
  used when retention is contractually permitted;
- computation and expiry time;
- fallback or degradation indication; and
- request, candidate, assignment, or trip context.

The platform MUST label ETA as an estimate unless a separate product guarantee
explicitly applies.

Provider geometry MUST be retained, reproduced, cached, or disclosed only
within the provider contract, attribution rules, purpose, and permitted
retention interval. When geometry retention is not permitted, the platform
MUST preserve only the allowed provider reference, request provenance, and
derived facts needed to explain its decision; it MUST NOT copy the geometry
into a permanent trip record.

Basis: route results depend on declared inputs and provider response
([S-PROV-01](sources.md#s-prov-01)).

### MKT-033 — Candidate route matrix

Candidate-to-pickup route results MUST be associated with the correct candidate
even when provider responses are partial, delayed, or out of order. An
element-level error MUST NOT become zero duration, zero distance, or a
successful route.

If the provider reports fallback behavior, the dispatch policy MUST either:

- allow it and mark the result degraded;
- retry or use an approved alternate source; or
- exclude the candidate with an explainable reason.

It MUST NOT rank an incomparable fallback as though it used the requested
traffic assumptions.

Basis: candidate route providers impose limits and may return fallback
conditions ([S-PROV-02](sources.md#s-prov-02)).

### MKT-034 — Route-provider degradation

A route-provider timeout or unavailable response is an unavailable read result,
not an unknown external mutation. It does not prove that a route exists or does
not exist, and it MUST NOT silently produce a route.

The platform MAY safely repeat the read within provider limits, but each result
MUST retain its own computation time and MUST NOT overwrite a newer result
merely because it arrived later. Market policy MUST define whether the product:

- retries within the same purpose and deadline;
- uses a named fallback;
- presents a degraded estimate;
- continues an already assigned or active trip without platform navigation; or
- stops new quote or dispatch work.

Maps failure MUST NOT erase an accepted assignment, arrival, trip start, or
physical completion.

## Pickup and Trip Execution

### MKT-040 — Pickup correction

Before trip start, the passenger MAY request a pickup correction only within
market limits. The platform MUST revalidate service area, driver approach,
route and ETA, quote effects, accessibility instructions, and open offers.

The corrected pickup becomes authoritative only after platform acceptance. The
result MUST state whether:

- the current assignment continues under new terms;
- offers are withdrawn and search restarts;
- passenger or driver consent is required; or
- the request must be canceled or replaced.

The original and corrected pickup versions MUST remain explainable.

### MKT-041 — Arrival declaration

Only the active assigned driver MAY declare arrival. The platform MUST evaluate
the declaration against the configured pickup, route progress, location
quality, timing, and market policy.

Acceptance MUST retain the driver's declaration, evidence considered,
platform acceptance time, and policy version. The platform MAY reject, defer,
or flag an implausible declaration.

GPS proximity alone MUST NOT conclusively prove arrival.

Basis: cancellation evidence is market-specific and GPS may disagree with the
physical pickup ([S-PROD-06](sources.md#s-prod-06)).

### MKT-042 — Waiting interval

An accepted arrival MAY start a waiting interval only under the governing
market policy. Waiting MUST use the platform clock and preserve:

- accepted arrival time;
- grace interval;
- chargeable interval if any;
- no-show eligibility time;
- accessibility extension or waiver;
- required contact evidence; and
- interruptions, corrections, or termination.

A client countdown is a projection and MUST recover from the authoritative
timeline after reconnect.

### MKT-043 — No-show decision

Only the assigned driver MAY submit a no-show claim, and only after the
configured prerequisites. The platform MUST decide separately:

- whether the claim is accepted;
- whether the assignment and request terminate;
- whether rematch is allowed;
- whether a passenger fee is eligible;
- whether driver compensation is eligible; and
- whether safety, accessibility, fraud, or operator review is required.

Missing, stale, contradictory, or poor-quality evidence MUST follow an explicit
review or denial policy rather than being treated as success.

Basis: comparator behavior requires phase-specific arrival, waiting, contact,
and location evidence ([S-COMP-03](sources.md#s-comp-03)).

### MKT-044 — Pickup verification

Before trip start, the platform MUST expose the configured reciprocal
verification method. If a PIN or equivalent challenge is required:

- it MUST be scoped to the active assignment;
- it MUST expire on release, cancellation, or rematch;
- failed attempts MUST be bounded and safety-aware;
- successful use MUST be one-time and auditable; and
- knowledge of the challenge MUST NOT be presented as proof of every physical
  safety fact.

### MKT-045 — Trip start

Only the active assigned driver MAY request trip start. The platform MUST
validate assignment fence, allowed phase, driver and vehicle, required pickup
verification, current restrictions, and conflicting terminal commands.

Accepted start MUST establish one authoritative start time and the market
profile governing the active trip. Duplicate start MUST return the existing
outcome. Early, stale, or conflicting start MUST be rejected without moving a
newer lifecycle backward.

The start result MUST remain recoverable after client or network timeout.

### MKT-046 — Destination change

During an active trip, the passenger MAY request one authoritative destination
at a time when market policy permits. The platform MUST:

- preserve the prior destination;
- validate the new destination and service boundary;
- compute material route, ETA, fare, and earning effects;
- present required terms and capture required passenger consent;
- notify the driver and obtain any acknowledgement required by policy; and
- make the new destination authoritative only after acceptance.

A driver or operator cannot author a live destination change for the passenger.
A live safety matter MUST use support or the explicit early-termination
workflow. A historical correction may repair the record but MUST NOT change
active route intent.

### MKT-047 — Active-trip continuity

Once started, the physical trip MUST remain safely completable when passenger
or driver projections disconnect, notifications fail, or maps degrade.

The platform MUST recover accepted trip state on reconnect and reject queued
commands whose assignment fence or expected revision is stale. Offline client
behavior MUST NOT create a second trip timeline.

### MKT-048 — Completion

Only the active assigned driver MAY request completion. The platform MUST
validate the assignment fence and active-trip phase and retain:

- driver declaration and platform acceptance times;
- destination version and trip revision;
- available route and location evidence with quality;
- completion policy and any anomaly indication; and
- downstream work requested for fare, payment, earning, receipt, and release.

Duplicate completion MUST return the accepted result. Maps, notification,
payment, or payout failure MUST NOT force a physically completed trip to remain
active indefinitely.

### MKT-049 — Early termination

Passenger, assigned driver, or an authorized operator under active-trip policy
MAY request that transportation end after trip start. The command MUST carry
the active assignment fence and trip revision and MUST preserve actor, reason,
requested time, safety context, and available location or communication
evidence.

The platform MUST accept, reject, or defer the command as one explicit
decision. An accepted decision MUST create a terminal `terminated` or
equivalent trip outcome, not make the pre-trip request appear canceled or
unfulfilled. It MUST preserve:

- authoritative trip start and termination decision times;
- prior and current destination;
- traveled route, time, and location evidence that may be retained;
- whether the passenger reached the intended destination;
- initiating actor, reason, and safety or support case;
- applicable market and termination policy; and
- disputed, unavailable, or contradictory evidence.

Termination racing with completion, another termination request, safety
intervention, or operator resolution MUST produce one accepted terminal
ordering. The losing command MUST return the terminal outcome without
reopening or reclassifying the trip.

Accepted termination MUST make the assignment terminal, advance its fence,
release driver and vehicle capacity according to current safety and eligibility
policy, end active-phase disclosure and contact, and durably publish work for:

- passenger and driver projections and notifications;
- partial or exceptional fare finalization;
- driver earning decision;
- payment or collection work;
- receipt and statement revision;
- safety, support, claim, or operator review when required; and
- audit and market reporting.

Those financial and case outcomes remain separate lifecycles. A terminated
trip MUST NOT rematch; arranging further transportation requires a new
passenger request.

## Cancellation, Release, Rematch, and Correction

### MKT-050 — Cancellation command

Passenger or assigned driver MAY request cancellation before trip start under
market policy. The platform MUST preserve actor, reason, request and assignment
revision, receipt time, evidence, and accepted outcome.

The cancellation response MUST distinguish:

- request canceled before assignment;
- assignment released and search resumed;
- request terminated;
- trip already started, so the cancellation command was rejected or routed to
  the distinct `MKT-049` early-termination decision;
- command rejected as stale or conflicting; and
- action requiring operator review.

### MKT-051 — Financial consequence is separate

An accepted cancellation or no-show MUST NOT directly imply a passenger charge
or driver earning. Fee eligibility and compensation MUST be evaluated as
separate decisions using the applicable policy, timeline, progress, arrival,
waiting, contact, accessibility, and exception evidence.

Later review MUST create a new decision or adjustment and preserve the original
assessment.

### MKT-052 — Assignment release

Releasing an assignment MUST:

- make the old assignment fence terminal;
- stop new assignment-scoped disclosure and contact;
- resolve driver and bound-vehicle availability under current eligibility,
  restriction, online intent, and conflicting work;
- withdraw or terminate related offers;
- state whether the passenger request terminates or may rematch; and
- preserve the release actor, reason, evidence, and policy.

Release MUST NOT erase the driver's accepted work, approach evidence, waiting
evidence, or possible financial review.

### MKT-053 — Rematch

Rematch MAY occur only before trip start and when request terms and market
policy still permit it. Rematch MUST create a new dispatch attempt and, if
successful, a new assignment identity and fence.

Before rematch, the platform MUST decide whether the accepted quote, pickup,
destination, payment readiness, cancellation terms, and search deadline remain
valid. The passenger MUST see whether search resumed under retained terms,
requires new consent, or terminated.

Passenger-visible driver and vehicle details MUST change to the new assignment
without creating a second passenger trip.

### MKT-054 — Concurrent cancellation and transition

Passenger cancellation, driver cancellation, offer response, reservation
expiry, assignment, arrival, trip start, early termination, completion, and
exceptional resolution may race. Exactly one accepted ordering MUST determine
the request, reservation, assignment, trip, and financial inputs.

A losing command MUST return the authoritative newer outcome and MUST NOT be
reinterpreted under a later assignment or policy version.

### MKT-055 — Operator correction

The product MUST distinguish:

- an evidence annotation or source correction that does not change operational
  lifecycle state;
- a historical lifecycle resolution that appends a corrected interpretation
  and downstream consequences; and
- an active operational transition, which MUST use the normal assignment,
  release, rematch, start, termination, or completion path rather than being
  disguised as a correction.

An authorized operator MAY append a correction to an accepted arrival, wait
interval, no-show, start, destination, completion, cancellation, or assignment
outcome only under the operator policy and evidence standard for that
correction.

A correction MUST NOT:

- assign or reassign a driver or vehicle;
- consume or create a dispatch reservation;
- turn a losing offer response into an active assignment;
- substitute a vehicle;
- create an active trip or impersonate a passenger or driver command;
- bypass release and rematch;
- reopen a terminal offer, reservation, request, assignment, or trip;
- bypass pickup verification, eligibility, safety, or financial authority; or
- violate an invariant in `MKT-060`.

These restrictions preserve the initial product's exclusion of human dispatch.
If a live request needs another driver, the operator may authorize or request
normal release and rematch only when policy permits; the platform remains the
assignment authority.

The correction MUST retain:

- original outcome and evidence;
- operator identity, role, purpose, and reason;
- new evidence and decision;
- original-event policy and correction-authority policy;
- effective time and correction time;
- approvals when required;
- resulting lifecycle revision; and
- downstream consequences for passenger, driver, money, reporting, and cases.

Correction MUST NOT delete the original command, provider receipt, assignment,
or trip history.

Before accepting a correction with operational, financial, safety, privacy, or
reporting consequences, the platform MUST verify that the resulting historical
interpretation and all active state still satisfy the product invariants. If
they cannot, it MUST leave active state unchanged and create an
action-required outcome.

### MKT-056 — Mid-lifecycle restriction

If passenger eligibility, driver eligibility, vehicle eligibility, market
operation, or a safety restriction changes during search, assignment, waiting,
or an active trip, policy MUST choose an explicit outcome:

- block only new work;
- withdraw offers;
- release and rematch;
- allow safe completion;
- direct an early termination;
- pause for operator action; or
- invoke an emergency path.

Passenger and driver projections MUST receive compatible instructions without
exposing restricted evidence.

### MKT-057 — Exceptional unresolved-trip resolution

If a trip has authoritatively started but the assigned driver never reconnects
to submit or recover a completion or termination command, the trip MUST enter
an explicit action-required condition after the market's resolution interval.
It MUST NOT remain silently active forever or be auto-completed from GPS
proximity.

Before exceptional resolution, the platform MUST attempt normal command
recovery and determine whether a driver command or provider result already
exists. An authorized platform policy or operator acting through a case MAY
then decide only:

- completed by exceptional resolution;
- terminated by exceptional resolution; or
- unresolved pending additional evidence.

Each accepted choice MUST end the active assignment and trip lifecycle.
`Unresolved` means a terminal, explicitly unknown historical outcome while the
evidence case remains open; later evidence may produce an append-only
correction or finding, but MUST NOT reopen the trip.

The decision MUST preserve:

- absence or unknown outcome of the driver-authored terminal command;
- last accepted trip revision and assignment fence;
- passenger and driver statements when available;
- route, time, location, communication, and destination evidence with quality;
- conflicting, disputed, and unavailable evidence;
- deciding service or operator, case, authority, policy, and approvals;
- event, effective, decision, and notification times; and
- review or appeal path.

An exceptional resolution MUST NOT be recorded as a driver-authored completion,
invent a physical drop-off time, reopen a terminal trip, or create another
assignment. It MUST advance the assignment fence, resolve driver and vehicle
capacity under current safety and eligibility policy, end active-phase
disclosure, and durably publish the same downstream fare, earning, payment,
receipt, case, audit, and reporting work required by completion or early
termination.

A late driver command racing with or following exceptional resolution MUST be
revision-checked. At most one terminal outcome may take effect; a losing late
command may be attached as evidence but MUST NOT duplicate downstream work.

## Invariants and Failure Behavior

### MKT-060 — Initial-scope invariants

The platform MUST maintain all of these invariants:

1. one passenger has at most one active request or trip;
2. one request has at most one active assignment;
3. one driver has at most one active assignment or trip;
4. one vehicle has at most one active reservation, assignment, or trip;
5. one request and one driver have at most one active temporary reservation;
6. one assignment identifies one driver, vehicle, passenger request, and
   service class;
7. an offer response cannot mutate a request after its assignment fence
   advances;
8. only the bound driver and vehicle under the active assignment may author
   driver-side trip commands;
9. accepted lifecycle state never regresses because of stale input;
10. location and ETA never independently authorize a lifecycle transition;
11. cancellation and financial consequence remain separate; and
12. correction creates history rather than replacing it.

An invariant violation MUST stop the conflicting new transition, emit an
operational fault, and preserve evidence for reconciliation. The platform MUST
NOT repair it by silently deleting one side.

### MKT-061 — Terminality and recovery

Terminal offers, reservations, assignments, requests, and trips MUST remain
terminal. A linked correction or replacement MAY change the historical
interpretation or create a new object, but MUST NOT reopen the terminal object.

Every accepted command outcome MUST be retrievable by actor and command
identity. Client recovery MUST fetch current state and outstanding action
rather than replaying an entire local history as truth.

### MKT-062 — External failure isolation

A timeout or unavailable maps, push, payment, identity, or messaging provider
MUST be classified by purpose and lifecycle phase. The product MUST define:

- what remains available;
- which new transition stops;
- which retry is safe;
- whether an external mutation outcome remains indeterminate;
- what must be reconciled; and
- when an operator or user must act.

An external provider response received later, more than once, or out of order
MUST NOT duplicate or regress a business outcome.

A read-only route or matrix timeout is unavailable input, not an indeterminate
external mutation. It follows the safe read retry, fallback, freshness, and
late-result rules in `MKT-034`.

### MKT-063 — Representative failure cases

The operational and release suites MUST cover at least:

- duplicate request submission after an ambiguous timeout;
- passenger cancellation while one or more driver responses are in flight;
- two positive responses to a multi-driver offer;
- exclusive acceptance after expiry or withdrawal;
- driver go-offline, selected-vehicle change, or driver or vehicle eligibility
  loss during reservation or assignment;
- reservation expiry while an assignment attempt is in flight;
- assignment success with one or both notifications missing;
- disconnected client replay under an old assignment fence;
- late, out-of-order, inaccurate, or implausible location;
- partial candidate-route failure or provider fallback;
- inaccessible or corrected pickup during approach;
- incorrect passenger, driver, vehicle, or pickup challenge;
- premature, duplicated, or conflicting start and completion;
- destination change while driver or passenger is offline;
- maps failure during a physical trip;
- early termination for safety;
- permanent driver-client loss after physical drop-off;
- provider timeout followed by late external success; and
- operator correction after financial or reporting work has begun.

For each case, the runbook MUST identify authority, retained evidence,
role-specific user state, safe retry, recovery or reconciliation owner, and
terminal outcome.

## Marketplace Acceptance Scenarios

The release suite MUST prove:

1. concurrent activation attempts for overlapping market-profile versions
   accept one ordering and reject the other; a scope gap makes that scope
   unavailable rather than selecting a historical, newer, or less-specific
   profile;
2. activating a new market profile changes only decisions for which its
   effective policy applies and does not rewrite an active trip;
3. failure of any request prerequisite in `MKT-010` creates neither a
   searchable request nor a dispatch search;
4. repeated request submission creates one request, one search, and at most one
   initial provider payment operation;
5. exhausted search creates an explainable unfulfilled request rather than an
   invented assignment;
6. an exclusive response received after expiry cannot assign;
7. two positive multi-driver responses create at most one assignment and the
   losing response is not driver cancellation;
8. a temporary reservation is created, consumed, released, or expired at most
   once; while active, its driver and vehicle cannot be reserved or assigned to
   competing work, and a stale command cannot revive it;
9. two drivers cannot reserve, accept, or use the same vehicle concurrently,
   and a selected-vehicle change during reservation, assignment, or trip is
   rejected without retargeting an existing command;
10. driver go-offline racing with reservation or assignment yields one
    authoritative driver, vehicle, reservation, and assignment result;
11. passenger cancellation racing with assignment yields one ordered request,
    reservation, assignment, and fee-input history;
12. an authoritative assignment commits once even when projection or
    notification work fails; durable pending work rebuilds the missing
    projection or notification without creating a second assignment;
13. release and rematch advance the assignment fence so a disconnected former
    driver or formerly bound vehicle cannot arrive, start, cancel, or complete
    the replacement trip;
14. delayed or out-of-order location does not regress the current-location
    projection or prove arrival;
15. partial route-matrix error cannot become a zero-duration candidate, and a
    provider fallback remains visibly degraded;
16. a route-read timeout is an unavailable input: safe read retry or policy
    fallback does not create an unknown mutation or fabricate a route, and an
    older late result cannot overwrite a newer accepted result;
17. route geometry is retained, reproduced, cached, and disclosed only when
    the provider contract permits it; otherwise the durable record remains
    explainable from permitted references, provenance, and derived facts;
18. stale route and ETA projections are labeled stale and a maps outage does
    not erase an accepted assignment;
19. waiting follows the platform timeline after client reconnect;
20. no-show requires the configured arrival, time, contact, and location
    evidence, while its fee and driver compensation remain separate decisions;
21. a pickup correction either revalidates the assignment under explicit terms
    or withdraws it without applying old offers to the new pickup;
22. incorrect or replayed pickup verification cannot start a trip;
23. repeated or timed-out start creates at most one accepted trip start;
24. destination change becomes authoritative only after required validation,
    repricing, consent, and driver notification;
25. passenger application failure does not prevent safe physical completion;
26. accepted early termination racing with completion creates one terminal
    trip outcome, releases driver and vehicle capacity, ends active disclosure,
    and publishes fare, earning, payment, receipt, case, audit, and reporting
    work at most once; it does not rematch the terminated trip;
27. permanent driver-client loss after physical drop-off creates an
    action-required outcome and may reach only an evidence-based exceptional
    resolution; it is not recorded as a driver-authored command, does not
    invent a physical drop-off time, and a late driver command cannot create a
    second terminal outcome or duplicate downstream work;
28. duplicate completion and a competing cancellation create one terminal trip
    outcome;
29. delayed, missing, or reordered push delivery cannot regress passenger or
    driver state; and
30. operator correction produces a new auditable revision without erasing the
    original outcome, and cannot assign or substitute a driver or vehicle,
    consume a reservation, bypass release and rematch, impersonate a trip
    command, reopen a terminal object, or violate an invariant.

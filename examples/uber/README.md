# On-Demand Mobility

This directory studies an Uber-like on-demand mobility service as a product and
backend harness.

The service coordinates four connected planes:

- the passenger or requester experience;
- the driver experience;
- the platform operator experience; and
- the infrastructure that observes, decides, communicates, and reconciles
  across all three.

All four are part of the product. This is not merely a database of rides. It is
a live marketplace in which people move, locations become stale, offers race,
external providers disagree, money settles later, and privileged operators
sometimes need to intervene.

## Why This Harness

Instagram primarily pressures identity, publishing, social relationships,
viewer-relative access, moderation, and feeds. Mobility introduces a different
set of backend problems:

- several applications participate in one business lifecycle;
- high-frequency location observations are useful without always being
  authoritative;
- several drivers may receive offers, but only one assignment may prevail;
- estimates and quotes are time-bounded while trips and financial outcomes are
  durable;
- realtime screens are projections over slower business facts;
- clients disconnect and retry while the physical trip continues;
- maps, payments, identity, messaging, and verification remain external
  authorities;
- operator corrections must change outcomes without erasing history; and
- market, safety, pricing, and eligibility policies vary by place and time.

The harness is useful only if those pressures remain visible. It should not be
reduced to a trip table and a handful of status values.

## Product Planes

### Passenger and Requester

The demand side chooses a pickup and destination, reviews an offer, requests a
trip, follows matching and pickup, completes the trip, pays, receives a
receipt, rates the experience, and seeks support when necessary.

The requester and passenger are the same person in the initial scope. They are
named separately because requesting for another person is a plausible future
requirement and should not be made impossible accidentally.

### Driver

The supply side establishes eligibility, associates an eligible vehicle, goes
online, receives and responds to offers, navigates to pickup, conducts a trip,
reviews earnings, and reports incidents.

This harness uses **passenger** for the person being transported and **driver**
for the supply-side provider. It avoids using **rider** for both sides.

### Operator

The operator configures markets and service rules, observes live operations,
supports passengers and drivers, handles safety and fraud cases, corrects
exceptional outcomes, reconciles money, and preserves an audit trail.

Support, safety, finance, compliance, and platform administration are distinct
authorities even if an early demonstration presents them through one
application.

### Orchestration and Infrastructure

The platform coordinates identity and access, driver eligibility, live
availability, location intake, route and ETA providers, quoting, dispatch,
trip progression, realtime delivery, notifications, payments, driver earnings,
payouts, queues, storage, observability, and external-provider recovery and
reconciliation.

Infrastructure is included here because its failure modes directly change the
product. A delayed callback, duplicated acceptance, stale coordinate, or
missing notification is not an invisible implementation detail.

## Current Status

The [`PRD.md`](PRD.md) is a sourced, implementation-independent product
requirements baseline. Its detailed requirements are split by authority plane:

- [passenger and requester](prd/passenger.md);
- [driver](prd/driver.md);
- [marketplace and trip](prd/marketplace.md);
- [operations, safety, and compliance](prd/operations.md);
- [money](prd/money.md);
- [platform and reliability](prd/platform.md);
- [end-to-end acceptance and metrics](prd/acceptance.md); and
- [source ledger](prd/sources.md).

This directory currently contains:

- no Spock program;
- no proposed schema;
- no reference implementation;
- no chosen infrastructure architecture; and
- no claim that public sources replace market-specific domain review.

The language must grow to fit the product, not the reverse.

## Grounding Rules

Future work on this harness should:

- distinguish observed reference-product behavior from assumptions and product
  choices;
- separate universal marketplace constraints from jurisdiction-specific policy;
- identify who or what is authoritative for every consequential fact;
- preserve the difference between commands, observations, estimates,
  decisions, and settled outcomes;
- study cancellation, retries, disputes, degradation, and operator intervention
  alongside the happy path;
- treat external services as independent systems with explicit recovery and,
  where effects outlive a call, reconciliation—not as infallible function
  calls; and
- cite product documentation, regulation, provider contracts, operational
  evidence, or domain review before turning an outline item into a settled
  requirement.

## Intended Progression

1. Review the requirements with mobility, safety, accessibility, operations,
   payments, privacy, regulatory, and reliability expertise.
2. Choose and version one concrete launch-market profile without turning it
   into universal product truth.
3. Build conventional answer sheets that solve the product honestly.
4. Only then derive a readable Spock slice and record where the language does
   not fit.

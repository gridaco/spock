# Source Ledger

This ledger grounds the Uber-like mobility PRD. It was reviewed on 2026-07-17.

Sources are evidence for a requirement, not substitutes for product judgment.
Public product documentation establishes observable behavior, not private
architecture. A representative provider contract establishes an integration
constraint, not a vendor selection. A regulator or standard establishes scope
only where it legally or contractually applies.

## Evidence Classes

- **Product** — official public behavior from a mobility product.
- **Comparator** — official behavior from another mobility product, used to
  distinguish category needs from one company's choices.
- **Regulatory** — regulator or government material.
- **Standard** — published technical or accessibility standard.
- **Provider** — representative external-service contract.

## Product and Comparator Sources

<a id="s-prod-01"></a>
### S-PROD-01 — Uber passenger journey

- Class: Product.
- Source: [How Uber works](https://www.uber.com/us/en/about/how-does-uber-work/).
- Supports: destination and service selection, pickup confirmation, nearby
  driver acceptance, approach notification, party verification, trip,
  two-sided rating, and tipping.
- Does not prove: matching algorithm, internal states, assignment transaction,
  or universal market policy.

<a id="s-prod-02"></a>
### S-PROD-02 — Uber rider request and pickup

- Class: Product.
- Source: [How to use the Uber app](https://www.uber.com/us/en/ride/how-it-works/).
- Supports: account, route intent, pickup confirmation, nearby matching,
  approach tracking, driver/vehicle/license-plate verification, electronic
  payment, and post-trip rating.
- Does not prove: one global payment method, pricing rule, or pickup-verification
  policy.

<a id="s-prod-03"></a>
### S-PROD-03 — Uber driver application

- Class: Product.
- Source: [Uber Driver app guide](https://www.uber.com/us/en/drive/driver-app/).
- Supports: online availability, service preferences, time-bounded exclusive
  offers, multi-driver Trip Radar interest, authoritative match feedback,
  navigation to pickup, trip start, completion, earnings, safety, document
  status, and support.
- Does not prove: one dispatch strategy in every market or that driver
  acceptance alone creates the assignment.

<a id="s-prod-04"></a>
### S-PROD-04 — Uber driver eligibility and trip basics

- Class: Product.
- Source: [Driver app basics](https://www.uber.com/us/en/drive/basics/).
- Supports: approval before going online, vehicle- and market-dependent ride
  eligibility, offer information, trip formats, airport-specific rules,
  two-sided ratings, earnings visibility, and driver support.
- Does not prove: regulator-specific eligibility criteria or worker
  classification.

<a id="s-prod-05"></a>
### S-PROD-05 — Uber driver earnings

- Class: Product.
- Source: [Your earnings, explained](https://www.uber.com/us/en/drive/how-much-drivers-make/).
- Supports: market-dependent upfront or time-and-distance earnings, wait time,
  tolls, cancellation compensation, tips, adjustments, weekly statements,
  platform and government amounts, instant and scheduled payout.
- Does not prove: one universal fare formula, payout schedule, or accounting
  implementation.

<a id="s-prod-06"></a>
### S-PROD-06 — Uber cancellation review

- Class: Product.
- Source: [Review cancellation fee](https://help.uber.com/riders/article/dispute-my-cancellation-fee?nodeId=6bec690f-ee35-40ba-96ee-c38a8ae796e0).
- Supports: both parties may cancel; fee eligibility depends on market, service,
  elapsed time, driver progress, arrival/wait evidence, and policy; GPS is
  imperfect; authorization holds can outlive a canceled request; fees can be
  reviewed.
- Does not prove: the exact timer or fee for this PRD's launch market.

<a id="s-prod-07"></a>
### S-PROD-07 — Uber safety tools

- Class: Product.
- Source: [Uber safety commitment](https://www.uber.com/us/en/safety/our-commitment/).
- Supports: driver/vehicle details, GPS trip tracking, trusted-contact sharing,
  trip-anomaly detection, PIN verification, masked contact, recording options,
  emergency access, safety support, and privacy controls.
- Does not prove: effectiveness, universal availability, emergency-service
  integration, retention, or incident adjudication.

<a id="s-prod-08"></a>
### S-PROD-08 — Uber service-animal behavior

- Class: Product.
- Sources:
  [supporting riders with service animals](https://www.uber.com/us/en/newsroom/serviceanimals/)
  and [community guidelines](https://www.uber.com/us/en/safety/uber-community-guidelines/follow-law/).
- Supports: optional passenger accessibility disclosure, driver notification,
  policy enforcement, and the need to distinguish accessibility from ordinary
  ride preference.
- Does not prove: every jurisdiction's legal rule or accessible-vehicle supply.

<a id="s-prod-09"></a>
### S-PROD-09 — Uber Community Guidelines enforcement

- Class: Product.
- Source:
  [US and Canada Community Guidelines](https://www.uber.com/legal/en/document/?country=united-states&lang=en&name=general-community-guidelines).
- Supports: reports of potential violations, review by support or a specialized
  team, temporary account hold or inactivation during review, policy
  enforcement, and possible loss of platform access.
- Does not prove: private role design, investigation or evidence thresholds,
  universal notice or appeal, or that a report proves an incident.

<a id="s-prod-10"></a>
### S-PROD-10 — Uber driver deactivation review

- Class: Product.
- Source:
  [Deactivations: losing account access](https://www.uber.com/us/en/drive/driver-app/deactivation-review/).
- Supports: human involvement, advance notice when possible, additional
  evidence, review paths for many driver account decisions, and controls
  intended to keep abusive ratings or support reports out of account decisions.
- Does not prove: that every decision is appealable, that the same deadline or
  threshold applies everywhere, or the private separation of investigation,
  decision, and appeal roles.

<a id="s-prod-11"></a>
### S-PROD-11 — Uber US Safety Report methodology

- Class: Product.
- Source:
  [Uber's US Safety Reports](https://www.uber.com/us/en/about/reports/us-safety-report/).
- Supports: explicit serious-incident categories, declared reporting scope and
  publication cutoff, reports from riders and drivers, and the distinction
  between incidents reported to Uber and a determination that they occurred.
- Does not prove: a complete case taxonomy, an individual incident outcome,
  prevalence outside the report's scope, or an internal adjudication workflow.

<a id="s-prod-12"></a>
### S-PROD-12 — Uber rider privacy notice

- Class: Product.
- Source:
  [Privacy Notice: Riders and Order Recipients](https://www.uber.com/global/en/privacy-notice-riders-order-recipients/).
- Supports: distinct rider and guest subjects; account, identity, location,
  trip, payment, device, communications, recordings, support evidence, rating,
  automated-processing, sharing, retention, deletion, and rights categories;
  and jurisdiction-dependent practices.
- Does not prove: one universal legal basis, retention period, user right, or
  internal storage architecture for this PRD.

<a id="s-prod-13"></a>
### S-PROD-13 — Uber driver privacy notice

- Class: Product.
- Source:
  [Privacy Notice: Drivers and Delivery People](https://www.uber.com/global/en/privacy-notice-drivers-delivery-people/).
- Supports: distinct driver subjects; account, background, identity and
  biometric, vehicle, location, trip, earnings, communications, recordings,
  ratings, safety, fraud, verification, sharing, retention, deletion, and
  rights categories; and jurisdiction-dependent practices.
- Does not prove: that every described processing purpose is permitted in
  every market or that its retention rules should be copied into this PRD.

<a id="s-comp-01"></a>
### S-COMP-01 — Lyft ride request

- Class: Comparator.
- Source: [How to request a ride](https://help.lyft.com/hc/en-us/all/articles/115013079988-How-to-request-a-ride).
- Supports: destination, ride type, pickup confirmation, payment readiness,
  unavailable-supply outcome, pickup notes, requesting for another person, and
  accessibility-specific client paths.
- Does not add delegated rides, teens, multiple stops, or family payment to
  this PRD's initial scope.

<a id="s-comp-02"></a>
### S-COMP-02 — Lyft pickup

- Class: Comparator.
- Source: [How to get picked up](https://help.lyft.com/hc/en-us/all/articles/115013080908).
- Supports: driver name, photograph, rating, vehicle description, ETA,
  in-product contact, and arrival notification.
- Does not prove: arrival authority or notification delivery.

<a id="s-comp-03"></a>
### S-COMP-03 — Lyft cancellation and no-show

- Class: Comparator.
- Sources:
  [rider cancellation](https://help.lyft.com/hc/en-us/rider/articles/115012922687-Cancellation-policy-for-passengers)
  and [driver cancellation](https://help.lyft.com/hc/en-us/all/articles/115012922847-Cancel-and-no-show-fee-policy-for-drivers).
- Supports: separate rider and driver cancellation paths; time-, progress-,
  arrival-, contact-, and waiting-based fee evidence; pending authorization;
  dispute; region-specific rules; and driver cancellation reasons.
- Does not prove: that these thresholds should be copied.

<a id="s-comp-04"></a>
### S-COMP-04 — Lyft safety and location sharing

- Class: Comparator.
- Sources:
  [rider ride sharing](https://help.lyft.com/hc/en-us/all/articles/360051084234)
  and [driver location sharing](https://help.lyft.com/hc/en-us/all/articles/360037644574-Sharing-your-driving-location-with-friends-and-family).
- Supports: trusted-contact sharing, explicit sharing modes, viewer notice,
  different passenger and driver privacy boundaries, and approximate rather
  than exact third-party location.
- Does not prove: one required sharing design or emergency response contract.

<a id="s-comp-05"></a>
### S-COMP-05 — Lyft rider verification

- Class: Comparator.
- Source: [Rider verification for drivers](https://help.lyft.com/hc/en-us/all/articles/2771733102-Rider-verification-for-drivers).
- Supports: verification state distinct from account existence; progressive
  proofing; bounded driver-visible identity; optional verification; mismatch
  reporting; and nondiscrimination constraints.
- Does not require passenger identity proofing in the initial product.

<a id="s-comp-06"></a>
### S-COMP-06 — Lyft safety support

- Class: Comparator.
- Source: [Safety info for riders](https://help.lyft.com/hc/en-us/all/articles/7229653855-Safety-info-for-riders).
- Supports: in-trip safety entry, external emergency coordination, real-time
  location and vehicle information, opt-out implications, and explicit advice
  to call emergency services in an emergency.
- Does not make the platform itself an emergency service.

## Regulatory and Standard Sources

<a id="s-reg-01"></a>
### S-REG-01 — California TNC authority

- Class: Regulatory.
- Source: [California Public Utilities Commission TNC portal](https://www.cpuc.ca.gov/tncinfo).
- Supports: TNC permit, zero-tolerance, accessibility plan and driver training,
  insurance, driver-record, unaccompanied-minor, clean-miles, reporting, and
  access-program concerns are market-governed obligations.
- Applies: reference California market only.

<a id="s-reg-02"></a>
### S-REG-02 — California driver-record checks

- Class: Regulatory.
- Source: [CPUC DMV records check requirements](https://www.cpuc.ca.gov/regulatory-services/licensing/transportation-licensing-and-analysis-branch/transportation-network-companies/tnc-dmv-records-check-requirements).
- Supports: eligibility depends on dated external records, thresholds, and
  continuing notices; it is not a permanent account property.
- Applies: reference California market only; exact thresholds belong in its
  market profile.

<a id="s-reg-03"></a>
### S-REG-03 — California TNC insurance

- Class: Regulatory.
- Source: [CPUC insurance requirements](https://www.cpuc.ca.gov/regulatory-services/licensing/transportation-licensing-and-analysis-branch/transportation-network-companies/tnc-insurance-requirements).
- Supports: coverage depends on operating phase and may be maintained by driver,
  TNC, or both; evidence and effective dates matter.
- Applies: reference California market only.

<a id="s-reg-04"></a>
### S-REG-04 — California TNC reporting

- Class: Regulatory.
- Source: [CPUC required reports](https://www.cpuc.ca.gov/regulatory-services/licensing/transportation-licensing-and-analysis-branch/transportation-network-companies/required-reports-for-transportation-network-companies).
- Supports: regulator reporting is a first-class product obligation requiring
  preserved identities, trips, accessibility, zero-tolerance, and other
  reportable facts.
- Applies: reference California market only; schedules and fields must be
  separately verified before launch.

<a id="s-reg-05"></a>
### S-REG-05 — California privacy rights

- Class: Regulatory.
- Source: [California Consumer Privacy Act](https://oag.ca.gov/privacy/ccpa).
- Supports: rights to know, delete, correct, opt out where applicable, and limit
  certain uses of sensitive personal information; precise geolocation and
  account/payment credentials are sensitive categories.
- Applies: qualifying businesses and California residents; legal review decides
  exact applicability and exceptions.

<a id="s-reg-06"></a>
### S-REG-06 — Service animals

- Class: Regulatory.
- Source: [ADA.gov service animals](https://www.ada.gov/topics/service-animals/).
- Supports: disability-related service-animal access and nondiscrimination
  require explicit product and support handling.
- Applies: United States baseline; transport-specific counsel must confirm each
  market's complete obligations.

<a id="s-reg-07"></a>
### S-REG-07 — California zero-tolerance response

- Class: Regulatory.
- Source:
  [CPUC TNC Zero Tolerance Policy Information](https://www.cpuc.ca.gov/regulatory-services/licensing/transportation-licensing-and-analysis-branch/transportation-network-companies/tnc-zero-tolerance-policy-information).
- Supports: required public notice and complaint channels and prompt driver
  suspension after a drug- or alcohol-impairment complaint, pending further
  investigation.
- Applies: California TNCs only. The complaint-triggered suspension is an
  interim safeguard, not a finding; investigation and final-decision workflow
  still require California policy and domain review.

<a id="s-std-01"></a>
### S-STD-01 — Interface accessibility

- Class: Standard.
- Source: [WCAG 2.2](https://www.w3.org/TR/WCAG22/).
- Supports: testable perceivable, operable, understandable, robust, accessible
  authentication, error prevention, focus, target-size, and status-message
  requirements for web content across devices.
- Product decision: target WCAG 2.2 AA for passenger, driver, and operator web
  surfaces; native applications require equivalent platform testing.

<a id="s-std-02"></a>
### S-STD-02 — Authentication assurance

- Class: Standard.
- Source: [NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html).
- Supports: risk-based authentication assurance, multifactor authentication,
  phishing resistance, authenticator lifecycle, replay resistance, protected
  channels, and session controls.
- Product decision: privileged operator access uses stronger assurance than an
  ordinary passenger session.

<a id="s-std-03"></a>
### S-STD-03 — Payment-card protection

- Class: Standard.
- Sources:
  [PCI DSS](https://www.pcisecuritystandards.org/standards/pci-dss/) and
  [cardholder-data retention](https://www.pcisecuritystandards.org/faq/articles/Frequently_Asked_Question/What-is-the-maximum-period-of-time-that-cardholder-data-can-be-stored/).
- Supports: entities storing, processing, transmitting, or affecting card data
  have technical and operational obligations; sensitive authentication data
  cannot be retained after authorization; retained cardholder data needs a
  justified disposal policy.
- Does not prove: compliance scope; a qualified assessment must determine it.

## Representative Provider Sources

<a id="s-prov-01"></a>
### S-PROV-01 — Point-to-point routes

- Class: Provider.
- Source: [Google Routes: Compute Routes](https://developers.google.com/maps/documentation/routes/compute_route_directions).
- Supports: route results depend on origin, destination, travel mode, traffic
  preference, language, units, requested fields, and provider response; they
  include duration, distance, and geometry.
- Does not select Google or make a route result physical truth.

<a id="s-prov-02"></a>
### S-PROV-02 — Candidate route matrix

- Class: Provider.
- Source: [Google Routes: Compute Route Matrix](https://developers.google.com/maps/documentation/routes/reference/rest/v2/TopLevel/computeRouteMatrix).
- Supports: candidate-to-pickup travel estimates are time- and traffic-model
  dependent, bounded by provider limits, and may report fallback conditions.
- Does not prescribe dispatch ranking.

<a id="s-prov-03"></a>
### S-PROV-03 — Marketplace money movement

- Class: Provider.
- Source: [Stripe Connect marketplace tasks](https://docs.stripe.com/connect/marketplace/essential-tasks).
- Supports: connected-provider onboarding, passenger payment, platform fee,
  provider transfer, payout, refund, and dispute are separate responsibilities.
- Does not select Stripe or determine merchant-of-record and loss liability.

<a id="s-prov-04"></a>
### S-PROV-04 — Payment idempotency

- Class: Provider.
- Source: [Stripe idempotent requests](https://docs.stripe.com/api/idempotent_requests).
- Supports: network timeouts leave ambiguous outcomes; idempotency keys make
  retries safe only within a defined provider scope and retention window.
- Does not replace product-level idempotency or reconciliation.

<a id="s-prov-05"></a>
### S-PROV-05 — Payment webhook behavior

- Class: Provider.
- Source: [Stripe webhooks](https://docs.stripe.com/webhooks).
- Supports: automatic retries, duplicate events, non-guaranteed ordering,
  versioned payloads, signature verification, asynchronous handling, and
  explicit retrieval of missing provider state.
- Does not make webhook receipt the sole financial ledger.

<a id="s-prov-06"></a>
### S-PROV-06 — Marketplace disputes and payouts

- Class: Provider.
- Source: [Stripe marketplace guide](https://docs.stripe.com/connect/end-to-end-marketplace).
- Supports: settlement-merchant responsibility, disputes, refund authority,
  connected-account balances, and payout scheduling are distinct choices.
- Does not decide this product's legal funds flow.

<a id="s-prov-07"></a>
### S-PROV-07 — Push delivery

- Class: Provider.
- Source: [Firebase message lifespan](https://firebase.google.com/docs/cloud-messaging/customize-messages/setting-message-lifespan).
- Supports: provider acceptance is not device delivery; messages may wait,
  collapse, expire, be discarded, or target an invalid registration.
- Does not select Firebase. It proves push is a hint, not state authority.

## Evidence Gaps Requiring Domain Review

Public sources do not complete these areas:

- operator console workflows, market-policy governance, and internal separation
  of duty;
- safety adjudication thresholds and sexual misconduct, collision,
  law-enforcement, and emergency playbooks;
- fraud and account-takeover models;
- actuarial, coverage interpretation, insurer integration, and claim handling;
- merchant-of-record, money-transmission, tax, and driver-classification
  decisions;
- accessibility supply and service-level obligations by market;
- regulator-specific record formats and retention schedules;
- dispatch fairness, driver-distribution effects, and algorithmic transparency;
- production SLOs, regional failover, and support staffing; and
- exact legal authority for recording, sharing, and retaining trip evidence.

The PRD identifies requirements and launch gates for these areas but does not
pretend public documentation supplies the missing expertise.

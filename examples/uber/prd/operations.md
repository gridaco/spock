# Operator and Governance Requirements

This document specifies the human-operated control plane for marketplace and
trip intervention, support, safety, fraud and risk, eligibility, finance,
privacy, accessibility, claims, policy governance, and regulatory work. An
operator is an authenticated person acting through a bounded role; automation
acting on the same workflows remains separately attributable.

Grounding: [mobility safety tools](sources.md#s-prod-07),
[Community Guidelines enforcement](sources.md#s-prod-09),
[driver deactivation review](sources.md#s-prod-10),
[US safety-report methodology](sources.md#s-prod-11),
[rider privacy](sources.md#s-prod-12),
[driver privacy](sources.md#s-prod-13),
[California TNC authority](sources.md#s-reg-01),
[California reporting](sources.md#s-reg-04),
[California privacy rights](sources.md#s-reg-05),
[service-animal access](sources.md#s-reg-06),
[California zero-tolerance response](sources.md#s-reg-07),
[interface accessibility](sources.md#s-std-01), and
[authentication assurance](sources.md#s-std-02).

## Operator Identity, Authority, and Audit

### OPS-001 — Individual operator identity

Every operator action MUST be attributable to one authenticated person and one
active operator session. Shared operator accounts MUST NOT be permitted.
Automation MUST use a service identity and MUST NOT be recorded as a human
operator.

### OPS-002 — Bounded operator roles

The product MUST distinguish at least:

- general support;
- marketplace and trip operations;
- safety triage;
- safety investigation;
- safety decision;
- appeal review;
- fraud and risk review;
- driver and vehicle eligibility review;
- accessibility support;
- insurance and claims;
- finance, refund, and reconciliation operations;
- privacy operations;
- compliance and regulatory reporting;
- market-policy authoring;
- market-policy approval and activation;
- legal-response operations; and
- operator identity and access administration.

One person MAY hold more than one role, but each action MUST identify the role
under which it was authorized. Holding a role MUST NOT grant access outside the
markets, case types, and data purposes assigned to that operator.

Feedback and reputation moderation belongs to general support or fraud and
risk review according to the reason for intervention. It does not grant safety
investigation or safety-decision authority.

### OPS-003 — Least privilege and separation

Each role MUST receive the minimum data and actions needed for the current
case. In particular:

- general support MUST NOT receive raw historical location, identity
  documents, biometrics, or unrelated safety history by default;
- eligibility reviewers MUST NOT receive unrelated incident narratives;
- accessibility staff MUST see only operationally necessary access information
  and the related complaint;
- compliance staff MUST receive only the jurisdictional records needed for a
  submission; and
- access administrators MUST NOT gain mobility-record access merely because
  they administer roles.

An appeal reviewer MUST be independent of both the original investigator and
the original decision maker. The effective policy MUST identify which
high-impact safety decisions also require:

- investigator and decision-maker separation;
- two-person approval; or
- review by a specialist role.

Where policy permits an emergency exception, it MUST name the reason,
approving authority, scope, and expiry and MUST trigger prompt independent
review. An exception MUST NOT be available where applicable law or policy
prohibits it.

Basis: product decision; privileged access requires stronger assurance and
bounded sessions ([S-STD-02](sources.md#s-std-02)).

### OPS-004 — Privileged authentication

Operator authentication MUST use the assurance, multifactor, recovery, session,
and reauthentication controls assigned to the role's risk. A session used for
ordinary support MUST NOT silently become a legal-disclosure, privacy-deletion,
or account-deactivation session.

High-impact actions MUST require recent authentication. Recovery of an
operator account MUST revoke or review existing sessions and credentials.

### OPS-005 — Privileged activity record

The product MUST append an audit record for every sensitive read, search,
export, disclosure, safeguard, eligibility decision, account restriction,
financial correction, privacy action, regulatory submission, and role change.
The record MUST include:

- operator or service identity and role;
- authenticated session;
- action and target;
- case or approved purpose;
- market and jurisdiction;
- policy version;
- evidence references;
- time;
- result; and
- approval or break-glass context when applicable.

Audit access MUST itself be audited. An operator MUST NOT edit or delete their
own activity record.

### OPS-006 — Break-glass access

Emergency access beyond ordinary role scope MUST require an explicit reason,
defined duration, smallest practical scope, and prompt independent review. It
MUST NOT permanently expand the operator's role.

The system MUST alert the appropriate reviewer and preserve whether the access
was attempted, granted, used, expired, and reviewed.

## Bounded Marketplace and Trip Operations

### OPS-007 — Operator views are projections

Marketplace and trip operator views MUST be projections of authoritative
request, offer, assignment, and trip records. An operator interface MUST NOT
become a second writer that can directly edit lifecycle state.

The view MUST expose source revision, effective market policy, relevant
timeouts, and whether a displayed value is measured, provider-derived,
reported, inferred, or corrected.

### OPS-008 — Explicit intervention commands

A marketplace or trip operator MAY invoke only explicit commands allowed by
the effective market policy, including:

- pausing new quote, search, offer, or availability work;
- withdrawing an unaccepted offer;
- releasing or rematching an assignment before trip start;
- blocking new work while allowing an existing trip to finish safely; and
- initiating an approved safe-completion or early-termination workflow after
  trip start.

The initial release MUST NOT provide a human-dispatcher shortcut. An operator
MUST NOT manually select a driver, fabricate an acceptance or assignment, or
mark arrival, trip start, or completion without the corresponding authoritative
event or an append-only correction workflow.

### OPS-009 — Fenced intervention

Every marketplace or trip intervention MUST identify actor and role, market,
case or reason, target identity and revision, policy version, command,
idempotency identity, creation time, result, and any approval. A command against
a stale revision, replaced assignment, or incompatible trip phase MUST be
refused without changing authoritative state.

Operator intervention MUST be visible to affected workflows and MUST NOT
silently race an automated transition.

## Cases and Incident Truth

### OPS-010 — Durable case identity

Every support, marketplace intervention, safety, feedback-moderation,
accessibility, eligibility, fraud-and-risk, financial-reconciliation, privacy,
claim, legal, and regulatory matter MUST have one durable case identity, type,
subject, market, priority, status, owner, and timeline.

A case MAY reference multiple accounts or trips. It MUST NOT make those
references evidence that every subject participated in or caused the reported
event.

### OPS-011 — Report, signal, and finding

The product MUST distinguish:

- a person's report;
- an automated or provider signal;
- submitted evidence;
- an investigator's classification;
- an interim safeguard;
- a decision or finding;
- an appeal outcome; and
- a regulator, insurer, court, or law-enforcement outcome.

A report or anomaly MUST NOT become a confirmed incident, culpability finding,
or permanent account decision merely through ingestion or classification.

Basis: public safety behavior establishes reporting and support tools, not
incident adjudication. Public reporting also distinguishes reports received
from proven occurrences and depends on declared scope and methodology
([S-PROD-07](sources.md#s-prod-07),
[S-PROD-11](sources.md#s-prod-11)).

### OPS-012 — Original statement and amendment

The reporter's original statement, submission time, channel, language, and
declared relationship to the event MUST be preserved. A later clarification,
translation, correction, withdrawal, or operator note MUST be appended with
authorship and time; it MUST NOT replace the original.

The interface MUST make clear which text came from the reporter, an operator,
an automated classifier, or a translation service.

### OPS-013 — Classification and severity

Cases MUST support versioned classification and severity. A reclassification
MUST preserve the prior classification, reason, actor, evidence, and effective
time.

At minimum, safety operations MUST be able to distinguish collisions, unsafe
driving, physical assault, sexual assault or misconduct, threats or
harassment, discrimination, accessibility denial, suspected impairment,
identity mismatch, fraud, property damage, and lost property.

Classification determines routing and safeguards; it does not prove the
reported event.

### OPS-014 — Case-scoped search and disclosure

Operator search MUST be constrained by role, market, approved purpose, and case
scope. Searching by a high-risk identifier such as precise address, telephone
number, government identifier, or payment reference MUST require an allowed
purpose and MUST be audited.

Case participants MUST receive only disclosures allowed for their role and
phase. The product MUST NOT reveal a reporter's confidential contact data or
another party's unrelated history as an ordinary investigation convenience.

### OPS-015 — Feedback and reputation moderation

Passenger and driver ratings and feedback MUST remain two-sided source
submissions tied to their original trip, author, audience, and submission
time. An authorized support or fraud-and-risk operator MAY hide content from a
display, exclude a rating from a derived reputation score under policy, or
append a correction or abuse finding; the operator MUST NOT rewrite the
original submission.

Moderation MUST preserve reason, policy, evidence, actor, time, affected
derived values, and any notice or review path. A rating, low reputation score,
or moderation action is not a safety report or safety finding and MUST NOT
become one without a separately created and investigated safety case.

Basis: public mobility flows establish two-sided ratings, while the moderation
authority and append-only treatment are deliberate harness decisions
([S-PROD-01](sources.md#s-prod-01),
[S-PROD-04](sources.md#s-prod-04)).

## Safeguards, Investigation, Decision, and Appeal

### OPS-020 — Interim safeguard

An authorized operator or policy MAY apply a temporary safeguard before a
final finding when immediate risk, law, or market policy requires it. The
safeguard MUST record:

- subject and scope;
- permitted and blocked actions;
- reason category;
- triggering report, signal, or authority;
- start and review or expiry time;
- active-trip handling;
- policy version; and
- applying actor.

A temporary safeguard MUST NOT be displayed or analyzed as a permanent finding.
California's complaint-triggered zero-tolerance suspension is an example of an
interim response that remains distinct from the later investigation and
decision. It belongs only to the California market profile
([S-REG-07](sources.md#s-reg-07)).

### OPS-021 — Active-trip intervention

When a safeguard arises during an assignment or active trip, the operator
workflow MUST explicitly choose among blocking new offers, allowing safe
completion, directing the trip to stop, arranging support, or escalating to
emergency services.

The passenger and driver products MUST receive compatible instructions. A
background restriction MUST NOT leave the two parties in contradictory hidden
states.

### OPS-022 — Investigation

An investigation MUST preserve its questions, evidence requests, participant
contacts, material responses, failed contact attempts, conflicts, and open
uncertainties. Operators MUST be able to mark evidence unavailable or disputed
without inventing a value.

Contact with a participant MUST be suppressible when it may increase danger,
violate a legal restriction, or prejudice an investigation. The reason MUST be
recorded.

The investigator MAY recommend an outcome but, where separation is required,
MUST NOT record the final decision. Assignment, reassignment, recusal, and
conflict-of-interest declarations MUST remain in the case timeline.

### OPS-023 — Decision

A restrictive, restorative, or remedial decision MUST identify:

- decision type and affected subject;
- facts treated as established, disputed, or unknown;
- evidence relied upon;
- effective market, product, and time scope;
- policy and jurisdiction applied;
- deciding actor;
- required approvers and their decisions;
- notification limits;
- expiry or review condition; and
- available appeal or correction path.

The platform decision MUST remain distinct from an external source, screening
result, allegation, or regulator action.

A decision requiring two-person approval MUST remain pending until two eligible
actors have independently approved the same decision revision. A material
change to facts, evidence, scope, or remedy invalidates prior approval.

### OPS-024 — Notice

Affected parties MUST receive the decision category, practical effect,
effective time, permitted explanation, required next step, and available review
path. Information MAY be withheld only under a recorded safety, privacy, or
legal rule.

Notification delivery and reading MUST be tracked separately from decision
creation. A failed notification MUST NOT erase or silently postpone the
decision.

### OPS-025 — Appeal

An eligible decision MUST support a durable appeal case that can receive
submissions and evidence and end in affirmed, modified, overturned, withdrawn,
or unable-to-decide status. Market policy determines eligibility, deadlines,
and whether a later materially new submission can create another review
proceeding; the data model MUST NOT encode exactly one appeal opportunity.

The appeal MUST preserve the original decision and MUST NOT reopen it by
editing its fields. Its reviewer MUST be independent of the original
investigator and decision maker. The outcome MUST identify reviewer, evidence
considered, policy, reason, effective time, and resulting restorative or
corrective actions.

Public Uber materials support manual review, evidence submission, and review
paths for some driver deactivations; they do not establish a universal private
workflow or universal appeal right
([S-PROD-09](sources.md#s-prod-09),
[S-PROD-10](sources.md#s-prod-10)).

### OPS-026 — Safety domain gate

The separation, case states, and outcomes above are harness decisions for
testing the service boundary. Before launch, qualified safety and legal owners
MUST approve the market-specific:

- incident taxonomy and routing;
- evidence and decision thresholds;
- contact and disclosure rules;
- interim safeguards and expiry;
- decision and dual-approval matrix;
- notice and appeal eligibility; and
- staffing, escalation, emergency, and after-hours procedures.

Public product and regulatory sources MUST NOT be treated as proof that this
private adjudication workflow is complete.

## Evidence and Custody

### OPS-030 — Evidence identity and provenance

Every evidence item MUST preserve:

- stable identity and case;
- source and submitting actor;
- acquisition and alleged event times;
- original content reference and integrity value;
- media type and size;
- jurisdiction and consent caveat when known;
- sensitivity and permitted audience;
- retention class; and
- relationships to redacted, translated, transcribed, or analyzed derivatives.

A derivative MUST NOT replace the original or be presented as if it were the
original.

### OPS-031 — Evidence custody

Collection, access, export, transfer, redaction, disclosure, hold, and deletion
of evidence MUST append custody events naming actor, time, purpose, recipient,
and result.

Evidence needed for an active case MUST be protected from ordinary mutation.
An integrity failure MUST be visible and MUST NOT be repaired by silently
substituting another copy.

### OPS-032 — Scoped disclosure

A disclosure to an insurer, regulator, court, law-enforcement body, or other
third party MUST identify its authority, requester, case, subjects, fields,
trip range, time range, redactions, approving actor, transfer, and receipt.

The disclosure preview MUST make over-broad scope reviewable before release.
Disclosure MUST NOT grant the recipient ongoing access unless a separately
authorized product and policy expressly provides it.

### OPS-033 — Recording and consent uncertainty

Audio, video, biometric, and other high-risk evidence MUST retain the capture
and consent information available to the platform. If legality or consent is
unknown, the item MUST be restricted for review rather than treated as freely
usable.

An operator MUST NOT infer consent merely because a file was uploaded.

## Driver and Vehicle Eligibility Review

### OPS-040 — Reviewable requirement

Each driver, vehicle, screening, insurance, inspection, and market requirement
MUST be independently reviewable with subject, issuer or source, jurisdiction,
service class, evidence, effective period, status, reviewer, and policy.

Passing one requirement MUST NOT make the person, vehicle, or combination
eligible for every market or service class.

Basis: driver records and insurance are dated, external, and
jurisdiction-bound ([S-REG-02](sources.md#s-reg-02),
[S-REG-03](sources.md#s-reg-03)).

### OPS-041 — Source fact and platform decision

The product MUST distinguish submitted document, provider verification,
government or screening notice, reviewer interpretation, and platform
eligibility decision.

A correction to a source record MUST trigger reevaluation. It MUST NOT rewrite
what source and policy were used for a historical assignment.

### OPS-042 — Expiry and continuing change

Credential expiry, revocation, new screening notice, vehicle change, insurance
change, policy change, or safety restriction MUST create a reevaluation with an
effective time.

The operator MUST be able to block new availability without corrupting an
active-trip decision or historical eligibility record.

### OPS-043 — Adverse eligibility review

An adverse eligibility decision MUST expose the correction, resubmission,
dispute, and appeal paths available under the applicable market policy. The
operator MUST be able to receive corrected evidence without deleting the
original result.

Where an automated or biometric check cannot decide, the product MUST support
manual review and a suitable alternative path.

## Insurance Phase, Collision, and Claims

### OPS-044 — Event-time insurance phase

For a collision or other potentially covered event, the product MUST derive an
insurance-phase assessment from event time, driver availability, offer,
assignment, and trip state plus the market policy and coverage evidence
effective at that time. A current policy, credential, or corrected trip MUST
NOT silently rewrite the original assessment.

California's phase-dependent coverage is one jurisdictional example, not a
global phase model ([S-REG-03](sources.md#s-reg-03)). Every market profile MUST
define its own phase mapping, required evidence, and unresolved-state handling
before insurance-dependent service is enabled.

### OPS-045 — Distinct collision records

A collision report, safety case, insurance claim, financial adjustment, and
regulatory report MUST be distinct, durably linked records. Creating or closing
one MUST NOT imply that another exists, is accepted, or has the same outcome.

The event-time driver, vehicle, assignment, trip, location provenance, and
insurance evidence MUST remain available to authorized claim reviewers even if
the account, vehicle, credential, or market profile later changes.

### OPS-046 — Claim lifecycle and external truth

A claim MUST distinguish at least:

- reported and triaged;
- insurer notification pending, submitted, and acknowledged;
- evidence pending and under review;
- platform decision, when the platform has decision authority;
- insurer decision, paid, or denied, only when supported by insurer evidence;
- closed; and
- reopened.

Provider timeout or an ambiguous response MUST remain unknown and
reconcilable. An operator MUST NOT fabricate acknowledgement, coverage,
liability, payment, or denial, and MUST NOT adjudicate an external insurer's
decision.

### OPS-047 — Claims disclosure and custody

A claims disclosure MUST be limited to the claim purpose, recipient, covered
subjects, fields, and time range and MUST follow OPS-032. The platform MUST
preserve the submitted package, redactions, authority, delivery result,
acknowledgement, and later supplements.

Evidence sent to an insurer remains linked to its original and custody history.
An insurer request MUST NOT grant standing access to unrelated trips or
accounts.

### OPS-048 — Claim correction and appeal

A platform-side claim correction or review MUST append to, not replace, the
prior assessment. It MUST identify whether the change affects only platform
records or also requires a new insurer notice, financial adjustment, user
notice, or regulatory correction.

Where a market or coverage contract provides a dispute or appeal path, the
claim interface MUST preserve its deadline, submission, evidence, actor,
external acknowledgement, and outcome without representing that path as
universal.

## Privacy, Retention, Holds, and Deletion

### OPS-050 — Purpose-limited operator view

Operator views MUST minimize precise location, home and destination addresses,
communications, government identifiers, payment data, biometrics,
accessibility information, and incident evidence according to current case
purpose.

Unmasking MUST require an allowed reason and MUST append an access record.

### OPS-051 — Retention classes

The product MUST assign separate, jurisdiction-aware retention rules to at
least:

- account and identity data;
- driver and vehicle credentials;
- location observations and route summaries;
- trip and financial records;
- communications and support cases;
- safety and accessibility evidence;
- biometrics and recordings;
- operator audit records; and
- regulatory submissions.

One case or account lifetime MUST NOT become the default retention period for
every related field.

Basis: deletion, correction, and limits on sensitive-data use have scoped
rights and exceptions. Public notices also show that rider and driver
processing differs by subject, purpose, data category, and jurisdiction
([S-REG-05](sources.md#s-reg-05),
[S-PROD-12](sources.md#s-prod-12),
[S-PROD-13](sources.md#s-prod-13)).

### OPS-052 — Preservation hold

An authorized safety, claim, regulatory, or legal hold MUST identify authority,
case, subjects, fields, time range, reason, creator, start, review or expiry,
and release. A hold blocks only the covered deletion; it MUST NOT make retained
data generally visible or usable for unrelated purposes.

Release of a hold MUST resume the ordinary retention decision and MUST NOT
delete data whose independent retention rule still applies.

### OPS-053 — Privacy request and deletion

A privacy request MUST produce field- or category-level outcomes of corrected,
provided, deleted, retained, restricted, or denied, with authority, reason, and
review path.

Account deletion MUST NOT silently destroy held evidence or regulated records.
Conversely, an unresolved case MUST NOT justify indefinite retention of every
field in the account.

### OPS-054 — Conflicting obligations

When user rights, safety needs, litigation, insurance, and regulatory
requirements conflict, the operator workflow MUST preserve the applicable
jurisdiction, decision maker, policy, affected fields, outcome, and next review
time. The product MUST NOT resolve the conflict through an undocumented global
exception.

## Accessibility Operations

### OPS-060 — Accessible operator surface

Operator authentication, search, case handling, evidence review, decisions,
and emergency controls MUST meet the product's accessibility target. A
time-critical action MUST NOT depend only on color, map position, pointer
precision, sound, or an expiring visual prompt.

Basis: product target [WCAG 2.2 AA](sources.md#s-std-01).

### OPS-061 — Accessibility case

Accessibility denial, service-animal concern, assistive-device concern,
wheelchair-accessible service failure, and accessibility surcharge MUST be
first-class case categories. The case MUST retain the related request, offer,
assignment, cancellation, fare, driver response, support action, and remedy.

Operational access instructions MUST remain distinct from a diagnosis or
general health profile.

For the initial standard ride service, the product deliberately MUST NOT add a
surcharge or eligibility penalty solely because a passenger has a service
animal, assistive device, or operational accessibility instruction. This is a
product rule grounded in public product behavior and a qualified United States
regulatory baseline, subject to market-specific legal review; it is not a
claim about every market's law
([S-PROD-08](sources.md#s-prod-08),
[S-REG-06](sources.md#s-reg-06)).

### OPS-062 — Accessibility outcome review

Operators MUST be able to review whether access needs affected estimated or
actual wait, matching, driver cancellation, completion, fare, refund, rating,
or account action. An accessibility complaint MUST NOT be reduced to an
ordinary low rating.

Any market-specific accessible-service or reporting obligation MUST be
evaluated from that market's effective policy
([S-REG-01](sources.md#s-reg-01)).

Wheelchair-accessible vehicle service, supply targets, response times, fares,
subsidies, and reporting MUST apply only where that service is enabled or
required by the effective market profile. The standard-service no-surcharge
rule MUST NOT be misrepresented as proof that every market must launch a
wheelchair-accessible fleet product.

### OPS-063 — Protected instruction disclosure

Vehicle and service capability constraints needed to form a valid offer MAY be
evaluated or disclosed before a driver responds. A passenger's protected
operational accessibility instructions or service-animal information MUST be
disclosed to the driver only after assignment, unless a reviewed market rule
requires an earlier, purpose-limited phase.

The driver MUST receive only the instruction needed to perform the trip, not a
diagnosis or general health profile. Every earlier disclosure rule MUST record
its purpose, fields, audience, market, and approval.

## Finance, Refund, Reconciliation, and Fraud Operations

### OPS-064 — Bounded financial authority

An operator MUST NOT directly edit a posted fare, earning, transfer, payout,
refund, dispute, or ledger entry. Finance actions MUST be explicit,
idempotent commands that identify amount and currency, reason, policy,
evidence, financial targets, downstream artifacts, and required approval.

Role and amount thresholds MUST determine who can propose, approve, and
execute a financial action. Ordinary support MAY open a case or propose a
bounded remedy but MUST NOT approve an amount beyond its assigned authority.

### OPS-065 — Refund and adjustment lifecycle

The product MUST distinguish:

- refund or adjustment request;
- policy evaluation and internal approval;
- provider submission;
- provider acknowledgement and final outcome;
- passenger balance or payment effect;
- driver earning or payout effect; and
- receipt, statement, tax, and dispute consequences.

One stage MUST NOT imply another. A provider timeout, duplicate event, or
unknown result MUST remain reconcilable rather than becoming a fabricated
success or a blind retry
([S-PROV-03](sources.md#s-prov-03),
[S-PROV-04](sources.md#s-prov-04),
[S-PROV-05](sources.md#s-prov-05)).

### OPS-066 — Reconciliation

Each discrepancy MUST have a durable identity, financial references, internal
and provider observations, expected and observed amounts, currency, severity,
age, owner, status, and resolution. The operator MUST be able to record that
the provider outcome is unknown or evidence conflicts.

Reconciliation MUST repair state through provider operations and immutable
adjustment or correction entries. It MUST NOT fabricate a provider event,
overwrite a posted ledger entry, or silently force internal and external
balances to agree.

### OPS-067 — Fraud and risk review

The product MUST distinguish a fraud signal, risk score, temporary hold,
investigator conclusion, financial dispute, safety matter, and account
decision. An automated signal MAY trigger a policy-bounded safeguard; it MUST
NOT become a fraud finding or safety finding by ingestion alone.

Fraud and risk operators MUST receive only the linked accounts, trips, device
signals, payment references, and evidence needed for the case. Cross-account
linkage, identity inference, and a decision affecting access or money MUST be
audited and MUST expose the notice, correction, or appeal path provided by the
effective market policy.

## Regulatory Reporting and Corrections

### OPS-070 — Reporting obligation

Each regulatory obligation MUST identify authority, jurisdiction, report type,
covered period, due time, schema version, required records, retention rule,
submission channel, and responsible role.

The product MUST NOT assume one regulator, annual schedule, or global trip
definition.

Basis: California requires multiple distinct TNC reports
([S-REG-04](sources.md#s-reg-04)).

### OPS-071 — Reproducible submission

A regulatory submission MUST be reproducible from a recorded cutoff and
effective policy. It MUST preserve:

- included record identities and query or rule version;
- schema and generation version;
- generation and approval actors;
- validation result;
- payload integrity value;
- submission attempt and external acknowledgement; and
- rejection or partial-acceptance detail.

Retrying an ambiguous submission MUST NOT create an untraceable duplicate.

### OPS-072 — Regulatory correction

A correction MUST reference the original submission, identify changed records
and reasons, and preserve both versions and acknowledgements. Where the
regulator exposes a correction mechanism, the product MUST use it; otherwise
it MUST use the approved jurisdiction-specific resubmission or notice path.

An operator MUST NOT make a past report appear correct by changing the stored
payload in place. The platform MUST NOT assume that a regulator supports
idempotency, replacement, or supersession unless the applicable reporting
contract establishes it.

### OPS-073 — Effective jurisdiction and policy

Compliance evaluation MUST use the jurisdiction, authority, policy, and schema
effective for the event or reporting period. A later rule change MUST NOT
silently recalculate historical compliance without identifying the new
evaluation basis.

## Append-Only Operator Corrections

### OPS-080 — Correction instead of overwrite

An operator correction to trip state, route intent, assignment, arrival,
completion, fare, earning, eligibility, case classification, or decision MUST
append a correction that references the prior fact. It MUST preserve:

- corrected fact and prior value;
- replacement value or outcome;
- reason;
- evidence;
- operator and authority;
- policy and jurisdiction;
- creation and effective times; and
- affected downstream records.

Marketplace and trip corrections MUST obey the correction classes, assignment
fences, terminality, and invariants in `MKT-055` and `MKT-060`. They MUST NOT
assign or substitute a driver or vehicle, reopen a terminal trip, impersonate a
passenger or driver command, or bypass normal release, rematch, termination, or
exceptional-resolution authority.

The original fact MUST remain auditable.

### OPS-081 — Corrective consequences

A correction that changes money, receipt, statement, eligibility,
notification, privacy outcome, or regulatory reporting MUST create the
corresponding adjustment, revision, reevaluation, notification, or corrected
submission. It MUST NOT mutate an already issued artifact without a new
revision.

### OPS-082 — Conflict and reversal

Competing corrections MUST be ordered and reviewable. Reversing a correction
MUST append another correction; deletion or status rollback MUST NOT be used to
erase the intervening history.

If the product cannot safely apply a correction, it MUST leave the authoritative
record unchanged, create an action-required outcome, and preserve the failed
attempt.

## Market-Profile Governance

### OPS-090 — Versioned policy artifact

Every market profile MUST be an immutable, versioned artifact with market,
jurisdictions, service classes, effective interval, author, source and legal
provenance, dependency versions, change summary, and semantic diff from its
predecessor.

An operator MUST NOT edit an approved, active, or retired version. A correction
or rollback MUST create a new candidate version.

### OPS-091 — Governed lifecycle

A market-profile version MUST move only through:

`draft -> validated -> approved -> active -> retired`

The system MUST refuse skipped, reversed, or ambiguous transitions. Validation
MUST check schema, completeness, jurisdiction, service scope, effective
interval, references and dependencies, required evidence, source currency, and
approval rules. Validation success MUST NOT itself approve or activate the
profile.

### OPS-092 — Author, approver, and activator

Every transition MUST identify actor, role, time, source revision, reason, and
result. The effective governance policy MUST state:

- who may author, validate, approve, and activate;
- that author and approver must be different eligible people;
- when activation needs a second approver; and
- which legal, safety, finance, accessibility, insurance, or regulatory domain
  owners must sign off.

An actor MUST NOT self-approve or bypass required domain approval. If one
person holds multiple roles, each action remains separately authorized and
audited.

### OPS-093 — Activation fence

Activation MUST be atomic and fenced against the currently active profile.
For one market, service class, and effective instant, at most one applicable
profile version may be active. The system MUST reject overlapping active
intervals, an unexpected predecessor, an unapproved dependency, or activation
against a stale revision.

Quotes, offers, assignments, trips, financial decisions, and operator actions
already created under an older profile MUST retain that version. Activation
MUST NOT silently reinterpret in-flight or historical records.

### OPS-094 — Superseding rollback

Rollback MUST create, validate, approve, and activate a new superseding version
that restores or adapts prior behavior. It MUST identify the defective version,
reason, affected interval, migration or compensation plan, and records needing
reevaluation.

Retirement or rollback MUST NOT mutate the content, effective history, or
provenance of the version being replaced.

### OPS-095 — Emergency market control

An authorized operator MAY apply only a predefined, policy-bounded emergency
control to pause or limit new quotes, searches, offers, assignments, or driver
availability for a declared market and service scope. The control MUST record:

- triggering event and reason;
- actor, role, and required approval;
- affected market, geography, service, and action;
- start, review, and mandatory expiry;
- treatment of open requests and unaccepted offers;
- safe-completion or termination rule for assigned or active trips; and
- passenger, driver, operator, and regulator communication duties.

An emergency control MUST NOT silently rewrite the market profile, create a
permanent policy exception, or expire without an observable state transition.
It MUST receive prompt retrospective review and either close or proceed through
the ordinary profile lifecycle.

### OPS-096 — Profile audit and reproducibility

For any quote, assignment, trip, financial action, case decision, claim, or
regulatory submission, an authorized reviewer MUST be able to retrieve the
exact market-profile version and emergency controls that governed it.

The audit MUST distinguish policy as authored, approval and activation events,
runtime evaluation inputs, evaluation result, and any later correction. A
current profile MUST NOT be substituted when reproducing a historical
decision.

## Operator Acceptance Scenarios

The release suite MUST prove:

1. a support operator cannot inspect raw historical location without an
   allowed case purpose and audited unmask action;
2. a marketplace operator cannot fabricate driver acceptance, assignment,
   arrival, trip start, or completion;
3. a stale operator command against a replaced assignment is refused without
   changing authoritative trip state;
4. a safety triager can apply a temporary safeguard but cannot record it as a
   permanent finding;
5. a report, anomaly signal, classification, decision, and appeal remain
   independently queryable;
6. a reporter clarification appends to rather than replaces the original
   statement;
7. a decision configured for investigator separation and two-person approval
   cannot be finalized by its investigator or one approver;
8. an independent appeal can overturn a decision while preserving the original
   decision and evidence;
9. an active-trip safeguard gives passenger and driver compatible instructions;
10. original two-sided ratings survive moderation, a derived reputation score
    is reproducible, and neither becomes a safety finding;
11. evidence export preserves custody, recipient, scope, and integrity;
12. an expired driver or vehicle credential blocks new availability without
   rewriting historical trip eligibility;
13. corrected screening evidence triggers reevaluation without deleting the
   source result;
14. a collision in each configured insurance phase uses event-time profile and
    trip evidence rather than current account or credential state;
15. a collision report, safety case, claim, adjustment, and regulator report
    remain distinct, and an ambiguous insurer response cannot become a
    fabricated acknowledgement or decision;
16. account deletion preserves only fields under an applicable retention rule
    or scoped hold and explains each retained category;
17. releasing one hold does not delete data covered by another retention rule;
18. the initial standard service does not surcharge solely for a service
    animal, assistive device, or operational accessibility instruction;
19. vehicle capability can constrain an offer before response while protected
    accessibility instructions are withheld until assignment, and a
    wheelchair-accessible service obligation appears only under an applicable
    market profile;
20. refund approval, provider submission, provider outcome, passenger refund,
    driver adjustment, receipt, and statement revision remain distinct;
21. reconciliation preserves an unknown provider outcome and resolves it with
    provider evidence and immutable adjustments rather than overwritten
    entries;
22. a fraud signal can create a bounded hold but cannot become a fraud or
    safety finding without its respective decision;
23. a repeated regulatory submission is attributable and does not silently
    duplicate an accepted report;
24. a regulatory correction preserves its original and uses the configured
    regulator correction, resubmission, or notice path without assuming
    supersession;
25. only a validated and approved market profile can activate, self-approval,
    overlapping profiles, and stale activation fences are refused, and
    in-flight work keeps its original profile version;
26. rollback creates a new approved superseding profile instead of mutating an
    active or retired version;
27. an emergency market control expires observably, does not rewrite policy,
    and applies the configured safe treatment to active trips;
28. a fare or trip correction creates downstream receipt and statement
    revisions; and
29. an operator cannot edit or delete the audit record of their own sensitive
    read or decision.

# Examples

This directory is for current-valid Spock examples only.

Examples here should reflect the language surface that Spock is actively trying
to build now:

- `model` for persistent data
- `view` for public projections
- `fn` for RPC-style backend operations

Do not use proposal-only concepts here. Future-facing ideas belong in
`docs/rfd/`.

## Scenario Tracks

Spock examples should be grounded in real application engineering problems.
Good example tracks include:

- `reddit` - communities, posts, comments, voting, moderation workflows
- `commerce` - products, carts, orders, payments, fulfillment state
- `saas` - workspaces, projects, members, invitations, billing-facing records
- `marketplace` - sellers, listings, buyers, offers, reviews
- `support` - customers, tickets, assignments, status transitions
- `cms` - authors, articles, drafts, publishing, public content views

Each scenario should stay small enough to read quickly, but real enough to show
why `model`, `view`, and `fn` belong together.

## Example Rules

An example should:

- use only accepted Spock syntax
- model a concrete application scenario
- include public views for the data that leaves the backend
- include functions for operations that would be called over RPC
- avoid speculative authorization, effects, traits, decorators, or test syntax

If an example needs a language feature that does not exist yet, write an RFD
instead of adding the feature here.

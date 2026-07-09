# Instagram PRD

## Purpose

Instagram is a social photo and video sharing application where people publish
visual posts, follow other people, browse a personalized feed, and interact
through likes and comments.

This PRD covers the main product flows only. It excludes direct messages,
stories, reels, live video, shopping, ads, creator monetization, and advanced
recommendation systems.

## Goals

- Let users create and maintain a public profile.
- Let users sign in, sign out, recover access, and deactivate or delete accounts.
- Let users publish image or video posts with optional captions.
- Let users follow and unfollow other users.
- Let users browse posts from accounts they follow.
- Let users view public profiles and post detail pages.
- Let users like and unlike posts.
- Let users comment on posts and delete their own comments.
- Let users reply to comments.
- Let users mention profiles in captions and comments.
- Let users tag profiles on post media.
- Let users save posts.
- Let users report posts, comments, and profiles.
- Let users block, restrict, and manage comment visibility.
- Let users search for profiles.
- Let users discover public posts by hashtags and locations.
- Let users receive activity notifications for important interactions.
- Keep the product understandable, fast, and safe at consumer social scale.

## Non-Goals

- Direct messaging.
- Stories or ephemeral content.
- Reels.
- Livestreaming.
- Paid promotion or advertising.
- Shopping and checkout.
- Creator analytics or monetization.
- Complex recommendation ranking beyond a simple home feed, profile search,
  hashtag pages, and location pages.
- Multi-account management.

## Users

### Visitor

A visitor is not signed in. Visitors may view public profiles and public posts
if the product allows logged-out browsing, but cannot create posts, follow,
like, or comment.

### User

A user has an account, a profile, and a unique username. Users can publish
posts, follow others, like posts, and comment.

### Moderator or Operator

An internal operator can remove abusive content, disable accounts, and respond
to reports. The operator workflow is not a primary product flow, but the product
must preserve enough data and state to support moderation.

## Core Entities

### Account

Represents a registered user identity.

Important fields:

- id
- email or phone
- password or external authentication identity
- status
- created time

### Profile

Represents the public identity displayed in the app.

Important fields:

- id
- account id
- username
- display name
- bio
- avatar image
- profile visibility
- tag and mention permissions
- comment permissions
- follower count
- following count
- post count

Usernames must be unique, stable enough for links, and validated for allowed
characters and length.

Private profiles require approval before another user can follow them. Only
approved followers can see, like, or comment on private profile posts.

### Post

Represents published visual content.

Important fields:

- id
- author profile id
- media assets
- caption
- created time
- updated time
- visibility
- location
- like count
- comment count
- save count
- moderation status
- archive state
- deletion state

A post may contain one or more media assets. A carousel is treated as a single
post with ordered media items.

### Media Asset

Represents an uploaded image or video file and its processed variants.

Important fields:

- id
- original file reference
- media type
- width
- height
- duration for video
- processed variants
- processing status

### Follow

Represents one profile following another profile.

Important fields:

- follower profile id
- followed profile id
- created time

A user cannot follow the same profile twice. A user cannot follow themselves.

### Like

Represents one profile liking one post.

Important fields:

- profile id
- post id
- created time

A user can like a post at most once.

### Comment

Represents a comment on a post.

Important fields:

- id
- post id
- author profile id
- body
- created time
- deletion state
- moderation status

Comment replies may be represented as comments linked to a parent comment. A
reply should preserve the author being replied to, even if the visible text also
contains an `@username` mention.

### Mention

Represents a profile mention inside a caption or comment.

Important fields:

- id
- source type
- source id
- mentioned profile id
- mention text
- created time

A mention should point to a profile identity, not only store raw username text.
The displayed text may preserve what the author typed, but the target should
remain stable if the mentioned user later changes username.

### Media Tag

Represents a profile tagged on a post media item.

Important fields:

- id
- post id
- media asset id
- tagged profile id
- tag text
- position
- created time
- removed time

Media tags are different from caption or comment mentions. A tagged profile may
appear on the post and in the tagged profile's tagged-posts surface, subject to
privacy, moderation, and the tagged user's tag settings.

### Save

Represents a user saving a post for later.

Important fields:

- profile id
- post id
- collection id
- created time

A user can save a post at most once per collection. Saved posts are private to
the saving user unless a collaborative collection feature is explicitly added.

### Collection

Represents an optional grouping of saved posts.

Important fields:

- id
- owner profile id
- name
- cover post id
- created time
- updated time

Collaborative collections are out of scope for the first version.

### Follow Request

Represents a request to follow a private profile.

Important fields:

- requester profile id
- target profile id
- status
- created time
- responded time

A private profile owner can approve or deny requests. Approved requests create
or activate a follow relationship.

### Block

Represents one profile blocking another profile.

Important fields:

- blocker profile id
- blocked profile id
- created time

Blocked profiles should not be able to follow, like, comment, mention, tag, or
view restricted surfaces of the blocker according to product policy.

### Restriction

Represents one profile restricting another profile.

Important fields:

- restricting profile id
- restricted profile id
- created time

Restricted interactions may be hidden or require approval without fully blocking
the other profile.

### Report

Represents a user report against content or a profile.

Important fields:

- id
- reporter profile id
- target type
- target id
- reason
- created time
- review status

Reports support moderation review and may result in removal, reduced
distribution, account action, or no action.

### Hashtag

Represents a hashtag extracted from a caption or comment.

Important fields:

- tag
- normalized tag
- created time

Public posts with hashtags may appear in hashtag search results. Private posts
must not become public because they include a hashtag.

### Location

Represents a location attached to a post.

Important fields:

- id
- name
- external place id
- created time

Only supported existing locations can be tagged in posts. Creating new locations
is out of scope.

### Notification

Represents an activity item delivered to a user.

Important fields:

- id
- recipient profile id
- actor profile id
- activity type
- source type
- source id
- created time
- read time

Notifications may be generated for follows, follow request decisions, likes,
comments, replies, mentions, media tags, and report outcomes.

## Main Flows

### Sign Up

1. A visitor creates an account using email, phone, or an external identity.
2. The user chooses a unique username.
3. The user may add a display name, bio, and avatar.
4. The product creates an account and profile.
5. The user lands on an empty home feed or profile setup screen.

Acceptance criteria:

- A username cannot be used by more than one active profile.
- Invalid usernames are rejected.
- Duplicate sign-up attempts should not create duplicate accounts.
- A newly created profile can be viewed by its owner immediately.

### Sign In and Sign Out

1. A registered user signs in using a supported authentication method.
2. The product creates an authenticated session.
3. The user can sign out and end the session.

Acceptance criteria:

- Disabled or deleted accounts cannot create normal sessions.
- Authentication failures do not reveal whether an email, phone, or username
  belongs to an account beyond product policy.
- Signing out invalidates the active session for that client.

### Recover Account Access

1. A user requests help accessing their account.
2. The product verifies control of an email, phone, or external identity.
3. The product lets the user restore access or reset credentials.

Acceptance criteria:

- Recovery flows are rate limited.
- Recovery does not allow takeover of another profile.
- Recently changed account identifiers may require additional verification.

### Edit Profile

1. A signed-in user opens profile settings.
2. The user edits display name, bio, avatar, visibility, tag permissions,
   mention permissions, or comment permissions.
3. The product validates and saves the changes.
4. The updated profile is visible on the profile page.

Acceptance criteria:

- Username changes, if allowed, must preserve uniqueness.
- Avatar uploads must be processed before being shown as final.
- Profile counters are not manually editable.
- Making a profile private affects future visibility and follow behavior.
- Tag, mention, and comment permissions must be enforced by compose,
  autocomplete, submission, and read surfaces.
- Username changes must preserve existing mention and tag references.

### Deactivate or Delete Account

1. A signed-in user requests account deactivation or deletion.
2. The product explains what will happen to profile, posts, comments, likes,
   saves, follows, tags, mentions, and username availability.
3. The user confirms the action.
4. The account becomes disabled immediately or enters a deletion grace period
   according to product policy.

Acceptance criteria:

- Disabled accounts do not appear in normal search, autocomplete, feed, or
  discovery surfaces.
- Deleted accounts eventually remove or anonymize user-facing content according
  to policy.
- Account deletion must not corrupt counters or expose private content.
- Username reuse policy must be explicit.

### Manage Private Profile Follow Requests

1. A private profile receives a follow request.
2. The profile owner opens pending follow requests.
3. The profile owner approves or denies the request.
4. If approved, the requester becomes a follower.
5. If denied, the requester does not gain access to private content.

Acceptance criteria:

- Public profiles do not require follow request approval.
- Private profiles require approval before private content is visible.
- Approving a request is idempotent and creates at most one follow relationship.
- Denying a request is idempotent and must not create a follow relationship.
- Requests from blocked profiles are rejected or hidden according to product
  policy.

### Create Post

1. A signed-in user selects one or more images or videos.
2. The product uploads the media.
3. The product processes media into supported display variants.
4. The user adds an optional caption, location, media tags, and hashtags.
5. The user publishes the post.
6. The post appears on the user's profile and may appear in follower feeds,
   hashtag pages, or location pages according to visibility rules.

Acceptance criteria:

- A post cannot be published without at least one valid media asset.
- Media processing failures must leave the post unpublished or clearly failed.
- The author can view the post after publishing.
- The profile post count eventually reflects the new post.
- Mentions in the caption are parsed and linked to mentioned profiles.
- Hashtags in the caption are parsed and linked to hashtag results.
- Media tags are linked to tagged profiles only when allowed by tag settings.
- Private posts must not become public because they include hashtags, locations,
  mentions, or media tags.

### Edit Post

1. A post author opens an existing post.
2. The author edits the caption, location, or media tags.
3. The product validates and saves the changes.
4. Updated text, mentions, hashtags, locations, and tags are reflected on read
   surfaces.

Acceptance criteria:

- Editing media files after publish is out of scope.
- Caption edits re-parse mentions and hashtags.
- Tag edits must re-check tag permissions.
- Updates should not reset likes, comments, saves, or original created time.

### Archive Post

1. A post author archives one of their posts.
2. The post is removed from normal public profile, feed, hashtag, and location
   surfaces.
3. The author can still view the archived post.
4. Likes and comments are preserved.

Acceptance criteria:

- Archived posts do not count as publicly visible posts.
- Unarchiving restores the post to eligible surfaces.
- Archive and unarchive operations are idempotent.

### Delete Post

1. A post author deletes one of their posts.
2. The post is hidden from normal user surfaces.
3. Associated comments, likes, saves, tags, mentions, and feed references stop
   appearing to normal users.
4. The product retains enough state for recovery, audit, moderation, and counter
   repair according to retention policy.

Acceptance criteria:

- Delete is idempotent.
- Deleted posts do not appear in feeds, profiles, hashtag pages, location pages,
  search, saved posts, or tagged-post surfaces.
- The product may support a recently deleted recovery window.
- Permanent deletion after the recovery window is a background lifecycle event.

### View Home Feed

1. A signed-in user opens the home feed.
2. The product returns posts from followed profiles.
3. Each feed item shows author summary, media preview, caption preview, like
   count, comment count, and whether the viewer has liked the post.
4. The user can paginate through older feed items.

Acceptance criteria:

- Feed ordering should be deterministic for a given pagination cursor.
- Deleted or moderated posts must not appear.
- Private posts must not appear unless the viewer is allowed to see them.
- Feed reads must remain fast as the number of posts and follows grows.

### View Profile

1. A user opens a profile by username or id.
2. The product returns profile summary, counters, and a paginated post grid.
3. The viewer can open a post from the grid.

Acceptance criteria:

- Public profiles are visible to allowed viewers.
- Deleted or moderated posts are excluded from the grid.
- Counters should be displayed consistently enough for user trust, but do not
  need to be transactionally perfect at all times.

### View Post Detail

1. A user opens a post.
2. The product returns full media, author summary, caption, counts, viewer like
   state, and recent comments.
3. The user can paginate comments.

Acceptance criteria:

- The post detail must not expose internal moderation or storage fields.
- Deleted comments are hidden or shown as deleted according to product policy.
- Comment pagination must remain stable while new comments are added.

### Follow User

1. A signed-in user opens another profile.
2. The user taps follow.
3. If the target profile is public, the product creates the follow relationship.
4. If the target profile is private, the product creates a pending follow
   request.
5. The target profile's follower count, request count, and viewer follow state
   update according to the resulting state.
6. Future posts from followed public profiles or approved private profiles may
   appear in the follower's home feed.

Acceptance criteria:

- Following is idempotent.
- A user cannot follow themselves.
- Follower and following counts are updated without requiring expensive reads.
- The home feed may update asynchronously.
- Following a private profile does not grant access until approved.
- A blocked profile cannot follow or request to follow the blocker.

### Unfollow User

1. A signed-in user opens a followed profile.
2. The user taps unfollow.
3. The product removes or deactivates the follow relationship.
4. The target profile's follower count and viewer follow state update.
5. Future posts from the unfollowed profile no longer appear in the home feed.

Acceptance criteria:

- Unfollowing is idempotent.
- Existing feed entries may disappear immediately or during feed refresh, but
  new feed pages must respect the unfollow.

### Like Post

1. A signed-in user views a post or feed item.
2. The user likes the post.
3. The product records the like.
4. The post like count and viewer like state update.

Acceptance criteria:

- Liking is idempotent.
- A user can like a post at most once.
- Like count must not require counting all like rows on every read.

### Unlike Post

1. A signed-in user views a liked post.
2. The user removes the like.
3. The product removes or deactivates the like.
4. The post like count and viewer like state update.

Acceptance criteria:

- Unliking is idempotent.
- Like count should not become negative.
- Repeated like and unlike calls should not corrupt counters.

### Add Comment

1. A signed-in user opens a post.
2. The user writes a comment. The comment may include profile mentions.
3. The product validates and creates the comment.
4. The comment appears on the post detail page.
5. The post comment count updates.

Acceptance criteria:

- Empty comments are rejected.
- Comment length is limited.
- Comment count must not require counting all comment rows on every read.
- Moderation or spam checks may delay visibility.
- Mentions in the comment are parsed and linked to mentioned profiles.
- A user can comment only if allowed by profile visibility, block state, comment
  settings, and moderation rules.

### Reply to Comment

1. A signed-in user opens comments on a post.
2. The user taps reply on a comment.
3. The composer indicates the reply target and may insert an `@username`
   mention.
4. The user submits the reply.
5. The reply appears in the comment thread according to visibility and
   moderation rules.

Acceptance criteria:

- Replies preserve a link to the parent comment.
- Replies preserve the profile being replied to.
- Reply submission follows the same validation, mention parsing, rate limiting,
  and moderation behavior as comments.
- Deleted parent comments should not break reply display.

### Mention Autocomplete

1. A signed-in user types `@` followed by a partial username or display name in a
   caption or comment composer.
2. The product returns matching profiles.
3. The user selects a profile.
4. The composer inserts a mention token that displays as username text.
5. When the post or comment is submitted, the mention is saved as a reference to
   the selected profile.

Acceptance criteria:

- Results include username, display name, and avatar.
- Disabled, moderated, blocked, or otherwise unavailable profiles are excluded.
- Results are ranked by likely relevance, such as exact username prefix,
  followed accounts, mutual connections, or recent interactions.
- Autocomplete should be fast enough for interactive typing.
- The submitted mention must resolve to a profile id, not only raw text.
- The product should handle stale autocomplete selections if a profile is
  renamed, disabled, or blocked before submission.

### Tag People on Media

1. A signed-in user creates or edits a post.
2. The user selects a media item and searches for a profile to tag.
3. The user places the tag on the media item.
4. The product validates the tagged profile's tag permissions.
5. The post displays the tag to viewers who can see the post.

Acceptance criteria:

- Tags are attached to media items, not only to the caption.
- A carousel post can tag different profiles on different media items.
- A profile's tag settings may allow tags from everyone, followed people, or no
  one.
- A tagged user can remove themselves from a post according to product policy.
- Tags on private posts are visible only to viewers who can see the post.

### Save Post

1. A signed-in user views a post.
2. The user saves the post.
3. The post appears in the user's saved posts.
4. The user may add the post to a collection.

Acceptance criteria:

- Saving is idempotent.
- Saved posts are private to the saving user.
- Unsaving removes the post from saved posts and collections.
- Deleted, moderated, or unavailable posts must not appear in saved-post
  surfaces.

### Manage Saved Collections

1. A signed-in user opens saved posts.
2. The user creates, renames, or deletes a collection.
3. The user adds or removes saved posts from the collection.

Acceptance criteria:

- Collection names are validated and limited in length.
- Deleting a collection does not delete the underlying posts.
- Collaborative collections are out of scope.

### Delete Comment

1. A comment author opens their comment.
2. The author deletes the comment.
3. The product hides the comment from normal readers.
4. The post comment count updates according to product policy.

Acceptance criteria:

- A user can delete their own comments.
- A post author can delete comments on their own posts.
- Deletion should preserve enough information for audit and moderation.
- Repeated delete requests are safe.

### Block User

1. A signed-in user opens another profile or interaction surface.
2. The user blocks the other profile.
3. The blocked profile loses access to restricted surfaces and interactions.
4. Existing follow relationships between the two profiles are removed or
   disabled according to product policy.

Acceptance criteria:

- Blocking is idempotent.
- Blocked profiles cannot follow, like, comment, mention, tag, or message the
  blocker in scoped product surfaces.
- Blocked profiles are excluded from search, autocomplete, feeds, and profile
  suggestions where appropriate.
- Unblocking does not automatically restore prior follow relationships unless
  product policy says otherwise.

### Restrict User

1. A signed-in user restricts another profile.
2. Interactions from the restricted profile are limited without fully blocking
   them.
3. Comments from restricted profiles may be hidden, held for approval, or shown
   only to the restricted commenter.

Acceptance criteria:

- Restricting is idempotent.
- Restriction affects comments, mentions, and interaction visibility according
  to product policy.
- Restriction should be less visible to the restricted user than blocking.

### Report Content or Profile

1. A user reports a post, comment, or profile.
2. The product asks for a reason.
3. The report is submitted for moderation.
4. The product may remove, reduce distribution, hide, or leave the target
   unchanged after review.
5. The reporter may be able to view report status.

Acceptance criteria:

- Reporting is available near the content or profile being reported.
- Reports preserve target identity and enough context for review.
- Reports from the same user against the same target should not create
  unbounded duplicates.
- Report outcomes should not expose private moderator reasoning beyond product
  policy.

### Search Profiles

1. A user enters a query.
2. The product returns matching profiles.
3. Results show username, display name, avatar, and follower count.

Acceptance criteria:

- Search handles partial usernames and display names.
- Disabled or moderated profiles are excluded.
- Search is paginated or limited.

### View Hashtag Page

1. A user taps or searches for a hashtag.
2. The product returns public posts associated with that hashtag.
3. The user can paginate through eligible posts.

Acceptance criteria:

- Public posts with matching hashtags may appear.
- Private posts must not appear to unapproved viewers.
- Deleted, moderated, archived, or unavailable posts are excluded.
- Ordering may be ranked or recent, but must be stable for pagination.

### View Location Page

1. A user taps a location on a post or searches for a supported location.
2. The product returns public posts associated with that location.
3. The user can paginate through eligible posts.

Acceptance criteria:

- Only supported existing locations can be attached to posts.
- Private posts must not appear to unapproved viewers.
- Deleted, moderated, archived, or unavailable posts are excluded.

### View Notifications

1. A signed-in user opens activity notifications.
2. The product returns recent activity involving that user.
3. The user can mark notifications as read by viewing them.

Acceptance criteria:

- Notifications may include follows, follow requests, likes, comments, replies,
  mentions, media tags, and report outcomes.
- Notification generation must respect privacy, block state, mention settings,
  tag settings, and moderation state.
- Duplicate retries should not create duplicate notification rows for the same
  logical activity.
- Users can configure notification delivery preferences separately from the
  existence of in-app activity records.

## Views and Surfaces

### Home Feed Item

Shows:

- post id
- author username, display name, avatar
- media preview
- caption preview
- linked caption mentions
- media tags when opened or expanded
- location when present
- created time
- like count
- comment count
- viewer save state
- whether viewer liked the post

### Post Detail

Shows:

- post id
- author summary
- full media
- full caption
- linked caption mentions
- media tags
- hashtags
- location
- created time
- like count
- comment count
- viewer like state
- viewer save state
- paginated comments

### Profile Page

Shows:

- profile id
- username
- display name
- bio
- avatar
- follower count
- following count
- post count
- viewer follow state
- paginated post grid
- tagged-posts entry point if visible to the viewer

### Comment Item

Shows:

- comment id
- author summary
- body
- linked mentions
- parent comment reference for replies
- created time

### Tagged Posts

Shows:

- profile id
- tagged profile summary
- paginated posts where the profile is tagged

The surface must respect post visibility, tag permissions, removed tags,
moderation state, and block state.

### Saved Posts

Shows:

- viewer profile id
- paginated saved posts
- collections

Saved posts are private to the viewer.

### Hashtag Page

Shows:

- normalized hashtag
- paginated public post grid

The surface excludes private, archived, deleted, moderated, or unavailable
posts.

### Location Page

Shows:

- location id
- location name
- paginated public post grid

The surface excludes private, archived, deleted, moderated, or unavailable
posts.

### Notification Item

Shows:

- notification id
- activity type
- actor summary
- source reference
- created time
- read state

## Important Engineering Caveats

### Counters

Like counts, comment counts, save counts, follower counts, following counts,
post counts, pending follow request counts, and tagged-post counts should be
materialized or separately tracked. They should not be computed by counting all
related rows on every read.

Counter updates must handle retries, duplicate requests, and concurrent writes.
The product can tolerate short-lived eventual consistency, but counters must
converge and must not drift permanently.

### Feed Generation

The home feed can become expensive because it depends on the follow graph. The
system should support a feed strategy that can scale beyond simple joins.

Possible approaches include:

- read-time fan-in from followed profiles
- write-time fan-out to follower inboxes
- hybrid fan-out for high-volume accounts

The PRD does not mandate the strategy, but feed reads must remain fast and
pagination must remain stable.

### Media Processing

Uploads should not be treated as immediately final. Images and videos need
validation, virus or abuse scanning, transcoding, thumbnail generation, metadata
extraction, and storage of multiple display variants.

Posts should represent media processing state so failed or incomplete uploads do
not appear as broken content.

### Pagination

Feeds, profile grids, comments, likes, followers, and search results should use
cursor-style pagination. Offset pagination may become unstable or expensive as
data grows.

Pagination should produce deterministic results for a cursor even when new
content is created.

### Viewer-Specific State

Many views are partly viewer-specific. A post card may include whether the
current viewer liked the post. A profile view may include whether the current
viewer follows that profile.

The product should treat these as part of the read contract, not as client-only
derived state.

### Mentions

Mentions should be stored as structured references in addition to the text body
that contains them. Parsing mention text on every read is fragile and makes
username changes difficult.

Mention autocomplete should use a search-friendly index over usernames and
display names. It must respect blocking, privacy, disabled accounts, moderation
state, and rate limits.

Mention extraction should be deterministic. The product should define how it
handles punctuation, duplicate mentions, renamed profiles, deleted profiles, and
plain text that looks like a mention but was not selected from autocomplete.

Mentions should create in-app activity when policy allows. Push delivery depends
on the recipient's notification settings.

### Tags

Media tags should be stored as structured references to profiles and media
items. They should preserve placement and support removal by the tagged user.

Tag permissions must be enforced when composing, editing, displaying, and
notifying. A user may allow tags from everyone, people they follow, or no one.
Policy must define how pending or disallowed tags are handled.

Tagged profiles are visible to anyone who can see the post. Tags on private
posts must not grant visibility to users who cannot otherwise see the post.

### Hashtags and Locations

Hashtags and locations create discovery surfaces. Public posts may appear in
hashtag and location results. Private posts must remain private even when they
contain hashtags or locations.

Hashtag and location indexes may be eventually consistent, but they must remove
or hide deleted, archived, moderated, private, blocked, and unavailable content.

Location tagging should use supported existing locations. Creating arbitrary new
locations is out of scope.

### Saves and Collections

Saved posts and collections are private user library surfaces. They should not
be exposed to other users unless a collaborative collection feature is added.

Save state is viewer-specific and should be part of post read contracts where
the UI needs to show whether the current viewer has saved a post.

### Notifications

The product should create in-app activity records for important interactions
such as follows, follow requests, likes, comments, replies, mentions, media tags,
and report outcomes.

Notification delivery preferences are separate from notification existence. A
user may disable push delivery while the in-app activity record still exists.

Notification creation must respect privacy, blocking, restriction, tag settings,
mention settings, moderation, and duplicate retries. For example, a mention from
a private profile may not notify someone who cannot see the source content.

### Privacy, Blocking, and Moderation

Even in a minimal version, the product should account for hidden or unavailable
content. Deleted, moderated, disabled, private, or blocked content must not leak
through feeds, profiles, search, mention autocomplete, tag autocomplete,
hashtag pages, location pages, saved posts, tagged posts, notifications, or
direct post links.

If full privacy and blocking features are deferred, the data model should still
avoid making them impossible to add later.

Private profiles require follow approval. Only approved followers can see, like,
or comment on private posts. Public profiles can be followed without approval
unless blocked or otherwise restricted.

Blocking should prevent the blocked profile from interacting with the blocker
and should remove or disable existing follow relationships according to product
policy. Restricting should limit interaction visibility without being as explicit
as blocking.

Reports must preserve enough context for review and audit while limiting what is
revealed back to the reporter or reported user.

### Idempotency

Follow, unfollow, follow request approval, follow request denial, like, unlike,
save, unsave, comment delete, post publish, post archive, post delete, block,
unblock, restrict, unrestrict, tag removal, and report submission operations
should be safe to retry. Client retries and network failures must not create
duplicate relationships or corrupt counters.

### Deletion

User-facing deletion should usually be soft deletion. The product may need to
retain records for audit, moderation, legal, abuse prevention, or counter
repair.

Deletion behavior must be clear for posts, comments, media assets, profiles, and
relationships. Deleted content may remain recoverable for a limited recently
deleted window before permanent deletion.

Archiving is different from deletion. Archived posts are hidden from normal
public surfaces but preserve likes and comments and remain available to the
author.

### Abuse and Rate Limits

The product should protect write-heavy actions such as follow, like, comment,
search, mention autocomplete, tag autocomplete, saving, reporting, and post
creation. Rate limits and abuse detection are product requirements, even if the
first implementation uses simple rules.

Comment rules should account for spam-like behavior such as excessive mentions,
excessive hashtags, repeated comments, and unsafe content. Product policy may
limit the number of mentions or hashtags allowed in one comment.

### Consistency Model

The product does not require strict global consistency for every read. It can
accept eventual consistency for feeds, counters, search indexes, hashtag pages,
location pages, saved-post repair, and notification delivery.

The product does require strong correctness for identity, uniqueness,
relationship uniqueness, ownership checks, privacy checks, block checks,
permission checks, and user-visible mutation results.

### Account Lifecycle

The product must handle login, logout, authentication recovery, account disable,
account deletion, and username changes. These flows can be thin in the first
version, but core records should preserve enough state to prevent username
collisions, orphaned profiles, leaked private content, and broken mentions or
tags.

## Success Metrics

- Users can complete sign-up and create a profile.
- Users can sign in, sign out, recover access, and deactivate or delete accounts.
- Users can publish posts with valid processed media.
- Users can edit, archive, delete, and restore eligible posts according to
  policy.
- Users can follow others and see followed posts in the home feed.
- Private profile follow requests can be approved or denied.
- Users can like and unlike posts without duplicate likes or counter corruption.
- Users can comment and see comments on post detail pages.
- Users can reply to comments.
- Users can mention profiles in captions and comments.
- Users can tag profiles on media, and tagged users can remove themselves.
- Users can save posts and organize saved posts into private collections.
- Users can search profiles and browse eligible hashtag and location pages.
- Users can report posts, comments, and profiles.
- Users can block, restrict, and manage interaction visibility.
- Users receive in-app activity records for important interactions.
- Feed, profile, and post views remain fast with paginated results.
- Moderated or deleted content does not appear in normal user surfaces.

## Open Product Questions

- Are profiles public by default?
- Are private profiles supported in the first version?
- Are videos supported in the first version, or only images?
- Are carousel posts required immediately?
- Can users edit captions after publishing?
- Can users hide like counts?
- Are logged-out visitors allowed to view public profiles and posts?
- What is the exact recovery window for deleted posts and accounts?
- When can a deleted or renamed username be reused?
- Should mentions notify mentioned users in the first version?
- Should users be able to disable mentions from strangers?
- Should users be able to require manual approval for tags?
- Which notification types support push delivery in addition to in-app activity?
- Which report outcomes are visible to reporters?
- Should post deletion remove the post from follower feeds immediately or
  eventually?

## Policy References

This PRD is grounded in the following Instagram Help Center behaviors:

- [Who can mention or tag you on Instagram](https://help.instagram.com/345505572238391/)
- [Tag people in your posts on Instagram](https://help.instagram.com/174635396025538/)
- [Who can see a post when you tag someone in it on Instagram](https://help.instagram.com/412981112149384/)
- [Remove yourself from a post someone tagged you in on Instagram](https://help.instagram.com/178891742266091/)
- [Comments and mentions from private profiles on Instagram](https://help.instagram.com/282667658504758/)
- [Who can like or comment on your Instagram content](https://help.instagram.com/486923551356292/)
- [Approve or deny a follower request on Instagram](https://help.instagram.com/207917546007234/)
- [Blocking People](https://help.instagram.com/426700567389543/)
- [Restrict or unrestrict someone on Instagram](https://help.instagram.com/2638385956221960/)
- [Report a post or profile on Instagram](https://help.instagram.com/192435014247952/)
- [Report a comment on Instagram](https://help.instagram.com/198034803689028/)
- [What happens when you report a post or profile on Instagram](https://help.instagram.com/478796623912131/)
- [Save posts you see on Instagram](https://help.instagram.com/1744643532522513/)
- [Manage your collections on Instagram](https://help.instagram.com/1524436600950937/)
- [Add, edit or delete the caption of an existing Instagram post](https://help.instagram.com/1490745927855762)
- [Edit and Delete Your Posts](https://help.instagram.com/997924900322403/)
- [Archive a post you've shared on Instagram](https://help.instagram.com/136706673552668/)
- [What happens to content you delete on Instagram](https://help.instagram.com/711062676142607)
- [Reply to a comment on Instagram](https://help.instagram.com/366968666719222/)
- [Use hashtags on Instagram](https://help.instagram.com/351460621611097/)
- [See place pages or hashtag search results on Instagram](https://help.instagram.com/458423657648149)
- [Posts that appear on Instagram place and hashtag search results](https://help.instagram.com/355932664593846/)
- [Locations you can tag in your Instagram posts](https://help.instagram.com/1618893218361276/)
- [How notifications work on Instagram](https://help.instagram.com/124119401075803/)
- [When Instagram sends push notifications to your device](https://help.instagram.com/162672033874406/)
- [Manage push notifications you receive](https://help.instagram.com/546541825361643/)

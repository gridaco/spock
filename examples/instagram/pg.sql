-- ═══════════════════════════════════════════════════════════════════════════
-- instagram/pg.sql — reference implementation ("answer sheet")
-- ═══════════════════════════════════════════════════════════════════════════
--
-- The Instagram PRD (./PRD.md) implemented in plain PostgreSQL, the way a
-- seasoned Postgres engineer would: schema, constraints, triggers, policy
-- predicates, an API of SECURITY DEFINER functions, RLS as defense in depth,
-- and maintenance jobs. No ORM, no framework, no DSL — the database is the
-- backend.
--
-- Requirements: PostgreSQL 16+ (PG 18 ships native uuidv7(); a backport is
-- included). Extensions: citext, pgcrypto, pg_trgm.
--
-- Run:   psql -X -v ON_ERROR_STOP=1 -f pg.sql -d <dev-db>
-- Safe to re-run: drops and recreates the app/api/ext schemas.
-- The smoke test at the bottom runs inside BEGIN…ROLLBACK.
--
-- ── Pattern catalog ─────────────────────────────────────────────────────────
-- Tagged inline as [P<n>] at each site of use.
--
--  P1   time-ordered uuidv7 primary keys (index locality, natural cursors)
--  P2   dedicated `ext` schema for extensions + pinned search_path
--  P3   domains: validation lives in the type, not in app code
--  P4   enums for closed vocabularies
--  P5   citext for case-insensitive identifiers (email)
--  P6   password hashing in-database (pgcrypto crypt/bf)
--  P7   tokens stored as digests; single-use via consumed_at + FOR UPDATE
--  P8   composite primary keys on relationship tables (no surrogate id)
--  P9   CHECK constraints: cross-column, conditional, counter floors
--  P10  partial UNIQUE preserving history + ON CONFLICT with partial predicate
--  P11  composite FKs to superkeys: cross-table consistency, declaratively
--  P12  stored generated columns as polymorphic dedupe keys
--  P13  num_nonnulls() for exactly-one-of polymorphic targets
--  P14  soft delete + partial indexes over live rows only
--  P15  denormalized counters: statement-level triggers w/ transition tables
--       (bulk-safe) and row-level was/now delta triggers (state flips)
--  P16  deadlock avoidance by consistent update ordering
--  P17  idempotent verbs: ON CONFLICT DO NOTHING; triggers only fire on
--       rows actually inserted/deleted, so counters cannot drift on retries
--  P18  state machines enforced by one generic transition trigger
--  P19  auto-provisioning rows via AFTER INSERT triggers
--  P20  updated_at touch trigger
--  P21  keyset pagination: row-value comparison + matching DESC index
--  P22  partial covering indexes for the hot read paths
--  P23  LATERAL + jsonb_agg for nested payloads in one round trip
--  P24  EXISTS subselects for viewer-specific state (liked/saved/follows)
--  P25  policy predicates as STABLE LANGUAGE sql functions (inlinable)
--  P26  guard once, then scan — privacy checks hoisted out of row loops
--  P27  forbidden ≡ not found: privacy errors do not leak existence
--  P28  pg_trgm search + text_pattern_ops prefix autocomplete
--  P29  set-returning regexp for mention/hashtag extraction; upsert dims,
--       then re-select (ON CONFLICT…RETURNING misses existing rows)
--  P30  data-modifying CTEs: several writes, one statement
--  P31  work queues with FOR UPDATE SKIP LOCKED
--  P32  advisory locks for singleton maintenance jobs
--  P33  counter reconciliation guarded by IS DISTINCT FROM (no dead updates)
--  P34  fixed-window rate limiting in an UNLOGGED table
--  P35  SECURITY DEFINER + SET search_path; the API is functions, not tables
--  P36  updatable views (security_barrier + CHECK OPTION) as column-level
--       write grants
--  P37  row-level security keyed to a request GUC — defense in depth
--  P38  LISTEN/NOTIFY as the delivery hint over an outbox table
--  P39  fillfactor headroom so counter updates stay HOT
--  P40  named constraints + GET STACKED DIAGNOSTICS → precise error mapping
--
-- ── Error codes (custom SQLSTATEs, class "IG") ──────────────────────────────
--  IG401 authentication failed        IG403 forbidden
--  IG404 not found (or not visible)   IG409 conflict (mapped uniques)
--  IG422 invalid input / transition   IG429 rate limited
-- ═══════════════════════════════════════════════════════════════════════════


-- ─────────────────────────────────────────────────────────────────────────────
-- §1  Schemas, roles, extensions, core helpers
-- ─────────────────────────────────────────────────────────────────────────────

drop schema if exists app cascade;
drop schema if exists api cascade;
drop schema if exists ext cascade;

create schema app;   -- tables, triggers, internal functions
create schema api;   -- the callable surface (the only thing granted)
create schema ext;   -- extensions live apart from user objects        [P2]

create extension if not exists citext   with schema ext;
create extension if not exists pgcrypto with schema ext;
create extension if not exists pg_trgm  with schema ext;

-- application role: may execute api.*, may touch nothing else directly
do $$
begin
  if not exists (select 1 from pg_roles where rolname = 'ig_api') then
    create role ig_api nologin;
  end if;
end $$;

-- uuidv7: time-ordered uuids — b-tree locality for append-heavy tables and
-- a monotonic tiebreaker for keyset cursors. PG 18: use built-in uuidv7()
-- and delete this backport (the widely used overlay/set_bit construction).
create function app.uuid_v7() returns uuid                        -- [P1]
language plpgsql volatile as $$
begin
  return encode(
    set_bit(set_bit(
      overlay(uuid_send(gen_random_uuid())
              placing substring(int8send((extract(epoch from clock_timestamp()) * 1000)::bigint) from 3)
              from 1 for 6),
      52, 1), 53, 1), 'hex')::uuid;
end $$;

-- request-scoped identity for RLS; set by api.auth() per connection/txn [P37]
create function app.current_profile_id() returns uuid
language sql stable as $$
  select nullif(current_setting('app.profile_id', true), '')::uuid
$$;

-- generic touch trigger                                             [P20]
create function app.tg_touch() returns trigger
language plpgsql as $$
begin
  new.updated_at := now();
  return new;
end $$;

-- generic state-machine enforcement: TG_ARGV[0] = column, rest = 'from>to'
-- edges. One trigger function serves every lifecycle column.        [P18]
create function app.tg_enforce_transition() returns trigger
language plpgsql as $$
declare
  col text := tg_argv[0];
  o   text := to_jsonb(old) ->> tg_argv[0];
  n   text := to_jsonb(new) ->> tg_argv[0];
begin
  if o is distinct from n and not (o || '>' || n = any (tg_argv[1:])) then
    raise exception '%.% cannot change from % to %', tg_table_name, col, o, n
      using errcode = 'IG422',
            hint    = 'allowed: ' || array_to_string(tg_argv[1:], ', ');
  end if;
  return new;
end $$;

-- fixed-window rate limiter. UNLOGGED: losing counters on crash is an
-- acceptable trade for write cost on this hot, disposable table.    [P34]
create unlogged table app.rate_limit (
  profile_id   uuid        not null,
  action       text        not null,
  window_start timestamptz not null,
  hits         int         not null default 1,
  primary key (profile_id, action, window_start)
);

create function app.hit_rate_limit(
  p_profile uuid, p_action text, p_limit int, p_window interval
) returns void
language plpgsql as $$
declare
  v_window timestamptz :=
    to_timestamp(floor(extract(epoch from now()) / extract(epoch from p_window))
                 * extract(epoch from p_window));
  v_hits int;
begin
  insert into app.rate_limit (profile_id, action, window_start)
  values (p_profile, p_action, v_window)
  on conflict (profile_id, action, window_start)
  do update set hits = app.rate_limit.hits + 1
  returning hits into v_hits;

  if v_hits > p_limit then
    raise exception 'rate limit exceeded for %', p_action using errcode = 'IG429';
  end if;
end $$;
-- purge old windows from cron:
--   delete from app.rate_limit where window_start < now() - interval '2 days';


-- ─────────────────────────────────────────────────────────────────────────────
-- §2  Types: enums and domains
-- ─────────────────────────────────────────────────────────────────────────────

create type app.account_status as enum                             -- [P4]
  ('active', 'deactivated', 'disabled', 'deletion_pending', 'deleted');
  -- deactivated = self-service, signing in reactivates
  -- disabled    = moderation action, signing in is refused

create type app.visibility        as enum ('public', 'private');
create type app.audience_perm     as enum ('everyone', 'followed', 'no_one');
create type app.comment_perm      as enum ('everyone', 'followers', 'no_one');
create type app.post_status       as enum ('published', 'archived', 'deleted');
create type app.media_kind        as enum ('image', 'video');
create type app.media_status      as enum ('uploaded', 'processing', 'ready', 'failed');
create type app.request_status    as enum ('pending', 'approved', 'denied');
create type app.moderation_status as enum ('visible', 'limited', 'held', 'removed');
create type app.report_status     as enum ('open', 'reviewed');
create type app.report_reason     as enum
  ('spam', 'abuse', 'nudity', 'violence', 'intellectual_property', 'self_harm', 'other');
create type app.report_outcome    as enum ('removed', 'limited', 'dismissed');
create type app.notification_kind as enum
  ('follow', 'follow_request', 'follow_request_approved',
   'like', 'comment', 'reply', 'mention', 'media_tag', 'report_resolved');

-- validation as types: invalid values are unrepresentable at rest    [P3]
-- usernames are stored strictly lowercase; normalize at the boundary.
create domain app.username as text
  check (value ~ '^[a-z0-9._]{3,30}$');

create domain app.display_name    as text check (length(value) between 1 and 30);
create domain app.bio_text        as text check (length(value) <= 150);
create domain app.caption_text    as text check (length(value) <= 2200);
create domain app.comment_body    as text check (length(value) between 1 and 2200);
create domain app.collection_name as text check (length(value) between 1 and 50);
create domain app.hashtag_text    as text check (value ~ '^[a-z0-9_]{1,64}$');


-- ─────────────────────────────────────────────────────────────────────────────
-- §3  Auth: accounts, sessions, recovery
--     (kept minimal but real: hashed credentials, hashed tokens, lifecycle)
-- ─────────────────────────────────────────────────────────────────────────────

create table app.account (
  id              uuid primary key default app.uuid_v7(),
  email           ext.citext,                                      -- [P5]
  phone           text,
  password_hash   text,           -- null ⇒ external identity only
  status          app.account_status not null default 'active',
  is_moderator    boolean not null default false,
  deletion_due_at timestamptz,    -- set when status = 'deletion_pending'
  created_at      timestamptz not null default now(),

  constraint uq_account_email unique (email),
  constraint uq_account_phone unique (phone),
  -- conditional constraint: purged accounts are scrubbed of identifiers [P9]
  constraint ck_account_identifier
    check (status = 'deleted' or num_nonnulls(email, phone) >= 1)
);

create trigger trg_account_status_machine
  before update of status on app.account
  for each row when (old.status is distinct from new.status)
  execute function app.tg_enforce_transition(                       -- [P18]
    'status',
    'active>deactivated',      'deactivated>active',
    'active>disabled',         'disabled>active',
    'active>deletion_pending', 'deactivated>deletion_pending',
    'deletion_pending>active', 'deletion_pending>deleted');

create table app.session (
  id         uuid primary key default app.uuid_v7(),
  account_id uuid not null references app.account (id) on delete cascade,
  -- never store the bearer token itself; store its digest            [P7]
  token_hash bytea not null,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null,
  revoked_at timestamptz,

  constraint uq_session_token unique (token_hash)
);

create index idx_session_account on app.session (account_id);

create table app.recovery_token (
  id          uuid primary key default app.uuid_v7(),
  account_id  uuid not null references app.account (id) on delete cascade,
  token_hash  bytea not null,
  expires_at  timestamptz not null,
  consumed_at timestamptz,                                          -- [P7]

  constraint uq_recovery_token unique (token_hash)
);

-- ─────────────────────────────────────────────────────────────────────────────
-- §4  Identity: profile, notification preferences
-- ─────────────────────────────────────────────────────────────────────────────

create table app.profile (
  id                    uuid primary key default app.uuid_v7(),     -- [P1]
  account_id            uuid not null references app.account (id),
  username              app.username not null,
  display_name          app.display_name,
  bio                   app.bio_text,
  avatar_media_id       uuid,      -- FK added in §6 (circular with media_asset)
  visibility            app.visibility   not null default 'public',
  tag_permission        app.audience_perm not null default 'everyone',
  mention_permission    app.audience_perm not null default 'everyone',
  comment_permission    app.comment_perm  not null default 'everyone',
  -- denormalized from account.status by trigger: keeps the hot read paths
  -- to a single join. The account row stays the source of truth.
  is_active             boolean not null default true,

  -- materialized counters, maintained by triggers (§5, §7, §8).      [P15]
  -- CHECK floors are the tripwire: a bug that would drive a counter
  -- negative fails loudly instead of drifting silently.              [P9]
  follower_count        int not null default 0 check (follower_count  >= 0),
  following_count       int not null default 0 check (following_count >= 0),
  post_count            int not null default 0 check (post_count      >= 0),
  pending_request_count int not null default 0 check (pending_request_count >= 0),

  created_at            timestamptz not null default now(),
  updated_at            timestamptz not null default now(),

  constraint uq_profile_account  unique (account_id),
  constraint uq_profile_username unique (username)
  -- Username reuse policy: the name is held for the account's whole
  -- lifetime, including deactivation and the deletion grace window; it is
  -- released only when purge renames it to a tombstone (§15). That makes
  -- the reuse policy an explicit line of code, not an accident.
);

-- counter columns are updated in place constantly: leave page headroom so
-- those updates stay HOT (no index maintenance, less bloat).         [P39]
alter table app.profile set (fillfactor = 90);

create trigger trg_profile_touch
  before update on app.profile
  for each row execute function app.tg_touch();                     -- [P20]

-- search: trigram for fuzzy, text_pattern_ops for prefix autocomplete [P28]
create index idx_profile_username_trgm
  on app.profile using gin (username ext.gin_trgm_ops);
create index idx_profile_display_name_trgm
  on app.profile using gin (display_name ext.gin_trgm_ops);
create index idx_profile_username_prefix
  on app.profile (username text_pattern_ops);

comment on table app.profile is
  'Public identity. Counters are denormalized; reconcile via app.reconcile_counters().';

create table app.notification_prefs (
  profile_id    uuid primary key references app.profile (id) on delete cascade,
  push_follows  boolean not null default true,
  push_likes    boolean not null default true,
  push_comments boolean not null default true,
  push_mentions boolean not null default true,
  push_tags     boolean not null default true
);
-- Preferences gate *delivery* (the push worker reads them); they never gate
-- the existence of the in-app activity row (PRD, "Notifications").

-- auto-provision prefs with the profile                              [P19]
create function app.tg_provision_profile() returns trigger
language plpgsql as $$
begin
  insert into app.notification_prefs (profile_id) values (new.id);
  return null;
end $$;

create trigger trg_profile_provision
  after insert on app.profile
  for each row execute function app.tg_provision_profile();

-- mirror account.status into profile.is_active (denormalization kept
-- honest: one writer, trigger-owned)
create function app.tg_mirror_account_status() returns trigger
language plpgsql as $$
begin
  update app.profile
     set is_active = (new.status = 'active')
   where account_id = new.id
     and is_active is distinct from (new.status = 'active');        -- [P33]
  return null;
end $$;

create trigger trg_account_status_mirror
  after update of status on app.account
  for each row execute function app.tg_mirror_account_status();


-- ─────────────────────────────────────────────────────────────────────────────
-- §5  Graph: follow, follow_request, block, restriction
-- ─────────────────────────────────────────────────────────────────────────────

create table app.follow (
  follower_id uuid not null references app.profile (id),
  followee_id uuid not null references app.profile (id),
  created_at  timestamptz not null default now(),

  constraint pk_follow primary key (follower_id, followee_id),      -- [P8]
  constraint ck_follow_no_self check (follower_id <> followee_id)   -- [P9]
);

-- PK serves follower→followees (feed membership); the reverse index
-- serves followee→followers (follower lists, is_follower checks).
create index idx_follow_reverse on app.follow (followee_id, follower_id);

-- Both endpoint counters in one trigger. Two profile rows are updated per
-- follow; concurrent A↔B follows would deadlock if update order were
-- arbitrary — so always update in primary-key order.                 [P16]
create function app.tg_follow_counters() returns trigger
language plpgsql as $$
declare
  d int;
  f uuid;   -- follower
  t uuid;   -- followee
begin
  if tg_op = 'INSERT' then d := 1;  f := new.follower_id; t := new.followee_id;
  else                     d := -1; f := old.follower_id; t := old.followee_id;
  end if;

  if f < t then
    update app.profile set following_count = following_count + d where id = f;
    update app.profile set follower_count  = follower_count  + d where id = t;
  else
    update app.profile set follower_count  = follower_count  + d where id = t;
    update app.profile set following_count = following_count + d where id = f;
  end if;
  return null;
end $$;

create trigger trg_follow_counters
  after insert or delete on app.follow
  for each row execute function app.tg_follow_counters();
-- Idempotency note: api.follow() inserts ON CONFLICT DO NOTHING, and a
-- no-op conflict fires no trigger — retries cannot inflate counters. [P17]

create table app.follow_request (
  requester_id uuid not null references app.profile (id),
  target_id    uuid not null references app.profile (id),
  status       app.request_status not null default 'pending',
  created_at   timestamptz not null default now(),
  responded_at timestamptz,

  constraint pk_follow_request primary key (requester_id, target_id),
  constraint ck_follow_request_no_self check (requester_id <> target_id)
);

create trigger trg_follow_request_machine
  before update of status on app.follow_request
  for each row when (old.status is distinct from new.status)
  execute function app.tg_enforce_transition(
    'status',
    'pending>approved', 'pending>denied',
    'denied>pending');   -- re-requesting after a denial reopens the row

-- inbox scan + badge count both hit only pending rows                [P22]
create index idx_follow_request_pending
  on app.follow_request (target_id, created_at desc)
  where status = 'pending';

-- pending_request_count: row-level was/now delta — counts a *condition*,
-- not row existence, so INSERT/UPDATE/DELETE all adjust it.          [P15]
create function app.tg_pending_request_count() returns trigger
language plpgsql as $$
declare
  was int := case when tg_op = 'INSERT' then 0
                  else (old.status = 'pending')::int end;
  now_ int := case when tg_op = 'DELETE' then 0
                   else (new.status = 'pending')::int end;
begin
  if now_ <> was then
    update app.profile
       set pending_request_count = pending_request_count + (now_ - was)
     where id = coalesce(new.target_id, old.target_id);
  end if;
  return null;
end $$;

create trigger trg_pending_request_count
  after insert or delete or update of status on app.follow_request
  for each row execute function app.tg_pending_request_count();

create table app.block (
  blocker_id uuid not null references app.profile (id),
  blocked_id uuid not null references app.profile (id),
  created_at timestamptz not null default now(),

  constraint pk_block primary key (blocker_id, blocked_id),
  constraint ck_block_no_self check (blocker_id <> blocked_id)
);

-- blocking is checked in both directions: index both orders
create index idx_block_reverse on app.block (blocked_id, blocker_id);

create table app.restriction (
  restrictor_id uuid not null references app.profile (id),
  restricted_id uuid not null references app.profile (id),
  created_at    timestamptz not null default now(),

  constraint pk_restriction primary key (restrictor_id, restricted_id),
  constraint ck_restriction_no_self check (restrictor_id <> restricted_id)
);

-- ─────────────────────────────────────────────────────────────────────────────
-- §6  Assets and places
-- ─────────────────────────────────────────────────────────────────────────────

create table app.media_asset (
  id          uuid primary key default app.uuid_v7(),
  owner_id    uuid not null references app.profile (id),
  storage_key text not null,          -- object-storage pointer (upload done out of band)
  kind        app.media_kind not null,
  width       int,
  height      int,
  duration_ms int,
  variants    jsonb not null default '[]'::jsonb,   -- processed renditions
  status      app.media_status not null default 'uploaded',
  created_at  timestamptz not null default now(),

  constraint ck_media_video_fields check (kind = 'video' or duration_ms is null)
);

create index idx_media_owner on app.media_asset (owner_id, created_at desc);

-- uploads are not final until a worker walks them through the machine:
-- uploaded → processing → ready | failed. Nothing can skip to 'ready'.
create trigger trg_media_status_machine
  before update of status on app.media_asset
  for each row when (old.status is distinct from new.status)
  execute function app.tg_enforce_transition(
    'status',
    'uploaded>processing', 'processing>ready', 'processing>failed');

-- resolve the profile ↔ media_asset circularity now that both exist
alter table app.profile
  add constraint fk_profile_avatar
  foreign key (avatar_media_id) references app.media_asset (id);

create table app.location (
  id                uuid primary key default app.uuid_v7(),
  name              text not null,
  external_place_id text not null,
  created_at        timestamptz not null default now(),

  constraint uq_location_place unique (external_place_id)
);
-- Locations are imported from a places provider; the product never mints
-- them from user input (PRD, "Locations").


-- ─────────────────────────────────────────────────────────────────────────────
-- §7  Posts
-- ─────────────────────────────────────────────────────────────────────────────

create table app.post (
  id            uuid primary key default app.uuid_v7(),             -- [P1]
  author_id     uuid not null references app.profile (id),
  caption       app.caption_text,
  location_id   uuid references app.location (id),
  status        app.post_status not null default 'published',
  moderation    app.moderation_status not null default 'visible',
  like_count    int not null default 0 check (like_count    >= 0),
  comment_count int not null default 0 check (comment_count >= 0),
  save_count    int not null default 0 check (save_count    >= 0),
  created_at    timestamptz not null default now(),
  updated_at    timestamptz not null default now(),
  deleted_at    timestamptz,

  -- soft delete is a status, and the timestamp must agree with it    [P9]
  constraint ck_post_deleted_at
    check ((status = 'deleted') = (deleted_at is not null))
);

alter table app.post set (fillfactor = 90);                          -- [P39]

create trigger trg_post_touch
  before update on app.post
  for each row execute function app.tg_touch();

create trigger trg_post_status_machine
  before update of status on app.post
  for each row when (old.status is distinct from new.status)
  execute function app.tg_enforce_transition(
    'status',
    'published>archived', 'archived>published',
    'published>deleted',  'archived>deleted',
    'deleted>published');   -- restore within the recovery window (checked in api)

-- THE hot-path index: profile grids and home-feed fan-in both scan
-- "live posts of author X, newest first". Partial: archived, deleted and
-- moderated rows never pollute it.                              [P14][P22]
create index idx_post_author_live
  on app.post (author_id, created_at desc, id desc)
  where status = 'published' and moderation = 'visible';

-- location pages scan by place
create index idx_post_location_live
  on app.post (location_id, created_at desc, id desc)
  where location_id is not null
    and status = 'published' and moderation = 'visible';

-- profile.post_count counts a condition (live posts), so it moves on
-- INSERT, on soft-delete/archive/restore, and on moderation flips.  [P15]
create function app.tg_post_count() returns trigger
language plpgsql as $$
declare
  was int := case when tg_op = 'INSERT' then 0
                  else (old.status = 'published' and old.moderation = 'visible')::int end;
  now_ int := case when tg_op = 'DELETE' then 0
                   else (new.status = 'published' and new.moderation = 'visible')::int end;
begin
  if now_ <> was then
    update app.profile
       set post_count = post_count + (now_ - was)
     where id = coalesce(new.author_id, old.author_id);
  end if;
  return null;
end $$;

create trigger trg_post_count
  after insert or delete or update of status, moderation on app.post
  for each row execute function app.tg_post_count();

-- carousel: ordered media under one post
create table app.post_media (
  post_id        uuid not null references app.post (id) on delete cascade,
  position       int  not null,
  media_asset_id uuid not null references app.media_asset (id),

  constraint pk_post_media primary key (post_id, position),          -- [P8]
  constraint uq_post_media_asset unique (post_id, media_asset_id),
  constraint ck_post_media_position check (position between 0 and 9)
);
-- FK stance: children of the post aggregate CASCADE; everything else in
-- this schema is RESTRICT, because user-facing deletion is soft and hard
-- deletes happen only in controlled purge paths.


-- ─────────────────────────────────────────────────────────────────────────────
-- §8  Interactions: likes, comments, saves, collections, media tags
-- ─────────────────────────────────────────────────────────────────────────────

create table app.post_like (
  profile_id uuid not null references app.profile (id),
  post_id    uuid not null references app.post (id),
  created_at timestamptz not null default now(),

  -- "a user can like a post at most once" is the primary key, not code [P8]
  constraint pk_post_like primary key (profile_id, post_id)
);

create index idx_post_like_reverse on app.post_like (post_id);

-- like_count via *statement-level* triggers with transition tables: one
-- UPDATE per statement no matter how many rows, which also makes bulk
-- backfills and purges counter-safe by construction.                [P15]
create function app.tg_like_count_ins() returns trigger
language plpgsql as $$
begin
  update app.post p
     set like_count = p.like_count + n.c
    from (select post_id, count(*)::int c from new_rows group by 1) n
   where p.id = n.post_id;
  return null;
end $$;

create function app.tg_like_count_del() returns trigger
language plpgsql as $$
begin
  update app.post p
     set like_count = p.like_count - o.c
    from (select post_id, count(*)::int c from old_rows group by 1) o
   where p.id = o.post_id;
  return null;
end $$;

create trigger trg_like_count_ins
  after insert on app.post_like
  referencing new table as new_rows
  for each statement execute function app.tg_like_count_ins();

create trigger trg_like_count_del
  after delete on app.post_like
  referencing old table as old_rows
  for each statement execute function app.tg_like_count_del();

create table app.comment (
  id            uuid primary key default app.uuid_v7(),
  post_id       uuid not null references app.post (id),
  author_id     uuid not null references app.profile (id),
  parent_id     uuid references app.comment (id),
  -- who is being replied to, captured at write time: survives parent
  -- deletion and later renames (PRD, "Reply to Comment")
  reply_to_id   uuid references app.profile (id),
  body          app.comment_body not null,
  moderation    app.moderation_status not null default 'visible',
  created_at    timestamptz not null default now(),
  deleted_at    timestamptz
);

-- comment pagination: ascending keyset over live rows of a post      [P22]
create index idx_comment_page
  on app.comment (post_id, created_at, id)
  where deleted_at is null;

-- post.comment_count counts *visible, undeleted* comments — so soft
-- deletes and moderation flips adjust it, not just inserts.          [P15]
create function app.tg_comment_count() returns trigger
language plpgsql as $$
declare
  was int := case when tg_op = 'INSERT' then 0
                  else (old.deleted_at is null and old.moderation = 'visible')::int end;
  now_ int := case when tg_op = 'DELETE' then 0
                   else (new.deleted_at is null and new.moderation = 'visible')::int end;
begin
  if now_ <> was then
    update app.post
       set comment_count = comment_count + (now_ - was)
     where id = coalesce(new.post_id, old.post_id);
  end if;
  return null;
end $$;

create trigger trg_comment_count
  after insert or delete or update of deleted_at, moderation on app.comment
  for each row execute function app.tg_comment_count();

create table app.save (
  profile_id uuid not null references app.profile (id),
  post_id    uuid not null references app.post (id),
  created_at timestamptz not null default now(),

  constraint pk_save primary key (profile_id, post_id)
);

create index idx_save_reverse on app.save (post_id);

create function app.tg_save_count_ins() returns trigger
language plpgsql as $$
begin
  update app.post p set save_count = p.save_count + n.c
    from (select post_id, count(*)::int c from new_rows group by 1) n
   where p.id = n.post_id;
  return null;
end $$;

create function app.tg_save_count_del() returns trigger
language plpgsql as $$
begin
  update app.post p set save_count = p.save_count - o.c
    from (select post_id, count(*)::int c from old_rows group by 1) o
   where p.id = o.post_id;
  return null;
end $$;

create trigger trg_save_count_ins
  after insert on app.save
  referencing new table as new_rows
  for each statement execute function app.tg_save_count_ins();

create trigger trg_save_count_del
  after delete on app.save
  referencing old table as old_rows
  for each statement execute function app.tg_save_count_del();

create table app.collection (
  id               uuid primary key default app.uuid_v7(),
  owner_profile_id uuid not null references app.profile (id),
  name             app.collection_name not null,
  cover_post_id    uuid references app.post (id),
  created_at       timestamptz not null default now(),
  updated_at       timestamptz not null default now(),

  constraint uq_collection_owner_name unique (owner_profile_id, name),
  -- superkey for the composite-FK trick below                        [P11]
  constraint uq_collection_id_owner unique (id, owner_profile_id)
);

create trigger trg_collection_touch
  before update on app.collection
  for each row execute function app.tg_touch();

-- Filing a post into a collection. Two composite FKs make three business
-- rules pure DDL:                                                   [P11]
--   1. you can only file a post you have SAVED
--      (owner_profile_id, post_id) → save
--   2. you can only file into a collection you OWN
--      (collection_id, owner_profile_id) → collection's superkey
--   3. unsaving a post removes it from every collection — the FK to save
--      cascades, no application code involved
create table app.collection_item (
  collection_id    uuid not null,
  post_id          uuid not null,
  owner_profile_id uuid not null,
  added_at         timestamptz not null default now(),

  constraint pk_collection_item primary key (collection_id, post_id),
  constraint fk_item_saved
    foreign key (owner_profile_id, post_id)
    references app.save (profile_id, post_id) on delete cascade,
  constraint fk_item_owned_collection
    foreign key (collection_id, owner_profile_id)
    references app.collection (id, owner_profile_id) on delete cascade
);

create index idx_collection_item_owner on app.collection_item (owner_profile_id);

create table app.media_tag (
  id                uuid primary key default app.uuid_v7(),
  post_id           uuid not null references app.post (id),
  media_asset_id    uuid not null references app.media_asset (id),
  tagged_profile_id uuid not null references app.profile (id),
  x                 real not null,
  y                 real not null,
  created_at        timestamptz not null default now(),
  removed_at        timestamptz,   -- tagged user removed themselves

  constraint ck_media_tag_pos
    check (x between 0.0 and 1.0 and y between 0.0 and 1.0)
);

-- one *active* tag per person per media item, while removed tags remain
-- as history (audit, "don't re-add silently")                        [P10]
create unique index uq_media_tag_active
  on app.media_tag (media_asset_id, tagged_profile_id)
  where removed_at is null;

-- tagged-posts surface reads only active tags
create index idx_media_tag_by_profile
  on app.media_tag (tagged_profile_id, created_at desc)
  where removed_at is null;

-- ─────────────────────────────────────────────────────────────────────────────
-- §9  Annotations: mentions, hashtags
-- ─────────────────────────────────────────────────────────────────────────────

-- A mention is a structured reference (profile id), not text — renames
-- cannot break it; `handle` preserves what the author typed.
create table app.mention (
  id                   uuid primary key default app.uuid_v7(),
  -- polymorphic source: exactly one of post/comment                  [P13]
  post_id              uuid references app.post (id),
  comment_id           uuid references app.comment (id),
  author_profile_id    uuid not null references app.profile (id),
  mentioned_profile_id uuid not null references app.profile (id),
  handle               text not null,
  created_at           timestamptz not null default now(),

  constraint ck_mention_one_source check (num_nonnulls(post_id, comment_id) = 1)
);

-- dedupe per source: partial uniques, one per polymorphic arm        [P10]
create unique index uq_mention_in_post
  on app.mention (post_id, mentioned_profile_id) where post_id is not null;
create unique index uq_mention_in_comment
  on app.mention (comment_id, mentioned_profile_id) where comment_id is not null;

create index idx_mention_by_profile
  on app.mention (mentioned_profile_id, created_at desc);
create index idx_mention_by_post    on app.mention (post_id);
create index idx_mention_by_comment on app.mention (comment_id);

-- hashtags are stored normalized; the domain CHECK enforces it at rest
create table app.hashtag (
  tag        app.hashtag_text primary key,
  created_at timestamptz not null default now()
);

create table app.post_hashtag (
  post_id uuid not null references app.post (id) on delete cascade,
  tag     app.hashtag_text not null references app.hashtag (tag),

  constraint pk_post_hashtag primary key (post_id, tag)
);

create index idx_post_hashtag_by_tag on app.post_hashtag (tag);


-- ─────────────────────────────────────────────────────────────────────────────
-- §10  Policy predicates
--      STABLE LANGUAGE sql, no SET clause — so the planner can inline them
--      into calling queries instead of calling them per row.         [P25]
--      They relate profiles (domain objects); resolving "who is asking"
--      happens at the API boundary. p_viewer NULL means anonymous.
-- ─────────────────────────────────────────────────────────────────────────────

create function app.is_blocked_between(p_a uuid, p_b uuid) returns boolean
language sql stable as $$
  select p_a is not null and exists (
    select 1 from app.block b
     where (b.blocker_id = p_a and b.blocked_id = p_b)
        or (b.blocker_id = p_b and b.blocked_id = p_a))
$$;

create function app.is_follower(p_viewer uuid, p_profile uuid) returns boolean
language sql stable as $$
  select p_viewer is not null and exists (
    select 1 from app.follow f
     where f.follower_id = p_viewer and f.followee_id = p_profile)
$$;

-- profile shell: visible unless dead or blocked (private profiles show
-- their summary; their content is gated separately)
create function app.can_view_profile(p_viewer uuid, p_profile uuid) returns boolean
language sql stable as $$
  select exists (
    select 1 from app.profile pr
     where pr.id = p_profile
       and pr.is_active
       and not app.is_blocked_between(p_viewer, pr.id))
$$;

-- profile content: public, or self, or approved follower
create function app.can_view_content_of(p_viewer uuid, p_profile uuid) returns boolean
language sql stable as $$
  select exists (
    select 1 from app.profile pr
     where pr.id = p_profile
       and pr.is_active
       and not app.is_blocked_between(p_viewer, pr.id)
       and (pr.visibility = 'public'
            or pr.id = p_viewer
            or app.is_follower(p_viewer, pr.id)))
$$;

-- ONE post-visibility predicate serves feed, grid, detail, hashtag page,
-- location page, saved posts, tagged posts, notifications, and direct
-- links. Fix a leak here, fix it everywhere.
create function app.can_view_post(p_viewer uuid, p_post uuid) returns boolean
language sql stable as $$
  select exists (
    select 1 from app.post p
     where p.id = p_post
       and (p.author_id = p_viewer         -- author sees own archived/held
            or (p.status = 'published'
                and p.moderation = 'visible'
                and app.can_view_content_of(p_viewer, p.author_id))))
$$;

create function app.can_view_comment(p_viewer uuid, p_comment uuid) returns boolean
language sql stable as $$
  select exists (
    select 1
      from app.comment c
      join app.post p on p.id = c.post_id
     where c.id = p_comment
       and c.deleted_at is null
       and app.can_view_post(p_viewer, c.post_id)
       -- held (restricted) comments: visible only to their author and to
       -- the post owner — quieter than a block by design
       and (c.moderation = 'visible'
            or c.author_id = p_viewer
            or p.author_id = p_viewer))
$$;

create function app.can_comment(p_viewer uuid, p_post uuid) returns boolean
language sql stable as $$
  select exists (
    select 1
      from app.post p
      join app.profile author on author.id = p.author_id
     where p.id = p_post
       and app.can_view_post(p_viewer, p.id)
       and not app.is_blocked_between(p_viewer, p.author_id)
       and (p.author_id = p_viewer
            or author.comment_permission = 'everyone'
            or (author.comment_permission = 'followers'
                and app.is_follower(p_viewer, p.author_id))))
$$;

-- "who can mention/tag you": everyone / people YOU follow / no one —
-- note the direction: the *target* must follow the author for 'followed'
create function app.can_mention(p_author uuid, p_target uuid) returns boolean
language sql stable as $$
  select exists (
    select 1 from app.profile t
     where t.id = p_target
       and t.is_active
       and not app.is_blocked_between(p_author, t.id)
       and (t.mention_permission = 'everyone'
            or (t.mention_permission = 'followed'
                and app.is_follower(t.id, p_author))))
$$;

create function app.can_tag(p_author uuid, p_target uuid) returns boolean
language sql stable as $$
  select exists (
    select 1 from app.profile t
     where t.id = p_target
       and t.is_active
       and not app.is_blocked_between(p_author, t.id)
       and (t.tag_permission = 'everyone'
            or (t.tag_permission = 'followed'
                and app.is_follower(t.id, p_author))))
$$;

create function app.require_moderator(p_viewer uuid) returns void
language plpgsql stable as $$
begin
  if not exists (
    select 1 from app.profile pr
      join app.account a on a.id = pr.account_id
     where pr.id = p_viewer and a.is_moderator and a.status = 'active')
  then
    raise exception 'moderator access required' using errcode = 'IG403';
  end if;
end $$;


-- ─────────────────────────────────────────────────────────────────────────────
-- §11  Reports and notifications (+ activity triggers)
-- ─────────────────────────────────────────────────────────────────────────────

create table app.report (
  id                  uuid primary key default app.uuid_v7(),
  reporter_profile_id uuid not null references app.profile (id),
  -- exactly one target                                               [P13]
  post_id             uuid references app.post (id),
  comment_id          uuid references app.comment (id),
  profile_id          uuid references app.profile (id),
  reason              app.report_reason not null,
  note                text,
  status              app.report_status not null default 'open',
  outcome             app.report_outcome,
  reviewed_by         uuid references app.profile (id),
  created_at          timestamptz not null default now(),
  reviewed_at         timestamptz,

  constraint ck_report_one_target
    check (num_nonnulls(post_id, comment_id, profile_id) = 1),

  -- a stored generated column collapses the polymorphic target into one
  -- text key — a single UNIQUE bounds duplicate reports per reporter [P12]
  target_key text generated always as (
    coalesce('post:'    || post_id::text,
             'comment:' || comment_id::text,
             'profile:' || profile_id::text)
  ) stored,

  constraint uq_report_reporter_target unique (reporter_profile_id, target_key)
);

create trigger trg_report_status_machine
  before update of status on app.report
  for each row when (old.status is distinct from new.status)
  execute function app.tg_enforce_transition('status', 'open>reviewed');

-- the moderation queue reads open reports oldest-first               [P22]
create index idx_report_queue on app.report (created_at) where status = 'open';

-- The notification table is the outbox: existence here is the in-app
-- activity record; push delivery is a separate worker that reads
-- notification_prefs. bigint identity PK: cheapest possible index for an
-- append-only table, and a monotonic keyset cursor for free.
create table app.notification (
  id                   bigint generated always as identity primary key,
  recipient_profile_id uuid not null references app.profile (id),
  kind                 app.notification_kind not null,
  actor_profile_id     uuid references app.profile (id),
  post_id              uuid references app.post (id),
  comment_id           uuid references app.comment (id),
  report_id            uuid references app.report (id),
  created_at           timestamptz not null default now(),
  read_at              timestamptz,

  -- logical-activity identity: retried triggers and repeated actions
  -- (unlike→re-like) collapse into one row                     [P12][P17]
  source_key text generated always as (
    coalesce('post:'    || post_id::text,
             'comment:' || comment_id::text,
             'report:'  || report_id::text,
             'actor:'   || actor_profile_id::text)
  ) stored,

  constraint uq_notification_dedupe
    unique (recipient_profile_id, kind, source_key)
);

create index idx_notification_page
  on app.notification (recipient_profile_id, id desc);
create index idx_notification_unread                                 -- [P22]
  on app.notification (recipient_profile_id) where read_at is null;

-- single chokepoint for creating activity: silence self-notifications,
-- respect blocks, dedupe, and hint the delivery worker            [P38]
create function app.push_notification(
  p_recipient uuid, p_kind app.notification_kind, p_actor uuid,
  p_post uuid default null, p_comment uuid default null, p_report uuid default null
) returns void
language plpgsql as $$
declare
  v_id bigint;
begin
  if p_recipient is null
     or p_recipient = p_actor
     or app.is_blocked_between(p_actor, p_recipient) then
    return;
  end if;

  insert into app.notification
    (recipient_profile_id, kind, actor_profile_id, post_id, comment_id, report_id)
  values (p_recipient, p_kind, p_actor, p_post, p_comment, p_report)
  on conflict on constraint uq_notification_dedupe do nothing        -- [P17]
  returning id into v_id;

  if v_id is not null then
    -- payload is a hint, not the data: the worker re-reads the row (and
    -- notification_prefs) — pg_notify payloads are capped at 8k anyway
    perform pg_notify('app_notification',
                      json_build_object('id', v_id, 'recipient', p_recipient)::text);
  end if;
end $$;

-- activity fan-out. NOTE the split of responsibilities: triggers may only
-- append derived records (notifications); domain consequences (block
-- severing follows) live in api functions where they are explicit.
create function app.tg_notify_follow() returns trigger
language plpgsql as $$
begin
  perform app.push_notification(new.followee_id, 'follow', new.follower_id);
  return null;
end $$;

create trigger trg_notify_follow
  after insert on app.follow
  for each row execute function app.tg_notify_follow();

create function app.tg_notify_follow_request() returns trigger
language plpgsql as $$
begin
  if tg_op = 'INSERT' or (old.status <> 'pending' and new.status = 'pending') then
    perform app.push_notification(new.target_id, 'follow_request', new.requester_id);
  elsif tg_op = 'UPDATE' and new.status = 'approved' then
    perform app.push_notification(new.requester_id, 'follow_request_approved', new.target_id);
  end if;
  -- denial is deliberately silent (PRD: deny must not signal anything)
  return null;
end $$;

create trigger trg_notify_follow_request
  after insert or update of status on app.follow_request
  for each row execute function app.tg_notify_follow_request();

create function app.tg_notify_like() returns trigger
language plpgsql as $$
begin
  perform app.push_notification(
    (select author_id from app.post where id = new.post_id),
    'like', new.profile_id, p_post => new.post_id);
  return null;
end $$;

create trigger trg_notify_like
  after insert on app.post_like
  for each row execute function app.tg_notify_like();
-- (post_like carries two triggers of different granularity: statement-level
-- for the counter, row-level for activity. Both are legitimate.)

create function app.tg_notify_comment() returns trigger
language plpgsql as $$
declare
  v_post_author uuid := (select author_id from app.post where id = new.post_id);
begin
  if new.moderation <> 'visible' then
    return null;   -- held comments notify no one until approved
  end if;
  perform app.push_notification(
    v_post_author,
    case when new.parent_id is null then 'comment' else 'reply' end::app.notification_kind,
    new.author_id, p_post => new.post_id, p_comment => new.id);
  if new.reply_to_id is not null and new.reply_to_id <> v_post_author
     and app.can_view_post(new.reply_to_id, new.post_id) then
    perform app.push_notification(new.reply_to_id, 'reply', new.author_id,
                                  p_post => new.post_id, p_comment => new.id);
  end if;
  return null;
end $$;

create trigger trg_notify_comment
  after insert on app.comment
  for each row execute function app.tg_notify_comment();

create function app.tg_notify_mention() returns trigger
language plpgsql as $$
declare
  v_post uuid := coalesce(new.post_id,
                          (select post_id from app.comment where id = new.comment_id));
begin
  -- a mention from a private post must not signal content the target
  -- cannot open (PRD, "Notifications")
  if app.can_view_post(new.mentioned_profile_id, v_post) then
    perform app.push_notification(new.mentioned_profile_id, 'mention',
                                  new.author_profile_id,
                                  p_post => v_post, p_comment => new.comment_id);
  end if;
  return null;
end $$;

create trigger trg_notify_mention
  after insert on app.mention
  for each row execute function app.tg_notify_mention();

create function app.tg_notify_media_tag() returns trigger
language plpgsql as $$
begin
  if app.can_view_post(new.tagged_profile_id, new.post_id) then
    perform app.push_notification(
      new.tagged_profile_id, 'media_tag',
      (select author_id from app.post where id = new.post_id),
      p_post => new.post_id);
  end if;
  return null;
end $$;

create trigger trg_notify_media_tag
  after insert on app.media_tag
  for each row execute function app.tg_notify_media_tag();

-- ─────────────────────────────────────────────────────────────────────────────
-- §12  Write API
--      SECURITY DEFINER functions with a pinned search_path (search-path
--      hijack hardening); the ig_api role gets EXECUTE on these and no
--      table privileges at all — the API is functions, not tables.   [P35]
--      p_viewer is the caller's profile id, resolved by api.auth().
-- ─────────────────────────────────────────────────────────────────────────────

-- ---- auth ----

create function api.sign_up(
  p_email text, p_password text, p_username text
) returns table (account_id uuid, profile_id uuid)
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_account uuid;
  v_profile uuid;
  v_constraint text;
begin
  if length(p_password) < 8 then
    raise exception 'password too short' using errcode = 'IG422';
  end if;

  insert into app.account (email, password_hash)
  values (p_email, ext.crypt(p_password, ext.gen_salt('bf', 12)))   -- [P6]
  returning id into v_account;

  insert into app.profile (account_id, username)
  values (v_account, lower(p_username))   -- domain CHECK validates   [P3]
  returning id into v_profile;

  return query select v_account, v_profile;
exception
  when unique_violation then
    -- constraint names are part of the contract: map them precisely [P40]
    get stacked diagnostics v_constraint = constraint_name;
    case v_constraint
      when 'uq_account_email'    then
        raise exception 'email already registered' using errcode = 'IG409';
      when 'uq_profile_username' then
        raise exception 'username already taken'   using errcode = 'IG409';
      else raise;
    end case;
  when check_violation then
    raise exception 'invalid username' using errcode = 'IG422',
      hint = '3-30 characters: a-z, 0-9, dot, underscore';
end $$;

create function api.sign_in(p_email text, p_password text)
returns table (token text, profile_id uuid)
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_account app.account%rowtype;
  v_token   text;
begin
  select * into v_account from app.account a where a.email = p_email;

  -- one error for every failure mode: no account/credential enumeration
  if v_account.id is null
     or v_account.password_hash is null
     or v_account.password_hash <> ext.crypt(p_password, v_account.password_hash) then
    raise exception 'invalid credentials' using errcode = 'IG401';
  end if;

  -- lifecycle: self-deactivation reactivates on sign-in; a moderation
  -- 'disabled' does not; deletion_pending cancels within the grace window
  if v_account.status = 'deactivated' then
    update app.account set status = 'active' where id = v_account.id;
  elsif v_account.status = 'deletion_pending'
        and v_account.deletion_due_at > now() then
    update app.account set status = 'active', deletion_due_at = null
     where id = v_account.id;
  elsif v_account.status <> 'active' then
    raise exception 'invalid credentials' using errcode = 'IG401';
  end if;

  v_token := encode(ext.gen_random_bytes(32), 'hex');
  insert into app.session (account_id, token_hash, expires_at)
  values (v_account.id, ext.digest(v_token, 'sha256'), now() + interval '30 days');

  return query
    select v_token, pr.id from app.profile pr
     where pr.account_id = v_account.id;
end $$;

-- resolve a bearer token → profile id, and pin it into the RLS GUC for the
-- rest of the transaction (PostgREST-style pre-request hook)   [P7][P37]
create function api.auth(p_token text) returns uuid
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_profile uuid;
begin
  select pr.id into v_profile
    from app.session s
    join app.account a on a.id = s.account_id
    join app.profile pr on pr.account_id = a.id
   where s.token_hash = ext.digest(p_token, 'sha256')
     and s.expires_at > now()
     and s.revoked_at is null
     and a.status = 'active';

  if v_profile is null then
    raise exception 'invalid or expired session' using errcode = 'IG401';
  end if;

  perform set_config('app.profile_id', v_profile::text, true);  -- txn-local
  return v_profile;
end $$;

create function api.sign_out(p_token text) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  update app.session
     set revoked_at = now()
   where token_hash = ext.digest(p_token, 'sha256')
     and revoked_at is null
$$;

create function api.request_password_recovery(p_email text) returns text
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_account uuid;
  v_token   text := encode(ext.gen_random_bytes(32), 'hex');
begin
  select id into v_account from app.account
   where email = p_email and status in ('active', 'deactivated');
  if v_account is not null then
    perform app.hit_rate_limit(v_account, 'recovery', 5, interval '1 hour');
    insert into app.recovery_token (account_id, token_hash, expires_at)
    values (v_account, ext.digest(v_token, 'sha256'), now() + interval '1 hour');
    -- v_token goes out via the mailer; returned here for the exercise.
    return v_token;
  end if;
  return null;   -- indistinguishable from success: no email enumeration
end $$;

create function api.reset_password(p_token text, p_new_password text) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_rec app.recovery_token%rowtype;
begin
  if length(p_new_password) < 8 then
    raise exception 'password too short' using errcode = 'IG422';
  end if;

  -- FOR UPDATE serializes concurrent redemption of the same token     [P7]
  select * into v_rec from app.recovery_token
   where token_hash = ext.digest(p_token, 'sha256')
   for update;

  if v_rec.id is null or v_rec.consumed_at is not null or v_rec.expires_at < now() then
    raise exception 'invalid or expired recovery token' using errcode = 'IG401';
  end if;

  update app.recovery_token set consumed_at = now() where id = v_rec.id;
  update app.account
     set password_hash = ext.crypt(p_new_password, ext.gen_salt('bf', 12))
   where id = v_rec.account_id;
  -- credential change invalidates every live session
  update app.session set revoked_at = now()
   where account_id = v_rec.account_id and revoked_at is null;
end $$;

-- ---- account & profile lifecycle ----

create function api.change_username(p_viewer uuid, p_username text) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  update app.profile set username = lower(p_username) where id = p_viewer;
  -- mention/tag references are profile-id based: renames break nothing
exception
  when unique_violation then
    raise exception 'username already taken' using errcode = 'IG409';
  when check_violation then
    raise exception 'invalid username' using errcode = 'IG422';
end $$;

create function api.deactivate_account(p_viewer uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_account uuid := (select account_id from app.profile where id = p_viewer);
begin
  update app.account set status = 'deactivated'
   where id = v_account and status = 'active';
  update app.session set revoked_at = now()
   where account_id = v_account and revoked_at is null;
end $$;

create function api.request_account_deletion(p_viewer uuid) returns timestamptz
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_account uuid := (select account_id from app.profile where id = p_viewer);
  v_due timestamptz := now() + interval '30 days';
begin
  update app.account
     set status = 'deletion_pending', deletion_due_at = v_due
   where id = v_account and status in ('active', 'deactivated');
  update app.session set revoked_at = now()
   where account_id = v_account and revoked_at is null;
  return v_due;   -- purge happens in §15, from cron, after this instant
end $$;

-- ---- social graph ----

create function api.follow(p_viewer uuid, p_target uuid) returns text
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_target app.profile%rowtype;
begin
  perform app.hit_rate_limit(p_viewer, 'follow', 60, interval '1 hour');

  if p_viewer = p_target then
    raise exception 'cannot follow yourself' using errcode = 'IG422';
  end if;

  select * into v_target from app.profile where id = p_target and is_active;
  if v_target.id is null
     or app.is_blocked_between(p_viewer, p_target) then
    -- blocked reads as nonexistent: no probe signal                  [P27]
    raise exception 'profile not found' using errcode = 'IG404';
  end if;

  if v_target.visibility = 'private'
     and not app.is_follower(p_viewer, p_target) then
    insert into app.follow_request as fr (requester_id, target_id)
    values (p_viewer, p_target)
    on conflict (requester_id, target_id) do update
      set status = 'pending', responded_at = null            -- re-request
      where fr.status = 'denied';
    return 'requested';
  end if;

  insert into app.follow (follower_id, followee_id)
  values (p_viewer, p_target)
  on conflict do nothing;                                     -- [P17]
  return 'following';
end $$;

create function api.unfollow(p_viewer uuid, p_target uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  delete from app.follow
   where follower_id = p_viewer and followee_id = p_target;
  delete from app.follow_request
   where requester_id = p_viewer and target_id = p_target and status = 'pending';
$$;

create function api.approve_follow_request(p_viewer uuid, p_requester uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_req app.follow_request%rowtype;
begin
  -- lock the request row: concurrent approve/deny cannot interleave
  select * into v_req from app.follow_request
   where requester_id = p_requester and target_id = p_viewer
   for update;

  if v_req.requester_id is null then
    raise exception 'no such request' using errcode = 'IG404';
  end if;
  if v_req.status = 'approved' then
    return;   -- idempotent replay
  end if;

  update app.follow_request
     set status = 'approved', responded_at = now()
   where requester_id = p_requester and target_id = p_viewer;

  insert into app.follow (follower_id, followee_id)
  values (p_requester, p_viewer)
  on conflict do nothing;
end $$;

create function api.deny_follow_request(p_viewer uuid, p_requester uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  update app.follow_request
     set status = 'denied', responded_at = now()
   where requester_id = p_requester and target_id = p_viewer
     and status = 'pending';
  -- zero rows ⇒ already decided or never existed: both fine, idempotent
end $$;

create function api.block_profile(p_viewer uuid, p_target uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  if p_viewer = p_target then
    raise exception 'cannot block yourself' using errcode = 'IG422';
  end if;

  insert into app.block (blocker_id, blocked_id)
  values (p_viewer, p_target)
  on conflict do nothing;

  -- domain consequences are explicit, here, not hidden in a trigger:
  -- blocking severs the relationship in both directions
  delete from app.follow
   where (follower_id = p_viewer and followee_id = p_target)
      or (follower_id = p_target and followee_id = p_viewer);
  delete from app.follow_request
   where status = 'pending'
     and ((requester_id = p_viewer and target_id = p_target)
       or (requester_id = p_target and target_id = p_viewer));
end $$;

create function api.unblock_profile(p_viewer uuid, p_target uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- unblocking does NOT restore severed follows (PRD)
  delete from app.block where blocker_id = p_viewer and blocked_id = p_target
$$;

create function api.restrict_profile(p_viewer uuid, p_target uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  insert into app.restriction (restrictor_id, restricted_id)
  values (p_viewer, p_target)
  on conflict do nothing
$$;

create function api.unrestrict_profile(p_viewer uuid, p_target uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  delete from app.restriction
   where restrictor_id = p_viewer and restricted_id = p_target
$$;

-- ---- media & posts ----

create function api.create_media(
  p_viewer uuid, p_storage_key text, p_kind app.media_kind
) returns uuid
language sql security definer set search_path = app, ext, pg_temp as $$
  -- the client uploads to object storage out of band; this registers the
  -- object. A worker drives uploaded→processing→ready|failed (§6 machine).
  insert into app.media_asset (owner_id, storage_key, kind)
  values (p_viewer, p_storage_key, p_kind)
  returning id
$$;

-- shared by publish and edit: extract @handles, resolve to profiles,
-- store structured references. Set-based, one statement.       [P29][P30]
create function app.write_mentions(
  p_author uuid, p_text text, p_post uuid, p_comment uuid
) returns void
language sql as $$
  insert into app.mention
    (post_id, comment_id, author_profile_id, mentioned_profile_id, handle)
  select p_post, p_comment, p_author, pr.id, h.handle
    from (select distinct lower(m[1]) as handle
            from regexp_matches(coalesce(p_text, ''),
                                '@([A-Za-z0-9._]{3,30})', 'g') m
           limit 10) h                       -- product cap on mentions
    join app.profile pr on pr.username = h.handle
   where app.can_mention(p_author, pr.id)    -- disallowed ⇒ stays plain text
  on conflict do nothing
$$;

create function app.write_hashtags(p_post uuid, p_text text) returns void
language sql as $$
  -- upsert the dimension, then link. Deliberately NOT chained through
  -- ON CONFLICT…RETURNING: it returns nothing for pre-existing tags, so
  -- re-select from the parsed set instead.                     [P29][P30]
  with tags as (
    select distinct lower(m[1])::app.hashtag_text as tag
      from regexp_matches(coalesce(p_text, ''), '#([A-Za-z0-9_]{1,64})', 'g') m
     limit 30
  ),
  dim as (
    insert into app.hashtag (tag)
    select tag from tags
    on conflict do nothing
  )
  insert into app.post_hashtag (post_id, tag)
  select p_post, tag from tags
  on conflict do nothing
$$;

create function app.write_media_tags(
  p_author uuid, p_post uuid, p_tags jsonb
) returns void
language plpgsql as $$
declare
  v_bad int;
begin
  if p_tags is null or jsonb_array_length(p_tags) = 0 then
    return;
  end if;

  -- every tagged media item must belong to this post
  select count(*) into v_bad
    from jsonb_to_recordset(p_tags)
           as t(media_asset_id uuid, tagged_profile_id uuid, x real, y real)
   where not exists (select 1 from app.post_media pm
                      where pm.post_id = p_post
                        and pm.media_asset_id = t.media_asset_id);
  if v_bad > 0 then
    raise exception 'tag references media not on this post' using errcode = 'IG422';
  end if;

  -- tag permissions are enforced at compose time, loudly (PRD)
  select count(*) into v_bad
    from jsonb_to_recordset(p_tags)
           as t(media_asset_id uuid, tagged_profile_id uuid, x real, y real)
   where not app.can_tag(p_author, t.tagged_profile_id);
  if v_bad > 0 then
    raise exception 'a tagged account does not allow tags from you'
      using errcode = 'IG403';
  end if;

  insert into app.media_tag (post_id, media_asset_id, tagged_profile_id, x, y)
  select p_post, t.media_asset_id, t.tagged_profile_id, t.x, t.y
    from jsonb_to_recordset(p_tags)
           as t(media_asset_id uuid, tagged_profile_id uuid, x real, y real)
  -- ON CONFLICT against a *partial* unique index: predicate must match [P10]
  on conflict (media_asset_id, tagged_profile_id) where removed_at is null
  do nothing;
end $$;

create function api.publish_post(
  p_viewer uuid,
  p_media uuid[],                -- ordered: array position = carousel slot
  p_caption text default null,
  p_location uuid default null,
  p_tags jsonb default null      -- [{media_asset_id, tagged_profile_id, x, y}]
) returns uuid
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_post uuid;
begin
  perform app.hit_rate_limit(p_viewer, 'publish', 20, interval '1 hour');

  if p_media is null or array_length(p_media, 1) not between 1 and 10 then
    raise exception 'a post carries 1 to 10 media items' using errcode = 'IG422';
  end if;

  -- all media must be owned by the author and fully processed: a failed
  -- or pending upload can never become a published post (PRD)
  if exists (
    select 1 from unnest(p_media) mid
    left join app.media_asset ma
           on ma.id = mid and ma.owner_id = p_viewer and ma.status = 'ready'
    where ma.id is null)
  then
    raise exception 'media missing, not yours, or not ready' using errcode = 'IG422';
  end if;

  insert into app.post (author_id, caption, location_id)
  values (p_viewer, p_caption, p_location)
  returning id into v_post;

  -- array order → carousel order                                     [P23]
  insert into app.post_media (post_id, position, media_asset_id)
  select v_post, ord - 1, mid
    from unnest(p_media) with ordinality as u(mid, ord);

  perform app.write_mentions(p_viewer, p_caption, v_post, null);
  perform app.write_hashtags(v_post, p_caption);
  perform app.write_media_tags(p_viewer, v_post, p_tags);
  return v_post;
end $$;

create function api.edit_post(
  p_viewer uuid, p_post uuid,
  p_caption text default null,
  p_location uuid default null,
  p_tags jsonb default null
) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  -- ownership gate; media files are immutable after publish (PRD)
  if not exists (select 1 from app.post
                  where id = p_post and author_id = p_viewer
                    and status <> 'deleted') then
    raise exception 'post not found' using errcode = 'IG404';       -- [P27]
  end if;

  update app.post
     set caption = p_caption, location_id = p_location
   where id = p_post;
  -- likes, comments, saves, created_at untouched by construction:
  -- nothing here writes them (PRD, "Edit Post")

  -- caption edits re-parse annotations
  delete from app.mention where post_id = p_post;
  delete from app.post_hashtag where post_id = p_post;
  perform app.write_mentions(p_viewer, p_caption, p_post, null);
  perform app.write_hashtags(p_post, p_caption);

  if p_tags is not null then
    delete from app.media_tag where post_id = p_post and removed_at is null;
    perform app.write_media_tags(p_viewer, p_post, p_tags);
  end if;
end $$;

-- lifecycle verbs: idempotent by guarded UPDATE — replays match zero rows
-- and the state machine (§7) rejects any illegal jump             [P17][P18]
create function api.archive_post(p_viewer uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  update app.post set status = 'archived'
   where id = p_post and author_id = p_viewer and status = 'published'
$$;

create function api.unarchive_post(p_viewer uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  update app.post set status = 'published'
   where id = p_post and author_id = p_viewer and status = 'archived'
$$;

create function api.delete_post(p_viewer uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- soft delete: likes/comments/saves/tags/mentions survive for audit and
  -- counter repair; every read path already excludes deleted posts  [P14]
  update app.post set status = 'deleted', deleted_at = now()
   where id = p_post and author_id = p_viewer and status <> 'deleted'
$$;

create function api.restore_post(p_viewer uuid, p_post uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_deleted_at timestamptz;
begin
  select deleted_at into v_deleted_at from app.post
   where id = p_post and author_id = p_viewer and status = 'deleted';
  if v_deleted_at is null then
    raise exception 'post not found' using errcode = 'IG404';
  end if;
  if v_deleted_at < now() - interval '30 days' then
    raise exception 'recovery window has passed' using errcode = 'IG422';
  end if;
  update app.post set status = 'published', deleted_at = null where id = p_post;
end $$;

-- ---- interactions ----

create function api.like_post(p_viewer uuid, p_post uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  perform app.hit_rate_limit(p_viewer, 'like', 300, interval '1 hour');
  if not app.can_view_post(p_viewer, p_post) then
    raise exception 'post not found' using errcode = 'IG404';       -- [P27]
  end if;
  insert into app.post_like (profile_id, post_id)
  values (p_viewer, p_post)
  on conflict do nothing;                                            -- [P17]
end $$;

create function api.unlike_post(p_viewer uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  delete from app.post_like where profile_id = p_viewer and post_id = p_post
$$;
-- like/unlike replay safety: the counter triggers fire only on rows that
-- actually appear/disappear, so hammering these cannot corrupt counts.

create function api.add_comment(
  p_viewer uuid, p_post uuid, p_body text, p_parent uuid default null
) returns uuid
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_post app.post%rowtype;
  v_reply_to uuid;
  v_held boolean;
  v_comment uuid;
begin
  perform app.hit_rate_limit(p_viewer, 'comment', 60, interval '1 hour');

  select * into v_post from app.post where id = p_post;
  if not app.can_view_post(p_viewer, p_post) then
    raise exception 'post not found' using errcode = 'IG404';
  end if;
  if not app.can_comment(p_viewer, p_post) then
    raise exception 'comments are limited on this post' using errcode = 'IG403';
  end if;

  if p_parent is not null then
    select author_id into v_reply_to from app.comment
     where id = p_parent and post_id = p_post;    -- parent must be on-post
    if v_reply_to is null then
      raise exception 'parent comment not found' using errcode = 'IG404';
    end if;
  end if;

  -- restriction: the comment lands 'held' — visible to its author and the
  -- post owner only, until the owner approves it (quieter than a block)
  v_held := exists (select 1 from app.restriction
                     where restrictor_id = v_post.author_id
                       and restricted_id = p_viewer);

  insert into app.comment (post_id, author_id, parent_id, reply_to_id, body, moderation)
  values (p_post, p_viewer, p_parent, v_reply_to, p_body,
          case when v_held then 'held' else 'visible' end::app.moderation_status)
  returning id into v_comment;

  perform app.write_mentions(p_viewer, p_body, null, v_comment);
  return v_comment;
end $$;

create function api.approve_comment(p_viewer uuid, p_comment uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- post owner surfaces a held comment (no state machine here: moderation
  -- is an authority column, not a lifecycle)
  update app.comment c
     set moderation = 'visible'
    from app.post p
   where c.id = p_comment and p.id = c.post_id
     and p.author_id = p_viewer and c.moderation = 'held'
$$;

create function api.delete_comment(p_viewer uuid, p_comment uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- the comment author OR the post author may delete (PRD)
  update app.comment c
     set deleted_at = now()
    from app.post p
   where c.id = p_comment and p.id = c.post_id
     and c.deleted_at is null
     and (c.author_id = p_viewer or p.author_id = p_viewer)
$$;

create function api.remove_my_tag(p_viewer uuid, p_tag uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  update app.media_tag
     set removed_at = now()
   where id = p_tag and tagged_profile_id = p_viewer and removed_at is null
$$;

create function api.save_post(p_viewer uuid, p_post uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  if not app.can_view_post(p_viewer, p_post) then
    raise exception 'post not found' using errcode = 'IG404';
  end if;
  insert into app.save (profile_id, post_id)
  values (p_viewer, p_post)
  on conflict do nothing;
end $$;

create function api.unsave_post(p_viewer uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- the composite FK cascades this delete into every collection    [P11]
  delete from app.save where profile_id = p_viewer and post_id = p_post
$$;

create function api.create_collection(p_viewer uuid, p_name text) returns uuid
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_id uuid;
begin
  insert into app.collection (owner_profile_id, name)
  values (p_viewer, p_name)
  returning id into v_id;
  return v_id;
exception
  when unique_violation then
    raise exception 'you already have a collection named that' using errcode = 'IG409';
  when check_violation then
    raise exception 'invalid collection name' using errcode = 'IG422';
end $$;

create function api.rename_collection(p_viewer uuid, p_collection uuid, p_name text) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  update app.collection set name = p_name
   where id = p_collection and owner_profile_id = p_viewer;
exception
  when unique_violation then
    raise exception 'you already have a collection named that' using errcode = 'IG409';
end $$;

create function api.add_to_collection(p_viewer uuid, p_collection uuid, p_post uuid) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  insert into app.collection_item (collection_id, post_id, owner_profile_id)
  values (p_collection, p_post, p_viewer)
  on conflict do nothing;
exception
  when foreign_key_violation then
    -- either not your collection or a post you haven't saved: the two
    -- composite FKs (§8) are the authorization                     [P11]
    raise exception 'save the post first, in your own collection' using errcode = 'IG422';
end $$;

create function api.remove_from_collection(p_viewer uuid, p_collection uuid, p_post uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  delete from app.collection_item
   where collection_id = p_collection and post_id = p_post
     and owner_profile_id = p_viewer
$$;

create function api.delete_collection(p_viewer uuid, p_collection uuid) returns void
language sql security definer set search_path = app, ext, pg_temp as $$
  -- items cascade with the collection; the saves themselves remain (PRD:
  -- deleting a collection does not delete the underlying posts)
  delete from app.collection
   where id = p_collection and owner_profile_id = p_viewer
$$;

create function api.report(
  p_viewer uuid, p_reason app.report_reason, p_note text default null,
  p_post uuid default null, p_comment uuid default null, p_profile uuid default null
) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  perform app.hit_rate_limit(p_viewer, 'report', 20, interval '1 day');
  insert into app.report (reporter_profile_id, post_id, comment_id, profile_id, reason, note)
  values (p_viewer, p_post, p_comment, p_profile, p_reason, p_note)
  on conflict on constraint uq_report_reporter_target do nothing;   -- [P12]
  -- ck_report_one_target enforces exactly-one; num_nonnulls raises 23514
  -- for zero/multiple targets — let that bubble as a caller bug
end $$;

create function api.mark_notifications_read(p_viewer uuid, p_up_to bigint) returns int
language sql security definer set search_path = app, ext, pg_temp as $$
  with updated as (
    update app.notification
       set read_at = now()
     where recipient_profile_id = p_viewer
       and id <= p_up_to and read_at is null
    returning 1)
  select count(*)::int from updated                                  -- [P30]
$$;

-- ---- moderation ----

create function api.claim_next_report(p_viewer uuid)
returns table (report_id uuid, reason app.report_reason, note text,
               target_kind text, target_id uuid, created_at timestamptz)
language plpgsql security definer set search_path = app, ext, pg_temp as $$
begin
  perform app.require_moderator(p_viewer);
  return query
    select r.id, r.reason, r.note,
           split_part(r.target_key, ':', 1),
           coalesce(r.post_id, r.comment_id, r.profile_id),
           r.created_at
      from app.report r
     where r.status = 'open'
     order by r.created_at
     for update skip locked   -- N moderators, no collisions, no waiting [P31]
     limit 1;
end $$;

create function api.resolve_report(
  p_viewer uuid, p_report uuid, p_action app.report_outcome
) returns void
language plpgsql security definer set search_path = app, ext, pg_temp as $$
declare
  v_rep app.report%rowtype;
begin
  perform app.require_moderator(p_viewer);

  select * into v_rep from app.report where id = p_report for update;
  if v_rep.id is null or v_rep.status <> 'open' then
    raise exception 'report not open' using errcode = 'IG404';
  end if;

  if p_action in ('removed', 'limited') then
    if v_rep.post_id is not null then
      update app.post set moderation =
        case p_action when 'removed' then 'removed' else 'limited' end::app.moderation_status
       where id = v_rep.post_id;
    elsif v_rep.comment_id is not null then
      update app.comment set moderation =
        case p_action when 'removed' then 'removed' else 'limited' end::app.moderation_status
       where id = v_rep.comment_id;
    else
      -- profile target: removal means a moderation disable
      if p_action = 'removed' then
        update app.account a set status = 'disabled'
          from app.profile pr
         where pr.id = v_rep.profile_id and a.id = pr.account_id
           and a.status = 'active';
      end if;
    end if;
  end if;

  update app.report
     set status = 'reviewed', outcome = p_action,
         reviewed_by = p_viewer, reviewed_at = now()
   where id = p_report;

  -- the reporter learns *that* it was resolved, not the verdict detail
  perform app.push_notification(v_rep.reporter_profile_id, 'report_resolved',
                                null, p_report => p_report);
end $$;

-- ─────────────────────────────────────────────────────────────────────────────
-- §13  Read API
--      Keyset pagination everywhere: (created_at, id) row-value cursors
--      against matching DESC indexes. Offset pagination does not appear in
--      this file.                                                    [P21]
-- ─────────────────────────────────────────────────────────────────────────────

-- reusable nested payloads                                           [P23]
create function app.media_json(p_post uuid) returns jsonb
language sql stable as $$
  select coalesce(jsonb_agg(jsonb_build_object(
           'position',    pm.position,
           'kind',        ma.kind,
           'storage_key', ma.storage_key,
           'variants',    ma.variants,
           'width',       ma.width,
           'height',      ma.height,
           'duration_ms', ma.duration_ms
         ) order by pm.position), '[]'::jsonb)
    from app.post_media pm
    join app.media_asset ma on ma.id = pm.media_asset_id
   where pm.post_id = p_post
$$;

create function app.author_json(p_profile uuid) returns jsonb
language sql stable as $$
  select jsonb_build_object(
           'id', pr.id, 'username', pr.username,
           'display_name', pr.display_name, 'avatar_media_id', pr.avatar_media_id)
    from app.profile pr where pr.id = p_profile
$$;

create function api.home_feed(
  p_viewer uuid,
  p_cursor_ts timestamptz default null,
  p_cursor_id uuid default null,
  p_page int default 20
) returns table (
  post_id uuid, author jsonb, media jsonb, caption text, location_id uuid,
  created_at timestamptz, like_count int, comment_count int,
  viewer_has_liked boolean, viewer_has_saved boolean
)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select p.id,
         app.author_json(p.author_id),
         app.media_json(p.id),
         p.caption,
         p.location_id,
         p.created_at,
         p.like_count,
         p.comment_count,
         exists (select 1 from app.post_like l                       -- [P24]
                  where l.profile_id = p_viewer and l.post_id = p.id),
         exists (select 1 from app.save s
                  where s.profile_id = p_viewer and s.post_id = p.id)
    from app.post p
    join app.profile author on author.id = p.author_id
   where p.author_id in (select f.followee_id from app.follow f
                          where f.follower_id = p_viewer
                         union all
                         select p_viewer)          -- own posts appear too
     and p.status = 'published'
     and p.moderation = 'visible'
     and author.is_active
     -- Membership implies permission here: a follow row on a private
     -- profile IS the approval, and blocking severed any follow rows. The
     -- authoritative per-post gate (can_view_post) guards direct links.
     -- row-value keyset: first page passes NULL cursors              [P21]
     and (p_cursor_ts is null
          or (p.created_at, p.id) < (p_cursor_ts, p_cursor_id))
   order by p.created_at desc, p.id desc
   limit least(p_page, 50)
$$;
-- Scaling note: this is read-time fan-in over idx_post_author_live. At
-- consumer-social scale you add a write-time fan-out inbox
-- (feed_entry(recipient_id, post_id, created_at), filled by a trigger or a
-- worker on publish, skipped for mega-follower accounts, merged with
-- fan-in at read: the hybrid). The contract of this function — membership,
-- order, cursor — does not change, which is why it is a function.

create function api.profile_by_username(p_viewer uuid, p_username text)
returns table (
  profile_id uuid, username text, display_name text, bio text,
  avatar_media_id uuid, visibility app.visibility,
  follower_count int, following_count int, post_count int,
  is_self boolean, viewer_follows boolean, viewer_requested boolean,
  content_visible boolean
)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  -- the shell is visible for private profiles; the grid is gated separately
  select pr.id, pr.username::text, pr.display_name::text, pr.bio::text,
         pr.avatar_media_id, pr.visibility,
         pr.follower_count, pr.following_count, pr.post_count,
         pr.id = p_viewer,
         app.is_follower(p_viewer, pr.id),
         exists (select 1 from app.follow_request fr
                  where fr.requester_id = p_viewer and fr.target_id = pr.id
                    and fr.status = 'pending'),
         app.can_view_content_of(p_viewer, pr.id)
    from app.profile pr
   where pr.username = lower(p_username)
     and app.can_view_profile(p_viewer, pr.id)   -- blocked ⇒ zero rows [P27]
$$;

create function api.profile_grid(
  p_viewer uuid, p_profile uuid,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 24
) returns table (post_id uuid, cover jsonb, like_count int, comment_count int,
                 created_at timestamptz)
language plpgsql stable security definer set search_path = app, ext, pg_temp as $$
begin
  -- hoist the privacy check: one gate, then a pure index scan — not a
  -- policy call per row                                              [P26]
  if not app.can_view_content_of(p_viewer, p_profile) then
    return;   -- empty grid: private-and-not-approved ≡ nothing to show
  end if;

  return query
    select p.id,
           (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key,
                                      'variants', ma.variants)
              from app.post_media pm
              join app.media_asset ma on ma.id = pm.media_asset_id
             where pm.post_id = p.id
             order by pm.position limit 1),
           p.like_count, p.comment_count, p.created_at
      from app.post p
     where p.author_id = p_profile
       and p.status = 'published' and p.moderation = 'visible'
       and (p_cursor_ts is null
            or (p.created_at, p.id) < (p_cursor_ts, p_cursor_id))
     order by p.created_at desc, p.id desc
     limit least(p_page, 50);
end $$;

create function api.post_detail(p_viewer uuid, p_post uuid)
returns table (
  post_id uuid, author jsonb, media jsonb, caption text,
  mentions jsonb, hashtags text[], media_tags jsonb, location jsonb,
  created_at timestamptz, like_count int, comment_count int,
  viewer_has_liked boolean, viewer_has_saved boolean
)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select p.id,
         app.author_json(p.author_id),
         app.media_json(p.id),
         p.caption,
         (select coalesce(jsonb_agg(jsonb_build_object(
                   'profile_id', m.mentioned_profile_id, 'handle', m.handle)), '[]')
            from app.mention m where m.post_id = p.id),
         (select coalesce(array_agg(ph.tag::text), '{}')
            from app.post_hashtag ph where ph.post_id = p.id),
         (select coalesce(jsonb_agg(jsonb_build_object(
                   'tag_id', mt.id, 'media_asset_id', mt.media_asset_id,
                   'profile', app.author_json(mt.tagged_profile_id),
                   'x', mt.x, 'y', mt.y)), '[]')
            from app.media_tag mt
           where mt.post_id = p.id and mt.removed_at is null),
         (select jsonb_build_object('id', l.id, 'name', l.name)
            from app.location l where l.id = p.location_id),
         p.created_at, p.like_count, p.comment_count,
         exists (select 1 from app.post_like pl
                  where pl.profile_id = p_viewer and pl.post_id = p.id),
         exists (select 1 from app.save s
                  where s.profile_id = p_viewer and s.post_id = p.id)
    from app.post p
   where p.id = p_post
     and app.can_view_post(p_viewer, p.id)
  -- forbidden and nonexistent are the same empty result: a direct link to
  -- a private post reveals nothing, not even existence               [P27]
$$;

create function api.post_comments(
  p_viewer uuid, p_post uuid,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 20
) returns table (comment_id uuid, author jsonb, parent_id uuid,
                 reply_to jsonb, body text, moderation app.moderation_status,
                 created_at timestamptz)
language plpgsql stable security definer set search_path = app, ext, pg_temp as $$
begin
  if not app.can_view_post(p_viewer, p_post) then
    return;                                                          -- [P27]
  end if;

  return query
    select c.id, app.author_json(c.author_id), c.parent_id,
           case when c.reply_to_id is not null
                then app.author_json(c.reply_to_id) end,
           c.body::text, c.moderation, c.created_at
      from app.comment c
      join app.post p on p.id = c.post_id
     where c.post_id = p_post
       and c.deleted_at is null
       -- held comments: author and post owner only (restriction UX)
       and (c.moderation = 'visible'
            or c.author_id = p_viewer or p.author_id = p_viewer)
       -- ascending keyset: comments read oldest-first, stable while new
       -- comments append (PRD, "Pagination")                        [P21]
       and (p_cursor_ts is null
            or (c.created_at, c.id) > (p_cursor_ts, p_cursor_id))
     order by c.created_at, c.id
     limit least(p_page, 50);
end $$;

create function api.tagged_posts(
  p_viewer uuid, p_profile uuid,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 24
) returns table (post_id uuid, author jsonb, cover jsonb, created_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select p.id, app.author_json(p.author_id),
         (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key)
            from app.post_media pm join app.media_asset ma on ma.id = pm.media_asset_id
           where pm.post_id = p.id order by pm.position limit 1),
         mt.created_at
    from app.media_tag mt
    join app.post p on p.id = mt.post_id
   where mt.tagged_profile_id = p_profile
     and mt.removed_at is null
     and app.can_view_profile(p_viewer, p_profile)
     and app.can_view_post(p_viewer, p.id)   -- per-row: tag ≠ access [P25]
     and (p_cursor_ts is null
          or (mt.created_at, p.id) < (p_cursor_ts, p_cursor_id))
   order by mt.created_at desc, p.id desc
   limit least(p_page, 50)
$$;

create function api.hashtag_page(
  p_viewer uuid, p_tag text,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 24
) returns table (post_id uuid, author jsonb, cover jsonb, created_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  -- discovery is public-only: a private post carrying a hashtag never
  -- appears here, no matter who asks — same predicate as direct links,
  -- plus the public-author requirement (PRD, "Hashtags and Locations")
  select p.id, app.author_json(p.author_id),
         (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key)
            from app.post_media pm join app.media_asset ma on ma.id = pm.media_asset_id
           where pm.post_id = p.id order by pm.position limit 1),
         p.created_at
    from app.post_hashtag ph
    join app.post p on p.id = ph.post_id
    join app.profile author on author.id = p.author_id
   where ph.tag = lower(p_tag)
     and p.status = 'published' and p.moderation = 'visible'
     and author.visibility = 'public' and author.is_active
     and not app.is_blocked_between(p_viewer, author.id)
     and (p_cursor_ts is null
          or (p.created_at, p.id) < (p_cursor_ts, p_cursor_id))
   order by p.created_at desc, p.id desc
   limit least(p_page, 50)
$$;

create function api.location_page(
  p_viewer uuid, p_location uuid,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 24
) returns table (post_id uuid, author jsonb, cover jsonb, created_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select p.id, app.author_json(p.author_id),
         (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key)
            from app.post_media pm join app.media_asset ma on ma.id = pm.media_asset_id
           where pm.post_id = p.id order by pm.position limit 1),
         p.created_at
    from app.post p
    join app.profile author on author.id = p.author_id
   where p.location_id = p_location                        -- [P22] partial idx
     and p.status = 'published' and p.moderation = 'visible'
     and author.visibility = 'public' and author.is_active
     and not app.is_blocked_between(p_viewer, author.id)
     and (p_cursor_ts is null
          or (p.created_at, p.id) < (p_cursor_ts, p_cursor_id))
   order by p.created_at desc, p.id desc
   limit least(p_page, 50)
$$;

create function api.profile_search(p_viewer uuid, p_q text, p_page int default 20)
returns table (profile_id uuid, username text, display_name text,
               avatar_media_id uuid, follower_count int)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  -- trigram match (GIN-indexed via the % operator); private profiles are
  -- findable — their content is not                                 [P28]
  select pr.id, pr.username::text, pr.display_name::text,
         pr.avatar_media_id, pr.follower_count
    from app.profile pr
   where pr.is_active
     and not app.is_blocked_between(p_viewer, pr.id)
     and (pr.username % p_q
          or pr.display_name % p_q)
   order by greatest(similarity(pr.username, p_q),
                     similarity(coalesce(pr.display_name, ''), p_q)) desc,
            pr.follower_count desc
   limit least(p_page, 50)
$$;

create function api.mention_autocomplete(p_viewer uuid, p_q text, p_page int default 10)
returns table (profile_id uuid, username text, display_name text, avatar_media_id uuid)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  -- prefix scan on the text_pattern_ops index; only offer handles the
  -- author is actually allowed to mention — no dead selections      [P28]
  select pr.id, pr.username::text, pr.display_name::text, pr.avatar_media_id
    from app.profile pr
   where pr.username like lower(p_q) || '%'
     and pr.is_active
     and app.can_mention(p_viewer, pr.id)
   order by pr.follower_count desc
   limit least(p_page, 20)
$$;

create function api.saved_posts(
  p_viewer uuid,
  p_cursor_ts timestamptz default null, p_cursor_id uuid default null,
  p_page int default 24
) returns table (post_id uuid, author jsonb, cover jsonb, saved_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select p.id, app.author_json(p.author_id),
         (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key)
            from app.post_media pm join app.media_asset ma on ma.id = pm.media_asset_id
           where pm.post_id = p.id order by pm.position limit 1),
         s.created_at
    from app.save s
    join app.post p on p.id = s.post_id
   where s.profile_id = p_viewer          -- private to the viewer, by shape
     and app.can_view_post(p_viewer, p.id)  -- unavailable posts drop out
     and (p_cursor_ts is null
          or (s.created_at, p.id) < (p_cursor_ts, p_cursor_id))
   order by s.created_at desc, p.id desc
   limit least(p_page, 50)
$$;

create function api.my_collections(p_viewer uuid)
returns table (collection_id uuid, name text, item_count bigint, cover jsonb)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select c.id, c.name::text,
         (select count(*) from app.collection_item ci
           where ci.collection_id = c.id),
         (select jsonb_build_object('kind', ma.kind, 'storage_key', ma.storage_key)
            from app.collection_item ci
            join app.post_media pm on pm.post_id = ci.post_id and pm.position = 0
            join app.media_asset ma on ma.id = pm.media_asset_id
           order by ci.added_at desc limit 1)
    from app.collection c
   where c.owner_profile_id = p_viewer
   order by c.created_at
$$;

create function api.follow_requests(p_viewer uuid, p_page int default 20)
returns table (requester jsonb, requested_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select app.author_json(fr.requester_id), fr.created_at
    from app.follow_request fr
   where fr.target_id = p_viewer and fr.status = 'pending'  -- [P22] partial idx
   order by fr.created_at desc
   limit least(p_page, 50)
$$;

create function api.notifications(
  p_viewer uuid, p_before_id bigint default null, p_page int default 30
) returns table (notification_id bigint, kind app.notification_kind,
                 actor jsonb, post_id uuid, comment_id uuid, report_id uuid,
                 created_at timestamptz, read_at timestamptz)
language sql stable security definer set search_path = app, ext, pg_temp as $$
  -- identity PK doubles as the cursor: strictly monotonic, no ties  [P21]
  select n.id, n.kind,
         case when n.actor_profile_id is not null
              then app.author_json(n.actor_profile_id) end,
         n.post_id, n.comment_id, n.report_id, n.created_at, n.read_at
    from app.notification n
   where n.recipient_profile_id = p_viewer
     and (p_before_id is null or n.id < p_before_id)
   order by n.id desc
   limit least(p_page, 100)
$$;

create function api.unread_notification_count(p_viewer uuid) returns bigint
language sql stable security definer set search_path = app, ext, pg_temp as $$
  select count(*) from app.notification
   where recipient_profile_id = p_viewer and read_at is null  -- [P22] partial
$$;

-- ─────────────────────────────────────────────────────────────────────────────
-- §14  Updatable views, RLS, privileges
-- ─────────────────────────────────────────────────────────────────────────────

-- Settings as an auto-updatable view: single-relation views are updatable
-- in Postgres with no triggers. The projection IS the write grant —
-- username and the counters are simply not in it, so they cannot be
-- written here; CHECK OPTION stops a row from escaping the predicate;
-- security_barrier keeps planner-pushed functions from peeking.      [P36]
create view api.my_settings
with (security_barrier = true) as
  select id, display_name, bio, avatar_media_id,
         visibility, tag_permission, mention_permission, comment_permission
    from app.profile
   where id = app.current_profile_id()
  with local check option;

create view api.my_notification_prefs
with (security_barrier = true) as
  select profile_id, push_follows, push_likes, push_comments,
         push_mentions, push_tags
    from app.notification_prefs
   where profile_id = app.current_profile_id()
  with local check option;

-- RLS as defense in depth on the most private tables. The api functions
-- are SECURITY DEFINER (owner bypasses RLS), so these policies protect
-- any *direct* table access path a future tool might open (PostgREST,
-- ad-hoc grants, a compromised app role).                            [P37]
alter table app.save              enable row level security;
alter table app.collection        enable row level security;
alter table app.collection_item   enable row level security;
alter table app.notification      enable row level security;
alter table app.follow_request    enable row level security;
alter table app.notification_prefs enable row level security;

create policy save_owner on app.save
  using (profile_id = app.current_profile_id());
create policy collection_owner on app.collection
  using (owner_profile_id = app.current_profile_id());
create policy collection_item_owner on app.collection_item
  using (owner_profile_id = app.current_profile_id());
create policy notification_recipient on app.notification
  using (recipient_profile_id = app.current_profile_id());
create policy follow_request_party on app.follow_request
  using (app.current_profile_id() in (requester_id, target_id));
create policy prefs_owner on app.notification_prefs
  using (profile_id = app.current_profile_id())
  with check (profile_id = app.current_profile_id());

-- privileges: the app role sees functions and the two views — no tables.
-- NOTE the often-missed default: new functions are EXECUTABLE BY PUBLIC
-- until revoked.                                                     [P35]
revoke all on all functions in schema app from public;
revoke all on all functions in schema api from public;
grant usage on schema api to ig_api;
grant execute on all functions in schema api to ig_api;
grant select, update on api.my_settings           to ig_api;
grant select, update on api.my_notification_prefs to ig_api;
alter default privileges in schema api revoke execute on functions from public;
alter default privileges in schema app revoke execute on functions from public;
-- (covers the maintenance functions §15 creates after this point)


-- ─────────────────────────────────────────────────────────────────────────────
-- §15  Maintenance (cron / pg_cron)
-- ─────────────────────────────────────────────────────────────────────────────

-- Counters converge by construction, but paranoia is cheap: a repair job
-- recomputes from ground truth. Advisory lock: at most one instance runs;
-- IS DISTINCT FROM: rows already correct are not rewritten.    [P32][P33]
create function app.reconcile_counters() returns table (fixed_rows bigint)
language plpgsql as $$
declare
  v_fixed bigint := 0;
  v_n bigint;
begin
  if not pg_try_advisory_lock(hashtext('app.reconcile_counters')) then
    raise notice 'reconcile already running, skipping';
    return;
  end if;

  update app.post p
     set like_count = t.n
    from (select post_id, count(*)::int n from app.post_like group by 1) t
   where p.id = t.post_id and p.like_count is distinct from t.n;
  get diagnostics v_n = row_count; v_fixed := v_fixed + v_n;

  update app.post p
     set comment_count = t.n
    from (select post_id, count(*)::int n
            from app.comment
           where deleted_at is null and moderation = 'visible'
           group by 1) t
   where p.id = t.post_id and p.comment_count is distinct from t.n;
  get diagnostics v_n = row_count; v_fixed := v_fixed + v_n;

  update app.profile pr
     set follower_count = coalesce(t.n, 0)
    from app.profile pr2
    left join (select followee_id, count(*)::int n from app.follow group by 1) t
      on t.followee_id = pr2.id
   where pr.id = pr2.id and pr.follower_count is distinct from coalesce(t.n, 0);
  get diagnostics v_n = row_count; v_fixed := v_fixed + v_n;

  perform pg_advisory_unlock(hashtext('app.reconcile_counters'));
  return query select v_fixed;
end $$;
-- (post.save_count, profile.following_count/post_count/pending_request_count
-- reconcile identically; elided for brevity — same shape, same guards.)

-- Grace-window purge: anonymize rather than hard-delete. Content rows
-- survive for integrity/audit; identifiers and the username are scrubbed.
-- Renaming the username to a tombstone is the explicit reuse policy.
create function app.purge_due_accounts() returns int
language plpgsql as $$
declare
  v_count int := 0;
  r record;
begin
  if not pg_try_advisory_lock(hashtext('app.purge_due_accounts')) then
    return 0;                                                        -- [P32]
  end if;

  for r in
    select a.id as account_id, pr.id as profile_id
      from app.account a
      join app.profile pr on pr.account_id = a.id
     where a.status = 'deletion_pending' and a.deletion_due_at <= now()
     for update of a skip locked                                     -- [P31]
  loop
    update app.account
       set status = 'deleted', email = null, phone = null,
           password_hash = null, deletion_due_at = null
     where id = r.account_id;

    update app.profile
       set username = 'deleted_' || left(replace(r.profile_id::text, '-', ''), 16),
           display_name = null, bio = null, avatar_media_id = null
     where id = r.profile_id;

    delete from app.session where account_id = r.account_id;
    delete from app.recovery_token where account_id = r.account_id;
    -- object-storage cleanup is the storage lifecycle's job, keyed off
    -- media_asset rows of this owner — outside the database's remit
    v_count := v_count + 1;
  end loop;

  perform pg_advisory_unlock(hashtext('app.purge_due_accounts'));
  return v_count;
end $$;
-- schedule (pg_cron):
--   select cron.schedule('purge',     '7 3 * * *', $$select app.purge_due_accounts()$$);
--   select cron.schedule('reconcile', '0 4 * * 0', $$select * from app.reconcile_counters()$$);
--   select cron.schedule('ratelimit', '0 * * * *',
--     $$delete from app.rate_limit where window_start < now() - interval '2 days'$$);


-- ─────────────────────────────────────────────────────────────────────────────
-- §16  Smoke test — runs and rolls back; the file leaves no data behind
-- ─────────────────────────────────────────────────────────────────────────────

begin;

do $$
declare
  v_maya  uuid;  v_luis  uuid;  v_vera  uuid;
  v_media uuid;  v_post  uuid;  v_comment uuid;
  v_n     bigint;
begin
  select profile_id into v_maya from api.sign_up('maya@example.com', 'password1', 'maya');
  select profile_id into v_luis from api.sign_up('luis@example.com', 'password1', 'luis');
  select profile_id into v_vera from api.sign_up('vera@example.com', 'password1', 'vera');

  -- vera goes private (direct app-level write; the view path needs the GUC)
  update app.profile set visibility = 'private' where id = v_vera;

  -- media pipeline: uploaded → processing → ready, and no skipping
  v_media := api.create_media(v_maya, 's3://bucket/beach.jpg', 'image');
  begin
    update app.media_asset set status = 'ready' where id = v_media;
    raise exception 'state machine failed to block uploaded>ready';
  exception when sqlstate 'IG422' then null;   -- expected             [P18]
  end;
  update app.media_asset set status = 'processing' where id = v_media;
  update app.media_asset set status = 'ready'      where id = v_media;

  -- publish with a mention and a hashtag
  v_post := api.publish_post(v_maya, array[v_media],
                             'day one with @luis at the beach #sunset');
  assert (select post_count from app.profile where id = v_maya) = 1;
  assert exists (select 1 from app.mention
                  where post_id = v_post and mentioned_profile_id = v_luis);
  assert exists (select 1 from app.post_hashtag
                  where post_id = v_post and tag = 'sunset');

  -- follow: public is immediate, private queues a request
  assert api.follow(v_luis, v_maya) = 'following';
  assert api.follow(v_luis, v_vera) = 'requested';
  assert (select follower_count from app.profile where id = v_maya) = 1;
  assert (select pending_request_count from app.profile where id = v_vera) = 1;

  perform api.approve_follow_request(v_vera, v_luis);
  perform api.approve_follow_request(v_vera, v_luis);   -- idempotent replay
  assert app.is_follower(v_luis, v_vera);
  assert (select pending_request_count from app.profile where id = v_vera) = 0;

  -- like twice: one row, one count — retries cannot drift            [P17]
  perform api.like_post(v_luis, v_post);
  perform api.like_post(v_luis, v_post);
  assert (select like_count from app.post where id = v_post) = 1;
  perform api.unlike_post(v_luis, v_post);
  perform api.unlike_post(v_luis, v_post);
  assert (select like_count from app.post where id = v_post) = 0;
  perform api.like_post(v_luis, v_post);

  v_comment := api.add_comment(v_luis, v_post, 'take me next time @maya');
  assert (select comment_count from app.post where id = v_post) = 1;

  -- luis sees maya's post in his feed
  select count(*) into v_n from api.home_feed(v_luis);
  assert v_n = 1, 'expected 1 feed item, got ' || v_n;

  -- activity landed: follow, like, comment, mention
  select count(*) into v_n from app.notification
   where recipient_profile_id = v_maya;
  assert v_n >= 3, 'expected maya to have notifications';
  select count(*) into v_n from app.notification
   where recipient_profile_id = v_luis and kind = 'mention';
  assert v_n = 1, 'expected luis to be notified of the mention';

  -- blocking severs the graph and hides everything, both ways
  perform api.block_profile(v_maya, v_luis);
  assert not app.is_follower(v_luis, v_maya);
  assert not app.can_view_post(v_luis, v_post);
  select count(*) into v_n from api.post_detail(v_luis, v_post);
  assert v_n = 0, 'blocked viewer must see nothing';                -- [P27]

  -- soft delete hides the post from its surfaces but keeps the rows
  perform api.delete_post(v_maya, v_post);
  assert (select post_count from app.profile where id = v_maya) = 0;
  assert exists (select 1 from app.comment where id = v_comment);
  perform api.restore_post(v_maya, v_post);
  assert (select post_count from app.profile where id = v_maya) = 1;

  raise notice 'smoke test passed';
end $$;

rollback;

-- ═══════════════════════════════════════════════════════════════════════════
-- Not in this file, on purpose: connection pooling (pgbouncer), object
-- storage and the transcode worker (they drive §6's machine from outside),
-- the push-delivery worker (LISTEN on app_notification, honor
-- notification_prefs), full-text/vector search infra, feed fan-out at
-- celebrity scale (§13 note), and sharding. The database owns correctness:
-- identity, uniqueness, relationships, permissions, counters, lifecycles.
-- ═══════════════════════════════════════════════════════════════════════════

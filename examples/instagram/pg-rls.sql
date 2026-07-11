-- ═══════════════════════════════════════════════════════════════════════════
-- instagram/pg-rls.sql — the Supabase-native RLS oracle
-- ═══════════════════════════════════════════════════════════════════════════
--
-- The Instagram PRD (./PRD.md) and v0 entity set (./v0.spock) implemented the
-- way you build on Supabase: RLS *is* the authorization layer. Clients hit the
-- tables directly (PostgREST / pg_graphql). There is NO SECURITY DEFINER
-- function API in front of the tables — that raw-Postgres "the database is the
-- backend, the API is functions" style is exactly what pg.sql does and what we
-- deliberately do NOT do here.
--
-- The predicates are mined from pg.sql (the answer sheet) but RE-CAST:
--   * actor          = auth.uid()               (the JWT `sub`, a profile id)
--   * claims         = auth.jwt()               (app_metadata role, etc.)
--   * audiences      = TO anon / TO authenticated
--   * per-command    = separate SELECT / INSERT / UPDATE / DELETE policies
--   * USING          governs which rows are visible to read/delete
--   * WITH CHECK      governs which rows an insert/update may produce
--
-- SECURITY DEFINER helpers appear in EXACTLY two sanctioned roles:
--   (a) anti-recursion — a policy that must read the very table it guards
--       (post→post, comment→comment, membership→membership) would recurse; a
--       SECURITY DEFINER function reads it once, out of band.               [P25/P37]
--   (b) visibility hoist — the block/follow/private graph must be consulted
--       from rows the viewer cannot themselves SELECT (you cannot see the
--       block row that hides a post from you). A DEFINER function owned by a
--       BYPASSRLS role (postgres, on Supabase) sees the whole graph so the
--       predicate is correct. Clients never gain that reach.
--
-- The graph/visibility helpers live in a NON-EXPOSED schema `private` (see §3):
-- they take arbitrary principals as arguments and run BYPASSRLS, so leaving them
-- in `public` — a PostgREST-exposed schema — would publish them as RPC
-- endpoints (`/rest/v1/rpc/is_blocked_between?a=..&b=..`) and leak exactly the
-- block/follow secrets the policies protect. `private` is not in PostgREST's
-- exposed-schema list, so RLS can still call the functions cross-schema while no
-- client can reach them directly.
--
-- Assumes a Supabase database: schema `auth` with auth.users, the functions
-- auth.uid()/auth.jwt(), and the roles `anon` and `authenticated`. A profile
-- row is keyed 1:1 to auth.users — so auth.uid() *is* the current profile id,
-- and the account/profile split of pg.sql collapses into Supabase's auth.users.
--
-- Run:  psql / Supabase SQL editor. Idempotent-ish within a fresh schema.
-- ═══════════════════════════════════════════════════════════════════════════


-- ─────────────────────────────────────────────────────────────────────────────
-- §1  Tables — social core (minimal columns; only what authorization reads)
-- ─────────────────────────────────────────────────────────────────────────────

-- profile.id = auth.users.id : the JWT subject is the profile identity.
create table public.profile (
  id         uuid primary key references auth.users (id) on delete cascade,
  username   text not null unique check (username ~ '^[a-z0-9._]{1,30}$'),
  full_name  text,
  bio        text,
  private    boolean not null default false,   -- public vs private account
  created_at timestamptz not null default now()
);

-- reference data: imported from a places provider, never minted by clients.
create table public.location (
  id       uuid primary key default gen_random_uuid(),
  name     text not null,
  place_id text not null unique
);

create table public.post (
  id          uuid primary key default gen_random_uuid(),
  author_id   uuid not null references public.profile (id) on delete cascade,
  caption     text,
  location_id uuid references public.location (id),
  archived_at timestamptz,                       -- archived ⇒ author-only
  created_at  timestamptz not null default now()
);
create index on public.post (author_id, created_at desc);

create table public.media (
  id         uuid primary key default gen_random_uuid(),
  post_id    uuid not null references public.post (id) on delete cascade,
  position   int  not null check (position >= 0),
  kind       text not null check (kind in ('image', 'video')),
  created_at timestamptz not null default now(),
  unique (post_id, position)
);

create table public.follow (
  follower_id uuid not null references public.profile (id) on delete cascade,
  followee_id uuid not null references public.profile (id) on delete cascade,
  created_at  timestamptz not null default now(),
  primary key (follower_id, followee_id),
  check (follower_id <> followee_id)            -- no self-follow (DDL, not RLS)
);
create index on public.follow (followee_id, follower_id);

create table public.follow_request (
  requester_id uuid not null references public.profile (id) on delete cascade,
  target_id    uuid not null references public.profile (id) on delete cascade,
  status       text not null default 'pending'
               check (status in ('pending', 'approved', 'denied')),
  created_at   timestamptz not null default now(),
  responded_at timestamptz,
  primary key (requester_id, target_id),
  check (requester_id <> target_id)
);

create table public.block (
  blocker_id uuid not null references public.profile (id) on delete cascade,
  blocked_id uuid not null references public.profile (id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (blocker_id, blocked_id),
  check (blocker_id <> blocked_id)
);
create index on public.block (blocked_id, blocker_id);

create table public.restriction (
  restrictor_id uuid not null references public.profile (id) on delete cascade,
  restricted_id uuid not null references public.profile (id) on delete cascade,
  created_at    timestamptz not null default now(),
  primary key (restrictor_id, restricted_id),
  check (restrictor_id <> restricted_id)
);

create table public.post_like (
  profile_id uuid not null references public.profile (id) on delete cascade,
  post_id    uuid not null references public.post (id)    on delete cascade,
  created_at timestamptz not null default now(),
  primary key (profile_id, post_id)             -- like a post at most once
);
create index on public.post_like (post_id);

create table public.comment (
  id         uuid primary key default gen_random_uuid(),
  post_id    uuid not null references public.post (id)     on delete cascade,
  author_id  uuid not null references public.profile (id)  on delete cascade,
  parent_id  uuid references public.comment (id) on delete set null,
  body       text not null check (length(btrim(body)) between 1 and 2200),
  status     text not null default 'visible'
             check (status in ('visible', 'held')),   -- held ⇒ author+owner only
  created_at timestamptz not null default now()
);
create index on public.comment (post_id, created_at);

create table public.save (
  profile_id uuid not null references public.profile (id) on delete cascade,
  post_id    uuid not null references public.post (id)    on delete cascade,
  created_at timestamptz not null default now(),
  primary key (profile_id, post_id)             -- private library, by shape
);

create table public.collection (
  id         uuid primary key default gen_random_uuid(),
  owner_id   uuid not null references public.profile (id) on delete cascade,
  name       text not null check (length(name) between 1 and 50),
  created_at timestamptz not null default now(),
  unique (owner_id, name)
);

create table public.collection_item (
  collection_id uuid not null references public.collection (id) on delete cascade,
  post_id       uuid not null references public.post (id)       on delete cascade,
  added_at      timestamptz not null default now(),
  primary key (collection_id, post_id)
);

create table public.mention (
  id            uuid primary key default gen_random_uuid(),
  mentioned_id  uuid not null references public.profile (id) on delete cascade,
  post_id       uuid references public.post (id)    on delete cascade,
  comment_id    uuid references public.comment (id) on delete cascade,
  display       text not null,
  created_at    timestamptz not null default now(),
  check (num_nonnulls(post_id, comment_id) = 1)     -- exactly one source
);

create table public.media_tag (
  id         uuid primary key default gen_random_uuid(),
  media_id   uuid not null references public.media (id)   on delete cascade,
  tagged_id  uuid not null references public.profile (id) on delete cascade,
  x          real not null check (x between 0 and 1),
  y          real not null check (y between 0 and 1),
  created_at timestamptz not null default now(),
  unique (media_id, tagged_id)
);

create table public.hashtag (
  tag        text primary key check (tag ~ '^[a-z0-9_]{1,64}$'),
  created_at timestamptz not null default now()
);

create table public.post_hashtag (
  post_id uuid not null references public.post (id) on delete cascade,
  tag     text not null references public.hashtag (tag),
  primary key (post_id, tag)
);

-- report subjects use ON DELETE CASCADE (mirroring `mention`): a report is about
-- exactly one subject (the num_nonnulls check), so if the subject is deleted the
-- report goes with it. ON DELETE SET NULL would blank the only non-null column
-- and violate `num_nonnulls(...) = 1`, aborting the subject's own delete.
create table public.report (
  id          uuid primary key default gen_random_uuid(),
  reporter_id uuid not null references public.profile (id) on delete cascade,
  post_id     uuid references public.post (id)    on delete cascade,
  comment_id  uuid references public.comment (id) on delete cascade,
  profile_id  uuid references public.profile (id) on delete cascade,
  reason      text not null check (length(btrim(reason)) > 0),
  status      text not null default 'open'
              check (status in ('open', 'reviewed', 'actioned')),
  created_at  timestamptz not null default now(),
  check (num_nonnulls(post_id, comment_id, profile_id) = 1)
);

create table public.notification (
  id           bigint generated always as identity primary key,
  recipient_id uuid not null references public.profile (id) on delete cascade,
  actor_id     uuid references public.profile (id) on delete cascade,
  kind         text not null check (kind in
               ('follow','follow_request','like','comment','mention','tag')),
  post_id      uuid references public.post (id)    on delete cascade,
  comment_id   uuid references public.comment (id) on delete cascade,
  created_at   timestamptz not null default now(),
  read_at      timestamptz
);
create index on public.notification (recipient_id, id desc);


-- ─────────────────────────────────────────────────────────────────────────────
-- §2  Tables — enterprise RBAC addendum (the role-gated family)
-- ─────────────────────────────────────────────────────────────────────────────

create table public.organization (
  id         uuid primary key default gen_random_uuid(),
  name       text not null,
  owner_id   uuid not null references public.profile (id) on delete restrict,
  created_at timestamptz not null default now()
);

-- membership.role is the RBAC axis. A naive "members can see co-members" policy
-- that reads membership would recurse; §3's is_org_member() / org_role() break
-- the cycle with SECURITY DEFINER.
create table public.membership (
  org_id     uuid not null references public.organization (id) on delete cascade,
  member_id  uuid not null references public.profile (id)      on delete cascade,
  role       text not null default 'member'
             check (role in ('owner', 'admin', 'member')),
  created_at timestamptz not null default now(),
  primary key (org_id, member_id)
);

-- the one org-owned resource: any member reads, only owner/admin writes.
create table public.org_setting (
  org_id     uuid not null references public.organization (id) on delete cascade,
  key        text not null,
  value      text not null,
  updated_at timestamptz not null default now(),
  primary key (org_id, key)
);


-- ─────────────────────────────────────────────────────────────────────────────
-- §3  Authorization helpers — SECURITY DEFINER STABLE, in schema `private`
--     Owned by postgres (a BYPASSRLS role on Supabase), so they read the whole
--     block/follow/private graph regardless of the caller's RLS — and are not
--     re-entered by the policies that call them. search_path is pinned to
--     defeat search-path hijack. auth.uid() still resolves inside a DEFINER
--     function (it reads the request JWT GUC, not the current role).
--
--     WHY `private`, not `public`: PostgREST exposes `public`, so any function
--     there becomes a callable RPC. is_blocked_between(a,b)/is_follower(a,b)
--     take ARBITRARY principals and run BYPASSRLS — as public RPCs they would
--     let anyone (even anon) probe the block/follow graph, defeating block
--     secrecy. `private` is outside PostgREST's exposed-schema list, so the
--     functions stay callable from RLS (cross-schema) but unreachable as RPC.
-- ─────────────────────────────────────────────────────────────────────────────

create schema if not exists private;

-- blocking severs visibility BOTH ways; anon (a IS NULL) is never blocked.
create or replace function private.is_blocked_between(a uuid, b uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select a is not null and b is not null and exists (
    select 1 from public.block
     where (blocker_id = a and blocked_id = b)
        or (blocker_id = b and blocked_id = a));
$$;

-- "a follows b" — the follow row IS the approval (private accounts only get one
-- through the approve trigger in §4).
create or replace function private.is_follower(a uuid, b uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select a is not null and exists (
    select 1 from public.follow where follower_id = a and followee_id = b);
$$;

create or replace function private.profile_is_public(p uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (select 1 from public.profile where id = p and not private);
$$;

-- profile visibility: public, or self, or approved follower — never across a
-- block. Anon sees public profiles only. (SECURITY DEFINER: also stops the
-- profile SELECT policy that calls it from recursing on profile.)
create or replace function private.can_view_profile(target uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (
    select 1 from public.profile p
     where p.id = target
       and not private.is_blocked_between(auth.uid(), target)
       and ( not p.private
             or p.id = auth.uid()
             or private.is_follower(auth.uid(), p.id) ));
$$;

-- THE post-visibility predicate — the one gate behind feed, grid, detail,
-- hashtag/location pages, saved & tagged surfaces, direct links. Author sees
-- their own posts (incl. archived); everyone else needs a visible, non-archived
-- post whose author's content they may see. SECURITY DEFINER ⇒ no post→post
-- recursion.
create or replace function private.can_view_post(p_post uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (
    select 1 from public.post p
     where p.id = p_post
       and ( p.author_id = auth.uid()
             or ( p.archived_at is null
                  and private.can_view_profile(p.author_id) ) ));
$$;

-- comment visibility: on a viewable post, and either 'visible', or 'held' but
-- the viewer is its author or the post owner (the restriction rule).
create or replace function private.can_view_comment(p_comment uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (
    select 1 from public.comment c join public.post p on p.id = c.post_id
     where c.id = p_comment
       and private.can_view_post(c.post_id)
       and ( c.status = 'visible'
             or c.author_id = auth.uid()
             or p.author_id = auth.uid() ));
$$;

create or replace function private.is_post_owner(p_post uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (select 1 from public.post where id = p_post and author_id = auth.uid());
$$;

-- RBAC, recursion-safe: read membership out of band so membership's own
-- policies can consult it.
create or replace function private.is_org_member(p_org uuid)
returns boolean language sql stable security definer set search_path = public, pg_temp as $$
  select exists (select 1 from public.membership
                  where org_id = p_org and member_id = auth.uid());
$$;

create or replace function private.org_role(p_org uuid)
returns text language sql stable security definer set search_path = public, pg_temp as $$
  select role from public.membership
   where org_id = p_org and member_id = auth.uid();
$$;


-- ─────────────────────────────────────────────────────────────────────────────
-- §4  Correctness triggers — the state transitions RLS cannot express
--     (RLS decides who may act; these carry the consequences of the act, and
--     pin the columns RLS's WITH CHECK cannot — RLS sees only the NEW row, so
--     "column X may not change" and "this transition is the only legal one" are
--     BEFORE-triggers, not policies.)
--     All SECURITY DEFINER so they may write rows the actor's own policies
--     forbid (e.g. a private-account follow the follow INSERT policy blocks).
-- ─────────────────────────────────────────────────────────────────────────────

-- Approving a follow request materializes the follow edge (the private path).
-- The parties are IMMUTABLE: without this guard a target could rewrite
-- requester_id to a victim and approve, forging a follow the victim never asked
-- for (RLS WITH CHECK cannot see OLD, so it cannot pin the identity columns).
create or replace function public.tg_follow_request_approved()
returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if new.requester_id is distinct from old.requester_id
     or new.target_id is distinct from old.target_id then
    raise exception 'follow_request parties are immutable';
  end if;
  if new.status = 'approved' and old.status is distinct from 'approved' then
    insert into public.follow (follower_id, followee_id)
    values (new.requester_id, new.target_id)
    on conflict do nothing;
    new.responded_at := now();
  elsif new.status = 'denied' and old.status is distinct from 'denied' then
    new.responded_at := now();
  end if;
  return new;
end $$;
create trigger trg_follow_request_approved
  before update on public.follow_request
  for each row execute function public.tg_follow_request_approved();

-- A restricted user's comment lands 'held' (visible to author + post owner
-- only). Column state, not authorization — so it belongs in a trigger.
create or replace function public.tg_comment_hold_if_restricted()
returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if exists (
    select 1 from public.restriction r
      join public.post p on p.id = new.post_id
     where r.restrictor_id = p.author_id and r.restricted_id = new.author_id)
  then
    new.status := 'held';
  end if;
  return new;
end $$;
create trigger trg_comment_hold
  before insert on public.comment
  for each row execute function public.tg_comment_hold_if_restricted();

-- comment_update's policy lets the post owner touch a comment, but the ONLY
-- legal edit is approving a held comment (held → visible). Everything else —
-- rewriting the body, reassigning author_id, re-holding a visible comment — is
-- content forgery. RLS can't scope columns or reference OLD, so pin it here.
create or replace function public.tg_comment_owner_edit_guard()
returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if new.post_id    is distinct from old.post_id
     or new.author_id  is distinct from old.author_id
     or new.parent_id  is distinct from old.parent_id
     or new.body       is distinct from old.body
     or new.created_at is distinct from old.created_at then
    raise exception 'only a comment''s status may change (held->visible)';
  end if;
  if new.status is distinct from old.status
     and not (old.status = 'held' and new.status = 'visible') then
    raise exception 'comment status may only move held->visible';
  end if;
  return new;
end $$;
create trigger trg_comment_owner_edit_guard
  before update on public.comment
  for each row execute function public.tg_comment_owner_edit_guard();

-- notification_update's policy is scoped to the recipient, but the only field a
-- recipient may change is read_at (mark read/unread). Pin the rest so a client
-- can't rewrite the kind/actor/subject of its own notifications.
create or replace function public.tg_notification_readonly()
returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if new.recipient_id is distinct from old.recipient_id
     or new.actor_id   is distinct from old.actor_id
     or new.kind       is distinct from old.kind
     or new.post_id    is distinct from old.post_id
     or new.comment_id is distinct from old.comment_id
     or new.created_at is distinct from old.created_at then
    raise exception 'only read_at may change on a notification';
  end if;
  return new;
end $$;
create trigger trg_notification_readonly
  before update on public.notification
  for each row execute function public.tg_notification_readonly();

-- Creating an org makes the creator its owner-member (bootstraps RBAC; the
-- membership INSERT policy can't, since the creator has no role row yet).
create or replace function public.tg_org_owner_membership()
returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  insert into public.membership (org_id, member_id, role)
  values (new.id, new.owner_id, 'owner')
  on conflict do nothing;
  return new;
end $$;
create trigger trg_org_owner
  after insert on public.organization
  for each row execute function public.tg_org_owner_membership();


-- ─────────────────────────────────────────────────────────────────────────────
-- §5  Enable RLS on every app table. FORCE on the privacy-critical ones.
--     RLS enabled ⇒ default-deny: with no policy, a command sees/writes zero
--     rows. anon simply gets no write policy anywhere, so it can write nothing.
--
--     FORCE note: the table OWNER bypasses RLS unless FORCEd. On Supabase the
--     owner is postgres (BYPASSRLS — so even FORCE won't subject our DEFINER
--     helpers to RLS, which is what we want). FORCE matters the day the app
--     connects as a plain owner role, or an owned-but-not-BYPASSRLS role runs
--     ad-hoc DML: these tables must still enforce their policies. Belt and
--     suspenders on the rows that would hurt most if leaked.
--
--     `membership` is deliberately NOT FORCEd. Its membership_read policy calls
--     is_org_member(), a DEFINER helper that itself reads `membership`. Under a
--     non-BYPASSRLS owner FORCE would subject that inner read to membership_read
--     again — reintroducing exactly the recursion the DEFINER exists to break.
--     The anti-recursion guarantee for any table whose own DEFINER helper reads
--     it depends on the helper's owner bypassing that table's RLS; FORCE would
--     void it. (org_setting stays FORCEd safely: its helpers read `membership`,
--     never `org_setting`, so no self-recursion.)
-- ─────────────────────────────────────────────────────────────────────────────

alter table public.profile         enable row level security;
alter table public.location        enable row level security;
alter table public.post            enable row level security;
alter table public.media           enable row level security;
alter table public.follow          enable row level security;
alter table public.follow_request  enable row level security;
alter table public.block           enable row level security;
alter table public.restriction     enable row level security;
alter table public.post_like       enable row level security;
alter table public.comment         enable row level security;
alter table public.save            enable row level security;
alter table public.collection      enable row level security;
alter table public.collection_item enable row level security;
alter table public.mention         enable row level security;
alter table public.media_tag       enable row level security;
alter table public.hashtag         enable row level security;
alter table public.post_hashtag    enable row level security;
alter table public.report          enable row level security;
alter table public.notification    enable row level security;
alter table public.organization    enable row level security;
alter table public.membership      enable row level security;
alter table public.org_setting     enable row level security;

alter table public.save            force row level security;
alter table public.collection      force row level security;
alter table public.collection_item force row level security;
alter table public.follow_request  force row level security;
alter table public.block           force row level security;
alter table public.restriction     force row level security;
alter table public.report          force row level security;
alter table public.notification    force row level security;
alter table public.org_setting     force row level security;


-- ─────────────────────────────────────────────────────────────────────────────
-- §6  Policies — social core
--     Convention: (select auth.uid()) so the planner hoists it to an initplan
--     (evaluated once, not per row). anon has a NULL uid, so every self / owner
--     / follower test collapses to false — anon reads only what a policy hands
--     to `anon` explicitly, and writes nothing.
-- ─────────────────────────────────────────────────────────────────────────────

-- profile ─ public shells to anyone; private details to self + approved
-- followers; never across a block.
create policy profile_read on public.profile
  for select to anon, authenticated
  using ( private.can_view_profile(id) );
create policy profile_insert on public.profile
  for insert to authenticated
  with check ( id = (select auth.uid()) );          -- may only mint your own row
create policy profile_update on public.profile
  for update to authenticated
  using ( id = (select auth.uid()) )
  with check ( id = (select auth.uid()) );

-- location ─ reference data: world-readable, client-unwritable (no write policy).
create policy location_read on public.location
  for select to anon, authenticated
  using ( true );

-- post ─ author public / self / approved follower, not blocked, archived⇒author.
create policy post_read on public.post
  for select to anon, authenticated
  using ( private.can_view_post(id) );
create policy post_insert on public.post
  for insert to authenticated
  with check ( author_id = (select auth.uid()) );
create policy post_update on public.post                -- edit + archive/unarchive
  for update to authenticated
  using ( author_id = (select auth.uid()) )
  with check ( author_id = (select auth.uid()) );
create policy post_delete on public.post
  for delete to authenticated
  using ( author_id = (select auth.uid()) );

-- media ─ rides its post's visibility; only the post's author may attach it.
create policy media_read on public.media
  for select to anon, authenticated
  using ( private.can_view_post(post_id) );
create policy media_insert on public.media
  for insert to authenticated
  with check ( private.is_post_owner(post_id) );

-- follow ─ both parties see the edge; you may direct-follow only PUBLIC, unblocked
-- accounts (private ⇒ follow_request); either party may drop it.
create policy follow_read on public.follow
  for select to authenticated
  using ( follower_id = (select auth.uid()) or followee_id = (select auth.uid()) );
create policy follow_insert on public.follow
  for insert to authenticated
  with check ( follower_id = (select auth.uid())
               and private.profile_is_public(followee_id)
               and not private.is_blocked_between((select auth.uid()), followee_id) );
create policy follow_delete on public.follow
  for delete to authenticated
  using ( follower_id = (select auth.uid()) or followee_id = (select auth.uid()) );

-- follow_request ─ both parties see it; you request only a PRIVATE, unblocked
-- account; only the TARGET approves/denies (status flip → §4 trigger makes the
-- follow, and pins the immutable parties).
create policy follow_request_read on public.follow_request
  for select to authenticated
  using ( requester_id = (select auth.uid()) or target_id = (select auth.uid()) );
create policy follow_request_insert on public.follow_request
  for insert to authenticated
  with check ( requester_id = (select auth.uid())
               and not private.profile_is_public(target_id)
               and not private.is_blocked_between((select auth.uid()), target_id) );
create policy follow_request_update on public.follow_request
  for update to authenticated
  using ( target_id = (select auth.uid()) )
  with check ( target_id = (select auth.uid()) );

-- block ─ visible only to the blocker (the blocked must not learn of it);
-- create/remove your own.
create policy block_read on public.block
  for select to authenticated
  using ( blocker_id = (select auth.uid()) );
create policy block_insert on public.block
  for insert to authenticated
  with check ( blocker_id = (select auth.uid()) );
create policy block_delete on public.block
  for delete to authenticated
  using ( blocker_id = (select auth.uid()) );

-- restriction ─ quieter than a block: visible only to the restrictor; its
-- consequence (held comments) is applied by the §4 trigger.
create policy restriction_read on public.restriction
  for select to authenticated
  using ( restrictor_id = (select auth.uid()) );
create policy restriction_insert on public.restriction
  for insert to authenticated
  with check ( restrictor_id = (select auth.uid()) );
create policy restriction_delete on public.restriction
  for delete to authenticated
  using ( restrictor_id = (select auth.uid()) );

-- like ─ readable wherever the post is; you may like (as yourself) only a post
-- you can see; unlike your own.
create policy like_read on public.post_like
  for select to anon, authenticated
  using ( private.can_view_post(post_id) );
create policy like_insert on public.post_like
  for insert to authenticated
  with check ( profile_id = (select auth.uid())
               and private.can_view_post(post_id) );
create policy like_delete on public.post_like
  for delete to authenticated
  using ( profile_id = (select auth.uid()) );

-- comment ─ visible ones ride the post; held ones only to author + post owner;
-- comment on a post you can see; post owner may approve (update) a held one
-- (the §4 guard pins the transition to held→visible); author OR post owner may
-- delete.
create policy comment_read on public.comment
  for select to anon, authenticated
  using ( private.can_view_comment(id) );
create policy comment_insert on public.comment
  for insert to authenticated
  with check ( author_id = (select auth.uid())
               and private.can_view_post(post_id) );
create policy comment_update on public.comment                -- owner approves held
  for update to authenticated
  using ( private.is_post_owner(post_id) )
  with check ( private.is_post_owner(post_id) );
create policy comment_delete on public.comment
  for delete to authenticated
  using ( author_id = (select auth.uid()) or private.is_post_owner(post_id) );

-- save ─ a private library: only the saver reads it; save only visible posts;
-- unsave your own.
create policy save_read on public.save
  for select to authenticated
  using ( profile_id = (select auth.uid()) );
create policy save_insert on public.save
  for insert to authenticated
  with check ( profile_id = (select auth.uid())
               and private.can_view_post(post_id) );
create policy save_delete on public.save
  for delete to authenticated
  using ( profile_id = (select auth.uid()) );

-- collection ─ private to the owner.
create policy collection_read on public.collection
  for select to authenticated
  using ( owner_id = (select auth.uid()) );
create policy collection_insert on public.collection
  for insert to authenticated
  with check ( owner_id = (select auth.uid()) );
create policy collection_delete on public.collection
  for delete to authenticated
  using ( owner_id = (select auth.uid()) );

-- collection_item ─ inside your own collection; an item may be filed only if the
-- post is one YOU have saved (the cross-table rule, as a WITH CHECK).
create policy collection_item_read on public.collection_item
  for select to authenticated
  using ( exists (select 1 from public.collection c
                   where c.id = collection_id and c.owner_id = (select auth.uid())) );
create policy collection_item_insert on public.collection_item
  for insert to authenticated
  with check ( exists (select 1 from public.collection c
                        where c.id = collection_id and c.owner_id = (select auth.uid()))
               and exists (select 1 from public.save s
                            where s.post_id = collection_item.post_id
                              and s.profile_id = (select auth.uid())) );
create policy collection_item_delete on public.collection_item
  for delete to authenticated
  using ( exists (select 1 from public.collection c
                   where c.id = collection_id and c.owner_id = (select auth.uid())) );

-- mention ─ rides the visibility of whichever source (post or comment) carries it.
-- Derived server-side from caption/comment text (no client INSERT policy — like
-- notifications, mentions are minted by activity triggers / the service role).
create policy mention_read on public.mention
  for select to anon, authenticated
  using ( (post_id    is not null and private.can_view_post(post_id))
       or (comment_id is not null and private.can_view_comment(comment_id)) );

-- media_tag ─ visible to anyone who can see the post (via its media); the post's
-- author attaches tags; a tagged user may remove themselves.
create policy media_tag_read on public.media_tag
  for select to anon, authenticated
  using ( exists (select 1 from public.media m where m.id = media_id) )   -- media RLS = post visibility
;
create policy media_tag_insert on public.media_tag
  for insert to authenticated
  with check ( exists (select 1 from public.media m
                        join public.post p on p.id = m.post_id
                       where m.id = media_id and p.author_id = (select auth.uid())) );
create policy media_tag_delete on public.media_tag
  for delete to authenticated
  using ( tagged_id = (select auth.uid()) );

-- hashtag / post_hashtag ─ the discovery join. hashtag rows are public labels;
-- a post_hashtag link is visible only if its post is (so private posts never
-- surface on a hashtag page, no matter who asks). Both are derived server-side
-- from caption text — no client INSERT policy, so clients cannot mint labels or
-- links (matching the mention / notification pattern).
create policy hashtag_read on public.hashtag
  for select to anon, authenticated
  using ( true );
create policy post_hashtag_read on public.post_hashtag
  for select to anon, authenticated
  using ( private.can_view_post(post_id) );

-- report ─ reporter-only reads; plus a claims-gated moderator override.
create policy report_read_own on public.report
  for select to authenticated
  using ( reporter_id = (select auth.uid()) );
-- jwt-claims: platform staff carrying app_metadata.role = 'moderator' read all.
create policy report_read_moderator on public.report
  for select to authenticated
  using ( (auth.jwt() -> 'app_metadata' ->> 'role') = 'moderator' );
create policy report_insert on public.report
  for insert to authenticated
  with check ( reporter_id = (select auth.uid()) );

-- notification ─ recipient-only; recipient marks them read (update, pinned to
-- read_at by the §4 guard). No client INSERT policy: notifications are written
-- by SECURITY DEFINER activity triggers / the service role, never by clients.
create policy notification_read on public.notification
  for select to authenticated
  using ( recipient_id = (select auth.uid()) );
create policy notification_update on public.notification
  for update to authenticated
  using ( recipient_id = (select auth.uid()) )
  with check ( recipient_id = (select auth.uid()) );


-- ─────────────────────────────────────────────────────────────────────────────
-- §7  Policies — enterprise RBAC addendum  (patterns: rbac-role, jwt-claims)
--     The role gate checks WHO acts AND WHAT they may touch: an admin may not
--     act on, assign, or become the `owner` role — only an owner can. Without
--     the target-role guard an admin could demote/remove the owner or mint a
--     second owner (the actor-only check is a classic RBAC escalation hole).
-- ─────────────────────────────────────────────────────────────────────────────

-- organization ─ members see their orgs; the creator opens one as its owner
-- (the §4 trigger then writes their owner-membership).
create policy org_read on public.organization
  for select to authenticated
  using ( private.is_org_member(id) );
create policy org_insert on public.organization
  for insert to authenticated
  with check ( owner_id = (select auth.uid()) );

-- membership ─ co-members see the roster (recursion-safe via is_org_member);
-- owner/admin add or re-role members, but only an OWNER may touch an owner row
-- or assign the owner role; a member may always remove themselves (leave).
create policy membership_read on public.membership
  for select to authenticated
  using ( private.is_org_member(org_id) );
create policy membership_insert on public.membership
  for insert to authenticated
  with check ( private.org_role(org_id) = 'owner'
               or (private.org_role(org_id) = 'admin' and role <> 'owner') );
create policy membership_update on public.membership
  for update to authenticated
  using ( private.org_role(org_id) = 'owner'                       -- target row (OLD)
          or (private.org_role(org_id) = 'admin' and role <> 'owner') )
  with check ( private.org_role(org_id) = 'owner'                  -- resulting row (NEW)
               or (private.org_role(org_id) = 'admin' and role <> 'owner') );
create policy membership_delete on public.membership
  for delete to authenticated
  using ( private.org_role(org_id) = 'owner'
          or (private.org_role(org_id) = 'admin' and role <> 'owner')  -- admin: non-owner rows only
          or member_id = (select auth.uid()) );                        -- anyone may leave

-- org_setting ─ the role-gated resource: ANY member reads; only owner/admin
-- writes. Non-members match nothing → they see and touch nothing.
create policy org_setting_read on public.org_setting
  for select to authenticated
  using ( private.is_org_member(org_id) );
create policy org_setting_insert on public.org_setting
  for insert to authenticated
  with check ( private.org_role(org_id) in ('owner', 'admin') );
create policy org_setting_update on public.org_setting
  for update to authenticated
  using ( private.org_role(org_id) in ('owner', 'admin') )
  with check ( private.org_role(org_id) in ('owner', 'admin') );
create policy org_setting_delete on public.org_setting
  for delete to authenticated
  using ( private.org_role(org_id) in ('owner', 'admin') );


-- ─────────────────────────────────────────────────────────────────────────────
-- §8  Grants — PostgREST needs the table privilege too; RLS then filters rows.
--     anon: SELECT only (may read public content, may write NOTHING).
--     authenticated: full DML, narrowed by the policies above.
--
--     The §3 helpers live in `private`: RLS evaluation requires the querying
--     role to hold EXECUTE, so we grant usage+execute on `private` — but because
--     `private` is not a PostgREST-exposed schema, that grant does NOT publish
--     them as RPC. The §4 functions return `trigger` (never RPC-exposable) and
--     fire regardless of caller EXECUTE, so they need no grant.
-- ─────────────────────────────────────────────────────────────────────────────

grant usage on schema public to anon, authenticated;
grant usage on schema private to anon, authenticated;
grant select on all tables in schema public to anon;
grant select, insert, update, delete on all tables in schema public to authenticated;
grant execute on all functions in schema private to anon, authenticated;

-- ═══════════════════════════════════════════════════════════════════════════
-- Deliberately elsewhere (not authorization, so not here): denormalized counters
-- and their triggers, notification fan-out, media-processing state machine,
-- moderation review workflow, feed fan-out at scale, rate limiting. This file
-- owns exactly one thing — who may see and do what — and says it in policies.
-- ═══════════════════════════════════════════════════════════════════════════

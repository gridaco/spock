// E2E: verify the generated provider tables against a real spock backend.
// usage: node e2e.mjs <baseUrl> <providerModulePath>
import { strict as assert } from "node:assert";

const [base, modulePath] = process.argv.slice(2);
const { SNAPSHOT_QUERY, COMMAND_REFUSALS } = await import(modulePath);

const gql = async (query) => {
  const res = await fetch(`${base}/graphql/v1`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query }),
  });
  assert.equal(res.status, 200, `graphql http ${res.status}`);
  const body = await res.json();
  assert.ok(!body.errors, `graphql errors: ${JSON.stringify(body.errors)}`);
  return body.data;
};

// Measured protocol: success/failure is the HTTP status; the body is the raw row (or {error:{code}}).
const rpc = async (fn, args, actor) => {
  const headers = { "content-type": "application/json" };
  if (actor) headers["x-spock-actor"] = actor;
  const res = await fetch(`${base}/rest/v1/rpc/${fn}`, {
    method: "POST",
    headers,
    body: JSON.stringify(args),
  });
  return { ok: res.ok, body: await res.json() };
};

// 1) The real server accepts the generated snapshot query and the counts match the seed
const data = await gql(SNAPSHOT_QUERY);
const counts = Object.fromEntries(
  Object.entries(data).map(([k, v]) => [k, v.length]),
);
const expected = {
  users: 9, stories: 12, storyViews: 5, posts: 23, slides: 3,
  comments: 13, likes: 52, saves: 3, follows: 36, postTags: 6,
};
assert.deepEqual(counts, expected, `seed counts: ${JSON.stringify(counts)}`);

// 2) Real mutation round-trip: find a (user, post) pair without a like, then like → unlike to restore
const liked = new Set(data.likes.map((l) => `${l.user.id}/${l.post.id}`));
let actor = null;
let post = null;
for (const u of data.users) {
  for (const p of data.posts) {
    if (!liked.has(`${u.id}/${p.id}`)) {
      actor = u.id;
      post = p.id;
      break;
    }
  }
  if (actor) break;
}
assert.ok(actor && post, "free (user,post) pair exists");

const likeReply = await rpc("like_post", { post }, actor);
assert.ok(likeReply.ok, `like_post: ${JSON.stringify(likeReply)}`);
assert.equal(likeReply.body.post, post, "returned row targets the post");
const after = await gql(SNAPSHOT_QUERY);
assert.equal(after.likes.length, expected.likes + 1, "like landed");
const unlikeReply = await rpc("unlike_post", { post }, actor);
assert.ok(unlikeReply.ok, `unlike_post: ${JSON.stringify(unlikeReply)}`);
const restored = await gql(SNAPSHOT_QUERY);
assert.equal(restored.likes.length, expected.likes, "state restored");

// 3) Unauthenticated refusal → the error code must agree with the generated whitelist
const refused = await rpc("like_post", { post }, null);
assert.ok(!refused.ok, "unauthenticated like must be refused");
const code = String(refused.body.error?.code ?? "").replace(/_/g, "-");
assert.ok(
  COMMAND_REFUSALS["feed/like-post"].includes(code),
  `refusal code \`${code}\` must be whitelisted for feed/like-post`,
);

console.log("e2e ok: snapshot counts, like/unlike round-trip, refusal admission");

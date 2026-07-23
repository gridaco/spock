// E2E: 생성된 provider 테이블을 실제 spock 백엔드에 대해 검증한다.
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

// 실측 프로토콜: 성공/실패는 HTTP 상태, 본문은 행(또는 {error:{code}}) 그대로.
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

// 1) 생성된 스냅샷 쿼리를 실서버가 수락하고, 시드 수치와 일치하는가
const data = await gql(SNAPSHOT_QUERY);
const counts = Object.fromEntries(
  Object.entries(data).map(([k, v]) => [k, v.length]),
);
const expected = {
  users: 9, stories: 12, storyViews: 5, posts: 23, slides: 3,
  comments: 13, likes: 52, saves: 3, follows: 36, postTags: 6,
};
assert.deepEqual(counts, expected, `seed counts: ${JSON.stringify(counts)}`);

// 2) 실 뮤테이션 왕복: 아직 좋아요 안 한 (user, post) 쌍을 찾아 like → unlike 복원
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

// 3) 무인증 거절 → 에러 코드가 생성된 화이트리스트와 정합하는가
const refused = await rpc("like_post", { post }, null);
assert.ok(!refused.ok, "unauthenticated like must be refused");
const code = String(refused.body.error?.code ?? "").replace(/_/g, "-");
assert.ok(
  COMMAND_REFUSALS["feed/like-post"].includes(code),
  `refusal code \`${code}\` must be whitelisted for feed/like-post`,
);

console.log("e2e ok: snapshot counts, like/unlike round-trip, refusal admission");

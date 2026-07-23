// 공유 런타임 단위 검증 (가짜 서버): 분기 라우팅, 정산 3경로, 직렬화 큐.
// usage: node runtime-unit.mjs <tablesModulePath> <runtimeModulePath>
import { strict as assert } from "node:assert";

const [tablesPath, runtimePath] = process.argv.slice(2);
const tables = await import(tablesPath);
const { createProvider } = await import(runtimePath);

const log = [];
let rpcDelay = 0;
let rpcScript = () => ({ ok: true, body: { id: "row" } });
const fakeFetch = async (url, init) => {
  if (url.endsWith("/graphql/v1")) {
    log.push("gql");
    return { ok: true, json: async () => ({ data: { posts: [] } }) };
  }
  const fn = url.split("/rpc/")[1];
  log.push(`rpc:${fn}:${init.headers["x-spock-actor"] ?? "-"}`);
  if (rpcDelay) await new Promise((r) => setTimeout(r, rpcDelay));
  const { ok, body } = rpcScript(fn, JSON.parse(init.body));
  return { ok, json: async () => body };
};

const provider = createProvider({ base: "", tables, fetchImpl: fakeFetch });

// 1) 분기: liked=true → like_post / liked=false → unlike_post, 인자 = post id
let r = await provider.dispatch("SetLike", { post: "P1", liked: true }, "U1");
assert.equal(r.settlement, "accepted");
r = await provider.dispatch("SetLike", { post: "P1", liked: false }, "U1");
assert.equal(r.settlement, "accepted");
assert.deepEqual(
  log.filter((l) => l.startsWith("rpc")),
  ["rpc:like_post:U1", "rpc:unlike_post:U1"],
);

// 2) 정산: 수락 경로는 반드시 RPC 후 재스냅샷
assert.deepEqual(log, ["rpc:like_post:U1", "gql", "rpc:unlike_post:U1", "gql"]);

// 3) 거절 — 화이트리스트 안: 선언된 이유가 그대로 나온다
rpcScript = () => ({ ok: false, body: { error: { code: "not_authorized" } } });
r = await provider.dispatch("SetLike", { post: "P1", liked: true }, null);
assert.deepEqual(r, { settlement: "refused", reason: "not-authorized", declared: true });

// 4) 거절 — 화이트리스트 밖: 일반 refused로 뭉개진다 (내부 유출 방지)
rpcScript = () => ({ ok: false, body: { error: { code: "disk_on_fire" } } });
r = await provider.dispatch("SetLike", { post: "P1", liked: true }, "U1");
assert.deepEqual(r, { settlement: "refused", reason: "refused", declared: false });

// 5) 로컬 모드: RPC 없이 수락
const before = log.length;
r = await provider.dispatch("LoadMore", {}, "U1");
assert.deepEqual(r, { settlement: "accepted", local: true });
assert.equal(log.length, before, "local mutation performs no fetch");

// 6) 직렬화: 두 디스패치가 겹치지 않는다 (첫 정산 완료 후 둘째 RPC)
log.length = 0;
rpcScript = () => ({ ok: true, body: {} });
rpcDelay = 30;
const p1 = provider.dispatch("SetLike", { post: "A", liked: true }, "U1");
const p2 = provider.dispatch("SetSave", { post: "B", saved: true }, "U1");
await Promise.all([p1, p2]);
assert.deepEqual(
  log,
  ["rpc:like_post:U1", "gql", "rpc:save_post:U1", "gql"],
  "queue serializes settlements",
);

console.log("runtime ok: branching, settlement x3, local, serialization");
